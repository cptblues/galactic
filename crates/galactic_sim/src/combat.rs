// MVP-025-B: pure deterministic combat and atomic strategic application.
// COMBAT-001-A: split into combat/{state,doctrine,rounds} — this root file keeps
// the snapshot/report types and the resolve_combat façade; the tactical round
// engine lives in the submodules (mirrors the mission.rs/mission/ pattern).
use std::collections::BTreeMap;
use std::fmt;

use galactic_domain::{FleetId, MissionId, Owner, PlanetId, ResourceStock};
use serde::Deserialize;

use crate::{
    CombatTargetBonuses, CombatTargetClass, CraftableCatalog, CraftableId, FleetComposition,
    FleetState, GameState, NEUTRAL_COMBAT_BONUS_PER_MILLE, PlanetaryForceLoss, PlanetaryForceStack,
    PlanetaryIntelPrecision, PlanetaryPresence, PlanetaryPresenceRules, ShipStack, StrategicTick,
    default_ruleset, refresh_planetary_intelligence,
};

mod ai;
mod doctrine;
mod intel;
mod rounds;
mod state;

use ai::{CombatAiRules, CombatAiRulesConfig};
pub use doctrine::CombatDoctrineId;
use doctrine::{CombatTacticsRules, CombatTacticsRulesConfig};
use intel::{CombatIntelRules, CombatIntelRulesConfig};
use state::effective_hull;

pub const MAX_COMBAT_SHIP_DEFINITIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatShipDefinition {
    pub craftable: CraftableId,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
    pub target_class: CombatTargetClass,
    pub bonuses: CombatTargetBonuses,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatRules {
    version: u32,
    maximum_rounds: u16,
    damage_scale: u32,
    defense_weight_per_mille: u32,
    damage_variance_per_mille: u32,
    salvage_per_destroyed_defender: ResourceStock,
    ships: BTreeMap<CraftableId, CombatShipDefinition>,
    tactics: CombatTacticsRules,
    intel: CombatIntelRules,
    ai: CombatAiRules,
}

impl CombatRules {
    pub(crate) fn from_config(
        config: CombatRulesConfig,
        craftables: &CraftableCatalog,
        planetary_presence: &PlanetaryPresenceRules,
    ) -> Result<Self, CombatRulesError> {
        if config.version != 4 {
            return Err(CombatRulesError::UnsupportedVersion(config.version));
        }
        if config.maximum_rounds == 0 {
            return Err(CombatRulesError::InvalidMaximumRounds);
        }
        if config.damage_scale == 0 {
            return Err(CombatRulesError::InvalidDamageScale);
        }
        if config.defense_weight_per_mille > 10_000 {
            return Err(CombatRulesError::InvalidDefenseWeight);
        }
        if config.damage_variance_per_mille > 500 {
            return Err(CombatRulesError::InvalidDamageVariance);
        }
        if config.salvage_per_destroyed_defender.into_stock().is_zero() {
            return Err(CombatRulesError::InvalidSalvage);
        }
        let mut ships = BTreeMap::new();
        for craftable in craftables.definitions() {
            let Some(ship) = craftable.ship else {
                continue;
            };
            let Some(combat) = ship.combat else {
                continue;
            };
            ships.insert(
                craftable.id,
                CombatShipDefinition {
                    craftable: craftable.id,
                    offense: combat.offense,
                    defense: combat.defense,
                    durability: combat.durability,
                    target_class: combat.target_class,
                    bonuses: combat.bonuses,
                },
            );
        }
        if ships.is_empty() || ships.len() > MAX_COMBAT_SHIP_DEFINITIONS {
            return Err(CombatRulesError::InvalidShipCount {
                found: ships.len(),
                maximum: MAX_COMBAT_SHIP_DEFINITIONS,
            });
        }

        if planetary_presence.definitions().any(|definition| {
            definition.offense == 0 || definition.defense == 0 || definition.durability == 0
        }) {
            return Err(CombatRulesError::InvalidPlanetaryForceCatalog);
        }

        let tactics = CombatTacticsRules::from_config(config.tactics)?;
        let intel = CombatIntelRules::from_config(config.intel)?;
        let ai = CombatAiRules::from_config(config.ai)?;

        Ok(Self {
            version: config.version,
            maximum_rounds: config.maximum_rounds,
            damage_scale: config.damage_scale,
            defense_weight_per_mille: config.defense_weight_per_mille,
            damage_variance_per_mille: config.damage_variance_per_mille,
            salvage_per_destroyed_defender: config.salvage_per_destroyed_defender.into_stock(),
            ships,
            tactics,
            intel,
            ai,
        })
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn maximum_rounds(&self) -> u16 {
        self.maximum_rounds
    }

    pub(crate) const fn tactics(&self) -> &CombatTacticsRules {
        &self.tactics
    }

    pub(crate) const fn intel(&self) -> &CombatIntelRules {
        &self.intel
    }

    pub(crate) const fn ai(&self) -> &CombatAiRules {
        &self.ai
    }

    pub fn ship(&self, craftable: CraftableId) -> Option<&CombatShipDefinition> {
        self.ships.get(&craftable)
    }

    pub fn ships(&self) -> impl Iterator<Item = &CombatShipDefinition> {
        self.ships.values()
    }

    pub fn is_combat_fleet(&self, fleet: &FleetState) -> bool {
        fleet
            .composition
            .entries()
            .all(|stack| stack.quantity > 0 && self.ships.contains_key(&stack.craftable))
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("combat:");
        output.push_str(&self.version.to_string());
        output.push_str(";ships:");
        for definition in self.ships.values() {
            output.push_str(definition.craftable.key());
            output.push(':');
            output.push_str(definition.target_class.structural_key());
            output.push(':');
            for (target_class, multiplier) in definition.bonuses.entries() {
                if multiplier != NEUTRAL_COMBAT_BONUS_PER_MILLE {
                    output.push_str(target_class.structural_key());
                    output.push(',');
                }
            }
            output.push(';');
        }
        output.push_str("tactics:");
        output.push_str(&doctrine::ALL_COMBAT_DOCTRINES.len().to_string());
        output.push_str(";counters:");
        output.push_str(&self.tactics.counters_len().to_string());
        output.push_str(";intel:1;ai:1;");
    }

    fn snapshot_fleet(
        &self,
        fleet: &FleetState,
    ) -> Result<CombatFleetSnapshot, CombatSnapshotError> {
        if !self.is_combat_fleet(fleet) {
            return Err(CombatSnapshotError::FleetNotCombatCapable(fleet.id));
        }
        let capabilities = fleet
            .capabilities()
            .map_err(|_| CombatSnapshotError::InvalidFleet(fleet.id))?;
        let ships = fleet
            .composition
            .entries()
            .map(|stack| {
                let definition = self
                    .ship(stack.craftable)
                    .expect("a validated combat fleet has a combat definition");
                CombatShipStack {
                    craftable: stack.craftable,
                    quantity: stack.quantity,
                    offense: definition.offense,
                    defense: definition.defense,
                    durability: definition.durability,
                    target_class: definition.target_class,
                    bonuses: definition.bonuses,
                }
            })
            .collect();
        Ok(CombatFleetSnapshot {
            fleet_id: fleet.id,
            owner: fleet.owner,
            ships,
            cargo: fleet.cargo,
            cargo_capacity: capabilities.cargo_capacity,
        })
    }

    pub fn fleet_power(&self, fleet: &FleetState) -> Option<CombatFleetPower> {
        self.snapshot_fleet(fleet)
            .ok()
            .map(|snapshot| fleet_power_from_snapshot(&snapshot, self))
    }

    fn snapshot_defender(&self, presence: &PlanetaryPresence) -> PlanetDefenseSnapshot {
        PlanetDefenseSnapshot {
            planet_id: presence.planet_id,
            occupant: presence.occupant,
            population: presence.population,
            forces: presence.forces.clone(),
            revision: presence.revision,
        }
    }
}

pub fn combat_rules() -> &'static CombatRules {
    default_ruleset().combat()
}

pub fn combat_fleet_power(fleet: &FleetState) -> Option<CombatFleetPower> {
    combat_rules().fleet_power(fleet)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatRulesError {
    UnsupportedVersion(u32),
    InvalidMaximumRounds,
    InvalidDamageScale,
    InvalidDefenseWeight,
    InvalidDamageVariance,
    InvalidSalvage,
    InvalidShipCount { found: usize, maximum: usize },
    UnknownCraftable,
    CraftableIsNotShip(CraftableId),
    CraftableIsNotMilitary(CraftableId),
    InvalidShipStats(CraftableId),
    DuplicateCraftable(CraftableId),
    InvalidPlanetaryForceCatalog,
    InvalidTacticsVersion(u32),
    InvalidDoctrineCount { found: usize, expected: usize },
    DuplicateDoctrine(CombatDoctrineId),
    MissingDoctrine(CombatDoctrineId),
    InvalidDoctrineMultiplier { doctrine: CombatDoctrineId },
    BalancedEngagementMustBeRepetitionExempt,
    InvalidCounterCount { found: usize, expected: usize },
    InvalidCounterMultiplier { doctrine: CombatDoctrineId },
    DuplicateCounter(CombatDoctrineId),
    InvalidRepetitionPenalty,
    InvalidSupportProtection,
    InvalidFlankingBonus,
    InvalidDispersionCap,
    InvalidConcentrationBonus,
    InvalidIntelVersion(u32),
    InvalidIntelBasePercent,
    InvalidIntelStaleness,
    InvalidIntelGain,
    InvalidIntelBounds,
    InvalidAiVersion(u32),
    InvalidAiScore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatSnapshotError {
    UnknownFleet(FleetId),
    InvalidFleet(FleetId),
    FleetNotCombatCapable(FleetId),
    UnknownPlanet(PlanetId),
    UnoccupiedTarget(PlanetId),
    FriendlyTarget(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatShipStack {
    pub craftable: CraftableId,
    pub quantity: u64,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
    pub target_class: CombatTargetClass,
    pub bonuses: CombatTargetBonuses,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatFleetSnapshot {
    pub fleet_id: FleetId,
    pub owner: Owner,
    pub ships: Vec<CombatShipStack>,
    pub cargo: ResourceStock,
    pub cargo_capacity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatFleetPower {
    pub ships: u64,
    pub offense: u64,
    pub hull: u64,
    pub light_ships: u64,
    pub medium_ships: u64,
    pub heavy_ships: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanetDefenseSnapshot {
    pub planet_id: PlanetId,
    pub occupant: Owner,
    pub population: u64,
    pub forces: Vec<PlanetaryForceStack>,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttackMissionCommitment {
    pub seed: u64,
    pub attacker: CombatFleetSnapshot,
    pub defender: PlanetDefenseSnapshot,
}

pub fn prepare_attack_commitment(
    state: &GameState,
    fleet_id: FleetId,
    planet_id: PlanetId,
    seed: u64,
) -> Result<AttackMissionCommitment, CombatSnapshotError> {
    let fleet = state
        .fleet(fleet_id)
        .ok_or(CombatSnapshotError::UnknownFleet(fleet_id))?;
    let attacker = combat_rules().snapshot_fleet(fleet)?;
    let presence = state
        .planetary_presence(planet_id)
        .ok_or(CombatSnapshotError::UnknownPlanet(planet_id))?;
    let Owner::Faction(occupant) = presence.occupant else {
        return Err(CombatSnapshotError::UnoccupiedTarget(planet_id));
    };
    if attacker.owner == Owner::Faction(occupant) {
        return Err(CombatSnapshotError::FriendlyTarget(planet_id));
    }
    Ok(AttackMissionCommitment {
        seed,
        attacker,
        defender: combat_rules().snapshot_defender(presence),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatOutcome {
    AttackerVictory,
    DefenderVictory,
    Stalemate,
    MutualDestruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatControlChange {
    Unchanged,
    Secured { previous: Owner, current: Owner },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatShipLoss {
    pub craftable: CraftableId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatResolution {
    pub outcome: CombatOutcome,
    pub rounds: u16,
    pub attacker_losses: Vec<CombatShipLoss>,
    pub attacker_survivors: Vec<CombatShipStack>,
    pub defender_losses: Vec<PlanetaryForceLoss>,
    pub defender_survivors: Vec<PlanetaryForceStack>,
    pub attacker_damage: u64,
    pub defender_damage: u64,
    pub salvage_recoverable: ResourceStock,
    pub salvage_recovered: ResourceStock,
    pub control: CombatControlChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AttackInvalidReason {
    TargetOwnerChanged,
    TargetPresenceChanged,
    AttackerFleetChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatReportStatus {
    Resolved(CombatResolution),
    TargetInvalid(AttackInvalidReason),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatReport {
    pub mission_id: MissionId,
    pub planet_id: PlanetId,
    pub resolved_at: StrategicTick,
    pub rules_version: u32,
    pub seed: u64,
    pub attacker: CombatFleetSnapshot,
    pub defender: PlanetDefenseSnapshot,
    pub status: CombatReportStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AttackMissionOutcome {
    Resolved(CombatOutcome),
    TargetInvalid(AttackInvalidReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttackMissionResult {
    pub target: PlanetId,
    pub outcome: AttackMissionOutcome,
    pub secured: bool,
    pub attackers_destroyed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatApplicationError {
    AlreadyApplied(MissionId),
    UnknownFleet(FleetId),
    UnknownPlanet(PlanetId),
    RevisionOverflow(PlanetId),
    InvalidSurvivingFleet(FleetId),
    CargoOverflow(FleetId),
}

/// Runs the same round-by-round tactical engine used by (a future)
/// interactive combat screen to completion, picking both sides' doctrine via
/// the rule-based AI profile (`ai::choose_ai_doctrine`) each round — the
/// single implementation COMBAT-001 doc §8 requires, rather than a separate
/// aggregate calculation for auto-resolution. Both sides use the same AI
/// profile since no interactive player choice exists yet (COMBAT-001-C/D)
/// and per-faction differentiation is out of scope for B (see `ai.rs`'s
/// module doc).
pub(crate) fn resolve_combat(
    attacker: &CombatFleetSnapshot,
    defender: &PlanetDefenseSnapshot,
    seed: u64,
    rules: &CombatRules,
    planetary_rules: &PlanetaryPresenceRules,
    intel_precision: intel::CombatIntelPrecision,
    intel_report_age_ticks: u64,
) -> CombatResolution {
    let mut combat_state = state::prepare_fleet_combat(
        attacker,
        defender,
        seed,
        rules,
        planetary_rules,
        intel_precision,
        intel_report_age_ticks,
    )
    .expect("a commitment valid enough to reach resolve_combat always prepares successfully");

    while combat_state.phase != state::FleetCombatPhase::Completed {
        let round = combat_state.round.saturating_add(1);
        let attacker_doctrine = ai::choose_ai_doctrine(
            &combat_state.attacker,
            &combat_state.defender,
            round,
            rules.ai(),
        );
        let defender_doctrine = ai::choose_ai_doctrine(
            &combat_state.defender,
            &combat_state.attacker,
            round,
            rules.ai(),
        );
        let resolution = rounds::resolve_combat_round(
            &combat_state,
            attacker_doctrine,
            defender_doctrine,
            rules,
            rules.tactics(),
        )
        .expect("the loop only resolves rounds while the combat is not yet completed");
        rounds::apply_round_resolution(&mut combat_state, resolution)
            .expect("the loop only applies rounds while the combat is not yet completed");
    }

    state::finalize_fleet_combat(
        &combat_state,
        attacker.cargo,
        attacker.cargo_capacity,
        rules,
    )
    .expect("a completed, non-retreated combat always finalizes")
}

/// Gathers the two intel inputs only the live `GameState` can provide
/// (precision and age of the defender's `PlanetaryIntelligenceReport` at the
/// moment combat resolves, i.e. mission arrival — not launch, so staleness
/// reflects the real gap). The reconnaissance-ship bonus needs no live state
/// (derived from the attacker snapshot already in hand, see
/// `state::prepare_fleet_combat`). Missing report is a defensive fallback,
/// not an expected path — attacking already requires an analyzed target.
fn intel_inputs_from_state(
    state: &GameState,
    planet_id: PlanetId,
    resolved_at: StrategicTick,
) -> (intel::CombatIntelPrecision, u64) {
    let Some(report) = state.planetary_intelligence_report(planet_id) else {
        return (intel::CombatIntelPrecision::Contact, 0);
    };
    let precision = match report.precision {
        PlanetaryIntelPrecision::Contact => intel::CombatIntelPrecision::Contact,
        PlanetaryIntelPrecision::Surveyed => intel::CombatIntelPrecision::Surveyed,
        PlanetaryIntelPrecision::Exact => intel::CombatIntelPrecision::Exact,
    };
    let age_ticks = resolved_at
        .value()
        .saturating_sub(report.observed_at.value());
    (precision, age_ticks)
}

pub fn resolve_and_apply_attack(
    state: &mut GameState,
    mission_id: MissionId,
    resolved_at: StrategicTick,
    commitment: &AttackMissionCommitment,
) -> Result<(CombatReport, AttackMissionResult), CombatApplicationError> {
    if state
        .combat_reports
        .iter()
        .any(|report| report.mission_id == mission_id)
    {
        return Err(CombatApplicationError::AlreadyApplied(mission_id));
    }

    let current_fleet =
        state
            .fleet(commitment.attacker.fleet_id)
            .ok_or(CombatApplicationError::UnknownFleet(
                commitment.attacker.fleet_id,
            ))?;
    let current_presence = state
        .planetary_presence(commitment.defender.planet_id)
        .ok_or(CombatApplicationError::UnknownPlanet(
            commitment.defender.planet_id,
        ))?;

    let invalid = if current_presence.occupant != commitment.defender.occupant {
        Some(AttackInvalidReason::TargetOwnerChanged)
    } else if current_presence.revision != commitment.defender.revision
        || current_presence.forces != commitment.defender.forces
    {
        Some(AttackInvalidReason::TargetPresenceChanged)
    } else if !fleet_matches_snapshot(current_fleet, &commitment.attacker) {
        Some(AttackInvalidReason::AttackerFleetChanged)
    } else {
        None
    };
    if let Some(reason) = invalid {
        let report = CombatReport {
            mission_id,
            planet_id: commitment.defender.planet_id,
            resolved_at,
            rules_version: combat_rules().version,
            seed: commitment.seed,
            attacker: commitment.attacker.clone(),
            defender: commitment.defender.clone(),
            status: CombatReportStatus::TargetInvalid(reason),
        };
        insert_combat_report(state, report.clone());
        return Ok((
            report,
            AttackMissionResult {
                target: commitment.defender.planet_id,
                outcome: AttackMissionOutcome::TargetInvalid(reason),
                secured: false,
                attackers_destroyed: false,
            },
        ));
    }

    let (intel_precision, intel_report_age_ticks) =
        intel_inputs_from_state(state, commitment.defender.planet_id, resolved_at);
    let resolution = resolve_combat(
        &commitment.attacker,
        &commitment.defender,
        commitment.seed,
        combat_rules(),
        default_ruleset().planetary_presence(),
        intel_precision,
        intel_report_age_ticks,
    );
    let mut candidate = state.clone();
    let planet_id = commitment.defender.planet_id;
    let next_revision = candidate
        .planetary_presence(planet_id)
        .expect("the target presence was validated")
        .revision
        .checked_add(1)
        .ok_or(CombatApplicationError::RevisionOverflow(planet_id))?;
    {
        let presence = candidate
            .planetary_presence_mut(planet_id)
            .expect("the target presence was validated");
        presence.forces = resolution.defender_survivors.clone();
        presence.revision = next_revision;
        if let CombatControlChange::Secured { current, .. } = resolution.control {
            presence.occupant = current;
        }
    }

    if resolution.attacker_survivors.is_empty() {
        candidate
            .fleets
            .retain(|fleet| fleet.id != commitment.attacker.fleet_id);
    } else {
        let composition = FleetComposition::from_stacks(
            resolution
                .attacker_survivors
                .iter()
                .map(|stack| ShipStack::new(stack.craftable, stack.quantity)),
        )
        .map_err(|_| CombatApplicationError::InvalidSurvivingFleet(commitment.attacker.fleet_id))?;
        let fleet = candidate
            .fleet_mut(commitment.attacker.fleet_id)
            .expect("the attacker fleet was validated");
        fleet.composition = composition;
        fleet.cargo = fleet
            .cargo
            .checked_add(resolution.salvage_recovered)
            .ok_or(CombatApplicationError::CargoOverflow(fleet.id))?;
    }
    refresh_planetary_intelligence(
        &mut candidate,
        planet_id,
        PlanetaryIntelPrecision::Exact,
        resolved_at,
    )
    .expect("the combat target has a validated planetary presence");

    let report = CombatReport {
        mission_id,
        planet_id,
        resolved_at,
        rules_version: combat_rules().version,
        seed: commitment.seed,
        attacker: commitment.attacker.clone(),
        defender: commitment.defender.clone(),
        status: CombatReportStatus::Resolved(resolution.clone()),
    };
    insert_combat_report(&mut candidate, report.clone());
    let result = AttackMissionResult {
        target: planet_id,
        outcome: AttackMissionOutcome::Resolved(resolution.outcome),
        secured: matches!(resolution.control, CombatControlChange::Secured { .. }),
        attackers_destroyed: resolution.attacker_survivors.is_empty(),
    };
    *state = candidate;
    Ok((report, result))
}

fn insert_combat_report(state: &mut GameState, report: CombatReport) {
    let index = state
        .combat_reports
        .binary_search_by_key(&report.mission_id, |existing| existing.mission_id)
        .unwrap_or_else(|index| index);
    state.combat_reports.insert(index, report);
}

fn fleet_matches_snapshot(fleet: &FleetState, snapshot: &CombatFleetSnapshot) -> bool {
    fleet.id == snapshot.fleet_id
        && fleet.owner == snapshot.owner
        && fleet.cargo == snapshot.cargo
        && fleet
            .composition
            .entries()
            .map(|stack| (stack.craftable, stack.quantity))
            .eq(snapshot
                .ships
                .iter()
                .map(|stack| (stack.craftable, stack.quantity)))
}

fn fleet_power_from_snapshot(
    snapshot: &CombatFleetSnapshot,
    rules: &CombatRules,
) -> CombatFleetPower {
    let mut power = CombatFleetPower {
        ships: 0,
        offense: 0,
        hull: 0,
        light_ships: 0,
        medium_ships: 0,
        heavy_ships: 0,
    };
    for stack in &snapshot.ships {
        power.ships = power.ships.saturating_add(stack.quantity);
        power.offense = power.offense.saturating_add(clamp_u128(
            u128::from(stack.offense).saturating_mul(u128::from(stack.quantity)),
        ));
        power.hull = power.hull.saturating_add(clamp_u128(
            effective_hull(
                stack.defense,
                stack.durability,
                rules.defense_weight_per_mille,
            )
            .saturating_mul(u128::from(stack.quantity)),
        ));
        match stack.target_class {
            CombatTargetClass::Light => {
                power.light_ships = power.light_ships.saturating_add(stack.quantity);
            }
            CombatTargetClass::Medium => {
                power.medium_ships = power.medium_ships.saturating_add(stack.quantity);
            }
            CombatTargetClass::Heavy => {
                power.heavy_ships = power.heavy_ships.saturating_add(stack.quantity);
            }
        }
    }
    power
}

fn cap_salvage(recoverable: ResourceStock, current: ResourceStock, capacity: u64) -> ResourceStock {
    let occupied = current
        .metal
        .saturating_add(current.crystal)
        .saturating_add(current.fuel);
    let mut remaining = capacity.saturating_sub(occupied);
    let metal = recoverable.metal.min(remaining);
    remaining -= metal;
    let crystal = recoverable.crystal.min(remaining);
    remaining -= crystal;
    let fuel = recoverable.fuel.min(remaining);
    ResourceStock::new(metal, crystal, fuel)
}

fn multiply_stock(stock: ResourceStock, multiplier: u64) -> ResourceStock {
    ResourceStock::new(
        stock.metal.saturating_mul(multiplier),
        stock.crystal.saturating_mul(multiplier),
        stock.fuel.saturating_mul(multiplier),
    )
}

fn clamp_u128(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

impl fmt::Display for CombatOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AttackerVictory => "attacker victory",
            Self::DefenderVictory => "defender victory",
            Self::Stalemate => "stalemate",
            Self::MutualDestruction => "mutual destruction",
        })
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CombatRulesConfig {
    version: u32,
    maximum_rounds: u16,
    damage_scale: u32,
    defense_weight_per_mille: u32,
    damage_variance_per_mille: u32,
    salvage_per_destroyed_defender: CombatResourceConfig,
    tactics: CombatTacticsRulesConfig,
    intel: CombatIntelRulesConfig,
    ai: CombatAiRulesConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct CombatResourceConfig {
    metal: u64,
    crystal: u64,
    fuel: u64,
}

impl CombatResourceConfig {
    const fn into_stock(self) -> ResourceStock {
        ResourceStock::new(self.metal, self.crystal, self.fuel)
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FactionId, UniverseConfig};

    use crate::{CraftableId, FleetLocation, FleetState, PlanetaryForceId, Simulation};

    use super::*;

    fn snapshot(
        fleet_id: u64,
        owner: FactionId,
        quantity: u64,
        offense: u32,
        defense: u32,
        durability: u32,
    ) -> CombatFleetSnapshot {
        CombatFleetSnapshot {
            fleet_id: FleetId::new(fleet_id),
            owner: Owner::Faction(owner),
            ships: vec![CombatShipStack {
                craftable: CraftableId::FRIGATE_BULWARK,
                quantity,
                offense,
                defense,
                durability,
                target_class: CombatTargetClass::Medium,
                bonuses: CombatTargetBonuses::default(),
            }],
            cargo: ResourceStock::ZERO,
            cargo_capacity: 1_000,
        }
    }

    fn defense(
        planet_id: PlanetId,
        owner: FactionId,
        force: PlanetaryForceId,
        quantity: u32,
    ) -> PlanetDefenseSnapshot {
        PlanetDefenseSnapshot {
            planet_id,
            occupant: Owner::Faction(owner),
            population: 5_000,
            forces: vec![PlanetaryForceStack {
                definition_id: force,
                quantity,
            }],
            revision: 0,
        }
    }

    fn specialist_stack(
        offense: u32,
        target_class: CombatTargetClass,
        bonuses: CombatTargetBonuses,
    ) -> state::CombatStackState {
        state::CombatStackState {
            stack_id: state::CombatStackId(0),
            source: state::CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
            initial_quantity: 1,
            surviving_quantity: 1,
            current_hull: 1_000,
            maximum_hull: 1_000,
            offense,
            defense: 40,
            durability: 40,
            target_class,
            bonuses,
            tactical_role: state::CombatTacticalRole::Line,
        }
    }

    fn defender_stack(
        definition_id: PlanetaryForceId,
        target_class: CombatTargetClass,
    ) -> state::CombatStackState {
        state::CombatStackState {
            stack_id: state::CombatStackId(1),
            source: state::CombatUnitRef::PlanetaryForce(definition_id),
            initial_quantity: 10,
            surviving_quantity: 10,
            current_hull: 1_000,
            maximum_hull: 1_000,
            offense: 20,
            defense: 20,
            durability: 20,
            target_class,
            bonuses: CombatTargetBonuses::default(),
            tactical_role: state::CombatTacticalRole::Line,
        }
    }

    #[test]
    fn default_combat_rules_are_sourced_from_military_craftables() {
        let rules = combat_rules();

        assert!(rules.ship(CraftableId::NEEDLE_INTERCEPTOR).is_some());
        assert!(rules.ship(CraftableId::FRIGATE_BULWARK).is_some());
        assert!(rules.ship(CraftableId::BASTION_CRUISER).is_some());
        assert!(rules.ship(CraftableId::LIGHT_CARGO).is_none());
        assert_eq!(
            rules
                .ship(CraftableId::NEEDLE_INTERCEPTOR)
                .expect("interceptor combat stats exist")
                .bonuses
                .multiplier_for(CombatTargetClass::Light),
            1_400,
        );
    }

    #[test]
    fn offensive_bonus_depends_on_defender_target_class() {
        // Successor of the legacy aggregate `attacker_totals`'s target-class
        // blending, now `rounds::offense_pool` (per-round, per-stack) — same
        // property, same fixture shape, adapted to `CombatStackState`.
        let planetary = default_ruleset().planetary_presence();
        let light_force = planetary
            .id_by_key("confins_militia")
            .expect("default light force exists");
        let heavy_force = planetary
            .id_by_key("local_bastion")
            .expect("default heavy force exists");
        let mut light_bonuses = CombatTargetBonuses::default();
        light_bonuses.light_per_mille = 1_400;
        let mut heavy_bonuses = CombatTargetBonuses::default();
        heavy_bonuses.heavy_per_mille = 1_400;
        let light_specialist = vec![specialist_stack(
            100,
            CombatTargetClass::Light,
            light_bonuses,
        )];
        let heavy_specialist = vec![specialist_stack(
            100,
            CombatTargetClass::Heavy,
            heavy_bonuses,
        )];
        let light_defender = vec![defender_stack(light_force, CombatTargetClass::Light)];
        let heavy_defender = vec![defender_stack(heavy_force, CombatTargetClass::Heavy)];

        assert!(
            rounds::offense_pool(&light_specialist, &light_defender)
                > rounds::offense_pool(&heavy_specialist, &light_defender)
        );
        assert!(
            rounds::offense_pool(&heavy_specialist, &heavy_defender)
                > rounds::offense_pool(&light_specialist, &heavy_defender)
        );
    }

    #[test]
    fn identical_seed_and_snapshots_produce_identical_report() {
        let rules = combat_rules();
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("confins_guard")
            .expect("default force exists");
        let attacker = snapshot(0, FactionId::new(0), 3, 70, 45, 60);
        let defender = defense(
            PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            FactionId::new(2),
            force,
            8,
        );

        let first = resolve_combat(
            &attacker,
            &defender,
            42,
            rules,
            planetary,
            intel::CombatIntelPrecision::Surveyed,
            0,
        );
        let second = resolve_combat(
            &attacker,
            &defender,
            42,
            rules,
            planetary,
            intel::CombatIntelPrecision::Surveyed,
            0,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn simultaneous_lethal_damage_is_mutual_destruction() {
        let rules = combat_rules();
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("confins_guard")
            .expect("default force exists");
        let attacker = snapshot(0, FactionId::new(0), 1, 10_000, 1, 1);
        let defender = defense(
            PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            FactionId::new(2),
            force,
            1,
        );

        let report = resolve_combat(
            &attacker,
            &defender,
            7,
            rules,
            planetary,
            intel::CombatIntelPrecision::Surveyed,
            0,
        );

        assert_eq!(report.outcome, CombatOutcome::MutualDestruction);
        assert!(report.attacker_survivors.is_empty());
        assert!(report.defender_survivors.is_empty());
        assert_eq!(report.control, CombatControlChange::Unchanged);
    }

    #[test]
    fn applying_a_report_twice_is_rejected_without_mutation() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                presence.occupant == Owner::Faction(FactionId::new(2))
                    && !presence.forces.is_empty()
            })
            .expect("a hostile target exists")
            .planet_id;
        let colony_id = ColonyId::new(0);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::FRIGATE_BULWARK, 3)])
                .expect("combat composition");
        simulation.state_mut().fleets.push(FleetState {
            id: FleetId::new(0),
            name: "Force de test".to_string(),
            owner: Owner::Faction(FactionId::new(0)),
            location: FleetLocation::Docked(colony_id),
            composition,
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Idle,
        });
        simulation.state_mut().next_fleet_id = 1;
        let commitment = prepare_attack_commitment(simulation.state(), FleetId::new(0), target, 91)
            .expect("attack snapshot");
        let mission_id = MissionId::new(0);
        resolve_and_apply_attack(
            simulation.state_mut(),
            mission_id,
            StrategicTick::new(10),
            &commitment,
        )
        .expect("first application");
        let after_first = simulation.state().clone();

        assert_eq!(
            resolve_and_apply_attack(
                simulation.state_mut(),
                mission_id,
                StrategicTick::new(10),
                &commitment,
            ),
            Err(CombatApplicationError::AlreadyApplied(mission_id)),
        );
        assert_eq!(simulation.state(), &after_first);
    }
}

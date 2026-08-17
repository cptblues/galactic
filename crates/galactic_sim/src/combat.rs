// MVP-025-B: pure deterministic combat and atomic strategic application.
// COMBAT-001-A: split into combat/{state,doctrine,rounds} — this root file keeps
// the snapshot/report types and the resolve_combat façade; the tactical round
// engine lives in the submodules (mirrors the mission.rs/mission/ pattern).
//
// COMBAT-001-C: an attack mission's arrival no longer resolves synchronously.
// `begin_attack` starts a `PendingCombat` (combat/session.rs) that persists
// across ticks *and* saves, and only `GameCommand::ChooseCombatDoctrine`/
// `RetreatFromCombat`/`AutoResolveCombat` (dispatched through
// `Simulation::apply_command`) can advance or finalize it —
// `apply_combat_resolution` is the single atomic write-back all three share.
// COMBAT-001-D: `galactic_client`'s `combat_ui` module drives these commands
// from a real per-round decision screen — the engine underneath is
// unchanged from C.
use std::collections::BTreeMap;
use std::fmt;

use galactic_domain::{FleetId, MissionId, Owner, PlanetId, ResourceStock};
use serde::Deserialize;

use crate::{
    CombatTargetBonuses, CombatTargetClass, CraftableCatalog, CraftableId, FleetComposition,
    FleetState, GameState, MissionResult, NEUTRAL_COMBAT_BONUS_PER_MILLE, PlanetaryForceLoss,
    PlanetaryForceStack, PlanetaryIntelPrecision, PlanetaryPresence, PlanetaryPresenceRules,
    ShipStack, StrategicTick, default_ruleset, refresh_planetary_intelligence,
};

mod ai;
mod doctrine;
mod intel;
mod plan;
mod retreat;
mod rounds;
mod session;
mod state;
mod view;

use ai::{CombatAiRules, CombatAiRulesConfig};
pub use doctrine::{ALL_COMBAT_DOCTRINES, CombatDoctrineId};
use doctrine::{CombatTacticsRules, CombatTacticsRulesConfig};
use intel::{CombatIntelRules, CombatIntelRulesConfig};
pub use intel::{
    CombatIntelTier, CombatStackView, IntegrityBand, IntegrityReveal, QuantityBand, QuantityReveal,
    ThreatLevel, intel_tier,
};
pub use plan::{
    CombatGroupPlan, CombatGroupPlanId, CombatGroupRole, CombatIntervention,
    CombatInterventionEffect, CombatInterventionRecord, CombatPlan, CombatPlanValidationError,
    CombatTargetPriority, MAX_COMBAT_GROUPS,
};
use retreat::{CombatRetreatRules, CombatRetreatRulesConfig};
pub use session::{
    CombatAutoResolveRejected, CombatCommandError, CombatCompleted, CombatDecisionRequired,
    CombatDoctrineRejected, CombatIntelUpdated, CombatInterventionError, CombatPlanConfirmed,
    CombatPlanRejected, CombatRetreatRejected, CombatRoundResolved, PendingCombat,
};
pub(crate) use session::{
    auto_resolve_combat, choose_combat_doctrine, confirm_combat_plan, retreat_from_combat,
};
use state::effective_hull;
pub use state::{
    CombatRoundEvent, CombatRoundRecord, CombatSide, CombatStackExchange, CombatStackId,
    CombatStackLoss, CombatTacticalRole, CombatUnitRef,
};
pub use view::{
    AlliedSideSummary, AlliedStackView, DoctrineOverview, EnemyIntelView, QualitativePrediction,
    RepetitionPenaltyPreview, allied_side_summary, allied_stacks, doctrine_overview, enemy_intel,
    qualitative_prediction, repetition_penalty_preview, round_history,
};

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
    retreat: CombatRetreatRules,
    command: CombatCommandRules,
}

impl CombatRules {
    pub(crate) fn from_config(
        config: CombatRulesConfig,
        craftables: &CraftableCatalog,
        planetary_presence: &PlanetaryPresenceRules,
    ) -> Result<Self, CombatRulesError> {
        if config.version != 6 {
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
        let retreat = CombatRetreatRules::from_config(config.retreat)?;
        let command = CombatCommandRules::from_config(config.command)?;

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
            retreat,
            command,
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

    // Consumed by `rounds::apply_retreat_penalty`, wired in starting
    // COMBAT-001-C's session-orchestration step.
    #[allow(dead_code)]
    pub(crate) const fn retreat(&self) -> &CombatRetreatRules {
        &self.retreat
    }

    pub const fn command(&self) -> &CombatCommandRules {
        &self.command
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

    pub fn has_combat_ships(&self, fleet: &FleetState) -> bool {
        fleet
            .composition
            .entries()
            .any(|stack| stack.quantity > 0 && self.ships.contains_key(&stack.craftable))
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
        output.push_str(";intel:1;ai:1;retreat:1;command:1;");
    }

    fn snapshot_fleet(
        &self,
        fleet: &FleetState,
    ) -> Result<CombatFleetSnapshot, CombatSnapshotError> {
        if !self.has_combat_ships(fleet) {
            return Err(CombatSnapshotError::FleetNotCombatCapable(fleet.id));
        }
        let capabilities = fleet
            .capabilities()
            .map_err(|_| CombatSnapshotError::InvalidFleet(fleet.id))?;
        let ships = fleet
            .composition
            .entries()
            .filter_map(|stack| {
                let definition = self.ship(stack.craftable)?;
                Some(CombatShipStack {
                    craftable: stack.craftable,
                    quantity: stack.quantity,
                    offense: definition.offense,
                    defense: definition.defense,
                    durability: definition.durability,
                    target_class: definition.target_class,
                    bonuses: definition.bonuses,
                })
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
    InvalidRetreatVersion(u32),
    InvalidRetreatPenalty,
    InvalidCommandVersion(u32),
    InvalidCommandPoints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatCommandRules {
    starting_points: u8,
    change_doctrine_cost: u8,
    focus_fire_cost: u8,
    commit_reserve_cost: u8,
}

impl CombatCommandRules {
    fn from_config(config: CombatCommandRulesConfig) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
            return Err(CombatRulesError::InvalidCommandVersion(config.version));
        }
        if config.starting_points == 0
            || config.change_doctrine_cost == 0
            || config.focus_fire_cost == 0
            || config.commit_reserve_cost == 0
        {
            return Err(CombatRulesError::InvalidCommandPoints);
        }
        Ok(Self {
            starting_points: config.starting_points,
            change_doctrine_cost: config.change_doctrine_cost,
            focus_fire_cost: config.focus_fire_cost,
            commit_reserve_cost: config.commit_reserve_cost,
        })
    }

    pub const fn starting_points(&self) -> u8 {
        self.starting_points
    }

    pub const fn change_doctrine_cost(&self) -> u8 {
        self.change_doctrine_cost
    }

    pub const fn focus_fire_cost(&self) -> u8 {
        self.focus_fire_cost
    }

    pub const fn commit_reserve_cost(&self) -> u8 {
        self.commit_reserve_cost
    }
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

/// Which side retreated — a small public-facing mirror of the engine-internal
/// `CombatSide` (kept `pub(crate)`, never promoted to the public API surface;
/// see `combat/state.rs`), so `CombatOutcome::Retreat` doesn't leak an
/// internal engine type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RetreatingSide {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatOutcome {
    AttackerVictory,
    DefenderVictory,
    Stalemate,
    MutualDestruction,
    Retreat { retreating_side: RetreatingSide },
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
    #[serde(default)]
    pub round_history: Vec<CombatRoundRecord>,
    #[serde(default)]
    pub initial_plan: Option<CombatPlan>,
    #[serde(default)]
    pub final_plan: Option<CombatPlan>,
    #[serde(default)]
    pub intervention_history: Vec<CombatInterventionRecord>,
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
    RevisionOverflow(PlanetId),
    InvalidSurvivingFleet(FleetId),
    CargoOverflow(FleetId),
}

/// Superseded as the mission-facing entry point by COMBAT-001-C's
/// `AutoResolveCombat` command (`session::auto_resolve_combat`, which shares
/// this function's own `run_combat_to_completion` engine loop) — kept as a
/// self-contained pure entry point (no `GameState`/`PendingCombat` needed)
/// for tests that only care about the round engine's own properties
/// (determinism, outcome classification, etc.), exercised only by this
/// module's own tests today.
///
/// Runs the same round-by-round tactical engine picking both sides'
/// doctrine via the rule-based AI profile (`ai::choose_ai_doctrine`) each
/// round — the single implementation COMBAT-001 doc §8 requires, rather
/// than a separate aggregate calculation for auto-resolution.
#[allow(dead_code)]
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

    run_combat_to_completion(&mut combat_state, rules);

    state::finalize_fleet_combat(
        &combat_state,
        attacker.cargo,
        attacker.cargo_capacity,
        rules,
    )
    .expect("a completed combat always finalizes")
}

/// Drives an already-`prepare_fleet_combat`'d state to completion by picking
/// both sides' doctrine via the AI profile each round — extracted so
/// COMBAT-001-C's `AutoResolveCombat` command can run the exact same engine
/// on a `FleetCombatState` that may already be mid-fight (not just a freshly
/// prepared one), satisfying doc §21's "l'auto-résolution utilise le même
/// moteur" from both entry points.
fn run_combat_to_completion(combat_state: &mut state::FleetCombatState, rules: &CombatRules) {
    while combat_state.phase != state::FleetCombatPhase::Completed {
        let round = combat_state.round.saturating_add(1);
        let attacker_plan = ai::choose_ai_plan(
            &combat_state.attacker,
            &combat_state.defender,
            round,
            rules.ai(),
        );
        let defender_plan = ai::choose_ai_plan(
            &combat_state.defender,
            &combat_state.attacker,
            round,
            rules.ai(),
        );
        let resolution = rounds::resolve_combat_round(
            combat_state,
            attacker_plan.doctrine,
            defender_plan.doctrine,
            Some(&attacker_plan),
            Some(&defender_plan),
            rules,
            rules.tactics(),
        )
        .expect("the loop only resolves rounds while the combat is not yet completed");
        rounds::apply_round_resolution(combat_state, resolution)
            .expect("the loop only applies rounds while the combat is not yet completed");
    }
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

/// The two ways an attack mission's arrival can go: either the target is no
/// longer valid (resolved immediately, exactly as before COMBAT-001-C) or a
/// real tactical combat begins and waits for a decision (`GameCommand::
/// ChooseCombatDoctrine`/`RetreatFromCombat`/`AutoResolveCombat`).
pub(crate) enum AttackBeginOutcome {
    Invalid(AttackMissionResult),
    Pending,
}

fn commitment_invalidity(
    state: &GameState,
    commitment: &AttackMissionCommitment,
) -> Option<AttackInvalidReason> {
    let current_fleet = state
        .fleet(commitment.attacker.fleet_id)
        .expect("a validated attack mission's fleet still exists at arrival");
    let current_presence = state
        .planetary_presence(commitment.defender.planet_id)
        .expect("a validated attack mission's target still exists at arrival");

    if current_presence.occupant != commitment.defender.occupant {
        Some(AttackInvalidReason::TargetOwnerChanged)
    } else if current_presence.revision != commitment.defender.revision
        || current_presence.forces != commitment.defender.forces
    {
        Some(AttackInvalidReason::TargetPresenceChanged)
    } else if !fleet_matches_snapshot(current_fleet, &commitment.attacker) {
        Some(AttackInvalidReason::AttackerFleetChanged)
    } else {
        None
    }
}

/// Called once, at the `Outbound → OnSite` transition of an attack mission
/// (`mission.rs`). Re-validates the frozen launch-time commitment against
/// live state exactly as the old synchronous `resolve_and_apply_attack` did
/// — an invalid target still resolves immediately with no tactical screen
/// (doc §11: "si la cible devient invalide avant l'arrivée, aucun écran
/// tactique ne doit être créé"). A valid target now starts a `PendingCombat`
/// instead of resolving synchronously; the mission stays locked at `OnSite`
/// until a command (`ChooseCombatDoctrine`/`RetreatFromCombat`/
/// `AutoResolveCombat`) finalizes it via `apply_combat_resolution`.
pub(crate) fn begin_attack(
    state: &mut GameState,
    mission_id: MissionId,
    resolved_at: StrategicTick,
    commitment: &AttackMissionCommitment,
) -> AttackBeginOutcome {
    if let Some(reason) = commitment_invalidity(state, commitment) {
        let report = CombatReport {
            mission_id,
            planet_id: commitment.defender.planet_id,
            resolved_at,
            rules_version: combat_rules().version,
            seed: commitment.seed,
            attacker: commitment.attacker.clone(),
            defender: commitment.defender.clone(),
            round_history: Vec::new(),
            initial_plan: None,
            final_plan: None,
            intervention_history: Vec::new(),
            status: CombatReportStatus::TargetInvalid(reason),
        };
        insert_combat_report(state, report);
        return AttackBeginOutcome::Invalid(AttackMissionResult {
            target: commitment.defender.planet_id,
            outcome: AttackMissionOutcome::TargetInvalid(reason),
            secured: false,
            attackers_destroyed: false,
        });
    }
    session::begin_pending_combat(
        state,
        mission_id,
        commitment.defender.planet_id,
        commitment.attacker.fleet_id,
        resolved_at,
        commitment,
    );
    AttackBeginOutcome::Pending
}

/// Atomically applies a finalized `CombatResolution` to strategic state
/// (planetary presence, attacker fleet, planetary intel, the persistent
/// `CombatReport` log, and the mission's own result) via the same
/// clone-mutate-swap pattern the old synchronous `resolve_and_apply_attack`
/// used — reused by all three combat commands (`choose_combat_doctrine`/
/// `retreat_from_combat`/`auto_resolve_combat` in `combat/session.rs`), the
/// single point where a finalized `PendingCombat` becomes strategic reality.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_combat_resolution(
    state: &mut GameState,
    mission_id: MissionId,
    resolved_at: StrategicTick,
    fleet_id: FleetId,
    planet_id: PlanetId,
    attacker: &CombatFleetSnapshot,
    defender: &PlanetDefenseSnapshot,
    seed: u64,
    resolution: CombatResolution,
    round_history: Vec<CombatRoundRecord>,
    initial_plan: Option<CombatPlan>,
    final_plan: Option<CombatPlan>,
    intervention_history: Vec<CombatInterventionRecord>,
) -> Result<(CombatReport, AttackMissionResult), CombatApplicationError> {
    let mut candidate = state.clone();
    let next_revision = candidate
        .planetary_presence(planet_id)
        .expect("a finalized combat's target presence still exists")
        .revision
        .checked_add(1)
        .ok_or(CombatApplicationError::RevisionOverflow(planet_id))?;
    {
        let presence = candidate
            .planetary_presence_mut(planet_id)
            .expect("a finalized combat's target presence still exists");
        presence.forces = resolution.defender_survivors.clone();
        presence.revision = next_revision;
        if let CombatControlChange::Secured { current, .. } = resolution.control {
            presence.occupant = current;
        }
    }

    let non_combat_stacks = candidate
        .fleet(fleet_id)
        .expect("a finalized combat's attacker fleet still exists")
        .composition
        .entries()
        .filter(|stack| !combat_rules().ships.contains_key(&stack.craftable))
        .collect::<Vec<_>>();
    let surviving_stacks = resolution
        .attacker_survivors
        .iter()
        .map(|stack| ShipStack::new(stack.craftable, stack.quantity))
        .chain(non_combat_stacks)
        .collect::<Vec<_>>();

    if surviving_stacks.is_empty() {
        candidate.fleets.retain(|fleet| fleet.id != fleet_id);
    } else {
        let composition = FleetComposition::from_stacks(surviving_stacks)
            .map_err(|_| CombatApplicationError::InvalidSurvivingFleet(fleet_id))?;
        let fleet = candidate
            .fleet_mut(fleet_id)
            .expect("a finalized combat's attacker fleet still exists");
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
    .expect("a finalized combat's target has a validated planetary presence");

    let report = CombatReport {
        mission_id,
        planet_id,
        resolved_at,
        rules_version: combat_rules().version,
        seed,
        attacker: attacker.clone(),
        defender: defender.clone(),
        round_history,
        initial_plan,
        final_plan,
        intervention_history,
        status: CombatReportStatus::Resolved(resolution.clone()),
    };
    insert_combat_report(&mut candidate, report.clone());
    let result = AttackMissionResult {
        target: planet_id,
        outcome: AttackMissionOutcome::Resolved(resolution.outcome),
        secured: matches!(resolution.control, CombatControlChange::Secured { .. }),
        attackers_destroyed: candidate.fleet(fleet_id).is_none(),
    };
    // Finalization now happens from `apply_command`, outside
    // `advance_missions`'s generic tail that used to set `mission.result` for
    // the old synchronous path — set it here instead, on the same atomic
    // clone. Also removes the now-finished `PendingCombat` entry, the
    // idempotency guard for the new flow (a second command against the same
    // `mission_id` finds nothing and rejects with `UnknownCombat`, replacing
    // the old `CombatApplicationError::AlreadyApplied` check).
    if let Some(mission) = candidate.mission_mut(mission_id) {
        mission.result = Some(MissionResult::Attack(result));
    }
    candidate
        .pending_combats
        .retain(|pending| pending.mission_id != mission_id);
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
            .filter(|stack| combat_rules().ships.contains_key(&stack.craftable))
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
            Self::Retreat {
                retreating_side: RetreatingSide::Attacker,
            } => "attacker retreat",
            Self::Retreat {
                retreating_side: RetreatingSide::Defender,
            } => "defender retreat",
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
    retreat: CombatRetreatRulesConfig,
    command: CombatCommandRulesConfig,
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

#[derive(Debug, Clone, Copy, Deserialize)]
struct CombatCommandRulesConfig {
    version: u32,
    starting_points: u8,
    change_doctrine_cost: u8,
    focus_fire_cost: u8,
    commit_reserve_cost: u8,
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
        assert!(
            !rules.is_combat_fleet(&FleetState {
                id: FleetId::new(9),
                name: "Escorte mixte".to_string(),
                owner: Owner::Faction(FactionId::new(0)),
                location: FleetLocation::Docked(ColonyId::new(0)),
                composition: FleetComposition::from_stacks([
                    ShipStack::new(CraftableId::FRIGATE_BULWARK, 1),
                    ShipStack::new(CraftableId::LIGHT_CARGO, 1),
                ])
                .expect("mixed fleet composition is valid"),
                cargo: ResourceStock::ZERO,
                assignment: crate::FleetAssignment::Idle,
            })
        );
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
    fn mixed_fleet_snapshots_only_combat_ships() {
        let rules = combat_rules();
        let fleet = FleetState {
            id: FleetId::new(9),
            name: "Escorte mixte".to_string(),
            owner: Owner::Faction(FactionId::new(0)),
            location: FleetLocation::Docked(ColonyId::new(0)),
            composition: FleetComposition::from_stacks([
                ShipStack::new(CraftableId::LIGHT_CARGO, 2),
                ShipStack::new(CraftableId::FRIGATE_BULWARK, 3),
            ])
            .expect("mixed fleet composition is valid"),
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Idle,
        };

        assert!(rules.has_combat_ships(&fleet));
        assert!(!rules.is_combat_fleet(&fleet));
        let snapshot = rules
            .snapshot_fleet(&fleet)
            .expect("mixed fleet with military escort can attack");

        assert_eq!(snapshot.ships.len(), 1);
        assert_eq!(snapshot.ships[0].craftable, CraftableId::FRIGATE_BULWARK);
        assert_eq!(snapshot.ships[0].quantity, 3);
        assert_eq!(
            snapshot.cargo_capacity,
            fleet.capabilities().unwrap().cargo_capacity
        );
    }

    #[test]
    fn combat_resolution_preserves_non_combat_ships_when_escorts_die() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| !presence.forces.is_empty())
            .expect("a defended target exists")
            .planet_id;
        let player_faction = simulation.state().player_faction;
        let fleet_id = FleetId::new(0);
        simulation.state_mut().fleets.push(FleetState {
            id: fleet_id,
            name: "Convoi escorté".to_string(),
            owner: Owner::Faction(player_faction),
            location: FleetLocation::InSystem(target.system_id()),
            composition: FleetComposition::from_stacks([
                ShipStack::new(CraftableId::LIGHT_CARGO, 2),
                ShipStack::new(CraftableId::FRIGATE_BULWARK, 1),
            ])
            .expect("mixed fleet composition is valid"),
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Mission(MissionId::new(0)),
        });
        simulation.state_mut().next_fleet_id = 1;

        let attacker = combat_rules()
            .snapshot_fleet(simulation.state().fleet(fleet_id).unwrap())
            .expect("mixed fleet snapshots for combat");
        let defender = defense(
            target,
            FactionId::new(2),
            simulation
                .state()
                .planetary_presence(target)
                .unwrap()
                .forces[0]
                .definition_id,
            1,
        );
        let (_, result) = apply_combat_resolution(
            simulation.state_mut(),
            MissionId::new(0),
            StrategicTick::new(10),
            fleet_id,
            target,
            &attacker,
            &defender,
            7,
            CombatResolution {
                outcome: CombatOutcome::DefenderVictory,
                rounds: 1,
                attacker_losses: vec![CombatShipLoss {
                    craftable: CraftableId::FRIGATE_BULWARK,
                    quantity: 1,
                }],
                attacker_survivors: Vec::new(),
                defender_losses: Vec::new(),
                defender_survivors: defender.forces.clone(),
                attacker_damage: 0,
                defender_damage: 70,
                salvage_recoverable: ResourceStock::ZERO,
                salvage_recovered: ResourceStock::ZERO,
                control: CombatControlChange::Unchanged,
            },
            Vec::new(),
            None,
            None,
            Vec::new(),
        )
        .expect("combat resolution applies");

        let fleet = simulation
            .state()
            .fleet(fleet_id)
            .expect("non-combat ships keep the fleet alive");
        assert_eq!(fleet.composition.quantity(CraftableId::FRIGATE_BULWARK), 0);
        assert_eq!(fleet.composition.quantity(CraftableId::LIGHT_CARGO), 2);
        assert!(!result.attackers_destroyed);
    }

    #[test]
    fn combat_report_deserializes_without_tactical_trace_fields() {
        let report = CombatReport {
            mission_id: MissionId::new(7),
            planet_id: PlanetId::new(3),
            resolved_at: StrategicTick::new(42),
            rules_version: combat_rules().version(),
            seed: 99,
            attacker: snapshot(2, FactionId::new(0), 1, 70, 45, 60),
            defender: defense(
                PlanetId::new(3),
                FactionId::new(1),
                PlanetaryForceId::from_static("confins_militia"),
                2,
            ),
            round_history: Vec::new(),
            initial_plan: None,
            final_plan: None,
            intervention_history: Vec::new(),
            status: CombatReportStatus::TargetInvalid(AttackInvalidReason::AttackerFleetChanged),
        };
        let source = ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::default())
            .expect("combat report serializes");
        let legacy_source = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("round_history:")
                    && !trimmed.starts_with("initial_plan:")
                    && !trimmed.starts_with("final_plan:")
                    && !trimmed.starts_with("intervention_history:")
            })
            .collect::<Vec<_>>()
            .join("\n");

        let decoded: CombatReport =
            ron::de::from_str(&legacy_source).expect("legacy combat report deserializes");

        assert!(decoded.round_history.is_empty());
        assert_eq!(decoded.initial_plan, None);
        assert_eq!(decoded.final_plan, None);
        assert!(decoded.intervention_history.is_empty());
        assert_eq!(decoded.status, report.status);
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
        let light_bonuses = CombatTargetBonuses {
            light_per_mille: 1_400,
            ..Default::default()
        };
        let heavy_bonuses = CombatTargetBonuses {
            heavy_per_mille: 1_400,
            ..Default::default()
        };
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
    fn a_finalized_combat_cannot_be_applied_twice() {
        // Successor of the legacy `resolve_and_apply_attack`'s own
        // `AlreadyApplied` guard: finalizing removes the `PendingCombat`
        // entry, so a second command against the same mission finds nothing
        // and is rejected with `UnknownCombat` — same property (a combat
        // cannot be applied twice), new mechanism (COMBAT-001-C).
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
        let player_faction = simulation.state().player_faction;
        simulation.state_mut().fleets.push(FleetState {
            id: FleetId::new(0),
            name: "Force de test".to_string(),
            owner: Owner::Faction(player_faction),
            location: FleetLocation::Docked(colony_id),
            composition,
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Idle,
        });
        simulation.state_mut().next_fleet_id = 1;
        let commitment = prepare_attack_commitment(simulation.state(), FleetId::new(0), target, 91)
            .expect("attack snapshot");
        let mission_id = MissionId::new(0);
        assert!(matches!(
            begin_attack(
                simulation.state_mut(),
                mission_id,
                StrategicTick::new(10),
                &commitment,
            ),
            AttackBeginOutcome::Pending
        ));

        auto_resolve_combat(simulation.state_mut(), player_faction, mission_id)
            .expect("first application");
        let after_first = simulation.state().clone();

        assert_eq!(
            auto_resolve_combat(simulation.state_mut(), player_faction, mission_id),
            Err(CombatCommandError::UnknownCombat(mission_id)),
        );
        assert_eq!(simulation.state(), &after_first);
    }
}

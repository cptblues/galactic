// MVP-025-B: pure deterministic combat and atomic strategic application.
use std::collections::BTreeMap;
use std::fmt;

use galactic_domain::{FleetId, MissionId, Owner, PlanetId, ResourceStock};
use serde::Deserialize;

use crate::{
    CraftableCatalog, CraftableId, FleetComposition, FleetState, GameState, PlanetaryForceLoss,
    PlanetaryForceStack, PlanetaryIntelPrecision, PlanetaryPresence, PlanetaryPresenceRules,
    ShipStack, StrategicTick, default_ruleset, refresh_planetary_intelligence,
};

pub const MAX_COMBAT_SHIP_DEFINITIONS: usize = 64;
const COMBAT_STAT_LIMIT: u32 = 1_000_000;
const PER_MILLE: u128 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatShipDefinition {
    pub craftable: CraftableId,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
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
}

impl CombatRules {
    pub(crate) fn from_config(
        config: CombatRulesConfig,
        craftables: &CraftableCatalog,
        planetary_presence: &PlanetaryPresenceRules,
    ) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
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
        if config.ships.is_empty() || config.ships.len() > MAX_COMBAT_SHIP_DEFINITIONS {
            return Err(CombatRulesError::InvalidShipCount {
                found: config.ships.len(),
                maximum: MAX_COMBAT_SHIP_DEFINITIONS,
            });
        }

        let mut ships = BTreeMap::new();
        for configured in config.ships {
            let Some(craftable) = craftables.id_by_key(&configured.id) else {
                return Err(CombatRulesError::UnknownCraftable);
            };
            let definition = craftables.definition(craftable);
            let Some(ship) = definition.ship else {
                return Err(CombatRulesError::CraftableIsNotShip(craftable));
            };
            if ship.class != crate::ShipClass::Military {
                return Err(CombatRulesError::CraftableIsNotMilitary(craftable));
            }
            if configured.offense == 0
                || configured.defense == 0
                || configured.durability == 0
                || configured.offense > COMBAT_STAT_LIMIT
                || configured.defense > COMBAT_STAT_LIMIT
                || configured.durability > COMBAT_STAT_LIMIT
            {
                return Err(CombatRulesError::InvalidShipStats(craftable));
            }
            if ships
                .insert(
                    craftable,
                    CombatShipDefinition {
                        craftable,
                        offense: configured.offense,
                        defense: configured.defense,
                        durability: configured.durability,
                    },
                )
                .is_some()
            {
                return Err(CombatRulesError::DuplicateCraftable(craftable));
            }
        }

        if planetary_presence.definitions().any(|definition| {
            definition.offense == 0 || definition.defense == 0 || definition.durability == 0
        }) {
            return Err(CombatRulesError::InvalidPlanetaryForceCatalog);
        }

        Ok(Self {
            version: config.version,
            maximum_rounds: config.maximum_rounds,
            damage_scale: config.damage_scale,
            defense_weight_per_mille: config.defense_weight_per_mille,
            damage_variance_per_mille: config.damage_variance_per_mille,
            salvage_per_destroyed_defender: config.salvage_per_destroyed_defender.into_stock(),
            ships,
        })
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn maximum_rounds(&self) -> u16 {
        self.maximum_rounds
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
            output.push(';');
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatShipStack {
    pub craftable: CraftableId,
    pub quantity: u64,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatFleetSnapshot {
    pub fleet_id: FleetId,
    pub owner: Owner,
    pub ships: Vec<CombatShipStack>,
    pub cargo: ResourceStock,
    pub cargo_capacity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetDefenseSnapshot {
    pub planet_id: PlanetId,
    pub occupant: Owner,
    pub population: u64,
    pub forces: Vec<PlanetaryForceStack>,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatOutcome {
    AttackerVictory,
    DefenderVictory,
    Stalemate,
    MutualDestruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatControlChange {
    Unchanged,
    Secured { previous: Owner, current: Owner },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatShipLoss {
    pub craftable: CraftableId,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackInvalidReason {
    TargetOwnerChanged,
    TargetPresenceChanged,
    AttackerFleetChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatReportStatus {
    Resolved(CombatResolution),
    TargetInvalid(AttackInvalidReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackMissionOutcome {
    Resolved(CombatOutcome),
    TargetInvalid(AttackInvalidReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn resolve_combat(
    attacker: &CombatFleetSnapshot,
    defender: &PlanetDefenseSnapshot,
    seed: u64,
    rules: &CombatRules,
    planetary_rules: &PlanetaryPresenceRules,
) -> CombatResolution {
    let attacker_totals = attacker_totals(attacker, rules);
    let defender_totals = defender_totals(defender, rules, planetary_rules);
    let attacker_initial_hull = attacker_totals.hull;
    let defender_initial_hull = defender_totals.hull;
    let mut attacker_hull = attacker_initial_hull;
    let mut defender_hull = defender_initial_hull;
    let mut rounds = 0;

    while rounds < rules.maximum_rounds && attacker_hull > 0 && defender_hull > 0 {
        rounds += 1;
        let attacker_power = scaled_power(
            attacker_totals.offense,
            attacker_hull,
            attacker_initial_hull,
        );
        let defender_power = scaled_power(
            defender_totals.offense,
            defender_hull,
            defender_initial_hull,
        );
        let attacker_damage = varied_damage(
            attacker_power.saturating_mul(u128::from(rules.damage_scale)),
            seed ^ u64::from(rounds) ^ 0x4154_5441_434b_4552,
            rules.damage_variance_per_mille,
        );
        let defender_damage = varied_damage(
            defender_power.saturating_mul(u128::from(rules.damage_scale)),
            seed ^ u64::from(rounds) ^ 0x4445_4645_4e44_4552,
            rules.damage_variance_per_mille,
        );
        attacker_hull = attacker_hull.saturating_sub(defender_damage);
        defender_hull = defender_hull.saturating_sub(attacker_damage);
    }

    let outcome = match (attacker_hull > 0, defender_hull > 0) {
        (true, false) => CombatOutcome::AttackerVictory,
        (false, true) => CombatOutcome::DefenderVictory,
        (false, false) => CombatOutcome::MutualDestruction,
        (true, true) => CombatOutcome::Stalemate,
    };
    let attacker_survivors =
        surviving_attackers(&attacker.ships, attacker_hull, attacker_initial_hull);
    let defender_survivors =
        surviving_defenders(&defender.forces, defender_hull, defender_initial_hull);
    let attacker_losses = ship_losses(&attacker.ships, &attacker_survivors);
    let defender_losses = planetary_losses(&defender.forces, &defender_survivors);
    let destroyed_defenders = defender_losses.iter().fold(0_u64, |total, loss| {
        total.saturating_add(u64::from(loss.quantity))
    });
    let salvage_recoverable =
        multiply_stock(rules.salvage_per_destroyed_defender, destroyed_defenders);
    let salvage_recovered =
        if outcome == CombatOutcome::AttackerVictory && !attacker_survivors.is_empty() {
            cap_salvage(salvage_recoverable, attacker.cargo, attacker.cargo_capacity)
        } else {
            ResourceStock::ZERO
        };
    let control = if outcome == CombatOutcome::AttackerVictory {
        CombatControlChange::Secured {
            previous: defender.occupant,
            current: attacker.owner,
        }
    } else {
        CombatControlChange::Unchanged
    };

    CombatResolution {
        outcome,
        rounds,
        attacker_losses,
        attacker_survivors,
        defender_losses,
        defender_survivors,
        attacker_damage: clamp_u128(attacker_initial_hull.saturating_sub(attacker_hull)),
        defender_damage: clamp_u128(defender_initial_hull.saturating_sub(defender_hull)),
        salvage_recoverable,
        salvage_recovered,
        control,
    }
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

    let resolution = resolve_combat(
        &commitment.attacker,
        &commitment.defender,
        commitment.seed,
        combat_rules(),
        default_ruleset().planetary_presence(),
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

#[derive(Debug, Clone, Copy)]
struct SideTotals {
    offense: u128,
    hull: u128,
}

fn attacker_totals(attacker: &CombatFleetSnapshot, rules: &CombatRules) -> SideTotals {
    attacker.ships.iter().fold(
        SideTotals {
            offense: 0,
            hull: 0,
        },
        |totals, stack| SideTotals {
            offense: totals.offense.saturating_add(
                u128::from(stack.offense).saturating_mul(u128::from(stack.quantity)),
            ),
            hull: totals.hull.saturating_add(
                effective_hull(
                    stack.defense,
                    stack.durability,
                    rules.defense_weight_per_mille,
                )
                .saturating_mul(u128::from(stack.quantity)),
            ),
        },
    )
}

fn defender_totals(
    defender: &PlanetDefenseSnapshot,
    rules: &CombatRules,
    planetary_rules: &PlanetaryPresenceRules,
) -> SideTotals {
    defender.forces.iter().fold(
        SideTotals {
            offense: 0,
            hull: 0,
        },
        |totals, stack| {
            let definition = planetary_rules
                .definition(stack.definition_id)
                .expect("a validated defense snapshot references known forces");
            SideTotals {
                offense: totals.offense.saturating_add(
                    u128::from(definition.offense).saturating_mul(u128::from(stack.quantity)),
                ),
                hull: totals.hull.saturating_add(
                    effective_hull(
                        definition.defense,
                        definition.durability,
                        rules.defense_weight_per_mille,
                    )
                    .saturating_mul(u128::from(stack.quantity)),
                ),
            }
        },
    )
}

fn effective_hull(defense: u32, durability: u32, defense_weight_per_mille: u32) -> u128 {
    u128::from(durability)
        .saturating_mul(PER_MILLE)
        .saturating_add(u128::from(defense).saturating_mul(u128::from(defense_weight_per_mille)))
}

fn scaled_power(initial_power: u128, current_hull: u128, initial_hull: u128) -> u128 {
    if current_hull == 0 || initial_hull == 0 {
        return 0;
    }
    initial_power
        .saturating_mul(current_hull)
        .div_ceil(initial_hull)
}

fn varied_damage(base: u128, seed: u64, variance_per_mille: u32) -> u128 {
    if base == 0 || variance_per_mille == 0 {
        return base;
    }
    let variance = u64::from(variance_per_mille);
    let width = variance.saturating_mul(2).saturating_add(1);
    let modifier = 1_000_u64
        .saturating_sub(variance)
        .saturating_add(splitmix64(seed) % width);
    base.saturating_mul(u128::from(modifier)) / PER_MILLE
}

fn surviving_attackers(
    initial: &[CombatShipStack],
    remaining_hull: u128,
    initial_hull: u128,
) -> Vec<CombatShipStack> {
    initial
        .iter()
        .filter_map(|stack| {
            let quantity = proportional_survivors(stack.quantity, remaining_hull, initial_hull);
            (quantity > 0).then_some(CombatShipStack { quantity, ..*stack })
        })
        .collect()
}

fn surviving_defenders(
    initial: &[PlanetaryForceStack],
    remaining_hull: u128,
    initial_hull: u128,
) -> Vec<PlanetaryForceStack> {
    initial
        .iter()
        .filter_map(|stack| {
            let quantity =
                proportional_survivors(u64::from(stack.quantity), remaining_hull, initial_hull);
            (quantity > 0).then_some(PlanetaryForceStack {
                definition_id: stack.definition_id,
                quantity: u32::try_from(quantity)
                    .expect("survivors cannot exceed the initial u32 quantity"),
            })
        })
        .collect()
}

fn proportional_survivors(quantity: u64, remaining_hull: u128, initial_hull: u128) -> u64 {
    if quantity == 0 || remaining_hull == 0 || initial_hull == 0 {
        return 0;
    }
    let survivors = u128::from(quantity)
        .saturating_mul(remaining_hull)
        .div_ceil(initial_hull);
    u64::try_from(survivors.min(u128::from(quantity)))
        .expect("the survivor count is bounded by a u64 quantity")
}

fn ship_losses(initial: &[CombatShipStack], survivors: &[CombatShipStack]) -> Vec<CombatShipLoss> {
    initial
        .iter()
        .filter_map(|stack| {
            let surviving = survivors
                .iter()
                .find(|survivor| survivor.craftable == stack.craftable)
                .map(|survivor| survivor.quantity)
                .unwrap_or(0);
            let quantity = stack.quantity.saturating_sub(surviving);
            (quantity > 0).then_some(CombatShipLoss {
                craftable: stack.craftable,
                quantity,
            })
        })
        .collect()
}

fn planetary_losses(
    initial: &[PlanetaryForceStack],
    survivors: &[PlanetaryForceStack],
) -> Vec<PlanetaryForceLoss> {
    initial
        .iter()
        .filter_map(|stack| {
            let surviving = survivors
                .iter()
                .find(|survivor| survivor.definition_id == stack.definition_id)
                .map(|survivor| survivor.quantity)
                .unwrap_or(0);
            let quantity = stack.quantity.saturating_sub(surviving);
            (quantity > 0).then_some(PlanetaryForceLoss {
                definition_id: stack.definition_id,
                quantity,
            })
        })
        .collect()
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

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
    ships: Vec<CombatShipConfig>,
}

#[derive(Debug, Deserialize)]
struct CombatShipConfig {
    id: String,
    offense: u32,
    defense: u32,
    durability: u32,
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

    #[test]
    fn identical_seed_and_snapshots_produce_identical_report() {
        let rules = combat_rules();
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("line_garrison")
            .expect("default force exists");
        let attacker = snapshot(0, FactionId::new(0), 3, 70, 45, 60);
        let defender = defense(
            PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            FactionId::new(2),
            force,
            8,
        );

        let first = resolve_combat(&attacker, &defender, 42, rules, planetary);
        let second = resolve_combat(&attacker, &defender, 42, rules, planetary);

        assert_eq!(first, second);
    }

    #[test]
    fn simultaneous_lethal_damage_is_mutual_destruction() {
        let rules = combat_rules();
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("line_garrison")
            .expect("default force exists");
        let attacker = snapshot(0, FactionId::new(0), 1, 10_000, 1, 1);
        let defender = defense(
            PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            FactionId::new(2),
            force,
            1,
        );

        let report = resolve_combat(&attacker, &defender, 7, rules, planetary);

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

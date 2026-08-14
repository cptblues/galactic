// COMBAT-001-A: fixed-point math shared by both the legacy aggregate combat
// path (`combat.rs`) and the per-stack tactical round engine (`rounds.rs`).
// One toolbox, two consumers — never two separate implementations of the
// same arithmetic (see COMBAT-001 doc §8).

pub(crate) const PER_MILLE: u128 = 1_000;

pub(crate) fn effective_hull(defense: u32, durability: u32, defense_weight_per_mille: u32) -> u128 {
    u128::from(durability)
        .saturating_mul(PER_MILLE)
        .saturating_add(u128::from(defense).saturating_mul(u128::from(defense_weight_per_mille)))
}

pub(crate) fn scaled_power(initial_power: u128, current_hull: u128, initial_hull: u128) -> u128 {
    if current_hull == 0 || initial_hull == 0 {
        return 0;
    }
    initial_power
        .saturating_mul(current_hull)
        .div_ceil(initial_hull)
}

pub(crate) fn varied_damage(base: u128, seed: u64, variance_per_mille: u32) -> u128 {
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

pub(crate) fn proportional_survivors(
    quantity: u64,
    remaining_hull: u128,
    initial_hull: u128,
) -> u64 {
    if quantity == 0 || remaining_hull == 0 || initial_hull == 0 {
        return 0;
    }
    let survivors = u128::from(quantity)
        .saturating_mul(remaining_hull)
        .div_ceil(initial_hull);
    u64::try_from(survivors.min(u128::from(quantity)))
        .expect("the survivor count is bounded by a u64 quantity")
}

pub(crate) fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// --- Tactical round-engine state -------------------------------------------
//
// Deliberately narrower than a mission-linked wrapper: no `MissionId`,
// `player_side`, or intel state here. This keeps the engine ignorant of
// missions, so it is independently testable and reusable both by a future
// interactive path (COMBAT-001-C) and by the `resolve_combat` auto-resolve
// façade (used already, see combat.rs).

use galactic_domain::Owner;

use crate::{CombatTargetBonuses, CombatTargetClass, CraftableId, PlanetaryForceId};

use super::intel::{CombatIntelPrecision, CombatIntelSourceInputs, base_intel_percent};
use super::{
    CombatDoctrineId, CombatFleetSnapshot, CombatRules, PlanetDefenseSnapshot,
    PlanetaryPresenceRules,
};

/// Stable per-stack identity within one combat — small, immutable "identity"
/// data, safe to expose to `galactic_client` (COMBAT-001-D) unlike the
/// mutable aggregate state types below (`CombatStackState`/`CombatSideState`/
/// `FleetCombatState`), which stay `pub(crate)` forever.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct CombatStackId(pub(crate) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatSide {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatUnitRef {
    Ship(CraftableId),
    PlanetaryForce(PlanetaryForceId),
}

/// `Support` is reserved for future Support-category ships / support-domain
/// planetary forces. The default ruleset has none today (only `Military`
/// craftables carry combat stats, and no planetary force domain maps to
/// `Support`) — this role is implemented and unit-testable via synthetic
/// fixtures, but currently unreachable through real game data. See
/// `FlankingManeuver` in `doctrine.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatTacticalRole {
    Line,
    Support,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CombatStackState {
    pub(crate) stack_id: CombatStackId,
    pub(crate) source: CombatUnitRef,
    pub(crate) initial_quantity: u64,
    pub(crate) surviving_quantity: u64,
    /// Same fixed-point pool as `effective_hull()` — not a raw per-vessel HP
    /// count. Preserves the existing "partial damage → whole-unit loss" rule
    /// via `proportional_survivors` without inventing a second damage model.
    pub(crate) current_hull: u128,
    pub(crate) maximum_hull: u128,
    pub(crate) offense: u32,
    pub(crate) defense: u32,
    pub(crate) durability: u32,
    pub(crate) target_class: CombatTargetClass,
    pub(crate) bonuses: CombatTargetBonuses,
    pub(crate) tactical_role: CombatTacticalRole,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CombatSideState {
    pub(crate) owner: Owner,
    pub(crate) stacks: Vec<CombatStackState>,
    pub(crate) last_doctrine: Option<CombatDoctrineId>,
    pub(crate) consecutive_doctrine_uses: u8,
    pub(crate) retreated: bool,
}

impl CombatSideState {
    pub(crate) fn has_operational_stack(&self) -> bool {
        self.stacks.iter().any(|stack| stack.surviving_quantity > 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum FleetCombatPhase {
    AwaitingDoctrine,
    Resolving,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FleetCombatState {
    pub(crate) seed: u64,
    pub(crate) round: u16,
    pub(crate) maximum_rounds: u16,
    pub(crate) phase: FleetCombatPhase,
    pub(crate) attacker: CombatSideState,
    pub(crate) defender: CombatSideState,
    pub(crate) history: Vec<CombatRoundRecord>,
    /// Attacker's knowledge of the defender's composition, 0-100. One
    /// direction only — see `combat/intel.rs`'s module doc for why.
    pub(crate) intel_percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatStackLoss {
    pub stack_id: CombatStackId,
    pub quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CombatRoundEvent {
    StackDestroyed {
        side: CombatSide,
        stack_id: CombatStackId,
    },
    CounterTriggered {
        countered_side: CombatSide,
        countering_side: CombatSide,
    },
    RepetitionPenaltyApplied {
        side: CombatSide,
        consecutive_uses: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatRoundRecord {
    pub round: u16,
    pub attacker_doctrine: CombatDoctrineId,
    pub defender_doctrine: CombatDoctrineId,
    pub attacker_damage: u128,
    pub defender_damage: u128,
    pub attacker_losses: Vec<CombatStackLoss>,
    pub defender_losses: Vec<CombatStackLoss>,
    pub notable_events: Vec<CombatRoundEvent>,
    /// Attacker's intel percent *after* this round's gain is applied — the
    /// additive field COMBAT-001-A left room for.
    pub intel_after: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatEngineError {
    RoundAlreadyCompleted,
}

fn tactical_role_for_ship(craftable: CraftableId) -> CombatTacticalRole {
    match crate::craftable_catalog().definition(craftable).category {
        crate::CraftableCategory::Support => CombatTacticalRole::Support,
        _ => CombatTacticalRole::Line,
    }
}

/// Snapshots a launched attack into a fresh round-engine state: one
/// `CombatStackState` per attacker ship stack and per defender planetary
/// force stack, `CombatStackId`s assigned sequentially. Planetary forces
/// always get `CombatTacticalRole::Line` — no support-role domain exists on
/// the defender side today.
pub(crate) fn prepare_fleet_combat(
    attacker: &CombatFleetSnapshot,
    defender: &PlanetDefenseSnapshot,
    seed: u64,
    rules: &CombatRules,
    planetary_rules: &PlanetaryPresenceRules,
    intel_precision: CombatIntelPrecision,
    intel_report_age_ticks: u64,
) -> Result<FleetCombatState, CombatEngineError> {
    let mut next_id = 0_u32;

    let attacker_stacks = attacker
        .ships
        .iter()
        .map(|stack| {
            let stack_id = CombatStackId(next_id);
            next_id += 1;
            let maximum_hull = effective_hull(
                stack.defense,
                stack.durability,
                rules.defense_weight_per_mille,
            )
            .saturating_mul(u128::from(stack.quantity));
            CombatStackState {
                stack_id,
                source: CombatUnitRef::Ship(stack.craftable),
                initial_quantity: stack.quantity,
                surviving_quantity: stack.quantity,
                current_hull: maximum_hull,
                maximum_hull,
                offense: stack.offense,
                defense: stack.defense,
                durability: stack.durability,
                target_class: stack.target_class,
                bonuses: stack.bonuses,
                tactical_role: tactical_role_for_ship(stack.craftable),
            }
        })
        .collect();

    let defender_stacks = defender
        .forces
        .iter()
        .map(|stack| {
            let definition = planetary_rules
                .definition(stack.definition_id)
                .expect("a validated defense snapshot references known forces");
            let stack_id = CombatStackId(next_id);
            next_id += 1;
            let maximum_hull = effective_hull(
                definition.defense,
                definition.durability,
                rules.defense_weight_per_mille,
            )
            .saturating_mul(u128::from(stack.quantity));
            CombatStackState {
                stack_id,
                source: CombatUnitRef::PlanetaryForce(stack.definition_id),
                initial_quantity: u64::from(stack.quantity),
                surviving_quantity: u64::from(stack.quantity),
                current_hull: maximum_hull,
                maximum_hull,
                offense: definition.offense,
                defense: definition.defense,
                durability: definition.durability,
                target_class: definition.target_class,
                bonuses: CombatTargetBonuses::default(),
                tactical_role: CombatTacticalRole::Line,
            }
        })
        .collect();

    let has_reconnaissance_ship = attacker
        .ships
        .iter()
        .any(|stack| stack.craftable == CraftableId::LIGHT_PROBE);
    let intel_percent = base_intel_percent(
        CombatIntelSourceInputs {
            precision: intel_precision,
            report_age_ticks: intel_report_age_ticks,
            has_reconnaissance_ship,
        },
        rules.intel(),
    );

    Ok(FleetCombatState {
        seed,
        round: 0,
        maximum_rounds: rules.maximum_rounds(),
        phase: FleetCombatPhase::AwaitingDoctrine,
        attacker: CombatSideState {
            owner: attacker.owner,
            stacks: attacker_stacks,
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        },
        defender: CombatSideState {
            owner: defender.occupant,
            stacks: defender_stacks,
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        },
        history: Vec::new(),
        intel_percent,
    })
}

/// Total cargo capacity of only the ships that actually survived combat —
/// COMBAT-001-E: `finalize_fleet_combat`'s `attacker_cargo_capacity`
/// parameter is the fleet's *pre-combat* capacity (frozen at commitment
/// time), which can now legitimately exceed what's left once real losses
/// happen (an engine-balance fix elsewhere made losing ships an actually
/// reachable outcome instead of a near-impossible edge case). Salvage must
/// fit in what survivors can carry, not what the whole pre-battle fleet
/// could.
fn surviving_cargo_capacity(survivors: &[super::CombatShipStack]) -> u64 {
    survivors.iter().fold(0_u64, |total, stack| {
        let capacity = crate::craftable_catalog()
            .definition(stack.craftable)
            .ship
            .map(|ship| ship.cargo_capacity)
            .unwrap_or(0);
        total.saturating_add(capacity.saturating_mul(stack.quantity))
    })
}

fn ship_survivors(stacks: &[CombatStackState]) -> Vec<super::CombatShipStack> {
    stacks
        .iter()
        .filter_map(|stack| {
            let CombatUnitRef::Ship(craftable) = stack.source else {
                return None;
            };
            (stack.surviving_quantity > 0).then_some(super::CombatShipStack {
                craftable,
                quantity: stack.surviving_quantity,
                offense: stack.offense,
                defense: stack.defense,
                durability: stack.durability,
                target_class: stack.target_class,
                bonuses: stack.bonuses,
            })
        })
        .collect()
}

fn planetary_survivors(stacks: &[CombatStackState]) -> Vec<crate::PlanetaryForceStack> {
    stacks
        .iter()
        .filter_map(|stack| {
            let CombatUnitRef::PlanetaryForce(definition_id) = stack.source else {
                return None;
            };
            (stack.surviving_quantity > 0).then_some(crate::PlanetaryForceStack {
                definition_id,
                quantity: u32::try_from(stack.surviving_quantity)
                    .expect("survivors cannot exceed the launch-time u32 quantity"),
            })
        })
        .collect()
}

fn ship_losses(stacks: &[CombatStackState]) -> Vec<super::CombatShipLoss> {
    stacks
        .iter()
        .filter_map(|stack| {
            let CombatUnitRef::Ship(craftable) = stack.source else {
                return None;
            };
            let lost = stack
                .initial_quantity
                .saturating_sub(stack.surviving_quantity);
            (lost > 0).then_some(super::CombatShipLoss {
                craftable,
                quantity: lost,
            })
        })
        .collect()
}

fn planetary_losses(stacks: &[CombatStackState]) -> Vec<crate::PlanetaryForceLoss> {
    stacks
        .iter()
        .filter_map(|stack| {
            let CombatUnitRef::PlanetaryForce(definition_id) = stack.source else {
                return None;
            };
            let lost = stack
                .initial_quantity
                .saturating_sub(stack.surviving_quantity);
            (lost > 0).then_some(crate::PlanetaryForceLoss {
                definition_id,
                quantity: u32::try_from(lost)
                    .expect("losses cannot exceed the launch-time u32 quantity"),
            })
        })
        .collect()
}

/// Converts the round-by-round tactical state into the same
/// `CombatResolution` shape `resolve_and_apply_attack` already consumes —
/// only how it is produced changes. `attacker_cargo`/`attacker_cargo_capacity`
/// come from the original launch-time snapshot (the engine itself tracks no
/// logistics, only combat stacks) — salvage is additionally capped against
/// `surviving_cargo_capacity`, since real ship losses during combat can
/// leave the fleet with less capacity than it started with.
/// `CombatResolution.control` (planet-control flip on attacker
/// victory) is computed here exactly as today, preserving the doc's §4.7
/// "décision de compatibilité temporaire".
pub(crate) fn finalize_fleet_combat(
    state: &FleetCombatState,
    attacker_cargo: galactic_domain::ResourceStock,
    attacker_cargo_capacity: u64,
    rules: &CombatRules,
) -> Result<super::CombatResolution, CombatEngineError> {
    let attacker_alive = state.attacker.has_operational_stack();
    let defender_alive = state.defender.has_operational_stack();
    let outcome = if state.attacker.retreated {
        super::CombatOutcome::Retreat {
            retreating_side: super::RetreatingSide::Attacker,
        }
    } else if state.defender.retreated {
        super::CombatOutcome::Retreat {
            retreating_side: super::RetreatingSide::Defender,
        }
    } else {
        match (attacker_alive, defender_alive) {
            (true, false) => super::CombatOutcome::AttackerVictory,
            (false, true) => super::CombatOutcome::DefenderVictory,
            (false, false) => super::CombatOutcome::MutualDestruction,
            (true, true) => super::CombatOutcome::Stalemate,
        }
    };

    let attacker_survivors = ship_survivors(&state.attacker.stacks);
    let defender_survivors = planetary_survivors(&state.defender.stacks);
    let attacker_losses = ship_losses(&state.attacker.stacks);
    let defender_losses = planetary_losses(&state.defender.stacks);

    let destroyed_defenders: u64 = defender_losses.iter().fold(0_u64, |total, loss| {
        total.saturating_add(u64::from(loss.quantity))
    });
    let salvage_recoverable =
        super::multiply_stock(rules.salvage_per_destroyed_defender, destroyed_defenders);
    // A retreat never recovers salvage (doc §16: "ne récupère pas
    // nécessairement tout le butin") — combined with the deterministic
    // damage penalty already applied by `rounds::apply_retreat_penalty`
    // before this state was marked retreated, retreating has a real cost.
    let salvage_recovered = if matches!(outcome, super::CombatOutcome::Retreat { .. }) {
        galactic_domain::ResourceStock::ZERO
    } else if outcome == super::CombatOutcome::AttackerVictory && !attacker_survivors.is_empty() {
        let effective_capacity =
            attacker_cargo_capacity.min(surviving_cargo_capacity(&attacker_survivors));
        super::cap_salvage(salvage_recoverable, attacker_cargo, effective_capacity)
    } else {
        galactic_domain::ResourceStock::ZERO
    };

    let control = if outcome == super::CombatOutcome::AttackerVictory {
        super::CombatControlChange::Secured {
            previous: state.defender.owner,
            current: state.attacker.owner,
        }
    } else {
        super::CombatControlChange::Unchanged
    };

    // Field naming matches the legacy `CombatResolution`: `attacker_damage`
    // is damage the attacker *received* (i.e. dealt by the defender across
    // every round), and vice versa.
    let (attacker_damage, defender_damage) = state.history.iter().fold(
        (0_u128, 0_u128),
        |(taken_by_attacker, taken_by_defender), record| {
            (
                taken_by_attacker.saturating_add(record.defender_damage),
                taken_by_defender.saturating_add(record.attacker_damage),
            )
        },
    );

    Ok(super::CombatResolution {
        outcome,
        rounds: state.round,
        attacker_losses,
        attacker_survivors,
        defender_losses,
        defender_survivors,
        attacker_damage: super::clamp_u128(attacker_damage),
        defender_damage: super::clamp_u128(defender_damage),
        salvage_recoverable,
        salvage_recovered,
        control,
    })
}

/// Marks `side` as having retreated and ends the tactical combat. Cheap and
/// pure so COMBAT-001-C can call it from `GameCommand::RetreatFromCombat`
/// without touching the engine again. Deliberately does **not** attempt to
/// map a retreated state onto a `CombatOutcome` — `CombatOutcome` has no
/// retreat variant today, and deciding how one should interact with
/// missions/reports is a COMBAT-001-C concern, not this engine's.
///
/// Unused outside its own tests until COMBAT-001-C wires a
/// `GameCommand::RetreatFromCombat` to it — no auto-resolve façade ever
/// retreats, so this has no production caller yet by design.
#[allow(dead_code)]
pub(crate) fn retreat_side(
    state: &mut FleetCombatState,
    side: CombatSide,
) -> Result<(), CombatEngineError> {
    if state.phase == FleetCombatPhase::Completed {
        return Err(CombatEngineError::RoundAlreadyCompleted);
    }
    match side {
        CombatSide::Attacker => state.attacker.retreated = true,
        CombatSide::Defender => state.defender.retreated = true,
    }
    state.phase = FleetCombatPhase::Completed;
    Ok(())
}

#[cfg(test)]
mod tests {
    use galactic_domain::{FactionId, FleetId, PlanetId, ResourceStock};

    use crate::{CraftableId, combat_rules, default_ruleset};

    use super::*;

    fn attacker_snapshot() -> CombatFleetSnapshot {
        CombatFleetSnapshot {
            fleet_id: FleetId::new(0),
            owner: Owner::Faction(FactionId::new(0)),
            ships: vec![
                super::super::CombatShipStack {
                    craftable: CraftableId::FRIGATE_BULWARK,
                    quantity: 3,
                    offense: 70,
                    defense: 45,
                    durability: 60,
                    target_class: CombatTargetClass::Medium,
                    bonuses: CombatTargetBonuses::default(),
                },
                super::super::CombatShipStack {
                    craftable: CraftableId::NEEDLE_INTERCEPTOR,
                    quantity: 5,
                    offense: 40,
                    defense: 15,
                    durability: 20,
                    target_class: CombatTargetClass::Light,
                    bonuses: CombatTargetBonuses::default(),
                },
            ],
            cargo: ResourceStock::ZERO,
            cargo_capacity: 1_000,
        }
    }

    fn defender_snapshot() -> PlanetDefenseSnapshot {
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("confins_guard")
            .expect("default force exists");
        PlanetDefenseSnapshot {
            planet_id: PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            occupant: Owner::Faction(FactionId::new(2)),
            population: 5_000,
            forces: vec![crate::PlanetaryForceStack {
                definition_id: force,
                quantity: 8,
            }],
            revision: 0,
        }
    }

    fn prepared() -> FleetCombatState {
        prepare_fleet_combat(
            &attacker_snapshot(),
            &defender_snapshot(),
            42,
            combat_rules(),
            default_ruleset().planetary_presence(),
            CombatIntelPrecision::Surveyed,
            0,
        )
        .expect("valid snapshots prepare successfully")
    }

    #[test]
    fn every_stack_gets_a_unique_id() {
        let state = prepared();
        let mut ids: Vec<CombatStackId> = state
            .attacker
            .stacks
            .iter()
            .chain(state.defender.stacks.iter())
            .map(|stack| stack.stack_id)
            .collect();
        let count_before_dedup = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count_before_dedup);
    }

    #[test]
    fn stack_count_matches_the_source_snapshots() {
        let state = prepared();
        assert_eq!(state.attacker.stacks.len(), 2);
        assert_eq!(state.defender.stacks.len(), 1);
    }

    #[test]
    fn initial_hull_matches_effective_hull_times_quantity() {
        let state = prepared();
        let frigate_stack = &state.attacker.stacks[0];
        let expected =
            effective_hull(45, 60, combat_rules().defense_weight_per_mille).saturating_mul(3);
        assert_eq!(frigate_stack.maximum_hull, expected);
        assert_eq!(frigate_stack.current_hull, frigate_stack.maximum_hull);
    }

    #[test]
    fn round_and_phase_start_at_the_beginning() {
        let state = prepared();
        assert_eq!(state.round, 0);
        assert_eq!(state.phase, FleetCombatPhase::AwaitingDoctrine);
        assert_eq!(state.maximum_rounds, combat_rules().maximum_rounds());
        assert!(state.history.is_empty());
        assert!(!state.attacker.retreated);
        assert!(!state.defender.retreated);
    }

    #[test]
    fn planetary_forces_default_to_the_line_role() {
        let state = prepared();
        assert_eq!(
            state.defender.stacks[0].tactical_role,
            CombatTacticalRole::Line
        );
    }

    #[test]
    fn every_stack_starts_fully_operational() {
        let state = prepared();
        assert!(state.attacker.has_operational_stack());
        assert!(state.defender.has_operational_stack());
        for stack in state
            .attacker
            .stacks
            .iter()
            .chain(state.defender.stacks.iter())
        {
            assert_eq!(stack.surviving_quantity, stack.initial_quantity);
        }
    }

    fn play_out(
        mut state: FleetCombatState,
        attacker_doctrine: CombatDoctrineId,
        defender_doctrine: CombatDoctrineId,
    ) -> FleetCombatState {
        while state.phase != FleetCombatPhase::Completed {
            let resolution = super::super::rounds::resolve_combat_round(
                &state,
                attacker_doctrine,
                defender_doctrine,
                combat_rules(),
                combat_rules().tactics(),
            )
            .expect("a round resolves while combat is not completed");
            super::super::rounds::apply_round_resolution(&mut state, resolution)
                .expect("a resolved round applies while combat is not completed");
        }
        state
    }

    fn overwhelming_attacker_snapshot() -> CombatFleetSnapshot {
        CombatFleetSnapshot {
            fleet_id: FleetId::new(0),
            owner: Owner::Faction(FactionId::new(0)),
            ships: vec![super::super::CombatShipStack {
                craftable: CraftableId::FRIGATE_BULWARK,
                quantity: 50,
                offense: 5_000,
                defense: 5_000,
                durability: 5_000,
                target_class: CombatTargetClass::Medium,
                bonuses: CombatTargetBonuses::default(),
            }],
            cargo: ResourceStock::ZERO,
            cargo_capacity: 1_000,
        }
    }

    fn frail_defender_snapshot() -> PlanetDefenseSnapshot {
        let planetary = default_ruleset().planetary_presence();
        let force = planetary
            .id_by_key("confins_guard")
            .expect("default force exists");
        PlanetDefenseSnapshot {
            planet_id: PlanetId::from_system_index(crate::MVP_HOME_SYSTEM_ID, 1),
            occupant: Owner::Faction(FactionId::new(2)),
            population: 5_000,
            forces: vec![crate::PlanetaryForceStack {
                definition_id: force,
                quantity: 1,
            }],
            revision: 0,
        }
    }

    #[test]
    fn finalize_reports_attacker_victory_and_secures_control() {
        let state = prepare_fleet_combat(
            &overwhelming_attacker_snapshot(),
            &frail_defender_snapshot(),
            42,
            combat_rules(),
            default_ruleset().planetary_presence(),
            CombatIntelPrecision::Surveyed,
            0,
        )
        .expect("prepares");
        let finished = play_out(
            state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );

        let resolution =
            finalize_fleet_combat(&finished, ResourceStock::ZERO, 1_000, combat_rules())
                .expect("a completed, non-retreated combat finalizes");

        assert_eq!(
            resolution.outcome,
            super::super::CombatOutcome::AttackerVictory
        );
        assert_eq!(
            resolution.control,
            super::super::CombatControlChange::Secured {
                previous: frail_defender_snapshot().occupant,
                current: overwhelming_attacker_snapshot().owner,
            }
        );
        assert!(resolution.defender_survivors.is_empty());
        assert!(!resolution.attacker_survivors.is_empty());
    }

    #[test]
    fn finalize_caps_salvage_at_the_attacker_cargo_capacity() {
        let state = prepare_fleet_combat(
            &overwhelming_attacker_snapshot(),
            &frail_defender_snapshot(),
            42,
            combat_rules(),
            default_ruleset().planetary_presence(),
            CombatIntelPrecision::Surveyed,
            0,
        )
        .expect("prepares");
        let finished = play_out(
            state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );

        let uncapped =
            finalize_fleet_combat(&finished, ResourceStock::ZERO, u64::MAX, combat_rules())
                .expect("finalizes");
        let capped = finalize_fleet_combat(&finished, ResourceStock::ZERO, 1, combat_rules())
            .expect("finalizes");

        assert!(!uncapped.salvage_recoverable.is_zero());
        let capped_total = capped.salvage_recovered.metal
            + capped.salvage_recovered.crystal
            + capped.salvage_recovered.fuel;
        assert!(capped_total <= 1);
    }

    #[test]
    fn finalize_produces_a_retreat_outcome_for_the_retreating_side() {
        let mut attacker_retreated = prepared();
        attacker_retreated.attacker.retreated = true;
        attacker_retreated.phase = FleetCombatPhase::Completed;
        let resolution = finalize_fleet_combat(
            &attacker_retreated,
            ResourceStock::ZERO,
            1_000,
            combat_rules(),
        )
        .expect("a retreated state finalizes");
        assert_eq!(
            resolution.outcome,
            super::super::CombatOutcome::Retreat {
                retreating_side: super::super::RetreatingSide::Attacker,
            }
        );
        assert_eq!(
            resolution.control,
            super::super::CombatControlChange::Unchanged
        );
        assert!(resolution.salvage_recovered.is_zero());
        assert!(!resolution.attacker_survivors.is_empty());

        let mut defender_retreated = prepared();
        defender_retreated.defender.retreated = true;
        defender_retreated.phase = FleetCombatPhase::Completed;
        let resolution = finalize_fleet_combat(
            &defender_retreated,
            ResourceStock::ZERO,
            1_000,
            combat_rules(),
        )
        .expect("a retreated state finalizes");
        assert_eq!(
            resolution.outcome,
            super::super::CombatOutcome::Retreat {
                retreating_side: super::super::RetreatingSide::Defender,
            }
        );
        assert_eq!(
            resolution.control,
            super::super::CombatControlChange::Unchanged
        );
        assert!(resolution.salvage_recovered.is_zero());
    }

    #[test]
    fn retreat_side_ends_the_combat_and_marks_the_side() {
        let mut state = prepared();

        retreat_side(&mut state, CombatSide::Attacker).expect("retreat succeeds");

        assert!(state.attacker.retreated);
        assert!(!state.defender.retreated);
        assert_eq!(state.phase, FleetCombatPhase::Completed);
    }

    #[test]
    fn retreat_side_rejects_an_already_completed_combat() {
        let mut state = prepared();
        retreat_side(&mut state, CombatSide::Defender).expect("first retreat succeeds");

        assert_eq!(
            retreat_side(&mut state, CombatSide::Attacker),
            Err(CombatEngineError::RoundAlreadyCompleted)
        );
    }
}

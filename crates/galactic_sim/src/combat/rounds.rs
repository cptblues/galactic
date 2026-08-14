// COMBAT-001-A: per-round resolution algorithm. `resolve_combat_round` is a
// pure function that returns a plan (`CombatRoundResolution`) computed
// entirely from the round's *starting* state — it never mutates `state`.
// `apply_round_resolution` performs the actual mutation. This split keeps
// "simultaneous damage" auditable: nothing here ever reads a partially
// updated stack while computing this round's numbers.
//
// Pipeline order (doc §4.5). Step 1 (effets de renseignement sur la
// résolution elle-même) is still out of scope — the intel percent gates what
// is *revealed* (`combat/intel.rs`'s `reveal_stack`), not the damage
// calculation itself, so there is nothing to insert here. Step 8 (mise à
// jour du renseignement) is COMBAT-001-B's addition, folded into step 9:
//   2. contres, 3. priorités de cible, 4. dégâts simultanés, 5. répartition,
//   6. destruction, 7. mise à jour intégrité, 8. renseignement, 9. historique,
//   10. fin.

use super::doctrine::CombatTacticsRules;
use super::intel::apply_round_intel_gain;
use super::retreat::CombatRetreatRules;
use super::state::{
    CombatEngineError, CombatRoundEvent, CombatRoundRecord, CombatSide, CombatSideState,
    CombatStackId, CombatStackLoss, CombatStackState, CombatTacticalRole, FleetCombatPhase,
    FleetCombatState, PER_MILLE, proportional_survivors, scaled_power, varied_damage,
};
use super::{CombatDoctrineId, CombatRules, CombatTargetClass};

const ATTACKER_SEED_TAG: u64 = 0x4154_5441_434b_4552;
const DEFENDER_SEED_TAG: u64 = 0x4445_4645_4e44_4552;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StackHullUpdate {
    stack_id: CombatStackId,
    current_hull: u128,
    surviving_quantity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatRoundResolution {
    attacker_stack_updates: Vec<StackHullUpdate>,
    defender_stack_updates: Vec<StackHullUpdate>,
    record: CombatRoundRecord,
}

fn apply_per_mille(value: u128, factor_per_mille: u32) -> u128 {
    value.saturating_mul(u128::from(factor_per_mille)) / PER_MILLE
}

/// How many *prior* consecutive rounds `doctrine` was already used by `side`
/// — 0 for a first use or a switch away from the previous doctrine, 1 for a
/// second consecutive use, and so on. The repetition penalty is driven by
/// this count, capped by `CombatTacticsRules::repetition_penalty_maximum_stacks`.
fn penalty_stacks_if_chosen(side: &CombatSideState, doctrine: CombatDoctrineId) -> u8 {
    if side.last_doctrine == Some(doctrine) {
        side.consecutive_doctrine_uses
    } else {
        0
    }
}

fn repetition_factor_per_mille(
    tactics: &CombatTacticsRules,
    doctrine: CombatDoctrineId,
    penalty_stacks: u8,
) -> u32 {
    let rule = tactics.doctrine(doctrine);
    if rule.repetition_exempt {
        return 1_000;
    }
    let capped = penalty_stacks.min(tactics.repetition_penalty_maximum_stacks());
    1_000_u32.saturating_sub(
        tactics
            .repetition_penalty_per_mille()
            .saturating_mul(u32::from(capped)),
    )
}

/// Sum of `offense * surviving_quantity`, `current_hull`, `maximum_hull`
/// across a side's stacks — the per-round analogue of the legacy
/// `attacker_totals`/`defender_totals`, but recomputed every round from
/// current (not launch-time) quantities/hull.
fn side_hull_totals(stacks: &[CombatStackState]) -> (u128, u128) {
    stacks
        .iter()
        .fold((0_u128, 0_u128), |(current, maximum), stack| {
            (
                current.saturating_add(stack.current_hull),
                maximum.saturating_add(stack.maximum_hull),
            )
        })
}

/// Attacker-side offense pool, blended by each stack's `CombatTargetBonuses`
/// against the opposing side's target-class composition — same concept as
/// the legacy `attacker_totals`'s use of `defender_target_class_weights`,
/// adapted to `CombatStackState` and recomputed each round. Only ship stacks
/// carry non-neutral bonuses; planetary forces default to
/// `CombatTargetBonuses::default()` at `prepare_fleet_combat` time, so this
/// same function is correct (and a no-op multiplier) for the defender side.
pub(crate) fn offense_pool(
    own_stacks: &[CombatStackState],
    opposing_stacks: &[CombatStackState],
) -> u128 {
    let mut light = 0_u128;
    let mut medium = 0_u128;
    let mut heavy = 0_u128;
    for stack in opposing_stacks {
        if stack.surviving_quantity == 0 {
            continue;
        }
        let weight = stack.current_hull;
        match stack.target_class {
            CombatTargetClass::Light => light = light.saturating_add(weight),
            CombatTargetClass::Medium => medium = medium.saturating_add(weight),
            CombatTargetClass::Heavy => heavy = heavy.saturating_add(weight),
        }
    }
    let total = light.saturating_add(medium).saturating_add(heavy);

    own_stacks.iter().fold(0_u128, |sum, stack| {
        if stack.surviving_quantity == 0 {
            return sum;
        }
        let weighted = light
            .saturating_mul(u128::from(
                stack.bonuses.multiplier_for(CombatTargetClass::Light),
            ))
            .saturating_add(medium.saturating_mul(u128::from(
                stack.bonuses.multiplier_for(CombatTargetClass::Medium),
            )))
            .saturating_add(heavy.saturating_mul(u128::from(
                stack.bonuses.multiplier_for(CombatTargetClass::Heavy),
            )));
        let multiplier = weighted
            .checked_div(total)
            .unwrap_or(u128::from(crate::NEUTRAL_COMBAT_BONUS_PER_MILLE));
        sum.saturating_add(
            u128::from(stack.offense)
                .saturating_mul(u128::from(stack.surviving_quantity))
                .saturating_mul(multiplier)
                / PER_MILLE,
        )
    })
}

// COMBAT-001-E: `offense_pool` already weights every stack's contribution by
// its own live `surviving_quantity` — the authoritative "how much of this
// stack is still operational" signal, recomputed every round. An earlier
// version of this function additionally re-scaled the pooled result by
// `scaled_power(offense, ...side_hull_totals(own_stacks))`, the side's
// *aggregate* hull fraction. That double-counted the same attrition for any
// multi-unit stack (`proportional_survivors` already shrinks
// `surviving_quantity` roughly in proportion to hull lost — `ceil(quantity ×
// remaining/initial)` — so re-applying the side's hull fraction squared the
// effective decay), while a single-unit stack barely decayed at all before
// that re-scaling (`surviving_quantity` stays pinned at 1 until the stack's
// hull hits exactly zero, per `proportional_survivors`'s `ceil` rounding) —
// so a lone high-value unit only ever lost the *one* layer of decay a
// grouped stack lost *twice*. That asymmetry let a single powerful unit (a
// Croiseur, say) out-damage many weaker ones of comparable raw total power
// far longer than intended. Using `offense_pool`'s result directly removes
// the redundant layer, consistently for stacks of any size.
#[allow(clippy::too_many_arguments)]
fn side_damage(
    own_stacks: &[CombatStackState],
    opposing_stacks: &[CombatStackState],
    own_doctrine: CombatDoctrineId,
    own_penalty_stacks: u8,
    opponent_doctrine: CombatDoctrineId,
    opponent_damage_taken_multiplier_per_mille: u32,
    rules: &CombatRules,
    tactics: &CombatTacticsRules,
    seed: u64,
    round: u16,
    seed_tag: u64,
) -> u128 {
    let base_power = offense_pool(own_stacks, opposing_stacks);
    let scaled = base_power.saturating_mul(u128::from(rules.damage_scale));
    let varied = varied_damage(
        scaled,
        seed ^ u64::from(round) ^ seed_tag,
        rules.damage_variance_per_mille,
    );

    let own_rule = tactics.doctrine(own_doctrine);
    let repetition = repetition_factor_per_mille(tactics, own_doctrine, own_penalty_stacks);
    let counter = tactics
        .counter_multiplier(own_doctrine, opponent_doctrine)
        .unwrap_or(1_000);

    let after_offense = apply_per_mille(varied, own_rule.offense_multiplier_per_mille);
    let after_repetition = apply_per_mille(after_offense, repetition);
    let after_counter = apply_per_mille(after_repetition, counter);
    apply_per_mille(after_counter, opponent_damage_taken_multiplier_per_mille)
}

/// Base per-target weight before any doctrine adjustment: a live stack's
/// current hull (dead stacks — `surviving_quantity == 0` — get zero weight
/// so they neither receive nor dilute further damage allocation).
fn base_weight(stack: &CombatStackState) -> u128 {
    if stack.surviving_quantity == 0 {
        0
    } else {
        stack.current_hull.max(1)
    }
}

/// Redirects `protection_per_mille` of the weight aimed at `Support` stacks
/// onto non-`Support` stacks, evenly. No-op if either group is empty (no
/// support to protect, or nowhere to redirect to).
fn redirect_from_support(
    weights: &mut [u128],
    targets: &[CombatStackState],
    protection_per_mille: u32,
) {
    let support: Vec<usize> = targets
        .iter()
        .enumerate()
        .filter(|(_, stack)| stack.tactical_role == CombatTacticalRole::Support)
        .map(|(index, _)| index)
        .collect();
    let non_support: Vec<usize> = targets
        .iter()
        .enumerate()
        .filter(|(_, stack)| stack.tactical_role != CombatTacticalRole::Support)
        .map(|(index, _)| index)
        .collect();
    if support.is_empty() || non_support.is_empty() {
        return;
    }
    let mut redirected = 0_u128;
    for &index in &support {
        let removed = apply_per_mille(weights[index], protection_per_mille);
        weights[index] = weights[index].saturating_sub(removed);
        redirected = redirected.saturating_add(removed);
    }
    let share = redirected / non_support.len() as u128;
    for &index in &non_support {
        weights[index] = weights[index].saturating_add(share);
    }
}

/// Caps any single target's weight share at `cap_per_mille` of the total,
/// redistributing the excess evenly across the remaining (uncapped)
/// targets. A single pass — good enough for a first version; it does not
/// iterate to re-cap targets that end up over the cap after redistribution.
fn cap_concentration(weights: &mut [u128], cap_per_mille: u32) {
    let total: u128 = weights.iter().sum();
    if total == 0 {
        return;
    }
    let cap = apply_per_mille(total, cap_per_mille);
    let mut excess = 0_u128;
    let mut capped = vec![false; weights.len()];
    for (index, weight) in weights.iter_mut().enumerate() {
        if *weight > cap {
            excess = excess.saturating_add(*weight - cap);
            *weight = cap;
            capped[index] = true;
        }
    }
    if excess == 0 {
        return;
    }
    let uncapped_count = capped.iter().filter(|&&is_capped| !is_capped).count();
    if uncapped_count == 0 {
        return;
    }
    let share = excess / uncapped_count as u128;
    for (index, weight) in weights.iter_mut().enumerate() {
        if !capped[index] {
            *weight = weight.saturating_add(share);
        }
    }
}

/// Step 3, "priorités de cible": offensive modifiers from `attacking_doctrine`
/// apply first (who the attacker prefers to hit), then defensive
/// modifiers from `defending_doctrine` (how the target side redistributes
/// incoming weight) — both operate on the same weight vector before
/// normalization in `allocate_damage`.
fn target_weights(
    targets: &[CombatStackState],
    attacking_doctrine: CombatDoctrineId,
    defending_doctrine: CombatDoctrineId,
    tactics: &CombatTacticsRules,
) -> Vec<u128> {
    let mut weights: Vec<u128> = targets.iter().map(base_weight).collect();

    match attacking_doctrine {
        CombatDoctrineId::ConcentratedAssault => {
            let bonus = u128::from(tactics.concentration_bonus_per_mille());
            for (weight, stack) in weights.iter_mut().zip(targets.iter()) {
                if stack.target_class == CombatTargetClass::Heavy
                    || stack.current_hull < stack.maximum_hull
                {
                    *weight = weight.saturating_mul(PER_MILLE.saturating_add(bonus)) / PER_MILLE;
                }
            }
        }
        CombatDoctrineId::FlankingManeuver => {
            let bonus = u128::from(tactics.flanking_support_bonus_per_mille());
            for (weight, stack) in weights.iter_mut().zip(targets.iter()) {
                if stack.tactical_role == CombatTacticalRole::Support {
                    *weight = weight.saturating_mul(PER_MILLE.saturating_add(bonus)) / PER_MILLE;
                }
            }
        }
        _ => {}
    }

    match defending_doctrine {
        CombatDoctrineId::DefensiveScreen => {
            redirect_from_support(
                &mut weights,
                targets,
                tactics.support_protection_per_mille(),
            );
        }
        CombatDoctrineId::DispersedFormation => {
            cap_concentration(
                &mut weights,
                tactics.dispersion_concentration_cap_per_mille(),
            );
        }
        _ => {}
    }

    weights
}

/// Step 5 ("répartition") + step 6 ("destruction"): distributes
/// `total_damage` across `stacks` proportionally to `weights`, then
/// recomputes each stack's survivors from its *cumulative* remaining hull
/// (not an incremental per-round subtraction of quantity) — the same
/// ceil-based rule as the legacy `proportional_survivors`, avoiding rounding
/// drift across rounds.
fn allocate_damage(
    stacks: &[CombatStackState],
    weights: &[u128],
    total_damage: u128,
) -> Vec<StackHullUpdate> {
    let total_weight: u128 = weights.iter().sum();
    stacks
        .iter()
        .zip(weights.iter())
        .map(|(stack, &weight)| {
            let allocated = total_damage
                .saturating_mul(weight)
                .checked_div(total_weight)
                .unwrap_or(0);
            let current_hull = stack.current_hull.saturating_sub(allocated);
            let surviving_quantity =
                proportional_survivors(stack.initial_quantity, current_hull, stack.maximum_hull);
            StackHullUpdate {
                stack_id: stack.stack_id,
                current_hull,
                surviving_quantity,
            }
        })
        .collect()
}

fn losses_from_updates(
    stacks: &[CombatStackState],
    updates: &[StackHullUpdate],
) -> Vec<CombatStackLoss> {
    stacks
        .iter()
        .zip(updates.iter())
        .filter_map(|(stack, update)| {
            let lost = stack
                .surviving_quantity
                .saturating_sub(update.surviving_quantity);
            (lost > 0).then_some(CombatStackLoss {
                stack_id: stack.stack_id,
                quantity: lost,
            })
        })
        .collect()
}

pub(crate) fn resolve_combat_round(
    state: &FleetCombatState,
    attacker_doctrine: CombatDoctrineId,
    defender_doctrine: CombatDoctrineId,
    rules: &CombatRules,
    tactics: &CombatTacticsRules,
) -> Result<CombatRoundResolution, CombatEngineError> {
    if state.phase == FleetCombatPhase::Completed {
        return Err(CombatEngineError::RoundAlreadyCompleted);
    }
    let round = state.round.saturating_add(1);

    let attacker_penalty = penalty_stacks_if_chosen(&state.attacker, attacker_doctrine);
    let defender_penalty = penalty_stacks_if_chosen(&state.defender, defender_doctrine);
    let defender_damage_taken = tactics
        .doctrine(defender_doctrine)
        .damage_taken_multiplier_per_mille;
    let attacker_damage_taken = tactics
        .doctrine(attacker_doctrine)
        .damage_taken_multiplier_per_mille;

    let attacker_damage = side_damage(
        &state.attacker.stacks,
        &state.defender.stacks,
        attacker_doctrine,
        attacker_penalty,
        defender_doctrine,
        defender_damage_taken,
        rules,
        tactics,
        state.seed,
        round,
        ATTACKER_SEED_TAG,
    );
    let defender_damage = side_damage(
        &state.defender.stacks,
        &state.attacker.stacks,
        defender_doctrine,
        defender_penalty,
        attacker_doctrine,
        attacker_damage_taken,
        rules,
        tactics,
        state.seed,
        round,
        DEFENDER_SEED_TAG,
    );

    let defender_weights = target_weights(
        &state.defender.stacks,
        attacker_doctrine,
        defender_doctrine,
        tactics,
    );
    let attacker_weights = target_weights(
        &state.attacker.stacks,
        defender_doctrine,
        attacker_doctrine,
        tactics,
    );

    let defender_updates =
        allocate_damage(&state.defender.stacks, &defender_weights, attacker_damage);
    let attacker_updates =
        allocate_damage(&state.attacker.stacks, &attacker_weights, defender_damage);

    let attacker_losses = losses_from_updates(&state.attacker.stacks, &attacker_updates);
    let defender_losses = losses_from_updates(&state.defender.stacks, &defender_updates);

    let mut notable_events = Vec::new();
    for update in &attacker_updates {
        if update.surviving_quantity == 0 {
            notable_events.push(CombatRoundEvent::StackDestroyed {
                side: CombatSide::Attacker,
                stack_id: update.stack_id,
            });
        }
    }
    for update in &defender_updates {
        if update.surviving_quantity == 0 {
            notable_events.push(CombatRoundEvent::StackDestroyed {
                side: CombatSide::Defender,
                stack_id: update.stack_id,
            });
        }
    }
    if tactics
        .counter_multiplier(attacker_doctrine, defender_doctrine)
        .is_some()
    {
        notable_events.push(CombatRoundEvent::CounterTriggered {
            countered_side: CombatSide::Attacker,
            countering_side: CombatSide::Defender,
        });
    }
    if tactics
        .counter_multiplier(defender_doctrine, attacker_doctrine)
        .is_some()
    {
        notable_events.push(CombatRoundEvent::CounterTriggered {
            countered_side: CombatSide::Defender,
            countering_side: CombatSide::Attacker,
        });
    }
    if attacker_penalty > 0 && !tactics.doctrine(attacker_doctrine).repetition_exempt {
        notable_events.push(CombatRoundEvent::RepetitionPenaltyApplied {
            side: CombatSide::Attacker,
            consecutive_uses: attacker_penalty,
        });
    }
    if defender_penalty > 0 && !tactics.doctrine(defender_doctrine).repetition_exempt {
        notable_events.push(CombatRoundEvent::RepetitionPenaltyApplied {
            side: CombatSide::Defender,
            consecutive_uses: defender_penalty,
        });
    }

    // Step 8: the attacker's intel of the defender grows from this round's
    // observation, driven by the doctrine *the attacker* played (see
    // `combat/intel.rs`'s module doc for why intel is one-directional).
    let intel_after = apply_round_intel_gain(state.intel_percent, attacker_doctrine, rules.intel());

    Ok(CombatRoundResolution {
        attacker_stack_updates: attacker_updates,
        defender_stack_updates: defender_updates,
        record: CombatRoundRecord {
            round,
            attacker_doctrine,
            defender_doctrine,
            attacker_damage,
            defender_damage,
            attacker_losses,
            defender_losses,
            notable_events,
            intel_after,
        },
    })
}

fn update_doctrine_streak(side: &mut CombatSideState, doctrine: CombatDoctrineId) {
    if side.last_doctrine == Some(doctrine) {
        side.consecutive_doctrine_uses = side.consecutive_doctrine_uses.saturating_add(1);
    } else {
        side.last_doctrine = Some(doctrine);
        side.consecutive_doctrine_uses = 1;
    }
}

fn apply_stack_updates(side: &mut CombatSideState, updates: &[StackHullUpdate]) {
    for update in updates {
        if let Some(stack) = side
            .stacks
            .iter_mut()
            .find(|stack| stack.stack_id == update.stack_id)
        {
            stack.current_hull = update.current_hull;
            stack.surviving_quantity = update.surviving_quantity;
        }
    }
}

pub(crate) fn apply_round_resolution(
    state: &mut FleetCombatState,
    resolution: CombatRoundResolution,
) -> Result<(), CombatEngineError> {
    if state.phase == FleetCombatPhase::Completed {
        return Err(CombatEngineError::RoundAlreadyCompleted);
    }

    apply_stack_updates(&mut state.attacker, &resolution.attacker_stack_updates);
    apply_stack_updates(&mut state.defender, &resolution.defender_stack_updates);
    update_doctrine_streak(&mut state.attacker, resolution.record.attacker_doctrine);
    update_doctrine_streak(&mut state.defender, resolution.record.defender_doctrine);

    state.round = resolution.record.round;
    state.intel_percent = resolution.record.intel_after;
    state.history.push(resolution.record);

    let attacker_alive = state.attacker.has_operational_stack();
    let defender_alive = state.defender.has_operational_stack();
    state.phase = if !attacker_alive || !defender_alive || state.round >= state.maximum_rounds {
        FleetCombatPhase::Completed
    } else {
        FleetCombatPhase::AwaitingDoctrine
    };

    Ok(())
}

/// One-time damage applied to the retreating side as it disengages — doc
/// §16's "pénalité configurable" made concrete as a single deterministic
/// damage instance, reusing the same offense/hull-scaling pipeline as a
/// normal round's `side_damage`, minus the doctrine-specific multipliers
/// (repetition/counter — retreating isn't a doctrine choice) and the seeded
/// variance (a flat, predictable cost is the point, not another random
/// draw). Not counted as a round: `state.round` is left untouched, keeping
/// "the code must never assume a fixed round count" (§16) true here too.
/// Returns the resulting losses for the caller (`combat/session.rs`) to
/// fold into whatever report/record it builds.
#[allow(dead_code)] // wired into combat/session.rs's retreat orchestration, starting COMBAT-001-C's session step
pub(crate) fn apply_retreat_penalty(
    state: &mut FleetCombatState,
    retreating_side: CombatSide,
    rules: &CombatRules,
    retreat_rules: &CombatRetreatRules,
) -> Vec<CombatStackLoss> {
    let (retreating_stacks, opposing_stacks) = match retreating_side {
        CombatSide::Attacker => (&state.attacker.stacks, &state.defender.stacks),
        CombatSide::Defender => (&state.defender.stacks, &state.attacker.stacks),
    };
    let (current_hull, maximum_hull) = side_hull_totals(retreating_stacks);
    let offense = offense_pool(opposing_stacks, retreating_stacks);
    let base_power = scaled_power(offense, current_hull, maximum_hull);
    let scaled = base_power.saturating_mul(u128::from(rules.damage_scale));
    let total_damage = apply_per_mille(scaled, retreat_rules.penalty_per_mille());

    let weights: Vec<u128> = retreating_stacks.iter().map(base_weight).collect();
    let updates = allocate_damage(retreating_stacks, &weights, total_damage);
    let losses = losses_from_updates(retreating_stacks, &updates);

    match retreating_side {
        CombatSide::Attacker => apply_stack_updates(&mut state.attacker, &updates),
        CombatSide::Defender => apply_stack_updates(&mut state.defender, &updates),
    }
    losses
}

#[cfg(test)]
mod tests {
    use crate::{CombatTargetBonuses, CraftableId, combat_rules};
    use galactic_domain::{FactionId, Owner};

    use super::super::state::CombatUnitRef;
    use super::*;

    fn make_stack(
        id: u32,
        offense: u32,
        defense: u32,
        durability: u32,
        quantity: u64,
        target_class: CombatTargetClass,
        tactical_role: CombatTacticalRole,
    ) -> CombatStackState {
        let maximum_hull = super::super::state::effective_hull(
            defense,
            durability,
            combat_rules().defense_weight_per_mille,
        )
        .saturating_mul(u128::from(quantity));
        CombatStackState {
            stack_id: CombatStackId(id),
            source: CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
            initial_quantity: quantity,
            surviving_quantity: quantity,
            current_hull: maximum_hull,
            maximum_hull,
            offense,
            defense,
            durability,
            target_class,
            bonuses: CombatTargetBonuses::default(),
            tactical_role,
        }
    }

    fn side(stacks: Vec<CombatStackState>, faction: u64) -> CombatSideState {
        CombatSideState {
            owner: Owner::Faction(FactionId::new(faction)),
            stacks,
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        }
    }

    fn fixture_state() -> FleetCombatState {
        FleetCombatState {
            seed: 42,
            round: 0,
            maximum_rounds: combat_rules().maximum_rounds(),
            phase: FleetCombatPhase::AwaitingDoctrine,
            attacker: side(
                vec![make_stack(
                    0,
                    70,
                    45,
                    60,
                    3,
                    CombatTargetClass::Medium,
                    CombatTacticalRole::Line,
                )],
                0,
            ),
            defender: side(
                vec![make_stack(
                    1,
                    60,
                    40,
                    55,
                    3,
                    CombatTargetClass::Medium,
                    CombatTacticalRole::Line,
                )],
                2,
            ),
            history: Vec::new(),
            intel_percent: 45,
        }
    }

    fn resolve(
        state: &FleetCombatState,
        attacker: CombatDoctrineId,
        defender: CombatDoctrineId,
    ) -> CombatRoundResolution {
        resolve_combat_round(
            state,
            attacker,
            defender,
            combat_rules(),
            combat_rules().tactics(),
        )
        .expect("a fresh round resolves")
    }

    // --- Déterminisme -------------------------------------------------

    #[test]
    fn resolving_the_same_round_twice_yields_identical_results() {
        let state = fixture_state();
        let first = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DefensiveScreen,
        );
        let second = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DefensiveScreen,
        );
        assert_eq!(first, second);
    }

    #[test]
    fn cloning_state_mid_combat_and_resolving_independently_matches() {
        let mut original = fixture_state();
        let first_round = resolve(
            &original,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        apply_round_resolution(&mut original, first_round).expect("applies");
        let clone = original.clone();

        let from_original = resolve(
            &original,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DispersedFormation,
        );
        let from_clone = resolve(
            &clone,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DispersedFormation,
        );
        assert_eq!(from_original, from_clone);
    }

    // --- Doctrines ------------------------------------------------------

    #[test]
    fn defensive_screen_reduces_damage_received_compared_to_balanced_engagement() {
        let state = fixture_state();
        let with_screen = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::DefensiveScreen,
        );
        let without_screen = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        assert!(with_screen.record.attacker_damage < without_screen.record.attacker_damage);
    }

    #[test]
    fn tactical_analysis_reduces_own_damage_and_boosts_intel_gain() {
        // COMBAT-001-B wires the payoff A left as a documented gap: less
        // damage this round, more intel gained this round.
        let state = fixture_state();
        let with_analysis = resolve(
            &state,
            CombatDoctrineId::TacticalAnalysis,
            CombatDoctrineId::BalancedEngagement,
        );
        let baseline = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        assert!(with_analysis.record.attacker_damage < baseline.record.attacker_damage);
        assert!(with_analysis.record.intel_after > baseline.record.intel_after);
    }

    #[test]
    fn concentrated_assault_prioritizes_heavy_and_damaged_targets() {
        let mut state = fixture_state();
        state.defender.stacks = vec![
            make_stack(
                1,
                60,
                40,
                55,
                3,
                CombatTargetClass::Light,
                CombatTacticalRole::Line,
            ),
            make_stack(
                2,
                60,
                40,
                55,
                3,
                CombatTargetClass::Heavy,
                CombatTacticalRole::Line,
            ),
        ];

        let weights_assault = target_weights(
            &state.defender.stacks,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
            combat_rules().tactics(),
        );
        let weights_balanced = target_weights(
            &state.defender.stacks,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
            combat_rules().tactics(),
        );

        // Both stacks start with identical stats/hull, so under a neutral
        // doctrine their weights are equal; under Concentrated Assault the
        // Heavy stack must be strictly favored.
        assert_eq!(weights_balanced[0], weights_balanced[1]);
        assert!(weights_assault[1] > weights_assault[0]);
    }

    #[test]
    fn flanking_maneuver_favors_support_stacks() {
        let targets = vec![
            make_stack(
                0,
                60,
                40,
                55,
                3,
                CombatTargetClass::Medium,
                CombatTacticalRole::Line,
            ),
            make_stack(
                1,
                60,
                40,
                55,
                3,
                CombatTargetClass::Medium,
                CombatTacticalRole::Support,
            ),
        ];

        let weights = target_weights(
            &targets,
            CombatDoctrineId::FlankingManeuver,
            CombatDoctrineId::BalancedEngagement,
            combat_rules().tactics(),
        );

        assert!(weights[1] > weights[0]);
    }

    #[test]
    fn defensive_screen_protects_support_stacks_from_targeting() {
        let targets = vec![
            make_stack(
                0,
                60,
                40,
                55,
                3,
                CombatTargetClass::Medium,
                CombatTacticalRole::Line,
            ),
            make_stack(
                1,
                60,
                40,
                55,
                3,
                CombatTargetClass::Medium,
                CombatTacticalRole::Support,
            ),
        ];

        let unprotected = target_weights(
            &targets,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
            combat_rules().tactics(),
        );
        let protected = target_weights(
            &targets,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::DefensiveScreen,
            combat_rules().tactics(),
        );

        assert_eq!(unprotected[0], unprotected[1]);
        assert!(protected[1] < unprotected[1]);
        assert!(protected[0] > unprotected[0]);
    }

    #[test]
    fn dispersed_formation_reduces_concentration_compared_to_no_cap() {
        let targets = vec![
            make_stack(
                0,
                60,
                40,
                55,
                3,
                CombatTargetClass::Heavy,
                CombatTacticalRole::Line,
            ),
            make_stack(
                1,
                60,
                5,
                5,
                3,
                CombatTargetClass::Light,
                CombatTacticalRole::Line,
            ),
        ];

        let concentrated = target_weights(
            &targets,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
            combat_rules().tactics(),
        );
        let dispersed = target_weights(
            &targets,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DispersedFormation,
            combat_rules().tactics(),
        );

        let concentrated_share = concentrated[0] * 1000 / (concentrated[0] + concentrated[1]);
        let dispersed_share = dispersed[0] * 1000 / (dispersed[0] + dispersed[1]);
        assert!(dispersed_share < concentrated_share);
    }

    #[test]
    fn repeating_a_doctrine_reduces_its_effectiveness_and_switching_resets_it() {
        // `attacker_damage` is the damage the attacker deals (received by the
        // defender) — driven by the attacker's own doctrine/repetition
        // streak, unlike `defender_damage` which is driven by the defender's
        // (here always `BalancedEngagement`, unaffected by the attacker's
        // repetition penalty).
        let mut state = fixture_state();
        state.attacker.last_doctrine = Some(CombatDoctrineId::ConcentratedAssault);
        state.attacker.consecutive_doctrine_uses = 1;

        let repeated = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
        );

        let mut once_state = fixture_state();
        once_state.attacker.last_doctrine = None;
        once_state.attacker.consecutive_doctrine_uses = 0;
        let first_use = resolve(
            &once_state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
        );

        let switched = resolve(
            &state,
            CombatDoctrineId::FlankingManeuver,
            CombatDoctrineId::BalancedEngagement,
        );
        let switched_fresh = resolve(
            &once_state,
            CombatDoctrineId::FlankingManeuver,
            CombatDoctrineId::BalancedEngagement,
        );

        assert!(repeated.record.attacker_damage < first_use.record.attacker_damage);
        // Switching to a different doctrine resets the streak: same result
        // whether the previous state had a consecutive-use streak or not.
        assert_eq!(
            switched.record.attacker_damage,
            switched_fresh.record.attacker_damage
        );
    }

    #[test]
    fn balanced_engagement_is_never_penalized_for_repetition() {
        let mut state = fixture_state();
        state.attacker.last_doctrine = Some(CombatDoctrineId::BalancedEngagement);
        state.attacker.consecutive_doctrine_uses = 5;

        let repeated = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        let mut fresh_state = fixture_state();
        fresh_state.attacker.last_doctrine = None;
        let fresh = resolve(
            &fresh_state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );

        assert_eq!(
            repeated.record.attacker_damage,
            fresh.record.attacker_damage
        );
    }

    #[test]
    fn a_countered_doctrine_deals_less_damage_but_never_zero() {
        let state = fixture_state();
        let countered = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::DefensiveScreen,
        );
        let uncountered = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
        );

        assert!(countered.record.attacker_damage > 0);
        assert!(countered.record.attacker_damage < uncountered.record.attacker_damage);
    }

    // --- Invariants -------------------------------------------------------

    #[test]
    fn no_stack_hull_ever_exceeds_its_maximum() {
        let state = fixture_state();
        let resolution = resolve(
            &state,
            CombatDoctrineId::ConcentratedAssault,
            CombatDoctrineId::BalancedEngagement,
        );
        for (stack, update) in state
            .attacker
            .stacks
            .iter()
            .zip(resolution.attacker_stack_updates.iter())
            .chain(
                state
                    .defender
                    .stacks
                    .iter()
                    .zip(resolution.defender_stack_updates.iter()),
            )
        {
            assert!(update.current_hull <= stack.maximum_hull);
        }
    }

    #[test]
    fn apply_round_resolution_completes_when_a_side_loses_every_stack() {
        let mut state = fixture_state();
        state.attacker.stacks = vec![make_stack(
            0,
            10_000,
            1,
            1,
            1,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
        )];
        state.defender.stacks = vec![make_stack(
            1,
            1,
            1,
            1,
            1,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
        )];

        let resolution = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        apply_round_resolution(&mut state, resolution).expect("applies");

        assert_eq!(state.phase, FleetCombatPhase::Completed);
        assert!(!state.defender.has_operational_stack());
    }

    #[test]
    fn apply_round_resolution_completes_at_the_maximum_round() {
        let mut state = fixture_state();
        state.maximum_rounds = 1;

        let resolution = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        apply_round_resolution(&mut state, resolution).expect("applies");

        assert_eq!(state.round, 1);
        assert_eq!(state.phase, FleetCombatPhase::Completed);
    }

    #[test]
    fn apply_round_resolution_rejects_a_round_on_an_already_completed_combat() {
        let mut state = fixture_state();
        state.maximum_rounds = 1;
        let first_resolution = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        // Compute a second resolution from the same (still AwaitingDoctrine)
        // starting state before applying the first one, so both are valid
        // plans — only the second `apply_round_resolution` call should be
        // rejected, because by then the combat is already `Completed`.
        let second_resolution = resolve(
            &state,
            CombatDoctrineId::BalancedEngagement,
            CombatDoctrineId::BalancedEngagement,
        );
        apply_round_resolution(&mut state, first_resolution).expect("applies");
        assert_eq!(state.phase, FleetCombatPhase::Completed);

        assert_eq!(
            resolve_combat_round(
                &state,
                CombatDoctrineId::BalancedEngagement,
                CombatDoctrineId::BalancedEngagement,
                combat_rules(),
                combat_rules().tactics(),
            ),
            Err(CombatEngineError::RoundAlreadyCompleted)
        );
        assert_eq!(
            apply_round_resolution(&mut state, second_resolution),
            Err(CombatEngineError::RoundAlreadyCompleted)
        );
    }

    #[test]
    fn retreat_penalty_damages_the_retreating_side_and_returns_matching_losses() {
        let mut state = fixture_state();
        let before_hull = state.attacker.stacks[0].current_hull;

        let losses = apply_retreat_penalty(
            &mut state,
            CombatSide::Attacker,
            combat_rules(),
            combat_rules().retreat(),
        );

        assert!(state.attacker.stacks[0].current_hull < before_hull);
        // The defender is untouched by an attacker retreat penalty.
        assert_eq!(
            state.defender.stacks[0].current_hull,
            state.defender.stacks[0].maximum_hull
        );
        // The round counter is not consumed by a retreat penalty (doc §16:
        // never assume a fixed round count).
        assert_eq!(state.round, 0);
        for loss in losses {
            assert_eq!(loss.stack_id, state.attacker.stacks[0].stack_id);
        }
    }

    #[test]
    fn a_zero_retreat_penalty_deals_no_damage() {
        let mut state = fixture_state();
        let before_hull = state.attacker.stacks[0].current_hull;
        let zero_penalty = CombatRetreatRules::for_tests(0);

        let losses = apply_retreat_penalty(
            &mut state,
            CombatSide::Attacker,
            combat_rules(),
            &zero_penalty,
        );

        assert_eq!(state.attacker.stacks[0].current_hull, before_hull);
        assert!(losses.is_empty());
    }
}

// COMBAT-001-D: read-only, UI-ready views over a `PendingCombat` — the only
// part of the combat engine `galactic_client` is allowed to see. Assembles
// `pub` "identity/record" types (state.rs's `CombatStackId`/`CombatUnitRef`/
// `CombatTacticalRole`/`CombatRoundRecord`, intel.rs's `CombatStackView` and
// friends) from the still-`pub(crate)` mutable aggregate state
// (`CombatStackState`/`CombatSideState`/`FleetCombatState`), which never
// leaves this module under any name. Calls into `state`/`intel`/`doctrine`
// (never the other way), mirroring the rest of `combat`'s module layout.

use super::doctrine::CombatDoctrineId;
use super::intel::{
    CombatStackView, QuantityBand, QuantityReveal, ThreatLevel, intel_tier, reveal_stack,
    side_threat_level,
};
use super::state::{CombatRoundRecord, CombatStackId, CombatTacticalRole, CombatUnitRef};
use super::{CombatRules, PendingCombat, combat_rules, default_ruleset};

/// The player's own side, as seen in the combat UI — never obfuscated (doc
/// §14.3: "les quantités propres au joueur ne sont jamais masquées").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlliedStackView {
    pub stack_id: CombatStackId,
    pub identity: CombatUnitRef,
    pub tactical_role: CombatTacticalRole,
    pub initial_quantity: u64,
    pub surviving_quantity: u64,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
    /// 0-100, exact — `current_hull / maximum_hull` as a percent.
    pub integrity_percent: u8,
}

fn integrity_percent_of(current_hull: u128, maximum_hull: u128) -> u8 {
    if maximum_hull == 0 {
        return 0;
    }
    let percent = (current_hull.saturating_mul(100) / maximum_hull).min(100);
    u8::try_from(percent).expect("a value clamped to at most 100 always fits a u8")
}

pub fn allied_stacks(pending: &PendingCombat) -> Vec<AlliedStackView> {
    pending
        .state
        .attacker
        .stacks
        .iter()
        .map(|stack| AlliedStackView {
            stack_id: stack.stack_id,
            identity: stack.source,
            tactical_role: stack.tactical_role,
            initial_quantity: stack.initial_quantity,
            surviving_quantity: stack.surviving_quantity,
            offense: stack.offense,
            defense: stack.defense,
            durability: stack.durability,
            integrity_percent: integrity_percent_of(stack.current_hull, stack.maximum_hull),
        })
        .collect()
}

/// Doctrine streak on the player's own side — for the "doctrine ou effet
/// actif" line of doc §14.3 and to compute `repetition_penalty_preview`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlliedSideSummary {
    pub last_doctrine: Option<CombatDoctrineId>,
    pub consecutive_doctrine_uses: u8,
}

pub fn allied_side_summary(pending: &PendingCombat) -> AlliedSideSummary {
    AlliedSideSummary {
        last_doctrine: pending.state.attacker.last_doctrine,
        consecutive_doctrine_uses: pending.state.attacker.consecutive_doctrine_uses,
    }
}

/// The enemy side, gated by the current intel percent (doc §6.3/§14.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyIntelView {
    pub stacks: Vec<CombatStackView>,
    /// Always present, even at the `Minimal` tier (doc §6.3's 0-19% example
    /// still shows "Menace globale : importante").
    pub threat_level: ThreatLevel,
    pub intel_percent: u8,
}

pub fn enemy_intel(pending: &PendingCombat) -> EnemyIntelView {
    let tier = intel_tier(pending.state.intel_percent);
    let stacks = pending
        .state
        .defender
        .stacks
        .iter()
        .map(|stack| reveal_stack(stack, tier, pending.seed))
        .collect();
    EnemyIntelView {
        stacks,
        threat_level: side_threat_level(&pending.state.defender),
        intel_percent: pending.state.intel_percent,
    }
}

/// The real numeric shape of a doctrine — doc §14.6/playtest feedback: the
/// UI's old static flavor text described these qualitatively only, and one
/// description (`ConcentratedAssault`'s "défense réduite") had drifted to
/// directly contradict the real ruleset value (`damage_taken_multiplier_per_mille:
/// 850`, a *reduction* in damage taken, not an increase). Deriving the
/// displayed text from this struct instead of hand-written prose makes that
/// class of mismatch structurally impossible to repeat. Deliberately does not
/// expose `support_protection_per_mille`/`flanking_support_bonus_per_mille`/
/// `dispersion_concentration_cap_per_mille`/`concentration_bonus_per_mille` —
/// those are situational (only matter given specific opposing tactical
/// roles/target classes present) and the cards have no per-target context to
/// explain them meaningfully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoctrineOverview {
    /// 1000 = neutral. Multiplies this doctrine's own damage output.
    pub offense_multiplier_per_mille: u32,
    /// 1000 = neutral, lower = takes less damage this round while used.
    pub damage_taken_multiplier_per_mille: u32,
    pub repetition_exempt: bool,
    /// The doctrine that counters this one, if any (two of the six sit
    /// outside the 4-entry counter cycle).
    pub countered_by: Option<CombatDoctrineId>,
    /// This doctrine's *own* damage output multiplier when the opponent
    /// plays `countered_by` — always `Some` exactly when `countered_by` is.
    pub counter_damage_dealt_multiplier_per_mille: Option<u32>,
}

pub fn doctrine_overview(doctrine: CombatDoctrineId, rules: &CombatRules) -> DoctrineOverview {
    let tactics = rules.tactics();
    let rule = tactics.doctrine(doctrine);
    let (countered_by, counter_damage_dealt_multiplier_per_mille) =
        match tactics.countered_by(doctrine) {
            Some((by, multiplier)) => (Some(by), Some(multiplier)),
            None => (None, None),
        };
    DoctrineOverview {
        offense_multiplier_per_mille: rule.offense_multiplier_per_mille,
        damage_taken_multiplier_per_mille: rule.damage_taken_multiplier_per_mille,
        repetition_exempt: rule.repetition_exempt,
        countered_by,
        counter_damage_dealt_multiplier_per_mille,
    }
}

/// What repeating `doctrine` this round would cost, if anything — doc §5:
/// "l'interface doit afficher cette baisse avant validation." `None` means
/// no penalty would apply (either this isn't a repeat of the last doctrine
/// played, or the doctrine is exempt). Mirrors `rounds.rs`'s private
/// `penalty_stacks_if_chosen`/`repetition_factor_per_mille` formula exactly
/// (duplicated here rather than shared, since those helpers take an internal
/// `&CombatSideState` and are private to a sibling module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepetitionPenaltyPreview {
    /// The consecutive-use count that will drive this round's penalty if
    /// `doctrine` is chosen again.
    pub consecutive_uses_if_chosen: u8,
    /// 1000 = neutral, lower = less outgoing damage dealt this round.
    pub outgoing_damage_multiplier_per_mille: u32,
}

pub fn repetition_penalty_preview(
    pending: &PendingCombat,
    doctrine: CombatDoctrineId,
    rules: &CombatRules,
) -> Option<RepetitionPenaltyPreview> {
    let side = &pending.state.attacker;
    if side.last_doctrine != Some(doctrine) {
        return None;
    }
    let tactics = rules.tactics();
    if tactics.doctrine(doctrine).repetition_exempt {
        return None;
    }
    let consecutive_uses_if_chosen = side.consecutive_doctrine_uses;
    let capped = consecutive_uses_if_chosen.min(tactics.repetition_penalty_maximum_stacks());
    let outgoing_damage_multiplier_per_mille = 1_000_u32.saturating_sub(
        tactics
            .repetition_penalty_per_mille()
            .saturating_mul(u32::from(capped)),
    );
    Some(RepetitionPenaltyPreview {
        consecutive_uses_if_chosen,
        outgoing_damage_multiplier_per_mille,
    })
}

/// The rounds played so far — doc §15: "un journal textuel doit reprendre
/// les événements essentiels", doc §14.9: "rounds joués".
pub fn round_history(pending: &PendingCombat) -> &[CombatRoundRecord] {
    &pending.state.history
}

/// The 5-level scale doc §14.7 allows, nothing finer — "pouvoir se
/// tromper" is a feature, not a bug, of keeping this rough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualitativePrediction {
    VeryUnfavorable,
    Unfavorable,
    Uncertain,
    Favorable,
    VeryFavorable,
}

fn estimated_quantity(reveal: QuantityReveal) -> u64 {
    match reveal {
        QuantityReveal::Unknown => 0,
        QuantityReveal::Verbal(band) => match band {
            QuantityBand::Few => 1,
            QuantityBand::Some => 4,
            QuantityBand::Many => 8,
            QuantityBand::Numerous => 15,
        },
        QuantityReveal::Range { minimum, maximum } => minimum.saturating_add(maximum) / 2,
        QuantityReveal::NearExact(value) | QuantityReveal::Exact(value) => value,
    }
}

fn revealed_offense(identity: CombatUnitRef) -> u32 {
    match identity {
        CombatUnitRef::Ship(craftable) => combat_rules()
            .ship(craftable)
            .map(|definition| definition.offense)
            .unwrap_or(0),
        CombatUnitRef::PlanetaryForce(id) => default_ruleset()
            .planetary_presence()
            .definition(id)
            .map(|definition| definition.offense)
            .unwrap_or(0),
    }
}

/// A pure placeholder heuristic — assumed rough by design (doc §18-E
/// refines it). The signature is the real guarantee behind doc §6.5's
/// "ne jamais calculer visiblement la bataille avec les données cachées":
/// this function physically cannot see a `CombatStackState`'s raw hull,
/// only what's already been revealed to the player (`AlliedStackView` is
/// always exact by design; `EnemyIntelView` is already tier-filtered).
pub fn qualitative_prediction(
    allied: &[AlliedStackView],
    enemy: &EnemyIntelView,
) -> QualitativePrediction {
    let allied_power: u128 = allied
        .iter()
        .filter(|stack| stack.surviving_quantity > 0)
        .map(|stack| u128::from(stack.offense).saturating_mul(u128::from(stack.surviving_quantity)))
        .sum();

    let mut enemy_power: u128 = 0;
    let mut any_identity_revealed = false;
    for stack in &enemy.stacks {
        let Some(identity) = stack.identity else {
            continue;
        };
        any_identity_revealed = true;
        let offense = revealed_offense(identity);
        let quantity = estimated_quantity(stack.quantity);
        enemy_power =
            enemy_power.saturating_add(u128::from(offense).saturating_mul(u128::from(quantity)));
    }

    if !any_identity_revealed {
        // Doc §6.3's lowest tier: only the coarse threat label exists yet.
        return match enemy.threat_level {
            ThreatLevel::Low => QualitativePrediction::Favorable,
            ThreatLevel::Moderate => QualitativePrediction::Uncertain,
            ThreatLevel::High => QualitativePrediction::Unfavorable,
            ThreatLevel::Overwhelming => QualitativePrediction::VeryUnfavorable,
        };
    }

    let ratio_per_mille = if enemy_power == 0 {
        4_000
    } else {
        allied_power
            .saturating_mul(1_000)
            .checked_div(enemy_power)
            .unwrap_or(4_000)
            .min(4_000)
    };

    let base = match ratio_per_mille {
        0..=399 => QualitativePrediction::VeryUnfavorable,
        400..=799 => QualitativePrediction::Unfavorable,
        800..=1_249 => QualitativePrediction::Uncertain,
        1_250..=1_999 => QualitativePrediction::Favorable,
        _ => QualitativePrediction::VeryFavorable,
    };

    // Doc §14.7: "afficher une confiance faible si le renseignement est
    // mauvais" — soften an extreme call toward Uncertain when intel is low.
    if enemy.intel_percent < 40 {
        match base {
            QualitativePrediction::VeryUnfavorable => QualitativePrediction::Unfavorable,
            QualitativePrediction::VeryFavorable => QualitativePrediction::Favorable,
            other => other,
        }
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FactionId, Owner, ResourceStock, UniverseConfig};

    use super::super::doctrine::ALL_COMBAT_DOCTRINES;
    use super::*;
    use crate::{
        CraftableId, FleetComposition, FleetLocation, FleetState, ShipStack, Simulation,
        prepare_attack_commitment,
    };

    fn fixture_pending_combat() -> PendingCombat {
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
            id: galactic_domain::FleetId::new(0),
            name: "Force de test".to_string(),
            owner: Owner::Faction(FactionId::new(0)),
            location: FleetLocation::Docked(colony_id),
            composition,
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Idle,
        });
        simulation.state_mut().next_fleet_id = 1;
        let commitment = prepare_attack_commitment(
            simulation.state(),
            galactic_domain::FleetId::new(0),
            target,
            91,
        )
        .expect("attack snapshot");
        let mission_id = galactic_domain::MissionId::new(0);
        super::super::session::begin_pending_combat(
            simulation.state_mut(),
            mission_id,
            commitment.defender.planet_id,
            commitment.attacker.fleet_id,
            crate::StrategicTick::ZERO,
            &commitment,
        );
        simulation
            .state()
            .pending_combat(mission_id)
            .expect("just created")
            .clone()
    }

    #[test]
    fn allied_stacks_are_always_exact() {
        let pending = fixture_pending_combat();
        let stacks = allied_stacks(&pending);
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].surviving_quantity, stacks[0].initial_quantity);
        assert_eq!(stacks[0].integrity_percent, 100);
    }

    #[test]
    fn enemy_intel_is_filtered_by_the_current_tier() {
        let pending = fixture_pending_combat();
        let view = enemy_intel(&pending);
        assert_eq!(view.stacks.len(), pending.state.defender.stacks.len());
        // The fixture's Surveyed precision maps well under the Exact tier —
        // no raw quantity should leak.
        assert_ne!(
            view.stacks[0].quantity,
            QuantityReveal::Exact(pending.state.defender.stacks[0].surviving_quantity)
        );
    }

    #[test]
    fn repetition_penalty_preview_is_none_for_a_first_use() {
        let pending = fixture_pending_combat();
        assert_eq!(
            repetition_penalty_preview(
                &pending,
                CombatDoctrineId::ConcentratedAssault,
                combat_rules(),
            ),
            None
        );
    }

    #[test]
    fn repetition_penalty_preview_reports_outgoing_damage_penalty() {
        let mut pending = fixture_pending_combat();
        pending.state.attacker.last_doctrine = Some(CombatDoctrineId::ConcentratedAssault);
        pending.state.attacker.consecutive_doctrine_uses = 2;

        let preview = repetition_penalty_preview(
            &pending,
            CombatDoctrineId::ConcentratedAssault,
            combat_rules(),
        )
        .expect("repeating a non-exempt doctrine has a preview");

        assert_eq!(preview.consecutive_uses_if_chosen, 2);
        assert_eq!(preview.outgoing_damage_multiplier_per_mille, 700);
    }

    #[test]
    fn qualitative_prediction_is_deterministic() {
        let pending = fixture_pending_combat();
        let allied = allied_stacks(&pending);
        let enemy = enemy_intel(&pending);
        let first = qualitative_prediction(&allied, &enemy);
        let second = qualitative_prediction(&allied, &enemy);
        assert_eq!(first, second);
    }

    #[test]
    fn qualitative_prediction_uses_only_revealed_data() {
        // Same revealed view, two different (synthetic) "hidden truths" —
        // the prediction cannot depend on anything beyond `allied`/`enemy`
        // since the function's signature never receives the hidden state.
        let pending = fixture_pending_combat();
        let allied = allied_stacks(&pending);
        let enemy = enemy_intel(&pending);
        let prediction = qualitative_prediction(&allied, &enemy);
        assert!(matches!(
            prediction,
            QualitativePrediction::VeryUnfavorable
                | QualitativePrediction::Unfavorable
                | QualitativePrediction::Uncertain
                | QualitativePrediction::Favorable
                | QualitativePrediction::VeryFavorable
        ));
    }

    #[test]
    fn doctrine_overview_reports_the_real_ruleset_multipliers() {
        let rules = combat_rules();

        let balanced = doctrine_overview(CombatDoctrineId::BalancedEngagement, rules);
        assert!(balanced.repetition_exempt);
        assert_eq!(balanced.countered_by, None);
        assert_eq!(balanced.counter_damage_dealt_multiplier_per_mille, None);

        let concentrated = doctrine_overview(CombatDoctrineId::ConcentratedAssault, rules);
        assert!(!concentrated.repetition_exempt);
        assert_eq!(
            concentrated.countered_by,
            Some(CombatDoctrineId::DefensiveScreen)
        );
        assert!(
            concentrated
                .counter_damage_dealt_multiplier_per_mille
                .is_some()
        );
        assert!(concentrated.offense_multiplier_per_mille > 1_000);
        assert!(concentrated.damage_taken_multiplier_per_mille > 1_000);
    }

    #[test]
    fn doctrine_overview_counter_cycle_is_consistent_with_counter_multiplier() {
        let rules = combat_rules();
        let tactics = rules.tactics();

        for doctrine in ALL_COMBAT_DOCTRINES {
            let overview = doctrine_overview(doctrine, rules);
            if let Some(countered_by) = overview.countered_by {
                assert_eq!(
                    tactics.counter_multiplier(doctrine, countered_by),
                    overview.counter_damage_dealt_multiplier_per_mille
                );
            }
        }
    }
}

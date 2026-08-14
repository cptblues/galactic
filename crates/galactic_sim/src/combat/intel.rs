// COMBAT-001-B: combat intel — a single percentage (0-100) representing the
// *attacker's* knowledge of the *defender's* composition. Deliberately
// one-directional: only the player attacks in this game today (no enemy
// attacks before MVP-039), and the defending side is always AI-controlled —
// `ai::choose_ai_doctrine` reads the real `CombatSideState` directly, no
// fog-of-war needed for its own decisions (doc §13's documented "anti-triche"
// allowance). A symmetric `defender_of_attacker` value would be an additive
// field for a future ticket, not a refactor, if ever needed.

use serde::Deserialize;

use crate::CombatTargetClass;

use super::CombatRulesError;
use super::doctrine::CombatDoctrineId;
use super::state::{CombatSideState, CombatStackId, CombatStackState, CombatUnitRef, splitmix64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatIntelPrecision {
    Contact,
    Surveyed,
    Exact,
}

pub(crate) struct CombatIntelSourceInputs {
    /// Precision of the `PlanetaryIntelligenceReport` at the moment combat
    /// begins (i.e. at mission arrival, not launch — staleness is measured
    /// from the same moment).
    pub(crate) precision: CombatIntelPrecision,
    /// `resolved_at - report.observed_at`, in strategic ticks.
    pub(crate) report_age_ticks: u64,
    /// Whether `CraftableId::LIGHT_PROBE` is present in the attacker's
    /// fleet — the closest existing proxy for "vaisseau de reconnaissance
    /// approprié" (doc §6.2); no dedicated detection stat exists in the
    /// game today, see the COMBAT-001-B plan's constat.
    pub(crate) has_reconnaissance_ship: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatIntelRules {
    contact_base_percent: u8,
    surveyed_base_percent: u8,
    exact_base_percent: u8,
    staleness_penalty_percent_per_interval: u8,
    staleness_interval_ticks: u64,
    staleness_penalty_maximum_percent: u8,
    reconnaissance_bonus_percent: u8,
    round_gain_percent: u8,
    tactical_analysis_bonus_percent: u8,
    minimum_percent: u8,
    maximum_percent: u8,
}

impl CombatIntelRules {
    pub(crate) fn from_config(config: CombatIntelRulesConfig) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
            return Err(CombatRulesError::InvalidIntelVersion(config.version));
        }
        if config.minimum_percent == 0 || config.minimum_percent > config.maximum_percent {
            return Err(CombatRulesError::InvalidIntelBounds);
        }
        if config.maximum_percent > 100 {
            return Err(CombatRulesError::InvalidIntelBounds);
        }
        for base in [
            config.contact_base_percent,
            config.surveyed_base_percent,
            config.exact_base_percent,
        ] {
            if base > 100 {
                return Err(CombatRulesError::InvalidIntelBasePercent);
            }
        }
        if config.contact_base_percent > config.surveyed_base_percent
            || config.surveyed_base_percent > config.exact_base_percent
        {
            return Err(CombatRulesError::InvalidIntelBasePercent);
        }
        if config.staleness_interval_ticks == 0 || config.staleness_penalty_maximum_percent > 100 {
            return Err(CombatRulesError::InvalidIntelStaleness);
        }
        if config.round_gain_percent > 100 || config.tactical_analysis_bonus_percent > 100 {
            return Err(CombatRulesError::InvalidIntelGain);
        }

        Ok(Self {
            contact_base_percent: config.contact_base_percent,
            surveyed_base_percent: config.surveyed_base_percent,
            exact_base_percent: config.exact_base_percent,
            staleness_penalty_percent_per_interval: config.staleness_penalty_percent_per_interval,
            staleness_interval_ticks: config.staleness_interval_ticks,
            staleness_penalty_maximum_percent: config.staleness_penalty_maximum_percent,
            reconnaissance_bonus_percent: config.reconnaissance_bonus_percent,
            round_gain_percent: config.round_gain_percent,
            tactical_analysis_bonus_percent: config.tactical_analysis_bonus_percent,
            minimum_percent: config.minimum_percent,
            maximum_percent: config.maximum_percent,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CombatIntelRulesConfig {
    version: u32,
    contact_base_percent: u8,
    surveyed_base_percent: u8,
    exact_base_percent: u8,
    staleness_penalty_percent_per_interval: u8,
    staleness_interval_ticks: u64,
    staleness_penalty_maximum_percent: u8,
    reconnaissance_bonus_percent: u8,
    round_gain_percent: u8,
    tactical_analysis_bonus_percent: u8,
    minimum_percent: u8,
    maximum_percent: u8,
}

/// Base intel percent from the source inputs, before any per-round gain.
/// Bounded `[minimum_percent, maximum_percent]` (doc §6.2: "valeur finale
/// limitée entre 5 et 100").
pub(crate) fn base_intel_percent(inputs: CombatIntelSourceInputs, rules: &CombatIntelRules) -> u8 {
    let base = match inputs.precision {
        CombatIntelPrecision::Contact => rules.contact_base_percent,
        CombatIntelPrecision::Surveyed => rules.surveyed_base_percent,
        CombatIntelPrecision::Exact => rules.exact_base_percent,
    };
    let staleness_intervals = inputs.report_age_ticks / rules.staleness_interval_ticks;
    let staleness_penalty = staleness_intervals
        .saturating_mul(u64::from(rules.staleness_penalty_percent_per_interval))
        .min(u64::from(rules.staleness_penalty_maximum_percent));
    let after_staleness = u64::from(base).saturating_sub(staleness_penalty);
    let with_recon = if inputs.has_reconnaissance_ship {
        after_staleness.saturating_add(u64::from(rules.reconnaissance_bonus_percent))
    } else {
        after_staleness
    };
    u8::try_from(with_recon.clamp(
        u64::from(rules.minimum_percent),
        u64::from(rules.maximum_percent),
    ))
    .expect("clamped to a u8-representable range")
}

/// Per-round intel gain (doc §6.2: "chaque round observé : +5 points";
/// `TacticalAnalysis` : "bonus supplémentaire"). Never exceeds
/// `rules.maximum_percent`.
pub(crate) fn apply_round_intel_gain(
    current: u8,
    doctrine_used: CombatDoctrineId,
    rules: &CombatIntelRules,
) -> u8 {
    let gain = if doctrine_used == CombatDoctrineId::TacticalAnalysis {
        rules
            .round_gain_percent
            .saturating_add(rules.tactical_analysis_bonus_percent)
    } else {
        rules.round_gain_percent
    };
    current.saturating_add(gain).min(rules.maximum_percent)
}

/// The six revelation tiers of doc §6.3, derived purely from the intel
/// percent — no separate configuration, the boundaries are a structural part
/// of the doc's design, not a balance lever (unlike the percent computation
/// itself, which is fully ruleset-driven).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatIntelTier {
    Minimal,
    Rough,
    Approximate,
    Detailed,
    NearExact,
    Exact,
}

pub fn intel_tier(percent: u8) -> CombatIntelTier {
    match percent {
        0..=19 => CombatIntelTier::Minimal,
        20..=39 => CombatIntelTier::Rough,
        40..=59 => CombatIntelTier::Approximate,
        60..=79 => CombatIntelTier::Detailed,
        80..=94 => CombatIntelTier::NearExact,
        _ => CombatIntelTier::Exact,
    }
}

/// Coarse, non-obfuscated threat label for the `Minimal` tier (doc §6.3,
/// 0-19 %: "menace globale" is the only thing shown — no per-stack data at
/// all). Derived directly from the true totals: at this tier there is
/// nothing precise enough to obfuscate in the first place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatLevel {
    Low,
    Moderate,
    High,
    Overwhelming,
}

pub(crate) fn side_threat_level(side: &CombatSideState) -> ThreatLevel {
    let total_offense: u128 = side
        .stacks
        .iter()
        .filter(|stack| stack.surviving_quantity > 0)
        .map(|stack| u128::from(stack.offense).saturating_mul(u128::from(stack.surviving_quantity)))
        .sum();
    match total_offense {
        0..=199 => ThreatLevel::Low,
        200..=599 => ThreatLevel::Moderate,
        600..=1_499 => ThreatLevel::High,
        _ => ThreatLevel::Overwhelming,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityBand {
    Few,
    Some,
    Many,
    Numerous,
}

fn quantity_band(quantity: u64) -> QuantityBand {
    match quantity {
        0..=2 => QuantityBand::Few,
        3..=5 => QuantityBand::Some,
        6..=10 => QuantityBand::Many,
        _ => QuantityBand::Numerous,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityReveal {
    Unknown,
    Verbal(QuantityBand),
    Range { minimum: u64, maximum: u64 },
    NearExact(u64),
    Exact(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityBand {
    Critical,
    Low,
    Moderate,
    High,
}

fn integrity_percent(stack: &CombatStackState) -> u8 {
    if stack.maximum_hull == 0 {
        return 0;
    }
    let percent = (stack.current_hull.saturating_mul(100) / stack.maximum_hull).min(100);
    u8::try_from(percent).expect("a value clamped to at most 100 always fits a u8")
}

fn integrity_band(percent: u8) -> IntegrityBand {
    match percent {
        0..=24 => IntegrityBand::Critical,
        25..=49 => IntegrityBand::Low,
        50..=74 => IntegrityBand::Moderate,
        _ => IntegrityBand::High,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityReveal {
    Unknown,
    Qualitative(IntegrityBand),
    Exact(u8),
}

/// Distinguishes the obfuscation draw used for a `Range` quantity from the
/// one used for a `NearExact` quantity, so the two tiers never derive the
/// same jitter from the same seed/stack pair — folded into the seed formula
/// alongside the combat seed and the group id (doc §6.4).
const RANGE_OBFUSCATION_TAG: u64 = 0x5241_4e47_4520_5441;
const NEAR_EXACT_OBFUSCATION_TAG: u64 = 0x4e45_4152_5f45_5841;

/// Widths as a per-mille fraction of the true quantity (minimum 1 unit) —
/// `Approximate` gets doc §6.3's "fourchette large", `Detailed` its
/// narrower "erreur déterministe".
const APPROXIMATE_RANGE_WIDTH_PER_MILLE: u64 = 600;
const DETAILED_RANGE_WIDTH_PER_MILLE: u64 = 250;
const NEAR_EXACT_MARGIN: u64 = 2;

/// A stable pseudo-random draw derived only from the combat seed, the
/// group's stable id and a tag — never resampled, so two calls with the
/// same inputs always agree (doc §6.4's non-negotiable requirement).
fn deterministic_draw(seed: u64, stack_id: CombatStackId, tag: u64, modulo: u64) -> u64 {
    if modulo == 0 {
        return 0;
    }
    splitmix64(seed ^ u64::from(stack_id.0) ^ tag) % modulo
}

fn obfuscated_range(
    truth: u64,
    stack_id: CombatStackId,
    seed: u64,
    width_per_mille: u64,
) -> (u64, u64) {
    let width = truth.saturating_mul(width_per_mille) / 1_000;
    let width = width.max(1);
    let jitter = deterministic_draw(seed, stack_id, RANGE_OBFUSCATION_TAG, width + 1);
    let minimum = truth.saturating_sub(width - jitter);
    let maximum = truth.saturating_add(jitter);
    (minimum, maximum)
}

fn obfuscated_near_exact(truth: u64, stack_id: CombatStackId, seed: u64) -> u64 {
    let span = NEAR_EXACT_MARGIN * 2 + 1;
    let offset = deterministic_draw(seed, stack_id, NEAR_EXACT_OBFUSCATION_TAG, span);
    truth
        .saturating_add(NEAR_EXACT_MARGIN)
        .saturating_sub(offset)
}

fn quantity_reveal(stack: &CombatStackState, tier: CombatIntelTier, seed: u64) -> QuantityReveal {
    let truth = stack.surviving_quantity;
    match tier {
        CombatIntelTier::Minimal => QuantityReveal::Unknown,
        CombatIntelTier::Rough => QuantityReveal::Verbal(quantity_band(truth)),
        CombatIntelTier::Approximate => {
            let (minimum, maximum) = obfuscated_range(
                truth,
                stack.stack_id,
                seed,
                APPROXIMATE_RANGE_WIDTH_PER_MILLE,
            );
            QuantityReveal::Range { minimum, maximum }
        }
        CombatIntelTier::Detailed => {
            let (minimum, maximum) =
                obfuscated_range(truth, stack.stack_id, seed, DETAILED_RANGE_WIDTH_PER_MILLE);
            QuantityReveal::Range { minimum, maximum }
        }
        CombatIntelTier::NearExact => {
            QuantityReveal::NearExact(obfuscated_near_exact(truth, stack.stack_id, seed))
        }
        CombatIntelTier::Exact => QuantityReveal::Exact(truth),
    }
}

fn integrity_reveal(stack: &CombatStackState, tier: CombatIntelTier) -> IntegrityReveal {
    match tier {
        CombatIntelTier::Minimal | CombatIntelTier::Rough | CombatIntelTier::Approximate => {
            IntegrityReveal::Unknown
        }
        CombatIntelTier::Detailed | CombatIntelTier::NearExact => {
            IntegrityReveal::Qualitative(integrity_band(integrity_percent(stack)))
        }
        CombatIntelTier::Exact => IntegrityReveal::Exact(integrity_percent(stack)),
    }
}

/// General class from `Rough` onward (doc §6.3, 20-39 %: "classes
/// générales"); the `Minimal` tier (0-19 %) shows nothing per-stack at all.
fn target_class_reveal(
    stack: &CombatStackState,
    tier: CombatIntelTier,
) -> Option<CombatTargetClass> {
    match tier {
        CombatIntelTier::Minimal => None,
        _ => Some(stack.target_class),
    }
}

/// Precise identity from `Approximate` onward (doc §6.3, 40-59 %: "types
/// identifiés"); `Rough` (20-39 %) explicitly withholds it ("aucune
/// identité précise").
fn identity_reveal(stack: &CombatStackState, tier: CombatIntelTier) -> Option<CombatUnitRef> {
    match tier {
        CombatIntelTier::Minimal | CombatIntelTier::Rough => None,
        _ => Some(stack.source),
    }
}

/// The obfuscated view of a single group, gated by `tier`. Never exposes a
/// stack's raw `current_hull`/`offense`/`defense`/`durability`/`bonuses` —
/// only `target_class`, `identity` and the already-rounded/obfuscated
/// quantity/integrity values (doc §6.5, "prévention des fuites
/// d'information").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatStackView {
    pub stack_id: CombatStackId,
    pub target_class: Option<CombatTargetClass>,
    pub identity: Option<CombatUnitRef>,
    pub quantity: QuantityReveal,
    pub integrity: IntegrityReveal,
}

/// Deliberately does not add the `Exact` tier's (95-100 %, doc §6.3)
/// "doctrine probable"/"priorité de cible probable" predictions: those
/// require a *differentiated* per-faction AI profile to predict *from*, and
/// B builds only a single generic rule-based profile (see `ai.rs`'s module
/// doc — differentiation is COMBAT-001-E territory). Inventing a prediction
/// against today's one-profile AI would be a hollow gadget, not a real
/// forecast, so B stops at the structured stack/quantity/integrity reveals
/// this function returns and leaves the prediction fields for whichever
/// ticket actually has faction profiles to predict from.
#[allow(dead_code)] // wired into the combat UI in COMBAT-001-D
pub(crate) fn reveal_stack(
    stack: &CombatStackState,
    tier: CombatIntelTier,
    seed: u64,
) -> CombatStackView {
    CombatStackView {
        stack_id: stack.stack_id,
        target_class: target_class_reveal(stack, tier),
        identity: identity_reveal(stack, tier),
        quantity: quantity_reveal(stack, tier, seed),
        integrity: integrity_reveal(stack, tier),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> CombatIntelRules {
        CombatIntelRules::from_config(CombatIntelRulesConfig {
            version: 1,
            contact_base_percent: 20,
            surveyed_base_percent: 45,
            exact_base_percent: 70,
            staleness_penalty_percent_per_interval: 2,
            staleness_interval_ticks: 600,
            staleness_penalty_maximum_percent: 20,
            reconnaissance_bonus_percent: 10,
            round_gain_percent: 5,
            tactical_analysis_bonus_percent: 15,
            minimum_percent: 5,
            maximum_percent: 100,
        })
        .expect("valid config")
    }

    fn inputs(
        precision: CombatIntelPrecision,
        report_age_ticks: u64,
        has_reconnaissance_ship: bool,
    ) -> CombatIntelSourceInputs {
        CombatIntelSourceInputs {
            precision,
            report_age_ticks,
            has_reconnaissance_ship,
        }
    }

    #[test]
    fn base_percent_increases_with_precision() {
        let rules = rules();
        let contact = base_intel_percent(inputs(CombatIntelPrecision::Contact, 0, false), &rules);
        let surveyed = base_intel_percent(inputs(CombatIntelPrecision::Surveyed, 0, false), &rules);
        let exact = base_intel_percent(inputs(CombatIntelPrecision::Exact, 0, false), &rules);
        assert!(contact < surveyed);
        assert!(surveyed < exact);
        assert_eq!(surveyed, 45);
    }

    #[test]
    fn staleness_reduces_the_base_percent_but_is_capped() {
        let rules = rules();
        let fresh = base_intel_percent(inputs(CombatIntelPrecision::Surveyed, 0, false), &rules);
        let stale = base_intel_percent(inputs(CombatIntelPrecision::Surveyed, 600, false), &rules);
        let ancient = base_intel_percent(
            inputs(CombatIntelPrecision::Surveyed, 1_000_000, false),
            &rules,
        );
        assert!(stale < fresh);
        assert_eq!(fresh - stale, 2);
        // Capped at staleness_penalty_maximum_percent (20), never below minimum_percent (5).
        assert_eq!(ancient, (45_u8).saturating_sub(20));
    }

    #[test]
    fn reconnaissance_ship_adds_a_flat_bonus() {
        let rules = rules();
        let without = base_intel_percent(inputs(CombatIntelPrecision::Surveyed, 0, false), &rules);
        let with = base_intel_percent(inputs(CombatIntelPrecision::Surveyed, 0, true), &rules);
        assert_eq!(with, without + 10);
    }

    #[test]
    fn base_percent_never_leaves_the_configured_bounds() {
        let rules = rules();
        let worst = base_intel_percent(
            inputs(CombatIntelPrecision::Contact, 1_000_000, false),
            &rules,
        );
        assert!(worst >= 5);
        let best = base_intel_percent(inputs(CombatIntelPrecision::Exact, 0, true), &rules);
        assert!(best <= 100);
    }

    #[test]
    fn round_gain_increments_and_caps_at_the_maximum() {
        let rules = rules();
        assert_eq!(
            apply_round_intel_gain(50, CombatDoctrineId::BalancedEngagement, &rules),
            55
        );
        assert_eq!(
            apply_round_intel_gain(98, CombatDoctrineId::BalancedEngagement, &rules),
            100
        );
    }

    #[test]
    fn tactical_analysis_grants_an_extra_gain() {
        let rules = rules();
        let normal = apply_round_intel_gain(50, CombatDoctrineId::BalancedEngagement, &rules);
        let analysis = apply_round_intel_gain(50, CombatDoctrineId::TacticalAnalysis, &rules);
        assert!(analysis > normal);
        assert_eq!(analysis - normal, 15);
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        let mut config = CombatIntelRulesConfig {
            version: 2,
            contact_base_percent: 20,
            surveyed_base_percent: 45,
            exact_base_percent: 70,
            staleness_penalty_percent_per_interval: 2,
            staleness_interval_ticks: 600,
            staleness_penalty_maximum_percent: 20,
            reconnaissance_bonus_percent: 10,
            round_gain_percent: 5,
            tactical_analysis_bonus_percent: 15,
            minimum_percent: 5,
            maximum_percent: 100,
        };
        assert_eq!(
            CombatIntelRules::from_config(config.clone()).unwrap_err(),
            CombatRulesError::InvalidIntelVersion(2)
        );
        config.version = 1;
        config.minimum_percent = 0;
        assert_eq!(
            CombatIntelRules::from_config(config).unwrap_err(),
            CombatRulesError::InvalidIntelBounds
        );
    }

    use galactic_domain::{FactionId, Owner};

    use super::super::state::CombatTacticalRole;
    use crate::CraftableId;

    #[test]
    fn tier_boundaries_match_the_documented_percent_ranges() {
        assert_eq!(intel_tier(0), CombatIntelTier::Minimal);
        assert_eq!(intel_tier(19), CombatIntelTier::Minimal);
        assert_eq!(intel_tier(20), CombatIntelTier::Rough);
        assert_eq!(intel_tier(39), CombatIntelTier::Rough);
        assert_eq!(intel_tier(40), CombatIntelTier::Approximate);
        assert_eq!(intel_tier(59), CombatIntelTier::Approximate);
        assert_eq!(intel_tier(60), CombatIntelTier::Detailed);
        assert_eq!(intel_tier(79), CombatIntelTier::Detailed);
        assert_eq!(intel_tier(80), CombatIntelTier::NearExact);
        assert_eq!(intel_tier(94), CombatIntelTier::NearExact);
        assert_eq!(intel_tier(95), CombatIntelTier::Exact);
        assert_eq!(intel_tier(100), CombatIntelTier::Exact);
    }

    fn view_stack(quantity: u64, current_hull: u128, maximum_hull: u128) -> CombatStackState {
        CombatStackState {
            stack_id: CombatStackId(7),
            source: CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
            initial_quantity: quantity,
            surviving_quantity: quantity,
            current_hull,
            maximum_hull,
            offense: 50,
            defense: 40,
            durability: 60,
            target_class: CombatTargetClass::Medium,
            bonuses: crate::CombatTargetBonuses::default(),
            tactical_role: CombatTacticalRole::Line,
        }
    }

    #[test]
    fn minimal_tier_reveals_nothing_per_stack() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::Minimal, 42);
        assert_eq!(view.target_class, None);
        assert_eq!(view.identity, None);
        assert_eq!(view.quantity, QuantityReveal::Unknown);
        assert_eq!(view.integrity, IntegrityReveal::Unknown);
    }

    #[test]
    fn rough_tier_reveals_class_and_a_verbal_quantity_but_no_identity() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::Rough, 42);
        assert_eq!(view.target_class, Some(CombatTargetClass::Medium));
        assert_eq!(view.identity, None);
        assert_eq!(view.quantity, QuantityReveal::Verbal(QuantityBand::Many));
        assert_eq!(view.integrity, IntegrityReveal::Unknown);
    }

    #[test]
    fn approximate_tier_reveals_identity_and_a_wide_range() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::Approximate, 42);
        assert_eq!(
            view.identity,
            Some(CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK))
        );
        assert_eq!(view.integrity, IntegrityReveal::Unknown);
        match view.quantity {
            QuantityReveal::Range { minimum, maximum } => {
                assert!(minimum <= 9 && 9 <= maximum);
            }
            other => panic!("expected a range, got {other:?}"),
        }
    }

    #[test]
    fn detailed_tier_reveals_a_narrower_range_and_qualitative_integrity() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::Detailed, 42);
        assert_eq!(
            view.integrity,
            IntegrityReveal::Qualitative(IntegrityBand::Moderate)
        );
        let (approximate_minimum, approximate_maximum) =
            obfuscated_range(9, CombatStackId(7), 42, APPROXIMATE_RANGE_WIDTH_PER_MILLE);
        match view.quantity {
            QuantityReveal::Range { minimum, maximum } => {
                assert!(minimum <= 9 && 9 <= maximum);
                assert!(maximum - minimum <= approximate_maximum - approximate_minimum);
            }
            other => panic!("expected a range, got {other:?}"),
        }
    }

    #[test]
    fn near_exact_tier_reveals_a_value_within_a_small_margin() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::NearExact, 42);
        assert_eq!(
            view.integrity,
            IntegrityReveal::Qualitative(IntegrityBand::Moderate)
        );
        match view.quantity {
            QuantityReveal::NearExact(value) => {
                assert!(value.abs_diff(9) <= NEAR_EXACT_MARGIN);
            }
            other => panic!("expected a near-exact value, got {other:?}"),
        }
    }

    #[test]
    fn exact_tier_reveals_the_true_quantity_and_integrity() {
        let stack = view_stack(9, 700, 1_000);
        let view = reveal_stack(&stack, CombatIntelTier::Exact, 42);
        assert_eq!(view.quantity, QuantityReveal::Exact(9));
        assert_eq!(view.integrity, IntegrityReveal::Exact(70));
    }

    #[test]
    fn no_tier_below_exact_leaks_the_raw_quantity_or_integrity() {
        let stack = view_stack(9, 700, 1_000);
        for tier in [
            CombatIntelTier::Minimal,
            CombatIntelTier::Rough,
            CombatIntelTier::Approximate,
            CombatIntelTier::Detailed,
            CombatIntelTier::NearExact,
        ] {
            let view = reveal_stack(&stack, tier, 42);
            assert_ne!(view.quantity, QuantityReveal::Exact(9));
            assert_ne!(view.integrity, IntegrityReveal::Exact(70));
        }
    }

    #[test]
    fn revealing_the_same_stack_twice_yields_the_identical_view() {
        let stack = view_stack(9, 700, 1_000);
        let first = reveal_stack(&stack, CombatIntelTier::Detailed, 42);
        let second = reveal_stack(&stack, CombatIntelTier::Detailed, 42);
        assert_eq!(first, second);
    }

    #[test]
    fn a_different_seed_can_change_the_obfuscated_value() {
        let stack = view_stack(9, 700, 1_000);
        let a = reveal_stack(&stack, CombatIntelTier::Detailed, 1);
        let b = reveal_stack(&stack, CombatIntelTier::Detailed, 2);
        // Not asserting inequality (a collision is possible and not a bug),
        // only that both remain valid, truth-containing ranges.
        for view in [a, b] {
            match view.quantity {
                QuantityReveal::Range { minimum, maximum } => assert!(minimum <= 9 && 9 <= maximum),
                other => panic!("expected a range, got {other:?}"),
            }
        }
    }

    fn threat_side(offense: u32, quantity: u64) -> CombatSideState {
        CombatSideState {
            owner: Owner::Faction(FactionId::new(0)),
            stacks: vec![CombatStackState {
                stack_id: CombatStackId(1),
                source: CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
                initial_quantity: quantity,
                surviving_quantity: quantity,
                current_hull: 1_000,
                maximum_hull: 1_000,
                offense,
                defense: 10,
                durability: 10,
                target_class: CombatTargetClass::Medium,
                bonuses: crate::CombatTargetBonuses::default(),
                tactical_role: CombatTacticalRole::Line,
            }],
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        }
    }

    #[test]
    fn threat_level_increases_with_total_offense() {
        assert_eq!(side_threat_level(&threat_side(10, 5)), ThreatLevel::Low);
        assert_eq!(
            side_threat_level(&threat_side(100, 5)),
            ThreatLevel::Moderate
        );
        assert_eq!(side_threat_level(&threat_side(200, 5)), ThreatLevel::High);
        assert_eq!(
            side_threat_level(&threat_side(1_000, 5)),
            ThreatLevel::Overwhelming
        );
    }
}

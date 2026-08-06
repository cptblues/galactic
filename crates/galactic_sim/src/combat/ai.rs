// COMBAT-001-B: a single rule-based AI profile, replacing COMBAT-001-A's
// seed-derived random `doctrine_pick` placeholder. Deterministic and
// side-agnostic (no fog-of-war for its own decisions, doc §13's documented
// "anti-triche" allowance) — used for both the attacker and the defender by
// the auto-resolve façade, since no interactive player choice exists yet
// (COMBAT-001-C/D). Per-faction differentiation is out of scope here (the
// user explicitly chose "one profile now, faction variants later if needed"
// when resolving a doc/comment discrepancy over which sub-ticket owns this).

use serde::Deserialize;

use super::CombatRulesError;
use super::CombatTargetClass;
use super::doctrine::{ALL_COMBAT_DOCTRINES, CombatDoctrineId};
use super::state::{CombatSideState, CombatTacticalRole};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatAiRules {
    concentrated_assault_target_bonus: u32,
    defensive_screen_damage_taken_bonus: u32,
    flanking_support_bonus: u32,
    dispersed_formation_counter_bonus: u32,
    tactical_analysis_first_round_bonus: u32,
    balanced_engagement_base_score: u32,
    repetition_avoidance_penalty: u32,
    repetition_avoidance_threshold: u8,
}

impl CombatAiRules {
    pub(crate) fn from_config(config: CombatAiRulesConfig) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
            return Err(CombatRulesError::InvalidAiVersion(config.version));
        }
        if config.repetition_avoidance_threshold == 0 {
            return Err(CombatRulesError::InvalidAiScore);
        }

        Ok(Self {
            concentrated_assault_target_bonus: config.concentrated_assault_target_bonus,
            defensive_screen_damage_taken_bonus: config.defensive_screen_damage_taken_bonus,
            flanking_support_bonus: config.flanking_support_bonus,
            dispersed_formation_counter_bonus: config.dispersed_formation_counter_bonus,
            tactical_analysis_first_round_bonus: config.tactical_analysis_first_round_bonus,
            balanced_engagement_base_score: config.balanced_engagement_base_score,
            repetition_avoidance_penalty: config.repetition_avoidance_penalty,
            repetition_avoidance_threshold: config.repetition_avoidance_threshold,
        })
    }

    pub(crate) const fn concentrated_assault_target_bonus(&self) -> u32 {
        self.concentrated_assault_target_bonus
    }

    pub(crate) const fn defensive_screen_damage_taken_bonus(&self) -> u32 {
        self.defensive_screen_damage_taken_bonus
    }

    pub(crate) const fn flanking_support_bonus(&self) -> u32 {
        self.flanking_support_bonus
    }

    pub(crate) const fn dispersed_formation_counter_bonus(&self) -> u32 {
        self.dispersed_formation_counter_bonus
    }

    pub(crate) const fn tactical_analysis_first_round_bonus(&self) -> u32 {
        self.tactical_analysis_first_round_bonus
    }

    pub(crate) const fn balanced_engagement_base_score(&self) -> u32 {
        self.balanced_engagement_base_score
    }

    pub(crate) const fn repetition_avoidance_penalty(&self) -> u32 {
        self.repetition_avoidance_penalty
    }

    pub(crate) const fn repetition_avoidance_threshold(&self) -> u8 {
        self.repetition_avoidance_threshold
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CombatAiRulesConfig {
    version: u32,
    concentrated_assault_target_bonus: u32,
    defensive_screen_damage_taken_bonus: u32,
    flanking_support_bonus: u32,
    dispersed_formation_counter_bonus: u32,
    tactical_analysis_first_round_bonus: u32,
    balanced_engagement_base_score: u32,
    repetition_avoidance_penalty: u32,
    repetition_avoidance_threshold: u8,
}

/// Deterministic rule-based doctrine choice — a pure scoring function, no
/// randomness at all (unlike COMBAT-001-A's seed-derived `doctrine_pick`
/// placeholder it replaces). Ties are broken by `ALL_COMBAT_DOCTRINES`'s
/// fixed order (`BalancedEngagement` first), never by a seed, so the choice
/// is unambiguous and reproducible from `own`/`opponent`/`round` alone.
pub(crate) fn choose_ai_doctrine(
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
    rules: &CombatAiRules,
) -> CombatDoctrineId {
    let mut best = CombatDoctrineId::BalancedEngagement;
    let mut best_score = 0_u32;
    for &doctrine in &ALL_COMBAT_DOCTRINES {
        let score = score_for(doctrine, own, opponent, round, rules);
        if score > best_score {
            best_score = score;
            best = doctrine;
        }
    }
    best
}

fn score_for(
    doctrine: CombatDoctrineId,
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
    rules: &CombatAiRules,
) -> u32 {
    let mut score = match doctrine {
        CombatDoctrineId::BalancedEngagement => rules.balanced_engagement_base_score(),
        CombatDoctrineId::ConcentratedAssault => {
            if has_heavy_or_damaged_stack(opponent) {
                rules.concentrated_assault_target_bonus()
            } else {
                0
            }
        }
        CombatDoctrineId::DefensiveScreen => {
            if has_damaged_stack(own) {
                rules.defensive_screen_damage_taken_bonus()
            } else {
                0
            }
        }
        CombatDoctrineId::FlankingManeuver => {
            if has_support_stack(opponent) {
                rules.flanking_support_bonus()
            } else {
                0
            }
        }
        CombatDoctrineId::DispersedFormation => {
            if opponent.last_doctrine == Some(CombatDoctrineId::ConcentratedAssault) {
                rules.dispersed_formation_counter_bonus()
            } else {
                0
            }
        }
        // "utilise l'analyse tactique rarement" (doc §13) — only worth it
        // early, before enough rounds have passed to make the intel less
        // valuable relative to the damage it costs.
        CombatDoctrineId::TacticalAnalysis => {
            if round <= 1 {
                rules.tactical_analysis_first_round_bonus()
            } else {
                0
            }
        }
    };
    // Repetition avoidance: the AI "knows" it will trigger its own
    // repetition penalty (see `doctrine.rs`) past the configured threshold
    // and steers away from it, mirroring the player's own incentive.
    if own.last_doctrine == Some(doctrine)
        && own.consecutive_doctrine_uses >= rules.repetition_avoidance_threshold()
    {
        score = score.saturating_sub(rules.repetition_avoidance_penalty());
    }
    score
}

fn has_heavy_or_damaged_stack(side: &CombatSideState) -> bool {
    side.stacks.iter().any(|stack| {
        stack.surviving_quantity > 0
            && (stack.target_class == CombatTargetClass::Heavy
                || stack.current_hull < stack.maximum_hull)
    })
}

fn has_damaged_stack(side: &CombatSideState) -> bool {
    side.stacks
        .iter()
        .any(|stack| stack.surviving_quantity > 0 && stack.current_hull < stack.maximum_hull)
}

fn has_support_stack(side: &CombatSideState) -> bool {
    side.stacks.iter().any(|stack| {
        stack.surviving_quantity > 0 && stack.tactical_role == CombatTacticalRole::Support
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> CombatAiRulesConfig {
        CombatAiRulesConfig {
            version: 1,
            concentrated_assault_target_bonus: 30,
            defensive_screen_damage_taken_bonus: 25,
            flanking_support_bonus: 35,
            dispersed_formation_counter_bonus: 20,
            tactical_analysis_first_round_bonus: 40,
            balanced_engagement_base_score: 10,
            repetition_avoidance_penalty: 50,
            repetition_avoidance_threshold: 2,
        }
    }

    #[test]
    fn a_valid_config_loads_successfully() {
        assert!(CombatAiRules::from_config(valid_config()).is_ok());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        let mut config = valid_config();
        config.version = 2;
        assert_eq!(
            CombatAiRules::from_config(config).unwrap_err(),
            CombatRulesError::InvalidAiVersion(2)
        );
    }

    #[test]
    fn a_zero_repetition_threshold_is_rejected() {
        let mut config = valid_config();
        config.repetition_avoidance_threshold = 0;
        assert_eq!(
            CombatAiRules::from_config(config).unwrap_err(),
            CombatRulesError::InvalidAiScore
        );
    }

    use galactic_domain::{FactionId, Owner};

    use super::super::state::{CombatStackId, CombatStackState, CombatUnitRef};
    use crate::CraftableId;

    fn rules() -> CombatAiRules {
        CombatAiRules::from_config(valid_config()).unwrap()
    }

    fn stack(
        id: u32,
        target_class: CombatTargetClass,
        tactical_role: CombatTacticalRole,
        current_hull: u128,
        maximum_hull: u128,
    ) -> CombatStackState {
        CombatStackState {
            stack_id: CombatStackId(id),
            source: CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
            initial_quantity: 3,
            surviving_quantity: 3,
            current_hull,
            maximum_hull,
            offense: 50,
            defense: 40,
            durability: 60,
            target_class,
            bonuses: crate::CombatTargetBonuses::default(),
            tactical_role,
        }
    }

    fn side(stacks: Vec<CombatStackState>) -> CombatSideState {
        CombatSideState {
            owner: Owner::Faction(FactionId::new(0)),
            stacks,
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        }
    }

    fn healthy_medium_stack(id: u32) -> CombatStackState {
        stack(
            id,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
            1_000,
            1_000,
        )
    }

    #[test]
    fn balanced_engagement_is_the_fallback_with_no_signal_present() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![healthy_medium_stack(1)]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::BalancedEngagement
        );
    }

    #[test]
    fn concentrated_assault_is_chosen_against_a_heavy_opponent_stack() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Line,
            1_000,
            1_000,
        )]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::ConcentratedAssault
        );
    }

    #[test]
    fn concentrated_assault_is_chosen_against_a_damaged_opponent_stack() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
            400,
            1_000,
        )]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::ConcentratedAssault
        );
    }

    #[test]
    fn defensive_screen_is_chosen_when_own_stacks_are_damaged() {
        let own = side(vec![stack(
            0,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
            300,
            1_000,
        )]);
        let opponent = side(vec![healthy_medium_stack(1)]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::DefensiveScreen
        );
    }

    #[test]
    fn flanking_maneuver_is_chosen_against_an_opponent_support_stack() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Medium,
            CombatTacticalRole::Support,
            1_000,
            1_000,
        )]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::FlankingManeuver
        );
    }

    #[test]
    fn dispersed_formation_counters_an_opponent_that_just_used_concentrated_assault() {
        let own = side(vec![healthy_medium_stack(0)]);
        let mut opponent = side(vec![healthy_medium_stack(1)]);
        opponent.last_doctrine = Some(CombatDoctrineId::ConcentratedAssault);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::DispersedFormation
        );
    }

    #[test]
    fn tactical_analysis_is_only_favored_on_the_first_round() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![healthy_medium_stack(1)]);
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 1, &rules()),
            CombatDoctrineId::TacticalAnalysis
        );
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 4, &rules()),
            CombatDoctrineId::BalancedEngagement
        );
    }

    #[test]
    fn repeating_a_doctrine_past_the_threshold_is_penalized_away() {
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Line,
            1_000,
            1_000,
        )]);
        let mut own = side(vec![healthy_medium_stack(0)]);
        own.last_doctrine = Some(CombatDoctrineId::ConcentratedAssault);
        own.consecutive_doctrine_uses = 2;
        // Without repetition avoidance, `ConcentratedAssault` would win
        // against a heavy opponent stack — the penalty must push the choice
        // elsewhere once the threshold is reached.
        assert_ne!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::ConcentratedAssault
        );
    }

    #[test]
    fn below_the_repetition_threshold_the_doctrine_is_kept() {
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Line,
            1_000,
            1_000,
        )]);
        let mut own = side(vec![healthy_medium_stack(0)]);
        own.last_doctrine = Some(CombatDoctrineId::ConcentratedAssault);
        own.consecutive_doctrine_uses = 1;
        assert_eq!(
            choose_ai_doctrine(&own, &opponent, 3, &rules()),
            CombatDoctrineId::ConcentratedAssault
        );
    }

    #[test]
    fn the_choice_is_deterministic_across_repeated_calls() {
        let own = side(vec![healthy_medium_stack(0)]);
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Line,
            1_000,
            1_000,
        )]);
        let first = choose_ai_doctrine(&own, &opponent, 3, &rules());
        let second = choose_ai_doctrine(&own, &opponent, 3, &rules());
        assert_eq!(first, second);
    }
}

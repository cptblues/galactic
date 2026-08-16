// COMBAT-001-B/COMBAT-002: deterministic rule-based AI. The first pass used a
// single side-agnostic profile; COMBAT-002 keeps the same pure scoring shape
// but differentiates the default factions and feeds tactical plans into the
// existing round engine instead of adding a second resolver.

use galactic_domain::{FactionId, Owner};
use serde::Deserialize;

use super::CombatRulesError;
use super::CombatTargetClass;
use super::doctrine::{ALL_COMBAT_DOCTRINES, CombatDoctrineId};
use super::plan::{CombatGroupRole, CombatPlan, CombatTargetPriority};
use super::state::{CombatSideState, CombatTacticalRole, CombatUnitRef};

const CONSORTIUM_FACTION_ID: FactionId = FactionId::new(0);
const CONFINS_FACTION_ID: FactionId = FactionId::new(1);
const SYLVE_FACTION_ID: FactionId = FactionId::new(2);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatAiProfile {
    Consortium,
    Confins,
    Sylve,
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
    choose_ai_doctrine_for_profile(own, opponent, round, rules, profile_for_side(own))
}

pub(crate) fn choose_ai_plan(
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
    rules: &CombatAiRules,
) -> CombatPlan {
    let profile = profile_for_side(own);
    let doctrine = choose_ai_doctrine(own, opponent, round, rules);
    let mut plan = CombatPlan::default_for_side(own, doctrine);
    apply_profile_plan(&mut plan, profile, own, opponent, round);
    plan
}

pub(crate) fn choose_ai_doctrine_for_profile(
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
    rules: &CombatAiRules,
    profile: CombatAiProfile,
) -> CombatDoctrineId {
    let mut best = CombatDoctrineId::BalancedEngagement;
    let mut best_score = 0_u32;
    for &doctrine in &ALL_COMBAT_DOCTRINES {
        let score = score_for(doctrine, own, opponent, round, rules).saturating_add(
            profile_doctrine_bonus(profile, doctrine, own, opponent, round),
        );
        if score > best_score {
            best_score = score;
            best = doctrine;
        }
    }
    best
}

fn profile_for_side(side: &CombatSideState) -> CombatAiProfile {
    match side.owner {
        Owner::Faction(id) if id == CONSORTIUM_FACTION_ID => CombatAiProfile::Consortium,
        Owner::Faction(id) if id == CONFINS_FACTION_ID => CombatAiProfile::Confins,
        Owner::Faction(id) if id == SYLVE_FACTION_ID => CombatAiProfile::Sylve,
        _ => profile_from_force_keys(side).unwrap_or(CombatAiProfile::Confins),
    }
}

fn profile_from_force_keys(side: &CombatSideState) -> Option<CombatAiProfile> {
    side.stacks.iter().find_map(|stack| {
        let CombatUnitRef::PlanetaryForce(force_id) = stack.source else {
            return None;
        };
        let key = force_id.key();
        if key.starts_with("sylve_") {
            Some(CombatAiProfile::Sylve)
        } else if key.starts_with("consortium_") {
            Some(CombatAiProfile::Consortium)
        } else if key.starts_with("confins_") || key.starts_with("local_") {
            Some(CombatAiProfile::Confins)
        } else {
            None
        }
    })
}

fn profile_doctrine_bonus(
    profile: CombatAiProfile,
    doctrine: CombatDoctrineId,
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
) -> u32 {
    match profile {
        CombatAiProfile::Consortium => match doctrine {
            CombatDoctrineId::BalancedEngagement => 4,
            CombatDoctrineId::DefensiveScreen if has_damaged_stack(own) => 24,
            CombatDoctrineId::TacticalAnalysis if round <= 1 => 4,
            _ => 0,
        },
        CombatAiProfile::Confins => match doctrine {
            CombatDoctrineId::ConcentratedAssault if has_heavy_or_damaged_stack(opponent) => 22,
            CombatDoctrineId::DispersedFormation
                if opponent.last_doctrine == Some(CombatDoctrineId::ConcentratedAssault) =>
            {
                12
            }
            CombatDoctrineId::BalancedEngagement => 1,
            _ => 0,
        },
        CombatAiProfile::Sylve => match doctrine {
            CombatDoctrineId::FlankingManeuver => 28,
            CombatDoctrineId::ConcentratedAssault if has_heavy_or_damaged_stack(opponent) => 8,
            _ => 0,
        },
    }
}

fn apply_profile_plan(
    plan: &mut CombatPlan,
    profile: CombatAiProfile,
    own: &CombatSideState,
    opponent: &CombatSideState,
    round: u16,
) {
    match profile {
        CombatAiProfile::Consortium => apply_consortium_plan(plan, opponent, round),
        CombatAiProfile::Confins => apply_confins_plan(plan, opponent),
        CombatAiProfile::Sylve => apply_sylve_plan(plan, opponent),
    }
    debug_assert!(plan.validate_for_side(own).is_ok());
}

fn apply_consortium_plan(plan: &mut CombatPlan, opponent: &CombatSideState, round: u16) {
    let priority = if has_damaged_stack(opponent) {
        CombatTargetPriority::Damaged
    } else if has_heavy_stack(opponent) {
        CombatTargetPriority::Heavy
    } else {
        CombatTargetPriority::Medium
    };
    for group in &mut plan.groups {
        group.target_priority = if group.role == CombatGroupRole::Screen {
            CombatTargetPriority::Support
        } else {
            priority
        };
    }
    if round == 1
        && plan.groups.len() > 1
        && let Some(group) = plan.groups.last_mut()
    {
        group.role = CombatGroupRole::Reserve;
    }
}

fn apply_confins_plan(plan: &mut CombatPlan, opponent: &CombatSideState) {
    let priority = if has_damaged_stack(opponent) {
        CombatTargetPriority::Damaged
    } else if has_heavy_stack(opponent) {
        CombatTargetPriority::Heavy
    } else {
        CombatTargetPriority::Light
    };
    for group in &mut plan.groups {
        group.role = CombatGroupRole::Assault;
        group.target_priority = priority;
    }
}

fn apply_sylve_plan(plan: &mut CombatPlan, opponent: &CombatSideState) {
    let priority = if has_support_stack(opponent) {
        CombatTargetPriority::Support
    } else if has_damaged_stack(opponent) {
        CombatTargetPriority::Damaged
    } else {
        CombatTargetPriority::Light
    };
    for group in &mut plan.groups {
        if group.role == CombatGroupRole::Screen {
            group.role = CombatGroupRole::Bombardment;
        }
        group.target_priority = priority;
    }
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

fn has_heavy_stack(side: &CombatSideState) -> bool {
    side.stacks
        .iter()
        .any(|stack| stack.surviving_quantity > 0 && stack.target_class == CombatTargetClass::Heavy)
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

    fn side_for_owner(owner: FactionId, stacks: Vec<CombatStackState>) -> CombatSideState {
        CombatSideState {
            owner: Owner::Faction(owner),
            stacks,
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        }
    }

    fn side(stacks: Vec<CombatStackState>) -> CombatSideState {
        side_for_owner(FactionId::new(0), stacks)
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
    fn faction_profiles_choose_differentiated_doctrines_for_the_same_state() {
        let own_stack = stack(
            0,
            CombatTargetClass::Medium,
            CombatTacticalRole::Line,
            500,
            1_000,
        );
        let opponent = side(vec![stack(
            1,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Support,
            1_000,
            1_000,
        )]);

        assert_eq!(
            choose_ai_doctrine(
                &side_for_owner(FactionId::new(0), vec![own_stack.clone()]),
                &opponent,
                3,
                &rules(),
            ),
            CombatDoctrineId::DefensiveScreen
        );
        assert_eq!(
            choose_ai_doctrine(
                &side_for_owner(FactionId::new(1), vec![own_stack.clone()]),
                &opponent,
                3,
                &rules(),
            ),
            CombatDoctrineId::ConcentratedAssault
        );
        assert_eq!(
            choose_ai_doctrine(
                &side_for_owner(FactionId::new(2), vec![own_stack]),
                &opponent,
                3,
                &rules(),
            ),
            CombatDoctrineId::FlankingManeuver
        );
    }

    #[test]
    fn faction_profiles_emit_distinct_tactical_plans() {
        let opponent = side(vec![stack(
            9,
            CombatTargetClass::Heavy,
            CombatTacticalRole::Support,
            1_000,
            1_000,
        )]);
        let stacks = vec![
            healthy_medium_stack(0),
            stack(
                1,
                CombatTargetClass::Medium,
                CombatTacticalRole::Support,
                1_000,
                1_000,
            ),
        ];

        let consortium = choose_ai_plan(
            &side_for_owner(FactionId::new(0), stacks.clone()),
            &opponent,
            1,
            &rules(),
        );
        let confins = choose_ai_plan(
            &side_for_owner(FactionId::new(1), stacks.clone()),
            &opponent,
            1,
            &rules(),
        );
        let sylve = choose_ai_plan(
            &side_for_owner(FactionId::new(2), stacks),
            &opponent,
            1,
            &rules(),
        );

        assert_eq!(
            consortium.groups.last().map(|group| group.role),
            Some(CombatGroupRole::Reserve)
        );
        assert!(
            confins
                .groups
                .iter()
                .all(|group| group.role == CombatGroupRole::Assault)
        );
        assert!(
            sylve
                .groups
                .iter()
                .any(|group| group.role == CombatGroupRole::Bombardment)
        );
        assert!(
            sylve
                .groups
                .iter()
                .all(|group| group.target_priority == CombatTargetPriority::Support)
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

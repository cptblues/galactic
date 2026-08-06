// COMBAT-001-A: doctrine data model. The six doctrines are fixed game design
// (a closed Rust enum, not a ruleset-defined dynamic id like `CraftableId`) —
// only their numeric magnitudes are ruleset-configurable. Folded into
// `combat.ron` (bumped to version 3) rather than a new `combat_tactics.ron`:
// `CombatRules::from_config`'s version guard is entirely local to `combat.rs`,
// so extending it in place costs no `ruleset.rs` ceremony (no new `Ruleset`
// field, no `RulesetLoadError` variant, no `RULESET_SCHEMA_VERSION` bump).
// Revisit the file split if a later sub-ticket (B/C) makes this section grow
// significantly (intel thresholds, AI profile weights).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::CombatRulesError;

/// Bound for multipliers that can amplify an effect above the neutral 1000
/// (e.g. `ConcentratedAssault`'s offense bonus) as well as reduce it.
const MAX_EFFECT_MULTIPLIER_PER_MILLE: u32 = 2_000;
/// Bound for values that only ever express a fraction of an effect (0..=1000),
/// e.g. how much weight a protection/bonus/cap redirects.
const MAX_FRACTION_PER_MILLE: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CombatDoctrineId {
    /// Neutral fallback; exempt from the repetition penalty.
    BalancedEngagement,
    /// Targets `Heavy`-class and already-damaged stacks.
    ConcentratedAssault,
    /// Reduces damage taken/dealt; redirects incoming weight away from `Support` stacks.
    DefensiveScreen,
    /// Bonus weight against `Support`-role stacks; own defense malus.
    FlankingManeuver,
    /// Caps the weight share any single opposing stack may receive.
    DispersedFormation,
    /// Offense malus this round; boosts attacker intel gain this round (see
    /// `combat/intel.rs`'s `apply_round_intel_gain`, wired in COMBAT-001-B).
    TacticalAnalysis,
}

pub(crate) const ALL_COMBAT_DOCTRINES: [CombatDoctrineId; 6] = [
    CombatDoctrineId::BalancedEngagement,
    CombatDoctrineId::ConcentratedAssault,
    CombatDoctrineId::DefensiveScreen,
    CombatDoctrineId::FlankingManeuver,
    CombatDoctrineId::DispersedFormation,
    CombatDoctrineId::TacticalAnalysis,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CombatDoctrineRule {
    pub(crate) offense_multiplier_per_mille: u32,
    pub(crate) damage_taken_multiplier_per_mille: u32,
    pub(crate) repetition_exempt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CombatCounterRule {
    pub(crate) doctrine: CombatDoctrineId,
    pub(crate) countered_by: CombatDoctrineId,
    pub(crate) damage_dealt_multiplier_per_mille: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatTacticsRules {
    repetition_penalty_per_mille: u32,
    repetition_penalty_maximum_stacks: u8,
    support_protection_per_mille: u32,
    flanking_support_bonus_per_mille: u32,
    dispersion_concentration_cap_per_mille: u32,
    concentration_bonus_per_mille: u32,
    doctrines: BTreeMap<CombatDoctrineId, CombatDoctrineRule>,
    counters: Vec<CombatCounterRule>,
}

impl CombatTacticsRules {
    pub(crate) fn from_config(config: CombatTacticsRulesConfig) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
            return Err(CombatRulesError::InvalidTacticsVersion(config.version));
        }
        if config.repetition_penalty_per_mille > MAX_FRACTION_PER_MILLE {
            return Err(CombatRulesError::InvalidRepetitionPenalty);
        }
        if config.repetition_penalty_maximum_stacks == 0 {
            return Err(CombatRulesError::InvalidRepetitionPenalty);
        }
        if config.support_protection_per_mille > MAX_FRACTION_PER_MILLE {
            return Err(CombatRulesError::InvalidSupportProtection);
        }
        if config.flanking_support_bonus_per_mille > MAX_EFFECT_MULTIPLIER_PER_MILLE {
            return Err(CombatRulesError::InvalidFlankingBonus);
        }
        if config.dispersion_concentration_cap_per_mille == 0
            || config.dispersion_concentration_cap_per_mille > MAX_FRACTION_PER_MILLE
        {
            return Err(CombatRulesError::InvalidDispersionCap);
        }
        if config.concentration_bonus_per_mille > MAX_EFFECT_MULTIPLIER_PER_MILLE {
            return Err(CombatRulesError::InvalidConcentrationBonus);
        }

        if config.doctrines.len() != ALL_COMBAT_DOCTRINES.len() {
            return Err(CombatRulesError::InvalidDoctrineCount {
                found: config.doctrines.len(),
                expected: ALL_COMBAT_DOCTRINES.len(),
            });
        }
        let mut doctrines = BTreeMap::new();
        for entry in config.doctrines {
            if entry.offense_multiplier_per_mille > MAX_EFFECT_MULTIPLIER_PER_MILLE
                || entry.damage_taken_multiplier_per_mille > MAX_EFFECT_MULTIPLIER_PER_MILLE
            {
                return Err(CombatRulesError::InvalidDoctrineMultiplier { doctrine: entry.id });
            }
            if entry.id == CombatDoctrineId::BalancedEngagement && !entry.repetition_exempt {
                return Err(CombatRulesError::BalancedEngagementMustBeRepetitionExempt);
            }
            if doctrines
                .insert(
                    entry.id,
                    CombatDoctrineRule {
                        offense_multiplier_per_mille: entry.offense_multiplier_per_mille,
                        damage_taken_multiplier_per_mille: entry.damage_taken_multiplier_per_mille,
                        repetition_exempt: entry.repetition_exempt,
                    },
                )
                .is_some()
            {
                return Err(CombatRulesError::DuplicateDoctrine(entry.id));
            }
        }
        for doctrine in ALL_COMBAT_DOCTRINES {
            if !doctrines.contains_key(&doctrine) {
                return Err(CombatRulesError::MissingDoctrine(doctrine));
            }
        }

        const EXPECTED_COUNTERS: usize = 4;
        if config.counters.len() != EXPECTED_COUNTERS {
            return Err(CombatRulesError::InvalidCounterCount {
                found: config.counters.len(),
                expected: EXPECTED_COUNTERS,
            });
        }
        let mut counters = Vec::with_capacity(config.counters.len());
        let mut countered_doctrines = BTreeMap::new();
        for entry in config.counters {
            if entry.damage_dealt_multiplier_per_mille == 0
                || entry.damage_dealt_multiplier_per_mille > MAX_EFFECT_MULTIPLIER_PER_MILLE
            {
                return Err(CombatRulesError::InvalidCounterMultiplier {
                    doctrine: entry.doctrine,
                });
            }
            if countered_doctrines.insert(entry.doctrine, ()).is_some() {
                return Err(CombatRulesError::DuplicateCounter(entry.doctrine));
            }
            counters.push(CombatCounterRule {
                doctrine: entry.doctrine,
                countered_by: entry.countered_by,
                damage_dealt_multiplier_per_mille: entry.damage_dealt_multiplier_per_mille,
            });
        }

        Ok(Self {
            repetition_penalty_per_mille: config.repetition_penalty_per_mille,
            repetition_penalty_maximum_stacks: config.repetition_penalty_maximum_stacks,
            support_protection_per_mille: config.support_protection_per_mille,
            flanking_support_bonus_per_mille: config.flanking_support_bonus_per_mille,
            dispersion_concentration_cap_per_mille: config.dispersion_concentration_cap_per_mille,
            concentration_bonus_per_mille: config.concentration_bonus_per_mille,
            doctrines,
            counters,
        })
    }

    // The following accessors are consumed starting COMBAT-001-A step 4
    // (resolve_combat_round's targeting/damage pipeline) — unused for now
    // while that step lands.
    pub(crate) fn doctrine(&self, id: CombatDoctrineId) -> CombatDoctrineRule {
        *self
            .doctrines
            .get(&id)
            .expect("all six doctrines are validated present at load time")
    }

    /// Multiplier applied to `doctrine`'s outgoing damage when the opponent
    /// plays `opponent`, if `opponent` counters `doctrine`. `None` means no
    /// counter relationship applies this round.
    pub(crate) fn counter_multiplier(
        &self,
        doctrine: CombatDoctrineId,
        opponent: CombatDoctrineId,
    ) -> Option<u32> {
        self.counters
            .iter()
            .find(|rule| rule.doctrine == doctrine && rule.countered_by == opponent)
            .map(|rule| rule.damage_dealt_multiplier_per_mille)
    }

    pub(crate) const fn repetition_penalty_per_mille(&self) -> u32 {
        self.repetition_penalty_per_mille
    }

    pub(crate) const fn repetition_penalty_maximum_stacks(&self) -> u8 {
        self.repetition_penalty_maximum_stacks
    }

    pub(crate) const fn support_protection_per_mille(&self) -> u32 {
        self.support_protection_per_mille
    }

    pub(crate) const fn flanking_support_bonus_per_mille(&self) -> u32 {
        self.flanking_support_bonus_per_mille
    }

    pub(crate) const fn dispersion_concentration_cap_per_mille(&self) -> u32 {
        self.dispersion_concentration_cap_per_mille
    }

    /// Weight multiplier `ConcentratedAssault` applies when targeting
    /// `Heavy`-class or already-damaged opposing stacks.
    pub(crate) const fn concentration_bonus_per_mille(&self) -> u32 {
        self.concentration_bonus_per_mille
    }

    pub(crate) fn counters_len(&self) -> usize {
        self.counters.len()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CombatTacticsRulesConfig {
    version: u32,
    repetition_penalty_per_mille: u32,
    repetition_penalty_maximum_stacks: u8,
    support_protection_per_mille: u32,
    flanking_support_bonus_per_mille: u32,
    dispersion_concentration_cap_per_mille: u32,
    concentration_bonus_per_mille: u32,
    doctrines: Vec<CombatDoctrineRuleConfig>,
    counters: Vec<CombatCounterRuleConfig>,
}

#[derive(Debug, Deserialize)]
struct CombatDoctrineRuleConfig {
    id: CombatDoctrineId,
    offense_multiplier_per_mille: u32,
    damage_taken_multiplier_per_mille: u32,
    repetition_exempt: bool,
}

#[derive(Debug, Deserialize)]
struct CombatCounterRuleConfig {
    doctrine: CombatDoctrineId,
    countered_by: CombatDoctrineId,
    damage_dealt_multiplier_per_mille: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_doctrines() -> Vec<CombatDoctrineRuleConfig> {
        ALL_COMBAT_DOCTRINES
            .iter()
            .map(|&id| CombatDoctrineRuleConfig {
                id,
                offense_multiplier_per_mille: 1_000,
                damage_taken_multiplier_per_mille: 1_000,
                repetition_exempt: id == CombatDoctrineId::BalancedEngagement,
            })
            .collect()
    }

    fn valid_counters() -> Vec<CombatCounterRuleConfig> {
        vec![
            CombatCounterRuleConfig {
                doctrine: CombatDoctrineId::ConcentratedAssault,
                countered_by: CombatDoctrineId::DefensiveScreen,
                damage_dealt_multiplier_per_mille: 800,
            },
            CombatCounterRuleConfig {
                doctrine: CombatDoctrineId::DefensiveScreen,
                countered_by: CombatDoctrineId::FlankingManeuver,
                damage_dealt_multiplier_per_mille: 800,
            },
            CombatCounterRuleConfig {
                doctrine: CombatDoctrineId::FlankingManeuver,
                countered_by: CombatDoctrineId::DispersedFormation,
                damage_dealt_multiplier_per_mille: 800,
            },
            CombatCounterRuleConfig {
                doctrine: CombatDoctrineId::DispersedFormation,
                countered_by: CombatDoctrineId::ConcentratedAssault,
                damage_dealt_multiplier_per_mille: 800,
            },
        ]
    }

    fn valid_config() -> CombatTacticsRulesConfig {
        CombatTacticsRulesConfig {
            version: 1,
            repetition_penalty_per_mille: 150,
            repetition_penalty_maximum_stacks: 3,
            support_protection_per_mille: 250,
            flanking_support_bonus_per_mille: 350,
            dispersion_concentration_cap_per_mille: 400,
            concentration_bonus_per_mille: 500,
            doctrines: valid_doctrines(),
            counters: valid_counters(),
        }
    }

    #[test]
    fn a_valid_config_loads_successfully() {
        assert!(CombatTacticsRules::from_config(valid_config()).is_ok());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        let mut config = valid_config();
        config.version = 2;
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidTacticsVersion(2))
        );
    }

    #[test]
    fn a_missing_doctrine_is_rejected() {
        let mut config = valid_config();
        config.doctrines.pop();
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidDoctrineCount {
                found: 5,
                expected: 6
            })
        );
    }

    #[test]
    fn a_duplicate_doctrine_is_rejected() {
        let mut config = valid_config();
        let extra = CombatDoctrineRuleConfig {
            id: CombatDoctrineId::BalancedEngagement,
            offense_multiplier_per_mille: 1_000,
            damage_taken_multiplier_per_mille: 1_000,
            repetition_exempt: true,
        };
        config.doctrines.push(extra);
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidDoctrineCount {
                found: 7,
                expected: 6
            })
        );
    }

    #[test]
    fn balanced_engagement_must_be_repetition_exempt() {
        let mut config = valid_config();
        for doctrine in &mut config.doctrines {
            if doctrine.id == CombatDoctrineId::BalancedEngagement {
                doctrine.repetition_exempt = false;
            }
        }
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::BalancedEngagementMustBeRepetitionExempt)
        );
    }

    #[test]
    fn an_out_of_range_doctrine_multiplier_is_rejected() {
        let mut config = valid_config();
        config.doctrines[1].offense_multiplier_per_mille = MAX_EFFECT_MULTIPLIER_PER_MILLE + 1;
        let doctrine = config.doctrines[1].id;
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidDoctrineMultiplier { doctrine })
        );
    }

    #[test]
    fn a_wrong_number_of_counters_is_rejected() {
        let mut config = valid_config();
        config.counters.pop();
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidCounterCount {
                found: 3,
                expected: 4
            })
        );
    }

    #[test]
    fn a_zero_counter_multiplier_is_rejected_never_fully_negating_a_doctrine() {
        let mut config = valid_config();
        config.counters[0].damage_dealt_multiplier_per_mille = 0;
        let doctrine = config.counters[0].doctrine;
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::InvalidCounterMultiplier { doctrine })
        );
    }

    #[test]
    fn a_duplicated_countered_doctrine_is_rejected() {
        let mut config = valid_config();
        config.counters[1].doctrine = config.counters[0].doctrine;
        let doctrine = config.counters[0].doctrine;
        assert_eq!(
            CombatTacticsRules::from_config(config),
            Err(CombatRulesError::DuplicateCounter(doctrine))
        );
    }

    #[test]
    fn doctrine_and_counter_lookups_return_the_configured_values() {
        let rules = CombatTacticsRules::from_config(valid_config()).expect("valid config");

        assert!(
            rules
                .doctrine(CombatDoctrineId::BalancedEngagement)
                .repetition_exempt
        );
        assert_eq!(
            rules.counter_multiplier(
                CombatDoctrineId::ConcentratedAssault,
                CombatDoctrineId::DefensiveScreen
            ),
            Some(800)
        );
        assert_eq!(
            rules.counter_multiplier(
                CombatDoctrineId::ConcentratedAssault,
                CombatDoctrineId::FlankingManeuver
            ),
            None
        );
    }
}

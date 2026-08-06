// COMBAT-001-C: retreat penalty — the deterministic, one-time damage
// instance a side takes when disengaging via `GameCommand::RetreatFromCombat`
// (doc §16: "peut subir une pénalité configurable... face à... certaines
// doctrines" — simplified for the MVP to a flat magnitude, not
// doctrine-specific; extending `CombatRetreatRules` for per-doctrine
// variation later is additive, not a refactor, if ever wanted). The actual
// damage computation lives in `combat/rounds.rs::apply_retreat_penalty`,
// next to the round-damage primitives it reuses — this module only owns the
// ruleset-configured magnitude.

use serde::Deserialize;

use super::CombatRulesError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CombatRetreatRules {
    penalty_per_mille: u32,
}

impl CombatRetreatRules {
    pub(crate) fn from_config(config: CombatRetreatRulesConfig) -> Result<Self, CombatRulesError> {
        if config.version != 1 {
            return Err(CombatRulesError::InvalidRetreatVersion(config.version));
        }
        if config.penalty_per_mille > 1_000 {
            return Err(CombatRulesError::InvalidRetreatPenalty);
        }
        Ok(Self {
            penalty_per_mille: config.penalty_per_mille,
        })
    }

    // Consumed by `rounds::apply_retreat_penalty`, wired in starting
    // COMBAT-001-C's session-orchestration step.
    #[allow(dead_code)]
    pub(crate) const fn penalty_per_mille(&self) -> u32 {
        self.penalty_per_mille
    }

    /// Test-only constructor letting sibling modules (e.g. `rounds.rs`'s own
    /// test suite) build a `CombatRetreatRules` with a specific magnitude
    /// without needing `CombatRetreatRulesConfig`'s fields to be more than
    /// module-private.
    #[cfg(test)]
    pub(crate) const fn for_tests(penalty_per_mille: u32) -> Self {
        Self { penalty_per_mille }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CombatRetreatRulesConfig {
    version: u32,
    penalty_per_mille: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> CombatRetreatRulesConfig {
        CombatRetreatRulesConfig {
            version: 1,
            penalty_per_mille: 300,
        }
    }

    #[test]
    fn a_valid_config_loads_successfully() {
        assert!(CombatRetreatRules::from_config(valid_config()).is_ok());
    }

    #[test]
    fn an_unsupported_version_is_rejected() {
        let mut config = valid_config();
        config.version = 2;
        assert_eq!(
            CombatRetreatRules::from_config(config).unwrap_err(),
            CombatRulesError::InvalidRetreatVersion(2)
        );
    }

    #[test]
    fn a_penalty_above_one_thousand_per_mille_is_rejected() {
        let mut config = valid_config();
        config.penalty_per_mille = 1_001;
        assert_eq!(
            CombatRetreatRules::from_config(config).unwrap_err(),
            CombatRulesError::InvalidRetreatPenalty
        );
    }

    #[test]
    fn a_penalty_of_exactly_one_thousand_per_mille_is_accepted() {
        let mut config = valid_config();
        config.penalty_per_mille = 1_000;
        assert!(CombatRetreatRules::from_config(config).is_ok());
    }
}

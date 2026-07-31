// MVP-029-B: deterministic, ruleset-driven remote extraction sites.
use std::collections::BTreeSet;

use galactic_domain::{
    ExtractionSiteId, MissionId, PlanetId, PlanetKind, ResourceKind, ResourceStock, SystemId,
};
use serde::Deserialize;

use crate::{GameState, UniverseRepository, default_ruleset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionSiteRule {
    pub kind: PlanetKind,
    pub resource: ResourceKind,
    pub initial_reserve: u64,
    pub yield_per_tick: u64,
    pub harvest_ticks: u64,
}

impl ExtractionSiteRule {
    pub fn maximum_harvest(self) -> u64 {
        self.yield_per_tick
            .checked_mul(self.harvest_ticks)
            .expect("validated extraction yields fit in u64")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionRules {
    version: u32,
    reserve_site_during_mission: bool,
    kinds: Vec<ExtractionSiteRule>,
}

impl ExtractionRules {
    pub(crate) fn from_config(config: ExtractionRulesConfig) -> Result<Self, ExtractionRulesError> {
        if config.version != 1 {
            return Err(ExtractionRulesError::UnsupportedVersion(config.version));
        }
        let mut configured_kinds = BTreeSet::new();
        let mut kinds = Vec::with_capacity(config.kinds.len());
        for configured in config.kinds {
            let kind = configured.kind.into();
            if !configured_kinds.insert(kind_fingerprint_tag(kind)) {
                return Err(ExtractionRulesError::DuplicatePlanetKind(kind));
            }
            let resource: ResourceKind = configured.resource.into();
            if !resource.is_stored() {
                return Err(ExtractionRulesError::InvalidResource(resource));
            }
            if configured.initial_reserve == 0 {
                return Err(ExtractionRulesError::EmptyReserve(kind));
            }
            if configured.yield_per_tick == 0 {
                return Err(ExtractionRulesError::EmptyYield(kind));
            }
            if configured.harvest_ticks == 0 {
                return Err(ExtractionRulesError::EmptyDuration(kind));
            }
            configured
                .yield_per_tick
                .checked_mul(configured.harvest_ticks)
                .ok_or(ExtractionRulesError::YieldOverflow(kind))?;
            kinds.push(ExtractionSiteRule {
                kind,
                resource,
                initial_reserve: configured.initial_reserve,
                yield_per_tick: configured.yield_per_tick,
                harvest_ticks: configured.harvest_ticks,
            });
        }
        if configured_kinds.len() != PlanetKind::ALL.len() {
            return Err(ExtractionRulesError::MissingPlanetKind);
        }
        kinds.sort_by_key(|rule| kind_fingerprint_tag(rule.kind));
        Ok(Self {
            version: config.version,
            reserve_site_during_mission: config.reserve_site_during_mission,
            kinds,
        })
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn reserves_sites(&self) -> bool {
        self.reserve_site_during_mission
    }

    pub fn rule_for(&self, kind: PlanetKind) -> ExtractionSiteRule {
        *self
            .kinds
            .iter()
            .find(|rule| rule.kind == kind)
            .expect("validated extraction rules cover every planet kind")
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("extraction:");
        output.push_str(&self.version.to_string());
        output.push(':');
        output.push_str(if self.reserve_site_during_mission {
            "reserved;"
        } else {
            "shared;"
        });
        for rule in &self.kinds {
            output.push_str("site:");
            output.push_str(&kind_fingerprint_tag(rule.kind).to_string());
            output.push(':');
            output.push_str(resource_key(rule.resource));
            output.push(':');
            output.push_str(&rule.initial_reserve.to_string());
            output.push(':');
            output.push_str(&rule.yield_per_tick.to_string());
            output.push(':');
            output.push_str(&rule.harvest_ticks.to_string());
            output.push(';');
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionSiteState {
    pub id: ExtractionSiteId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub resource: ResourceKind,
    pub remaining: u64,
    pub reserved_by: Option<MissionId>,
}

impl ExtractionSiteState {
    pub const fn is_depleted(self) -> bool {
        self.remaining == 0
    }

    pub const fn stock(self) -> ResourceStock {
        stock_for(self.resource, self.remaining)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionRulesError {
    UnsupportedVersion(u32),
    DuplicatePlanetKind(PlanetKind),
    MissingPlanetKind,
    InvalidResource(ResourceKind),
    EmptyReserve(PlanetKind),
    EmptyYield(PlanetKind),
    EmptyDuration(PlanetKind),
    YieldOverflow(PlanetKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionSiteStateError {
    DuplicateSite(ExtractionSiteId),
    DuplicatePlanet(PlanetId),
    MissingSite(PlanetId),
    UnknownSystem(SystemId),
    UnknownPlanet(PlanetId),
    PlanetSystemMismatch {
        planet_id: PlanetId,
        expected: SystemId,
        found: SystemId,
    },
    SiteIdMismatch {
        expected: ExtractionSiteId,
        found: ExtractionSiteId,
    },
    ResourceMismatch {
        site_id: ExtractionSiteId,
        expected: ResourceKind,
        found: ResourceKind,
    },
    ReserveExceedsConfigured {
        site_id: ExtractionSiteId,
        remaining: u64,
        maximum: u64,
    },
}

pub fn extraction_rules() -> &'static ExtractionRules {
    default_ruleset().extraction()
}

pub fn generate_extraction_sites(universe: &UniverseRepository) -> Vec<ExtractionSiteState> {
    let rules = extraction_rules();
    let mut sites = universe
        .definition()
        .systems
        .iter()
        .flat_map(|system| {
            system.planets.iter().map(move |planet| {
                let rule = rules.rule_for(planet.kind);
                ExtractionSiteState {
                    id: ExtractionSiteId::for_planet(planet.id),
                    system_id: system.id,
                    planet_id: planet.id,
                    resource: rule.resource,
                    remaining: rule.initial_reserve,
                    reserved_by: None,
                }
            })
        })
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| site.id);
    sites
}

pub fn validate_extraction_sites(
    state: &GameState,
    universe: &UniverseRepository,
) -> Result<(), ExtractionSiteStateError> {
    let rules = extraction_rules();
    let mut ids = BTreeSet::new();
    let mut planets = BTreeSet::new();
    for site in &state.extraction_sites {
        if !ids.insert(site.id) {
            return Err(ExtractionSiteStateError::DuplicateSite(site.id));
        }
        if !planets.insert(site.planet_id) {
            return Err(ExtractionSiteStateError::DuplicatePlanet(site.planet_id));
        }
        if universe.system(site.system_id).is_none() {
            return Err(ExtractionSiteStateError::UnknownSystem(site.system_id));
        }
        let Some((expected_system, planet)) = universe.planet_location(site.planet_id) else {
            return Err(ExtractionSiteStateError::UnknownPlanet(site.planet_id));
        };
        if expected_system != site.system_id {
            return Err(ExtractionSiteStateError::PlanetSystemMismatch {
                planet_id: site.planet_id,
                expected: expected_system,
                found: site.system_id,
            });
        }
        let expected_id = ExtractionSiteId::for_planet(site.planet_id);
        if site.id != expected_id {
            return Err(ExtractionSiteStateError::SiteIdMismatch {
                expected: expected_id,
                found: site.id,
            });
        }
        let rule = rules.rule_for(planet.kind);
        if site.resource != rule.resource {
            return Err(ExtractionSiteStateError::ResourceMismatch {
                site_id: site.id,
                expected: rule.resource,
                found: site.resource,
            });
        }
        if site.remaining > rule.initial_reserve {
            return Err(ExtractionSiteStateError::ReserveExceedsConfigured {
                site_id: site.id,
                remaining: site.remaining,
                maximum: rule.initial_reserve,
            });
        }
    }
    for system in &universe.definition().systems {
        for planet in &system.planets {
            if !planets.contains(&planet.id) {
                return Err(ExtractionSiteStateError::MissingSite(planet.id));
            }
        }
    }
    Ok(())
}

pub const fn stock_for(resource: ResourceKind, amount: u64) -> ResourceStock {
    match resource {
        ResourceKind::Metal => ResourceStock::new(amount, 0, 0),
        ResourceKind::Crystal => ResourceStock::new(0, amount, 0),
        ResourceKind::Fuel => ResourceStock::new(0, 0, amount),
        ResourceKind::Energy => ResourceStock::ZERO,
    }
}

const fn kind_fingerprint_tag(kind: PlanetKind) -> u8 {
    match kind {
        PlanetKind::Rocky => 1,
        PlanetKind::Ocean => 2,
        PlanetKind::Desert => 3,
        PlanetKind::Ice => 4,
        PlanetKind::GasGiant => 5,
        PlanetKind::Volcanic => 6,
    }
}

const fn resource_key(resource: ResourceKind) -> &'static str {
    match resource {
        ResourceKind::Metal => "metal",
        ResourceKind::Crystal => "crystal",
        ResourceKind::Fuel => "fuel",
        ResourceKind::Energy => "energy",
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExtractionRulesConfig {
    version: u32,
    reserve_site_during_mission: bool,
    kinds: Vec<ExtractionSiteRuleConfig>,
}

#[derive(Debug, Deserialize)]
struct ExtractionSiteRuleConfig {
    kind: PlanetKindConfig,
    resource: ResourceKindConfig,
    initial_reserve: u64,
    yield_per_tick: u64,
    harvest_ticks: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum PlanetKindConfig {
    Rocky,
    Ocean,
    Desert,
    Ice,
    GasGiant,
    Volcanic,
}

impl From<PlanetKindConfig> for PlanetKind {
    fn from(kind: PlanetKindConfig) -> Self {
        match kind {
            PlanetKindConfig::Rocky => Self::Rocky,
            PlanetKindConfig::Ocean => Self::Ocean,
            PlanetKindConfig::Desert => Self::Desert,
            PlanetKindConfig::Ice => Self::Ice,
            PlanetKindConfig::GasGiant => Self::GasGiant,
            PlanetKindConfig::Volcanic => Self::Volcanic,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum ResourceKindConfig {
    Metal,
    Crystal,
    Fuel,
}

impl From<ResourceKindConfig> for ResourceKind {
    fn from(resource: ResourceKindConfig) -> Self {
        match resource {
            ResourceKindConfig::Metal => Self::Metal,
            ResourceKindConfig::Crystal => Self::Crystal,
            ResourceKindConfig::Fuel => Self::Fuel,
        }
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;

    use crate::Simulation;

    use super::*;

    #[test]
    fn default_rules_cover_every_planet_kind() {
        let rules = extraction_rules();

        assert_eq!(rules.version(), 1);
        assert!(rules.reserves_sites());
        for kind in PlanetKind::ALL {
            let rule = rules.rule_for(kind);
            assert_eq!(rule.kind, kind);
            assert!(rule.resource.is_stored());
            assert!(rule.initial_reserve >= rule.maximum_harvest());
        }
    }

    #[test]
    fn every_generated_planet_has_one_stable_site() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let planet_count = simulation
            .universe()
            .systems
            .iter()
            .map(|system| system.planets.len())
            .sum::<usize>();

        assert_eq!(simulation.state().extraction_sites.len(), planet_count);
        for site in &simulation.state().extraction_sites {
            assert_eq!(site.id, ExtractionSiteId::for_planet(site.planet_id));
            assert!(site.remaining > 0);
            assert_eq!(site.reserved_by, None);
        }
    }

    #[test]
    fn validation_rejects_a_missing_site() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let removed = simulation
            .state_mut()
            .extraction_sites
            .pop()
            .expect("the reference universe has extraction sites");

        assert_eq!(
            validate_extraction_sites(simulation.state(), simulation.universe_repository()),
            Err(ExtractionSiteStateError::MissingSite(removed.planet_id)),
        );
    }
}

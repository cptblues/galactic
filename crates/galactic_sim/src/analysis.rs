// MVP-024: deterministic planetary analysis and explicit colonizability rules.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use galactic_domain::{FactionId, Owner, Planet, PlanetId, PlanetKind, ResourceCost, SystemId};
use serde::{Deserialize, Serialize};

use crate::{
    AuthorizationError, BuildingCatalog, BuildingCatalogError, BuildingLevels, CraftableCatalog,
    CraftableId, DiplomaticRelation, GameState, KnowledgeChange, KnowledgeLevel,
    PlanetResourceProfile, PlanetaryIntelPrecision, STRATEGIC_TICKS_PER_SECOND, ShipClass,
    StrategicDuration, StrategicTick, TechnologyUnlock, UniverseRepository, default_ruleset,
    refresh_planetary_intelligence,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanetEnvironment {
    Temperate,
    Oceanic,
    Arid,
    Frozen,
    Volcanic,
    Gaseous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum InstallationConstraint {
    ThinAtmosphere,
    GlobalOcean,
    AridClimate,
    CryogenicClimate,
    ExtremeVolcanism,
    NoSolidSurface,
}

impl InstallationConstraint {
    const fn bit(self) -> u16 {
        match self {
            Self::ThinAtmosphere => 1 << 0,
            Self::GlobalOcean => 1 << 1,
            Self::AridClimate => 1 << 2,
            Self::CryogenicClimate => 1 << 3,
            Self::ExtremeVolcanism => 1 << 4,
            Self::NoSolidSurface => 1 << 5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstallationConstraints(u16);

impl InstallationConstraints {
    pub const NONE: Self = Self(0);
    pub const ALL: [InstallationConstraint; 6] = [
        InstallationConstraint::ThinAtmosphere,
        InstallationConstraint::GlobalOcean,
        InstallationConstraint::AridClimate,
        InstallationConstraint::CryogenicClimate,
        InstallationConstraint::ExtremeVolcanism,
        InstallationConstraint::NoSolidSurface,
    ];

    pub fn from_constraints(constraints: impl IntoIterator<Item = InstallationConstraint>) -> Self {
        let mut value = Self::NONE;
        for constraint in constraints {
            value.0 |= constraint.bit();
        }
        value
    }

    pub const fn contains(self, constraint: InstallationConstraint) -> bool {
        self.0 & constraint.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = InstallationConstraint> {
        Self::ALL
            .into_iter()
            .filter(move |constraint| self.contains(*constraint))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetTypeAnalysisRule {
    pub kind: PlanetKind,
    pub environment: PlanetEnvironment,
    pub base_resource_profile: PlanetResourceProfile,
    pub colonizable: bool,
    pub constraints: InstallationConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColonyInitializationRules {
    pub population: u64,
    pub buildings: BuildingLevels,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryAnalysisRules {
    version: u32,
    minimum_habitability: u8,
    maximum_colonies: usize,
    analysis_duration: StrategicDuration,
    foundation_cost: ResourceCost,
    colony_ship: CraftableId,
    colony_initialization: ColonyInitializationRules,
    kinds: Vec<PlanetTypeAnalysisRule>,
}

impl PlanetaryAnalysisRules {
    pub(crate) fn from_config(
        config: PlanetaryAnalysisRulesConfig,
        craftables: &CraftableCatalog,
        buildings: &BuildingCatalog,
    ) -> Result<Self, PlanetaryAnalysisRulesError> {
        if config.version != 4 {
            return Err(PlanetaryAnalysisRulesError::UnsupportedVersion(
                config.version,
            ));
        }
        let analysis_duration_ticks = config
            .analysis_duration_seconds
            .checked_mul(u64::from(STRATEGIC_TICKS_PER_SECOND))
            .ok_or(PlanetaryAnalysisRulesError::InvalidAnalysisDuration)?;
        if analysis_duration_ticks == 0 {
            return Err(PlanetaryAnalysisRulesError::InvalidAnalysisDuration);
        }
        if config.minimum_habitability > 100 {
            return Err(PlanetaryAnalysisRulesError::InvalidMinimumHabitability(
                config.minimum_habitability,
            ));
        }
        if config.maximum_colonies == 0 {
            return Err(PlanetaryAnalysisRulesError::InvalidMaximumColonies);
        }
        if config.foundation_cost.metal == 0
            && config.foundation_cost.crystal == 0
            && config.foundation_cost.fuel == 0
        {
            return Err(PlanetaryAnalysisRulesError::EmptyFoundationCost);
        }
        let Some(colony_ship) = craftables.id_by_key(&config.colony_ship_id) else {
            return Err(PlanetaryAnalysisRulesError::UnknownColonyShip);
        };
        let colony_ship_definition = craftables.definition(colony_ship);
        let Some(ship) = colony_ship_definition.ship else {
            return Err(PlanetaryAnalysisRulesError::InvalidColonyShip(colony_ship));
        };
        if ship.class != ShipClass::Colony {
            return Err(PlanetaryAnalysisRulesError::InvalidColonyShip(colony_ship));
        }
        let foundation_cargo = config
            .foundation_cost
            .metal
            .checked_add(config.foundation_cost.crystal)
            .and_then(|total| total.checked_add(config.foundation_cost.fuel))
            .ok_or(PlanetaryAnalysisRulesError::FoundationCargoOverflow)?;
        if ship.cargo_capacity < foundation_cargo {
            return Err(PlanetaryAnalysisRulesError::FoundationCargoTooSmall {
                colony_ship,
                required: foundation_cargo,
                available: ship.cargo_capacity,
            });
        }
        if config.new_colony.population == 0 {
            return Err(PlanetaryAnalysisRulesError::InvalidNewColonyPopulation);
        }
        let mut initial_buildings = BuildingLevels::EMPTY;
        let mut configured_buildings = BTreeSet::new();
        for configured in config.new_colony.buildings {
            if !configured_buildings.insert(configured.id.clone()) {
                return Err(PlanetaryAnalysisRulesError::DuplicateNewColonyBuilding);
            }
            let Some(kind) = buildings.kind_by_key(&configured.id) else {
                return Err(PlanetaryAnalysisRulesError::UnknownNewColonyBuilding);
            };
            initial_buildings.set_level(kind, configured.level);
        }
        buildings
            .validate_levels(initial_buildings)
            .map_err(PlanetaryAnalysisRulesError::InvalidNewColonyBuildings)?;
        let initial_energy = buildings.energy_grid_for_levels(initial_buildings);
        if initial_energy.is_deficit() {
            return Err(PlanetaryAnalysisRulesError::InvalidNewColonyEnergy);
        }
        let foundation_cost = ResourceCost::new(
            config.foundation_cost.metal,
            config.foundation_cost.crystal,
            config.foundation_cost.fuel,
        );
        if !foundation_cost
            .as_stock()
            .is_within(buildings.storage_capacity_for_levels(initial_buildings))
        {
            return Err(PlanetaryAnalysisRulesError::FoundationExceedsNewColonyStorage);
        }

        let mut configured_kinds = BTreeSet::new();
        let mut kinds = Vec::with_capacity(config.kinds.len());
        for configured in config.kinds {
            let kind = configured.kind.into();
            if !configured_kinds.insert(kind_fingerprint_tag(kind)) {
                return Err(PlanetaryAnalysisRulesError::DuplicatePlanetKind(kind));
            }
            let base_resource_profile = PlanetResourceProfile::new(
                configured.base_resources.metal,
                configured.base_resources.crystal,
                configured.base_resources.fuel,
                configured.base_resources.energy,
            );
            if !base_resource_profile.is_viable() {
                return Err(PlanetaryAnalysisRulesError::InvalidResourceProfile(kind));
            }
            let mut unique_constraints = BTreeSet::new();
            for constraint in &configured.constraints {
                if !unique_constraints.insert(*constraint) {
                    return Err(PlanetaryAnalysisRulesError::DuplicateConstraint {
                        kind,
                        constraint: *constraint,
                    });
                }
            }
            kinds.push(PlanetTypeAnalysisRule {
                kind,
                environment: configured.environment,
                base_resource_profile,
                colonizable: configured.colonizable,
                constraints: InstallationConstraints::from_constraints(configured.constraints),
            });
        }

        for kind in PlanetKind::ALL {
            if !configured_kinds.contains(&kind_fingerprint_tag(kind)) {
                return Err(PlanetaryAnalysisRulesError::MissingPlanetKind(kind));
            }
        }
        kinds.sort_by_key(|rule| kind_fingerprint_tag(rule.kind));

        Ok(Self {
            version: config.version,
            minimum_habitability: config.minimum_habitability,
            maximum_colonies: config.maximum_colonies,
            analysis_duration: StrategicDuration::from_ticks(analysis_duration_ticks),
            foundation_cost,
            colony_ship,
            colony_initialization: ColonyInitializationRules {
                population: config.new_colony.population,
                buildings: initial_buildings,
            },
            kinds,
        })
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn minimum_habitability(&self) -> u8 {
        self.minimum_habitability
    }

    pub const fn maximum_colonies(&self) -> usize {
        self.maximum_colonies
    }

    pub const fn analysis_duration(&self) -> StrategicDuration {
        self.analysis_duration
    }

    pub const fn foundation_cost(&self) -> ResourceCost {
        self.foundation_cost
    }

    pub const fn colony_ship(&self) -> CraftableId {
        self.colony_ship
    }

    pub const fn colony_initialization(&self) -> ColonyInitializationRules {
        self.colony_initialization
    }

    pub fn rule_for(&self, kind: PlanetKind) -> PlanetTypeAnalysisRule {
        *self
            .kinds
            .iter()
            .find(|rule| rule.kind == kind)
            .expect("validated analysis rules contain every planet kind")
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("analysis:");
        output.push_str(&self.version.to_string());
        output.push(':');
        output.push_str(&self.minimum_habitability.to_string());
        output.push(':');
        output.push_str(&self.maximum_colonies.to_string());
        output.push(':');
        output.push_str(&self.analysis_duration.ticks().to_string());
        output.push(':');
        output.push_str(&self.foundation_cost.metal.to_string());
        output.push(',');
        output.push_str(&self.foundation_cost.crystal.to_string());
        output.push(',');
        output.push_str(&self.foundation_cost.fuel.to_string());
        output.push(':');
        output.push_str(self.colony_ship.key());
        output.push(':');
        output.push_str(&self.colony_initialization.population.to_string());
        output.push('[');
        for (kind, level) in self.colony_initialization.buildings.iter() {
            output.push_str(kind.key());
            output.push('=');
            output.push_str(&level.to_string());
            output.push(',');
        }
        output.push(']');
        output.push(';');
        for rule in &self.kinds {
            output.push_str(&kind_fingerprint_tag(rule.kind).to_string());
            output.push(':');
            output.push_str(environment_key(rule.environment));
            output.push(':');
            output.push_str(if rule.colonizable { "yes" } else { "no" });
            output.push(':');
            output.push_str(&rule.base_resource_profile.metal.to_string());
            output.push(',');
            output.push_str(&rule.base_resource_profile.crystal.to_string());
            output.push(',');
            output.push_str(&rule.base_resource_profile.fuel.to_string());
            output.push(',');
            output.push_str(&rule.base_resource_profile.energy.to_string());
            output.push(':');
            for constraint in rule.constraints.iter() {
                output.push_str(constraint_key(constraint));
                output.push(',');
            }
            output.push(';');
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanetAnalysisReport {
    pub planet_id: PlanetId,
    pub system_id: SystemId,
    pub analyzed_at: StrategicTick,
    pub habitability: u8,
    pub environment: PlanetEnvironment,
    pub resource_profile: PlanetResourceProfile,
    pub constraints: InstallationConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetAnalysisOutcome {
    pub report: PlanetAnalysisReport,
    pub knowledge_changes: Vec<KnowledgeChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetAnalysisError {
    Access(AuthorizationError),
    UnknownPlanet(PlanetId),
    PlanetNotProbed {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    MissingTechnology(TechnologyUnlock),
    AlreadyAnalyzed(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetAnalysisRejected {
    pub planet_id: PlanetId,
    pub error: PlanetAnalysisError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColonizationBlocker {
    UnknownPlanet(PlanetId),
    NotAnalyzed {
        current: KnowledgeLevel,
    },
    MissingAnalysisReport,
    AlreadyColonized,
    FoundationAlreadyPrepared,
    OccupiedPlanet {
        occupant: FactionId,
        relation: DiplomaticRelation,
    },
    MissingTechnology(TechnologyUnlock),
    UnsupportedEnvironment(PlanetEnvironment),
    HabitabilityTooLow {
        minimum: u8,
        found: u8,
    },
    NoAccessibleRoute,
    ColonyLimitReached {
        maximum: usize,
    },
    InsufficientFoundationResources {
        cost: ResourceCost,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonizabilityAssessment {
    pub planet_id: PlanetId,
    pub foundation_cost: ResourceCost,
    pub blockers: Vec<ColonizationBlocker>,
}

impl ColonizabilityAssessment {
    pub fn is_colonizable(&self) -> bool {
        self.blockers.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetAnalysisStateError {
    DuplicateReport(PlanetId),
    UnknownPlanet(PlanetId),
    SystemMismatch {
        planet_id: PlanetId,
        expected: SystemId,
        found: SystemId,
    },
    KnowledgeTooLow {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    ReportInFuture {
        planet_id: PlanetId,
        analyzed_at: StrategicTick,
        current_tick: StrategicTick,
    },
    ReportDoesNotMatchPlanet(PlanetId),
    MissingReport(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryAnalysisRulesError {
    UnsupportedVersion(u32),
    InvalidAnalysisDuration,
    InvalidMinimumHabitability(u8),
    InvalidMaximumColonies,
    EmptyFoundationCost,
    UnknownColonyShip,
    InvalidColonyShip(CraftableId),
    FoundationCargoOverflow,
    FoundationCargoTooSmall {
        colony_ship: CraftableId,
        required: u64,
        available: u64,
    },
    InvalidNewColonyPopulation,
    DuplicateNewColonyBuilding,
    UnknownNewColonyBuilding,
    InvalidNewColonyBuildings(BuildingCatalogError),
    InvalidNewColonyEnergy,
    FoundationExceedsNewColonyStorage,
    DuplicatePlanetKind(PlanetKind),
    MissingPlanetKind(PlanetKind),
    InvalidResourceProfile(PlanetKind),
    DuplicateConstraint {
        kind: PlanetKind,
        constraint: InstallationConstraint,
    },
}

pub fn planetary_analysis_rules() -> &'static PlanetaryAnalysisRules {
    default_ruleset().planetary_analysis()
}

pub fn analyze_planet(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    planet_id: PlanetId,
) -> Result<PlanetAnalysisOutcome, PlanetAnalysisError> {
    state
        .authorize_management(actor, Owner::Faction(state.player_faction))
        .map_err(PlanetAnalysisError::Access)?;
    let Some((system_id, planet)) = universe.planet_location(planet_id) else {
        return Err(PlanetAnalysisError::UnknownPlanet(planet_id));
    };
    let current = state.planet_knowledge_level(planet_id);
    if current < KnowledgeLevel::Probed {
        return Err(PlanetAnalysisError::PlanetNotProbed { planet_id, current });
    }
    if current.reveals_exact_details() {
        return Err(PlanetAnalysisError::AlreadyAnalyzed(planet_id));
    }
    if !state.research.has_unlock(TechnologyUnlock::AnalyzePlanets) {
        return Err(PlanetAnalysisError::MissingTechnology(
            TechnologyUnlock::AnalyzePlanets,
        ));
    }

    let report = build_planet_analysis_report(
        planet,
        system_id,
        state.clock.current_tick(),
        planetary_analysis_rules(),
    );
    let knowledge_changes =
        state.advance_planet_knowledge(universe, planet_id, KnowledgeLevel::Analyzed);
    state.planet_analysis_reports.push(report);
    state
        .planet_analysis_reports
        .sort_by_key(|entry| entry.planet_id);
    let observed_at = state.clock.current_tick();
    refresh_planetary_intelligence(
        state,
        planet_id,
        PlanetaryIntelPrecision::Surveyed,
        observed_at,
    )
    .expect("an analyzed planet has a validated planetary presence");

    Ok(PlanetAnalysisOutcome {
        report,
        knowledge_changes,
    })
}

pub fn assess_planet_colonizability(
    state: &GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    planet_id: PlanetId,
) -> ColonizabilityAssessment {
    let rules = planetary_analysis_rules();
    let mut blockers = Vec::new();
    let Some((system_id, planet)) = universe.planet_location(planet_id) else {
        blockers.push(ColonizationBlocker::UnknownPlanet(planet_id));
        return ColonizabilityAssessment {
            planet_id,
            foundation_cost: rules.foundation_cost(),
            blockers,
        };
    };

    let current = state.planet_knowledge_level(planet_id);
    if !current.reveals_exact_details() {
        blockers.push(ColonizationBlocker::NotAnalyzed { current });
    }
    let report = state.planet_analysis_report(planet_id);
    if current == KnowledgeLevel::Analyzed && report.is_none() {
        blockers.push(ColonizationBlocker::MissingAnalysisReport);
    }
    if state.colony_on_planet(planet_id).is_some() {
        blockers.push(ColonizationBlocker::AlreadyColonized);
    }
    if state.colony_on_planet(planet_id).is_none()
        && state.colony_foundation_on_planet(planet_id).is_some()
    {
        blockers.push(ColonizationBlocker::FoundationAlreadyPrepared);
    }
    if let Some(Owner::Faction(occupant)) = state
        .planetary_presence(planet_id)
        .map(|presence| presence.occupant)
        && occupant != actor
    {
        blockers.push(ColonizationBlocker::OccupiedPlanet {
            occupant,
            relation: state
                .relation_between(actor, occupant)
                .unwrap_or(DiplomaticRelation::Unknown),
        });
    }
    if !state.research.has_unlock(TechnologyUnlock::FoundColonies) {
        blockers.push(ColonizationBlocker::MissingTechnology(
            TechnologyUnlock::FoundColonies,
        ));
    }
    let kind_rule = rules.rule_for(planet.kind);
    if !kind_rule.colonizable {
        blockers.push(ColonizationBlocker::UnsupportedEnvironment(
            kind_rule.environment,
        ));
    }
    if planet.habitability < rules.minimum_habitability() {
        blockers.push(ColonizationBlocker::HabitabilityTooLow {
            minimum: rules.minimum_habitability(),
            found: planet.habitability,
        });
    }

    let actor_colonies = state
        .colonies
        .iter()
        .filter(|colony| state.can_manage(actor, colony.owner))
        .collect::<Vec<_>>();
    let actor_foundations = state
        .colony_foundations
        .iter()
        .filter(|foundation| {
            foundation.owner == actor && state.colony_on_planet(foundation.planet_id).is_none()
        })
        .count();
    if actor_colonies.len().saturating_add(actor_foundations) >= rules.maximum_colonies() {
        blockers.push(ColonizationBlocker::ColonyLimitReached {
            maximum: rules.maximum_colonies(),
        });
    }
    if !actor_colonies.iter().any(|colony| {
        colony.system_id == system_id
            || accessible_route_exists(state, universe, colony.system_id, system_id)
    }) {
        blockers.push(ColonizationBlocker::NoAccessibleRoute);
    }
    if !actor_colonies.iter().any(|colony| {
        colony
            .resources
            .available()
            .can_cover(rules.foundation_cost())
    }) {
        blockers.push(ColonizationBlocker::InsufficientFoundationResources {
            cost: rules.foundation_cost(),
        });
    }

    ColonizabilityAssessment {
        planet_id,
        foundation_cost: rules.foundation_cost(),
        blockers,
    }
}

pub fn validate_planet_analysis_state(
    state: &GameState,
    universe: &UniverseRepository,
) -> Result<(), PlanetAnalysisStateError> {
    let mut report_ids = BTreeSet::new();
    for report in &state.planet_analysis_reports {
        if !report_ids.insert(report.planet_id) {
            return Err(PlanetAnalysisStateError::DuplicateReport(report.planet_id));
        }
        let Some((system_id, planet)) = universe.planet_location(report.planet_id) else {
            return Err(PlanetAnalysisStateError::UnknownPlanet(report.planet_id));
        };
        if system_id != report.system_id {
            return Err(PlanetAnalysisStateError::SystemMismatch {
                planet_id: report.planet_id,
                expected: system_id,
                found: report.system_id,
            });
        }
        let current = state.planet_knowledge_level(report.planet_id);
        if !current.reveals_exact_details() {
            return Err(PlanetAnalysisStateError::KnowledgeTooLow {
                planet_id: report.planet_id,
                current,
            });
        }
        if report.analyzed_at > state.clock.current_tick() {
            return Err(PlanetAnalysisStateError::ReportInFuture {
                planet_id: report.planet_id,
                analyzed_at: report.analyzed_at,
                current_tick: state.clock.current_tick(),
            });
        }
        let expected = build_planet_analysis_report(
            planet,
            system_id,
            report.analyzed_at,
            planetary_analysis_rules(),
        );
        if *report != expected {
            return Err(PlanetAnalysisStateError::ReportDoesNotMatchPlanet(
                report.planet_id,
            ));
        }
    }

    for knowledge in &state.planet_knowledge {
        if knowledge.level == KnowledgeLevel::Analyzed && !report_ids.contains(&knowledge.planet_id)
        {
            return Err(PlanetAnalysisStateError::MissingReport(knowledge.planet_id));
        }
    }
    Ok(())
}

pub(crate) fn build_planet_analysis_report(
    planet: &Planet,
    system_id: SystemId,
    analyzed_at: StrategicTick,
    rules: &PlanetaryAnalysisRules,
) -> PlanetAnalysisReport {
    let rule = rules.rule_for(planet.kind);
    PlanetAnalysisReport {
        planet_id: planet.id,
        system_id,
        analyzed_at,
        habitability: planet.habitability,
        environment: rule.environment,
        resource_profile: varied_resource_profile(rule.base_resource_profile, planet.id),
        constraints: rule.constraints,
    }
}

fn varied_resource_profile(
    base: PlanetResourceProfile,
    planet_id: PlanetId,
) -> PlanetResourceProfile {
    PlanetResourceProfile::new(
        varied_resource_value(base.metal, planet_id, 0x004d_4554_414c),
        varied_resource_value(base.crystal, planet_id, 0x0043_5259_5354_414c),
        varied_resource_value(base.fuel, planet_id, 0x4655_454c),
        varied_resource_value(base.energy, planet_id, 0x454e_4552_4759),
    )
}

fn varied_resource_value(base: u16, planet_id: PlanetId, salt: u64) -> u16 {
    let mixed = splitmix64(planet_id.raw() ^ salt);
    let percentage = 90_u64 + mixed % 21;
    let value = u64::from(base).saturating_mul(percentage) / 100;
    value.clamp(1, u64::from(u16::MAX)) as u16
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn accessible_route_exists(
    state: &GameState,
    universe: &UniverseRepository,
    origin: SystemId,
    target: SystemId,
) -> bool {
    if origin == target {
        return true;
    }
    let mut adjacency = BTreeMap::<SystemId, BTreeSet<SystemId>>::new();
    for route in state.visible_routes(universe) {
        adjacency.entry(route.from).or_default().insert(route.to);
        adjacency.entry(route.to).or_default().insert(route.from);
    }
    let mut queue = VecDeque::from([origin]);
    let mut visited = BTreeSet::from([origin]);
    while let Some(current) = queue.pop_front() {
        let Some(neighbors) = adjacency.get(&current) else {
            continue;
        };
        for neighbor in neighbors {
            if *neighbor == target {
                return true;
            }
            if visited.insert(*neighbor) {
                queue.push_back(*neighbor);
            }
        }
    }
    false
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

const fn environment_key(environment: PlanetEnvironment) -> &'static str {
    match environment {
        PlanetEnvironment::Temperate => "temperate",
        PlanetEnvironment::Oceanic => "oceanic",
        PlanetEnvironment::Arid => "arid",
        PlanetEnvironment::Frozen => "frozen",
        PlanetEnvironment::Volcanic => "volcanic",
        PlanetEnvironment::Gaseous => "gaseous",
    }
}

const fn constraint_key(constraint: InstallationConstraint) -> &'static str {
    match constraint {
        InstallationConstraint::ThinAtmosphere => "thin_atmosphere",
        InstallationConstraint::GlobalOcean => "global_ocean",
        InstallationConstraint::AridClimate => "arid_climate",
        InstallationConstraint::CryogenicClimate => "cryogenic_climate",
        InstallationConstraint::ExtremeVolcanism => "extreme_volcanism",
        InstallationConstraint::NoSolidSurface => "no_solid_surface",
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlanetaryAnalysisRulesConfig {
    version: u32,
    minimum_habitability: u8,
    maximum_colonies: usize,
    analysis_duration_seconds: u64,
    foundation_cost: ResourceCostConfig,
    colony_ship_id: String,
    new_colony: NewColonyConfig,
    kinds: Vec<PlanetTypeAnalysisRuleConfig>,
}

#[derive(Debug, Deserialize)]
struct NewColonyConfig {
    population: u64,
    buildings: Vec<NewColonyBuildingConfig>,
}

#[derive(Debug, Deserialize)]
struct NewColonyBuildingConfig {
    id: String,
    level: u8,
}

#[derive(Debug, Deserialize)]
struct PlanetTypeAnalysisRuleConfig {
    kind: PlanetKindConfig,
    environment: PlanetEnvironment,
    base_resources: PlanetResourceProfileConfig,
    colonizable: bool,
    constraints: Vec<InstallationConstraint>,
}

#[derive(Debug, Deserialize)]
struct ResourceCostConfig {
    metal: u64,
    crystal: u64,
    fuel: u64,
}

#[derive(Debug, Deserialize)]
struct PlanetResourceProfileConfig {
    metal: u16,
    crystal: u16,
    fuel: u16,
    energy: u16,
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

#[cfg(test)]
mod tests {
    use galactic_domain::{ResourceStock, UniverseConfig};

    use crate::{ResearchState, Simulation, TechnologyId};

    use super::*;

    fn first_probed_planet(simulation: &mut Simulation) -> PlanetId {
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains detected planets")
            .planet_id;
        let universe = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &universe,
            planet_id,
            KnowledgeLevel::Probed,
        );
        planet_id
    }

    fn analyze_for_test(
        simulation: &mut Simulation,
        actor: FactionId,
        planet_id: PlanetId,
    ) -> Result<PlanetAnalysisOutcome, PlanetAnalysisError> {
        let universe = simulation.universe_repository().clone();
        analyze_planet(simulation.state_mut(), &universe, actor, planet_id)
    }

    #[test]
    fn default_rules_cover_every_planet_kind() {
        let rules = planetary_analysis_rules();

        assert_eq!(rules.version(), 4);
        assert!(!rules.analysis_duration().is_zero());
        for kind in PlanetKind::ALL {
            assert_eq!(rules.rule_for(kind).kind, kind);
        }
    }

    #[test]
    fn analysis_requires_probe_and_planetary_spectrometry() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let detected = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains detected planets")
            .planet_id;

        assert!(matches!(
            analyze_for_test(&mut simulation, actor, detected),
            Err(PlanetAnalysisError::PlanetNotProbed { .. })
        ));

        let probed = first_probed_planet(&mut simulation);
        assert_eq!(
            analyze_for_test(&mut simulation, actor, probed),
            Err(PlanetAnalysisError::MissingTechnology(
                TechnologyUnlock::AnalyzePlanets
            ))
        );
    }

    #[test]
    fn analysis_reveals_one_exact_report_and_is_monotone() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let planet_id = first_probed_planet(&mut simulation);
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);

        let outcome = analyze_for_test(&mut simulation, actor, planet_id)
            .expect("a probed planet can be analyzed");

        assert_eq!(
            simulation.state().planet_knowledge_level(planet_id),
            KnowledgeLevel::Analyzed
        );
        assert_eq!(
            simulation.state().planet_analysis_report(planet_id),
            Some(&outcome.report)
        );
        assert_eq!(
            analyze_for_test(&mut simulation, actor, planet_id),
            Err(PlanetAnalysisError::AlreadyAnalyzed(planet_id))
        );
    }

    #[test]
    fn colonizability_returns_all_current_blockers() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let planet_id = first_probed_planet(&mut simulation);

        let assessment = assess_planet_colonizability(
            simulation.state(),
            simulation.universe_repository(),
            actor,
            planet_id,
        );

        assert!(
            assessment
                .blockers
                .iter()
                .any(|blocker| matches!(blocker, ColonizationBlocker::NotAnalyzed { .. }))
        );
        assert!(assessment.blockers.iter().any(|blocker| matches!(
            blocker,
            ColonizationBlocker::MissingTechnology(TechnologyUnlock::FoundColonies)
        )));
        assert!(assessment.blockers.iter().any(|blocker| matches!(
            blocker,
            ColonizationBlocker::InsufficientFoundationResources { .. }
        )));
    }

    #[test]
    fn mvp_seed_contains_several_distinct_viable_targets() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let rules = planetary_analysis_rules();
        let mut profiles = BTreeSet::new();
        let viable = simulation
            .universe()
            .systems
            .iter()
            .flat_map(|system| {
                system.planets.iter().filter_map(move |planet| {
                    let rule = rules.rule_for(planet.kind);
                    (rule.colonizable && planet.habitability >= rules.minimum_habitability())
                        .then_some((system.id, planet))
                })
            })
            .take(12)
            .map(|(system_id, planet)| {
                let report =
                    build_planet_analysis_report(planet, system_id, StrategicTick::ZERO, rules);
                profiles.insert((
                    report.resource_profile.metal,
                    report.resource_profile.crystal,
                    report.resource_profile.fuel,
                    report.resource_profile.energy,
                ));
            })
            .count();

        assert!(viable >= 3);
        assert!(profiles.len() >= 3);
    }

    #[test]
    fn enough_resources_remove_the_cost_blocker() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let planet_id = first_probed_planet(&mut simulation);
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::COLONIZATION,
        ]);
        analyze_for_test(&mut simulation, actor, planet_id).expect("analysis succeeds");
        simulation.state_mut().colonies[0]
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test funding fits storage");

        let assessment = assess_planet_colonizability(
            simulation.state(),
            simulation.universe_repository(),
            actor,
            planet_id,
        );

        assert!(!assessment.blockers.iter().any(|blocker| matches!(
            blocker,
            ColonizationBlocker::MissingTechnology(_)
                | ColonizationBlocker::InsufficientFoundationResources { .. }
                | ColonizationBlocker::NotAnalyzed { .. }
        )));
    }
}

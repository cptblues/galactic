// MVP-016-B: global research driven by the active external ruleset.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use galactic_domain::{FactionId, Owner};
use serde::Deserialize;

use crate::{
    AuthorizationError, BuildingCatalog, BuildingEffect, BuildingKind, GameState,
    STRATEGIC_TICKS_PER_SECOND, StrategicDuration, default_building_catalog, default_ruleset,
};

pub const MAX_RULESET_TECHNOLOGIES: usize = 128;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TechnologyId(&'static str);

impl TechnologyId {
    pub const SPATIAL_DETECTION: Self = Self("spatial_detection");
    pub const PROPULSION: Self = Self("propulsion");
    pub const CARGO_CAPACITY: Self = Self("cargo_capacity");
    pub const REMOTE_EXTRACTION: Self = Self("remote_extraction");
    pub const PLANETARY_ANALYSIS: Self = Self("planetary_analysis");
    pub const COLONIZATION: Self = Self("colonization");

    pub const fn from_static(key: &'static str) -> Self {
        Self(key)
    }

    pub const fn key(self) -> &'static str {
        self.0
    }

    fn from_config(key: String) -> Result<Self, TechnologyCatalogError> {
        validate_identifier(&key).map_err(|()| TechnologyCatalogError::InvalidIdentifier)?;
        Ok(Self(Box::leak(key.into_boxed_str())))
    }
}

impl fmt::Debug for TechnologyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TechnologyId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for TechnologyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TechnologyUnlock {
    DetectUnknownSystems,
    InterstellarTravel,
    ExpandedCargo,
    RemoteExtraction,
    AnalyzePlanets,
    FoundColonies,
}

impl TechnologyUnlock {
    const fn key(self) -> &'static str {
        match self {
            Self::DetectUnknownSystems => "detect_unknown_systems",
            Self::InterstellarTravel => "interstellar_travel",
            Self::ExpandedCargo => "expanded_cargo",
            Self::RemoteExtraction => "remote_extraction",
            Self::AnalyzePlanets => "analyze_planets",
            Self::FoundColonies => "found_colonies",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechnologyBuildingPrerequisite {
    pub kind: BuildingKind,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnologyDefinition {
    pub id: TechnologyId,
    pub name: &'static str,
    pub description: &'static str,
    pub required_milli_points: u64,
    pub prerequisites: Vec<TechnologyId>,
    pub building_prerequisites: Vec<TechnologyBuildingPrerequisite>,
    pub unlock: TechnologyUnlock,
    pub unlock_label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnologyCatalog {
    version: u32,
    order: Vec<TechnologyId>,
    definitions: BTreeMap<TechnologyId, TechnologyDefinition>,
}

impl TechnologyCatalog {
    pub(crate) fn from_config(
        config: TechnologyCatalogConfig,
        buildings: &BuildingCatalog,
    ) -> Result<Self, TechnologyCatalogError> {
        if config.version != 1 {
            return Err(TechnologyCatalogError::UnsupportedVersion(config.version));
        }
        if config.technologies.is_empty() || config.technologies.len() > MAX_RULESET_TECHNOLOGIES {
            return Err(TechnologyCatalogError::InvalidTechnologyCount {
                found: config.technologies.len(),
                maximum: MAX_RULESET_TECHNOLOGIES,
            });
        }

        let mut ids = BTreeMap::new();
        let mut order = Vec::with_capacity(config.technologies.len());
        for technology in &config.technologies {
            let id = TechnologyId::from_config(technology.id.clone())?;
            if ids.insert(technology.id.clone(), id).is_some() {
                return Err(TechnologyCatalogError::DuplicateTechnology(id));
            }
            order.push(id);
        }

        let mut definitions = BTreeMap::new();
        for technology in config.technologies {
            let id = ids[&technology.id];
            let prerequisites = technology
                .prerequisites
                .into_iter()
                .map(|prerequisite| {
                    let Some(&prerequisite_id) = ids.get(&prerequisite) else {
                        return Err(TechnologyCatalogError::MissingPrerequisite {
                            technology: id,
                            prerequisite: TechnologyId::from_config(prerequisite)?,
                        });
                    };
                    Ok(prerequisite_id)
                })
                .collect::<Result<Vec<_>, TechnologyCatalogError>>()?;
            let building_prerequisites = technology
                .building_prerequisites
                .into_iter()
                .map(|prerequisite| {
                    let Some(kind) = buildings.kind_by_key(&prerequisite.id) else {
                        return Err(TechnologyCatalogError::MissingBuildingPrerequisite {
                            technology: id,
                            building: BuildingKind::from_static(Box::leak(
                                prerequisite.id.into_boxed_str(),
                            )),
                        });
                    };
                    Ok(TechnologyBuildingPrerequisite {
                        kind,
                        level: prerequisite.level,
                    })
                })
                .collect::<Result<Vec<_>, TechnologyCatalogError>>()?;
            definitions.insert(
                id,
                TechnologyDefinition {
                    id,
                    name: leak_non_empty(technology.name, TechnologyCatalogError::EmptyName(id))?,
                    description: leak_non_empty(
                        technology.description,
                        TechnologyCatalogError::EmptyDescription(id),
                    )?,
                    required_milli_points: technology.required_milli_points,
                    prerequisites,
                    building_prerequisites,
                    unlock: technology.unlock,
                    unlock_label: leak_non_empty(
                        technology.unlock_label,
                        TechnologyCatalogError::EmptyUnlockLabel(id),
                    )?,
                },
            );
        }

        let catalog = Self {
            version: config.version,
            order,
            definitions,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub fn ids(&self) -> impl Iterator<Item = TechnologyId> + '_ {
        self.order.iter().copied()
    }

    pub fn definitions(&self) -> impl Iterator<Item = &TechnologyDefinition> {
        self.order.iter().map(|id| self.definition(*id))
    }

    pub fn definition(&self, technology: TechnologyId) -> &TechnologyDefinition {
        self.definitions
            .get(&technology)
            .expect("validated technology identifier must exist in the active ruleset")
    }

    pub fn id_by_key(&self, key: &str) -> Option<TechnologyId> {
        self.definitions
            .keys()
            .copied()
            .find(|technology| technology.key() == key)
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        for definition in self.definitions() {
            output.push_str(definition.id.key());
            output.push(':');
            output.push_str(definition.unlock.key());
            output.push('[');
            for prerequisite in &definition.prerequisites {
                output.push_str(prerequisite.key());
                output.push(',');
            }
            output.push_str("];");
        }
    }

    fn validate(&self) -> Result<(), TechnologyCatalogError> {
        for definition in self.definitions() {
            if definition.required_milli_points == 0 {
                return Err(TechnologyCatalogError::InvalidCost(definition.id));
            }
            let mut prerequisites = BTreeSet::new();
            for prerequisite in &definition.prerequisites {
                if *prerequisite == definition.id {
                    return Err(TechnologyCatalogError::SelfPrerequisite(definition.id));
                }
                if !prerequisites.insert(*prerequisite) {
                    return Err(TechnologyCatalogError::DuplicatePrerequisite {
                        technology: definition.id,
                        prerequisite: *prerequisite,
                    });
                }
            }
        }
        for technology in self.ids() {
            self.visit(technology, &mut BTreeSet::new(), &mut BTreeSet::new())?;
        }
        Ok(())
    }

    fn visit(
        &self,
        technology: TechnologyId,
        visiting: &mut BTreeSet<TechnologyId>,
        visited: &mut BTreeSet<TechnologyId>,
    ) -> Result<(), TechnologyCatalogError> {
        if visited.contains(&technology) {
            return Ok(());
        }
        if !visiting.insert(technology) {
            return Err(TechnologyCatalogError::PrerequisiteCycle(technology));
        }
        for prerequisite in &self.definition(technology).prerequisites {
            self.visit(*prerequisite, visiting, visited)?;
        }
        visiting.remove(&technology);
        visited.insert(technology);
        Ok(())
    }
}

pub fn technology_catalog() -> &'static TechnologyCatalog {
    default_ruleset().technologies()
}

pub fn technology_definition(technology: TechnologyId) -> &'static TechnologyDefinition {
    technology_catalog().definition(technology)
}

pub fn max_research_queue() -> usize {
    default_ruleset().economy().research_queue_limit
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchProject {
    pub technology: TechnologyId,
    pub required_milli_points: u64,
    pub accumulated_milli_points: u64,
}

impl ResearchProject {
    pub const fn remaining_milli_points(self) -> u64 {
        self.required_milli_points
            .saturating_sub(self.accumulated_milli_points)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchState {
    completed: BTreeSet<TechnologyId>,
    queue: VecDeque<ResearchProject>,
}

impl ResearchState {
    pub fn from_completed(technologies: impl IntoIterator<Item = TechnologyId>) -> Self {
        Self {
            completed: technologies.into_iter().collect(),
            queue: VecDeque::new(),
        }
    }

    pub fn completed(&self) -> impl Iterator<Item = TechnologyId> + '_ {
        self.completed.iter().copied()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn has_completed(&self, technology: TechnologyId) -> bool {
        self.completed.contains(&technology)
    }

    pub fn has_unlock(&self, unlock: TechnologyUnlock) -> bool {
        self.completed()
            .any(|technology| technology_definition(technology).unlock == unlock)
    }

    pub fn queue(&self) -> impl Iterator<Item = &ResearchProject> {
        self.queue.iter()
    }

    pub fn active(&self) -> Option<&ResearchProject> {
        self.queue.front()
    }

    pub fn is_queued(&self, technology: TechnologyId) -> bool {
        self.queue
            .iter()
            .any(|project| project.technology == technology)
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    fn cancel_active(&mut self) -> Option<ResearchProject> {
        self.queue.pop_front()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchQuote {
    pub technology: TechnologyId,
    pub required_milli_points: u64,
    pub output_milli_points_per_tick: u64,
    pub estimated_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchQueued {
    pub project: ResearchProject,
    pub queue_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchCompleted {
    pub technology: TechnologyId,
    pub unlock: TechnologyUnlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchRejected {
    pub technology: TechnologyId,
    pub error: ResearchError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchCancelled {
    pub technology: TechnologyId,
    pub accumulated_milli_points: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchCancellationRejected {
    pub error: ResearchError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchError {
    Access(AuthorizationError),
    NoResearchCapacity,
    AlreadyCompleted(TechnologyId),
    AlreadyQueued(TechnologyId),
    QueueFull {
        maximum: usize,
    },
    MissingPrerequisite {
        technology: TechnologyId,
        prerequisite: TechnologyId,
    },
    MissingBuildingPrerequisite {
        building: BuildingKind,
        required: u8,
        found: u8,
    },
    NoActiveProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStateError {
    TooManyProjects {
        found: usize,
        maximum: usize,
    },
    UnknownCompletedTechnology(TechnologyId),
    UnknownQueuedTechnology(TechnologyId),
    CompletedMissingPrerequisite {
        technology: TechnologyId,
        prerequisite: TechnologyId,
    },
    CompletedAndQueued(TechnologyId),
    DuplicateQueued(TechnologyId),
    QueuedMissingPrerequisite {
        technology: TechnologyId,
        prerequisite: TechnologyId,
    },
    InvalidRequiredPoints {
        technology: TechnologyId,
        expected: u64,
        found: u64,
    },
    InvalidProgress {
        technology: TechnologyId,
        accumulated: u64,
        required: u64,
    },
}

pub fn research_quote(
    state: &GameState,
    actor: FactionId,
    technology: TechnologyId,
) -> Result<ResearchQuote, ResearchError> {
    state
        .authorize_management(actor, Owner::Faction(state.player_faction))
        .map_err(ResearchError::Access)?;
    if state.research.has_completed(technology) {
        return Err(ResearchError::AlreadyCompleted(technology));
    }
    if state.research.is_queued(technology) {
        return Err(ResearchError::AlreadyQueued(technology));
    }
    let maximum = max_research_queue();
    if state.research.queue_len() >= maximum {
        return Err(ResearchError::QueueFull { maximum });
    }

    let output = research_output_milli_points_per_tick(state, actor);
    if output == 0 {
        return Err(ResearchError::NoResearchCapacity);
    }

    let projected = projected_technologies(&state.research);
    let definition = technology_definition(technology);
    for prerequisite in &definition.prerequisites {
        if !projected.contains(prerequisite) {
            return Err(ResearchError::MissingPrerequisite {
                technology,
                prerequisite: *prerequisite,
            });
        }
    }
    for prerequisite in &definition.building_prerequisites {
        let found = best_building_level(state, actor, prerequisite.kind);
        if found < prerequisite.level {
            return Err(ResearchError::MissingBuildingPrerequisite {
                building: prerequisite.kind,
                required: prerequisite.level,
                found,
            });
        }
    }

    Ok(ResearchQuote {
        technology,
        required_milli_points: definition.required_milli_points,
        output_milli_points_per_tick: output,
        estimated_ticks: definition.required_milli_points.div_ceil(output),
    })
}

pub fn enqueue_research(
    state: &mut GameState,
    actor: FactionId,
    technology: TechnologyId,
) -> Result<ResearchQueued, ResearchError> {
    let quote = research_quote(state, actor, technology)?;
    let project = ResearchProject {
        technology,
        required_milli_points: quote.required_milli_points,
        accumulated_milli_points: 0,
    };
    state.research.queue.push_back(project);

    Ok(ResearchQueued {
        project,
        queue_length: state.research.queue_len(),
    })
}

pub fn cancel_research(
    state: &mut GameState,
    actor: FactionId,
) -> Result<ResearchCancelled, ResearchError> {
    state
        .authorize_management(actor, Owner::Faction(state.player_faction))
        .map_err(ResearchError::Access)?;
    let project = state
        .research
        .cancel_active()
        .ok_or(ResearchError::NoActiveProject)?;

    Ok(ResearchCancelled {
        technology: project.technology,
        accumulated_milli_points: project.accumulated_milli_points,
    })
}

pub fn advance_research(
    state: &mut GameState,
    actor: FactionId,
    ticks: StrategicDuration,
) -> Vec<ResearchCompleted> {
    let output = research_output_milli_points_per_tick(state, actor);
    let mut budget = output.saturating_mul(ticks.ticks());
    if budget == 0 {
        return Vec::new();
    }

    let mut completed = Vec::new();
    while budget > 0 {
        let finished = {
            let Some(active) = state.research.queue.front_mut() else {
                break;
            };
            let spent = active.remaining_milli_points().min(budget);
            active.accumulated_milli_points = active.accumulated_milli_points.saturating_add(spent);
            budget -= spent;
            (active.accumulated_milli_points >= active.required_milli_points).then_some(*active)
        };

        let Some(project) = finished else {
            continue;
        };
        state
            .research
            .queue
            .pop_front()
            .expect("the completed project is active");
        state.research.completed.insert(project.technology);
        completed.push(ResearchCompleted {
            technology: project.technology,
            unlock: technology_definition(project.technology).unlock,
        });
    }

    completed
}

/// The best level of a given building among the actor's colonies. A technology's building
/// prerequisite is satisfied as soon as one colony reaches it — research is a global, not a
/// per-colony, capability, so there is no single "the" colony to check against.
fn best_building_level(state: &GameState, actor: FactionId, kind: BuildingKind) -> u8 {
    state
        .colonies
        .iter()
        .filter(|colony| state.can_manage(actor, colony.owner))
        .map(|colony| colony.buildings.level(kind))
        .max()
        .unwrap_or(0)
}

pub fn research_output_milli_points_per_tick(state: &GameState, actor: FactionId) -> u64 {
    let catalog = default_building_catalog();
    state
        .colonies
        .iter()
        .filter(|colony| state.can_manage(actor, colony.owner))
        .map(|colony| {
            catalog
                .definitions()
                .filter_map(|definition| match definition.effect {
                    BuildingEffect::ResearchPoints {
                        milli_per_tick_per_level,
                    } => Some(
                        milli_per_tick_per_level
                            .saturating_mul(u64::from(colony.buildings.level(definition.kind))),
                    ),
                    _ => None,
                })
                .fold(0_u64, u64::saturating_add)
        })
        .fold(0_u64, u64::saturating_add)
}

pub fn research_lab_level_total(state: &GameState, actor: FactionId) -> u32 {
    let catalog = default_building_catalog();
    state
        .colonies
        .iter()
        .filter(|colony| state.can_manage(actor, colony.owner))
        .map(|colony| {
            catalog
                .definitions()
                .filter(|definition| {
                    matches!(definition.effect, BuildingEffect::ResearchPoints { .. })
                })
                .map(|definition| u32::from(colony.buildings.level(definition.kind)))
                .sum::<u32>()
        })
        .sum()
}

pub fn research_output_points_per_second(state: &GameState, actor: FactionId) -> f64 {
    research_output_milli_points_per_tick(state, actor) as f64
        * f64::from(STRATEGIC_TICKS_PER_SECOND)
        / 1_000.0
}

pub fn research_progress_ratio(project: ResearchProject) -> f32 {
    if project.required_milli_points == 0 {
        return 1.0;
    }
    (project.accumulated_milli_points as f64 / project.required_milli_points as f64).clamp(0.0, 1.0)
        as f32
}

pub fn validate_research_state(state: &GameState) -> Result<(), ResearchStateError> {
    let catalog = technology_catalog();
    let maximum = max_research_queue();
    if state.research.queue_len() > maximum {
        return Err(ResearchStateError::TooManyProjects {
            found: state.research.queue_len(),
            maximum,
        });
    }

    for technology in state.research.completed() {
        if !catalog.definitions.contains_key(&technology) {
            return Err(ResearchStateError::UnknownCompletedTechnology(technology));
        }
        let definition = catalog.definition(technology);
        for prerequisite in &definition.prerequisites {
            if !state.research.has_completed(*prerequisite) {
                return Err(ResearchStateError::CompletedMissingPrerequisite {
                    technology,
                    prerequisite: *prerequisite,
                });
            }
        }
    }

    let mut projected = state.research.completed.clone();
    let mut queued = BTreeSet::new();
    for project in state.research.queue() {
        if !catalog.definitions.contains_key(&project.technology) {
            return Err(ResearchStateError::UnknownQueuedTechnology(
                project.technology,
            ));
        }
        if state.research.has_completed(project.technology) {
            return Err(ResearchStateError::CompletedAndQueued(project.technology));
        }
        if !queued.insert(project.technology) {
            return Err(ResearchStateError::DuplicateQueued(project.technology));
        }

        let definition = catalog.definition(project.technology);
        if project.required_milli_points != definition.required_milli_points {
            return Err(ResearchStateError::InvalidRequiredPoints {
                technology: project.technology,
                expected: definition.required_milli_points,
                found: project.required_milli_points,
            });
        }
        if project.required_milli_points == 0
            || project.accumulated_milli_points >= project.required_milli_points
        {
            return Err(ResearchStateError::InvalidProgress {
                technology: project.technology,
                accumulated: project.accumulated_milli_points,
                required: project.required_milli_points,
            });
        }

        for prerequisite in &definition.prerequisites {
            if !projected.contains(prerequisite) {
                return Err(ResearchStateError::QueuedMissingPrerequisite {
                    technology: project.technology,
                    prerequisite: *prerequisite,
                });
            }
        }
        projected.insert(project.technology);
    }

    Ok(())
}

fn projected_technologies(research: &ResearchState) -> BTreeSet<TechnologyId> {
    let mut projected = research.completed.clone();
    projected.extend(research.queue().map(|project| project.technology));
    projected
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechnologyCatalogError {
    InvalidIdentifier,
    UnsupportedVersion(u32),
    InvalidTechnologyCount {
        found: usize,
        maximum: usize,
    },
    DuplicateTechnology(TechnologyId),
    EmptyName(TechnologyId),
    EmptyDescription(TechnologyId),
    EmptyUnlockLabel(TechnologyId),
    InvalidCost(TechnologyId),
    SelfPrerequisite(TechnologyId),
    DuplicatePrerequisite {
        technology: TechnologyId,
        prerequisite: TechnologyId,
    },
    MissingPrerequisite {
        technology: TechnologyId,
        prerequisite: TechnologyId,
    },
    MissingBuildingPrerequisite {
        technology: TechnologyId,
        building: BuildingKind,
    },
    PrerequisiteCycle(TechnologyId),
}

#[derive(Debug, Deserialize)]
pub(crate) struct TechnologyCatalogConfig {
    version: u32,
    technologies: Vec<TechnologyDefinitionConfig>,
}

#[derive(Debug, Deserialize)]
struct TechnologyDefinitionConfig {
    id: String,
    name: String,
    description: String,
    required_milli_points: u64,
    prerequisites: Vec<String>,
    building_prerequisites: Vec<TechnologyBuildingPrerequisiteConfig>,
    unlock: TechnologyUnlock,
    unlock_label: String,
}

#[derive(Debug, Deserialize)]
struct TechnologyBuildingPrerequisiteConfig {
    id: String,
    level: u8,
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(());
    };
    if !first.is_ascii_lowercase() {
        return Err(());
    }
    if chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(())
    }
}

fn leak_non_empty<T>(value: String, error: T) -> Result<&'static str, T> {
    if value.trim().is_empty() {
        Err(error)
    } else {
        Ok(Box::leak(value.into_boxed_str()))
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FactionId, Owner, UniverseConfig};

    use crate::{BuildingKind, Simulation, default_building_catalog};

    use super::*;

    fn simulation_with_lab() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        simulation
    }

    #[test]
    fn default_catalog_contains_the_expected_technologies() {
        assert_eq!(technology_catalog().ids().count(), 6);
        assert_eq!(
            technology_catalog().ids().next(),
            Some(TechnologyId::SPATIAL_DETECTION),
        );
    }

    #[test]
    fn research_requires_an_active_laboratory() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        assert_eq!(
            research_quote(
                simulation.state(),
                simulation.state().player_faction,
                TechnologyId::SPATIAL_DETECTION,
            ),
            Err(ResearchError::NoResearchCapacity),
        );
    }

    #[test]
    fn configured_tree_can_be_queued_in_order() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;
        {
            let colony = simulation
                .state_mut()
                .colonies
                .first_mut()
                .expect("home colony exists");
            colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 4);
            colony
                .buildings
                .set_level(BuildingKind::CONSTRUCTION_CENTER, 3);
            colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        }
        for technology in technology_catalog().ids() {
            enqueue_research(simulation.state_mut(), actor, technology)
                .expect("ordered tree is valid");
        }
        assert_eq!(
            simulation.state().research.queue_len(),
            technology_catalog().ids().count(),
        );
    }

    #[test]
    fn a_missing_building_prerequisite_blocks_research_even_with_science_and_tech_ready() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;
        enqueue_research(
            simulation.state_mut(),
            actor,
            TechnologyId::SPATIAL_DETECTION,
        )
        .expect("spatial detection has no prerequisite");
        enqueue_research(simulation.state_mut(), actor, TechnologyId::PROPULSION)
            .expect("propulsion has no unmet prerequisite");
        enqueue_research(simulation.state_mut(), actor, TechnologyId::CARGO_CAPACITY)
            .expect("cargo capacity has no unmet prerequisite");
        enqueue_research(
            simulation.state_mut(),
            actor,
            TechnologyId::PLANETARY_ANALYSIS,
        )
        .expect("planetary analysis has no unmet prerequisite");

        // Tech-to-tech prerequisites are satisfied (both are queued), but the home colony's
        // Institut d'analyse (research_lab) is still level 1, below colonization's building
        // requirement of level 4.
        assert_eq!(
            research_quote(simulation.state(), actor, TechnologyId::COLONIZATION),
            Err(ResearchError::MissingBuildingPrerequisite {
                building: BuildingKind::RESEARCH_LAB,
                required: 4,
                found: 1,
            }),
        );
    }

    #[test]
    fn a_building_prerequisite_is_satisfied_by_any_single_colony_reaching_the_level() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;
        enqueue_research(
            simulation.state_mut(),
            actor,
            TechnologyId::SPATIAL_DETECTION,
        )
        .expect("spatial detection has no prerequisite");
        enqueue_research(simulation.state_mut(), actor, TechnologyId::PROPULSION)
            .expect("propulsion has no unmet prerequisite");
        enqueue_research(simulation.state_mut(), actor, TechnologyId::CARGO_CAPACITY)
            .expect("cargo capacity has no unmet prerequisite");
        enqueue_research(
            simulation.state_mut(),
            actor,
            TechnologyId::PLANETARY_ANALYSIS,
        )
        .expect("planetary analysis has no unmet prerequisite");

        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 4);
        colony
            .buildings
            .set_level(BuildingKind::CONSTRUCTION_CENTER, 3);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);

        assert!(research_quote(simulation.state(), actor, TechnologyId::COLONIZATION).is_ok());
    }

    #[test]
    fn every_player_laboratory_contributes_to_the_global_research_output() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;
        let one_lab = research_output_milli_points_per_tick(simulation.state(), actor);
        assert!(one_lab > 0);

        let mut second = simulation.state().colonies[0].clone();
        second.id = ColonyId::new(1);
        second.name = "Laboratoire Boréal".to_string();
        second.buildings.set_level(BuildingKind::RESEARCH_LAB, 2);
        second.energy = default_building_catalog().energy_grid_for_levels(second.buildings);
        simulation.state_mut().colonies.push(second);

        let mut foreign = simulation.state().colonies[0].clone();
        foreign.id = ColonyId::new(99);
        foreign.owner = Owner::Faction(FactionId::new(2));
        foreign.buildings.set_level(BuildingKind::RESEARCH_LAB, 3);
        foreign.energy = default_building_catalog().energy_grid_for_levels(foreign.buildings);
        simulation.state_mut().colonies.push(foreign);

        assert_eq!(
            research_output_milli_points_per_tick(simulation.state(), actor),
            one_lab.saturating_mul(3),
        );
        assert_eq!(research_lab_level_total(simulation.state(), actor), 3);
    }

    #[test]
    fn a_new_technology_with_a_known_unlock_is_data_only() {
        let source = r#"(
            version: 1,
            technologies: [(
                id: "approved_truth",
                name: "Vérité homologuée",
                description: "Améliore officiellement tous les rapports.",
                required_milli_points: 1000,
                prerequisites: [],
                building_prerequisites: [],
                unlock: AnalyzePlanets,
                unlock_label: "Rapports certifiés",
            )],
        )"#;
        let config = ron::de::from_str(source).expect("test technology RON is valid");
        let catalog = TechnologyCatalog::from_config(config, default_building_catalog())
            .expect("known unlocks accept new identifiers");

        assert!(catalog.id_by_key("approved_truth").is_some());
    }

    #[test]
    fn a_technology_referencing_an_unknown_building_is_rejected_at_load_time() {
        let source = r#"(
            version: 1,
            technologies: [(
                id: "approved_truth",
                name: "Vérité homologuée",
                description: "Améliore officiellement tous les rapports.",
                required_milli_points: 1000,
                prerequisites: [],
                building_prerequisites: [(id: "nonexistent_building", level: 1)],
                unlock: AnalyzePlanets,
                unlock_label: "Rapports certifiés",
            )],
        )"#;
        let config = ron::de::from_str(source).expect("test technology RON is valid");

        let error = TechnologyCatalog::from_config(config, default_building_catalog())
            .expect_err("an unknown building key must be rejected");

        assert!(matches!(
            error,
            TechnologyCatalogError::MissingBuildingPrerequisite { .. }
        ));
    }

    #[test]
    fn cancelling_an_active_project_loses_its_progress() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;
        enqueue_research(
            simulation.state_mut(),
            actor,
            TechnologyId::SPATIAL_DETECTION,
        )
        .expect("spatial detection has no prerequisite");
        simulation.advance(std::time::Duration::from_secs(1));
        assert!(
            simulation
                .state()
                .research
                .active()
                .expect("a project is active")
                .accumulated_milli_points
                > 0,
            "the test must make some real progress before cancelling",
        );

        let cancelled = cancel_research(simulation.state_mut(), actor)
            .expect("an active project can be cancelled");
        assert_eq!(cancelled.technology, TechnologyId::SPATIAL_DETECTION);
        assert!(cancelled.accumulated_milli_points > 0);
        assert!(simulation.state().research.is_queue_empty());
        assert!(
            !simulation
                .state()
                .research
                .has_completed(TechnologyId::SPATIAL_DETECTION)
        );
    }

    #[test]
    fn cancelling_without_an_active_project_is_an_explicit_error() {
        let mut simulation = simulation_with_lab();
        let actor = simulation.state().player_faction;

        assert_eq!(
            cancel_research(simulation.state_mut(), actor),
            Err(ResearchError::NoActiveProject),
        );
    }
}

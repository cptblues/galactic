// MVP-016: global research queue and minimal technology tree.
use std::collections::{BTreeSet, VecDeque};

use crate::{
    BuildingEffect, BuildingKind, GameState, STRATEGIC_TICKS_PER_SECOND, StrategicDuration,
    default_building_catalog,
};

pub const RESEARCH_CATALOG_VERSION: u32 = 1;
pub const RESEARCH_CATALOG_FINGERPRINT: u64 = 0x2f55_29d1_26a4_7d83;
pub const MAX_RESEARCH_QUEUE: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TechnologyId {
    SpatialDetection,
    Propulsion,
    CargoCapacity,
    RemoteExtraction,
    PlanetaryAnalysis,
    Colonization,
}

impl TechnologyId {
    pub const ALL: [Self; 6] = [
        Self::SpatialDetection,
        Self::Propulsion,
        Self::CargoCapacity,
        Self::RemoteExtraction,
        Self::PlanetaryAnalysis,
        Self::Colonization,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TechnologyUnlock {
    DetectUnknownSystems,
    InterstellarTravel,
    ExpandedCargo,
    RemoteExtraction,
    AnalyzePlanets,
    FoundColonies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TechnologyDefinition {
    pub id: TechnologyId,
    pub name: &'static str,
    pub description: &'static str,
    pub required_milli_points: u64,
    pub prerequisites: &'static [TechnologyId],
    pub unlock: TechnologyUnlock,
    pub unlock_label: &'static str,
}

const NO_PREREQUISITES: &[TechnologyId] = &[];
const PROPULSION_PREREQUISITES: &[TechnologyId] = &[TechnologyId::SpatialDetection];
const CARGO_PREREQUISITES: &[TechnologyId] = &[TechnologyId::Propulsion];
const REMOTE_EXTRACTION_PREREQUISITES: &[TechnologyId] = &[TechnologyId::CargoCapacity];
const PLANETARY_ANALYSIS_PREREQUISITES: &[TechnologyId] = &[TechnologyId::SpatialDetection];
const COLONIZATION_PREREQUISITES: &[TechnologyId] =
    &[TechnologyId::CargoCapacity, TechnologyId::PlanetaryAnalysis];

pub const TECHNOLOGY_CATALOG: [TechnologyDefinition; 6] = [
    TechnologyDefinition {
        id: TechnologyId::SpatialDetection,
        name: "Détection spatiale",
        description: "Améliore la détection des systèmes et prépare les missions de reconnaissance.",
        required_milli_points: 15_000,
        prerequisites: NO_PREREQUISITES,
        unlock: TechnologyUnlock::DetectUnknownSystems,
        unlock_label: "Détection des systèmes inconnus",
    },
    TechnologyDefinition {
        id: TechnologyId::Propulsion,
        name: "Propulsion",
        description: "Débloque les moteurs nécessaires aux déplacements interstellaires.",
        required_milli_points: 20_000,
        prerequisites: PROPULSION_PREREQUISITES,
        unlock: TechnologyUnlock::InterstellarTravel,
        unlock_label: "Voyage interstellaire",
    },
    TechnologyDefinition {
        id: TechnologyId::CargoCapacity,
        name: "Capacité cargo",
        description: "Augmente la capacité logistique des futurs vaisseaux et transports.",
        required_milli_points: 25_000,
        prerequisites: CARGO_PREREQUISITES,
        unlock: TechnologyUnlock::ExpandedCargo,
        unlock_label: "Soutes cargo améliorées",
    },
    TechnologyDefinition {
        id: TechnologyId::RemoteExtraction,
        name: "Extraction distante",
        description: "Autorise l'exploitation de ressources sans colonie permanente.",
        required_milli_points: 30_000,
        prerequisites: REMOTE_EXTRACTION_PREREQUISITES,
        unlock: TechnologyUnlock::RemoteExtraction,
        unlock_label: "Missions d'extraction distante",
    },
    TechnologyDefinition {
        id: TechnologyId::PlanetaryAnalysis,
        name: "Analyse planétaire",
        description: "Révèle les caractéristiques nécessaires à l'évaluation des mondes.",
        required_milli_points: 25_000,
        prerequisites: PLANETARY_ANALYSIS_PREREQUISITES,
        unlock: TechnologyUnlock::AnalyzePlanets,
        unlock_label: "Analyse exacte des planètes",
    },
    TechnologyDefinition {
        id: TechnologyId::Colonization,
        name: "Colonisation",
        description: "Débloque la fondation de nouvelles colonies sur les mondes compatibles.",
        required_milli_points: 45_000,
        prerequisites: COLONIZATION_PREREQUISITES,
        unlock: TechnologyUnlock::FoundColonies,
        unlock_label: "Fondation de colonies",
    },
];

pub fn technology_definition(technology: TechnologyId) -> &'static TechnologyDefinition {
    TECHNOLOGY_CATALOG
        .iter()
        .find(|definition| definition.id == technology)
        .expect("the static technology catalog is complete")
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
pub enum ResearchError {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStateError {
    TooManyProjects {
        found: usize,
        maximum: usize,
    },
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
    technology: TechnologyId,
) -> Result<ResearchQuote, ResearchError> {
    if state.research.has_completed(technology) {
        return Err(ResearchError::AlreadyCompleted(technology));
    }
    if state.research.is_queued(technology) {
        return Err(ResearchError::AlreadyQueued(technology));
    }
    if state.research.queue_len() >= MAX_RESEARCH_QUEUE {
        return Err(ResearchError::QueueFull {
            maximum: MAX_RESEARCH_QUEUE,
        });
    }

    let output = research_output_milli_points_per_tick(state);
    if output == 0 {
        return Err(ResearchError::NoResearchCapacity);
    }

    let projected = projected_technologies(&state.research);
    let definition = technology_definition(technology);
    for prerequisite in definition.prerequisites {
        if !projected.contains(prerequisite) {
            return Err(ResearchError::MissingPrerequisite {
                technology,
                prerequisite: *prerequisite,
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
    technology: TechnologyId,
) -> Result<ResearchQueued, ResearchError> {
    let quote = research_quote(state, technology)?;
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

pub fn advance_research(state: &mut GameState, ticks: StrategicDuration) -> Vec<ResearchCompleted> {
    let output = research_output_milli_points_per_tick(state);
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

        let definition = technology_definition(project.technology);
        completed.push(ResearchCompleted {
            technology: project.technology,
            unlock: definition.unlock,
        });
    }

    completed
}

pub fn research_output_milli_points_per_tick(state: &GameState) -> u64 {
    let definition = default_building_catalog().definition(BuildingKind::ResearchLab);
    let per_level = match definition.effect {
        BuildingEffect::ResearchPoints {
            milli_per_tick_per_level,
        } => milli_per_tick_per_level,
        _ => 0,
    };

    state
        .colonies
        .iter()
        .filter(|colony| colony.faction == state.player_faction)
        .map(|colony| {
            per_level.saturating_mul(u64::from(colony.buildings.level(BuildingKind::ResearchLab)))
        })
        .fold(0_u64, u64::saturating_add)
}

pub fn research_lab_level_total(state: &GameState) -> u32 {
    state
        .colonies
        .iter()
        .filter(|colony| colony.faction == state.player_faction)
        .map(|colony| u32::from(colony.buildings.level(BuildingKind::ResearchLab)))
        .sum()
}

pub fn research_output_points_per_second(state: &GameState) -> f64 {
    research_output_milli_points_per_tick(state) as f64 * f64::from(STRATEGIC_TICKS_PER_SECOND)
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
    if state.research.queue_len() > MAX_RESEARCH_QUEUE {
        return Err(ResearchStateError::TooManyProjects {
            found: state.research.queue_len(),
            maximum: MAX_RESEARCH_QUEUE,
        });
    }

    for technology in state.research.completed() {
        let definition = technology_definition(technology);
        for prerequisite in definition.prerequisites {
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
        if state.research.has_completed(project.technology) {
            return Err(ResearchStateError::CompletedAndQueued(project.technology));
        }
        if !queued.insert(project.technology) {
            return Err(ResearchStateError::DuplicateQueued(project.technology));
        }

        let definition = technology_definition(project.technology);
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

        for prerequisite in definition.prerequisites {
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

#[cfg(test)]
impl TechnologyUnlock {
    const ALL_FOR_TESTS: [Self; 6] = [
        Self::DetectUnknownSystems,
        Self::InterstellarTravel,
        Self::ExpandedCargo,
        Self::RemoteExtraction,
        Self::AnalyzePlanets,
        Self::FoundColonies,
    ];
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;

    use crate::{Simulation, default_building_catalog};

    use super::*;

    fn simulation_with_lab() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.research_lab = 1;
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        simulation
    }

    #[test]
    fn catalog_contains_the_six_expected_technologies() {
        assert_eq!(
            TECHNOLOGY_CATALOG
                .iter()
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            TechnologyId::ALL.to_vec(),
        );
        assert!(TECHNOLOGY_CATALOG.iter().all(|definition| {
            definition.required_milli_points > 0
                && !definition.name.is_empty()
                && !definition.unlock_label.is_empty()
        }));
    }

    #[test]
    fn research_requires_an_active_laboratory() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        assert_eq!(
            research_quote(simulation.state(), TechnologyId::SpatialDetection,),
            Err(ResearchError::NoResearchCapacity),
        );
    }

    #[test]
    fn all_technologies_can_be_queued_in_valid_order() {
        let mut simulation = simulation_with_lab();
        for technology in TechnologyId::ALL {
            enqueue_research(simulation.state_mut(), technology)
                .expect("the canonical order is valid");
        }

        assert_eq!(
            simulation.state().research.queue_len(),
            TechnologyId::ALL.len(),
        );
        assert!(validate_research_state(simulation.state()).is_ok());
    }

    #[test]
    fn completed_technology_cannot_be_relaunched() {
        let mut simulation = simulation_with_lab();
        enqueue_research(simulation.state_mut(), TechnologyId::SpatialDetection)
            .expect("root technology is valid");
        advance_research(simulation.state_mut(), StrategicDuration::from_ticks(1_000));

        assert_eq!(
            research_quote(simulation.state(), TechnologyId::SpatialDetection,),
            Err(ResearchError::AlreadyCompleted(
                TechnologyId::SpatialDetection,
            )),
        );
    }

    #[test]
    fn large_tick_batches_complete_multiple_projects() {
        let mut simulation = simulation_with_lab();
        for technology in TechnologyId::ALL {
            enqueue_research(simulation.state_mut(), technology)
                .expect("the canonical order is valid");
        }

        let completed = advance_research(
            simulation.state_mut(),
            StrategicDuration::from_ticks(10_000),
        );

        assert_eq!(completed.len(), TechnologyId::ALL.len(),);
        assert!(
            TechnologyUnlock::ALL_FOR_TESTS
                .iter()
                .all(|unlock| { simulation.state().research.has_unlock(*unlock) })
        );
    }

    #[test]
    fn progress_ratio_is_clamped() {
        let project = ResearchProject {
            technology: TechnologyId::SpatialDetection,
            required_milli_points: 100,
            accumulated_milli_points: 50,
        };
        assert_eq!(research_progress_ratio(project), 0.5);
    }
}

#!/usr/bin/env python3
"""
Applique MVP-016 au dépôt Galactic.

Baseline exacte :
    1f73613d9c2d2b6df77b8d3c3ef7e9b4dd71674c
    feat improve build ui

Différence importante avec les anciens générateurs :
- vérification des blobs Git exacts avant patch ;
- nouveaux modules autonomes `research.rs` et `research_ui.rs` ;
- application et compilation dans un worktree temporaire ;
- aucun fichier du dépôt principal n'est modifié si le préflight échoue ;
- formatage, cargo check, Clippy et tests avant l'écriture réelle.

Usage :
    python tools/apply_mvp_016.py --dry-run
    python tools/apply_mvp_016.py
    python tools/apply_mvp_016.py --skip-checks
    python tools/apply_mvp_016.py --root /chemin/vers/galactic
"""

from __future__ import annotations

import argparse
import difflib
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = (
    "1f73613d9c2d2b6df77b8d3c3ef7e9b4dd71674c"
)

EXPECTED_BLOBS = {
    "crates/galactic_client/src/lib.rs":
        "b593d3a7cfe5f3db80ab80b3ba433b40812270dd",
    "crates/galactic_sim/src/lib.rs":
        "b589e5238f5910fcbe980ce3f9a76936770f1a51",
    "crates/galactic_sim/src/state.rs":
        "1ffaae2d9ffda4a3fae1e91d750d36aa2a803daa",
    "crates/galactic_sim/src/simulation.rs":
        "76d24c4fdc896af60911928bb60ae43d94739f9e",
    "crates/galactic_sim/src/command.rs":
        "878224aff75a562c9bda9aa7343c1126ef45fe54",
    "crates/galactic_sim/src/event.rs":
        "8a1c28b5e8411c50667fe1908419f15397da9113",
    "crates/galactic_persistence/src/lib.rs":
        "44f89b554df306e68071fe104aa817583e733b22",
    "docs/mvp_architecture.md":
        "9a5576b1aad0e91b73ac49111a35ab9377788134",
}

RESEARCH_RS = '// MVP-016: global research queue and minimal technology tree.\nuse std::collections::{BTreeSet, VecDeque};\n\nuse crate::{\n    BuildingEffect, BuildingKind, GameState, StrategicDuration,\n    STRATEGIC_TICKS_PER_SECOND, default_building_catalog,\n};\n\npub const RESEARCH_CATALOG_VERSION: u32 = 1;\npub const RESEARCH_CATALOG_FINGERPRINT: u64 =\n    0x2f55_29d1_26a4_7d83;\npub const MAX_RESEARCH_QUEUE: usize = 6;\n\n#[derive(\n    Debug,\n    Clone,\n    Copy,\n    PartialEq,\n    Eq,\n    PartialOrd,\n    Ord,\n    Hash,\n)]\npub enum TechnologyId {\n    SpatialDetection,\n    Propulsion,\n    CargoCapacity,\n    RemoteExtraction,\n    PlanetaryAnalysis,\n    Colonization,\n}\n\nimpl TechnologyId {\n    pub const ALL: [Self; 6] = [\n        Self::SpatialDetection,\n        Self::Propulsion,\n        Self::CargoCapacity,\n        Self::RemoteExtraction,\n        Self::PlanetaryAnalysis,\n        Self::Colonization,\n    ];\n}\n\n#[derive(\n    Debug,\n    Clone,\n    Copy,\n    PartialEq,\n    Eq,\n    PartialOrd,\n    Ord,\n    Hash,\n)]\npub enum TechnologyUnlock {\n    DetectUnknownSystems,\n    InterstellarTravel,\n    ExpandedCargo,\n    RemoteExtraction,\n    AnalyzePlanets,\n    FoundColonies,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct TechnologyDefinition {\n    pub id: TechnologyId,\n    pub name: &\'static str,\n    pub description: &\'static str,\n    pub required_milli_points: u64,\n    pub prerequisites: &\'static [TechnologyId],\n    pub unlock: TechnologyUnlock,\n    pub unlock_label: &\'static str,\n}\n\nconst NO_PREREQUISITES: &[TechnologyId] = &[];\nconst PROPULSION_PREREQUISITES: &[TechnologyId] =\n    &[TechnologyId::SpatialDetection];\nconst CARGO_PREREQUISITES: &[TechnologyId] =\n    &[TechnologyId::Propulsion];\nconst REMOTE_EXTRACTION_PREREQUISITES: &[TechnologyId] =\n    &[TechnologyId::CargoCapacity];\nconst PLANETARY_ANALYSIS_PREREQUISITES: &[TechnologyId] =\n    &[TechnologyId::SpatialDetection];\nconst COLONIZATION_PREREQUISITES: &[TechnologyId] = &[\n    TechnologyId::CargoCapacity,\n    TechnologyId::PlanetaryAnalysis,\n];\n\npub const TECHNOLOGY_CATALOG: [TechnologyDefinition; 6] = [\n    TechnologyDefinition {\n        id: TechnologyId::SpatialDetection,\n        name: "Détection spatiale",\n        description:\n            "Améliore la détection des systèmes et prépare les missions de reconnaissance.",\n        required_milli_points: 15_000,\n        prerequisites: NO_PREREQUISITES,\n        unlock: TechnologyUnlock::DetectUnknownSystems,\n        unlock_label:\n            "Détection des systèmes inconnus",\n    },\n    TechnologyDefinition {\n        id: TechnologyId::Propulsion,\n        name: "Propulsion",\n        description:\n            "Débloque les moteurs nécessaires aux déplacements interstellaires.",\n        required_milli_points: 20_000,\n        prerequisites: PROPULSION_PREREQUISITES,\n        unlock: TechnologyUnlock::InterstellarTravel,\n        unlock_label: "Voyage interstellaire",\n    },\n    TechnologyDefinition {\n        id: TechnologyId::CargoCapacity,\n        name: "Capacité cargo",\n        description:\n            "Augmente la capacité logistique des futurs vaisseaux et transports.",\n        required_milli_points: 25_000,\n        prerequisites: CARGO_PREREQUISITES,\n        unlock: TechnologyUnlock::ExpandedCargo,\n        unlock_label: "Soutes cargo améliorées",\n    },\n    TechnologyDefinition {\n        id: TechnologyId::RemoteExtraction,\n        name: "Extraction distante",\n        description:\n            "Autorise l\'exploitation de ressources sans colonie permanente.",\n        required_milli_points: 30_000,\n        prerequisites: REMOTE_EXTRACTION_PREREQUISITES,\n        unlock: TechnologyUnlock::RemoteExtraction,\n        unlock_label: "Missions d\'extraction distante",\n    },\n    TechnologyDefinition {\n        id: TechnologyId::PlanetaryAnalysis,\n        name: "Analyse planétaire",\n        description:\n            "Révèle les caractéristiques nécessaires à l\'évaluation des mondes.",\n        required_milli_points: 25_000,\n        prerequisites: PLANETARY_ANALYSIS_PREREQUISITES,\n        unlock: TechnologyUnlock::AnalyzePlanets,\n        unlock_label: "Analyse exacte des planètes",\n    },\n    TechnologyDefinition {\n        id: TechnologyId::Colonization,\n        name: "Colonisation",\n        description:\n            "Débloque la fondation de nouvelles colonies sur les mondes compatibles.",\n        required_milli_points: 45_000,\n        prerequisites: COLONIZATION_PREREQUISITES,\n        unlock: TechnologyUnlock::FoundColonies,\n        unlock_label: "Fondation de colonies",\n    },\n];\n\npub fn technology_definition(\n    technology: TechnologyId,\n) -> &\'static TechnologyDefinition {\n    TECHNOLOGY_CATALOG\n        .iter()\n        .find(|definition| definition.id == technology)\n        .expect("the static technology catalog is complete")\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResearchProject {\n    pub technology: TechnologyId,\n    pub required_milli_points: u64,\n    pub accumulated_milli_points: u64,\n}\n\nimpl ResearchProject {\n    pub const fn remaining_milli_points(self) -> u64 {\n        self.required_milli_points\n            .saturating_sub(self.accumulated_milli_points)\n    }\n}\n\n#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct ResearchState {\n    completed: BTreeSet<TechnologyId>,\n    queue: VecDeque<ResearchProject>,\n}\n\nimpl ResearchState {\n    pub fn completed(\n        &self,\n    ) -> impl Iterator<Item = TechnologyId> + \'_ {\n        self.completed.iter().copied()\n    }\n\n    pub fn completed_count(&self) -> usize {\n        self.completed.len()\n    }\n\n    pub fn has_completed(\n        &self,\n        technology: TechnologyId,\n    ) -> bool {\n        self.completed.contains(&technology)\n    }\n\n    pub fn has_unlock(\n        &self,\n        unlock: TechnologyUnlock,\n    ) -> bool {\n        self.completed().any(|technology| {\n            technology_definition(technology).unlock == unlock\n        })\n    }\n\n    pub fn queue(\n        &self,\n    ) -> impl Iterator<Item = &ResearchProject> {\n        self.queue.iter()\n    }\n\n    pub fn active(&self) -> Option<&ResearchProject> {\n        self.queue.front()\n    }\n\n    pub fn is_queued(\n        &self,\n        technology: TechnologyId,\n    ) -> bool {\n        self.queue\n            .iter()\n            .any(|project| project.technology == technology)\n    }\n\n    pub fn queue_len(&self) -> usize {\n        self.queue.len()\n    }\n\n    pub fn is_queue_empty(&self) -> bool {\n        self.queue.is_empty()\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResearchQuote {\n    pub technology: TechnologyId,\n    pub required_milli_points: u64,\n    pub output_milli_points_per_tick: u64,\n    pub estimated_ticks: u64,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResearchQueued {\n    pub project: ResearchProject,\n    pub queue_length: usize,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResearchCompleted {\n    pub technology: TechnologyId,\n    pub unlock: TechnologyUnlock,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResearchRejected {\n    pub technology: TechnologyId,\n    pub error: ResearchError,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ResearchError {\n    NoResearchCapacity,\n    AlreadyCompleted(TechnologyId),\n    AlreadyQueued(TechnologyId),\n    QueueFull {\n        maximum: usize,\n    },\n    MissingPrerequisite {\n        technology: TechnologyId,\n        prerequisite: TechnologyId,\n    },\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ResearchStateError {\n    TooManyProjects {\n        found: usize,\n        maximum: usize,\n    },\n    CompletedMissingPrerequisite {\n        technology: TechnologyId,\n        prerequisite: TechnologyId,\n    },\n    CompletedAndQueued(TechnologyId),\n    DuplicateQueued(TechnologyId),\n    QueuedMissingPrerequisite {\n        technology: TechnologyId,\n        prerequisite: TechnologyId,\n    },\n    InvalidRequiredPoints {\n        technology: TechnologyId,\n        expected: u64,\n        found: u64,\n    },\n    InvalidProgress {\n        technology: TechnologyId,\n        accumulated: u64,\n        required: u64,\n    },\n}\n\npub fn research_quote(\n    state: &GameState,\n    technology: TechnologyId,\n) -> Result<ResearchQuote, ResearchError> {\n    if state.research.has_completed(technology) {\n        return Err(ResearchError::AlreadyCompleted(\n            technology,\n        ));\n    }\n    if state.research.is_queued(technology) {\n        return Err(ResearchError::AlreadyQueued(technology));\n    }\n    if state.research.queue_len() >= MAX_RESEARCH_QUEUE {\n        return Err(ResearchError::QueueFull {\n            maximum: MAX_RESEARCH_QUEUE,\n        });\n    }\n\n    let output = research_output_milli_points_per_tick(state);\n    if output == 0 {\n        return Err(ResearchError::NoResearchCapacity);\n    }\n\n    let projected = projected_technologies(&state.research);\n    let definition = technology_definition(technology);\n    for prerequisite in definition.prerequisites {\n        if !projected.contains(prerequisite) {\n            return Err(ResearchError::MissingPrerequisite {\n                technology,\n                prerequisite: *prerequisite,\n            });\n        }\n    }\n\n    Ok(ResearchQuote {\n        technology,\n        required_milli_points:\n            definition.required_milli_points,\n        output_milli_points_per_tick: output,\n        estimated_ticks: definition\n            .required_milli_points\n            .div_ceil(output),\n    })\n}\n\npub fn enqueue_research(\n    state: &mut GameState,\n    technology: TechnologyId,\n) -> Result<ResearchQueued, ResearchError> {\n    let quote = research_quote(state, technology)?;\n    let project = ResearchProject {\n        technology,\n        required_milli_points:\n            quote.required_milli_points,\n        accumulated_milli_points: 0,\n    };\n    state.research.queue.push_back(project);\n\n    Ok(ResearchQueued {\n        project,\n        queue_length: state.research.queue_len(),\n    })\n}\n\npub fn advance_research(\n    state: &mut GameState,\n    ticks: StrategicDuration,\n) -> Vec<ResearchCompleted> {\n    let output = research_output_milli_points_per_tick(state);\n    let mut budget = output.saturating_mul(ticks.ticks());\n    if budget == 0 {\n        return Vec::new();\n    }\n\n    let mut completed = Vec::new();\n    while budget > 0 {\n        let finished = {\n            let Some(active) =\n                state.research.queue.front_mut()\n            else {\n                break;\n            };\n            let spent =\n                active.remaining_milli_points().min(budget);\n            active.accumulated_milli_points = active\n                .accumulated_milli_points\n                .saturating_add(spent);\n            budget -= spent;\n\n            (active.accumulated_milli_points\n                >= active.required_milli_points)\n                .then_some(*active)\n        };\n\n        let Some(project) = finished else {\n            continue;\n        };\n        state\n            .research\n            .queue\n            .pop_front()\n            .expect("the completed project is active");\n        state\n            .research\n            .completed\n            .insert(project.technology);\n\n        let definition =\n            technology_definition(project.technology);\n        completed.push(ResearchCompleted {\n            technology: project.technology,\n            unlock: definition.unlock,\n        });\n    }\n\n    completed\n}\n\npub fn research_output_milli_points_per_tick(\n    state: &GameState,\n) -> u64 {\n    let definition = default_building_catalog()\n        .definition(BuildingKind::ResearchLab);\n    let per_level = match definition.effect {\n        BuildingEffect::ResearchPoints {\n            milli_per_tick_per_level,\n        } => milli_per_tick_per_level,\n        _ => 0,\n    };\n\n    state\n        .colonies\n        .iter()\n        .filter(|colony| {\n            colony.faction == state.player_faction\n        })\n        .map(|colony| {\n            per_level.saturating_mul(u64::from(\n                colony\n                    .buildings\n                    .level(BuildingKind::ResearchLab),\n            ))\n        })\n        .fold(0_u64, u64::saturating_add)\n}\n\npub fn research_lab_level_total(\n    state: &GameState,\n) -> u32 {\n    state\n        .colonies\n        .iter()\n        .filter(|colony| {\n            colony.faction == state.player_faction\n        })\n        .map(|colony| {\n            u32::from(\n                colony\n                    .buildings\n                    .level(BuildingKind::ResearchLab),\n            )\n        })\n        .sum()\n}\n\npub fn research_output_points_per_second(\n    state: &GameState,\n) -> f64 {\n    research_output_milli_points_per_tick(state) as f64\n        * f64::from(STRATEGIC_TICKS_PER_SECOND)\n        / 1_000.0\n}\n\npub fn research_progress_ratio(\n    project: ResearchProject,\n) -> f32 {\n    if project.required_milli_points == 0 {\n        return 1.0;\n    }\n    (project.accumulated_milli_points as f64\n        / project.required_milli_points as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\npub fn validate_research_state(\n    state: &GameState,\n) -> Result<(), ResearchStateError> {\n    if state.research.queue_len() > MAX_RESEARCH_QUEUE {\n        return Err(ResearchStateError::TooManyProjects {\n            found: state.research.queue_len(),\n            maximum: MAX_RESEARCH_QUEUE,\n        });\n    }\n\n    for technology in state.research.completed() {\n        let definition = technology_definition(technology);\n        for prerequisite in definition.prerequisites {\n            if !state\n                .research\n                .has_completed(*prerequisite)\n            {\n                return Err(\n                    ResearchStateError::\n                        CompletedMissingPrerequisite {\n                            technology,\n                            prerequisite: *prerequisite,\n                        },\n                );\n            }\n        }\n    }\n\n    let mut projected = state.research.completed.clone();\n    let mut queued = BTreeSet::new();\n    for project in state.research.queue() {\n        if state\n            .research\n            .has_completed(project.technology)\n        {\n            return Err(\n                ResearchStateError::CompletedAndQueued(\n                    project.technology,\n                ),\n            );\n        }\n        if !queued.insert(project.technology) {\n            return Err(\n                ResearchStateError::DuplicateQueued(\n                    project.technology,\n                ),\n            );\n        }\n\n        let definition =\n            technology_definition(project.technology);\n        if project.required_milli_points\n            != definition.required_milli_points\n        {\n            return Err(\n                ResearchStateError::InvalidRequiredPoints {\n                    technology: project.technology,\n                    expected:\n                        definition.required_milli_points,\n                    found: project.required_milli_points,\n                },\n            );\n        }\n        if project.required_milli_points == 0\n            || project.accumulated_milli_points\n                >= project.required_milli_points\n        {\n            return Err(\n                ResearchStateError::InvalidProgress {\n                    technology: project.technology,\n                    accumulated:\n                        project.accumulated_milli_points,\n                    required:\n                        project.required_milli_points,\n                },\n            );\n        }\n\n        for prerequisite in definition.prerequisites {\n            if !projected.contains(prerequisite) {\n                return Err(\n                    ResearchStateError::\n                        QueuedMissingPrerequisite {\n                            technology: project.technology,\n                            prerequisite: *prerequisite,\n                        },\n                );\n            }\n        }\n        projected.insert(project.technology);\n    }\n\n    Ok(())\n}\n\nfn projected_technologies(\n    research: &ResearchState,\n) -> BTreeSet<TechnologyId> {\n    let mut projected = research.completed.clone();\n    projected.extend(\n        research.queue().map(|project| project.technology),\n    );\n    projected\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::UniverseConfig;\n\n    use crate::{Simulation, default_building_catalog};\n\n    use super::*;\n\n    fn simulation_with_lab() -> Simulation {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony.buildings.research_lab = 1;\n        colony.energy = default_building_catalog()\n            .energy_grid_for_levels(colony.buildings);\n        simulation\n    }\n\n    #[test]\n    fn catalog_contains_the_six_expected_technologies() {\n        assert_eq!(\n            TECHNOLOGY_CATALOG\n                .iter()\n                .map(|definition| definition.id)\n                .collect::<Vec<_>>(),\n            TechnologyId::ALL.to_vec(),\n        );\n        assert!(\n            TECHNOLOGY_CATALOG\n                .iter()\n                .all(|definition| {\n                    definition.required_milli_points > 0\n                        && !definition.name.is_empty()\n                        && !definition.unlock_label.is_empty()\n                })\n        );\n    }\n\n    #[test]\n    fn research_requires_an_active_laboratory() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n\n        assert_eq!(\n            research_quote(\n                simulation.state(),\n                TechnologyId::SpatialDetection,\n            ),\n            Err(ResearchError::NoResearchCapacity),\n        );\n    }\n\n    #[test]\n    fn all_technologies_can_be_queued_in_valid_order() {\n        let mut simulation = simulation_with_lab();\n        for technology in TechnologyId::ALL {\n            enqueue_research(\n                simulation.state_mut(),\n                technology,\n            )\n            .expect("the canonical order is valid");\n        }\n\n        assert_eq!(\n            simulation.state().research.queue_len(),\n            TechnologyId::ALL.len(),\n        );\n        assert!(\n            validate_research_state(simulation.state())\n                .is_ok()\n        );\n    }\n\n    #[test]\n    fn completed_technology_cannot_be_relaunched() {\n        let mut simulation = simulation_with_lab();\n        enqueue_research(\n            simulation.state_mut(),\n            TechnologyId::SpatialDetection,\n        )\n        .expect("root technology is valid");\n        advance_research(\n            simulation.state_mut(),\n            StrategicDuration::from_ticks(1_000),\n        );\n\n        assert_eq!(\n            research_quote(\n                simulation.state(),\n                TechnologyId::SpatialDetection,\n            ),\n            Err(ResearchError::AlreadyCompleted(\n                TechnologyId::SpatialDetection,\n            )),\n        );\n    }\n\n    #[test]\n    fn large_tick_batches_complete_multiple_projects() {\n        let mut simulation = simulation_with_lab();\n        for technology in TechnologyId::ALL {\n            enqueue_research(\n                simulation.state_mut(),\n                technology,\n            )\n            .expect("the canonical order is valid");\n        }\n\n        let completed = advance_research(\n            simulation.state_mut(),\n            StrategicDuration::from_ticks(10_000),\n        );\n\n        assert_eq!(\n            completed.len(),\n            TechnologyId::ALL.len(),\n        );\n        assert!(\n            TechnologyUnlock::ALL_FOR_TESTS\n                .iter()\n                .all(|unlock| {\n                    simulation\n                        .state()\n                        .research\n                        .has_unlock(*unlock)\n                })\n        );\n    }\n\n    #[test]\n    fn progress_ratio_is_clamped() {\n        let project = ResearchProject {\n            technology: TechnologyId::SpatialDetection,\n            required_milli_points: 100,\n            accumulated_milli_points: 50,\n        };\n        assert_eq!(research_progress_ratio(project), 0.5);\n    }\n}\n\n#[cfg(test)]\nimpl TechnologyUnlock {\n    const ALL_FOR_TESTS: [Self; 6] = [\n        Self::DetectUnknownSystems,\n        Self::InterstellarTravel,\n        Self::ExpandedCargo,\n        Self::RemoteExtraction,\n        Self::AnalyzePlanets,\n        Self::FoundColonies,\n    ];\n}\n'
RESEARCH_UI_RS = '// MVP-016: dedicated research screen kept outside the main client module.\nuse bevy::prelude::*;\nuse galactic_sim::{\n    GameCommand, GameEvent, ResearchError,\n    ResearchQuote, StrategicDuration, TechnologyId,\n    MAX_RESEARCH_QUEUE, STRATEGIC_TICKS_PER_SECOND,\n    research_lab_level_total,\n    research_output_milli_points_per_tick,\n    research_output_points_per_second,\n    research_progress_ratio, research_quote,\n    technology_definition,\n};\n\nuse super::{\n    ColonyManagementState, PresentationUpdateSet,\n    SimulationResource, UiPointerBlocker,\n    action_button_color, action_button_outline,\n    apply_simulation_command, collect_presentation_events,\n    format_strategic_duration, panel_background,\n    panel_outline, ui_text_font,\n};\n\nconst RESEARCH_Z_INDEX: i32 = 110;\n\npub(crate) struct ResearchUiPlugin;\n\nimpl Plugin for ResearchUiPlugin {\n    fn build(&self, app: &mut App) {\n        app.init_resource::<ResearchUiState>()\n            .add_systems(Startup, spawn_research_screen)\n            .add_systems(\n                Update,\n                capture_research_feedback\n                    .before(collect_presentation_events)\n                    .in_set(PresentationUpdateSet::View),\n            )\n            .add_systems(\n                Update,\n                (\n                    handle_research_shortcuts,\n                    handle_research_buttons,\n                )\n                    .chain()\n                    .in_set(\n                        PresentationUpdateSet::Interaction,\n                    ),\n            )\n            .add_systems(\n                Update,\n                (\n                    update_research_visibility,\n                    update_research_summary,\n                    update_research_technology_buttons,\n                    update_research_detail,\n                    update_research_queue,\n                )\n                    .chain()\n                    .in_set(\n                        PresentationUpdateSet::Management,\n                    ),\n            );\n    }\n}\n\n#[derive(Resource)]\npub(crate) struct ResearchUiState {\n    pub(crate) open: bool,\n    selected: TechnologyId,\n    feedback: String,\n}\n\nimpl Default for ResearchUiState {\n    fn default() -> Self {\n        Self {\n            open: false,\n            selected: TechnologyId::SpatialDetection,\n            feedback: String::new(),\n        }\n    }\n}\n\n#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]\nenum ResearchButtonAction {\n    Toggle,\n    Close,\n    Select(TechnologyId),\n    QueueSelected,\n}\n\n#[derive(Component)]\nstruct ResearchRoot;\n\n#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]\nenum ResearchTextRole {\n    Toggle,\n    Title,\n    Summary,\n    Detail,\n    Queue,\n    QueueButton,\n    Feedback,\n}\n\n#[derive(Component)]\nstruct TechnologyButton {\n    technology: TechnologyId,\n}\n\n#[derive(Component)]\nstruct TechnologyButtonText {\n    technology: TechnologyId,\n}\n\n#[derive(Component)]\nstruct QueueResearchButton;\n\n#[derive(Component)]\nstruct ResearchProgressFill;\n\npub(crate) fn spawn_research_toggle(\n    parent: &mut ChildSpawnerCommands,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Percent(100.0),\n                min_height: Val::Px(36.0),\n                padding: UiRect::axes(\n                    Val::Px(10.0),\n                    Val::Px(7.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.11, 0.13, 0.24, 0.96,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.54, 0.58, 0.96, 0.58),\n            ),\n            ResearchButtonAction::Toggle,\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new("Recherche  [T]"),\n                ui_text_font(12.0),\n                TextColor(Color::srgb(\n                    0.84, 0.86, 1.0,\n                )),\n                ResearchTextRole::Toggle,\n            ));\n        });\n}\n\nfn spawn_research_screen(mut commands: Commands) {\n    commands\n        .spawn((\n            Node {\n                position_type: PositionType::Absolute,\n                left: Val::Px(14.0),\n                right: Val::Px(14.0),\n                top: Val::Px(72.0),\n                bottom: Val::Px(14.0),\n                padding: UiRect::all(Val::Px(12.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(8.0),\n                ),\n                flex_direction: FlexDirection::Column,\n                row_gap: Val::Px(9.0),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.008, 0.012, 0.026, 0.995,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.48, 0.54, 0.94, 0.72),\n            ),\n            Visibility::Hidden,\n            GlobalZIndex(RESEARCH_Z_INDEX),\n            Interaction::None,\n            UiPointerBlocker,\n            ResearchRoot,\n        ))\n        .with_children(|root| {\n            spawn_research_header(root);\n            root.spawn((\n                Text::new(""),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.74, 0.82, 0.96,\n                )),\n                Node {\n                    min_height: Val::Px(28.0),\n                    ..default()\n                },\n                ResearchTextRole::Summary,\n            ));\n            spawn_research_main_row(root);\n            root.spawn((\n                Text::new(""),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.94, 0.72, 0.44,\n                )),\n                Node {\n                    min_height: Val::Px(18.0),\n                    ..default()\n                },\n                ResearchTextRole::Feedback,\n            ));\n        });\n}\n\nfn spawn_research_header(\n    root: &mut ChildSpawnerCommands,\n) {\n    root.spawn((\n        Node {\n            width: Val::Percent(100.0),\n            min_height: Val::Px(42.0),\n            flex_direction: FlexDirection::Row,\n            align_items: AlignItems::Center,\n            column_gap: Val::Px(8.0),\n            ..default()\n        },\n    ))\n    .with_children(|header| {\n        header.spawn((\n            Text::new("RECHERCHE"),\n            ui_text_font(18.0),\n            TextColor(Color::srgb(\n                0.86, 0.88, 1.0,\n            )),\n            Node {\n                flex_grow: 1.0,\n                ..default()\n            },\n            ResearchTextRole::Title,\n        ));\n        spawn_research_small_button(\n            header,\n            "Fermer  [T / Échap]",\n            ResearchButtonAction::Close,\n            160.0,\n        );\n    });\n}\n\nfn spawn_research_small_button(\n    parent: &mut ChildSpawnerCommands,\n    label: &str,\n    action: ResearchButtonAction,\n    width: f32,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Px(width),\n                min_height: Val::Px(32.0),\n                padding: UiRect::axes(\n                    Val::Px(8.0),\n                    Val::Px(5.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                justify_content: JustifyContent::Center,\n                align_items: AlignItems::Center,\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.07, 0.08, 0.16, 0.98,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.46, 0.52, 0.84, 0.58),\n            ),\n            action,\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new(label),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.84, 0.88, 0.98,\n                )),\n            ));\n        });\n}\n\nfn spawn_research_main_row(\n    root: &mut ChildSpawnerCommands,\n) {\n    root.spawn((\n        Node {\n            width: Val::Percent(100.0),\n            flex_grow: 1.0,\n            min_height: Val::Px(450.0),\n            flex_direction: FlexDirection::Row,\n            column_gap: Val::Px(9.0),\n            ..default()\n        },\n    ))\n    .with_children(|row| {\n        spawn_technology_list(row);\n        spawn_technology_detail(row);\n        spawn_research_queue(row);\n    });\n}\n\nfn spawn_technology_list(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            width: Val::Px(330.0),\n            padding: UiRect::all(Val::Px(9.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(7.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|list| {\n        list.spawn((\n            Text::new("ARBRE TECHNOLOGIQUE"),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.76, 0.82, 1.0,\n            )),\n        ));\n        for technology in TechnologyId::ALL {\n            spawn_technology_button(list, technology);\n        }\n    });\n}\n\nfn spawn_technology_button(\n    parent: &mut ChildSpawnerCommands,\n    technology: TechnologyId,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Percent(100.0),\n                min_height: Val::Px(52.0),\n                padding: UiRect::axes(\n                    Val::Px(9.0),\n                    Val::Px(7.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.04, 0.05, 0.10, 0.98,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.32, 0.36, 0.62, 0.46),\n            ),\n            ResearchButtonAction::Select(technology),\n            TechnologyButton { technology },\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new(""),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.80, 0.84, 0.94,\n                )),\n                TechnologyButtonText { technology },\n            ));\n        });\n}\n\nfn spawn_technology_detail(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            flex_grow: 1.0,\n            flex_basis: Val::Px(0.0),\n            padding: UiRect::all(Val::Px(12.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(10.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|detail| {\n        detail.spawn((\n            Text::new("Sélectionne une technologie."),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.84, 0.88, 0.96,\n            )),\n            Node {\n                flex_grow: 1.0,\n                ..default()\n            },\n            ResearchTextRole::Detail,\n        ));\n        detail\n            .spawn((\n                Button,\n                Node {\n                    width: Val::Percent(100.0),\n                    min_height: Val::Px(42.0),\n                    padding: UiRect::axes(\n                        Val::Px(12.0),\n                        Val::Px(8.0),\n                    ),\n                    border: UiRect::all(Val::Px(1.0)),\n                    border_radius: BorderRadius::all(\n                        Val::Px(6.0),\n                    ),\n                    justify_content: JustifyContent::Center,\n                    align_items: AlignItems::Center,\n                    ..default()\n                },\n                BackgroundColor(Color::srgba(\n                    0.12, 0.18, 0.42, 0.98,\n                )),\n                Outline::new(\n                    Val::Px(1.0),\n                    Val::ZERO,\n                    Color::srgba(\n                        0.52, 0.62, 1.0, 0.76,\n                    ),\n                ),\n                ResearchButtonAction::QueueSelected,\n                QueueResearchButton,\n                UiPointerBlocker,\n            ))\n            .with_children(|button| {\n                button.spawn((\n                    Text::new("LANCER LA RECHERCHE"),\n                    ui_text_font(12.0),\n                    TextColor(Color::srgb(\n                        0.88, 0.92, 1.0,\n                    )),\n                    ResearchTextRole::QueueButton,\n                ));\n            });\n    });\n}\n\nfn spawn_research_queue(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            width: Val::Px(320.0),\n            padding: UiRect::all(Val::Px(10.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(8.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|queue| {\n        queue.spawn((\n            Text::new("FILE DE RECHERCHE GLOBALE"),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.76, 0.82, 1.0,\n            )),\n        ));\n        queue\n            .spawn((\n                Node {\n                    width: Val::Percent(100.0),\n                    height: Val::Px(8.0),\n                    border_radius: BorderRadius::all(\n                        Val::Px(4.0),\n                    ),\n                    ..default()\n                },\n                BackgroundColor(Color::srgba(\n                    0.08, 0.09, 0.16, 0.96,\n                )),\n            ))\n            .with_children(|gauge| {\n                gauge.spawn((\n                    Node {\n                        width: Val::Percent(0.0),\n                        height: Val::Percent(100.0),\n                        border_radius: BorderRadius::all(\n                            Val::Px(4.0),\n                        ),\n                        ..default()\n                    },\n                    BackgroundColor(Color::srgb(\n                        0.52, 0.62, 1.0,\n                    )),\n                    ResearchProgressFill,\n                ));\n            });\n        queue.spawn((\n            Text::new("File vide."),\n            ui_text_font(11.0),\n            TextColor(Color::srgb(\n                0.78, 0.82, 0.92,\n            )),\n            ResearchTextRole::Queue,\n        ));\n    });\n}\n\nfn handle_research_shortcuts(\n    keyboard: Res<ButtonInput<KeyCode>>,\n    mut ui: ResMut<ResearchUiState>,\n    mut management: ResMut<ColonyManagementState>,\n) {\n    if keyboard.just_pressed(KeyCode::KeyT) {\n        ui.open = !ui.open;\n        ui.feedback.clear();\n        if ui.open {\n            management.open = false;\n        }\n        return;\n    }\n\n    if ui.open\n        && keyboard.just_pressed(KeyCode::Escape)\n    {\n        ui.open = false;\n    }\n}\n\nfn handle_research_buttons(\n    mut simulation: ResMut<SimulationResource>,\n    mut ui: ResMut<ResearchUiState>,\n    mut management: ResMut<ColonyManagementState>,\n    interactions: Query<\n        (&Interaction, &ResearchButtonAction),\n        (Changed<Interaction>, With<Button>),\n    >,\n) {\n    for (interaction, action) in &interactions {\n        if *interaction != Interaction::Pressed {\n            continue;\n        }\n\n        match *action {\n            ResearchButtonAction::Toggle => {\n                ui.open = !ui.open;\n                ui.feedback.clear();\n                if ui.open {\n                    management.open = false;\n                }\n            }\n            ResearchButtonAction::Close => {\n                ui.open = false;\n            }\n            ResearchButtonAction::Select(technology) => {\n                ui.selected = technology;\n                ui.feedback.clear();\n            }\n            ResearchButtonAction::QueueSelected => {\n                queue_selected_research(\n                    &mut simulation,\n                    &mut ui,\n                );\n            }\n        }\n    }\n}\n\nfn capture_research_feedback(\n    simulation: Res<SimulationResource>,\n    mut ui: ResMut<ResearchUiState>,\n) {\n    for event in &simulation.pending_events {\n        match *event {\n            GameEvent::ResearchQueued(queued) => {\n                let definition =\n                    technology_definition(\n                        queued.project.technology,\n                    );\n                ui.feedback = format!(\n                    "{} ajouté à la file.",\n                    definition.name,\n                );\n            }\n            GameEvent::ResearchCompleted(completed) => {\n                let definition =\n                    technology_definition(\n                        completed.technology,\n                    );\n                ui.feedback = format!(\n                    "{} terminé — {} débloqué.",\n                    definition.name,\n                    definition.unlock_label,\n                );\n            }\n            GameEvent::ResearchRejected(rejected) => {\n                ui.feedback = format!(\n                    "Recherche refusée : {}",\n                    research_error_text(rejected.error),\n                );\n            }\n            _ => {}\n        }\n    }\n}\n\nfn update_research_visibility(\n    ui: Res<ResearchUiState>,\n    mut roots: Query<\n        &mut Visibility,\n        With<ResearchRoot>,\n    >,\n    mut texts: Query<(\n        &ResearchTextRole,\n        &mut Text,\n    )>,\n) {\n    for mut visibility in &mut roots {\n        *visibility = if ui.open {\n            Visibility::Visible\n        } else {\n            Visibility::Hidden\n        };\n    }\n    for (role, mut text) in &mut texts {\n        if *role == ResearchTextRole::Toggle {\n            text.0 = if ui.open {\n                "Fermer recherche".to_string()\n            } else {\n                "Recherche  [T]".to_string()\n            };\n        }\n    }\n}\n\nfn update_research_summary(\n    simulation: Res<SimulationResource>,\n    ui: Res<ResearchUiState>,\n    mut texts: Query<(\n        &ResearchTextRole,\n        &mut Text,\n    )>,\n) {\n    if !ui.open {\n        return;\n    }\n    let state = simulation.simulation().state();\n    let labs = research_lab_level_total(state);\n    let output =\n        research_output_points_per_second(state);\n    let completed = state.research.completed_count();\n\n    for (role, mut text) in &mut texts {\n        match role {\n            ResearchTextRole::Title => {\n                text.0 = format!(\n                    "RECHERCHE — {} / {} technologie(s)",\n                    completed,\n                    TechnologyId::ALL.len(),\n                );\n            }\n            ResearchTextRole::Summary => {\n                text.0 = format!(\n                    "Laboratoires cumulés : niveau {}  •  production scientifique : {:.2} point(s)/s  •  file : {}/{}",\n                    labs,\n                    output,\n                    state.research.queue_len(),\n                    MAX_RESEARCH_QUEUE,\n                );\n            }\n            ResearchTextRole::Feedback => {\n                text.0 = ui.feedback.clone();\n            }\n            _ => {}\n        }\n    }\n}\n\nfn update_research_technology_buttons(\n    simulation: Res<SimulationResource>,\n    ui: Res<ResearchUiState>,\n    mut buttons: Query<(\n        &TechnologyButton,\n        &Interaction,\n        &mut BackgroundColor,\n        &mut Outline,\n    )>,\n    mut labels: Query<(\n        &TechnologyButtonText,\n        &mut Text,\n        &mut TextColor,\n    )>,\n) {\n    if !ui.open {\n        return;\n    }\n    let state = simulation.simulation().state();\n\n    for (button, interaction, mut background, mut outline)\n        in &mut buttons\n    {\n        let selected = button.technology == ui.selected;\n        background.0 = technology_button_color(\n            selected,\n            interaction,\n        );\n        outline.color =\n            technology_button_outline(selected);\n    }\n\n    for (label, mut text, mut color) in &mut labels {\n        let definition =\n            technology_definition(label.technology);\n        let status =\n            technology_status_label(state, label.technology);\n        text.0 = format!(\n            "{}\\n{}",\n            definition.name,\n            status,\n        );\n        color.0 = if state\n            .research\n            .has_completed(label.technology)\n        {\n            Color::srgb(0.54, 0.94, 0.74)\n        } else if label.technology == ui.selected {\n            Color::srgb(0.90, 0.92, 1.0)\n        } else {\n            Color::srgb(0.78, 0.82, 0.92)\n        };\n    }\n}\n\nfn update_research_detail(\n    simulation: Res<SimulationResource>,\n    ui: Res<ResearchUiState>,\n    mut texts: Query<(\n        &ResearchTextRole,\n        &mut Text,\n        &mut TextColor,\n    )>,\n    mut button: Query<\n        (\n            &Interaction,\n            &mut BackgroundColor,\n            &mut Outline,\n        ),\n        With<QueueResearchButton>,\n    >,\n) {\n    if !ui.open {\n        return;\n    }\n    let state = simulation.simulation().state();\n    let quote = research_quote(state, ui.selected);\n    let available = quote.is_ok();\n    let detail =\n        research_detail_text(state, ui.selected, quote);\n    let button_label = match quote {\n        Ok(_) => "LANCER LA RECHERCHE".to_string(),\n        Err(error) => research_error_text(error),\n    };\n\n    for (role, mut text, mut color) in &mut texts {\n        match role {\n            ResearchTextRole::Detail => {\n                text.0 = detail.clone();\n                color.0 =\n                    Color::srgb(0.84, 0.88, 0.96);\n            }\n            ResearchTextRole::QueueButton => {\n                text.0 = button_label.clone();\n                color.0 = if available {\n                    Color::srgb(0.88, 0.92, 1.0)\n                } else {\n                    Color::srgb(0.62, 0.64, 0.70)\n                };\n            }\n            _ => {}\n        }\n    }\n\n    for (interaction, mut background, mut outline)\n        in &mut button\n    {\n        background.0 = action_button_color(\n            available,\n            false,\n            interaction,\n        );\n        outline.color = action_button_outline(\n            available,\n            false,\n            interaction,\n        );\n    }\n}\n\nfn update_research_queue(\n    simulation: Res<SimulationResource>,\n    ui: Res<ResearchUiState>,\n    mut texts: Query<(\n        &ResearchTextRole,\n        &mut Text,\n    )>,\n    mut progress: Query<\n        &mut Node,\n        With<ResearchProgressFill>,\n    >,\n) {\n    if !ui.open {\n        return;\n    }\n    let state = simulation.simulation().state();\n    let label = research_queue_text(state);\n\n    for (role, mut text) in &mut texts {\n        if *role == ResearchTextRole::Queue {\n            text.0 = label.clone();\n        }\n    }\n\n    let ratio = state\n        .research\n        .active()\n        .copied()\n        .map(research_progress_ratio)\n        .unwrap_or(0.0);\n    for mut node in &mut progress {\n        node.width = Val::Percent(\n            (ratio * 100.0).clamp(0.0, 100.0),\n        );\n    }\n}\n\nfn queue_selected_research(\n    simulation: &mut SimulationResource,\n    ui: &mut ResearchUiState,\n) {\n    let technology = ui.selected;\n    match research_quote(\n        simulation.simulation().state(),\n        technology,\n    ) {\n        Ok(_) => {\n            apply_simulation_command(\n                simulation,\n                GameCommand::QueueResearch { technology },\n            );\n        }\n        Err(error) => {\n            ui.feedback = research_error_text(error);\n        }\n    }\n}\n\nfn technology_status_label(\n    state: &galactic_sim::GameState,\n    technology: TechnologyId,\n) -> String {\n    if state.research.has_completed(technology) {\n        return "ACQUISE".to_string();\n    }\n    if let Some(position) = state\n        .research\n        .queue()\n        .position(|project| {\n            project.technology == technology\n        })\n    {\n        return if position == 0 {\n            "EN COURS".to_string()\n        } else {\n            format!("EN ATTENTE — position {}", position + 1)\n        };\n    }\n\n    match research_quote(state, technology) {\n        Ok(_) => "DISPONIBLE".to_string(),\n        Err(error) => research_error_text(error),\n    }\n}\n\nfn research_detail_text(\n    state: &galactic_sim::GameState,\n    technology: TechnologyId,\n    quote: Result<ResearchQuote, ResearchError>,\n) -> String {\n    let definition =\n        technology_definition(technology);\n    let prerequisites = if definition\n        .prerequisites\n        .is_empty()\n    {\n        "aucun".to_string()\n    } else {\n        definition\n            .prerequisites\n            .iter()\n            .map(|prerequisite| {\n                technology_definition(*prerequisite).name\n            })\n            .collect::<Vec<_>>()\n            .join(", ")\n    };\n    let cost_points =\n        definition.required_milli_points as f64 / 1_000.0;\n\n    let mut lines = vec![\n        definition.name.to_uppercase(),\n        String::new(),\n        definition.description.to_string(),\n        String::new(),\n        format!("Prérequis : {prerequisites}"),\n        format!("Coût scientifique : {cost_points:.1} points"),\n        format!(\n            "Déblocage : {}",\n            definition.unlock_label,\n        ),\n    ];\n\n    if state.research.has_completed(technology) {\n        lines.push(String::new());\n        lines.push(\n            "TECHNOLOGIE ACQUISE".to_string(),\n        );\n        return lines.join("\\n");\n    }\n\n    if let Some(project) = state\n        .research\n        .queue()\n        .find(|project| {\n            project.technology == technology\n        })\n        .copied()\n    {\n        lines.extend([\n            String::new(),\n            format!(\n                "Progression : {:.1} / {:.1} points",\n                project.accumulated_milli_points as f64\n                    / 1_000.0,\n                project.required_milli_points as f64\n                    / 1_000.0,\n            ),\n            format!(\n                "État : {}",\n                technology_status_label(state, technology),\n            ),\n        ]);\n        return lines.join("\\n");\n    }\n\n    match quote {\n        Ok(value) => {\n            lines.extend([\n                String::new(),\n                format!(\n                    "Production actuelle : {:.2} point(s)/s",\n                    value.output_milli_points_per_tick\n                        as f64\n                        * f64::from(\n                            STRATEGIC_TICKS_PER_SECOND,\n                        )\n                        / 1_000.0,\n                ),\n                format!(\n                    "Durée estimée : {}",\n                    format_strategic_duration(\n                        StrategicDuration::from_ticks(\n                            value.estimated_ticks,\n                        ),\n                    ),\n                ),\n                String::new(),\n                "Prête à être ajoutée à la file."\n                    .to_string(),\n            ]);\n        }\n        Err(error) => {\n            lines.extend([\n                String::new(),\n                format!(\n                    "BLOCAGE : {}",\n                    research_error_text(error),\n                ),\n            ]);\n        }\n    }\n\n    lines.join("\\n")\n}\n\nfn research_queue_text(\n    state: &galactic_sim::GameState,\n) -> String {\n    if state.research.is_queue_empty() {\n        let has_output =\n            research_output_milli_points_per_tick(state)\n                > 0;\n        let hint = if !has_output {\n            "Construis un Laboratoire pour produire des points de recherche."\n        } else {\n            "Sélectionne une technologie disponible."\n        };\n        return format!(\n            "File vide\\n\\n{}\\n\\n{} emplacement(s) disponible(s).",\n            hint,\n            MAX_RESEARCH_QUEUE,\n        );\n    }\n\n    let output =\n        research_output_milli_points_per_tick(state);\n    let mut lines = Vec::new();\n    for (index, project) in\n        state.research.queue().enumerate()\n    {\n        let definition =\n            technology_definition(project.technology);\n        if index == 0 {\n            let remaining_ticks = if output == 0 {\n                None\n            } else {\n                Some(\n                    project\n                        .remaining_milli_points()\n                        .div_ceil(output),\n                )\n            };\n            let remaining = remaining_ticks\n                .map(|ticks| {\n                    format_strategic_duration(\n                        StrategicDuration::from_ticks(\n                            ticks,\n                        ),\n                    )\n                })\n                .unwrap_or_else(|| {\n                    "en pause — aucun laboratoire"\n                        .to_string()\n                });\n            lines.push(format!(\n                "EN COURS\\n{}. {}\\n{:.1} / {:.1} points\\n{} restante(s)",\n                index + 1,\n                definition.name,\n                project.accumulated_milli_points as f64\n                    / 1_000.0,\n                project.required_milli_points as f64\n                    / 1_000.0,\n                remaining,\n            ));\n        } else {\n            lines.push(format!(\n                "\\nEN ATTENTE\\n{}. {} — {:.1} points",\n                index + 1,\n                definition.name,\n                project.required_milli_points as f64\n                    / 1_000.0,\n            ));\n        }\n    }\n    lines.push(format!(\n        "\\n\\n{} / {} emplacement(s) utilisé(s)",\n        state.research.queue_len(),\n        MAX_RESEARCH_QUEUE,\n    ));\n    lines.join("\\n")\n}\n\nfn research_error_text(error: ResearchError) -> String {\n    match error {\n        ResearchError::NoResearchCapacity => {\n            "Laboratoire requis".to_string()\n        }\n        ResearchError::AlreadyCompleted(technology) => {\n            format!(\n                "{} déjà acquise",\n                technology_definition(technology).name,\n            )\n        }\n        ResearchError::AlreadyQueued(technology) => {\n            format!(\n                "{} déjà dans la file",\n                technology_definition(technology).name,\n            )\n        }\n        ResearchError::QueueFull { maximum } => {\n            format!("File pleine ({maximum})")\n        }\n        ResearchError::MissingPrerequisite {\n            prerequisite,\n            ..\n        } => {\n            format!(\n                "Requiert {}",\n                technology_definition(prerequisite).name,\n            )\n        }\n    }\n}\n\nfn technology_button_color(\n    selected: bool,\n    interaction: &Interaction,\n) -> Color {\n    if selected {\n        return Color::srgba(0.16, 0.20, 0.48, 0.98);\n    }\n    match interaction {\n        Interaction::Pressed => {\n            Color::srgba(0.14, 0.18, 0.40, 0.98)\n        }\n        Interaction::Hovered => {\n            Color::srgba(0.09, 0.12, 0.26, 0.98)\n        }\n        Interaction::None => {\n            Color::srgba(0.04, 0.05, 0.10, 0.98)\n        }\n    }\n}\n\nfn technology_button_outline(\n    selected: bool,\n) -> Color {\n    if selected {\n        Color::srgba(0.58, 0.68, 1.0, 0.88)\n    } else {\n        Color::srgba(0.32, 0.36, 0.62, 0.46)\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::UniverseConfig;\n    use galactic_sim::{\n        BuildingKind, Simulation,\n        default_building_catalog,\n    };\n\n    use super::*;\n\n    fn simulation_with_lab() -> Simulation {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony.buildings.set_level(\n            BuildingKind::ResearchLab,\n            1,\n        );\n        colony.energy = default_building_catalog()\n            .energy_grid_for_levels(colony.buildings);\n        simulation\n    }\n\n    #[test]\n    fn root_technology_explains_missing_laboratory() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n\n        let text = research_detail_text(\n            simulation.state(),\n            TechnologyId::SpatialDetection,\n            research_quote(\n                simulation.state(),\n                TechnologyId::SpatialDetection,\n            ),\n        );\n\n        assert!(text.contains("Laboratoire requis"));\n        assert!(text.contains("Détection"));\n    }\n\n    #[test]\n    fn queue_text_distinguishes_active_and_waiting() {\n        let mut simulation = simulation_with_lab();\n        simulation.apply_command(\n            GameCommand::QueueResearch {\n                technology:\n                    TechnologyId::SpatialDetection,\n            },\n        );\n        simulation.apply_command(\n            GameCommand::QueueResearch {\n                technology: TechnologyId::Propulsion,\n            },\n        );\n\n        let text =\n            research_queue_text(simulation.state());\n        assert!(text.contains("EN COURS"));\n        assert!(text.contains("EN ATTENTE"));\n    }\n\n    #[test]\n    fn research_overlay_is_above_colony_management() {\n        assert!(RESEARCH_Z_INDEX > 100);\n    }\n}\n'
SIM_TESTS = '\n    #[test]\n    fn research_and_laboratory_upgrade_are_frame_independent() {\n        fn configured_simulation() -> Simulation {\n            let mut simulation =\n                Simulation::new(UniverseConfig::mvp());\n            let colony_id = simulation\n                .state()\n                .player_home_colony()\n                .expect("home colony exists")\n                .id;\n            {\n                let colony = simulation\n                    .state_mut()\n                    .colony_mut(colony_id)\n                    .expect("home colony exists");\n                colony.buildings.set_level(\n                    BuildingKind::ResearchLab,\n                    1,\n                );\n                colony.energy = default_building_catalog()\n                    .energy_grid_for_levels(\n                        colony.buildings,\n                    );\n                colony\n                    .resources\n                    .credit(ResourceStock::new(\n                        1_000, 1_000, 500,\n                    ))\n                    .expect("test funding fits capacity");\n            }\n\n            for technology in TechnologyId::ALL {\n                let events = simulation.apply_command(\n                    GameCommand::QueueResearch {\n                        technology,\n                    },\n                );\n                assert!(matches!(\n                    events.as_slice(),\n                    [GameEvent::ResearchQueued(_)]\n                ));\n            }\n            let events = simulation.apply_command(\n                GameCommand::QueueBuildingUpgrade {\n                    colony_id,\n                    kind: BuildingKind::ResearchLab,\n                },\n            );\n            assert!(matches!(\n                events.as_slice(),\n                [GameEvent::ConstructionQueued(_)]\n            ));\n            simulation\n        }\n\n        let mut single_batch = configured_simulation();\n        let mut many_batches = configured_simulation();\n\n        single_batch.advance(Duration::from_secs(200));\n        advance_in_equal_frames(\n            &mut many_batches,\n            200,\n            Duration::from_secs(1),\n        );\n\n        assert_eq!(\n            single_batch.state(),\n            many_batches.state(),\n        );\n    }\n\n    #[test]\n    fn research_events_are_emitted_on_completion() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony.buildings.set_level(\n            BuildingKind::ResearchLab,\n            1,\n        );\n        colony.energy = default_building_catalog()\n            .energy_grid_for_levels(colony.buildings);\n\n        simulation.apply_command(\n            GameCommand::QueueResearch {\n                technology:\n                    TechnologyId::SpatialDetection,\n            },\n        );\n        let events =\n            simulation.advance(Duration::from_secs(60));\n\n        assert!(events.iter().any(|event| {\n            matches!(\n                event,\n                GameEvent::ResearchCompleted(completed)\n                    if completed.technology\n                        == TechnologyId::SpatialDetection\n            )\n        }));\n    }\n'
PERSISTENCE_TEST = '\n    #[test]\n    fn research_queue_survives_round_trip() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony.buildings.set_level(\n            BuildingKind::ResearchLab,\n            1,\n        );\n        colony.energy = default_building_catalog()\n            .energy_grid_for_levels(colony.buildings);\n\n        simulation.apply_command(\n            GameCommand::QueueResearch {\n                technology:\n                    TechnologyId::SpatialDetection,\n            },\n        );\n        simulation.advance(Duration::from_secs(12));\n\n        let save =\n            snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("research save is compatible");\n\n        assert_eq!(\n            restored.state().research,\n            simulation.state().research,\n        );\n        assert_eq!(\n            restored.state(),\n            simulation.state(),\n        );\n    }\n'
DOC_APPEND = "\n## MVP-016 — Recherche et arbre technologique minimal\n\nLa recherche est une progression globale au joueur. Tous les Laboratoires des\ncolonies du joueur contribuent à une seule production scientifique et à une\nfile commune de six projets maximum.\n\n### Technologies\n\nL'arbre minimal contient :\n\n1. Détection spatiale ;\n2. Propulsion ;\n3. Capacité cargo ;\n4. Extraction distante ;\n5. Analyse planétaire ;\n6. Colonisation.\n\nLes dépendances sont déterministes :\n\n```text\nDétection spatiale\n├── Propulsion\n│   └── Capacité cargo\n│       ├── Extraction distante\n│       └── Colonisation\n└── Analyse planétaire\n    └── Colonisation\n```\n\nChaque définition possède un nom, une description, un coût en milli-points,\ndes prérequis et une capacité débloquée. Les capacités sont des clés métier\nstables qui seront consommées par les missions, crafts et commandes des\ncheckpoints suivants.\n\n### Production scientifique\n\nLe Laboratoire du catalogue produit des milli-points par tick et par niveau.\nLa production globale additionne les niveaux actifs de Laboratoire de toutes\nles colonies du joueur.\n\nSans Laboratoire actif, aucune technologie ne peut être ajoutée à la file.\nUne amélioration de Laboratoire agit dès le tick stratégique où sa construction\nse termine.\n\nLa simulation traite production, construction et recherche tick par tick à\nl'intérieur d'un lot de temps. Le résultat reste donc identique quel que soit\nle découpage des frames, y compris lorsqu'un Laboratoire termine pendant le\nlot.\n\n### File globale\n\nUne technologie peut être ajoutée lorsque :\n\n- un Laboratoire produit des points ;\n- ses prérequis sont acquis ou placés avant elle dans la file ;\n- elle n'est ni acquise ni déjà planifiée ;\n- la file n'est pas pleine.\n\nUne technologie terminée ne peut pas être relancée. Les points excédentaires\nd'un lot passent au projet suivant.\n\n### Interface\n\nLa touche `T` et le bouton `Recherche` ouvrent un écran dédié au-dessus de la\nvue stratégique. Il présente :\n\n- les six technologies et leur état ;\n- les prérequis et le déblocage ;\n- le coût scientifique ;\n- la durée estimée avec la production actuelle ;\n- le projet actif, sa progression et les projets en attente ;\n- les refus et achèvements sous forme de messages non bloquants.\n\nL'écran de recherche et l'écran de gestion planétaire sont mutuellement\nexclusifs. La caméra stratégique est verrouillée lorsqu'un de ces écrans est\nouvert.\n\n### Persistance\n\nLe snapshot conserve l'ensemble acquis, la file et la progression du projet\nactif. Il stocke également la version et l'empreinte du catalogue\ntechnologique.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 9` ;\n- `SAVE_VERSION = 10` ;\n- `RESEARCH_CATALOG_VERSION = 1`.\n"


@dataclass(frozen=True)
class Update:
    path: Path
    before: str
    after: str


def run(
    command: list[str],
    *,
    cwd: Path,
    check: bool = True,
    capture: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
        env=env,
    )
    if capture and result.stdout:
        print(
            result.stdout,
            end="" if result.stdout.endswith("\n")
            else "\n",
        )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"Commande en échec ({result.returncode}) : "
            f"{' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (
                candidate
                / "crates/galactic_client/src/lib.rs"
            ).exists()
            and (
                candidate
                / "crates/galactic_sim/src/construction.rs"
            ).exists()
        ):
            return candidate
    raise SystemExit(
        "Racine Galactic introuvable. Utilise --root."
    )


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def replace_once(
    source: str,
    old: str,
    new: str,
    description: str,
) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"Patch impossible pour {description}: "
            f"{count} occurrence(s), 1 attendue."
        )
    return source.replace(old, new, 1)


def add_import_names(
    source: str,
    module: str,
    names: tuple[str, ...],
) -> str:
    pattern = re.compile(
        rf"use {re.escape(module)}::\{{"
        r"(?P<body>.*?)"
        r"\};",
        flags=re.DOTALL,
    )
    match = pattern.search(source)
    if match is None:
        raise SystemExit(
            f"Bloc d'import {module} introuvable."
        )

    body = match.group("body").rstrip()
    for name in names:
        if re.search(
            rf"\b{re.escape(name)}\b",
            body,
        ):
            continue
        body += f"\n    {name},"

    return (
        source[: match.start()]
        + f"use {module}::{{"
        + body
        + "\n};"
        + source[match.end() :]
    )


def verify_baseline(root: Path, force: bool) -> None:
    head = run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
    ).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return
    if force:
        print(
            "WARNING: HEAD différent ; --force désactive "
            "la vérification stricte de baseline."
        )
        return
    raise SystemExit(
        "Le dépôt n'est pas sur la baseline MVP-015 exacte.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}"
    )


def verify_exact_blobs(
    root: Path,
    force: bool,
) -> None:
    if force:
        return

    mismatches = []
    for relative, expected in EXPECTED_BLOBS.items():
        path = root / relative
        if not path.exists():
            mismatches.append(
                f"{relative}: fichier absent"
            )
            continue
        actual = run(
            ["git", "hash-object", relative],
            cwd=root,
        ).stdout.strip()
        if actual != expected:
            mismatches.append(
                f"{relative}: {actual} != {expected}"
            )

    for relative in (
        "crates/galactic_sim/src/research.rs",
        "crates/galactic_client/src/research_ui.rs",
    ):
        if (root / relative).exists():
            mismatches.append(
                f"{relative}: existe déjà"
            )

    if mismatches:
        raise SystemExit(
            "Préflight refusé : la baseline a été modifiée "
            "depuis son analyse.\n- "
            + "\n- ".join(mismatches)
        )


def verify_current_state(root: Path) -> None:
    markers = {
        "crates/galactic_sim/src/state.rs": (
            "GAME_STATE_VERSION: u32 = 8",
            "pub construction_queue: ConstructionQueue",
        ),
        "crates/galactic_persistence/src/lib.rs": (
            "SAVE_VERSION: u32 = 9",
            "pub construction_queue: ConstructionQueue",
        ),
        "crates/galactic_client/src/lib.rs": (
            "PresentationUpdateSet",
            "ColonyManagementState",
            "StrategicCameraInput",
            "GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX)",
        ),
    }

    failures = []
    for relative, expected_markers in markers.items():
        content = (root / relative).read_text(
            encoding="utf-8"
        )
        for marker in expected_markers:
            if marker not in content:
                failures.append(
                    f"{relative}: marqueur absent {marker}"
                )

    if failures:
        raise SystemExit(
            "Baseline MVP-015 incohérente :\n- "
            + "\n- ".join(failures)
        )


def cargo_edition(root: Path) -> str:
    cargo = (root / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    match = re.search(
        r'(?m)^edition\s*=\s*"([^"]+)"',
        cargo,
    )
    return match.group(1) if match else "2024"


def format_rust(root: Path, content: str) -> str:
    rustfmt = shutil.which("rustfmt")
    if rustfmt is None:
        raise SystemExit(
            "rustfmt est requis, y compris pour --dry-run."
        )

    with tempfile.NamedTemporaryFile(
        mode="w",
        suffix=".rs",
        encoding="utf-8",
        delete=False,
    ) as handle:
        temporary = Path(handle.name)
        handle.write(normalize(content))

    try:
        result = subprocess.run(
            [
                rustfmt,
                "--edition",
                cargo_edition(root),
                "--config",
                "skip_children=true",
                str(temporary),
            ],
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        if result.returncode != 0:
            raise SystemExit(
                "rustfmt n'a pas pu formater une source "
                f"générée :\n{result.stdout}"
            )
        return normalize(
            temporary.read_text(encoding="utf-8")
        )
    finally:
        temporary.unlink(missing_ok=True)


def fix_generated_research_source(source: str) -> str:
    unlock_test_impl = """#[cfg(test)]
impl TechnologyUnlock {
    const ALL_FOR_TESTS: [Self; 6] = [
        Self::DetectUnknownSystems,
        Self::InterstellarTravel,
        Self::ExpandedCargo,
        Self::RemoteExtraction,
        Self::AnalyzePlanets,
        Self::FoundColonies,
    ];
}"""
    tests_marker = "#[cfg(test)]\nmod tests {\n"
    impl_index = source.find(unlock_test_impl)
    tests_index = source.find(tests_marker)
    if impl_index == -1 or tests_index == -1:
        return normalize(source)
    if impl_index < tests_index:
        return normalize(source)

    source = replace_once(
        source,
        unlock_test_impl,
        "",
        "placement impl TechnologyUnlock",
    )
    source = replace_once(
        source,
        tests_marker,
        unlock_test_impl + "\n\n" + tests_marker,
        "impl TechnologyUnlock avant tests",
    )
    return normalize(source)


def fix_generated_research_ui_source(source: str) -> str:
    interaction_alias = """type ResearchButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ResearchButtonAction),
    (Changed<Interaction>, With<Button>),
>;

"""
    alias_marker = "#[derive(Component)]\nstruct ResearchRoot;\n"
    if "type ResearchButtonInteractionQuery" not in source:
        source = replace_once(
            source,
            alias_marker,
            interaction_alias + alias_marker,
            "alias interactions recherche",
        )

    source = replace_once(
        source,
        "    interactions: Query<\n"
        "        (&Interaction, &ResearchButtonAction),\n"
        "        (Changed<Interaction>, With<Button>),\n"
        "    >,\n",
        "    interactions: ResearchButtonInteractionQuery,\n",
        "paramètre interactions recherche",
    )
    source = source.replace(
        "        assert!(RESEARCH_Z_INDEX > 100);",
        "        const { assert!(RESEARCH_Z_INDEX > 100) };",
        1,
    )
    return normalize(source)


def patch_sim_lib(source: str) -> str:
    if "pub mod research;" not in source:
        source = replace_once(
            source,
            "pub mod production;\n",
            "pub mod production;\npub mod research;\n",
            "module research",
        )
    if "pub use research::*;" not in source:
        source = replace_once(
            source,
            "pub use production::*;\n",
            "pub use production::*;\npub use research::*;\n",
            "export research",
        )
    return normalize(source)


def patch_state(source: str) -> str:
    if "pub research: ResearchState" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-014: persistent production and construction queues",
        "// MVP-016: persistent production, construction and research",
        1,
    )
    source = add_import_names(
        source,
        "crate",
        ("ResearchState",),
    )
    source = replace_once(
        source,
        "/// Version 8 adds persistent construction queues.\n"
        "pub const GAME_STATE_VERSION: u32 = 8;",
        "/// Version 9 adds the global research state.\n"
        "pub const GAME_STATE_VERSION: u32 = 9;",
        "version d'état",
    )
    source = replace_once(
        source,
        "    pub colonies: Vec<ColonyState>,\n"
        "    pub system_knowledge: Vec<SystemKnowledge>,\n",
        "    pub colonies: Vec<ColonyState>,\n"
        "    pub research: ResearchState,\n"
        "    pub system_knowledge: Vec<SystemKnowledge>,\n",
        "champ research",
    )
    source = replace_once(
        source,
        "            }],\n"
        "            system_knowledge: Vec::new(),\n",
        "            }],\n"
        "            research: ResearchState::default(),\n"
        "            system_knowledge: Vec::new(),\n",
        "état de recherche initial",
    )
    return normalize(source)


def patch_command(source: str) -> str:
    if "QueueResearch" in source:
        return normalize(source)

    source = replace_once(
        source,
        "use crate::{BuildingKind, TimeSpeed};",
        "use crate::{BuildingKind, TechnologyId, TimeSpeed};",
        "import TechnologyId",
    )
    source = replace_once(
        source,
        "    QueueBuildingUpgrade {\n"
        "        colony_id: ColonyId,\n"
        "        kind: BuildingKind,\n"
        "    },\n"
        "    /// Temporary validation command",
        "    QueueBuildingUpgrade {\n"
        "        colony_id: ColonyId,\n"
        "        kind: BuildingKind,\n"
        "    },\n"
        "    QueueResearch {\n"
        "        technology: TechnologyId,\n"
        "    },\n"
        "    /// Temporary validation command",
        "commande de recherche",
    )
    return normalize(source)


def patch_event(source: str) -> str:
    if "ResearchQueued" in source:
        return normalize(source)

    source = add_import_names(
        source,
        "crate",
        (
            "ResearchCompleted",
            "ResearchQueued",
            "ResearchRejected",
        ),
    )
    source = replace_once(
        source,
        "    ConstructionRejected(ConstructionRejected),\n",
        "    ConstructionRejected(ConstructionRejected),\n"
        "    ResearchQueued(ResearchQueued),\n"
        "    ResearchCompleted(ResearchCompleted),\n"
        "    ResearchRejected(ResearchRejected),\n",
        "événements de recherche",
    )
    return normalize(source)


def patch_simulation(source: str) -> str:
    if "GameCommand::QueueResearch" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-014: catalog-driven production and construction",
        "// MVP-016: production, construction and research pipeline",
        1,
    )
    source = add_import_names(
        source,
        "crate",
        (
            "ResearchStateError",
            "StrategicDuration",
            "advance_research",
            "enqueue_research",
            "validate_research_state",
        ),
    )
    source = replace_once(
        source,
        "    InvalidConstructionQueue {\n"
        "        colony_id: ColonyId,\n"
        "        error: ConstructionQueueError,\n"
        "    },\n",
        "    InvalidConstructionQueue {\n"
        "        colony_id: ColonyId,\n"
        "        error: ConstructionQueueError,\n"
        "    },\n"
        "    InvalidResearchState(ResearchStateError),\n",
        "erreur d'état de recherche",
    )
    source = replace_once(
        source,
        "            GameCommand::DebugAdvanceSelectedKnowledge => "
        "self.debug_advance_selected_knowledge(),\n",
        "            GameCommand::QueueResearch { technology } => {\n"
        "                match enqueue_research(\n"
        "                    &mut self.state,\n"
        "                    technology,\n"
        "                ) {\n"
        "                    Ok(queued) => vec![\n"
        "                        GameEvent::ResearchQueued(queued),\n"
        "                    ],\n"
        "                    Err(error) => vec![\n"
        "                        GameEvent::ResearchRejected(\n"
        "                            crate::ResearchRejected {\n"
        "                                technology,\n"
        "                                error,\n"
        "                            },\n"
        "                        ),\n"
        "                    ],\n"
        "                }\n"
        "            },\n"
        "            GameCommand::DebugAdvanceSelectedKnowledge => "
        "self.debug_advance_selected_knowledge(),\n",
        "application commande recherche",
    )

    old_advance = """        let mut events = vec![GameEvent::TicksAdvanced {
            ticks: advance.ticks,
            current_tick: advance.current_tick,
        }];
        for colony in &mut self.state.colonies {
            if let Some(report) = queue_colony_production(colony, advance.ticks) {
                events.push(GameEvent::ProductionRefreshed(report));
            }
            let completed = advance_colony_construction(colony, advance.ticks)
                .expect("validated construction reservations must commit");
            events.extend(completed.into_iter().map(GameEvent::ConstructionCompleted));
        }
        events
"""
    new_advance = """        let mut events = vec![GameEvent::TicksAdvanced {
            ticks: advance.ticks,
            current_tick: advance.current_tick,
        }];
        let one_tick = StrategicDuration::from_ticks(1);
        for _ in 0..advance.ticks.ticks() {
            for colony in &mut self.state.colonies {
                if let Some(report) =
                    queue_colony_production(colony, one_tick)
                {
                    events.push(
                        GameEvent::ProductionRefreshed(report),
                    );
                }
                let completed =
                    advance_colony_construction(
                        colony,
                        one_tick,
                    )
                    .expect(
                        "validated construction reservations \
must commit",
                    );
                events.extend(
                    completed
                        .into_iter()
                        .map(
                            GameEvent::ConstructionCompleted,
                        ),
                );
            }
            events.extend(
                advance_research(
                    &mut self.state,
                    one_tick,
                )
                .into_iter()
                .map(GameEvent::ResearchCompleted),
            );
        }
        events
"""
    source = replace_once(
        source,
        old_advance,
        new_advance,
        "pipeline tick par tick",
    )

    validation_anchor = """    if player_faction.kind != FactionKind::Player {
        return Err(SimulationBuildError::PlayerFactionIsNotPlayer(
            state.player_faction,
        ));
    }

"""
    validation_insert = validation_anchor + """    if let Err(error) = validate_research_state(state) {
        return Err(
            SimulationBuildError::InvalidResearchState(
                error,
            ),
        );
    }

"""
    source = replace_once(
        source,
        validation_anchor,
        validation_insert,
        "validation recherche",
    )

    source = replace_once(
        source,
        "    use crate::KnowledgeTarget;\n",
        "    use crate::{\n"
        "        BuildingKind, KnowledgeTarget, TechnologyId,\n"
        "    };\n",
        "imports tests recherche",
    )
    test_marker = (
        "    #[test]\n"
        "    fn selection_events_use_domain_ids()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests recherche introuvable."
        )
    source = source.replace(
        test_marker,
        SIM_TESTS.rstrip()
        + "\n\n"
        + test_marker,
        1,
    )
    return normalize(source)


def patch_persistence(source: str) -> str:
    if "research_catalog_version" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-014: persist construction queues and reserved upgrade costs.",
        "// MVP-016: persist construction and global research.",
        1,
    )
    source = add_import_names(
        source,
        "galactic_sim",
        (
            "RESEARCH_CATALOG_FINGERPRINT",
            "RESEARCH_CATALOG_VERSION",
            "ResearchState",
        ),
    )
    source = replace_once(
        source,
        "pub const SAVE_VERSION: u32 = 9;",
        "pub const SAVE_VERSION: u32 = 10;",
        "version de sauvegarde",
    )
    source = replace_once(
        source,
        "    pub catalog_fingerprint: u64,\n"
        "    pub universe: UniverseReference,\n",
        "    pub catalog_fingerprint: u64,\n"
        "    pub research_catalog_version: u32,\n"
        "    pub research_catalog_fingerprint: u64,\n"
        "    pub universe: UniverseReference,\n",
        "métadonnées catalogue recherche",
    )
    source = replace_once(
        source,
        "    pub colonies: Vec<ColonySave>,\n",
        "    pub colonies: Vec<ColonySave>,\n"
        "    pub research: ResearchState,\n",
        "recherche sauvegardée",
    )
    source = replace_once(
        source,
        "    CatalogFingerprintMismatch {\n"
        "        expected: u64,\n"
        "        found: u64,\n"
        "    },\n",
        "    CatalogFingerprintMismatch {\n"
        "        expected: u64,\n"
        "        found: u64,\n"
        "    },\n"
        "    ResearchCatalogVersionMismatch {\n"
        "        expected: u32,\n"
        "        found: u32,\n"
        "    },\n"
        "    ResearchCatalogFingerprintMismatch {\n"
        "        expected: u64,\n"
        "        found: u64,\n"
        "    },\n",
        "erreurs catalogue recherche",
    )
    source = replace_once(
        source,
        "        catalog_fingerprint: catalog.fingerprint(),\n"
        "        universe: UniverseReference {\n",
        "        catalog_fingerprint: catalog.fingerprint(),\n"
        "        research_catalog_version:\n"
        "            RESEARCH_CATALOG_VERSION,\n"
        "        research_catalog_fingerprint:\n"
        "            RESEARCH_CATALOG_FINGERPRINT,\n"
        "        universe: UniverseReference {\n",
        "snapshot catalogue recherche",
    )
    source = replace_once(
        source,
        "                .collect(),\n"
        "        },\n"
        "    }\n"
        "}\n\n"
        "pub fn restore_from_snapshot",
        "                .collect(),\n"
        "            research: state.research.clone(),\n"
        "        },\n"
        "    }\n"
        "}\n\n"
        "pub fn restore_from_snapshot",
        "snapshot état recherche",
    )

    restore_anchor = """    if save.catalog_fingerprint != catalog.fingerprint() {
        return Err(SaveError::CatalogFingerprintMismatch {
            expected: catalog.fingerprint(),
            found: save.catalog_fingerprint,
        });
    }

"""
    restore_insert = restore_anchor + """    if save.research_catalog_version
        != RESEARCH_CATALOG_VERSION
    {
        return Err(
            SaveError::ResearchCatalogVersionMismatch {
                expected: RESEARCH_CATALOG_VERSION,
                found: save.research_catalog_version,
            },
        );
    }
    if save.research_catalog_fingerprint
        != RESEARCH_CATALOG_FINGERPRINT
    {
        return Err(
            SaveError::ResearchCatalogFingerprintMismatch {
                expected: RESEARCH_CATALOG_FINGERPRINT,
                found: save.research_catalog_fingerprint,
            },
        );
    }

"""
    source = replace_once(
        source,
        restore_anchor,
        restore_insert,
        "validation catalogue recherche",
    )
    source = replace_once(
        source,
        "        colonies,\n"
        "        system_knowledge: "
        "save.state.system_knowledge.clone(),\n",
        "        colonies,\n"
        "        research: save.state.research.clone(),\n"
        "        system_knowledge: "
        "save.state.system_knowledge.clone(),\n",
        "restauration recherche",
    )
    source = replace_once(
        source,
        "    use galactic_sim::{BuildingKind, "
        "GAME_STATE_VERSION, GameCommand};\n",
        "    use galactic_sim::{\n"
        "        BuildingKind, GAME_STATE_VERSION, "
        "GameCommand,\n"
        "        TechnologyId,\n"
        "    };\n",
        "imports tests persistance",
    )
    test_marker = (
        "    #[test]\n"
        "    fn catalog_changes_are_detected()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion test persistance introuvable."
        )
    source = source.replace(
        test_marker,
        PERSISTENCE_TEST.rstrip()
        + "\n\n"
        + test_marker,
        1,
    )
    source = source.replace(
        "fn state_and_save_versions_match_mvp_014()",
        "fn state_and_save_versions_match_mvp_016()",
        1,
    )
    return normalize(source)


def patch_client(source: str) -> str:
    if "mod research_ui;" in source:
        return normalize(source)

    source = replace_once(
        source,
        "use std::{collections::HashMap, time::Duration};\n",
        "mod research_ui;\n\n"
        "use std::{collections::HashMap, time::Duration};\n"
        "use research_ui::ResearchUiPlugin;\n",
        "module client recherche",
    )
    source = replace_once(
        source,
        "        .add_plugins(PresentationPlugin)\n"
        "        .add_systems(Startup, log_startup);\n",
        "        .add_plugins(PresentationPlugin)\n"
        "        .add_plugins(ResearchUiPlugin)\n"
        "        .add_systems(Startup, log_startup);\n",
        "plugin recherche",
    )
    source = replace_once(
        source,
        "            spawn_colony_management_toggle(parent);\n",
        "            spawn_colony_management_toggle(parent);\n"
        "            research_ui::spawn_research_toggle(parent);\n",
        "bouton recherche",
    )
    source = replace_once(
        source,
        "    mut management: ResMut<ColonyManagementState>,\n"
        ") {\n"
        "    if keyboard.just_pressed(KeyCode::KeyC) {\n",
        "    mut management: ResMut<ColonyManagementState>,\n"
        "    research: Res<research_ui::ResearchUiState>,\n"
        ") {\n"
        "    if research.open {\n"
        "        return;\n"
        "    }\n\n"
        "    if keyboard.just_pressed(KeyCode::KeyC) {\n",
        "verrouillage raccourcis recherche",
    )
    source = replace_once(
        source,
        "    management: Res<'w, ColonyManagementState>,\n"
        "}\n",
        "    management: Res<'w, ColonyManagementState>,\n"
        "    research: Res<'w, "
        "research_ui::ResearchUiState>,\n"
        "}\n",
        "paramètre caméra recherche",
    )
    source = replace_once(
        source,
        "    if input.management.open {\n"
        "        return;\n"
        "    }\n",
        "    if input.management.open || input.research.open {\n"
        "        return;\n"
        "    }\n",
        "verrouillage caméra recherche",
    )
    source = replace_once(
        source,
        "        GameEvent::ConstructionRejected(rejected) => "
        "format!(\n"
        "            \"construction {:?} refusée : {:?}\",\n"
        "            rejected.kind, rejected.error,\n"
        "        ),\n",
        "        GameEvent::ConstructionRejected(rejected) => "
        "format!(\n"
        "            \"construction {:?} refusée : {:?}\",\n"
        "            rejected.kind, rejected.error,\n"
        "        ),\n"
        "        GameEvent::ResearchQueued(queued) => "
        "format!(\n"
        "            \"recherche {:?} ajoutée ({})\",\n"
        "            queued.project.technology,\n"
        "            queued.queue_length,\n"
        "        ),\n"
        "        GameEvent::ResearchCompleted(completed) => "
        "format!(\n"
        "            \"recherche {:?} terminée\",\n"
        "            completed.technology,\n"
        "        ),\n"
        "        GameEvent::ResearchRejected(rejected) => "
        "format!(\n"
        "            \"recherche {:?} refusée : {:?}\",\n"
        "            rejected.technology,\n"
        "            rejected.error,\n"
        "        ),\n",
        "labels événements recherche",
    )
    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-016 — Recherche et arbre technologique minimal" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates: list[Update] = []

    new_rust_files = {
        root / "crates/galactic_sim/src/research.rs":
            fix_generated_research_source(RESEARCH_RS),
        root / "crates/galactic_client/src/research_ui.rs":
            fix_generated_research_ui_source(RESEARCH_UI_RS),
    }
    for path, content in new_rust_files.items():
        before = (
            path.read_text(encoding="utf-8")
            if path.exists()
            else ""
        )
        after = format_rust(root, content)
        if before != after:
            updates.append(Update(path, before, after))

    patchers = (
        (
            root / "crates/galactic_sim/src/lib.rs",
            patch_sim_lib,
        ),
        (
            root / "crates/galactic_sim/src/state.rs",
            patch_state,
        ),
        (
            root / "crates/galactic_sim/src/command.rs",
            patch_command,
        ),
        (
            root / "crates/galactic_sim/src/event.rs",
            patch_event,
        ),
        (
            root / "crates/galactic_sim/src/simulation.rs",
            patch_simulation,
        ),
        (
            root / "crates/galactic_persistence/src/lib.rs",
            patch_persistence,
        ),
        (
            root / "crates/galactic_client/src/lib.rs",
            patch_client,
        ),
    )
    for path, patcher in patchers:
        before = path.read_text(encoding="utf-8")
        after = format_rust(root, patcher(before))
        if before != after:
            updates.append(Update(path, before, after))

    docs = root / "docs/mvp_architecture.md"
    before = docs.read_text(encoding="utf-8")
    after = patch_docs(before)
    if before != after:
        updates.append(Update(docs, before, after))

    validate_prospective(root, updates)
    return updates


def validate_prospective(
    root: Path,
    updates: list[Update],
) -> None:
    mapped = {
        update.path: update.after for update in updates
    }
    required = {
        "crates/galactic_sim/src/research.rs": (
            "pub enum TechnologyId",
            "pub struct ResearchState",
            "pub fn advance_research",
            "pub fn validate_research_state",
        ),
        "crates/galactic_sim/src/state.rs": (
            "GAME_STATE_VERSION: u32 = 9",
            "pub research: ResearchState",
        ),
        "crates/galactic_persistence/src/lib.rs": (
            "SAVE_VERSION: u32 = 10",
            "research_catalog_version",
            "research: ResearchState",
        ),
        "crates/galactic_client/src/research_ui.rs": (
            "pub(crate) struct ResearchUiPlugin",
            "GameCommand::QueueResearch",
            "FILE DE RECHERCHE GLOBALE",
        ),
        "crates/galactic_client/src/lib.rs": (
            "mod research_ui;",
            ".add_plugins(ResearchUiPlugin)",
            "input.research.open",
        ),
    }

    failures = []
    for relative, markers in required.items():
        path = root / relative
        content = mapped.get(
            path,
            path.read_text(encoding="utf-8")
            if path.exists()
            else "",
        )
        for marker in markers:
            if marker not in content:
                failures.append(
                    f"{relative}: marqueur absent {marker}"
                )

    if failures:
        raise SystemExit(
            "Migration MVP-016 incomplète :\n- "
            + "\n- ".join(failures)
        )


def show_diff(update: Update, root: Path) -> None:
    relative = update.path.relative_to(root)
    print(
        "".join(
            difflib.unified_diff(
                update.before.splitlines(keepends=True),
                update.after.splitlines(keepends=True),
                fromfile=f"a/{relative}",
                tofile=f"b/{relative}",
            )
        ),
        end="",
    )


def write_updates_to(
    updates: list[Update],
    source_root: Path,
    destination_root: Path,
) -> None:
    for update in updates:
        relative = update.path.relative_to(source_root)
        destination = destination_root / relative
        destination.parent.mkdir(
            parents=True,
            exist_ok=True,
        )
        destination.write_text(
            update.after,
            encoding="utf-8",
        )


def preflight_in_worktree(
    root: Path,
    updates: list[Update],
    skip_checks: bool,
) -> None:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp016-"
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        run(
            [
                "git",
                "worktree",
                "add",
                "--detach",
                str(worktree),
                "HEAD",
            ],
            cwd=root,
            capture=False,
        )
        try:
            write_updates_to(
                updates,
                root,
                worktree,
            )
            run(
                ["git", "diff", "--check"],
                cwd=worktree,
                capture=False,
            )
            if skip_checks:
                print(
                    "WARNING: préflight Cargo ignoré par "
                    "--skip-checks."
                )
                return

            environment = os.environ.copy()
            environment["CARGO_TARGET_DIR"] = str(
                root / "target"
            )
            commands = (
                ["cargo", "fmt", "--all", "--", "--check"],
                [
                    "cargo",
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                ],
                [
                    "cargo",
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--",
                    "-D",
                    "warnings",
                ],
                ["cargo", "test", "--workspace"],
                ["cargo", "build", "--release"],
            )
            for command in commands:
                run(
                    command,
                    cwd=worktree,
                    capture=False,
                    env=environment,
                )
        finally:
            run(
                [
                    "git",
                    "worktree",
                    "remove",
                    "--force",
                    str(worktree),
                ],
                cwd=root,
                check=False,
                capture=False,
            )
            run(
                ["git", "worktree", "prune"],
                cwd=root,
                check=False,
                capture=False,
            )


def apply_updates(
    updates: list[Update],
    root: Path,
) -> Path | None:
    if not updates:
        print("MVP-016 est déjà appliqué.")
        return None

    backup_root = (
        root
        / ".mvp016-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        if update.path.exists():
            backup = backup_root / relative
            backup.parent.mkdir(
                parents=True,
                exist_ok=True,
            )
            shutil.copy2(update.path, backup)
        update.path.parent.mkdir(
            parents=True,
            exist_ok=True,
        )
        update.path.write_text(
            update.after,
            encoding="utf-8",
        )
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")
    return backup_root


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument(
        "--force",
        action="store_true",
    )
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_exact_blobs(root, args.force)
    verify_current_state(root)

    status = run(
        ["git", "status", "--porcelain"],
        cwd=root,
    ).stdout
    if status.strip():
        print(
            "WARNING: working tree already contains changes."
        )
        print(
            status,
            end="" if status.endswith("\n")
            else "\n",
        )

    updates = collect_updates(root)
    if not updates:
        print("MVP-016 est déjà appliqué.")
        return 0

    for update in updates:
        show_diff(update, root)

    print(
        "\nPréflight dans un worktree temporaire..."
    )
    try:
        preflight_in_worktree(
            root,
            updates,
            args.skip_checks,
        )
    except RuntimeError as error:
        raise SystemExit(
            "\nPréflight en échec. Le dépôt principal "
            "n'a pas été modifié.\n"
            f"{error}"
        ) from error

    if args.skip_checks:
        print(
            "\nPréflight structurel réussi. Les contrôles "
            "Cargo ont été ignorés par --skip-checks."
        )
    else:
        print(
            "\nPréflight réussi : les sources générées "
            "formatent, compilent et passent les tests."
        )
    if args.dry_run:
        print(
            f"Dry-run complete: {len(updates)} "
            "file(s) would change."
        )
        return 0

    verify_exact_blobs(root, args.force)
    apply_updates(updates, root)
    run(
        ["git", "diff", "--check"],
        cwd=root,
        capture=False,
    )

    print(
        "\nMVP-016 appliqué après validation complète.\n"
        "Vérifie ensuite :\n"
        "  git diff\n"
        "  cargo run --release\n"
        "\nRaccourci recherche : T"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

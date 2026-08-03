use std::collections::HashMap;

use bevy::prelude::*;
use galactic_domain::{SectorId, SystemId};

use crate::presentation::strategic_navigation::UniverseLod;

pub(crate) const LABEL_HIDE_DELAY_SECONDS: f32 = 0.4;
const OVERVIEW_LABEL_BUDGET: usize = 12;
const REGIONAL_LABEL_BUDGET: usize = 30;
pub(crate) const LABEL_MIN_SEPARATION_FACTOR: f32 = 0.09;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LabelMemory {
    pub(crate) visible: bool,
    pub(crate) losing_since: Option<f32>,
}

#[derive(Resource, Default)]
pub(crate) struct LabelBudgetState {
    pub(crate) systems: HashMap<SystemId, LabelMemory>,
}

pub(crate) const fn label_budget_for_lod(lod: UniverseLod) -> Option<usize> {
    match lod {
        UniverseLod::Overview => Some(OVERVIEW_LABEL_BUDGET),
        UniverseLod::Regional => Some(REGIONAL_LABEL_BUDGET),
        UniverseLod::Local => None,
    }
}

pub(crate) fn labels_overlap(a: Vec2, b: Vec2, min_distance: f32) -> bool {
    a.distance(b) < min_distance
}

pub(crate) fn advance_label_memory(
    previous: Option<LabelMemory>,
    currently_winning: bool,
    now: f32,
    hide_delay: f32,
) -> LabelMemory {
    let previous = previous.unwrap_or(LabelMemory {
        visible: currently_winning,
        losing_since: None,
    });

    if currently_winning {
        return LabelMemory {
            visible: true,
            losing_since: None,
        };
    }

    if !previous.visible {
        return LabelMemory {
            visible: false,
            losing_since: None,
        };
    }

    let losing_since = previous.losing_since.unwrap_or(now);
    if now - losing_since >= hide_delay {
        LabelMemory {
            visible: false,
            losing_since: None,
        }
    } else {
        LabelMemory {
            visible: true,
            losing_since: Some(losing_since),
        }
    }
}

pub(crate) fn group_missions_by_sector<'a>(
    missions: impl Iterator<Item = &'a galactic_sim::MissionState>,
    universe: &galactic_sim::UniverseRepository,
) -> HashMap<SectorId, usize> {
    let mut counts = HashMap::new();
    for mission in missions {
        if let Some(sector) = universe.sector_for_system(mission.order.origin) {
            *counts.entry(sector.id).or_insert(0usize) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn labels_overlap_detects_nearby_points_only() {
        assert!(labels_overlap(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0));
        assert!(!labels_overlap(Vec2::ZERO, Vec2::new(5.0, 0.0), 2.0));
    }

    #[test]
    fn advance_label_memory_keeps_a_winning_label_visible_without_delay() {
        let memory = advance_label_memory(None, true, 0.0, 0.4);
        assert!(memory.visible);
        assert_eq!(memory.losing_since, None);
    }

    #[test]
    fn advance_label_memory_hides_only_after_losing_for_the_full_delay() {
        let visible = LabelMemory {
            visible: true,
            losing_since: None,
        };

        let still_visible = advance_label_memory(Some(visible), false, 0.1, 0.4);
        assert!(still_visible.visible);
        assert_eq!(still_visible.losing_since, Some(0.1));

        let now_hidden = advance_label_memory(Some(still_visible), false, 0.6, 0.4);
        assert!(!now_hidden.visible);
    }

    #[test]
    fn advance_label_memory_winning_again_cancels_the_hide_timer() {
        let losing = LabelMemory {
            visible: true,
            losing_since: Some(0.1),
        };
        let recovered = advance_label_memory(Some(losing), true, 0.2, 0.4);
        assert!(recovered.visible);
        assert_eq!(recovered.losing_since, None);
    }

    #[test]
    fn group_missions_by_sector_counts_only_the_origin_sector() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = simulation.universe_repository().neighboring_systems(origin)[0];
        let colony = &mut simulation.state_mut().colonies[0];
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::CONSTRUCTION_CENTER, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::METAL_MINE, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::CRYSTAL_EXTRACTOR, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::SHIPYARD, 1);
        colony.energy =
            galactic_sim::default_building_catalog().energy_grid_for_levels(colony.buildings);
        colony
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test funding fits");
        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
        ]);
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: galactic_sim::CraftableId::LIGHT_PROBE,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(50));
        simulation.apply_player_action(GameAction::LaunchProbe {
            colony_id,
            target: MissionTarget::System(target),
        });
        simulation.advance(Duration::from_secs(1));

        let origin_sector = simulation
            .universe_repository()
            .sector_for_system(origin)
            .expect("origin belongs to a sector")
            .id;
        let missions = simulation
            .state()
            .player_missions()
            .filter(|mission| !mission.phase.is_terminal())
            .collect::<Vec<_>>();
        assert_eq!(missions.len(), 1);

        let counts =
            group_missions_by_sector(missions.into_iter(), simulation.universe_repository());
        assert_eq!(counts.get(&origin_sector), Some(&1));
        assert_eq!(counts.len(), 1);
    }
}

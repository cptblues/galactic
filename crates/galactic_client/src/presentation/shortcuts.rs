use bevy::prelude::*;
use galactic_domain::SystemId;
use galactic_sim::{GameAction, SelectionTarget, Simulation, TimeSpeed};
#[cfg(test)]
use galactic_sim::{KnowledgeLevel, MissionTarget, PlanetaryOccupancyIntel};

use crate::presentation::components::*;
use crate::presentation::scene::systems_for_universe_view;
use crate::presentation::strategic_navigation::*;
use crate::*;

pub(crate) fn simulation_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<UiAction> {
    if keyboard.just_pressed(KeyCode::Space) {
        Some(UiAction::TogglePause)
    } else if keyboard.just_pressed(KeyCode::Digit1) {
        Some(UiAction::SetSpeed(TimeSpeed::X1))
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(UiAction::SetSpeed(TimeSpeed::X2))
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(UiAction::SetSpeed(TimeSpeed::X4))
    } else {
        None
    }
}

pub(crate) fn view_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<UiAction> {
    if keyboard.just_pressed(KeyCode::KeyP) {
        Some(UiAction::ToggleProjection)
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        Some(UiAction::RebuildView)
    } else if keyboard.just_pressed(KeyCode::KeyG) {
        Some(UiAction::ToggleDebugGraph)
    } else if keyboard.just_pressed(KeyCode::Tab) {
        Some(UiAction::CycleTarget)
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        Some(UiAction::FocusSelection)
    } else if keyboard.just_pressed(KeyCode::Enter) {
        Some(UiAction::EnterSystem)
    } else if keyboard.just_pressed(KeyCode::Escape) {
        Some(UiAction::ExitSystem)
    } else {
        None
    }
}

pub(crate) fn apply_ui_action(
    action: UiAction,
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    rebuild: &mut ViewRebuildRequest,
    history: &mut NavigationHistory,
) {
    if !action_available(action, simulation, navigation) {
        return;
    }

    match action {
        UiAction::TogglePause => apply_simulation_command(simulation, GameAction::TogglePause),
        UiAction::SetSpeed(speed) => {
            apply_simulation_command(simulation, GameAction::SetSpeed(speed));
        }
        UiAction::CycleTarget => match navigation.mode {
            StrategicViewMode::Universe => {
                cycle_visible_selection(simulation, navigation.debug_full_graph);
            }
            StrategicViewMode::System(system_id) => {
                cycle_planet_selection(simulation, system_id);
            }
        },
        UiAction::FocusSelection => {
            focus_selected_system(simulation, navigation);
        }
        UiAction::EnterSystem => {
            if let Some(system_id) =
                enterable_selected_system(simulation, navigation.debug_full_graph)
            {
                let selected = simulation.simulation.state().selected;
                history.push(navigation.snapshot(selected));
                navigation.enter_system(system_id);
                rebuild.0 = true;
            }
        }
        UiAction::ExitSystem => {
            let selected = simulation.simulation.state().selected;
            history.push(navigation.snapshot(selected));
            navigation.exit_system();
            rebuild.0 = true;
        }
        UiAction::ToggleProjection => {
            navigation.toggle_projection();
        }
        UiAction::ToggleDebugGraph => {
            navigation.debug_full_graph = !navigation.debug_full_graph;
            rebuild.0 = true;
        }
        UiAction::RebuildView => {
            rebuild.0 = true;
        }
    }
}

pub(crate) fn apply_simulation_command(simulation: &mut SimulationResource, action: GameAction) {
    let events = simulation.simulation.apply_player_action(action);
    simulation.pending_events.extend(events);
}

/// Atomically swaps in a freshly loaded `Simulation` (MVP-031 save/load) and
/// requests a full view rebuild, the same mechanism `R` already uses to
/// despawn and recreate every view entity from `GameState`.
pub(crate) fn replace_simulation(
    simulation: &mut SimulationResource,
    loaded: Simulation,
    rebuild: &mut ViewRebuildRequest,
) {
    simulation.simulation = loaded;
    simulation.pending_events.clear();
    rebuild.0 = true;
}

pub(crate) fn action_available(
    action: UiAction,
    simulation: &SimulationResource,
    navigation: &StrategicNavigation,
) -> bool {
    match action {
        UiAction::TogglePause
        | UiAction::SetSpeed(_)
        | UiAction::ToggleDebugGraph
        | UiAction::RebuildView => true,
        UiAction::ToggleProjection => {
            matches!(navigation.mode, StrategicViewMode::Universe)
        }
        UiAction::CycleTarget => match navigation.mode {
            StrategicViewMode::Universe => {
                !systems_for_universe_view(simulation.simulation(), navigation.debug_full_graph)
                    .is_empty()
            }
            StrategicViewMode::System(system_id) => {
                !visible_planet_ids(simulation.simulation(), system_id).is_empty()
            }
        },
        UiAction::FocusSelection => {
            matches!(navigation.mode, StrategicViewMode::Universe)
                && selected_system(simulation.simulation.state().selected)
                    .and_then(|system_id| simulation.simulation.universe().system(system_id))
                    .is_some()
        }
        UiAction::EnterSystem => {
            matches!(navigation.mode, StrategicViewMode::Universe)
                && enterable_selected_system(simulation, navigation.debug_full_graph).is_some()
        }
        UiAction::ExitSystem => matches!(navigation.mode, StrategicViewMode::System(_)),
    }
}

pub(crate) fn action_active(
    action: UiAction,
    simulation: &SimulationResource,
    navigation: &StrategicNavigation,
) -> bool {
    match action {
        UiAction::TogglePause => simulation.simulation.state().clock.speed().is_paused(),
        UiAction::SetSpeed(speed) => simulation.simulation.state().clock.speed() == speed,
        UiAction::ToggleDebugGraph => navigation.debug_full_graph,
        UiAction::ToggleProjection => navigation.projection == UniverseProjection::Flattened,
        UiAction::ExitSystem => matches!(navigation.mode, StrategicViewMode::System(_)),
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn selected_attack_context(
    simulation: &Simulation,
) -> Option<(galactic_domain::ColonyId, MissionTarget)> {
    let state = simulation.state();
    let SelectionTarget::Planet {
        system_id,
        planet_id,
    } = state.selected
    else {
        return None;
    };
    if state.planet_knowledge_level(planet_id) < KnowledgeLevel::Analyzed {
        return None;
    }
    let report = state.planetary_intelligence_report(planet_id)?;
    let PlanetaryOccupancyIntel::Occupied(occupant) = report.occupancy else {
        return None;
    };
    if occupant == state.player_faction {
        return None;
    }
    Some((
        state.active_player_colony()?.id,
        MissionTarget::Planet {
            system_id,
            planet_id,
        },
    ))
}

pub(crate) fn focus_selected_system(
    simulation: &SimulationResource,
    navigation: &mut StrategicNavigation,
) {
    let Some(system_id) = selected_system(simulation.simulation.state().selected) else {
        return;
    };
    let Some(system) = simulation.simulation.universe().system(system_id) else {
        return;
    };

    navigation.universe_focus =
        projected_universe_position(system.position, navigation.projection_mix);
}

pub(crate) fn enterable_selected_system(
    simulation: &SimulationResource,
    debug_full_graph: bool,
) -> Option<SystemId> {
    let system_id = selected_system(simulation.simulation.state().selected)?;

    let level = simulation
        .simulation
        .state()
        .system_knowledge_level(system_id);

    if debug_full_graph || level.can_enter_system() {
        Some(system_id)
    } else {
        None
    }
}

pub(crate) fn cycle_visible_selection(simulation: &mut SimulationResource, debug_full_graph: bool) {
    let systems = systems_for_universe_view(simulation.simulation(), debug_full_graph)
        .into_iter()
        .filter(|entry| entry.tier != UniverseSystemTier::Observed || debug_full_graph)
        .collect::<Vec<_>>();
    if systems.is_empty() {
        return;
    }

    let current = selected_system(simulation.simulation.state().selected);
    let current_index =
        current.and_then(|current_id| systems.iter().position(|entry| entry.id == current_id));
    let next_index = current_index
        .map(|index| (index + 1) % systems.len())
        .unwrap_or(0);
    let next_system = systems[next_index].id;

    apply_simulation_command(simulation, GameAction::SelectSystem(next_system));
}

pub(crate) fn cycle_planet_selection(simulation: &mut SimulationResource, system_id: SystemId) {
    let visible_planets = visible_planet_ids(simulation.simulation(), system_id);
    if visible_planets.is_empty() {
        return;
    }

    let current = match simulation.simulation.state().selected {
        SelectionTarget::Planet { planet_id, .. } => Some(planet_id),
        SelectionTarget::None | SelectionTarget::System(_) => None,
    };
    let current_index = current.and_then(|planet_id| {
        visible_planets
            .iter()
            .position(|candidate| *candidate == planet_id)
    });
    let next_index = current_index
        .map(|index| (index + 1) % visible_planets.len())
        .unwrap_or(0);
    let planet_id = visible_planets[next_index];

    apply_simulation_command(
        simulation,
        GameAction::SelectPlanet {
            system_id,
            planet_id,
        },
    );
}

pub(crate) fn visible_planet_ids(
    simulation: &Simulation,
    system_id: SystemId,
) -> Vec<galactic_domain::PlanetId> {
    let Some(system) = simulation.universe().system(system_id) else {
        return Vec::new();
    };

    system
        .planets
        .iter()
        .filter(|planet| {
            simulation
                .state()
                .planet_knowledge_level(planet.id)
                .is_visible()
        })
        .map(|planet| planet.id)
        .collect()
}

pub(crate) fn selected_system(selection: SelectionTarget) -> Option<SystemId> {
    match selection {
        SelectionTarget::None => None,
        SelectionTarget::System(system_id) => Some(system_id),
        SelectionTarget::Planet { system_id, .. } => Some(system_id),
    }
}

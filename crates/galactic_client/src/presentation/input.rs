use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use galactic_domain::PlanetId;
use galactic_sim::{GameAction, KnowledgeLevel, SelectionTarget, Simulation};
use std::time::Duration;

use crate::presentation::colony_management_ui::toggle_colony_management;
use crate::presentation::components::*;
use crate::presentation::inspector_panel::*;
use crate::presentation::shortcuts::{
    enterable_selected_system, simulation_shortcut, view_shortcut,
};
use crate::presentation::strategic_navigation::*;
use crate::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_simulation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut history: ResMut<NavigationHistory>,
    navigation_ui: Res<navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    save_load_ui: Res<crate::save_load_ui::SaveLoadUiState>,
) {
    if navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
        || crate::save_load_ui::save_name_is_editing(&save_load_ui)
    {
        return;
    }
    if let Some(action) = simulation_shortcut(&keyboard) {
        apply_ui_action(
            action,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
            &mut history,
        );
    }
}

pub(crate) fn toggle_debug_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut debug_overlay: ResMut<DebugOverlayState>,
    navigation_ui: Res<navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    save_load_ui: Res<crate::save_load_ui::SaveLoadUiState>,
) {
    if navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
        || crate::save_load_ui::save_name_is_editing(&save_load_ui)
    {
        return;
    }
    if keyboard.just_pressed(KeyCode::Backquote) {
        debug_overlay.visible = !debug_overlay.visible;
    }
}

pub(crate) fn update_debug_overlay_visibility(
    debug_overlay: Res<DebugOverlayState>,
    mut roots: Query<&mut Visibility, With<DebugOverlayRoot>>,
) {
    if !debug_overlay.is_changed() {
        return;
    }
    let visibility = if debug_overlay.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut root_visibility in &mut roots {
        *root_visibility = visibility;
    }
}

#[derive(SystemParam)]
pub(crate) struct ViewInputState<'w> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    simulation: ResMut<'w, SimulationResource>,
    navigation: ResMut<'w, StrategicNavigation>,
    rebuild: ResMut<'w, ViewRebuildRequest>,
    pointer_state: ResMut<'w, PointerSelectionState>,
    management: ResMut<'w, ColonyManagementState>,
    history: ResMut<'w, NavigationHistory>,
    open_panel: ResMut<'w, OpenPanel>,
}

pub(crate) fn handle_view_input(
    input: ViewInputState,
    mut navigation_ui: ResMut<navigation_ui::NavigationUiState>,
) {
    let ViewInputState {
        keyboard,
        mut simulation,
        mut navigation,
        mut rebuild,
        mut pointer_state,
        mut management,
        mut history,
        mut open_panel,
    } = input;

    if matches!(
        *open_panel,
        OpenPanel::Research
            | OpenPanel::Craft
            | OpenPanel::Fleet
            | OpenPanel::Navigation
            | OpenPanel::Objectives
    ) {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyC) {
        toggle_colony_management(
            &mut management,
            &mut open_panel,
            &mut simulation,
            &mut navigation_ui,
        );
        pointer_state.ambiguity = None;
        return;
    }

    if *open_panel == OpenPanel::Colony {
        if keyboard.just_pressed(KeyCode::Escape) {
            *open_panel = OpenPanel::None;
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            cycle_management_colony(&mut management, &mut simulation, true);
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            cycle_management_colony(&mut management, &mut simulation, false);
        }
        return;
    }

    if pointer_state.ambiguity.is_some() {
        if keyboard.just_pressed(KeyCode::Tab) {
            let reverse = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            if let Some(target) = pointer_state.cycle_ambiguity(reverse) {
                select_pick_target(&mut simulation, target);
            }
            return;
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            pointer_state.ambiguity = None;
            return;
        }
        if keyboard.just_pressed(KeyCode::Escape) {
            pointer_state.ambiguity = None;
            return;
        }
    }

    if let Some(direction) = history_shortcut(&keyboard) {
        navigate_history(
            direction,
            &mut simulation,
            &mut navigation,
            &mut history,
            &mut rebuild,
        );
        return;
    }

    if let Some(action) = view_shortcut(&keyboard) {
        apply_ui_action(
            action,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
            &mut history,
        );
    }
}

pub(crate) fn handle_action_buttons(
    mut interactions: ActionButtonInteractionQuery,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut history: ResMut<NavigationHistory>,
) {
    for (interaction, button) in &mut interactions {
        if matches!(interaction, Interaction::Pressed) {
            apply_ui_action(
                button.action,
                &mut simulation,
                &mut navigation,
                &mut rebuild,
                &mut history,
            );
        }
    }
}

pub(crate) fn update_pointer_candidates(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &Transform), With<StrategicCamera>>,
    targets: Query<(&SelectableVisual, &Transform)>,
    blockers: Query<&Interaction, With<UiPointerBlocker>>,
    simulation: Res<SimulationResource>,
    mut pointer_state: ResMut<PointerSelectionState>,
) {
    let Ok(window) = windows.single() else {
        pointer_state.clear_hover();
        return;
    };
    let Some(cursor_position) = window.cursor_position() else {
        pointer_state.clear_hover();
        return;
    };
    if blockers
        .iter()
        .any(|interaction| *interaction != Interaction::None)
    {
        pointer_state.clear_hover();
        return;
    }

    let Ok((camera, camera_transform)) = cameras.single() else {
        pointer_state.clear_hover();
        return;
    };
    let camera_global = GlobalTransform::from(*camera_transform);
    let selected = simulation.simulation().state().selected;
    let mut candidates = Vec::new();

    for (selectable, visual_transform) in &targets {
        if !pick_target_is_visible(simulation.simulation(), selectable.target) {
            continue;
        }
        let world_position = visual_transform.translation;
        let Ok(screen_position) = camera.world_to_viewport(&camera_global, world_position) else {
            continue;
        };
        let screen_distance = cursor_position.distance(screen_position);
        if !screen_space_hit(cursor_position, screen_position, selectable.pick_radius_px) {
            continue;
        }

        let selected_bonus = if pick_target_matches_selection(selectable.target, selected) {
            32
        } else {
            0
        };
        candidates.push(PointerCandidate {
            target: selectable.target,
            screen_position,
            screen_distance,
            depth: camera_transform.translation.distance(world_position),
            priority: selectable.priority.saturating_add(selected_bonus),
        });
    }

    rank_pointer_candidates(&mut candidates);
    pointer_state.hovered = candidates.first().map(|candidate| candidate.target);
    pointer_state.hovered_screen_position = candidates
        .first()
        .map(|candidate| candidate.screen_position);
    pointer_state.candidates = candidates;
}

pub(crate) fn handle_pointer_selection(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut pointer_state: ResMut<PointerSelectionState>,
    targets: Query<(&SelectableVisual, &Transform)>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(primary) = pointer_state.candidates.first().copied() else {
        pointer_state.ambiguity = None;
        return;
    };

    let targets_under_pointer = pointer_state
        .candidates
        .iter()
        .map(|candidate| candidate.target)
        .collect::<Vec<_>>();
    pointer_state.ambiguity = (targets_under_pointer.len() > 1).then_some(AmbiguitySelection {
        targets: targets_under_pointer,
        active_index: 0,
    });

    select_pick_target(&mut simulation, primary.target);

    let now = time.elapsed();
    let is_double_click = pointer_state.last_click.is_some_and(|previous| {
        pointer_double_click(previous, primary.target, now, primary.screen_position)
    });
    pointer_state.last_click = Some(PointerClickRecord {
        target: primary.target,
        at: now,
        cursor_position: primary.screen_position,
    });

    if is_double_click {
        activate_pick_target(
            primary.target,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
            &targets,
        );
        pointer_state.ambiguity = None;
        pointer_state.last_click = None;
    }
}

pub(crate) fn update_pointer_halos(
    pointer_state: Res<PointerSelectionState>,
    mut halos: Query<(&PointerHalo, &mut Visibility)>,
) {
    if !pointer_state.is_changed() {
        return;
    }

    for (halo, mut visibility) in &mut halos {
        let next = if Some(halo.target) == pointer_state.hovered {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

pub(crate) fn update_pointer_tooltip(
    windows: Query<&Window, With<PrimaryWindow>>,
    simulation: Res<SimulationResource>,
    pointer_state: Res<PointerSelectionState>,
    mut tooltips: Query<(&mut Text, &mut Node, &mut Visibility), With<PointerTooltipText>>,
) {
    let Ok((mut text, mut node, mut visibility)) = tooltips.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(target) = pointer_state.hovered else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(screen_position) = pointer_state.hovered_screen_position else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let next_text = pointer_tooltip_text(simulation.simulation(), target);
    if text.0 != next_text {
        text.0 = next_text;
    }
    let next_left =
        Val::Px((screen_position.x + 18.0).clamp(8.0, (window.width() - 270.0).max(8.0)));
    if node.left != next_left {
        node.left = next_left;
    }
    let next_top =
        Val::Px((screen_position.y + 18.0).clamp(8.0, (window.height() - 110.0).max(8.0)));
    if node.top != next_top {
        node.top = next_top;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }
}

pub(crate) fn update_ambiguity_panel(
    simulation: Res<SimulationResource>,
    pointer_state: Res<PointerSelectionState>,
    mut panels: Query<(&mut Text, &mut Visibility), With<AmbiguityPanelText>>,
) {
    let Ok((mut text, mut visibility)) = panels.single_mut() else {
        return;
    };
    let Some(ambiguity) = pointer_state.ambiguity.as_ref() else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let mut lines = vec![
        "PLUSIEURS CIBLES SOUS LE CURSEUR".to_string(),
        "Tab / Maj+Tab : parcourir | Entrée : valider | Échap : fermer".to_string(),
        String::new(),
    ];
    for (index, target) in ambiguity.targets.iter().enumerate() {
        let marker = if index == ambiguity.active_index {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "{} {}. {}",
            marker,
            index + 1,
            pick_target_label(simulation.simulation(), *target),
        ));
    }

    let next_text = lines.join("\n");
    if text.0 != next_text {
        text.0 = next_text;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }
}

pub(crate) fn select_pick_target(simulation: &mut SimulationResource, target: PickTarget) {
    let command = match target {
        PickTarget::System(system_id) => GameAction::SelectSystem(system_id),
        PickTarget::Planet {
            system_id,
            planet_id,
        } => GameAction::SelectPlanet {
            system_id,
            planet_id,
        },
    };
    apply_simulation_command(simulation, command);
}

pub(crate) fn activate_pick_target(
    target: PickTarget,
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    rebuild: &mut ViewRebuildRequest,
    visuals: &Query<(&SelectableVisual, &Transform)>,
) {
    let visual_position = visuals.iter().find_map(|(selectable, transform)| {
        (selectable.target == target).then_some(transform.translation)
    });

    match target {
        PickTarget::System(system_id) => {
            if let Some(position) = visual_position
                && matches!(navigation.mode, StrategicViewMode::Universe)
            {
                navigation.universe_focus = position;
            }
            if matches!(
                navigation.mode,
                StrategicViewMode::System(current) if current == system_id
            ) {
                navigation.system_focus = Vec3::ZERO;
            }
            if matches!(navigation.mode, StrategicViewMode::Universe)
                && enterable_selected_system(simulation, navigation.debug_full_graph).is_some()
            {
                navigation.enter_system(system_id);
                navigation.system_focus = Vec3::ZERO;
                rebuild.0 = true;
            }
        }
        PickTarget::Planet { system_id, .. } => {
            if matches!(
                navigation.mode,
                StrategicViewMode::System(current) if current == system_id
            ) && let Some(position) = visual_position
            {
                navigation.system_focus = position;
            }
        }
    }
}

pub(crate) fn pointer_tooltip_text(simulation: &Simulation, target: PickTarget) -> String {
    let state = simulation.state();
    match target {
        PickTarget::System(system_id) => {
            let level = state.system_knowledge_level(system_id);
            let title = simulation
                .universe()
                .system(system_id)
                .map(|system| {
                    if level.reveals_identity() {
                        system.name.clone()
                    } else {
                        format!("Signal {}", system_id.index())
                    }
                })
                .unwrap_or_else(|| "Système invalide".to_string());
            format!(
                "{}\n{}\nClic : sélectionner | Double-clic : ouvrir ou recentrer",
                title,
                knowledge_badge_fr(level),
            )
        }
        PickTarget::Planet { planet_id, .. } => {
            let level = state.planet_knowledge_level(planet_id);
            let title = planet_display_label(simulation, planet_id, level)
                .unwrap_or_else(|| "Planète invalide".to_string());
            format!(
                "{}\n{}\nClic : sélectionner | Double-clic : recentrer",
                title,
                knowledge_badge_fr(level),
            )
        }
    }
}

pub(crate) fn pick_target_label(simulation: &Simulation, target: PickTarget) -> String {
    let state = simulation.state();
    match target {
        PickTarget::System(system_id) => simulation
            .universe()
            .system(system_id)
            .map(|system| {
                if state.system_knowledge_level(system_id).reveals_identity() {
                    format!("Système {}", system.name)
                } else {
                    format!("Signal {}", system_id.index())
                }
            })
            .unwrap_or_else(|| format!("Système {}", system_id.index())),
        PickTarget::Planet { planet_id, .. } => simulation
            .universe_repository()
            .planet(planet_id)
            .and_then(|_| {
                planet_display_label(
                    simulation,
                    planet_id,
                    state.planet_knowledge_level(planet_id),
                )
            })
            .map(|label| format!("Planète {label}"))
            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),
    }
}

pub(crate) fn planet_display_label(
    simulation: &Simulation,
    planet_id: PlanetId,
    level: KnowledgeLevel,
) -> Option<String> {
    let (system_id, planet) = simulation
        .universe_repository()
        .planet_location(planet_id)?;
    if level.reveals_identity() {
        return Some(planet.name.clone());
    }
    let system = simulation.universe().system(system_id)?;
    let index = system
        .planets
        .iter()
        .position(|candidate| candidate.id == planet_id)?;
    Some(provisional_planet_label(&system.name, index))
}

pub(crate) fn provisional_planet_label(system_name: &str, orbit_index: usize) -> String {
    const ROMAN: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    let suffix = ROMAN
        .get(orbit_index)
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| (orbit_index + 1).to_string());
    format!("{system_name} {suffix}")
}

pub(crate) fn rank_pointer_candidates(candidates: &mut [PointerCandidate]) {
    candidates.sort_by(|left, right| {
        left.screen_distance
            .total_cmp(&right.screen_distance)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left.depth.total_cmp(&right.depth))
            .then_with(|| left.target.sort_key().cmp(&right.target.sort_key()))
    });
}

pub(crate) fn screen_space_hit(
    cursor_position: Vec2,
    target_position: Vec2,
    radius_px: f32,
) -> bool {
    cursor_position.distance_squared(target_position) <= radius_px * radius_px
}

pub(crate) fn pointer_double_click(
    previous: PointerClickRecord,
    target: PickTarget,
    now: Duration,
    cursor_position: Vec2,
) -> bool {
    previous.target == target
        && now.saturating_sub(previous.at) <= Duration::from_millis(350)
        && previous.cursor_position.distance(cursor_position) <= 6.0
}

pub(crate) fn pick_target_is_visible(simulation: &Simulation, target: PickTarget) -> bool {
    match target {
        PickTarget::System(system_id) => simulation.state().is_system_visible(system_id),
        PickTarget::Planet { planet_id, .. } => simulation
            .state()
            .planet_knowledge_level(planet_id)
            .is_visible(),
    }
}

pub(crate) fn pick_target_matches_selection(
    target: PickTarget,
    selection: SelectionTarget,
) -> bool {
    match (target, selection) {
        (PickTarget::System(left), SelectionTarget::System(right)) => left == right,
        (
            PickTarget::Planet {
                system_id: left_system,
                planet_id: left_planet,
            },
            SelectionTarget::Planet {
                system_id: right_system,
                planet_id: right_planet,
            },
        ) => left_system == right_system && left_planet == right_planet,
        _ => false,
    }
}

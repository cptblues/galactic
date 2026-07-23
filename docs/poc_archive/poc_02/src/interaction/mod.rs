pub mod picking;
pub mod selection;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::data::{
    ActiveSystem, GalaxyConfig, GalaxyData, Notifications, OrbitAnimation, SelectableId, Selection,
    ViewOptions,
};
use crate::generation::galaxy::generate_galaxy;
use crate::map::{FilterPreset, GraphicsSettings, MapFilters};
use crate::rendering::{BaseScale, VisualMaterialSet};
use crate::state::ViewState;
use crate::usability::UsabilityMetrics;

pub use picking::*;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MeshPickingPlugin)
            .init_resource::<LastClick>()
            .add_systems(
                Update,
                (
                    update_visual_selection,
                    handle_keyboard_toggles,
                    handle_view_shortcuts,
                    selection::toggle_pause.run_if(in_state(ViewState::System)),
                    update_notifications,
                    tick_orbit_animation,
                ),
            );
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Selectable {
    pub id: SelectableId,
}

#[derive(Resource, Default, Debug)]
pub struct LastClick {
    pub id: Option<SelectableId>,
    pub time: f64,
}

fn update_visual_selection(
    selection: Res<Selection>,
    mut query: Query<(
        &Selectable,
        &VisualMaterialSet,
        &BaseScale,
        &mut MeshMaterial3d<StandardMaterial>,
        &mut Transform,
    )>,
) {
    for (selectable, material_set, base_scale, mut material, mut transform) in &mut query {
        if selection.selected == Some(selectable.id) {
            material.0 = material_set.selected.clone();
            transform.scale = base_scale.0 * 1.42;
        } else if selection.hovered == Some(selectable.id) {
            material.0 = material_set.hovered.clone();
            transform.scale = base_scale.0 * 1.24;
        } else {
            material.0 = material_set.normal.clone();
            transform.scale = base_scale.0;
        }
    }
}

fn handle_keyboard_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mut options: ResMut<ViewOptions>,
    mut filters: ResMut<MapFilters>,
    mut graphics: ResMut<GraphicsSettings>,
    mut notifications: ResMut<Notifications>,
    mut metrics: ResMut<UsabilityMetrics>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if keys.just_pressed(KeyCode::KeyL) {
        options.show_routes = !options.show_routes;
        filters.major_routes = options.show_routes;
        filters.minor_routes = options.show_routes;
        metrics.filter_change_count += 1;
        notifications.show(if options.show_routes {
            "Routes visibles"
        } else {
            "Routes masquees"
        });
    }
    if keys.just_pressed(KeyCode::KeyO) {
        options.show_orbits = !options.show_orbits;
        notifications.show(if options.show_orbits {
            "Orbites visibles"
        } else {
            "Orbites masquees"
        });
    }
    if keys.just_pressed(KeyCode::KeyT) {
        options.show_labels = !options.show_labels;
        filters.labels = options.show_labels;
        metrics.filter_change_count += 1;
        notifications.show(if options.show_labels {
            "Labels visibles"
        } else {
            "Labels masques"
        });
    }
    if keys.just_pressed(KeyCode::F3) {
        options.show_debug = !options.show_debug;
    }
    if keys.just_pressed(KeyCode::F4) {
        graphics.cycle();
        notifications.show(format!("Preset graphique {:?}", graphics.preset));
    }
    if keys.just_pressed(KeyCode::F1) {
        options.show_help = !options.show_help;
        notifications.show(if options.show_help {
            "Aide visible"
        } else {
            "Aide masquee"
        });
    }
    if keys.just_pressed(KeyCode::KeyB) {
        filters.borders = !filters.borders;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Frontieres", filters.borders));
    }
    if keys.just_pressed(KeyCode::KeyI) {
        filters.influence = !filters.influence;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Influence", filters.influence));
    }
    if keys.just_pressed(KeyCode::KeyV) {
        filters.fleets = !filters.fleets;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Flottes", filters.fleets));
    }
    if keys.just_pressed(KeyCode::KeyA) {
        filters.alerts = !filters.alerts;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Alertes", filters.alerts));
    }
    if keys.just_pressed(KeyCode::KeyG) {
        filters.anomalies = !filters.anomalies;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Anomalies", filters.anomalies));
    }
    if keys.just_pressed(KeyCode::KeyH) {
        filters.habitable_worlds = !filters.habitable_worlds;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Habitables", filters.habitable_worlds));
    }
    if keys.just_pressed(KeyCode::KeyU) {
        filters.unknown_systems = !filters.unknown_systems;
        metrics.filter_change_count += 1;
        notifications.show(layer_message("Inconnus", filters.unknown_systems));
    }
    if !ctrl && keys.just_pressed(KeyCode::Digit1) {
        filters.apply_preset(FilterPreset::Exploration);
        metrics.filter_change_count += 1;
        notifications.show("Preset Exploration");
    }
    if !ctrl && keys.just_pressed(KeyCode::Digit2) {
        filters.apply_preset(FilterPreset::Diplomacy);
        metrics.filter_change_count += 1;
        notifications.show("Preset Diplomatie");
    }
    if !ctrl && keys.just_pressed(KeyCode::Digit3) {
        filters.apply_preset(FilterPreset::Navigation);
        metrics.filter_change_count += 1;
        notifications.show("Preset Navigation");
    }
    if !ctrl && keys.just_pressed(KeyCode::Digit4) {
        filters.apply_preset(FilterPreset::Minimal);
        metrics.filter_change_count += 1;
        notifications.show("Preset Minimal");
    }
}

fn layer_message(name: &'static str, enabled: bool) -> String {
    format!("{name} {}", if enabled { "visibles" } else { "masques" })
}

#[derive(SystemParam)]
struct ShortcutParams<'w> {
    state: Res<'w, State<ViewState>>,
    next_state: ResMut<'w, NextState<ViewState>>,
    config: ResMut<'w, GalaxyConfig>,
    galaxy: ResMut<'w, GalaxyData>,
    selection: ResMut<'w, Selection>,
    active_system: ResMut<'w, ActiveSystem>,
    notifications: ResMut<'w, Notifications>,
    metrics: ResMut<'w, UsabilityMetrics>,
}

fn handle_view_shortcuts(keys: Res<ButtonInput<KeyCode>>, mut params: ShortcutParams) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::Digit1) {
        regenerate_with_system_count(100, &mut params);
    }
    if ctrl && keys.just_pressed(KeyCode::Digit2) {
        regenerate_with_system_count(500, &mut params);
    }
    if ctrl && keys.just_pressed(KeyCode::Digit3) {
        regenerate_with_system_count(1_000, &mut params);
    }

    if keys.just_pressed(KeyCode::KeyR) {
        let new_galaxy = generate_galaxy(&params.config);
        *params.galaxy = new_galaxy;
        params.selection.clear();
        params.active_system.id = None;
        let seed = params.config.seed;
        params
            .notifications
            .show(format!("Galaxie regeneree - graine {seed}"));
        if *params.state.get() == ViewState::System {
            params.next_state.set(ViewState::Galaxy);
        }
    }

    if keys.just_pressed(KeyCode::KeyN) {
        let previous_seed = params.config.seed;
        params.config.seed = fresh_seed(previous_seed);
        let new_galaxy = generate_galaxy(&params.config);
        *params.galaxy = new_galaxy;
        params.selection.clear();
        params.active_system.id = None;
        let seed = params.config.seed;
        params
            .notifications
            .show(format!("Galaxie generee - graine {seed}"));
        if *params.state.get() == ViewState::System {
            params.next_state.set(ViewState::Galaxy);
        }
    }

    match *params.state.get() {
        ViewState::Galaxy => {
            if keys.just_pressed(KeyCode::Enter) {
                if let Some(system_id) = params.selection.selected_system() {
                    params.active_system.id = Some(system_id);
                    params.next_state.set(ViewState::System);
                    params.metrics.view_transition_count += 1;
                    params.notifications.show("Ouverture du systeme");
                } else {
                    params.notifications.show("Aucun systeme selectionne");
                }
            }
        }
        ViewState::System => {
            if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Backspace) {
                params.selection.clear();
                params.next_state.set(ViewState::Galaxy);
                params.metrics.view_transition_count += 1;
                params.notifications.show("Retour galaxie");
            }
        }
    }
}

fn regenerate_with_system_count(count: usize, params: &mut ShortcutParams) {
    params.config.system_count = count;
    let new_galaxy = generate_galaxy(&params.config);
    *params.galaxy = new_galaxy;
    params.selection.clear();
    params.active_system.id = None;
    params.notifications.show(format!("{count} systemes"));
    if *params.state.get() == ViewState::System {
        params.next_state.set(ViewState::Galaxy);
    }
}

fn update_notifications(time: Res<Time>, mut notifications: ResMut<Notifications>) {
    if notifications.remaining > 0.0 {
        notifications.remaining -= time.delta_secs();
    }
    if notifications.remaining <= 0.0 {
        notifications.message = None;
        notifications.remaining = 0.0;
    }
}

fn tick_orbit_animation(time: Res<Time>, mut animation: ResMut<OrbitAnimation>) {
    if !animation.paused {
        animation.elapsed += time.delta_secs();
    }
}

fn fresh_seed(previous: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(previous.wrapping_mul(6364136223846793005));
    nanos ^ previous.rotate_left(17)
}

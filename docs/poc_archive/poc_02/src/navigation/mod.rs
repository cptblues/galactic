pub mod breadcrumb;
pub mod history;
pub mod search;

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::camera::{MainCamera, OrbitCamera};
use crate::data::{ActiveSystem, GalaxyData, SelectableId, Selection, SystemId};
use crate::map::LabelHighlight;
use crate::state::ViewState;
use crate::strategic::{StrategicGalaxyData, build_adjacency, shortest_path};
use crate::usability::UsabilityMetrics;

pub use breadcrumb::*;
pub use history::*;
pub use search::*;

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NavigationHistory>()
            .init_resource::<SearchState>()
            .init_resource::<HighlightedPath>()
            .add_systems(
                Update,
                (
                    handle_search_input,
                    handle_history_keys,
                    handle_home_key,
                    handle_path_key,
                ),
            );
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct HighlightedPath {
    pub systems: Vec<SystemId>,
}

#[derive(SystemParam)]
struct NavParams<'w, 's> {
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    selection: ResMut<'w, Selection>,
    active_system: ResMut<'w, ActiveSystem>,
    next_state: ResMut<'w, NextState<ViewState>>,
    camera: Query<'w, 's, &'static mut OrbitCamera, With<MainCamera>>,
    notifications: ResMut<'w, crate::data::Notifications>,
    history: ResMut<'w, NavigationHistory>,
    metrics: ResMut<'w, UsabilityMetrics>,
}

fn handle_history_keys(keys: Res<ButtonInput<KeyCode>>, mut params: NavParams) {
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if alt
        && keys.just_pressed(KeyCode::ArrowLeft)
        && let Some(entry) = params.history.previous()
    {
        apply_history_entry(entry, &mut params);
        params.metrics.navigation_back_count += 1;
    }
    if alt
        && keys.just_pressed(KeyCode::ArrowRight)
        && let Some(entry) = params.history.next()
    {
        apply_history_entry(entry, &mut params);
    }
}

fn handle_home_key(keys: Res<ButtonInput<KeyCode>>, mut params: NavParams) {
    if !keys.just_pressed(KeyCode::Home) {
        return;
    }
    let Some(origin) = params.strategic.origin else {
        params.notifications.show("Origine introuvable");
        return;
    };
    focus_system(origin, "Origine", &mut params);
}

fn handle_path_key(
    keys: Res<ButtonInput<KeyCode>>,
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    selection: Res<Selection>,
    mut highlighted: ResMut<HighlightedPath>,
    mut notifications: ResMut<crate::data::Notifications>,
) {
    if !keys.just_pressed(KeyCode::KeyK) {
        return;
    }
    let Some(origin) = strategic.origin else {
        notifications.show("Origine introuvable");
        return;
    };
    let Some(target) = selection.selected_system() else {
        notifications.show("Aucun systeme selectionne");
        return;
    };
    let adjacency = build_adjacency(&galaxy);
    if let Some(path) = shortest_path(&adjacency, origin, target) {
        highlighted.systems = path;
        notifications.show("Chemin affiche");
    } else {
        highlighted.systems.clear();
        notifications.show("Aucun chemin trouve");
    }
}

#[derive(SystemParam)]
struct SearchParams<'w, 's> {
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    search: ResMut<'w, SearchState>,
    highlight: ResMut<'w, LabelHighlight>,
    selection: ResMut<'w, Selection>,
    active_system: ResMut<'w, ActiveSystem>,
    next_state: ResMut<'w, NextState<ViewState>>,
    camera: Query<'w, 's, &'static mut OrbitCamera, With<MainCamera>>,
    notifications: ResMut<'w, crate::data::Notifications>,
    history: ResMut<'w, NavigationHistory>,
    metrics: ResMut<'w, UsabilityMetrics>,
}

fn handle_search_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    mut params: SearchParams,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if keys.just_pressed(KeyCode::Slash) || (ctrl && keys.just_pressed(KeyCode::KeyF)) {
        params.search.active = true;
        params.search.query.clear();
        params.search.results.clear();
        params.notifications.show("Recherche active");
        return;
    }
    if !params.search.active {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        params.search.active = false;
        params.notifications.show("Recherche fermee");
        return;
    }
    if keys.just_pressed(KeyCode::Backspace) {
        params.search.query.pop();
    }
    for event in key_events.read() {
        if event.state != ButtonState::Pressed || event.key_code == KeyCode::Slash {
            continue;
        }
        if let Some(text) = &event.text {
            for ch in text.chars().filter(|ch| !ch.is_control()) {
                if params.search.query.len() < 40 {
                    params.search.query.push(ch);
                }
            }
        }
    }
    params.search.results =
        search::search_galaxy(&params.galaxy, &params.strategic, &params.search.query);
    if keys.just_pressed(KeyCode::Enter) {
        let Some(result) = params.search.results.first().cloned() else {
            params.notifications.show("Aucun resultat");
            return;
        };
        params.search.active = false;
        params.metrics.search_count += 1;
        params.highlight.result = Some(result.id);
        apply_search_result(result, &mut params);
    }
}

fn apply_search_result(result: SearchResult, params: &mut SearchParams) {
    match result.id {
        SelectableId::System(system_id) | SelectableId::Star(system_id) => {
            params.selection.selected = Some(SelectableId::System(system_id));
            params.active_system.id = None;
            params.next_state.set(ViewState::Galaxy);
            if let Ok(mut camera) = params.camera.single_mut()
                && let Some(system) = params.galaxy.find_system(system_id)
            {
                camera.target_focus = system.position;
                camera.target_distance = 48.0;
            }
            params.history.push(NavigationEntry::new(
                result.label.clone(),
                ViewState::Galaxy,
                Some(SelectableId::System(system_id)),
                params
                    .galaxy
                    .find_system(system_id)
                    .map(|system| system.position)
                    .unwrap_or(Vec3::ZERO),
                48.0,
                None,
            ));
        }
        SelectableId::Planet(system_id, _) | SelectableId::Moon(system_id, _, _) => {
            params.selection.selected = Some(result.id);
            params.active_system.id = Some(system_id);
            params.next_state.set(ViewState::System);
            params.history.push(NavigationEntry::new(
                result.label.clone(),
                ViewState::System,
                Some(result.id),
                Vec3::ZERO,
                55.0,
                Some(system_id),
            ));
        }
    }
    params
        .notifications
        .show(format!("Resultat: {}", result.label));
}

fn focus_system(system_id: SystemId, label: &str, params: &mut NavParams) {
    let Some(system) = params.galaxy.find_system(system_id) else {
        params.notifications.show("Systeme introuvable");
        return;
    };
    params.selection.selected = Some(SelectableId::System(system_id));
    params.active_system.id = None;
    params.next_state.set(ViewState::Galaxy);
    if let Ok(mut camera) = params.camera.single_mut() {
        camera.target_focus = system.position;
        camera.target_distance = 58.0;
    }
    params.history.push(NavigationEntry::new(
        label.to_string(),
        ViewState::Galaxy,
        Some(SelectableId::System(system_id)),
        system.position,
        58.0,
        None,
    ));
    params.notifications.show(label);
}

fn apply_history_entry(entry: NavigationEntry, params: &mut NavParams) {
    params.selection.selected = entry.selection;
    params.active_system.id = entry.active_system;
    params.next_state.set(entry.view);
    if let Ok(mut camera) = params.camera.single_mut() {
        camera.target_focus = entry.focus;
        camera.target_distance = entry.distance;
    }
    params
        .notifications
        .show(format!("Historique: {}", entry.label));
}

use bevy::window::PrimaryWindow;
use bevy::{ecs::system::SystemParam, prelude::*};

use crate::camera::MainCamera;
use crate::data::{GalaxyData, Notifications, SelectableId, Selection, SystemId};
use crate::strategic::{ControlState, StrategicGalaxyData};
use crate::usability::UsabilityMetrics;

#[derive(Resource, Clone, Debug, Default)]
pub struct AmbiguousSelection {
    pub active: bool,
    pub candidates: Vec<AmbiguousCandidate>,
    pub index: usize,
    pub expires_at: f64,
    pub help_shown: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AmbiguousCandidate {
    pub id: SelectableId,
    pub label: String,
    pub screen_distance: f32,
    pub depth: f32,
    pub priority: i32,
}

#[derive(SystemParam)]
pub struct ScreenSelectionParams<'w, 's> {
    time: Res<'w, Time>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera_query: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    selection: ResMut<'w, Selection>,
    ambiguous: ResMut<'w, AmbiguousSelection>,
    metrics: ResMut<'w, UsabilityMetrics>,
    notifications: ResMut<'w, Notifications>,
}

pub fn screen_space_selection(
    mouse: Res<ButtonInput<MouseButton>>,
    mut params: ScreenSelectionParams,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = params.windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = params.camera_query.single() else {
        return;
    };

    let mut candidates = Vec::new();
    for system in &params.galaxy.systems {
        let Ok(screen) = camera.world_to_viewport_with_depth(camera_transform, system.position)
        else {
            continue;
        };
        let distance = screen.truncate().distance(cursor);
        if distance > 16.0 {
            continue;
        }
        candidates.push(AmbiguousCandidate {
            id: SelectableId::System(system.id),
            label: system.name.clone(),
            screen_distance: distance,
            depth: screen.z,
            priority: strategic_priority(system.id, &params.strategic),
        });
    }

    candidates = rank_candidates(candidates);
    if candidates.is_empty() {
        return;
    }
    if candidates.len() == 1 || candidates[1].screen_distance - candidates[0].screen_distance > 4.0
    {
        params.selection.selected = Some(candidates[0].id);
        params.ambiguous.active = false;
        return;
    }

    candidates.truncate(6);
    params.selection.selected = Some(candidates[0].id);
    params.ambiguous.active = true;
    params.ambiguous.index = 0;
    params.ambiguous.expires_at = params.time.elapsed_secs_f64() + 1.5;
    params.ambiguous.candidates = candidates;
    params.metrics.ambiguous_selection_count += 1;
    if !params.ambiguous.help_shown {
        params.ambiguous.help_shown = true;
        params
            .notifications
            .show("Selection ambigue: Tab parcourt les candidats");
    }
}

pub fn cycle_ambiguous_selection(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut selection: ResMut<Selection>,
    mut ambiguous: ResMut<AmbiguousSelection>,
) {
    if ambiguous.active && time.elapsed_secs_f64() > ambiguous.expires_at {
        ambiguous.active = false;
    }
    if keys.just_pressed(KeyCode::Escape) && ambiguous.active {
        ambiguous.active = false;
        return;
    }
    if !ambiguous.active || !keys.just_pressed(KeyCode::Tab) || ambiguous.candidates.is_empty() {
        return;
    }
    ambiguous.index = (ambiguous.index + 1) % ambiguous.candidates.len();
    selection.selected = Some(ambiguous.candidates[ambiguous.index].id);
    ambiguous.expires_at = time.elapsed_secs_f64() + 1.5;
}

pub fn rank_candidates(mut candidates: Vec<AmbiguousCandidate>) -> Vec<AmbiguousCandidate> {
    candidates.sort_by(|a, b| {
        a.screen_distance
            .total_cmp(&b.screen_distance)
            .then_with(|| b.priority.cmp(&a.priority))
            .then_with(|| a.depth.total_cmp(&b.depth))
            .then_with(|| format!("{:?}", a.id).cmp(&format!("{:?}", b.id)))
    });
    candidates
}

fn strategic_priority(system: SystemId, strategic: &StrategicGalaxyData) -> i32 {
    let Some(state) = strategic.system_states.get(&system) else {
        return 0;
    };
    let mut priority = 0;
    if matches!(state.control, ControlState::Capital(_)) {
        priority += 80;
    }
    if state.control.is_colonized() {
        priority += 20;
    }
    priority += state.alerts.len() as i32 * 12;
    priority
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_rank_by_distance_then_priority() {
        let ranked = rank_candidates(vec![
            AmbiguousCandidate {
                id: SelectableId::System(SystemId(1)),
                label: "A".to_string(),
                screen_distance: 8.0,
                depth: 10.0,
                priority: 0,
            },
            AmbiguousCandidate {
                id: SelectableId::System(SystemId(2)),
                label: "B".to_string(),
                screen_distance: 8.0,
                depth: 9.0,
                priority: 50,
            },
        ]);
        assert_eq!(ranked[0].id, SelectableId::System(SystemId(2)));
    }
}

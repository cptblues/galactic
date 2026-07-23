use bevy::{ecs::system::SystemParam, prelude::*};
use std::collections::HashSet;

use crate::camera::MainCamera;
use crate::data::{GalaxyData, SelectableId, Selection, SystemId};
use crate::map::{MapFilters, SemanticZoomState};
use crate::strategic::{AlertSeverity, ControlState, StrategicGalaxyData};
use crate::views::galaxy::GalaxyLabel;

#[derive(Resource, Clone, Debug, Default)]
pub struct LabelDiagnostics {
    pub candidates: usize,
    pub visible: usize,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct LabelHighlight {
    pub result: Option<SelectableId>,
}

#[derive(Clone, Debug)]
struct LabelCandidate {
    id: SystemId,
    score: i32,
    rect: Rect,
    selected: bool,
}

#[derive(SystemParam)]
pub struct LabelParams<'w, 's> {
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    filters: Res<'w, MapFilters>,
    zoom: Res<'w, SemanticZoomState>,
    selection: Res<'w, Selection>,
    highlight: Res<'w, LabelHighlight>,
    camera_query: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
}

pub fn update_dynamic_labels(
    params: LabelParams,
    mut diagnostics: ResMut<LabelDiagnostics>,
    mut labels: Query<(&mut GalaxyLabel, &mut Visibility)>,
) {
    let Ok((camera, camera_transform)) = params.camera_query.single() else {
        return;
    };

    if !params.filters.labels {
        diagnostics.candidates = 0;
        diagnostics.visible = 0;
        for (_, mut visibility) in &mut labels {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let mut candidates = Vec::new();
    for (label, _) in &labels {
        let Some(system) = params.galaxy.find_system(label.id) else {
            continue;
        };
        let Ok(screen) = camera.world_to_viewport(camera_transform, system.position) else {
            continue;
        };
        let selected = params
            .selection
            .selected
            .map(SelectableId::system_id)
            .map(|id| id == label.id)
            .unwrap_or(false);
        let score = label_score(
            label.id,
            &params.galaxy,
            &params.strategic,
            &params.selection,
            params.highlight.result,
            label.was_visible,
        );
        if score <= 0 && !selected {
            continue;
        }
        let width = 42.0 + label.name.len() as f32 * 7.0;
        candidates.push(LabelCandidate {
            id: label.id,
            score,
            rect: Rect::from_center_size(screen, Vec2::new(width, 18.0)),
            selected,
        });
    }
    candidates.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    diagnostics.candidates = candidates.len();

    let budget = params.zoom.level.label_budget();
    let mut accepted = Vec::<LabelCandidate>::new();
    for candidate in candidates {
        let collides = accepted
            .iter()
            .any(|accepted| rects_overlap(candidate.rect, accepted.rect));
        if candidate.selected || (!collides && accepted.len() < budget) {
            accepted.push(candidate);
        }
    }
    let visible_ids = accepted
        .iter()
        .map(|candidate| candidate.id)
        .collect::<HashSet<_>>();
    diagnostics.visible = visible_ids.len();

    for (mut label, mut visibility) in &mut labels {
        label.was_visible = visible_ids.contains(&label.id);
        *visibility = if label.was_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn label_score(
    id: SystemId,
    galaxy: &GalaxyData,
    strategic: &StrategicGalaxyData,
    selection: &Selection,
    highlight: Option<SelectableId>,
    was_visible: bool,
) -> i32 {
    let mut score = 0;
    if selection.selected.map(SelectableId::system_id) == Some(id) {
        score += 1000;
    }
    if highlight.map(SelectableId::system_id) == Some(id) {
        score += 900;
    }
    let Some(system) = galaxy.find_system(id) else {
        return score;
    };
    if let Some(state) = strategic.system_states.get(&id) {
        if matches!(state.control, ControlState::Capital(_)) {
            score += 800;
        }
        if state
            .alerts
            .iter()
            .filter_map(|alert_id| strategic.alerts.iter().find(|alert| alert.id == *alert_id))
            .any(|alert| alert.severity == AlertSeverity::Critical)
        {
            score += 700;
        }
        if state.control.is_colonized() {
            score += 200;
        }
    }
    if system.planets.len() >= 7 || system.tags.has_habitable_world {
        score += 500;
    }
    if selection.hovered.map(SelectableId::system_id) == Some(id) {
        score += 280;
    }
    if was_visible {
        score += 45;
    }
    score
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_rects_are_detected() {
        let a = Rect::from_center_size(Vec2::ZERO, Vec2::splat(10.0));
        let b = Rect::from_center_size(Vec2::new(4.0, 0.0), Vec2::splat(10.0));
        let c = Rect::from_center_size(Vec2::new(30.0, 0.0), Vec2::splat(10.0));
        assert!(rects_overlap(a, b));
        assert!(!rects_overlap(a, c));
    }
}

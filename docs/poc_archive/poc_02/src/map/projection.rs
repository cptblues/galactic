use bevy::prelude::*;

use crate::data::Notifications;
use crate::state::ViewState;
use crate::views::GalaxyViewEntity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapProjectionMode {
    ThreeDimensional,
    Flattened,
}

#[derive(Resource, Clone, Debug)]
pub struct MapProjectionState {
    pub mode: MapProjectionMode,
    pub blend: f32,
    pub target_blend: f32,
}

impl Default for MapProjectionState {
    fn default() -> Self {
        Self {
            mode: MapProjectionMode::ThreeDimensional,
            blend: 0.0,
            target_blend: 0.0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct GalaxyMapPosition {
    pub original: Vec3,
}

pub fn toggle_projection(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<ViewState>>,
    mut projection: ResMut<MapProjectionState>,
    mut notifications: ResMut<Notifications>,
) {
    if *state.get() != ViewState::Galaxy || !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    projection.mode = match projection.mode {
        MapProjectionMode::ThreeDimensional => MapProjectionMode::Flattened,
        MapProjectionMode::Flattened => MapProjectionMode::ThreeDimensional,
    };
    projection.target_blend = match projection.mode {
        MapProjectionMode::ThreeDimensional => 0.0,
        MapProjectionMode::Flattened => 1.0,
    };
    notifications.show(match projection.mode {
        MapProjectionMode::ThreeDimensional => "Projection 3D",
        MapProjectionMode::Flattened => "Projection aplatie",
    });
}

pub fn update_projection(time: Res<Time>, mut projection: ResMut<MapProjectionState>) {
    let alpha = 1.0 - (-8.0 * time.delta_secs()).exp();
    projection.blend = projection
        .blend
        .lerp(projection.target_blend, alpha)
        .clamp(0.0, 1.0);
}

pub fn apply_galaxy_projection(
    projection: Res<MapProjectionState>,
    mut query: Query<(&GalaxyMapPosition, &mut Transform), With<GalaxyViewEntity>>,
) {
    if !projection.is_changed() {
        return;
    }
    for (position, mut transform) in &mut query {
        transform.translation = projected_position(position.original, projection.blend);
    }
}

pub fn projected_position(position: Vec3, blend: f32) -> Vec3 {
    Vec3::new(
        position.x,
        position.y * (1.0 - blend.clamp(0.0, 1.0)),
        position.z,
    )
}

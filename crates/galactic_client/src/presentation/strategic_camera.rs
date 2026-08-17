use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

use crate::presentation::components::*;
use crate::presentation::strategic_navigation::*;
use crate::*;

pub(crate) fn tick_simulation(time: Res<Time>, mut simulation: ResMut<SimulationResource>) {
    let events = simulation.simulation.advance(time.delta());
    simulation.pending_events.extend(events);
}

#[derive(SystemParam)]
pub(crate) struct StrategicCameraInput<'w> {
    time: Res<'w, Time>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    mouse_motion: Res<'w, AccumulatedMouseMotion>,
    mouse_scroll: Res<'w, AccumulatedMouseScroll>,
    open_panel: Res<'w, OpenPanel>,
    windows: Res<'w, OpenWindows>,
    intro_pitch: Res<'w, IntroPitchUiState>,
    victory: Res<'w, VictoryUiState>,
}

pub(crate) fn update_strategic_camera(
    input: StrategicCameraInput,
    mut navigation: ResMut<StrategicNavigation>,
    mut query: Query<&mut Transform, With<StrategicCamera>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };
    // Colony/Research/Craft/Fleet are floating `GameWindowKind` windows tracked by
    // `OpenWindows`, not `OpenPanel` — the latter resets to `None` whenever one of
    // them opens, so matching on `OpenPanel` here never actually blocked the camera
    // for those four (playtest feedback: scroll/keys over the Fleet window leaked
    // through to the background map). Navigation/Objectives are non-window overlays
    // that genuinely do live in `OpenPanel`, so both checks are needed.
    if input.intro_pitch.visible
        || input.victory.visible
        || matches!(
            *input.open_panel,
            OpenPanel::Navigation | OpenPanel::Objectives
        )
        || input.windows.any_visible()
    {
        return;
    }

    let delta_seconds = input.time.delta_secs();
    let motion = input.mouse_motion.delta;
    let scroll_lines = match input.mouse_scroll.unit {
        MouseScrollUnit::Line => input.mouse_scroll.delta.y,
        MouseScrollUnit::Pixel => input.mouse_scroll.delta.y / 40.0,
    };

    match navigation.mode {
        StrategicViewMode::Universe => {
            if input.mouse_buttons.pressed(MouseButton::Right) {
                let mut yaw = navigation.universe_yaw;
                let mut pitch = navigation.universe_pitch;
                apply_orbit_drag(&mut yaw, &mut pitch, motion);
                if navigation.universe_yaw != yaw {
                    navigation.universe_yaw = yaw;
                }
                if navigation.universe_pitch != pitch {
                    navigation.universe_pitch = pitch;
                }
            }
            if input.mouse_buttons.pressed(MouseButton::Middle) {
                let pan = mouse_pan_delta(
                    navigation.universe_yaw,
                    motion,
                    navigation.universe_distance,
                );
                if pan != Vec3::ZERO {
                    navigation.universe_focus += pan;
                }
            }

            let keyboard_pan = keyboard_pan_direction(&input.keyboard, navigation.universe_yaw);
            if keyboard_pan.length_squared() > 0.0 {
                let pan_speed = (navigation.universe_distance * 0.55).max(18.0);
                navigation.universe_focus += keyboard_pan.normalize() * pan_speed * delta_seconds;
            }

            let maximum = navigation.universe_max_distance;
            let mut next_distance = navigation.universe_distance;
            apply_keyboard_zoom(
                &input.keyboard,
                delta_seconds,
                &mut next_distance,
                20.0,
                maximum,
            );
            apply_scroll_zoom(&mut next_distance, scroll_lines, 20.0, maximum);
            if navigation.universe_distance != next_distance {
                navigation.universe_distance = next_distance;
            }
            let next_lod = UniverseLod::from_distance(next_distance);
            if navigation.lod != next_lod {
                navigation.lod = next_lod;
            }

            let next_transform = orbit_transform(
                navigation.universe_focus,
                next_distance,
                navigation.universe_yaw,
                navigation.universe_pitch,
            );
            if *transform != next_transform {
                *transform = next_transform;
            }
        }
        StrategicViewMode::System(_) => {
            if input.mouse_buttons.pressed(MouseButton::Right) {
                let mut yaw = navigation.system_yaw;
                let mut pitch = navigation.system_pitch;
                apply_orbit_drag(&mut yaw, &mut pitch, motion);
                if navigation.system_yaw != yaw {
                    navigation.system_yaw = yaw;
                }
                if navigation.system_pitch != pitch {
                    navigation.system_pitch = pitch;
                }
            }
            if input.mouse_buttons.pressed(MouseButton::Middle) {
                let pan =
                    mouse_pan_delta(navigation.system_yaw, motion, navigation.system_distance);
                if pan != Vec3::ZERO {
                    navigation.system_focus += pan;
                }
            }

            let keyboard_pan = keyboard_pan_direction(&input.keyboard, navigation.system_yaw);
            if keyboard_pan.length_squared() > 0.0 {
                let pan_speed = (navigation.system_distance * 0.42).max(8.0);
                navigation.system_focus += keyboard_pan.normalize() * pan_speed * delta_seconds;
            }

            let mut next_distance = navigation.system_distance;
            apply_keyboard_zoom(
                &input.keyboard,
                delta_seconds,
                &mut next_distance,
                10.0,
                80.0,
            );
            apply_scroll_zoom(&mut next_distance, scroll_lines, 10.0, 80.0);
            if navigation.system_distance != next_distance {
                navigation.system_distance = next_distance;
            }

            let next_transform = orbit_transform(
                navigation.system_focus,
                next_distance,
                navigation.system_yaw,
                navigation.system_pitch,
            );
            if *transform != next_transform {
                *transform = next_transform;
            }
        }
    }
}

pub(crate) fn apply_orbit_drag(yaw: &mut f32, pitch: &mut f32, motion: Vec2) {
    const SENSITIVITY: f32 = 0.006;
    *yaw -= motion.x * SENSITIVITY;
    *pitch = (*pitch - motion.y * SENSITIVITY).clamp(-1.35, 1.35);
}

pub(crate) fn mouse_pan_delta(yaw: f32, motion: Vec2, distance: f32) -> Vec3 {
    if motion == Vec2::ZERO {
        return Vec3::ZERO;
    }

    let yaw_rotation = Quat::from_rotation_y(yaw);
    let right = yaw_rotation * Vec3::X;
    let forward = yaw_rotation * -Vec3::Z;
    let scale = (distance * 0.0028).max(0.025);

    (-motion.x * right + motion.y * forward) * scale
}

pub(crate) fn keyboard_pan_direction(keyboard: &ButtonInput<KeyCode>, yaw: f32) -> Vec3 {
    let mut input = Vec2::ZERO;
    if keyboard.pressed(AZERTY_LEFT_KEY) {
        input.x -= 1.0;
    }
    if keyboard.pressed(AZERTY_RIGHT_KEY) {
        input.x += 1.0;
    }
    if keyboard.pressed(AZERTY_FORWARD_KEY) {
        input.y += 1.0;
    }
    if keyboard.pressed(AZERTY_BACKWARD_KEY) {
        input.y -= 1.0;
    }

    let rotation = Quat::from_rotation_y(yaw);
    rotation * Vec3::new(input.x, 0.0, -input.y)
}

pub(crate) fn apply_keyboard_zoom(
    keyboard: &ButtonInput<KeyCode>,
    delta_seconds: f32,
    distance: &mut f32,
    minimum: f32,
    maximum: f32,
) {
    let zoom_speed = (*distance * 0.85).max(12.0);
    if keyboard.pressed(AZERTY_ZOOM_IN_KEY) {
        *distance -= zoom_speed * delta_seconds;
    }
    if keyboard.pressed(AZERTY_ZOOM_OUT_KEY) {
        *distance += zoom_speed * delta_seconds;
    }
    *distance = (*distance).clamp(minimum, maximum);
}

pub(crate) fn apply_scroll_zoom(distance: &mut f32, scroll_lines: f32, minimum: f32, maximum: f32) {
    if scroll_lines == 0.0 {
        return;
    }

    *distance *= (-scroll_lines * 0.12).exp();
    *distance = (*distance).clamp(minimum, maximum);
}

pub(crate) fn orbit_transform(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Transform {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    let eye = focus + rotation * Vec3::new(0.0, 0.0, distance);
    Transform::from_translation(eye).looking_at(focus, Vec3::Y)
}

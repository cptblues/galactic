pub mod controller;
pub mod transition;

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::data::ActiveSystem;
use crate::interaction::Selectable;
use crate::state::ViewState;

pub use controller::*;
pub use transition::*;

pub struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraTransition>()
            .add_systems(Startup, spawn_main_camera)
            .add_systems(
                Update,
                (
                    handle_orbit_camera_input,
                    focus_selected_camera,
                    apply_camera_transition,
                    apply_orbit_camera,
                )
                    .chain(),
            )
            .add_systems(OnEnter(ViewState::Galaxy), configure_galaxy_camera)
            .add_systems(OnEnter(ViewState::System), configure_system_camera);
    }
}

#[derive(Component)]
pub struct MainCamera;

fn spawn_main_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.005, 0.007, 0.014)),
            ..default()
        },
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 85.0, 145.0).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera::galaxy(),
        MainCamera,
    ));
}

fn handle_orbit_camera_input(
    mut motion_reader: MessageReader<MouseMotion>,
    mut wheel_reader: MessageReader<MouseWheel>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut OrbitCamera, &Transform), With<MainCamera>>,
) {
    let Ok((mut camera, transform)) = query.single_mut() else {
        return;
    };
    if !camera.input_enabled {
        return;
    }

    let delta = motion_reader
        .read()
        .fold(Vec2::ZERO, |acc, event| acc + event.delta);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if mouse_buttons.pressed(MouseButton::Right) && !shift {
        camera.target_yaw -= delta.x * camera.rotate_sensitivity;
        camera.target_pitch =
            (camera.target_pitch - delta.y * camera.rotate_sensitivity).clamp(-1.45, -0.08);
    } else if mouse_buttons.pressed(MouseButton::Middle)
        || (mouse_buttons.pressed(MouseButton::Right) && shift)
    {
        let right = transform.rotation * Vec3::X;
        let up = transform.rotation * Vec3::Y;
        let scale = camera.target_distance * camera.pan_sensitivity;
        camera.target_focus += (-right * delta.x + up * delta.y) * scale;
    }

    for wheel in wheel_reader.read() {
        let unit_scale = match wheel.unit {
            MouseScrollUnit::Line => 1.0,
            MouseScrollUnit::Pixel => 0.02,
        };
        let zoom = 1.0 - wheel.y * unit_scale * camera.zoom_sensitivity;
        camera.target_distance =
            (camera.target_distance * zoom).clamp(camera.min_distance, camera.max_distance);
    }
}

fn apply_orbit_camera(
    time: Res<Time>,
    mut query: Query<(&mut OrbitCamera, &mut Transform), With<MainCamera>>,
) {
    let Ok((mut camera, mut transform)) = query.single_mut() else {
        return;
    };
    let alpha = 1.0 - (-camera.smoothing * time.delta_secs()).exp();
    camera.focus = camera.focus.lerp(camera.target_focus, alpha);
    camera.yaw = camera.yaw.lerp(camera.target_yaw, alpha);
    camera.pitch = camera.pitch.lerp(camera.target_pitch, alpha);
    camera.distance = camera
        .distance
        .lerp(camera.target_distance, alpha)
        .clamp(camera.min_distance, camera.max_distance);

    let yaw = Quat::from_rotation_y(camera.yaw);
    let pitch = Quat::from_rotation_x(camera.pitch);
    let direction = yaw * pitch * Vec3::new(0.0, 0.0, 1.0);
    transform.translation = camera.focus + direction * camera.distance;
    transform.look_at(camera.focus, Vec3::Y);
}

fn focus_selected_camera(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<crate::data::Selection>,
    selectable_query: Query<(&Selectable, &GlobalTransform)>,
    mut camera_query: Query<&mut OrbitCamera, With<MainCamera>>,
) {
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(selected) = selection.selected else {
        return;
    };
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };
    for (selectable, transform) in &selectable_query {
        if selectable.id == selected {
            camera.target_focus = transform.translation();
            camera.target_distance =
                (camera.target_distance * 0.55).clamp(camera.min_distance, camera.max_distance);
            break;
        }
    }
}

fn apply_camera_transition(
    time: Res<Time>,
    mut transition: ResMut<CameraTransition>,
    mut next_state: ResMut<NextState<ViewState>>,
    mut query: Query<&mut OrbitCamera, With<MainCamera>>,
) {
    if !transition.active {
        return;
    }
    let Ok(mut camera) = query.single_mut() else {
        return;
    };

    transition.elapsed += time.delta_secs();
    let t = (transition.elapsed / transition.duration.max(0.001)).clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let pose = CameraPose {
        focus: transition.from.focus.lerp(transition.to.focus, eased),
        yaw: transition.from.yaw.lerp(transition.to.yaw, eased),
        pitch: transition.from.pitch.lerp(transition.to.pitch, eased),
        distance: transition.from.distance.lerp(transition.to.distance, eased),
    };
    camera.set_pose(pose);
    camera.input_enabled = false;

    if t >= 1.0 {
        transition.active = false;
        camera.input_enabled = true;
        if let Some(view) = transition.pending_view.take() {
            next_state.set(view);
        }
    }
}

fn configure_galaxy_camera(
    active_system: Res<ActiveSystem>,
    galaxy: Res<crate::data::GalaxyData>,
    mut query: Query<&mut OrbitCamera, With<MainCamera>>,
) {
    let Ok(mut camera) = query.single_mut() else {
        return;
    };
    let focus = active_system
        .id
        .and_then(|id| galaxy.find_system(id).map(|system| system.position))
        .unwrap_or(Vec3::ZERO);
    camera.set_bounds(8.0, 400.0);
    camera.target_focus = focus;
    camera.target_distance = if active_system.id.is_some() {
        72.0
    } else {
        170.0
    };
    camera.target_pitch = -0.55;
    camera.input_enabled = true;
}

fn configure_system_camera(mut query: Query<&mut OrbitCamera, With<MainCamera>>) {
    let Ok(mut camera) = query.single_mut() else {
        return;
    };
    camera.set_bounds(2.0, 150.0);
    camera.focus = Vec3::ZERO;
    camera.target_focus = Vec3::ZERO;
    camera.distance = 55.0;
    camera.target_distance = 55.0;
    camera.pitch = -0.45;
    camera.target_pitch = -0.45;
    camera.input_enabled = true;
}

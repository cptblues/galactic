use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PresentMode;
use galactic_domain::{PlanetKind, StarClass, SystemId, UniverseConfig, WorldPosition};
use galactic_sim::{
    GameCommand, GameEvent, KnowledgeLevel, KnowledgeTarget, MVP_HOME_SYSTEM_ID, SelectionTarget,
    Simulation, SystemVisibility, TimeSpeed,
};

pub fn run() {
    App::new().add_plugins(ClientPlugin).run();
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Galactic MVP".to_string(),
                resolution: (1280, 720).into(),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.006, 0.008, 0.014)))
        .insert_resource(SimulationResource {
            simulation: Simulation::new(UniverseConfig::default()),
            pending_events: Vec::new(),
        })
        .init_resource::<PresentationLog>()
        .init_resource::<VisualAssets>()
        .init_resource::<StrategicNavigation>()
        .init_resource::<ViewRebuildRequest>()
        .add_plugins(SimulationBridgePlugin)
        .add_plugins(PresentationPlugin)
        .add_systems(Startup, log_startup);
    }
}

pub struct SimulationBridgePlugin;

impl Plugin for SimulationBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (handle_simulation_input, handle_view_input, tick_simulation).chain(),
        );
    }
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (spawn_scene, spawn_strategic_view, spawn_ui).chain(),
        )
        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_strategic_camera,
                collect_presentation_events,
                update_system_visuals,
                update_system_labels,
                draw_strategic_overlays,
                update_ui,
                update_home_summary,
            ),
        );
    }
}

#[derive(Resource)]
pub struct SimulationResource {
    simulation: Simulation,
    pending_events: Vec<GameEvent>,
}

impl SimulationResource {
    pub fn simulation(&self) -> &Simulation {
        &self.simulation
    }
}

#[derive(Resource, Default)]
struct PresentationLog {
    last_event: Option<GameEvent>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum GraphicsPreset {
    #[default]
    Low,
}

#[derive(Resource)]
struct VisualAssets {
    system_mesh: Handle<Mesh>,
    known_star_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    detected_material: Handle<StandardMaterial>,
    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
}

impl FromWorld for VisualAssets {
    fn from_world(world: &mut World) -> Self {
        // Low preset: a very small shared mesh is sufficient at universe scale.
        let system_mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Sphere::default().mesh().ico(1).unwrap())
        };

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let known_star_materials = StarClass::ALL
            .into_iter()
            .map(|class| (class, materials.add(star_material(class))))
            .collect();
        let detected_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.34, 0.48, 0.62, 0.75),
            emissive: LinearRgba::rgb(0.28, 0.42, 0.62),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let planet_materials = PlanetKind::ALL
            .into_iter()
            .map(|kind| (kind, materials.add(planet_material(kind))))
            .collect();

        Self {
            system_mesh,
            known_star_materials,
            detected_material,
            planet_materials,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrategicViewMode {
    Universe,
    System(SystemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniverseLod {
    Overview,
    Regional,
    Local,
}

impl UniverseLod {
    fn from_distance(distance: f32) -> Self {
        if distance >= 88.0 {
            Self::Overview
        } else if distance >= 48.0 {
            Self::Regional
        } else {
            Self::Local
        }
    }
}

#[derive(Resource)]
struct StrategicNavigation {
    mode: StrategicViewMode,
    universe_focus: Vec3,
    universe_distance: f32,
    universe_yaw: f32,
    universe_pitch: f32,
    system_focus: Vec3,
    system_distance: f32,
    system_yaw: f32,
    system_pitch: f32,
    lod: UniverseLod,
    debug_full_graph: bool,
    preset: GraphicsPreset,
}

impl Default for StrategicNavigation {
    fn default() -> Self {
        let universe_distance = 108.0;
        Self {
            mode: StrategicViewMode::System(MVP_HOME_SYSTEM_ID),
            universe_focus: Vec3::ZERO,
            universe_distance,
            universe_yaw: 0.0,
            universe_pitch: -0.62,
            system_focus: Vec3::ZERO,
            system_distance: 34.0,
            system_yaw: 0.0,
            system_pitch: -0.62,
            lod: UniverseLod::from_distance(universe_distance),
            debug_full_graph: false,
            preset: GraphicsPreset::Low,
        }
    }
}

impl StrategicNavigation {
    fn enter_system(&mut self, system_id: SystemId) {
        self.mode = StrategicViewMode::System(system_id);
    }

    fn exit_system(&mut self) {
        self.mode = StrategicViewMode::Universe;
    }
}

#[derive(Resource, Default)]
struct ViewRebuildRequest(bool);

#[derive(Component)]
struct StrategicViewEntity;

#[derive(Component)]
struct StrategicCamera;

#[derive(Component)]
struct SystemVisual {
    id: SystemId,
    visibility: SystemVisibility,
    base_scale: Vec3,
}

#[derive(Component)]
struct SystemLabel {
    id: SystemId,
    visibility: SystemVisibility,
}

#[derive(Component)]
struct TopBarText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct HomeSummaryText;

fn log_startup() {
    info!("Galactic MVP client starting on Bevy 0.19");
}

fn spawn_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.006, 0.008, 0.014)),
            ..default()
        },
        Transform::from_xyz(0.0, 62.0, 88.0).looking_at(Vec3::ZERO, Vec3::Y),
        StrategicCamera,
    ));

    commands.spawn((
        PointLight {
            intensity: 9000.0,
            range: 240.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 40.0, 0.0),
    ));
}

fn spawn_strategic_view(
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    navigation: Res<StrategicNavigation>,
    existing: Query<Entity, With<StrategicViewEntity>>,
) {
    rebuild_strategic_view(&mut commands, &simulation, &assets, &navigation, &existing);
}

fn rebuild_strategic_view_if_requested(
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    navigation: Res<StrategicNavigation>,
    mut request: ResMut<ViewRebuildRequest>,
    existing: Query<Entity, With<StrategicViewEntity>>,
) {
    if !request.0 {
        return;
    }

    rebuild_strategic_view(&mut commands, &simulation, &assets, &navigation, &existing);
    request.0 = false;
}

fn rebuild_strategic_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    navigation: &StrategicNavigation,
    existing: &Query<Entity, With<StrategicViewEntity>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    match navigation.mode {
        StrategicViewMode::Universe => {
            spawn_universe_view(commands, simulation, assets, navigation);
        }
        StrategicViewMode::System(system_id) => {
            spawn_system_view(commands, simulation, assets, system_id);
        }
    }
}

fn spawn_universe_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    navigation: &StrategicNavigation,
) {
    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let state = simulation.state();

    let visible_systems = systems_for_universe_view(simulation, navigation.debug_full_graph);

    for (system_id, visibility) in visible_systems {
        let Some(system) = universe.system(system_id) else {
            continue;
        };

        let material = match visibility {
            SystemVisibility::Known => assets
                .known_star_materials
                .get(&system.star.class)
                .expect("star material exists")
                .clone(),
            SystemVisibility::Detected => assets.detected_material.clone(),
        };
        let visibility_scale = match visibility {
            SystemVisibility::Known => 1.0,
            SystemVisibility::Detected => 0.72,
        };
        let scale = Vec3::splat((0.72 + system.star.luminosity.min(2.4) * 0.16) * visibility_scale);
        let position = to_vec3(system.position);

        commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(scale),
            SystemVisual {
                id: system.id,
                visibility,
                base_scale: scale,
            },
            StrategicViewEntity,
        ));

        let label = match visibility {
            SystemVisibility::Known => system.name.clone(),
            SystemVisibility::Detected => format!("Signal {}", system.id.index()),
        };

        commands.spawn((
            Text2d::new(label),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(match visibility {
                SystemVisibility::Known => Color::srgba(0.76, 0.88, 1.0, 0.90),
                SystemVisibility::Detected => Color::srgba(0.48, 0.66, 0.82, 0.72),
            }),
            Transform::from_translation(position + Vec3::new(0.0, 1.8, 0.0))
                .with_scale(Vec3::splat(0.28)),
            SystemLabel {
                id: system.id,
                visibility,
            },
            StrategicViewEntity,
        ));
    }

    debug_assert!(
        navigation.debug_full_graph
            || state
                .visible_systems()
                .iter()
                .all(|(system_id, _)| { state.is_system_visible(*system_id) })
    );
}

fn systems_for_universe_view(
    simulation: &Simulation,
    debug_full_graph: bool,
) -> Vec<(SystemId, SystemVisibility)> {
    if debug_full_graph {
        return simulation
            .universe()
            .systems
            .iter()
            .map(|system| {
                (
                    system.id,
                    simulation
                        .state()
                        .system_visibility(system.id)
                        .unwrap_or(SystemVisibility::Detected),
                )
            })
            .collect();
    }

    simulation.state().visible_systems()
}

fn spawn_system_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    system_id: SystemId,
) {
    let simulation = simulation.simulation();
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };

    let star_material = assets
        .known_star_materials
        .get(&system.star.class)
        .expect("star material exists")
        .clone();

    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(star_material),
        Transform::from_scale(Vec3::splat(2.8)),
        StrategicViewEntity,
    ));

    commands.spawn((
        Text2d::new(system.name.clone()),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.94, 0.97, 1.0)),
        Transform::from_xyz(0.0, 3.6, 0.0).with_scale(Vec3::splat(0.34)),
        StrategicViewEntity,
    ));

    let state = simulation.state();
    for (index, planet) in system.planets.iter().enumerate() {
        let level = state.planet_knowledge_level(planet.id);
        if level == KnowledgeLevel::Unknown {
            continue;
        }

        let radius = 6.0 + index as f32 * 4.8;
        let angle = index as f32 * 1.37;
        let position = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        let colony = state.colony_on_planet(planet.id);
        let material = if level.reveals_identity() {
            assets
                .planet_materials
                .get(&planet.kind)
                .expect("planet material exists")
                .clone()
        } else {
            assets.detected_material.clone()
        };
        let scale = if level.reveals_identity() && planet.kind == PlanetKind::GasGiant {
            1.25
        } else {
            0.72
        };
        let label = match level {
            KnowledgeLevel::Unknown => {
                continue;
            }
            KnowledgeLevel::Detected => {
                format!("Corps détecté {}", index + 1)
            }
            KnowledgeLevel::Probed => {
                format!("{} — {:?}", planet.name, planet.kind)
            }
            KnowledgeLevel::Analyzed => format!(
                "{} — {:?} — hab {}%",
                planet.name, planet.kind, planet.habitability,
            ),
            KnowledgeLevel::Colonized => {
                let colony_name = colony.map(|value| value.name.as_str()).unwrap_or("Colonie");
                format!(
                    "{} — {} — hab {}%",
                    planet.name, colony_name, planet.habitability,
                )
            }
        };

        commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            StrategicViewEntity,
        ));

        commands.spawn((
            Text2d::new(label),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),
            Transform::from_translation(position + Vec3::new(0.0, 1.35, 0.0))
                .with_scale(Vec3::splat(0.25)),
            StrategicViewEntity,
        ));
    }
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.96, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            right: Val::Px(12.0),
            top: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.014, 0.022, 0.034, 0.78)),
        TopBarText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.82, 0.90, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(78.0),
            width: Val::Px(330.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.014, 0.022, 0.034, 0.78)),
        HomeSummaryText,
    ));

    commands.spawn((
        Text::new(
            "Space pause | 1/2/3 speed | RMB orbit | MMB pan | wheel zoom | WASD/QE fallback | Tab select | K advance knowledge | Enter/Esc views | F3 debug",
        ),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.72, 0.82, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(14.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.014, 0.022, 0.034, 0.66)),
        HelpText,
    ));
}

fn handle_simulation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
) {
    let command = if keyboard.just_pressed(KeyCode::Space) {
        Some(GameCommand::TogglePause)
    } else if keyboard.just_pressed(KeyCode::Digit1) {
        Some(GameCommand::SetSpeed(TimeSpeed::X1))
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(GameCommand::SetSpeed(TimeSpeed::X2))
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(GameCommand::SetSpeed(TimeSpeed::X4))
    } else if keyboard.just_pressed(KeyCode::KeyK) {
        Some(GameCommand::DebugAdvanceSelectedKnowledge)
    } else {
        None
    };

    let Some(command) = command else {
        return;
    };
    let events = simulation.simulation.apply_command(command);
    simulation.pending_events.extend(events);
}

fn handle_view_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        rebuild.0 = true;
    }

    if keyboard.just_pressed(KeyCode::F3) {
        navigation.debug_full_graph = !navigation.debug_full_graph;
        rebuild.0 = true;
    }

    if keyboard.just_pressed(KeyCode::Tab) {
        match navigation.mode {
            StrategicViewMode::Universe => {
                cycle_visible_selection(&mut simulation, navigation.debug_full_graph);
            }
            StrategicViewMode::System(system_id) => {
                cycle_planet_selection(&mut simulation, system_id);
            }
        }
    }

    if keyboard.just_pressed(KeyCode::KeyF)
        && matches!(navigation.mode, StrategicViewMode::Universe)
    {
        focus_selected_system(&simulation, &mut navigation);
    }

    if keyboard.just_pressed(KeyCode::Enter)
        && matches!(navigation.mode, StrategicViewMode::Universe)
        && let Some(system_id) = enterable_selected_system(&simulation, navigation.debug_full_graph)
    {
        navigation.enter_system(system_id);
        rebuild.0 = true;
    }

    if keyboard.just_pressed(KeyCode::Escape)
        && matches!(navigation.mode, StrategicViewMode::System(_))
    {
        navigation.exit_system();
        rebuild.0 = true;
    }
}

fn focus_selected_system(simulation: &SimulationResource, navigation: &mut StrategicNavigation) {
    let Some(system_id) = selected_system(simulation.simulation.state().selected) else {
        return;
    };
    let Some(system) = simulation.simulation.universe().system(system_id) else {
        return;
    };

    navigation.universe_focus = to_vec3(system.position);
}

fn enterable_selected_system(
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

fn cycle_visible_selection(simulation: &mut SimulationResource, debug_full_graph: bool) {
    let systems = systems_for_universe_view(simulation.simulation(), debug_full_graph);
    if systems.is_empty() {
        return;
    }

    let current = selected_system(simulation.simulation.state().selected);
    let current_index = current.and_then(|current_id| {
        systems
            .iter()
            .position(|(system_id, _)| *system_id == current_id)
    });
    let next_index = current_index
        .map(|index| (index + 1) % systems.len())
        .unwrap_or(0);
    let next_system = systems[next_index].0;

    let events = simulation
        .simulation
        .apply_command(GameCommand::SelectSystem(next_system));
    simulation.pending_events.extend(events);
}

fn cycle_planet_selection(simulation: &mut SimulationResource, system_id: SystemId) {
    let Some(system) = simulation.simulation.universe().system(system_id) else {
        return;
    };
    let visible_planets = system
        .planets
        .iter()
        .filter(|planet| {
            simulation
                .simulation
                .state()
                .planet_knowledge_level(planet.id)
                .is_visible()
        })
        .map(|planet| planet.id)
        .collect::<Vec<_>>();
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

    let events = simulation
        .simulation
        .apply_command(GameCommand::SelectPlanet {
            system_id,
            planet_id,
        });
    simulation.pending_events.extend(events);
}

fn selected_system(selection: SelectionTarget) -> Option<SystemId> {
    match selection {
        SelectionTarget::None => None,
        SelectionTarget::System(system_id) => Some(system_id),
        SelectionTarget::Planet { system_id, .. } => Some(system_id),
    }
}

fn tick_simulation(time: Res<Time>, mut simulation: ResMut<SimulationResource>) {
    let events = simulation.simulation.advance(time.delta());
    simulation.pending_events.extend(events);
}

fn update_strategic_camera(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut navigation: ResMut<StrategicNavigation>,
    mut query: Query<&mut Transform, With<StrategicCamera>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let delta_seconds = time.delta_secs();
    let motion = mouse_motion.delta;
    let scroll_lines = match mouse_scroll.unit {
        MouseScrollUnit::Line => mouse_scroll.delta.y,
        MouseScrollUnit::Pixel => mouse_scroll.delta.y / 40.0,
    };

    match navigation.mode {
        StrategicViewMode::Universe => {
            if mouse_buttons.pressed(MouseButton::Right) {
                let mut yaw = navigation.universe_yaw;
                let mut pitch = navigation.universe_pitch;
                apply_orbit_drag(&mut yaw, &mut pitch, motion);
                navigation.universe_yaw = yaw;
                navigation.universe_pitch = pitch;
            }
            if mouse_buttons.pressed(MouseButton::Middle) {
                let pan = mouse_pan_delta(
                    navigation.universe_yaw,
                    motion,
                    navigation.universe_distance,
                );
                navigation.universe_focus += pan;
            }

            let keyboard_pan = keyboard_pan_direction(&keyboard, navigation.universe_yaw);
            if keyboard_pan.length_squared() > 0.0 {
                let pan_speed = (navigation.universe_distance * 0.55).max(18.0);
                navigation.universe_focus += keyboard_pan.normalize() * pan_speed * delta_seconds;
            }

            apply_keyboard_zoom(
                &keyboard,
                delta_seconds,
                &mut navigation.universe_distance,
                20.0,
                150.0,
            );
            apply_scroll_zoom(&mut navigation.universe_distance, scroll_lines, 20.0, 150.0);
            navigation.lod = UniverseLod::from_distance(navigation.universe_distance);

            *transform = orbit_transform(
                navigation.universe_focus,
                navigation.universe_distance,
                navigation.universe_yaw,
                navigation.universe_pitch,
            );
        }
        StrategicViewMode::System(_) => {
            if mouse_buttons.pressed(MouseButton::Right) {
                let mut yaw = navigation.system_yaw;
                let mut pitch = navigation.system_pitch;
                apply_orbit_drag(&mut yaw, &mut pitch, motion);
                navigation.system_yaw = yaw;
                navigation.system_pitch = pitch;
            }
            if mouse_buttons.pressed(MouseButton::Middle) {
                let pan =
                    mouse_pan_delta(navigation.system_yaw, motion, navigation.system_distance);
                navigation.system_focus += pan;
            }

            let keyboard_pan = keyboard_pan_direction(&keyboard, navigation.system_yaw);
            if keyboard_pan.length_squared() > 0.0 {
                let pan_speed = (navigation.system_distance * 0.42).max(8.0);
                navigation.system_focus += keyboard_pan.normalize() * pan_speed * delta_seconds;
            }

            apply_keyboard_zoom(
                &keyboard,
                delta_seconds,
                &mut navigation.system_distance,
                10.0,
                80.0,
            );
            apply_scroll_zoom(&mut navigation.system_distance, scroll_lines, 10.0, 80.0);

            *transform = orbit_transform(
                navigation.system_focus,
                navigation.system_distance,
                navigation.system_yaw,
                navigation.system_pitch,
            );
        }
    }
}

fn apply_orbit_drag(yaw: &mut f32, pitch: &mut f32, motion: Vec2) {
    const SENSITIVITY: f32 = 0.006;
    *yaw -= motion.x * SENSITIVITY;
    *pitch = (*pitch - motion.y * SENSITIVITY).clamp(-1.35, 1.35);
}

fn mouse_pan_delta(yaw: f32, motion: Vec2, distance: f32) -> Vec3 {
    if motion == Vec2::ZERO {
        return Vec3::ZERO;
    }

    let yaw_rotation = Quat::from_rotation_y(yaw);
    let right = yaw_rotation * Vec3::X;
    let forward = yaw_rotation * -Vec3::Z;
    let scale = (distance * 0.0028).max(0.025);

    (-motion.x * right + motion.y * forward) * scale
}

fn keyboard_pan_direction(keyboard: &ButtonInput<KeyCode>, yaw: f32) -> Vec3 {
    let mut input = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyA) {
        input.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        input.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyW) {
        input.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        input.y -= 1.0;
    }

    let rotation = Quat::from_rotation_y(yaw);
    rotation * Vec3::new(input.x, 0.0, -input.y)
}

fn apply_keyboard_zoom(
    keyboard: &ButtonInput<KeyCode>,
    delta_seconds: f32,
    distance: &mut f32,
    minimum: f32,
    maximum: f32,
) {
    let zoom_speed = (*distance * 0.85).max(12.0);
    if keyboard.pressed(KeyCode::KeyQ) {
        *distance -= zoom_speed * delta_seconds;
    }
    if keyboard.pressed(KeyCode::KeyE) {
        *distance += zoom_speed * delta_seconds;
    }
    *distance = (*distance).clamp(minimum, maximum);
}

fn apply_scroll_zoom(distance: &mut f32, scroll_lines: f32, minimum: f32, maximum: f32) {
    if scroll_lines == 0.0 {
        return;
    }

    *distance *= (-scroll_lines * 0.12).exp();
    *distance = (*distance).clamp(minimum, maximum);
}

fn orbit_transform(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Transform {
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    let eye = focus + rotation * Vec3::new(0.0, 0.0, distance);
    Transform::from_translation(eye).looking_at(focus, Vec3::Y)
}

fn collect_presentation_events(
    mut simulation: ResMut<SimulationResource>,
    mut log: ResMut<PresentationLog>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    for event in simulation.pending_events.drain(..) {
        if matches!(event, GameEvent::KnowledgeChanged(_)) {
            rebuild.0 = true;
        }
        log.last_event = Some(event);
    }
}

fn update_system_visuals(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SystemVisual, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let selected_system = selected_system(simulation.simulation().state().selected);

    for (visual, mut transform) in &mut query {
        let selected_multiplier = if Some(visual.id) == selected_system {
            1.55
        } else {
            1.0
        };
        let lod_multiplier = match navigation.lod {
            UniverseLod::Overview => 0.78,
            UniverseLod::Regional => 0.92,
            UniverseLod::Local => 1.08,
        };
        let visibility_multiplier = match visual.visibility {
            SystemVisibility::Known => 1.0,
            SystemVisibility::Detected => 0.84,
        };

        transform.scale =
            visual.base_scale * selected_multiplier * lod_multiplier * visibility_multiplier;
    }
}

fn update_system_labels(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SystemLabel, &mut Visibility)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let state = simulation.simulation().state();
    let selected = selected_system(state.selected);

    for (label, mut visibility) in &mut query {
        let is_selected = Some(label.id) == selected;
        let is_colony = state
            .colonies
            .iter()
            .any(|colony| colony.system_id == label.id);

        let should_show = is_selected
            || is_colony
            || match navigation.lod {
                UniverseLod::Overview => false,
                UniverseLod::Regional => label.visibility == SystemVisibility::Known,
                UniverseLod::Local => true,
            };

        *visibility = if should_show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn draw_strategic_overlays(
    mut gizmos: Gizmos,
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
) {
    match navigation.mode {
        StrategicViewMode::Universe => {
            draw_universe_routes(&mut gizmos, simulation.simulation(), &navigation);
        }
        StrategicViewMode::System(system_id) => {
            draw_system_orbits(&mut gizmos, simulation.simulation(), system_id);
        }
    }
}

fn draw_universe_routes(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    navigation: &StrategicNavigation,
) {
    let universe = simulation.universe();
    let state = simulation.state();

    if navigation.debug_full_graph {
        for route in &universe.routes {
            draw_route(
                gizmos,
                universe,
                route.from,
                route.to,
                Color::srgba(0.42, 0.24, 0.62, 0.28),
            );
        }
        return;
    }

    for route in state.visible_routes(simulation.universe_repository()) {
        let both_known = state.is_system_known(route.from) && state.is_system_known(route.to);
        let color = if both_known {
            Color::srgba(0.28, 0.62, 0.94, 0.58)
        } else {
            Color::srgba(0.30, 0.48, 0.66, 0.38)
        };
        draw_route(gizmos, universe, route.from, route.to, color);
    }
}

fn draw_route(
    gizmos: &mut Gizmos,
    universe: &galactic_domain::UniverseDefinition,
    from_id: SystemId,
    to_id: SystemId,
    color: Color,
) {
    let Some(from) = universe.system(from_id) else {
        return;
    };
    let Some(to) = universe.system(to_id) else {
        return;
    };
    gizmos.line(to_vec3(from.position), to_vec3(to.position), color);
}

fn draw_system_orbits(gizmos: &mut Gizmos, simulation: &Simulation, system_id: SystemId) {
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };

    for index in 0..system.planets.len() {
        let radius = 6.0 + index as f32 * 4.8;
        draw_circle_xz(gizmos, radius, 48, Color::srgba(0.32, 0.46, 0.62, 0.26));
    }
}

fn draw_circle_xz(gizmos: &mut Gizmos, radius: f32, segments: usize, color: Color) {
    for segment in 0..segments {
        let start_angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
        let end_angle = (segment + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let start = Vec3::new(start_angle.cos() * radius, 0.0, start_angle.sin() * radius);
        let end = Vec3::new(end_angle.cos() * radius, 0.0, end_angle.sin() * radius);
        gizmos.line(start, end, color);
    }
}

fn update_ui(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    log: Res<PresentationLog>,
    mut query: Query<&mut Text, With<TopBarText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let repository = simulation.universe_repository();
    let state = simulation.state();
    let selected = selection_label(state.selected);
    let last_event = log
        .last_event
        .map(event_label)
        .unwrap_or_else(|| "ready".to_string());
    let visible_route_count = if navigation.debug_full_graph {
        universe.routes.len()
    } else {
        state.visible_routes(repository).len()
    };
    let visible_system_count = if navigation.debug_full_graph {
        universe.systems.len()
    } else {
        state.visible_systems().len()
    };
    let knowledge = state.system_knowledge_counts();
    let view_label = match navigation.mode {
        StrategicViewMode::Universe => format!("universe/{:?}", navigation.lod),
        StrategicViewMode::System(system_id) => {
            format!("system {}", system_id.index())
        }
    };

    text.0 = format!(
        "Galactic MVP | preset {:?} | view {} | seed {} | systems {}/{} | routes {}/{} | knowledge d{} p{} a{} c{} | tick {} | t {:.1}s | speed {} | selected {} | debug {} | event {}",
        navigation.preset,
        view_label,
        universe.seed,
        visible_system_count,
        universe.systems.len(),
        visible_route_count,
        universe.routes.len(),
        knowledge.detected,
        knowledge.probed,
        knowledge.analyzed,
        knowledge.colonized,
        state.clock.current_tick(),
        state.clock.elapsed_seconds(),
        state.clock.speed(),
        selected,
        navigation.debug_full_graph,
        last_event
    );
}

fn update_home_summary(
    simulation: Res<SimulationResource>,
    mut query: Query<&mut Text, With<HomeSummaryText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let simulation = simulation.simulation();
    let state = simulation.state();
    let Some(faction) = state.player_faction_state() else {
        text.0 = "Faction joueur invalide".to_string();
        return;
    };
    let Some(colony) = state.player_home_colony() else {
        text.0 = "Colonie mère introuvable".to_string();
        return;
    };
    let Some(system) = simulation.universe().system(colony.system_id) else {
        return;
    };
    let Some(planet) = simulation.universe_repository().planet(colony.planet_id) else {
        return;
    };

    text.0 = format!(
        "{}\n{} / {}\nHabitabilité : {}%\n\nStocks\nMétal {}  Cristal {}\nCarburant {}  Énergie {}\n\nPotentiel planète\nM {}  C {}  F {}  E {}\n\nBâtiments\nMines {}/{}/{}  Centrale {}\nEntrepôt {}  Construction {}\nLaboratoire {}  Chantier {}",
        faction.name,
        system.name,
        planet.name,
        planet.habitability,
        colony.stock.metal,
        colony.stock.crystal,
        colony.stock.fuel,
        colony.stock.energy,
        colony.resource_profile.metal,
        colony.resource_profile.crystal,
        colony.resource_profile.fuel,
        colony.resource_profile.energy,
        colony.buildings.metal_mine,
        colony.buildings.crystal_extractor,
        colony.buildings.fuel_refinery,
        colony.buildings.power_plant,
        colony.buildings.warehouse,
        colony.buildings.construction_center,
        colony.buildings.research_lab,
        colony.buildings.shipyard,
    );
}

fn to_vec3(position: WorldPosition) -> Vec3 {
    Vec3::new(position.x, position.y, position.z)
}

fn star_material(class: StarClass) -> StandardMaterial {
    StandardMaterial {
        base_color: star_color(class),
        emissive: star_emissive(class),
        unlit: true,
        ..default()
    }
}

fn planet_material(kind: PlanetKind) -> StandardMaterial {
    StandardMaterial {
        base_color: match kind {
            PlanetKind::Rocky => Color::srgb(0.48, 0.42, 0.36),
            PlanetKind::Ocean => Color::srgb(0.18, 0.46, 0.72),
            PlanetKind::Desert => Color::srgb(0.72, 0.52, 0.28),
            PlanetKind::Ice => Color::srgb(0.62, 0.78, 0.90),
            PlanetKind::GasGiant => Color::srgb(0.62, 0.50, 0.68),
            PlanetKind::Volcanic => Color::srgb(0.72, 0.24, 0.12),
        },
        unlit: true,
        ..default()
    }
}

fn star_color(class: StarClass) -> Color {
    match class {
        StarClass::Blue => Color::srgb(0.42, 0.66, 1.0),
        StarClass::White => Color::srgb(0.92, 0.96, 1.0),
        StarClass::Yellow => Color::srgb(1.0, 0.86, 0.44),
        StarClass::Orange => Color::srgb(1.0, 0.58, 0.28),
        StarClass::Red => Color::srgb(0.95, 0.28, 0.24),
    }
}

fn star_emissive(class: StarClass) -> LinearRgba {
    match class {
        StarClass::Blue => LinearRgba::rgb(1.2, 2.4, 5.0),
        StarClass::White => LinearRgba::rgb(2.6, 2.8, 3.0),
        StarClass::Yellow => LinearRgba::rgb(2.8, 2.1, 0.8),
        StarClass::Orange => LinearRgba::rgb(2.6, 1.2, 0.45),
        StarClass::Red => LinearRgba::rgb(2.2, 0.45, 0.35),
    }
}

fn selection_label(selection: SelectionTarget) -> String {
    match selection {
        SelectionTarget::None => "none".to_string(),
        SelectionTarget::System(system_id) => {
            format!("system {}", system_id.index())
        }
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => format!("planet {}:{}", system_id.index(), planet_id.index()),
    }
}

fn event_label(event: GameEvent) -> String {
    match event {
        GameEvent::SpeedChanged(speed) => format!("speed {}", speed),
        GameEvent::SelectionChanged(selection) => {
            format!("selection {}", selection_label(selection))
        }
        GameEvent::KnowledgeChanged(change) => {
            let target = match change.target {
                KnowledgeTarget::System(id) => {
                    format!("system {}", id.index())
                }
                KnowledgeTarget::Planet(id) => {
                    format!("planet {}", id.index())
                }
            };
            format!("{} {} -> {}", target, change.previous, change.current)
        }
        GameEvent::TicksAdvanced {
            ticks,
            current_tick,
        } => format!("+{} ticks -> {}", ticks.ticks(), current_tick),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_lod_uses_stable_distance_bands() {
        assert_eq!(UniverseLod::from_distance(120.0), UniverseLod::Overview);
        assert_eq!(UniverseLod::from_distance(64.0), UniverseLod::Regional);
        assert_eq!(UniverseLod::from_distance(32.0), UniverseLod::Local);
    }

    #[test]
    fn normal_view_instantiates_fewer_systems_than_debug_view() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        let normal = systems_for_universe_view(&simulation, false);
        let debug = systems_for_universe_view(&simulation, true);

        assert!(normal.len() <= debug.len());
        assert_eq!(debug.len(), simulation.universe().systems.len());
    }

    #[test]
    fn universe_camera_context_survives_system_transition() {
        let mut navigation = StrategicNavigation {
            universe_focus: Vec3::new(12.0, 0.0, -7.0),
            universe_distance: 73.0,
            ..default()
        };
        let focus = navigation.universe_focus;
        let distance = navigation.universe_distance;

        navigation.enter_system(SystemId::from_index(3));
        navigation.exit_system();

        assert_eq!(navigation.mode, StrategicViewMode::Universe);
        assert_eq!(navigation.universe_focus, focus);
        assert_eq!(navigation.universe_distance, distance);
    }

    #[test]
    fn mouse_orbit_clamps_pitch() {
        let mut yaw = 0.0;
        let mut pitch = 0.0;

        apply_orbit_drag(&mut yaw, &mut pitch, Vec2::new(100.0, -10_000.0));

        assert!(yaw < 0.0);
        assert_eq!(pitch, 1.35);
    }

    #[test]
    fn mouse_scroll_zoom_is_bounded() {
        let mut distance = 34.0;
        apply_scroll_zoom(&mut distance, 100.0, 10.0, 80.0);
        assert_eq!(distance, 10.0);

        apply_scroll_zoom(&mut distance, -100.0, 10.0, 80.0);
        assert_eq!(distance, 80.0);
    }

    #[test]
    fn presentation_labels_use_domain_selection_ids() {
        let label = selection_label(SelectionTarget::Planet {
            system_id: SystemId::new(2),
            planet_id: galactic_domain::PlanetId::new(1),
        });

        assert_eq!(label, "planet 2:1");
    }
}

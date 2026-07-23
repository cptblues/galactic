use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PresentMode;
use galactic_domain::{StarClass, SystemId, UniverseConfig, WorldPosition};
use galactic_sim::{GameCommand, GameEvent, SelectionTarget, Simulation, TimeSpeed};

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
        .add_plugins(SimulationBridgePlugin)
        .add_plugins(PresentationPlugin)
        .add_systems(Startup, log_startup);
    }
}

pub struct SimulationBridgePlugin;

impl Plugin for SimulationBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_simulation_input, tick_simulation).chain());
    }
}

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (spawn_scene, spawn_universe_view, spawn_ui).chain(),
        )
        .add_systems(
            Update,
            (
                rebuild_universe_view_on_input,
                collect_presentation_events,
                update_system_visuals,
                draw_routes,
                update_ui,
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

#[derive(Resource)]
struct VisualAssets {
    system_mesh: Handle<Mesh>,
    star_materials: HashMap<StarClass, Handle<StandardMaterial>>,
}

impl FromWorld for VisualAssets {
    fn from_world(world: &mut World) -> Self {
        let system_mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Sphere::default().mesh().ico(3).unwrap())
        };
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let star_materials = StarClass::ALL
            .into_iter()
            .map(|class| (class, materials.add(star_material(class))))
            .collect();

        Self {
            system_mesh,
            star_materials,
        }
    }
}

#[derive(Component)]
struct UniverseViewEntity;

#[derive(Component)]
struct SystemVisual {
    id: SystemId,
    base_scale: Vec3,
}

#[derive(Component)]
struct TopBarText;

#[derive(Component)]
struct HelpText;

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

fn spawn_universe_view(
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    existing: Query<Entity, With<UniverseViewEntity>>,
) {
    rebuild_universe_view(&mut commands, &simulation, &assets, &existing);
}

fn rebuild_universe_view_on_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    existing: Query<Entity, With<UniverseViewEntity>>,
) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        rebuild_universe_view(&mut commands, &simulation, &assets, &existing);
    }
}

fn rebuild_universe_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    existing: &Query<Entity, With<UniverseViewEntity>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    for system in &simulation.simulation().state().universe.systems {
        let material = assets
            .star_materials
            .get(&system.star.class)
            .expect("star material exists")
            .clone();
        let scale = Vec3::splat(0.8 + system.star.luminosity.min(2.4) * 0.18);
        let position = to_vec3(system.position);

        commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(scale),
            SystemVisual {
                id: system.id,
                base_scale: scale,
            },
            UniverseViewEntity,
        ));

        commands.spawn((
            Text2d::new(system.name.clone()),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgba(0.76, 0.88, 1.0, 0.84)),
            Transform::from_translation(position + Vec3::new(0.0, 1.8, 0.0))
                .with_scale(Vec3::splat(0.28)),
            UniverseViewEntity,
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
        Text::new(
            "Space pause | 1 x1 | 2 x2 | 3 x4 | R rebuild views | business state lives outside Bevy views",
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
    } else {
        None
    };

    let Some(command) = command else {
        return;
    };
    let events = simulation.simulation.apply_command(command);
    simulation.pending_events.extend(events);
}

fn tick_simulation(time: Res<Time>, mut simulation: ResMut<SimulationResource>) {
    let events = simulation.simulation.tick(time.delta_secs());
    simulation.pending_events.extend(events);
}

fn collect_presentation_events(
    mut simulation: ResMut<SimulationResource>,
    mut log: ResMut<PresentationLog>,
) {
    for event in simulation.pending_events.drain(..) {
        log.last_event = Some(event);
    }
}

fn update_ui(
    simulation: Res<SimulationResource>,
    log: Res<PresentationLog>,
    mut query: Query<&mut Text, With<TopBarText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let state = simulation.simulation().state();
    let selected = selection_label(state.selected);
    let last_event = log
        .last_event
        .map(event_label)
        .unwrap_or_else(|| "ready".to_string());

    text.0 = format!(
        "Galactic MVP | Bevy 0.19 | seed {} | gen v{} | fp {:016x} | systems {} | routes {} | colonies {} | known {} | t {:.1}s | speed {} | selected {} | event {}",
        state.universe.seed,
        state.universe.generation_version,
        state.universe.generation_fingerprint,
        state.universe.systems.len(),
        state.universe.routes.len(),
        state.colonies.len(),
        state.known_systems.len(),
        state.elapsed_seconds,
        state.speed,
        selected,
        last_event
    );
}

fn update_system_visuals(
    simulation: Res<SimulationResource>,
    mut query: Query<(&SystemVisual, &mut Transform)>,
) {
    let selected_system = match simulation.simulation().state().selected {
        SelectionTarget::None => None,
        SelectionTarget::System(system_id) => Some(system_id),
        SelectionTarget::Planet { system_id, .. } => Some(system_id),
    };

    for (visual, mut transform) in &mut query {
        transform.scale = if Some(visual.id) == selected_system {
            visual.base_scale * 1.45
        } else {
            visual.base_scale
        };
    }
}

fn draw_routes(mut gizmos: Gizmos, simulation: Res<SimulationResource>) {
    let universe = &simulation.simulation().state().universe;

    for route in &universe.routes {
        let Some(from) = universe.system(route.from) else {
            continue;
        };
        let Some(to) = universe.system(route.to) else {
            continue;
        };

        gizmos.line(
            to_vec3(from.position),
            to_vec3(to.position),
            Color::srgba(0.28, 0.62, 0.94, 0.35),
        );
    }
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
        SelectionTarget::System(system_id) => format!("system {}", system_id.index()),
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
        GameEvent::TickAdvanced {
            elapsed_seconds, ..
        } => format!("tick {:.1}s", elapsed_seconds),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_labels_use_domain_selection_ids() {
        let label = selection_label(SelectionTarget::Planet {
            system_id: SystemId::new(2),
            planet_id: galactic_domain::PlanetId::new(1),
        });

        assert_eq!(label, "planet 2:1");
    }
}

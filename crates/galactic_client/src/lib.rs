use std::{collections::HashMap, time::Duration};

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::window::{PresentMode, PrimaryWindow};
use galactic_domain::{PlanetId, PlanetKind, StarClass, SystemId, UniverseConfig, WorldPosition};
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
        .init_resource::<PointerSelectionState>()
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
                update_pointer_candidates,
                handle_pointer_selection,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_ui,
                update_info_panel,
            )
                .chain(),
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
    hover_material: Handle<StandardMaterial>,
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
        let hover_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.28, 0.92, 0.82, 0.18),
            emissive: LinearRgba::rgb(0.18, 1.2, 0.92),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        Self {
            system_mesh,
            known_star_materials,
            detected_material,
            planet_materials,
            hover_material,
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
struct InfoPanelText;

#[derive(Component)]
struct SelectableVisual {
    target: PickTarget,
    pick_radius_px: f32,
    priority: u8,
}

#[derive(Component)]
struct PointerHalo {
    target: PickTarget,
}

#[derive(Component)]
struct UiPointerBlocker;

#[derive(Component)]
struct PointerTooltipText;

#[derive(Component)]
struct AmbiguityPanelText;

// MVP-010: partial-information inspectors must never reveal hidden data.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorContent {
    level: Option<KnowledgeLevel>,
    badge: String,
    title: String,
    body: String,
    hint: String,
}

impl InspectorContent {
    fn render(&self) -> String {
        format!(
            "{}\n{}\n\n{}\n\n{}",
            self.badge, self.title, self.body, self.hint,
        )
    }
}

// MVP-010-B: screen-space picking uses displayed transforms, not domain positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PickTarget {
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

impl PickTarget {
    const fn sort_key(self) -> (u8, u64, u64) {
        match self {
            Self::System(system_id) => (0, system_id.raw(), 0),
            Self::Planet {
                system_id,
                planet_id,
            } => (1, system_id.raw(), planet_id.raw()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PointerCandidate {
    target: PickTarget,
    screen_position: Vec2,
    screen_distance: f32,
    depth: f32,
    priority: u8,
}

#[derive(Debug, Clone)]
struct AmbiguitySelection {
    targets: Vec<PickTarget>,
    active_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct PointerClickRecord {
    target: PickTarget,
    at: Duration,
    cursor_position: Vec2,
}

#[derive(Resource, Default)]
struct PointerSelectionState {
    hovered: Option<PickTarget>,
    hovered_screen_position: Option<Vec2>,
    candidates: Vec<PointerCandidate>,
    ambiguity: Option<AmbiguitySelection>,
    last_click: Option<PointerClickRecord>,
}

impl PointerSelectionState {
    fn clear_hover(&mut self) {
        self.hovered = None;
        self.hovered_screen_position = None;
        self.candidates.clear();
    }

    fn cycle_ambiguity(&mut self, reverse: bool) -> Option<PickTarget> {
        let ambiguity = self.ambiguity.as_mut()?;
        if ambiguity.targets.is_empty() {
            return None;
        }

        ambiguity.active_index = if reverse {
            ambiguity
                .active_index
                .checked_sub(1)
                .unwrap_or(ambiguity.targets.len() - 1)
        } else {
            (ambiguity.active_index + 1) % ambiguity.targets.len()
        };
        ambiguity.targets.get(ambiguity.active_index).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiAction {
    TogglePause,
    SetSpeed(TimeSpeed),
    CycleTarget,
    FocusSelection,
    EnterSystem,
    ExitSystem,
    AdvanceKnowledge,
    ToggleDebugGraph,
    RebuildView,
}

#[derive(Component)]
struct ActionButton {
    action: UiAction,
}

type ActionButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ActionButton),
    (Changed<Interaction>, With<Button>),
>;
type ActionButtonStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ActionButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
>;

// Bevy `KeyCode` values are physical key positions. These constants name the
// labels printed on an AZERTY keyboard for the movement cluster.
const AZERTY_FORWARD_KEY: KeyCode = KeyCode::KeyW;
const AZERTY_LEFT_KEY: KeyCode = KeyCode::KeyA;
const AZERTY_BACKWARD_KEY: KeyCode = KeyCode::KeyS;
const AZERTY_RIGHT_KEY: KeyCode = KeyCode::KeyD;
const AZERTY_ZOOM_IN_KEY: KeyCode = KeyCode::KeyQ;
const AZERTY_ZOOM_OUT_KEY: KeyCode = KeyCode::KeyE;

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
            SelectableVisual {
                target: PickTarget::System(system.id),
                pick_radius_px: 18.0,
                priority: system_pick_priority(simulation, system.id, visibility),
            },
            StrategicViewEntity,
        ));
        spawn_pointer_halo(
            commands,
            assets,
            PickTarget::System(system.id),
            position,
            scale.x * 1.65,
        );

        let label = match visibility {
            SystemVisibility::Known => system.name.clone(),
            SystemVisibility::Detected => format!("Signal {}", system.id.index()),
        };

        commands.spawn((
            Text2d::new(label),
            ui_text_font(12.0),
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
    let state = simulation.state();

    let star_material = assets
        .known_star_materials
        .get(&system.star.class)
        .expect("star material exists")
        .clone();

    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(star_material),
        Transform::from_scale(Vec3::splat(2.8)),
        SelectableVisual {
            target: PickTarget::System(system_id),
            pick_radius_px: 20.0,
            priority: system_pick_priority(simulation, system_id, SystemVisibility::Known),
        },
        StrategicViewEntity,
    ));
    spawn_pointer_halo(
        commands,
        assets,
        PickTarget::System(system_id),
        Vec3::ZERO,
        3.5,
    );

    commands.spawn((
        Text2d::new(system.name.clone()),
        ui_text_font(18.0),
        TextColor(Color::srgb(0.94, 0.97, 1.0)),
        Transform::from_xyz(0.0, 3.6, 0.0).with_scale(Vec3::splat(0.34)),
        StrategicViewEntity,
    ));

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
            SelectableVisual {
                target: PickTarget::Planet {
                    system_id,
                    planet_id: planet.id,
                },
                pick_radius_px: if level.reveals_identity() && planet.kind == PlanetKind::GasGiant {
                    18.0
                } else {
                    16.0
                },
                priority: planet_pick_priority(simulation, planet.id, level),
            },
            StrategicViewEntity,
        ));
        spawn_pointer_halo(
            commands,
            assets,
            PickTarget::Planet {
                system_id,
                planet_id: planet.id,
            },
            position,
            scale * 1.65,
        );

        commands.spawn((
            Text2d::new(label),
            ui_text_font(11.0),
            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),
            Transform::from_translation(position + Vec3::new(0.0, 1.35, 0.0))
                .with_scale(Vec3::splat(0.25)),
            StrategicViewEntity,
        ));
    }
}

fn spawn_pointer_halo(
    commands: &mut Commands,
    assets: &VisualAssets,
    target: PickTarget,
    position: Vec3,
    scale: f32,
) {
    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(assets.hover_material.clone()),
        Transform::from_translation(position).with_scale(Vec3::splat(scale)),
        Visibility::Hidden,
        PointerHalo { target },
        StrategicViewEntity,
    ));
}

fn system_pick_priority(
    simulation: &Simulation,
    system_id: SystemId,
    visibility: SystemVisibility,
) -> u8 {
    if simulation
        .state()
        .colonies
        .iter()
        .any(|colony| colony.system_id == system_id)
    {
        120
    } else if visibility == SystemVisibility::Known {
        90
    } else {
        70
    }
}

fn planet_pick_priority(simulation: &Simulation, planet_id: PlanetId, level: KnowledgeLevel) -> u8 {
    if simulation.state().colony_on_planet(planet_id).is_some() {
        120
    } else {
        match level {
            KnowledgeLevel::Unknown => 0,
            KnowledgeLevel::Detected => 70,
            KnowledgeLevel::Probed => 85,
            KnowledgeLevel::Analyzed => 95,
            KnowledgeLevel::Colonized => 120,
        }
    }
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        ui_text_font(14.0),
        TextColor(Color::srgb(0.9, 0.96, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            right: Val::Px(12.0),
            top: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        Interaction::None,
        UiPointerBlocker,
        TopBarText,
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(72.0),
                width: Val::Px(268.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|parent| {
            spawn_panel_heading(parent, "COMMANDES");
            spawn_action_button(parent, UiAction::TogglePause, "Pause", "Space");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X1), "Vitesse x1", "1");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X2), "Vitesse x2", "2");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X4), "Vitesse x4", "3");
            spawn_action_button(parent, UiAction::CycleTarget, "Cible suivante", "Tab");
            spawn_action_button(parent, UiAction::FocusSelection, "Recentrer", "F");
            spawn_action_button(parent, UiAction::EnterSystem, "Entrer système", "Enter");
            spawn_action_button(parent, UiAction::ExitSystem, "Retour univers", "Esc");
            spawn_action_button(parent, UiAction::AdvanceKnowledge, "Analyser cible", "K");
            spawn_action_button(parent, UiAction::ToggleDebugGraph, "Debug graphe", "G");
            spawn_action_button(parent, UiAction::RebuildView, "Reconstruire", "R");
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(14.0),
                top: Val::Px(72.0),
                width: Val::Px(348.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                ui_text_font(14.0),
                TextColor(Color::srgb(0.82, 0.90, 0.98)),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
                InfoPanelText,
            ));
        });

    commands.spawn((
        Text::new(
            "Clic sélectionner | Double-clic ouvrir/recentrer | Tab ambiguïtés | droit orbite | milieu déplacer | molette zoom",
        ),
        ui_text_font(12.0),
        TextColor(Color::srgb(0.76, 0.84, 0.90)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            right: Val::Px(14.0),
            bottom: Val::Px(14.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.022, 0.026, 0.030, 0.72)),
        Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.60, 0.50, 0.34, 0.35)),
        Interaction::None,
        UiPointerBlocker,
        HelpText,
    ));

    commands.spawn((
        Text::new(""),
        ui_text_font(12.0),
        TextColor(Color::srgb(0.88, 0.96, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(258.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.030, 0.94)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.28, 0.92, 0.82, 0.58),
        ),
        Visibility::Hidden,
        Interaction::None,
        UiPointerBlocker,
        PointerTooltipText,
    ));

    commands.spawn((
        Text::new(""),
        ui_text_font(13.0),
        TextColor(Color::srgb(0.88, 0.94, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            bottom: Val::Px(58.0),
            width: Val::Px(440.0),
            margin: UiRect::left(Val::Px(-220.0)),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.018, 0.025, 0.034, 0.96)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.74, 0.68, 0.34, 0.70),
        ),
        Visibility::Hidden,
        Interaction::None,
        UiPointerBlocker,
        AmbiguityPanelText,
    ));
}

fn spawn_panel_heading(parent: &mut ChildSpawnerCommands<'_>, label: &str) {
    parent.spawn((
        Text::new(label),
        ui_text_font(11.0),
        TextColor(Color::srgb(0.62, 0.86, 0.78)),
        Node {
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        },
    ));
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands<'_>,
    action: UiAction,
    label: &str,
    shortcut: &str,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.58, 0.72, 0.76, 0.30),
            ),
            ActionButton { action },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(13.0),
                TextColor(Color::srgb(0.90, 0.95, 0.96)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            button.spawn((
                Text::new(shortcut),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.70, 0.76, 0.72)),
            ));
        });
}

fn ui_text_font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::SansSerif,
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn panel_background() -> Color {
    Color::srgba(0.016, 0.020, 0.024, 0.84)
}

fn panel_outline() -> Color {
    Color::srgba(0.28, 0.56, 0.62, 0.42)
}

fn action_button_color(available: bool, active: bool, interaction: &Interaction) -> Color {
    if !available {
        return Color::srgba(0.050, 0.052, 0.052, 0.56);
    }
    if active {
        return match interaction {
            Interaction::Pressed => Color::srgba(0.22, 0.62, 0.52, 0.95),
            Interaction::Hovered => Color::srgba(0.18, 0.52, 0.46, 0.92),
            Interaction::None => Color::srgba(0.14, 0.42, 0.38, 0.88),
        };
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.26, 0.36, 0.42, 0.94),
        Interaction::Hovered => Color::srgba(0.18, 0.30, 0.35, 0.90),
        Interaction::None => Color::srgba(0.075, 0.095, 0.105, 0.86),
    }
}

fn action_button_outline(available: bool, active: bool, interaction: &Interaction) -> Color {
    if !available {
        return Color::srgba(0.30, 0.32, 0.32, 0.24);
    }
    if active {
        return Color::srgba(0.34, 0.92, 0.72, 0.70);
    }
    match interaction {
        Interaction::Pressed | Interaction::Hovered => Color::srgba(0.72, 0.74, 0.52, 0.64),
        Interaction::None => Color::srgba(0.58, 0.72, 0.76, 0.30),
    }
}

fn handle_simulation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    if let Some(action) = simulation_shortcut(&keyboard) {
        apply_ui_action(action, &mut simulation, &mut navigation, &mut rebuild);
    }
}

fn handle_view_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut pointer_state: ResMut<PointerSelectionState>,
) {
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

    if let Some(action) = view_shortcut(&keyboard) {
        apply_ui_action(action, &mut simulation, &mut navigation, &mut rebuild);
    }
}

fn handle_action_buttons(
    mut interactions: ActionButtonInteractionQuery,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    for (interaction, button) in &mut interactions {
        if matches!(interaction, Interaction::Pressed) {
            apply_ui_action(
                button.action,
                &mut simulation,
                &mut navigation,
                &mut rebuild,
            );
        }
    }
}

fn update_pointer_candidates(
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

fn handle_pointer_selection(
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

fn update_pointer_halos(
    pointer_state: Res<PointerSelectionState>,
    mut halos: Query<(&PointerHalo, &mut Visibility)>,
) {
    if !pointer_state.is_changed() {
        return;
    }

    for (halo, mut visibility) in &mut halos {
        *visibility = if Some(halo.target) == pointer_state.hovered {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_pointer_tooltip(
    windows: Query<&Window, With<PrimaryWindow>>,
    simulation: Res<SimulationResource>,
    pointer_state: Res<PointerSelectionState>,
    mut tooltips: Query<(&mut Text, &mut Node, &mut Visibility), With<PointerTooltipText>>,
) {
    let Ok((mut text, mut node, mut visibility)) = tooltips.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Some(target) = pointer_state.hovered else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Some(screen_position) = pointer_state.hovered_screen_position else {
        *visibility = Visibility::Hidden;
        return;
    };

    text.0 = pointer_tooltip_text(simulation.simulation(), target);
    node.left = Val::Px((screen_position.x + 18.0).clamp(8.0, (window.width() - 270.0).max(8.0)));
    node.top = Val::Px((screen_position.y + 18.0).clamp(8.0, (window.height() - 110.0).max(8.0)));
    *visibility = Visibility::Visible;
}

fn update_ambiguity_panel(
    simulation: Res<SimulationResource>,
    pointer_state: Res<PointerSelectionState>,
    mut panels: Query<(&mut Text, &mut Visibility), With<AmbiguityPanelText>>,
) {
    let Ok((mut text, mut visibility)) = panels.single_mut() else {
        return;
    };
    let Some(ambiguity) = pointer_state.ambiguity.as_ref() else {
        *visibility = Visibility::Hidden;
        return;
    };

    let mut lines = vec![
        "PLUSIEURS CIBLES SOUS LE CURSEUR".to_string(),
        "Tab / Maj+Tab : parcourir | Entrée : valider | Échap : fermer".to_string(),
        String::new(),
    ];
    for (index, target) in ambiguity.targets.iter().enumerate() {
        let marker = if index == ambiguity.active_index {
            "▶"
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

    text.0 = lines.join("\n");
    *visibility = Visibility::Visible;
}

fn select_pick_target(simulation: &mut SimulationResource, target: PickTarget) {
    let command = match target {
        PickTarget::System(system_id) => GameCommand::SelectSystem(system_id),
        PickTarget::Planet {
            system_id,
            planet_id,
        } => GameCommand::SelectPlanet {
            system_id,
            planet_id,
        },
    };
    apply_simulation_command(simulation, command);
}

fn activate_pick_target(
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

fn pointer_tooltip_text(simulation: &Simulation, target: PickTarget) -> String {
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
            let title = simulation
                .universe_repository()
                .planet(planet_id)
                .map(|planet| {
                    if level.reveals_identity() {
                        planet.name.clone()
                    } else {
                        format!("Corps détecté {}", planet_id.index())
                    }
                })
                .unwrap_or_else(|| "Planète invalide".to_string());
            format!(
                "{}\n{}\nClic : sélectionner | Double-clic : recentrer",
                title,
                knowledge_badge_fr(level),
            )
        }
    }
}

fn pick_target_label(simulation: &Simulation, target: PickTarget) -> String {
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
            .map(|planet| {
                if state.planet_knowledge_level(planet_id).reveals_identity() {
                    format!("Planète {}", planet.name)
                } else {
                    format!("Corps détecté {}", planet_id.index())
                }
            })
            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),
    }
}

fn rank_pointer_candidates(candidates: &mut [PointerCandidate]) {
    candidates.sort_by(|left, right| {
        left.screen_distance
            .total_cmp(&right.screen_distance)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left.depth.total_cmp(&right.depth))
            .then_with(|| left.target.sort_key().cmp(&right.target.sort_key()))
    });
}

fn screen_space_hit(cursor_position: Vec2, target_position: Vec2, radius_px: f32) -> bool {
    cursor_position.distance_squared(target_position) <= radius_px * radius_px
}

fn pointer_double_click(
    previous: PointerClickRecord,
    target: PickTarget,
    now: Duration,
    cursor_position: Vec2,
) -> bool {
    previous.target == target
        && now.saturating_sub(previous.at) <= Duration::from_millis(350)
        && previous.cursor_position.distance(cursor_position) <= 6.0
}

fn pick_target_is_visible(simulation: &Simulation, target: PickTarget) -> bool {
    match target {
        PickTarget::System(system_id) => simulation.state().is_system_visible(system_id),
        PickTarget::Planet { planet_id, .. } => simulation
            .state()
            .planet_knowledge_level(planet_id)
            .is_visible(),
    }
}

fn pick_target_matches_selection(target: PickTarget, selection: SelectionTarget) -> bool {
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

fn update_action_buttons(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut buttons: ActionButtonStyleQuery,
) {
    for (button, interaction, mut background, mut outline) in &mut buttons {
        let available = action_available(button.action, &simulation, &navigation);
        let active = action_active(button.action, &simulation, &navigation);
        background.0 = action_button_color(available, active, interaction);
        outline.color = action_button_outline(available, active, interaction);
    }
}

fn simulation_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<UiAction> {
    if keyboard.just_pressed(KeyCode::Space) {
        Some(UiAction::TogglePause)
    } else if keyboard.just_pressed(KeyCode::Digit1) {
        Some(UiAction::SetSpeed(TimeSpeed::X1))
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(UiAction::SetSpeed(TimeSpeed::X2))
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(UiAction::SetSpeed(TimeSpeed::X4))
    } else if keyboard.just_pressed(KeyCode::KeyK) {
        Some(UiAction::AdvanceKnowledge)
    } else {
        None
    }
}

fn view_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<UiAction> {
    if keyboard.just_pressed(KeyCode::KeyR) {
        Some(UiAction::RebuildView)
    } else if keyboard.just_pressed(KeyCode::KeyG) {
        Some(UiAction::ToggleDebugGraph)
    } else if keyboard.just_pressed(KeyCode::Tab) {
        Some(UiAction::CycleTarget)
    } else if keyboard.just_pressed(KeyCode::KeyF) {
        Some(UiAction::FocusSelection)
    } else if keyboard.just_pressed(KeyCode::Enter) {
        Some(UiAction::EnterSystem)
    } else if keyboard.just_pressed(KeyCode::Escape) {
        Some(UiAction::ExitSystem)
    } else {
        None
    }
}

fn apply_ui_action(
    action: UiAction,
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    rebuild: &mut ViewRebuildRequest,
) {
    if !action_available(action, simulation, navigation) {
        return;
    }

    match action {
        UiAction::TogglePause => apply_simulation_command(simulation, GameCommand::TogglePause),
        UiAction::SetSpeed(speed) => {
            apply_simulation_command(simulation, GameCommand::SetSpeed(speed));
        }
        UiAction::CycleTarget => match navigation.mode {
            StrategicViewMode::Universe => {
                cycle_visible_selection(simulation, navigation.debug_full_graph);
            }
            StrategicViewMode::System(system_id) => {
                cycle_planet_selection(simulation, system_id);
            }
        },
        UiAction::FocusSelection => {
            focus_selected_system(simulation, navigation);
        }
        UiAction::EnterSystem => {
            if let Some(system_id) =
                enterable_selected_system(simulation, navigation.debug_full_graph)
            {
                navigation.enter_system(system_id);
                rebuild.0 = true;
            }
        }
        UiAction::ExitSystem => {
            navigation.exit_system();
            rebuild.0 = true;
        }
        UiAction::AdvanceKnowledge => {
            apply_simulation_command(simulation, GameCommand::DebugAdvanceSelectedKnowledge);
        }
        UiAction::ToggleDebugGraph => {
            navigation.debug_full_graph = !navigation.debug_full_graph;
            rebuild.0 = true;
        }
        UiAction::RebuildView => {
            rebuild.0 = true;
        }
    }
}

fn apply_simulation_command(simulation: &mut SimulationResource, command: GameCommand) {
    let events = simulation.simulation.apply_command(command);
    simulation.pending_events.extend(events);
}

fn action_available(
    action: UiAction,
    simulation: &SimulationResource,
    navigation: &StrategicNavigation,
) -> bool {
    match action {
        UiAction::TogglePause
        | UiAction::SetSpeed(_)
        | UiAction::ToggleDebugGraph
        | UiAction::RebuildView => true,
        UiAction::CycleTarget => match navigation.mode {
            StrategicViewMode::Universe => {
                !systems_for_universe_view(simulation.simulation(), navigation.debug_full_graph)
                    .is_empty()
            }
            StrategicViewMode::System(system_id) => {
                !visible_planet_ids(simulation.simulation(), system_id).is_empty()
            }
        },
        UiAction::FocusSelection => {
            matches!(navigation.mode, StrategicViewMode::Universe)
                && selected_system(simulation.simulation.state().selected)
                    .and_then(|system_id| simulation.simulation.universe().system(system_id))
                    .is_some()
        }
        UiAction::EnterSystem => {
            matches!(navigation.mode, StrategicViewMode::Universe)
                && enterable_selected_system(simulation, navigation.debug_full_graph).is_some()
        }
        UiAction::ExitSystem => matches!(navigation.mode, StrategicViewMode::System(_)),
        UiAction::AdvanceKnowledge => selected_knowledge_level(simulation.simulation())
            .and_then(KnowledgeLevel::next_exploration_level)
            .is_some(),
    }
}

fn action_active(
    action: UiAction,
    simulation: &SimulationResource,
    navigation: &StrategicNavigation,
) -> bool {
    match action {
        UiAction::TogglePause => simulation.simulation.state().clock.speed().is_paused(),
        UiAction::SetSpeed(speed) => simulation.simulation.state().clock.speed() == speed,
        UiAction::ToggleDebugGraph => navigation.debug_full_graph,
        UiAction::ExitSystem => matches!(navigation.mode, StrategicViewMode::System(_)),
        _ => false,
    }
}

fn selected_knowledge_level(simulation: &Simulation) -> Option<KnowledgeLevel> {
    let state = simulation.state();
    match state.selected {
        SelectionTarget::None => None,
        SelectionTarget::System(system_id) => Some(state.system_knowledge_level(system_id)),
        SelectionTarget::Planet { planet_id, .. } => Some(state.planet_knowledge_level(planet_id)),
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
    let visible_planets = visible_planet_ids(simulation.simulation(), system_id);
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

fn visible_planet_ids(
    simulation: &Simulation,
    system_id: SystemId,
) -> Vec<galactic_domain::PlanetId> {
    let Some(system) = simulation.universe().system(system_id) else {
        return Vec::new();
    };

    system
        .planets
        .iter()
        .filter(|planet| {
            simulation
                .state()
                .planet_knowledge_level(planet.id)
                .is_visible()
        })
        .map(|planet| planet.id)
        .collect()
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

fn apply_keyboard_zoom(
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
        StrategicViewMode::Universe => format!("univers {:?}", navigation.lod),
        StrategicViewMode::System(system_id) => {
            format!("système {}", system_id.index())
        }
    };

    text.0 = format!(
        "Galactic MVP | preset {:?} | {} | tick {} | vitesse {} | cible {}\nSystèmes {}/{} | Routes {}/{} | Détectés/Sondés/Analysés/Colonisés {}/{}/{}/{} | debug {} | {}",
        navigation.preset,
        view_label,
        state.clock.current_tick(),
        state.clock.speed(),
        selected,
        visible_system_count,
        universe.systems.len(),
        visible_route_count,
        universe.routes.len(),
        knowledge.detected,
        knowledge.probed,
        knowledge.analyzed,
        knowledge.colonized,
        navigation.debug_full_graph,
        last_event
    );
}

fn update_info_panel(
    simulation: Res<SimulationResource>,
    mut query: Query<(&mut Text, &mut TextColor), With<InfoPanelText>>,
) {
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };
    let content = information_panel_content(simulation.simulation());
    text.0 = content.render();
    color.0 = knowledge_color(content.level);
}

fn information_panel_content(simulation: &Simulation) -> InspectorContent {
    match simulation.state().selected {
        SelectionTarget::System(system_id) => system_inspector_content(simulation, system_id),
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => planet_inspector_content(simulation, system_id, planet_id),
        SelectionTarget::None => home_inspector_content(simulation),
    }
}

fn home_inspector_content(simulation: &Simulation) -> InspectorContent {
    let state = simulation.state();
    let Some(faction) = state.player_faction_state() else {
        return inspector_error("Faction joueur invalide");
    };
    let Some(colony) = state.player_home_colony() else {
        return inspector_error("Colonie mère introuvable");
    };
    let Some(system) = simulation.universe().system(colony.system_id) else {
        return inspector_error("Système mère introuvable");
    };
    let Some(planet) = simulation.universe_repository().planet(colony.planet_id) else {
        return inspector_error("Planète mère introuvable");
    };

    InspectorContent {
        level: Some(KnowledgeLevel::Colonized),
        badge: knowledge_badge_fr(KnowledgeLevel::Colonized).to_string(),
        title: format!("{} — {}", system.name, planet.name),
        body: format!(
            "Faction : {}
Habitabilité : {}%

{}

POTENTIEL EXACT
Métal : {}
Cristal : {}
Carburant : {}
Énergie : {}

INFRASTRUCTURE
Mines : {}/{}/{}
Centrale : {}
Entrepôt : {}
Construction : {}
Laboratoire : {}
Chantier : {}",
            faction.name,
            planet.habitability,
            colony_economy_text(colony),
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
        ),
        hint: "Colonie active : ressources et énergie sont exactes.".to_string(),
    }
}

fn colony_economy_text(colony: &galactic_sim::ColonyState) -> String {
    let stock = colony.resources.stock();
    let available = colony.resources.available();
    let reserved = colony.resources.reserved_total();
    let production = galactic_sim::colony_production_snapshot(colony);

    format!(
        "STOCKS EXACTS
Total — Métal {}  Cristal {}  Carburant {}
Disponible — Métal {}  Cristal {}  Carburant {}
Réservé — Métal {}  Cristal {}  Carburant {}
Capacité — Métal {}  Cristal {}  Carburant {}

PRODUCTION ACTUELLE
Métal +{:.2}/s  Cristal +{:.2}/s  Carburant +{:.2}/s
Saturation — Métal {}  Cristal {}  Carburant {}

ÉNERGIE — CAPACITÉ
Nominale : {}
Effective planète : {}
Consommation : {}
Efficacité extracteurs : {}%
Bilan effectif : {:+}",
        stock.metal,
        stock.crystal,
        stock.fuel,
        available.metal,
        available.crystal,
        available.fuel,
        reserved.metal,
        reserved.crystal,
        reserved.fuel,
        production.capacity.metal,
        production.capacity.crystal,
        production.capacity.fuel,
        production.effective_rate.metal_per_second(),
        production.effective_rate.crystal_per_second(),
        production.effective_rate.fuel_per_second(),
        format_saturation_time(production.saturation.metal),
        format_saturation_time(production.saturation.crystal),
        format_saturation_time(production.saturation.fuel),
        production.nominal_energy_production,
        production.effective_energy_production,
        colony.energy.consumption(),
        u32::from(production.energy_efficiency_per_mille) / 10,
        i128::from(production.effective_energy_production)
            - i128::from(colony.energy.consumption()),
    )
}

fn format_saturation_time(saturation: galactic_sim::SaturationTime) -> String {
    match saturation {
        galactic_sim::SaturationTime::Full => "plein".to_string(),
        galactic_sim::SaturationTime::Never => "jamais".to_string(),
        galactic_sim::SaturationTime::In(duration) => format_strategic_duration(duration),
    }
}

fn format_strategic_duration(duration: galactic_sim::StrategicDuration) -> String {
    let seconds = duration.as_duration().as_secs();
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let remaining_seconds = seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {remaining_seconds:02}s")
    } else {
        format!("{remaining_seconds}s")
    }
}

fn system_inspector_content(simulation: &Simulation, system_id: SystemId) -> InspectorContent {
    let state = simulation.state();
    let Some(system) = simulation.universe().system(system_id) else {
        return inspector_error(&format!(
            "Référence système invalide : {}",
            system_id.index(),
        ));
    };

    let level = state.system_knowledge_level(system_id);
    let visible_planets = system
        .planets
        .iter()
        .filter(|planet| state.planet_knowledge_level(planet.id).is_visible())
        .count();
    let visible_routes = simulation
        .universe_repository()
        .neighboring_systems(system_id)
        .into_iter()
        .filter(|neighbor| state.is_system_visible(*neighbor))
        .count();

    let (title, body) = match level {
        KnowledgeLevel::Unknown => (
            "Système inconnu".to_string(),
            "Identité : ???\nClasse stellaire : ???\nCorps célestes : ???\nRoutes : ???\nPosition : inconnue"
                .to_string(),
        ),
        KnowledgeLevel::Detected => (
            format!("Signal {}", system_id.index()),
            "Identité : ???\nClasse stellaire : ???\nCorps célestes : non sondés\nRoutes : signaux partiels\nPosition : repérée sur la carte"
                .to_string(),
        ),
        KnowledgeLevel::Probed => (
            system.name.clone(),
            format!(
                "Classe stellaire : {:?}\nLuminosité estimée : {}\nCorps détectés : {}\nRoutes cartographiées : {}\nPosition estimée : x {:.0}  y {:.0}  z {:.0}",
                system.star.class,
                luminosity_estimate(system.star.luminosity),
                visible_planets,
                visible_routes,
                approximate_position(system.position.x),
                approximate_position(system.position.y),
                approximate_position(system.position.z),
            ),
        ),
        KnowledgeLevel::Analyzed | KnowledgeLevel::Colonized => (
            system.name.clone(),
            format!(
                "Classe stellaire : {:?}\nLuminosité exacte : {:.2}\nCorps recensés : {}\nRoutes cartographiées : {}\nPosition exacte : x {:.1}  y {:.1}  z {:.1}",
                system.star.class,
                system.star.luminosity,
                system.planets.len(),
                visible_routes,
                system.position.x,
                system.position.y,
                system.position.z,
            ),
        ),
    };

    InspectorContent {
        level: Some(level),
        badge: knowledge_badge_fr(level).to_string(),
        title,
        body,
        hint: system_knowledge_hint(level).to_string(),
    }
}

fn planet_inspector_content(
    simulation: &Simulation,
    selected_system_id: SystemId,
    planet_id: galactic_domain::PlanetId,
) -> InspectorContent {
    let state = simulation.state();
    let Some((system_id, planet)) = simulation.universe_repository().planet_location(planet_id)
    else {
        return inspector_error(&format!(
            "Référence planète invalide : {}",
            planet_id.index(),
        ));
    };
    let Some(system) = simulation.universe().system(system_id) else {
        return inspector_error("Système de la planète introuvable");
    };

    let level = state.planet_knowledge_level(planet_id);
    let colony = state.colony_on_planet(planet_id);
    let system_label = if state.system_knowledge_level(system_id).reveals_identity() {
        system.name.clone()
    } else {
        format!("Signal {}", system_id.index())
    };
    let selection_note = if selected_system_id == system_id {
        "Sélection : cohérente"
    } else {
        "Sélection : recoupée avec le système réel"
    };

    let (title, mut body) = match level {
        KnowledgeLevel::Unknown => (
            "Corps inconnu".to_string(),
            format!(
                "Système : {}
Nom : ???
Type : ???
Habitabilité : ???
Potentiel : ???
Lunes : ???
{}",
                system_label, selection_note,
            ),
        ),
        KnowledgeLevel::Detected => (
            format!("Corps détecté {}", planet_id.index()),
            format!(
                "Système : {}
Nom : ???
Type : ???
Habitabilité : ???
Potentiel : analyse requise
Lunes : non recensées
{}",
                system_label, selection_note,
            ),
        ),
        KnowledgeLevel::Probed => (
            planet.name.clone(),
            format!(
                "Système : {}
Type : {:?}
Habitabilité estimée : {}
Potentiel : analyse requise
Lunes : non recensées
{}",
                system_label,
                planet.kind,
                habitability_estimate(planet.habitability),
                selection_note,
            ),
        ),
        KnowledgeLevel::Analyzed => (
            planet.name.clone(),
            format!(
                "Système : {}
Type : {:?}
Habitabilité exacte : {}%
Statut : non colonisée
Potentiel : aucune valeur économique générée pour ce corps
Lunes : aucune donnée disponible
{}",
                system_label, planet.kind, planet.habitability, selection_note,
            ),
        ),
        KnowledgeLevel::Colonized => (
            planet.name.clone(),
            format!(
                "Système : {}
Type : {:?}
Habitabilité exacte : {}%
Statut : {}
Lunes : aucune donnée disponible
{}",
                system_label,
                planet.kind,
                planet.habitability,
                colony
                    .map(|value| value.name.as_str())
                    .unwrap_or("colonie non référencée"),
                selection_note,
            ),
        ),
    };

    if let Some(colony) = colony {
        body.push_str(&format!(
            "

{}

POTENTIEL EXACT
Métal : {}
Cristal : {}
Carburant : {}
Énergie : {}

INFRASTRUCTURE
Mines : {}/{}/{}
Centrale : {}
Entrepôt : {}
Construction : {}
Laboratoire : {}
Chantier : {}",
            colony_economy_text(colony),
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
        ));
    }

    InspectorContent {
        level: Some(level),
        badge: knowledge_badge_fr(level).to_string(),
        title,
        body,
        hint: planet_knowledge_hint(level).to_string(),
    }
}

fn inspector_error(message: &str) -> InspectorContent {
    InspectorContent {
        level: None,
        badge: "[ERREUR D’INSPECTEUR]".to_string(),
        title: "Donnée indisponible".to_string(),
        body: message.to_string(),
        hint: "La sélection ne correspond pas à une donnée valide.".to_string(),
    }
}

const fn knowledge_badge_fr(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "[INCONNU — DONNÉES MASQUÉES]",
        KnowledgeLevel::Detected => "[DÉTECTÉ — DONNÉES MASQUÉES]",
        KnowledgeLevel::Probed => "[SONDÉ — ESTIMATIONS]",
        KnowledgeLevel::Analyzed => "[ANALYSÉ — VALEURS EXACTES]",
        KnowledgeLevel::Colonized => "[COLONISÉ — VALEURS EXACTES]",
    }
}

const fn system_knowledge_hint(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "Action requise : détecter le système.",
        KnowledgeLevel::Detected => "Action requise : sonder le système pour révéler son identité.",
        KnowledgeLevel::Probed => {
            "Action requise : analyser le système pour obtenir les valeurs exactes."
        }
        KnowledgeLevel::Analyzed => "Analyse terminée : les valeurs disponibles sont exactes.",
        KnowledgeLevel::Colonized => "Système colonisé : les valeurs disponibles sont exactes.",
    }
}

const fn planet_knowledge_hint(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "Action requise : détecter ce corps céleste.",
        KnowledgeLevel::Detected => "Action requise : sonder la planète pour révéler son identité.",
        KnowledgeLevel::Probed => {
            "Action requise : analyser la planète pour obtenir les valeurs exactes."
        }
        KnowledgeLevel::Analyzed => {
            "Analyse terminée : les caractéristiques disponibles sont exactes."
        }
        KnowledgeLevel::Colonized => "Planète colonisée : les données économiques sont exactes.",
    }
}

fn knowledge_color(level: Option<KnowledgeLevel>) -> Color {
    match level {
        None | Some(KnowledgeLevel::Unknown) => Color::srgb(0.72, 0.76, 0.80),
        Some(KnowledgeLevel::Detected) => Color::srgb(0.58, 0.72, 0.88),
        Some(KnowledgeLevel::Probed) => Color::srgb(0.56, 0.88, 0.94),
        Some(KnowledgeLevel::Analyzed) => Color::srgb(0.96, 0.82, 0.48),
        Some(KnowledgeLevel::Colonized) => Color::srgb(0.58, 0.94, 0.72),
    }
}

fn luminosity_estimate(luminosity: f32) -> &'static str {
    if luminosity < 0.6 {
        "faible"
    } else if luminosity < 1.6 {
        "moyenne"
    } else if luminosity < 2.6 {
        "forte"
    } else {
        "très forte"
    }
}

fn habitability_estimate(habitability: u8) -> &'static str {
    match habitability {
        0..=19 => "très faible",
        20..=39 => "faible",
        40..=59 => "moyenne",
        60..=79 => "bonne",
        _ => "excellente",
    }
}

fn approximate_position(value: f32) -> f32 {
    (value / 5.0).round() * 5.0
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
        SelectionTarget::None => "aucune".to_string(),
        SelectionTarget::System(system_id) => {
            format!("système {}", system_id.index())
        }
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => format!("planète {}:{}", system_id.index(), planet_id.index()),
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
    fn screen_space_radius_is_constant_in_pixels() {
        assert!(screen_space_hit(
            Vec2::new(100.0, 100.0),
            Vec2::new(116.0, 100.0),
            16.0,
        ));
        assert!(!screen_space_hit(
            Vec2::new(100.0, 100.0),
            Vec2::new(117.0, 100.0),
            16.0,
        ));
    }

    #[test]
    fn candidate_ranking_is_deterministic() {
        let near_system = PickTarget::System(SystemId::new(2));
        let priority_system = PickTarget::System(SystemId::new(1));
        let deeper_planet = PickTarget::Planet {
            system_id: SystemId::new(0),
            planet_id: PlanetId::new(1),
        };
        let mut candidates = vec![
            PointerCandidate {
                target: deeper_planet,
                screen_position: Vec2::ZERO,
                screen_distance: 4.0,
                depth: 20.0,
                priority: 80,
            },
            PointerCandidate {
                target: priority_system,
                screen_position: Vec2::ZERO,
                screen_distance: 4.0,
                depth: 15.0,
                priority: 100,
            },
            PointerCandidate {
                target: near_system,
                screen_position: Vec2::ZERO,
                screen_distance: 2.0,
                depth: 30.0,
                priority: 10,
            },
        ];

        rank_pointer_candidates(&mut candidates);

        assert_eq!(candidates[0].target, near_system);
        assert_eq!(candidates[1].target, priority_system);
        assert_eq!(candidates[2].target, deeper_planet);
    }

    #[test]
    fn ambiguity_cycle_wraps_in_both_directions() {
        let first = PickTarget::System(SystemId::new(1));
        let second = PickTarget::System(SystemId::new(2));
        let mut pointer_state = PointerSelectionState {
            ambiguity: Some(AmbiguitySelection {
                targets: vec![first, second],
                active_index: 0,
            }),
            ..default()
        };

        assert_eq!(pointer_state.cycle_ambiguity(false), Some(second));
        assert_eq!(pointer_state.cycle_ambiguity(false), Some(first));
        assert_eq!(pointer_state.cycle_ambiguity(true), Some(second));
    }

    #[test]
    fn double_click_requires_same_target_time_and_position() {
        let target = PickTarget::System(SystemId::new(3));
        let previous = PointerClickRecord {
            target,
            at: Duration::from_millis(100),
            cursor_position: Vec2::new(40.0, 50.0),
        };

        assert!(pointer_double_click(
            previous,
            target,
            Duration::from_millis(400),
            Vec2::new(44.0, 50.0),
        ));
        assert!(!pointer_double_click(
            previous,
            PickTarget::System(SystemId::new(4)),
            Duration::from_millis(400),
            Vec2::new(44.0, 50.0),
        ));
        assert!(!pointer_double_click(
            previous,
            target,
            Duration::from_millis(500),
            Vec2::new(44.0, 50.0),
        ));
    }

    #[test]
    fn unknown_targets_are_not_pickable_even_in_debug_rendering() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let unknown = simulation
            .universe()
            .systems
            .iter()
            .find(|system| !simulation.state().is_system_visible(system.id))
            .expect("the MVP universe contains an unknown system")
            .id;

        assert!(!pick_target_is_visible(
            &simulation,
            PickTarget::System(unknown),
        ));
    }

    #[test]
    fn detected_pointer_labels_do_not_reveal_identity() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let detected = simulation
            .state()
            .system_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("a detected frontier system exists")
            .system_id;
        let actual_name = &simulation
            .universe()
            .system(detected)
            .expect("detected system exists")
            .name;

        let label = pick_target_label(&simulation, PickTarget::System(detected));

        assert!(label.contains("Signal"));
        assert!(!label.contains(actual_name));
    }

    #[test]
    fn ui_font_uses_a_system_sans_serif() {
        assert!(matches!(ui_text_font(14.0).font, FontSource::SansSerif));
    }

    #[test]
    fn detected_system_inspector_masks_secret_values() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let state = simulation.state();
        let detected = state
            .system_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the starting frontier contains a detected system")
            .system_id;
        let system = simulation
            .universe()
            .system(detected)
            .expect("detected system exists");

        let rendered = system_inspector_content(&simulation, detected).render();

        assert!(rendered.contains("DÉTECTÉ"));
        assert!(rendered.contains("Identité : ???"));
        assert!(rendered.contains("Classe stellaire : ???"));
        assert!(!rendered.contains(&system.name));
        assert!(!rendered.contains(&format!("{:?}", system.star.class)));
        assert!(!rendered.contains(&format!("{:.1}", system.position.x)));
    }

    #[test]
    fn system_inspector_distinguishes_estimates_and_exact_values() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let detected = simulation
            .state()
            .system_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the starting frontier contains a detected system")
            .system_id;
        simulation.apply_command(GameCommand::SelectSystem(detected));
        simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);

        let probed = system_inspector_content(&simulation, detected).render();
        assert!(probed.contains("SONDÉ"));
        assert!(probed.contains("Luminosité estimée"));

        simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);
        let analyzed = system_inspector_content(&simulation, detected).render();
        let system = simulation
            .universe()
            .system(detected)
            .expect("analyzed system exists");

        assert!(analyzed.contains("ANALYSÉ"));
        assert!(analyzed.contains("Luminosité exacte"));
        assert!(analyzed.contains(&format!("{:.2}", system.star.luminosity)));
    }

    #[test]
    fn detected_planet_inspector_hides_identity_and_habitability() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let detected = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, planet) = simulation
            .universe_repository()
            .planet_location(detected)
            .expect("detected planet exists");

        let rendered = planet_inspector_content(&simulation, system_id, detected).render();

        assert!(rendered.contains("DÉTECTÉ"));
        assert!(rendered.contains("Nom : ???"));
        assert!(rendered.contains("Habitabilité : ???"));
        assert!(!rendered.contains(&planet.name));
        assert!(!rendered.contains(&format!("{:?}", planet.kind)));
    }

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

        assert_eq!(label, "planète 2:1");
    }

    #[test]
    fn debug_shortcut_uses_g_instead_of_function_keys() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyG);

        assert_eq!(view_shortcut(&keyboard), Some(UiAction::ToggleDebugGraph));

        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::F3);

        assert_eq!(view_shortcut(&keyboard), None);
    }

    #[test]
    fn azerty_pan_keys_match_visible_zqsd_labels() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(AZERTY_FORWARD_KEY);
        assert_eq!(keyboard_pan_direction(&keyboard, 0.0), Vec3::NEG_Z);

        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(AZERTY_LEFT_KEY);
        assert_eq!(keyboard_pan_direction(&keyboard, 0.0), Vec3::NEG_X);

        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(AZERTY_BACKWARD_KEY);
        assert_eq!(keyboard_pan_direction(&keyboard, 0.0), Vec3::Z);

        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(AZERTY_RIGHT_KEY);
        assert_eq!(keyboard_pan_direction(&keyboard, 0.0), Vec3::X);
    }

    #[test]
    fn enter_system_action_requires_revealed_system_or_debug_graph() {
        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut navigation = StrategicNavigation {
            mode: StrategicViewMode::Universe,
            ..default()
        };

        assert!(action_available(
            UiAction::EnterSystem,
            &simulation,
            &navigation
        ));

        let neighbor = simulation
            .simulation()
            .universe_repository()
            .neighboring_systems(MVP_HOME_SYSTEM_ID)
            .into_iter()
            .next()
            .expect("home system has a frontier neighbor");
        let events = simulation
            .simulation
            .apply_command(GameCommand::SelectSystem(neighbor));
        simulation.pending_events.extend(events);

        assert!(!action_available(
            UiAction::EnterSystem,
            &simulation,
            &navigation
        ));

        navigation.debug_full_graph = true;

        assert!(action_available(
            UiAction::EnterSystem,
            &simulation,
            &navigation
        ));
    }

    #[test]
    fn planet_information_panel_includes_home_colony_details() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let panel = information_panel_content(&simulation);
        let rendered = panel.render();

        assert_eq!(panel.level, Some(KnowledgeLevel::Colonized));
        assert!(rendered.contains("Aster Prime Colony"));
        assert!(rendered.contains("STOCKS EXACTS"));
        assert!(rendered.contains("INFRASTRUCTURE"));
    }
}

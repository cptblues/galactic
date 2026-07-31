mod craft_ui;
mod research_ui;

use craft_ui::CraftUiPlugin;
use research_ui::ResearchUiPlugin;
use std::{collections::HashMap, time::Duration};

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::{
    RenderPlugin,
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    settings::{MemoryHints, RenderCreation, WgpuSettings},
};
use bevy::text::{FontAtlasSet, FontCx, FontSource};
use bevy::window::{PresentMode, PrimaryWindow};
use galactic_domain::{
    ExtractionSiteId, PlanetId, PlanetKind, ResourceKind, ResourceStock, StarClass, SystemId,
    UniverseConfig, UniverseScalePreset, WorldPosition,
};
use galactic_sim::{
    AttackMissionOutcome, ColonizationBlocker, ColonizationMissionOutcome, CombatControlChange,
    CombatOutcome, CombatReportStatus, DiplomaticRelation, EstimateRange, GameAction, GameEvent,
    GameEventKind, InstallationConstraint, KnowledgeLevel, KnowledgeTarget, MVP_HOME_SYSTEM_ID,
    MissionKind, MissionPhase, MissionResult, MissionTarget, PlanetAnalysisError,
    PlanetEnvironment, PlanetaryForceDomain, PlanetaryIntelPrecision, PlanetaryOccupancyIntel,
    SelectionTarget, Simulation, SystemVisibility, TechnologyUnlock, TimeSpeed,
    assess_planet_colonizability, default_ruleset,
};

const UNIVERSE_VERTICAL_EXAGGERATION: f32 = 3.4;
const INITIAL_OBSERVATION_SYSTEM_LIMIT: usize = 14;
const PLANET_TEXTURE_WIDTH: u32 = 64;
const PLANET_TEXTURE_HEIGHT: u32 = 32;

pub fn run() {
    let scale_preset = match universe_scale_preset_from_args(std::env::args().skip(1)) {
        Ok(preset) => preset,
        Err(message) => {
            eprintln!("{message}");
            return;
        }
    };
    App::new()
        .add_plugins(ClientPlugin::new(scale_preset))
        .run();
}

pub struct ClientPlugin {
    scale_preset: UniverseScalePreset,
}

impl ClientPlugin {
    pub const fn new(scale_preset: UniverseScalePreset) -> Self {
        Self { scale_preset }
    }
}

impl Default for ClientPlugin {
    fn default() -> Self {
        Self::new(UniverseScalePreset::default())
    }
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        let simulation = Simulation::new(UniverseConfig::for_preset(
            galactic_domain::MVP_UNIVERSE_SEED,
            self.scale_preset,
        ));
        let navigation =
            StrategicNavigation::for_universe(self.scale_preset, simulation.universe());

        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Galactic MVP".to_string(),
                        resolution: (1280, 720).into(),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(low_memory_render_plugin()),
        )
        .insert_resource(ClearColor(Color::srgb(0.006, 0.008, 0.014)))
        .insert_resource(SimulationResource {
            simulation,
            pending_events: Vec::new(),
        })
        .init_resource::<PresentationLog>()
        .init_resource::<VisualAssets>()
        .insert_resource(navigation)
        .init_resource::<ViewRebuildRequest>()
        .init_resource::<PointerSelectionState>()
        .init_resource::<ColonyManagementState>()
        .init_resource::<MemoryDiagnostics>()
        .add_plugins(SimulationBridgePlugin)
        .add_plugins(PresentationPlugin)
        .add_plugins(ResearchUiPlugin)
        .add_plugins(CraftUiPlugin)
        .add_systems(Startup, log_startup)
        .add_systems(Update, log_memory_diagnostics);
    }
}

fn low_memory_render_plugin() -> RenderPlugin {
    RenderPlugin {
        render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
            memory_hints: MemoryHints::MemoryUsage,
            ..default()
        })),
        ..default()
    }
}

fn universe_scale_preset_from_args(
    args: impl IntoIterator<Item = String>,
) -> Result<UniverseScalePreset, String> {
    let mut args = args.into_iter();
    let mut preset = UniverseScalePreset::default();

    while let Some(argument) = args.next() {
        let value = if argument == "--scale" {
            args.next()
                .ok_or_else(|| "Option --scale sans valeur (test, mvp ou stress).".to_string())?
        } else if let Some(value) = argument.strip_prefix("--scale=") {
            value.to_string()
        } else {
            return Err(format!(
                "Option inconnue « {argument} ». Utiliser --scale test|mvp|stress."
            ));
        };

        preset = UniverseScalePreset::from_slug(&value).ok_or_else(|| {
            format!("Preset inconnu « {value} ». Valeurs acceptées : test, mvp, stress.")
        })?;
    }

    Ok(preset)
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
            (
                install_stable_ui_font,
                spawn_scene,
                spawn_strategic_view,
                spawn_ui,
            )
                .chain(),
        )
        .configure_sets(
            Update,
            (
                PresentationUpdateSet::View,
                PresentationUpdateSet::Interaction,
                PresentationUpdateSet::Management,
                PresentationUpdateSet::Ui,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_projection_transition,
                update_system_visuals,
                update_orbiting_visuals,
                update_planet_spins,
                update_system_labels,
                update_sector_labels,
                update_pointer_halo_positions,
                update_strategic_camera,
                update_pointer_candidates,
                handle_pointer_selection,
                capture_colony_management_feedback,
                collect_presentation_events,
                update_pointer_halos,
                draw_strategic_overlays,
            )
                .chain()
                .in_set(PresentationUpdateSet::View),
        )
        .add_systems(
            Update,
            (handle_action_buttons, handle_colony_management_buttons)
                .chain()
                .in_set(PresentationUpdateSet::Interaction),
        )
        .add_systems(
            Update,
            (
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_colony_management_visibility,
                update_colony_management_resources,
                update_colony_management_buildings,
                update_colony_management_detail,
                update_colony_management_queue,
                update_colony_management_transport,
            )
                .chain()
                .in_set(PresentationUpdateSet::Management),
        )
        .add_systems(
            Update,
            (update_ui, update_info_panel)
                .chain()
                .in_set(PresentationUpdateSet::Ui),
        );
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PresentationUpdateSet {
    View,
    Interaction,
    Management,
    Ui,
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
struct MemoryDiagnostics {
    enabled: bool,
    sample_timer: Timer,
}

impl Default for MemoryDiagnostics {
    fn default() -> Self {
        let enabled = std::env::var("GALACTIC_MEMORY_DIAGNOSTICS").is_ok_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        });
        Self {
            enabled,
            sample_timer: Timer::from_seconds(15.0, TimerMode::Repeating),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ProcessMemorySnapshot {
    rss_kib: u64,
    anonymous_kib: u64,
    file_kib: u64,
    shared_kib: u64,
    swap_kib: u64,
}

#[derive(SystemParam)]
struct MemoryDiagnosticSources<'w, 's> {
    simulation: Res<'w, SimulationResource>,
    entities: Query<'w, 's, Entity>,
    strategic_entities: Query<'w, 's, Entity, With<StrategicViewEntity>>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<StandardMaterial>>,
    images: Res<'w, Assets<Image>>,
    font_atlases: Res<'w, FontAtlasSet>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum GraphicsPreset {
    #[default]
    Low,
}

#[derive(Resource)]
struct VisualAssets {
    system_mesh: Handle<Mesh>,
    planet_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    known_star_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    star_halo_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    detected_material: Handle<StandardMaterial>,
    observed_material: Handle<StandardMaterial>,
    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
    atmosphere_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
    ring_material: Handle<StandardMaterial>,
    hover_material: Handle<StandardMaterial>,
}

impl FromWorld for VisualAssets {
    fn from_world(world: &mut World) -> Self {
        // Low preset: every geometry and material is shared by all matching bodies.
        let (system_mesh, planet_mesh, ring_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Sphere::default().mesh().ico(1).unwrap()),
                meshes.add(Sphere::default().mesh().uv(32, 18)),
                meshes.add(Annulus::new(1.55, 2.25)),
            )
        };
        let planet_textures = {
            let mut images = world.resource_mut::<Assets<Image>>();
            PlanetKind::ALL
                .into_iter()
                .map(|kind| (kind, images.add(procedural_planet_texture(kind))))
                .collect::<HashMap<_, _>>()
        };

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let known_star_materials = StarClass::ALL
            .into_iter()
            .map(|class| (class, materials.add(star_material(class))))
            .collect();
        let star_halo_materials = StarClass::ALL
            .into_iter()
            .map(|class| (class, materials.add(star_halo_material(class))))
            .collect();
        let detected_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.34, 0.48, 0.62, 0.75),
            emissive: LinearRgba::rgb(0.28, 0.42, 0.62),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let observed_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.24, 0.30, 0.42, 0.30),
            emissive: LinearRgba::rgb(0.12, 0.18, 0.30),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let planet_materials = PlanetKind::ALL
            .into_iter()
            .map(|kind| {
                let texture = planet_textures
                    .get(&kind)
                    .expect("planet texture exists")
                    .clone();
                (kind, materials.add(planet_material(kind, texture)))
            })
            .collect();
        let atmosphere_materials = [PlanetKind::Ocean, PlanetKind::Ice, PlanetKind::GasGiant]
            .into_iter()
            .map(|kind| (kind, materials.add(atmosphere_material(kind))))
            .collect();
        let ring_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.66, 0.56, 0.48, 0.54),
            emissive: LinearRgba::rgb(0.12, 0.09, 0.07),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let hover_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.28, 0.92, 0.82, 0.18),
            emissive: LinearRgba::rgb(0.18, 1.2, 0.92),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        Self {
            system_mesh,
            planet_mesh,
            ring_mesh,
            known_star_materials,
            star_halo_materials,
            detected_material,
            observed_material,
            planet_materials,
            atmosphere_materials,
            ring_material,
            hover_material,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniverseSystemTier {
    Known,
    Detected,
    Observed,
}

impl UniverseSystemTier {
    const fn from_visibility(visibility: SystemVisibility) -> Self {
        match visibility {
            SystemVisibility::Known => Self::Known,
            SystemVisibility::Detected => Self::Detected,
        }
    }

    const fn visibility(self) -> Option<SystemVisibility> {
        match self {
            Self::Known => Some(SystemVisibility::Known),
            Self::Detected => Some(SystemVisibility::Detected),
            Self::Observed => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UniverseSystemEntry {
    id: SystemId,
    tier: UniverseSystemTier,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum UniverseProjection {
    #[default]
    Spatial,
    Flattened,
}

impl UniverseProjection {
    const fn target_mix(self) -> f32 {
        match self {
            Self::Spatial => 0.0,
            Self::Flattened => 1.0,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Spatial => "3D",
            Self::Flattened => "2,5D",
        }
    }
}

#[derive(Resource)]
struct StrategicNavigation {
    mode: StrategicViewMode,
    scale_preset: UniverseScalePreset,
    universe_focus: Vec3,
    universe_distance: f32,
    universe_max_distance: f32,
    universe_yaw: f32,
    universe_pitch: f32,
    system_focus: Vec3,
    system_distance: f32,
    system_yaw: f32,
    system_pitch: f32,
    lod: UniverseLod,
    debug_full_graph: bool,
    preset: GraphicsPreset,
    projection: UniverseProjection,
    projection_mix: f32,
}

impl Default for StrategicNavigation {
    fn default() -> Self {
        Self::new(UniverseScalePreset::default(), 108.0)
    }
}

impl StrategicNavigation {
    fn for_universe(
        scale_preset: UniverseScalePreset,
        universe: &galactic_domain::UniverseDefinition,
    ) -> Self {
        let extent = universe
            .systems
            .iter()
            .map(|system| {
                let position = to_vec3(system.position);
                Vec2::new(position.x, position.z).length()
            })
            .fold(0.0_f32, f32::max);
        let mut navigation = Self::new(scale_preset, 108.0);
        navigation.universe_max_distance = (extent * 1.8 + 80.0).max(150.0);
        navigation
    }

    fn new(scale_preset: UniverseScalePreset, universe_distance: f32) -> Self {
        Self {
            mode: StrategicViewMode::System(MVP_HOME_SYSTEM_ID),
            scale_preset,
            universe_focus: Vec3::ZERO,
            universe_distance,
            universe_max_distance: (universe_distance * 1.8).max(150.0),
            universe_yaw: 0.0,
            universe_pitch: -0.62,
            system_focus: Vec3::ZERO,
            system_distance: 34.0,
            system_yaw: 0.0,
            system_pitch: -0.62,
            lod: UniverseLod::from_distance(universe_distance),
            debug_full_graph: false,
            preset: GraphicsPreset::Low,
            projection: UniverseProjection::Spatial,
            projection_mix: 0.0,
        }
    }

    fn enter_system(&mut self, system_id: SystemId) {
        self.mode = StrategicViewMode::System(system_id);
    }

    fn exit_system(&mut self) {
        self.mode = StrategicViewMode::Universe;
    }

    fn toggle_projection(&mut self) {
        self.projection = match self.projection {
            UniverseProjection::Spatial => UniverseProjection::Flattened,
            UniverseProjection::Flattened => UniverseProjection::Spatial,
        };
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
    tier: UniverseSystemTier,
    base_scale: Vec3,
}

#[derive(Component)]
struct SystemLabel {
    id: SystemId,
    visibility: SystemVisibility,
}

#[derive(Component)]
struct SectorLabel {
    position: WorldPosition,
}

#[derive(Component, Debug, Clone, Copy)]
struct OrbitingVisual {
    radius: f32,
    phase: f32,
    angular_speed: f32,
    vertical_offset: f32,
}

impl OrbitingVisual {
    fn translation_at(self, elapsed_seconds: f32) -> Vec3 {
        let angle = self.phase + elapsed_seconds * self.angular_speed;
        Vec3::new(
            angle.cos() * self.radius,
            self.vertical_offset,
            angle.sin() * self.radius,
        )
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct AxialSpin {
    radians_per_second: f32,
}

#[derive(Debug, Clone, Copy)]
struct RouteVisualStyle {
    color: Color,
    dash_length: f32,
    gap_length: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct KnownSectorLabel {
    text: String,
    position: WorldPosition,
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

// MVP-015: dedicated colony management screen.
#[derive(Resource)]
struct ColonyManagementState {
    open: bool,
    selected_building: galactic_sim::BuildingKind,
    transport_destination_id: Option<galactic_domain::ColonyId>,
    transport_cargo: TransportCargoPreset,
    feedback: String,
}

impl Default for ColonyManagementState {
    fn default() -> Self {
        Self {
            open: false,
            selected_building: galactic_sim::default_building_catalog()
                .definitions()
                .next()
                .expect("validated ruleset contains at least one building")
                .kind,
            transport_destination_id: None,
            transport_cargo: TransportCargoPreset::Mixed,
            feedback: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportCargoPreset {
    Metal,
    Crystal,
    Fuel,
    Mixed,
}

impl TransportCargoPreset {
    const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Mixed];

    const fn cargo(self) -> ResourceStock {
        match self {
            Self::Metal => ResourceStock::new(400, 0, 0),
            Self::Crystal => ResourceStock::new(0, 400, 0),
            Self::Fuel => ResourceStock::new(0, 0, 300),
            Self::Mixed => ResourceStock::new(200, 150, 100),
        }
    }

    const fn short_label(self) -> &'static str {
        match self {
            Self::Metal => "M 400",
            Self::Crystal => "C 400",
            Self::Fuel => "F 300",
            Self::Mixed => "Mixte",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceHudKind {
    Metal,
    Crystal,
    Fuel,
    Energy,
}

impl ResourceHudKind {
    const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Energy];

    const fn title(self) -> &'static str {
        match self {
            Self::Metal => "MÉTAL",
            Self::Crystal => "CRISTAL",
            Self::Fuel => "CARBURANT",
            Self::Energy => "ÉNERGIE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceHudStatus {
    Normal,
    NearlyFull,
    Full,
    Deficit,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ManagementButtonAction {
    Toggle,
    Close,
    PreviousColony,
    NextColony,
    PreviousTransportDestination,
    NextTransportDestination,
    SelectTransportCargo(TransportCargoPreset),
    LaunchTransport,
    SelectBuilding(galactic_sim::BuildingKind),
    UpgradeSelected,
}

type ManagementButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ManagementButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct ColonyManagementRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ManagementTextRole {
    ToggleLabel,
    Title,
    Colony,
    ColonyList,
    TransportDestination,
    TransportCargo,
    TransportLaunchLabel,
    Feedback,
    BuildingDetail,
    UpgradeLabel,
    Queue,
}

#[derive(Component)]
struct ManagementResourceCardText {
    kind: ResourceHudKind,
}

#[derive(Component)]
struct ManagementResourceGaugeFill {
    kind: ResourceHudKind,
}

#[derive(Component)]
struct ManagementBuildingButton {
    kind: galactic_sim::BuildingKind,
}

#[derive(Component)]
struct ManagementBuildingButtonText {
    kind: galactic_sim::BuildingKind,
}

#[derive(Component)]
struct ManagementUpgradeButton;

#[derive(Component)]
struct ManagementTransportLaunchButton;

#[derive(Component)]
struct ManagementTransportPresetButton {
    preset: TransportCargoPreset,
}

type ManagementTransportLaunchStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    (
        With<ManagementTransportLaunchButton>,
        Without<ManagementTransportPresetButton>,
    ),
>;

type ManagementTransportPresetStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ManagementTransportPresetButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    Without<ManagementTransportLaunchButton>,
>;

#[derive(Component)]
struct ManagementQueueProgressFill;

const COLONY_MANAGEMENT_Z_INDEX: i32 = 100;

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
    LaunchProbe,
    LaunchAttack,
    LaunchHarvest,
    LaunchColonization,
    AnalyzePlanet,
    ToggleProjection,
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

fn install_stable_ui_font(mut fonts: ResMut<Assets<Font>>, mut font_cx: ResMut<FontCx>) {
    let Some(font_data) = stable_system_sans_serif_data(&mut font_cx) else {
        warn!(
            "No system sans-serif font could be frozen; using Bevy's ASCII fallback font instead"
        );
        return;
    };

    fonts
        .insert(AssetId::default(), Font::from_bytes(font_data))
        .expect("Bevy's default font asset should already be reserved");
}

fn stable_system_sans_serif_data(font_cx: &mut FontCx) -> Option<Vec<u8>> {
    let family_name = font_cx.get_family(&FontSource::SansSerif)?.to_owned();
    let family = font_cx.context.collection.family_by_name(&family_name)?;
    let font = family.default_font()?;
    let data = font.load(Some(&mut font_cx.context.source_cache))?;
    Some(data.as_ref().to_vec())
}

fn log_memory_diagnostics(
    time: Res<Time>,
    mut diagnostics: ResMut<MemoryDiagnostics>,
    sources: MemoryDiagnosticSources,
) {
    if !diagnostics.enabled || !diagnostics.sample_timer.tick(time.delta()).just_finished() {
        return;
    }

    let state = sources.simulation.simulation().state();
    let process = process_memory_snapshot();
    info!(
        target: "galactic_memory",
        "rss={} MiB anon={} MiB file={} MiB shmem={} MiB swap={} MiB | entities={} strategic={} meshes={} materials={} images={} font_atlas={} MiB pending_events={} missions={} reports={}",
        kib_to_mib(process.rss_kib),
        kib_to_mib(process.anonymous_kib),
        kib_to_mib(process.file_kib),
        kib_to_mib(process.shared_kib),
        kib_to_mib(process.swap_kib),
        sources.entities.iter().count(),
        sources.strategic_entities.iter().count(),
        sources.meshes.len(),
        sources.materials.len(),
        sources.images.len(),
        kib_to_mib(sources.font_atlases.total_bytes(&sources.images) / 1024),
        sources.simulation.pending_events.len(),
        state.missions.len(),
        state.mission_reports.len(),
    );
}

fn kib_to_mib(value: u64) -> u64 {
    value / 1024
}

#[cfg(target_os = "linux")]
fn process_memory_snapshot() -> ProcessMemorySnapshot {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .map(|status| parse_linux_process_status(&status))
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
fn process_memory_snapshot() -> ProcessMemorySnapshot {
    ProcessMemorySnapshot::default()
}

fn parse_linux_process_status(status: &str) -> ProcessMemorySnapshot {
    let mut snapshot = ProcessMemorySnapshot::default();
    for line in status.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let value_kib = value
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        match label {
            "VmRSS" => snapshot.rss_kib = value_kib,
            "RssAnon" => snapshot.anonymous_kib = value_kib,
            "RssFile" => snapshot.file_kib = value_kib,
            "RssShmem" => snapshot.shared_kib = value_kib,
            "VmSwap" => snapshot.swap_kib = value_kib,
            _ => {}
        }
    }
    snapshot
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

    for entry in visible_systems {
        let Some(system) = universe.system(entry.id) else {
            continue;
        };

        let material = match entry.tier {
            UniverseSystemTier::Known => assets
                .known_star_materials
                .get(&system.star.class)
                .expect("star material exists")
                .clone(),
            UniverseSystemTier::Detected => assets.detected_material.clone(),
            UniverseSystemTier::Observed => assets.observed_material.clone(),
        };
        let visibility_scale = match entry.tier {
            UniverseSystemTier::Known => 1.0,
            UniverseSystemTier::Detected => 0.72,
            UniverseSystemTier::Observed => 0.44,
        };
        let scale = Vec3::splat((0.72 + system.star.luminosity.min(2.4) * 0.16) * visibility_scale);
        let position = projected_universe_position(system.position, navigation.projection_mix);

        let mut entity = commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(scale),
            SystemVisual {
                id: system.id,
                tier: entry.tier,
                base_scale: scale,
            },
            StrategicViewEntity,
        ));
        let selectable_visibility = entry.tier.visibility().or_else(|| {
            navigation
                .debug_full_graph
                .then_some(SystemVisibility::Detected)
        });
        if let Some(visibility) = selectable_visibility {
            entity.insert(SelectableVisual {
                target: PickTarget::System(system.id),
                pick_radius_px: 18.0,
                priority: system_pick_priority(simulation, system.id, visibility),
            });
            spawn_pointer_halo(
                commands,
                assets,
                PickTarget::System(system.id),
                position,
                scale.x * 1.65,
            );
        }

        if entry.tier == UniverseSystemTier::Known {
            spawn_star_halo(
                commands,
                assets,
                system.id,
                system.star.class,
                position,
                scale.x * 1.8,
            );
        }

        if let Some(visibility) = selectable_visibility {
            let label = match entry.tier {
                UniverseSystemTier::Known => system.name.clone(),
                UniverseSystemTier::Detected => format!("Signal {}", system.id.index()),
                UniverseSystemTier::Observed => format!("Observation {}", system.id.index()),
            };

            commands.spawn((
                Text2d::new(label),
                ui_text_font(12.0),
                TextColor(match entry.tier {
                    UniverseSystemTier::Known => Color::srgba(0.76, 0.88, 1.0, 0.90),
                    UniverseSystemTier::Detected => Color::srgba(0.48, 0.66, 0.82, 0.72),
                    UniverseSystemTier::Observed => Color::srgba(0.38, 0.44, 0.56, 0.52),
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
    }

    for sector_label in known_sector_labels(simulation) {
        let position =
            projected_universe_position(sector_label.position, navigation.projection_mix);
        commands.spawn((
            Text2d::new(sector_label.text),
            ui_text_font(18.0),
            TextColor(Color::srgba(0.96, 0.74, 0.36, 0.88)),
            Transform::from_translation(position + Vec3::new(0.0, 4.2, 0.0))
                .with_scale(Vec3::splat(0.36)),
            SectorLabel {
                position: sector_label.position,
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

fn known_sector_labels(simulation: &Simulation) -> Vec<KnownSectorLabel> {
    let universe = simulation.universe();
    let state = simulation.state();

    universe
        .sectors
        .iter()
        .filter_map(|sector| {
            let known_positions = sector
                .systems
                .iter()
                .filter(|system_id| state.is_system_known(**system_id))
                .filter_map(|system_id| universe.system(*system_id).map(|system| system.position))
                .collect::<Vec<_>>();
            if known_positions.is_empty() {
                return None;
            }

            let divisor = known_positions.len() as f32;
            let position =
                known_positions
                    .iter()
                    .fold(WorldPosition::ZERO, |accumulator, position| {
                        WorldPosition::new(
                            accumulator.x + position.x,
                            accumulator.y + position.y,
                            accumulator.z + position.z,
                        )
                    });

            Some(KnownSectorLabel {
                text: sector.name.clone(),
                position: WorldPosition::new(
                    position.x / divisor,
                    position.y / divisor,
                    position.z / divisor,
                ),
            })
        })
        .collect()
}

fn systems_for_universe_view(
    simulation: &Simulation,
    debug_full_graph: bool,
) -> Vec<UniverseSystemEntry> {
    if debug_full_graph {
        return simulation
            .universe()
            .systems
            .iter()
            .map(|system| {
                let tier = simulation
                    .state()
                    .system_visibility(system.id)
                    .map(UniverseSystemTier::from_visibility)
                    .unwrap_or(UniverseSystemTier::Observed);
                UniverseSystemEntry {
                    id: system.id,
                    tier,
                }
            })
            .collect();
    }

    let mut systems = simulation
        .state()
        .visible_systems()
        .into_iter()
        .map(|(id, visibility)| UniverseSystemEntry {
            id,
            tier: UniverseSystemTier::from_visibility(visibility),
        })
        .collect::<Vec<_>>();
    let observation_ids = initial_observation_systems(
        simulation.universe(),
        MVP_HOME_SYSTEM_ID,
        INITIAL_OBSERVATION_SYSTEM_LIMIT,
    );
    for id in observation_ids {
        if simulation.state().system_visibility(id).is_none() {
            systems.push(UniverseSystemEntry {
                id,
                tier: UniverseSystemTier::Observed,
            });
        }
    }
    systems.sort_by_key(|entry| entry.id);
    systems
}

fn initial_observation_systems(
    universe: &galactic_domain::UniverseDefinition,
    origin_id: SystemId,
    limit: usize,
) -> Vec<SystemId> {
    let Some(origin) = universe.system(origin_id) else {
        return Vec::new();
    };
    let origin = to_vec3(origin.position);
    let mut candidates = universe
        .systems
        .iter()
        .map(|system| (system.id, to_vec3(system.position).distance_squared(origin)))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left_id.cmp(right_id))
    });
    candidates
        .into_iter()
        .take(limit.min(universe.systems.len()))
        .map(|(id, _)| id)
        .collect()
}

fn spawn_star_halo(
    commands: &mut Commands,
    assets: &VisualAssets,
    system_id: SystemId,
    class: StarClass,
    position: Vec3,
    scale: f32,
) {
    let material = assets
        .star_halo_materials
        .get(&class)
        .expect("star halo material exists")
        .clone();
    let base_scale = Vec3::splat(scale);
    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(base_scale),
        SystemVisual {
            id: system_id,
            tier: UniverseSystemTier::Known,
            base_scale,
        },
        StrategicViewEntity,
    ));
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
    let star_halo_material = assets
        .star_halo_materials
        .get(&system.star.class)
        .expect("star halo material exists")
        .clone();
    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(star_halo_material),
        Transform::from_scale(Vec3::splat(4.4)),
        StrategicViewEntity,
    ));
    commands.spawn((
        PointLight {
            color: star_color(system.star.class),
            intensity: 11_000.0,
            range: 90.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.5, 0.0),
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
        let orbit = planet_orbit(index, radius, 0.0);
        let position = orbit.translation_at(0.0);
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
        let scale = if level.reveals_identity() {
            planet_visual_scale(planet.kind)
        } else {
            0.72
        };
        let label = match level {
            KnowledgeLevel::Unknown => {
                continue;
            }
            KnowledgeLevel::Detected => provisional_planet_label(&system.name, index),
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
            Mesh3d(if level.reveals_identity() {
                assets.planet_mesh.clone()
            } else {
                assets.system_mesh.clone()
            }),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            orbit,
            AxialSpin {
                radians_per_second: planet_spin_speed(planet.id, planet.kind),
            },
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
        let halo = spawn_pointer_halo(
            commands,
            assets,
            PickTarget::Planet {
                system_id,
                planet_id: planet.id,
            },
            position,
            scale * 1.65,
        );
        commands.entity(halo).insert(orbit);

        if level.reveals_identity() {
            if let Some(atmosphere) = assets.atmosphere_materials.get(&planet.kind) {
                commands.spawn((
                    Mesh3d(assets.planet_mesh.clone()),
                    MeshMaterial3d(atmosphere.clone()),
                    Transform::from_translation(position).with_scale(Vec3::splat(scale * 1.07)),
                    orbit,
                    StrategicViewEntity,
                ));
            }
            if planet.kind == PlanetKind::GasGiant {
                let ring_orbit = OrbitingVisual {
                    vertical_offset: -0.02,
                    ..orbit
                };
                commands.spawn((
                    Mesh3d(assets.ring_mesh.clone()),
                    MeshMaterial3d(assets.ring_material.clone()),
                    Transform::from_translation(ring_orbit.translation_at(0.0)).with_rotation(
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
                            * Quat::from_rotation_z(0.22),
                    ),
                    ring_orbit,
                    StrategicViewEntity,
                ));
            }
        }

        commands.spawn((
            Text2d::new(label),
            ui_text_font(11.0),
            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),
            Transform::from_translation(position + Vec3::new(0.0, 1.35, 0.0))
                .with_scale(Vec3::splat(0.25)),
            OrbitingVisual {
                vertical_offset: 1.35,
                ..orbit
            },
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
) -> Entity {
    commands
        .spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(assets.hover_material.clone()),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            Visibility::Hidden,
            PointerHalo { target },
            StrategicViewEntity,
        ))
        .id()
}

fn planet_orbit(index: usize, radius: f32, vertical_offset: f32) -> OrbitingVisual {
    OrbitingVisual {
        radius,
        phase: index as f32 * 1.37,
        angular_speed: 0.014 / (index as f32 + 1.0).sqrt(),
        vertical_offset,
    }
}

fn planet_visual_scale(kind: PlanetKind) -> f32 {
    match kind {
        PlanetKind::Rocky => 0.68,
        PlanetKind::Ocean => 0.82,
        PlanetKind::Desert => 0.78,
        PlanetKind::Ice => 0.74,
        PlanetKind::GasGiant => 1.32,
        PlanetKind::Volcanic => 0.70,
    }
}

fn planet_spin_speed(planet_id: PlanetId, kind: PlanetKind) -> f32 {
    let base = match kind {
        PlanetKind::GasGiant => 0.025,
        PlanetKind::Ocean | PlanetKind::Ice => 0.040,
        PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => 0.034,
    };
    base + (planet_id.raw() % 5) as f32 * 0.003
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
            spawn_action_button(parent, UiAction::LaunchProbe, "Lancer reconnaissance", "K");
            spawn_action_button(parent, UiAction::AnalyzePlanet, "Analyser planète", "L");
            spawn_action_button(parent, UiAction::LaunchAttack, "Lancer attaque", "M");
            spawn_action_button(parent, UiAction::LaunchHarvest, "Lancer récolte", "H");
            spawn_action_button(
                parent,
                UiAction::LaunchColonization,
                "Lancer colonisation",
                "N",
            );
            spawn_action_button(
                parent,
                UiAction::ToggleProjection,
                "Projection 3D / 2,5D",
                "P",
            );
            spawn_action_button(parent, UiAction::ToggleDebugGraph, "Debug graphe", "G");
            spawn_action_button(parent, UiAction::RebuildView, "Reconstruire", "R");
            spawn_colony_management_toggle(parent);
            research_ui::spawn_research_toggle(parent);
            craft_ui::spawn_craft_toggle(parent);
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
            "Clic sélectionner | Double-clic ouvrir/recentrer | K sonder | L analyser | M attaquer | H récolter | N coloniser | P projection | droit orbite | milieu déplacer | molette zoom",
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

    spawn_colony_management_screen(&mut commands);
}

fn spawn_colony_management_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.18, 0.19, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.28, 0.78, 0.72, 0.55),
            ),
            ManagementButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Gestion colonie"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.94, 0.90)),
                ManagementTextRole::ToggleLabel,
            ));
        });
}

fn spawn_colony_management_screen(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(72.0),
                bottom: Val::Px(14.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(9.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.008, 0.014, 0.020, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.30, 0.72, 0.74, 0.68),
            ),
            Visibility::Hidden,
            GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            ColonyManagementRoot,
        ))
        .with_children(|root| {
            spawn_management_header(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.70, 0.84, 0.88)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ManagementTextRole::ColonyList,
            ));
            spawn_management_resource_row(root);
            spawn_management_main_row(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.90, 0.72, 0.42)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ManagementTextRole::Feedback,
            ));
        });
}

fn spawn_management_header(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(42.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    },))
        .with_children(|header| {
            header.spawn((
                Text::new("GESTION PLANÉTAIRE"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.82, 0.96, 0.96)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ManagementTextRole::Title,
            ));

            spawn_management_small_button(
                header,
                "◀",
                ManagementButtonAction::PreviousColony,
                36.0,
            );
            header.spawn((
                Text::new("Colonie"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.76, 0.86, 0.90)),
                Node {
                    width: Val::Px(250.0),
                    ..default()
                },
                ManagementTextRole::Colony,
            ));
            spawn_management_small_button(header, "▶", ManagementButtonAction::NextColony, 36.0);
            spawn_management_small_button(
                header,
                "Fermer  [C / Échap]",
                ManagementButtonAction::Close,
                154.0,
            );
        });
}

fn spawn_management_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ManagementButtonAction,
    width: f32,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(width),
                min_height: Val::Px(32.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.13, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.44, 0.62, 0.68, 0.50),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.90, 0.94)),
            ));
        });
}

fn spawn_management_resource_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(94.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(8.0),
        ..default()
    },))
        .with_children(|row| {
            for kind in ResourceHudKind::ALL {
                spawn_management_resource_card(row, kind);
            }
        });
}

fn spawn_management_resource_card(parent: &mut ChildSpawnerCommands, kind: ResourceHudKind) {
    parent
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(resource_card_background(kind)),
            Outline::new(Val::Px(1.0), Val::ZERO, resource_outline_color(kind)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(kind.title()),
                ui_text_font(11.0),
                TextColor(resource_kind_color(kind)),
                ManagementResourceCardText { kind },
            ));
            card.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(7.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.11, 0.14, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(resource_kind_color(kind)),
                    ManagementResourceGaugeFill { kind },
                ));
            });
        });
}

fn spawn_management_main_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(390.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(9.0),
        ..default()
    },))
        .with_children(|row| {
            spawn_management_building_list(row);
            spawn_management_building_detail(row);
            spawn_management_queue(row);
        });
}

fn spawn_management_building_list(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(292.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|list| {
        list.spawn((
            Text::new("BÂTIMENTS"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.72, 0.88, 0.92)),
        ));
        for definition in galactic_sim::default_building_catalog().definitions() {
            spawn_management_building_button(list, definition.kind);
        }
    });
}

fn spawn_management_building_button(
    parent: &mut ChildSpawnerCommands,
    kind: galactic_sim::BuildingKind,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(37.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.055, 0.070, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.30, 0.44, 0.50, 0.42),
            ),
            ManagementButtonAction::SelectBuilding(kind),
            ManagementBuildingButton { kind },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.88, 0.90)),
                ManagementBuildingButtonText { kind },
            ));
        });
}

fn spawn_management_building_detail(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|detail| {
        detail.spawn((
            Text::new("Sélectionne un bâtiment."),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.84, 0.90, 0.94)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ManagementTextRole::BuildingDetail,
        ));
        detail
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(42.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.28, 0.24, 0.98)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.30, 0.88, 0.72, 0.70),
                ),
                ManagementButtonAction::UpgradeSelected,
                ManagementUpgradeButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("AMÉLIORER"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.86, 0.98, 0.92)),
                    ManagementTextRole::UpgradeLabel,
                ));
            });
    });
}

fn spawn_management_queue(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(306.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|queue| {
        queue.spawn((
            Text::new("FILE DE CONSTRUCTION"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.72, 0.88, 0.92)),
        ));
        queue
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.11, 0.14, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.36, 0.84, 0.72)),
                    ManagementQueueProgressFill,
                ));
            });
        queue.spawn((
            Text::new("File vide."),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.78, 0.84, 0.88)),
            ManagementTextRole::Queue,
        ));
        queue.spawn((
            Text::new("LOGISTIQUE INTERCOLONIALE"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.72, 0.88, 0.92)),
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
        ));
        queue
            .spawn((Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },))
            .with_children(|row| {
                spawn_management_small_button(
                    row,
                    "◀",
                    ManagementButtonAction::PreviousTransportDestination,
                    34.0,
                );
                row.spawn((
                    Text::new("Destination"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(0.78, 0.88, 0.92)),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                    ManagementTextRole::TransportDestination,
                ));
                spawn_management_small_button(
                    row,
                    "▶",
                    ManagementButtonAction::NextTransportDestination,
                    34.0,
                );
            });
        queue.spawn((
            Text::new("Cargaison"),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.76, 0.84, 0.88)),
            ManagementTextRole::TransportCargo,
        ));
        queue
            .spawn((Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(5.0),
                ..default()
            },))
            .with_children(|row| {
                for preset in TransportCargoPreset::ALL {
                    spawn_management_transport_preset_button(row, preset);
                }
            });
        queue
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(40.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.28, 0.24, 0.98)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.30, 0.88, 0.72, 0.70),
                ),
                ManagementButtonAction::LaunchTransport,
                ManagementTransportLaunchButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("LANCER LE TRANSPORT"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(0.86, 0.98, 0.92)),
                    ManagementTextRole::TransportLaunchLabel,
                ));
            });
    });
}

fn spawn_management_transport_preset_button(
    parent: &mut ChildSpawnerCommands,
    preset: TransportCargoPreset,
) {
    parent
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.08, 0.10, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            ManagementButtonAction::SelectTransportCargo(preset),
            ManagementTransportPresetButton { preset },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(preset.short_label()),
                ui_text_font(10.0),
                TextColor(Color::srgb(0.80, 0.88, 0.90)),
            ));
        });
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
    mut management: ResMut<ColonyManagementState>,
    mut overlays: ParamSet<(
        Res<research_ui::ResearchUiState>,
        Res<craft_ui::CraftUiState>,
    )>,
) {
    let research_open = overlays.p0().open;
    let craft_open = overlays.p1().open;
    if research_open || craft_open {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyC) {
        toggle_colony_management(&mut management, &mut simulation);
        pointer_state.ambiguity = None;
        return;
    }

    if management.open {
        if keyboard.just_pressed(KeyCode::Escape) {
            management.open = false;
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            cycle_management_colony(&mut management, &mut simulation, true);
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            cycle_management_colony(&mut management, &mut simulation, false);
        }
        return;
    }

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
        let next = if Some(halo.target) == pointer_state.hovered {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
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
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(target) = pointer_state.hovered else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(screen_position) = pointer_state.hovered_screen_position else {
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let next_text = pointer_tooltip_text(simulation.simulation(), target);
    if text.0 != next_text {
        text.0 = next_text;
    }
    let next_left =
        Val::Px((screen_position.x + 18.0).clamp(8.0, (window.width() - 270.0).max(8.0)));
    if node.left != next_left {
        node.left = next_left;
    }
    let next_top =
        Val::Px((screen_position.y + 18.0).clamp(8.0, (window.height() - 110.0).max(8.0)));
    if node.top != next_top {
        node.top = next_top;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }
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
        if *visibility != Visibility::Hidden {
            *visibility = Visibility::Hidden;
        }
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

    let next_text = lines.join("\n");
    if text.0 != next_text {
        text.0 = next_text;
    }
    if *visibility != Visibility::Visible {
        *visibility = Visibility::Visible;
    }
}

fn select_pick_target(simulation: &mut SimulationResource, target: PickTarget) {
    let command = match target {
        PickTarget::System(system_id) => GameAction::SelectSystem(system_id),
        PickTarget::Planet {
            system_id,
            planet_id,
        } => GameAction::SelectPlanet {
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
            let title = planet_display_label(simulation, planet_id, level)
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
            .and_then(|_| {
                planet_display_label(
                    simulation,
                    planet_id,
                    state.planet_knowledge_level(planet_id),
                )
            })
            .map(|label| format!("Planète {label}"))
            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),
    }
}

fn planet_display_label(
    simulation: &Simulation,
    planet_id: PlanetId,
    level: KnowledgeLevel,
) -> Option<String> {
    let (system_id, planet) = simulation
        .universe_repository()
        .planet_location(planet_id)?;
    if level.reveals_identity() {
        return Some(planet.name.clone());
    }
    let system = simulation.universe().system(system_id)?;
    let index = system
        .planets
        .iter()
        .position(|candidate| candidate.id == planet_id)?;
    Some(provisional_planet_label(&system.name, index))
}

fn provisional_planet_label(system_name: &str, orbit_index: usize) -> String {
    const ROMAN: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    let suffix = ROMAN
        .get(orbit_index)
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| (orbit_index + 1).to_string());
    format!("{system_name} {suffix}")
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

fn handle_colony_management_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut management: ResMut<ColonyManagementState>,
    interactions: ManagementButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            ManagementButtonAction::Toggle => {
                toggle_colony_management(&mut management, &mut simulation);
            }
            ManagementButtonAction::Close => {
                management.open = false;
            }
            ManagementButtonAction::PreviousColony => {
                cycle_management_colony(&mut management, &mut simulation, true);
            }
            ManagementButtonAction::NextColony => {
                cycle_management_colony(&mut management, &mut simulation, false);
            }
            ManagementButtonAction::PreviousTransportDestination => {
                cycle_transport_destination(&mut management, simulation.simulation(), true);
            }
            ManagementButtonAction::NextTransportDestination => {
                cycle_transport_destination(&mut management, simulation.simulation(), false);
            }
            ManagementButtonAction::SelectTransportCargo(preset) => {
                management.transport_cargo = preset;
                management.feedback.clear();
            }
            ManagementButtonAction::LaunchTransport => {
                launch_selected_transport(&mut management, &mut simulation);
            }
            ManagementButtonAction::SelectBuilding(kind) => {
                management.selected_building = kind;
                management.feedback.clear();
            }
            ManagementButtonAction::UpgradeSelected => {
                queue_selected_management_upgrade(&mut management, &mut simulation);
            }
        }
    }
}

fn capture_colony_management_feedback(
    simulation: Res<SimulationResource>,
    mut management: ResMut<ColonyManagementState>,
) {
    let active_colony_id = simulation.simulation().state().active_colony_id;
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::ConstructionQueued(queued)
                if Some(queued.colony_id) == active_colony_id =>
            {
                let name = &galactic_sim::default_building_catalog()
                    .definition(queued.order.kind)
                    .name;
                management.feedback = format!(
                    "{} niveau {} ajouté à la file.",
                    name, queued.order.target_level,
                );
            }
            GameEventKind::ConstructionCompleted(completed)
                if Some(completed.colony_id) == active_colony_id =>
            {
                let name = &galactic_sim::default_building_catalog()
                    .definition(completed.kind)
                    .name;
                management.feedback = format!("{} niveau {} terminé.", name, completed.new_level,);
            }
            GameEventKind::ConstructionRejected(rejected)
                if Some(rejected.colony_id) == active_colony_id =>
            {
                management.feedback = format!(
                    "Amélioration refusée : {}",
                    construction_error_text(rejected.error),
                );
            }
            GameEventKind::MissionLaunched(launched)
                if management.open && launched.kind == MissionKind::Transport =>
            {
                management.feedback = simulation
                    .simulation()
                    .state()
                    .mission(launched.mission_id)
                    .and_then(|mission| mission.transport)
                    .map(|transport| {
                        format!(
                            "Transport lancé vers C{} : {}.",
                            transport.destination_colony_id.raw(),
                            transport_cargo_label(transport.cargo),
                        )
                    })
                    .unwrap_or_else(|| "Transport lancé.".to_string());
            }
            GameEventKind::MissionLaunchRejected(rejected) if management.open => {
                management.feedback =
                    format!("Transport refusé : {}", mission_error_text(rejected.error));
            }
            GameEventKind::MissionResolved(resolution)
                if matches!(resolution.result, MissionResult::Transport(_)) =>
            {
                let MissionResult::Transport(result) = resolution.result else {
                    unreachable!("the guard selected a transport result");
                };
                management.feedback = transport_result_label(result);
            }
            _ => {}
        }
    }
}

fn update_colony_management_visibility(
    simulation: Res<SimulationResource>,
    mut management: ResMut<ColonyManagementState>,
    mut roots: Query<&mut Visibility, With<ColonyManagementRoot>>,
    mut texts: Query<(&ManagementTextRole, &mut Text)>,
) {
    if management.open {
        sync_transport_destination(&mut management, simulation.simulation());
    }
    for mut visibility in &mut roots {
        let next = if management.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }

    let colony = active_management_colony(simulation.simulation());
    let colonies = player_colony_ids(simulation.simulation());
    let current_index = colony.and_then(|active| {
        colonies
            .iter()
            .position(|candidate| *candidate == active.id)
    });

    for (role, mut text) in &mut texts {
        let next = match role {
            ManagementTextRole::ToggleLabel => Some(if management.open {
                "Fermer gestion colonie".to_string()
            } else {
                "Gestion colonie  [C]".to_string()
            }),
            ManagementTextRole::Title => Some(
                colony
                    .map(|active| format!("GESTION PLANÉTAIRE — {}", active.name,))
                    .unwrap_or_else(|| "GESTION PLANÉTAIRE".to_string()),
            ),
            ManagementTextRole::Colony => Some(
                colony
                    .map(|active| {
                        let index = current_index.unwrap_or(0) + 1;
                        let planet = simulation
                            .simulation()
                            .universe_repository()
                            .planet(active.planet_id)
                            .map(|value| value.name.as_str())
                            .unwrap_or("Planète");
                        format!("{} / {}  •  {}", index, colonies.len().max(1), planet,)
                    })
                    .unwrap_or_else(|| "Aucune colonie".to_string()),
            ),
            ManagementTextRole::ColonyList => Some(colony_list_label(simulation.simulation())),
            ManagementTextRole::Feedback => Some(management.feedback.clone()),
            ManagementTextRole::TransportDestination
            | ManagementTextRole::TransportCargo
            | ManagementTextRole::TransportLaunchLabel
            | ManagementTextRole::BuildingDetail
            | ManagementTextRole::UpgradeLabel
            | ManagementTextRole::Queue => None,
        };
        if let Some(next) = next
            && text.0 != next
        {
            text.0 = next;
        }
    }
}

fn update_colony_management_resources(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    mut texts: Query<(&ManagementResourceCardText, &mut Text, &mut TextColor)>,
    mut gauges: Query<(
        &ManagementResourceGaugeFill,
        &mut Node,
        &mut BackgroundColor,
    )>,
) {
    if !management.open {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };
    let production = galactic_sim::colony_production_snapshot(colony);

    for (card, mut text, mut color) in &mut texts {
        let view = resource_hud_view(card.kind, colony, production);
        text.0 = view.text;
        color.0 = status_text_color(card.kind, view.status);
    }
    for (gauge, mut node, mut color) in &mut gauges {
        let view = resource_hud_view(gauge.kind, colony, production);
        node.width = Val::Percent((view.fill_ratio * 100.0).clamp(0.0, 100.0));
        color.0 = status_gauge_color(gauge.kind, view.status);
    }
}

fn update_colony_management_buildings(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    mut buttons: Query<(
        &ManagementBuildingButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
    mut labels: Query<(&ManagementBuildingButtonText, &mut Text, &mut TextColor)>,
) {
    if !management.open {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };
    let catalog = galactic_sim::default_building_catalog();
    let projected = galactic_sim::projected_building_levels(colony);

    for (button, interaction, mut background, mut outline) in &mut buttons {
        let selected = button.kind == management.selected_building;
        background.0 = management_building_button_color(selected, interaction);
        outline.color = management_building_button_outline(selected);
    }

    for (label, mut text, mut color) in &mut labels {
        let definition = catalog.definition(label.kind);
        let active_level = colony.buildings.level(label.kind);
        let projected_level = projected.level(label.kind);
        let queue_suffix = if projected_level > active_level {
            format!("  → {} en file", projected_level)
        } else {
            String::new()
        };
        text.0 = format!(
            "{}
Niveau {}{}",
            definition.name, active_level, queue_suffix,
        );
        color.0 = if label.kind == management.selected_building {
            Color::srgb(0.86, 0.98, 0.94)
        } else {
            Color::srgb(0.78, 0.84, 0.88)
        };
    }
}

fn update_colony_management_detail(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    mut texts: Query<(&ManagementTextRole, &mut Text, &mut TextColor)>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<ManagementUpgradeButton>,
    >,
) {
    if !management.open {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };

    let kind = management.selected_building;
    let state = simulation.simulation().state();
    let quote = galactic_sim::building_upgrade_quote(state, state.player_faction, colony.id, kind);
    let available = quote.is_ok();
    let detail = building_management_detail_text(colony, kind, quote);
    let upgrade_label = match quote {
        Ok(value) => format!("AMÉLIORER VERS LE NIVEAU {}", value.target_level,),
        Err(error) => construction_error_text(error),
    };

    for (role, mut text, mut color) in &mut texts {
        match role {
            ManagementTextRole::BuildingDetail => {
                text.0 = detail.clone();
                color.0 = Color::srgb(0.84, 0.90, 0.94);
            }
            ManagementTextRole::UpgradeLabel => {
                text.0 = upgrade_label.clone();
                color.0 = if available {
                    Color::srgb(0.86, 0.98, 0.92)
                } else {
                    Color::srgb(0.64, 0.66, 0.66)
                };
            }
            _ => {}
        }
    }

    for (interaction, mut background, mut outline) in &mut buttons {
        background.0 = action_button_color(available, false, interaction);
        outline.color = action_button_outline(available, false, interaction);
    }
}

fn update_colony_management_queue(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    mut texts: Query<(&ManagementTextRole, &mut Text)>,
    mut progress: Query<&mut Node, With<ManagementQueueProgressFill>>,
) {
    if !management.open {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };

    let label = construction_queue_detail_label(colony);
    for (role, mut text) in &mut texts {
        if *role == ManagementTextRole::Queue {
            text.0 = label.clone();
        }
    }

    let ratio = construction_progress_ratio(colony);
    for mut node in &mut progress {
        node.width = Val::Percent((ratio * 100.0).clamp(0.0, 100.0));
    }
}

fn update_colony_management_transport(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    mut texts: Query<(&ManagementTextRole, &mut Text, &mut TextColor)>,
    mut launch_buttons: ManagementTransportLaunchStyleQuery,
    mut preset_buttons: ManagementTransportPresetStyleQuery,
) {
    if !management.open {
        return;
    }
    let state = simulation.simulation().state();
    let destination = management
        .transport_destination_id
        .and_then(|colony_id| state.colony(colony_id));
    let cargo = management.transport_cargo.cargo();
    let available = state.active_player_colony().is_some_and(|origin| {
        destination.is_some() && origin.resources.available().can_cover(cargo)
    });
    let destination_label = destination
        .map(|colony| format!("C{} {}", colony.id.raw(), colony.name))
        .unwrap_or_else(|| "Deux colonies requises".to_string());
    let cargo_label = transport_cargo_label(cargo);
    let launch_label = if destination.is_none() {
        "DEUX COLONIES REQUISES"
    } else {
        "LANCER LE TRANSPORT"
    };

    for (role, mut text, mut color) in &mut texts {
        match role {
            ManagementTextRole::TransportDestination => {
                text.0 = destination_label.clone();
                color.0 = if destination.is_some() {
                    Color::srgb(0.78, 0.88, 0.92)
                } else {
                    Color::srgb(0.64, 0.66, 0.66)
                };
            }
            ManagementTextRole::TransportCargo => {
                text.0 = format!("Cargaison : {cargo_label}");
                color.0 = Color::srgb(0.76, 0.84, 0.88);
            }
            ManagementTextRole::TransportLaunchLabel => {
                text.0 = launch_label.to_string();
                color.0 = if available {
                    Color::srgb(0.86, 0.98, 0.92)
                } else {
                    Color::srgb(0.64, 0.66, 0.66)
                };
            }
            _ => {}
        }
    }
    for (interaction, mut background, mut outline) in &mut launch_buttons {
        background.0 = action_button_color(available, false, interaction);
        outline.color = action_button_outline(available, false, interaction);
    }
    for (button, interaction, mut background, mut outline) in &mut preset_buttons {
        let selected = button.preset == management.transport_cargo;
        background.0 = action_button_color(true, selected, interaction);
        outline.color = action_button_outline(true, selected, interaction);
    }
}

fn toggle_colony_management(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
) {
    management.open = !management.open;
    management.feedback.clear();
    if management.open
        && let Some(colony_id) = selected_player_colony_id(simulation.simulation())
    {
        apply_simulation_command(simulation, GameAction::SelectColony { colony_id });
    }
}

fn selected_player_colony_id(simulation: &Simulation) -> Option<galactic_domain::ColonyId> {
    let state = simulation.state();
    let colony = match state.selected {
        SelectionTarget::Planet { planet_id, .. } => state.colony_on_planet(planet_id),
        SelectionTarget::System(system_id) => state
            .player_colonies()
            .find(|colony| colony.system_id == system_id),
        SelectionTarget::None => state.active_player_colony(),
    }?;
    state
        .can_manage(state.player_faction, colony.owner)
        .then_some(colony.id)
}

fn player_colony_ids(simulation: &Simulation) -> Vec<galactic_domain::ColonyId> {
    simulation.state().player_colony_ids()
}

fn active_management_colony(simulation: &Simulation) -> Option<&galactic_sim::ColonyState> {
    simulation.state().active_player_colony()
}

fn colony_list_label(simulation: &Simulation) -> String {
    let state = simulation.state();
    let active = state.active_colony_id;
    let entries = state
        .player_colony_ids()
        .into_iter()
        .filter_map(|colony_id| {
            state.colony(colony_id).map(|colony| {
                let marker = if Some(colony_id) == active {
                    "●"
                } else {
                    "○"
                };
                format!("{marker} C{} {}", colony_id.raw(), colony.name)
            })
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "COLONIES : aucune".to_string()
    } else {
        format!("COLONIES : {}", entries.join("   "))
    }
}

fn sync_transport_destination(management: &mut ColonyManagementState, simulation: &Simulation) {
    let state = simulation.state();
    let origin = state.active_colony_id;
    let destinations = state
        .player_colony_ids()
        .into_iter()
        .filter(|colony_id| Some(*colony_id) != origin)
        .collect::<Vec<_>>();
    if management
        .transport_destination_id
        .is_some_and(|current| destinations.contains(&current))
    {
        return;
    }
    management.transport_destination_id = destinations.first().copied();
}

fn cycle_transport_destination(
    management: &mut ColonyManagementState,
    simulation: &Simulation,
    reverse: bool,
) {
    sync_transport_destination(management, simulation);
    let origin = simulation.state().active_colony_id;
    let destinations = simulation
        .state()
        .player_colony_ids()
        .into_iter()
        .filter(|colony_id| Some(*colony_id) != origin)
        .collect::<Vec<_>>();
    if destinations.is_empty() {
        management.transport_destination_id = None;
        return;
    }
    let current = management
        .transport_destination_id
        .and_then(|active| destinations.iter().position(|id| *id == active))
        .unwrap_or(0);
    let next = if reverse {
        current.checked_sub(1).unwrap_or(destinations.len() - 1)
    } else {
        (current + 1) % destinations.len()
    };
    management.transport_destination_id = Some(destinations[next]);
    management.feedback.clear();
}

fn launch_selected_transport(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
) {
    sync_transport_destination(management, simulation.simulation());
    let Some(origin_colony_id) = simulation.simulation().state().active_colony_id else {
        management.feedback = "Aucune colonie active.".to_string();
        return;
    };
    let Some(destination_colony_id) = management.transport_destination_id else {
        management.feedback = "Une deuxième colonie est nécessaire.".to_string();
        return;
    };
    apply_simulation_command(
        simulation,
        GameAction::LaunchTransport {
            origin_colony_id,
            destination_colony_id,
            cargo: management.transport_cargo.cargo(),
        },
    );
}

fn transport_cargo_label(cargo: ResourceStock) -> String {
    format!(
        "{} métal, {} cristal, {} carburant",
        cargo.metal, cargo.crystal, cargo.fuel,
    )
}

fn transport_result_label(result: galactic_sim::TransportMissionResult) -> String {
    match result.status {
        galactic_sim::TransportDeliveryStatus::Delivered => format!(
            "Livraison terminée vers C{} : {}.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.delivered),
        ),
        galactic_sim::TransportDeliveryStatus::PartiallyDelivered => format!(
            "Livraison partielle vers C{} : {} livrés, {} revenus, {} encore en soute.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.delivered),
            transport_cargo_label(result.returned),
            transport_cargo_label(result.retained),
        ),
        galactic_sim::TransportDeliveryStatus::DestinationInvalid => format!(
            "Destination C{} invalide : {} revenus, {} encore en soute.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.returned),
            transport_cargo_label(result.retained),
        ),
        galactic_sim::TransportDeliveryStatus::Pending => "Transport encore en cours.".to_string(),
    }
}

fn cycle_management_colony(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
    reverse: bool,
) {
    let colonies = player_colony_ids(simulation.simulation());
    if colonies.is_empty() {
        return;
    }

    let current = simulation
        .simulation()
        .state()
        .active_colony_id
        .and_then(|active| colonies.iter().position(|id| *id == active))
        .unwrap_or(0);
    let next = if reverse {
        current.checked_sub(1).unwrap_or(colonies.len() - 1)
    } else {
        (current + 1) % colonies.len()
    };
    management.feedback.clear();
    apply_simulation_command(
        simulation,
        GameAction::SelectColony {
            colony_id: colonies[next],
        },
    );
}

fn queue_selected_management_upgrade(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        management.feedback = "Aucune colonie active.".to_string();
        return;
    };
    let kind = management.selected_building;
    let state = simulation.simulation().state();
    match galactic_sim::building_upgrade_quote(state, state.player_faction, colony_id, kind) {
        Ok(_) => {
            apply_simulation_command(
                simulation,
                GameAction::QueueBuildingUpgrade { colony_id, kind },
            );
        }
        Err(error) => {
            management.feedback = construction_error_text(error);
        }
    }
}

struct ResourceHudView {
    text: String,
    fill_ratio: f32,
    status: ResourceHudStatus,
}

fn resource_hud_view(
    kind: ResourceHudKind,
    colony: &galactic_sim::ColonyState,
    production: galactic_sim::ColonyProductionSnapshot,
) -> ResourceHudView {
    if kind == ResourceHudKind::Energy {
        return energy_hud_view(production);
    }

    let stock = resource_value(kind, colony.resources.stock());
    let available = resource_value(kind, colony.resources.available());
    let reserved = resource_value(kind, colony.resources.reserved_total());
    let capacity = resource_value(kind, production.capacity);
    let rate = resource_rate_per_second(kind, production);
    let saturation = resource_saturation(kind, production);
    let fill_ratio = resource_fill_ratio(stock, capacity);
    let status = resource_hud_status(stock, capacity);
    let warning = if status == ResourceHudStatus::Full {
        "
PLEIN — PRODUCTION BLOQUÉE"
    } else {
        ""
    };

    ResourceHudView {
        text: format!(
            "{}  {} / {}
Disponible {}  •  Réservé {}
Production +{:.2}/s  •  plein {}{}",
            kind.title(),
            stock,
            capacity,
            available,
            reserved,
            rate,
            format_saturation_time(saturation),
            warning,
        ),
        fill_ratio,
        status,
    }
}

fn energy_hud_view(production: galactic_sim::ColonyProductionSnapshot) -> ResourceHudView {
    let produced = production.effective_energy_production;
    let consumed = production.energy_consumption;
    let available = produced.saturating_sub(consumed);
    let deficit = consumed > produced;
    let status = if deficit {
        ResourceHudStatus::Deficit
    } else {
        ResourceHudStatus::Normal
    };
    let warning = if deficit {
        "
DÉFICIT — PRODUCTION RALENTIE"
    } else {
        ""
    };

    ResourceHudView {
        text: format!(
            "ÉNERGIE  {} / {}
Disponible {}  •  Bilan {:+}
Rendement {}%{}",
            consumed,
            produced,
            available,
            i128::from(produced) - i128::from(consumed),
            u32::from(production.energy_efficiency_per_mille,) / 10,
            warning,
        ),
        fill_ratio: energy_fill_ratio(consumed, produced),
        status,
    }
}

fn resource_value(kind: ResourceHudKind, stock: ResourceStock) -> u64 {
    match kind {
        ResourceHudKind::Metal => stock.metal,
        ResourceHudKind::Crystal => stock.crystal,
        ResourceHudKind::Fuel => stock.fuel,
        ResourceHudKind::Energy => 0,
    }
}

fn resource_rate_per_second(
    kind: ResourceHudKind,
    production: galactic_sim::ColonyProductionSnapshot,
) -> f64 {
    match kind {
        ResourceHudKind::Metal => production.effective_rate.metal_per_second(),
        ResourceHudKind::Crystal => production.effective_rate.crystal_per_second(),
        ResourceHudKind::Fuel => production.effective_rate.fuel_per_second(),
        ResourceHudKind::Energy => 0.0,
    }
}

fn resource_saturation(
    kind: ResourceHudKind,
    production: galactic_sim::ColonyProductionSnapshot,
) -> galactic_sim::SaturationTime {
    match kind {
        ResourceHudKind::Metal => production.saturation.metal,
        ResourceHudKind::Crystal => production.saturation.crystal,
        ResourceHudKind::Fuel => production.saturation.fuel,
        ResourceHudKind::Energy => galactic_sim::SaturationTime::Never,
    }
}

fn resource_fill_ratio(stock: u64, capacity: u64) -> f32 {
    if capacity == 0 {
        return if stock == 0 { 0.0 } else { 1.0 };
    }
    (stock as f64 / capacity as f64).clamp(0.0, 1.0) as f32
}

fn energy_fill_ratio(consumption: u64, production: u64) -> f32 {
    if production == 0 {
        return if consumption == 0 { 0.0 } else { 1.0 };
    }
    (consumption as f64 / production as f64).clamp(0.0, 1.0) as f32
}

fn resource_hud_status(stock: u64, capacity: u64) -> ResourceHudStatus {
    if capacity > 0 && stock >= capacity {
        ResourceHudStatus::Full
    } else if capacity > 0 && stock.saturating_mul(100) >= capacity.saturating_mul(90) {
        ResourceHudStatus::NearlyFull
    } else {
        ResourceHudStatus::Normal
    }
}

fn resource_kind_color(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgb(0.90, 0.66, 0.42),
        ResourceHudKind::Crystal => Color::srgb(0.56, 0.82, 0.96),
        ResourceHudKind::Fuel => Color::srgb(0.86, 0.82, 0.38),
        ResourceHudKind::Energy => Color::srgb(0.52, 0.92, 0.62),
    }
}

fn resource_outline_color(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgba(0.90, 0.66, 0.42, 0.42),
        ResourceHudKind::Crystal => Color::srgba(0.56, 0.82, 0.96, 0.42),
        ResourceHudKind::Fuel => Color::srgba(0.86, 0.82, 0.38, 0.42),
        ResourceHudKind::Energy => Color::srgba(0.52, 0.92, 0.62, 0.42),
    }
}

fn resource_card_background(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgba(0.16, 0.10, 0.06, 0.94),
        ResourceHudKind::Crystal => Color::srgba(0.06, 0.11, 0.16, 0.94),
        ResourceHudKind::Fuel => Color::srgba(0.15, 0.14, 0.05, 0.94),
        ResourceHudKind::Energy => Color::srgba(0.05, 0.14, 0.08, 0.94),
    }
}

fn status_text_color(kind: ResourceHudKind, status: ResourceHudStatus) -> Color {
    match status {
        ResourceHudStatus::Full | ResourceHudStatus::Deficit => Color::srgb(1.0, 0.48, 0.42),
        ResourceHudStatus::NearlyFull => Color::srgb(1.0, 0.78, 0.36),
        ResourceHudStatus::Normal => resource_kind_color(kind),
    }
}

fn status_gauge_color(kind: ResourceHudKind, status: ResourceHudStatus) -> Color {
    status_text_color(kind, status)
}

fn management_building_button_color(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        return Color::srgba(0.08, 0.30, 0.26, 0.98);
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.09, 0.24, 0.24, 0.98),
        Interaction::Hovered => Color::srgba(0.07, 0.16, 0.18, 0.98),
        Interaction::None => Color::srgba(0.035, 0.055, 0.070, 0.96),
    }
}

fn management_building_button_outline(selected: bool) -> Color {
    if selected {
        Color::srgba(0.32, 0.92, 0.76, 0.80)
    } else {
        Color::srgba(0.30, 0.44, 0.50, 0.42)
    }
}

fn building_management_detail_text(
    colony: &galactic_sim::ColonyState,
    kind: galactic_sim::BuildingKind,
    quote: Result<galactic_sim::BuildingUpgradeQuote, galactic_sim::ConstructionError>,
) -> String {
    let catalog = galactic_sim::default_building_catalog();
    let definition = catalog.definition(kind);
    let actual_level = colony.buildings.level(kind);
    let projected_levels = galactic_sim::projected_building_levels(colony);
    let projected_level = projected_levels.level(kind);
    let current_effect = building_effect_label(colony, kind, colony.buildings);
    let projected_effect = building_effect_label(colony, kind, projected_levels);

    let mut lines = vec![
        definition.name.to_uppercase(),
        definition.description.to_string(),
        format!(
            "Niveau actif : {}  •  après la file : {}  •  maximum : {}",
            actual_level, projected_level, definition.max_level,
        ),
        String::new(),
        "EFFET".to_string(),
        format!("Actuel : {current_effect}"),
    ];
    if projected_level != actual_level {
        lines.push(format!("Après la file : {projected_effect}",));
    }

    if projected_level >= definition.max_level {
        lines.push(String::new());
        lines.push("Niveau maximal atteint ou déjà planifié.".to_string());
        return lines.join(
            "
",
        );
    }

    let target_level = projected_level + 1;
    let mut next_levels = projected_levels;
    next_levels.set_level(kind, target_level);
    let next_effect = building_effect_label(colony, kind, next_levels);
    let cost = definition
        .cost_for_level(target_level)
        .expect("catalog target level is valid");
    let duration = definition
        .duration_for_level(target_level)
        .expect("catalog target level is valid");

    lines.extend([
        format!("Prochain niveau : {next_effect}"),
        String::new(),
        format!("AMÉLIORATION VERS LE NIVEAU {}", target_level,),
        format!("Coût : {}", construction_cost_label(cost),),
        format!(
            "Durée catalogue : {}",
            format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(duration,),),
        ),
    ]);

    match quote {
        Ok(value) => {
            lines.push(format!(
                "Durée effective : {}",
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    value.duration_ticks,
                ),),
            ));
            lines.push(format!(
                "Énergie projetée : {} produite / {} consommée",
                value.projected_energy_production, value.projected_energy_consumption,
            ));
            lines.push(String::new());
            lines.push("Prêt à être ajouté à la file.".to_string());
        }
        Err(error) => {
            lines.push(String::new());
            lines.push(format!("BLOCAGE : {}", construction_error_text(error),));
        }
    }

    lines.join(
        "
",
    )
}

fn building_effect_label(
    colony: &galactic_sim::ColonyState,
    kind: galactic_sim::BuildingKind,
    levels: galactic_sim::BuildingLevels,
) -> String {
    let mut preview = colony.clone();
    preview.buildings = levels;
    preview.energy = galactic_sim::default_building_catalog().energy_grid_for_levels(levels);
    let production = galactic_sim::colony_production_snapshot(&preview);

    match galactic_sim::default_building_catalog()
        .definition(kind)
        .effect
    {
        galactic_sim::BuildingEffect::MetalProduction { .. } => {
            format!(
                "+{:.2} métal/s",
                production.effective_rate.metal_per_second(),
            )
        }
        galactic_sim::BuildingEffect::CrystalProduction { .. } => {
            format!(
                "+{:.2} cristal/s",
                production.effective_rate.crystal_per_second(),
            )
        }
        galactic_sim::BuildingEffect::FuelProduction { .. } => {
            format!(
                "+{:.2} carburant/s",
                production.effective_rate.fuel_per_second(),
            )
        }
        galactic_sim::BuildingEffect::EnergyProduction { .. } => {
            format!(
                "{} énergie effective",
                production.effective_energy_production,
            )
        }
        galactic_sim::BuildingEffect::Storage { .. } => {
            format!(
                "capacité {}/{}/{}",
                production.capacity.metal, production.capacity.crystal, production.capacity.fuel,
            )
        }
        galactic_sim::BuildingEffect::ConstructionSpeed { permille_per_level } => {
            let level = u64::from(levels.level(kind));
            let bonus = permille_per_level.saturating_mul(level) / 10;
            format!("vitesse de construction +{bonus}%")
        }
        galactic_sim::BuildingEffect::ResearchPoints { .. } => {
            future_points_effect_label(kind, levels.level(kind), "recherche")
        }
        galactic_sim::BuildingEffect::ShipyardPoints { .. } => {
            future_points_effect_label(kind, levels.level(kind), "chantier")
        }
    }
}

fn future_points_effect_label(kind: galactic_sim::BuildingKind, level: u8, label: &str) -> String {
    let definition = galactic_sim::default_building_catalog().definition(kind);
    let milli_per_tick = match definition.effect {
        galactic_sim::BuildingEffect::ResearchPoints {
            milli_per_tick_per_level,
        }
        | galactic_sim::BuildingEffect::ShipyardPoints {
            milli_per_tick_per_level,
        } => milli_per_tick_per_level,
        _ => 0,
    };
    let per_second = milli_per_tick as f64
        * f64::from(level)
        * f64::from(galactic_sim::STRATEGIC_TICKS_PER_SECOND)
        / 1_000.0;
    format!("{per_second:.2} points de {label}/s")
}

fn construction_queue_detail_label(colony: &galactic_sim::ColonyState) -> String {
    if colony.construction_queue.is_empty() {
        return format!(
            "File vide

{} emplacement(s) disponible(s).",
            galactic_sim::max_construction_queue(),
        );
    }

    let catalog = galactic_sim::default_building_catalog();
    let mut lines = Vec::new();
    for (index, order) in colony.construction_queue.orders().enumerate() {
        let definition = catalog.definition(order.kind);
        if index == 0 {
            lines.push(format!(
                "EN COURS
{}. {} — niveau {}
{} restant • coût réservé {}",
                index + 1,
                definition.name,
                order.target_level,
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    order.remaining_ticks,
                ),),
                construction_cost_label(order.cost),
            ));
        } else {
            lines.push(format!(
                "
EN ATTENTE
{}. {} — niveau {}
coût réservé {}",
                index + 1,
                definition.name,
                order.target_level,
                construction_cost_label(order.cost),
            ));
        }
    }

    lines.push(format!(
        "

{} / {} emplacement(s) utilisé(s)",
        colony.construction_queue.len(),
        galactic_sim::max_construction_queue(),
    ));
    lines.join(
        "
",
    )
}

fn construction_progress_ratio(colony: &galactic_sim::ColonyState) -> f32 {
    let Some(active) = colony.construction_queue.active() else {
        return 0.0;
    };
    if active.total_ticks == 0 {
        return 1.0;
    }
    let completed = active.total_ticks.saturating_sub(active.remaining_ticks);
    (completed as f64 / active.total_ticks as f64).clamp(0.0, 1.0) as f32
}

fn construction_error_text(error: galactic_sim::ConstructionError) -> String {
    match error {
        galactic_sim::ConstructionError::UnknownColony(_)
        | galactic_sim::ConstructionError::Access(_) => "Colonie indisponible".to_string(),
        galactic_sim::ConstructionError::QueueFull { maximum } => {
            format!("File pleine ({maximum})")
        }
        galactic_sim::ConstructionError::MaximumLevel { .. } => "Niveau maximal".to_string(),
        galactic_sim::ConstructionError::InsufficientResources { available, cost } => format!(
            "Manque: {}",
            construction_missing_resources_label(available, cost,),
        ),
        galactic_sim::ConstructionError::EnergyDeficit {
            production,
            consumption,
        } => format!("Énergie insuffisante : {production}/{consumption}",),
        galactic_sim::ConstructionError::Catalog(
            galactic_sim::BuildingCatalogError::UnsatisfiedPrerequisite {
                prerequisite,
                required,
                ..
            },
        ) => {
            let name = &galactic_sim::default_building_catalog()
                .definition(prerequisite)
                .name;
            format!("Requiert {name} niveau {required}")
        }
        galactic_sim::ConstructionError::Catalog(_) => "Règle catalogue invalide".to_string(),
        galactic_sim::ConstructionError::Reservation(_) => "Réservation impossible".to_string(),
    }
}

fn construction_cost_label(cost: galactic_domain::ResourceCost) -> String {
    construction_resource_amounts_label(cost.as_stock(), "gratuit")
}

fn construction_missing_resources_label(
    available: ResourceStock,
    cost: galactic_domain::ResourceCost,
) -> String {
    construction_resource_amounts_label(cost.as_stock().saturating_sub(available), "0")
}

fn construction_resource_amounts_label(resources: ResourceStock, empty_label: &str) -> String {
    let mut parts = Vec::new();
    append_resource_amount(&mut parts, resources.metal, "métal");
    append_resource_amount(&mut parts, resources.crystal, "cristal");
    append_resource_amount(&mut parts, resources.fuel, "carburant");

    if parts.is_empty() {
        empty_label.to_string()
    } else {
        parts.join(", ")
    }
}

fn append_resource_amount(parts: &mut Vec<String>, amount: u64, label: &str) {
    if amount > 0 {
        parts.push(format!("{amount} {label}"));
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
        let next_background = action_button_color(available, active, interaction);
        if background.0 != next_background {
            background.0 = next_background;
        }
        let next_outline = action_button_outline(available, active, interaction);
        if outline.color != next_outline {
            outline.color = next_outline;
        }
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
        Some(UiAction::LaunchProbe)
    } else if keyboard.just_pressed(KeyCode::KeyL) {
        Some(UiAction::AnalyzePlanet)
    } else if keyboard.just_pressed(KeyCode::KeyM) {
        Some(UiAction::LaunchAttack)
    } else if keyboard.just_pressed(KeyCode::KeyH) {
        Some(UiAction::LaunchHarvest)
    } else if keyboard.just_pressed(KeyCode::KeyN) {
        Some(UiAction::LaunchColonization)
    } else {
        None
    }
}

fn view_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<UiAction> {
    if keyboard.just_pressed(KeyCode::KeyP) {
        Some(UiAction::ToggleProjection)
    } else if keyboard.just_pressed(KeyCode::KeyR) {
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
        UiAction::TogglePause => apply_simulation_command(simulation, GameAction::TogglePause),
        UiAction::SetSpeed(speed) => {
            apply_simulation_command(simulation, GameAction::SetSpeed(speed));
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
        UiAction::LaunchProbe => {
            if let Some((colony_id, target)) = selected_probe_context(simulation.simulation()) {
                apply_simulation_command(simulation, GameAction::LaunchProbe { colony_id, target });
            }
        }
        UiAction::AnalyzePlanet => {
            if let Some(planet_id) = selected_analysis_target(simulation.simulation()) {
                apply_simulation_command(simulation, GameAction::AnalyzePlanet { planet_id });
            }
        }
        UiAction::LaunchAttack => {
            if let Some((colony_id, target)) = selected_attack_context(simulation.simulation()) {
                apply_simulation_command(
                    simulation,
                    GameAction::LaunchAttack { colony_id, target },
                );
            }
        }
        UiAction::LaunchHarvest => {
            if let Some((colony_id, site_id)) = selected_harvest_context(simulation.simulation()) {
                apply_simulation_command(
                    simulation,
                    GameAction::LaunchHarvest { colony_id, site_id },
                );
            }
        }
        UiAction::LaunchColonization => {
            if let Some((colony_id, target)) =
                selected_colonization_context(simulation.simulation())
            {
                apply_simulation_command(
                    simulation,
                    GameAction::LaunchColonization { colony_id, target },
                );
            }
        }
        UiAction::ToggleProjection => {
            navigation.toggle_projection();
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

fn apply_simulation_command(simulation: &mut SimulationResource, action: GameAction) {
    let events = simulation.simulation.apply_player_action(action);
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
        UiAction::ToggleProjection => {
            matches!(navigation.mode, StrategicViewMode::Universe)
        }
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
        UiAction::LaunchProbe => selected_probe_context(simulation.simulation()).is_some(),
        UiAction::LaunchAttack => selected_attack_context(simulation.simulation()).is_some(),
        UiAction::LaunchHarvest => selected_harvest_context(simulation.simulation()).is_some(),
        UiAction::LaunchColonization => {
            selected_colonization_context(simulation.simulation()).is_some()
        }
        UiAction::AnalyzePlanet => selected_analysis_target(simulation.simulation()).is_some(),
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
        UiAction::ToggleProjection => navigation.projection == UniverseProjection::Flattened,
        UiAction::ExitSystem => matches!(navigation.mode, StrategicViewMode::System(_)),
        _ => false,
    }
}

fn selected_probe_context(
    simulation: &Simulation,
) -> Option<(galactic_domain::ColonyId, MissionTarget)> {
    let state = simulation.state();
    let target = match state.selected {
        SelectionTarget::System(system_id)
            if state.system_knowledge_level(system_id) == KnowledgeLevel::Detected =>
        {
            MissionTarget::System(system_id)
        }
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } if state.planet_knowledge_level(planet_id) == KnowledgeLevel::Detected => {
            MissionTarget::Planet {
                system_id,
                planet_id,
            }
        }
        SelectionTarget::None | SelectionTarget::System(_) | SelectionTarget::Planet { .. } => {
            return None;
        }
    };
    Some((state.active_player_colony()?.id, target))
}

fn selected_analysis_target(simulation: &Simulation) -> Option<PlanetId> {
    let state = simulation.state();
    let SelectionTarget::Planet { planet_id, .. } = state.selected else {
        return None;
    };
    (state.planet_knowledge_level(planet_id) == KnowledgeLevel::Probed).then_some(planet_id)
}

fn selected_attack_context(
    simulation: &Simulation,
) -> Option<(galactic_domain::ColonyId, MissionTarget)> {
    let state = simulation.state();
    let SelectionTarget::Planet {
        system_id,
        planet_id,
    } = state.selected
    else {
        return None;
    };
    if state.planet_knowledge_level(planet_id) < KnowledgeLevel::Analyzed {
        return None;
    }
    let report = state.planetary_intelligence_report(planet_id)?;
    let PlanetaryOccupancyIntel::Occupied(occupant) = report.occupancy else {
        return None;
    };
    if occupant == state.player_faction {
        return None;
    }
    Some((
        state.active_player_colony()?.id,
        MissionTarget::Planet {
            system_id,
            planet_id,
        },
    ))
}

fn selected_harvest_context(
    simulation: &Simulation,
) -> Option<(galactic_domain::ColonyId, ExtractionSiteId)> {
    let state = simulation.state();
    let SelectionTarget::Planet { planet_id, .. } = state.selected else {
        return None;
    };
    if state.planet_knowledge_level(planet_id) < KnowledgeLevel::Analyzed
        || !state
            .research
            .has_unlock(TechnologyUnlock::RemoteExtraction)
        || state.colony_on_planet(planet_id).is_some()
    {
        return None;
    }
    let site = state.extraction_site_on_planet(planet_id)?;
    if site.is_depleted() || site.reserved_by.is_some() {
        return None;
    }
    Some((state.active_player_colony()?.id, site.id))
}

fn selected_colonization_context(
    simulation: &Simulation,
) -> Option<(galactic_domain::ColonyId, MissionTarget)> {
    let state = simulation.state();
    let SelectionTarget::Planet {
        system_id,
        planet_id,
    } = state.selected
    else {
        return None;
    };
    let colony_id = state.active_player_colony()?.id;
    assess_planet_colonizability(
        state,
        simulation.universe_repository(),
        state.player_faction,
        planet_id,
    )
    .is_colonizable()
    .then_some((
        colony_id,
        MissionTarget::Planet {
            system_id,
            planet_id,
        },
    ))
}

fn focus_selected_system(simulation: &SimulationResource, navigation: &mut StrategicNavigation) {
    let Some(system_id) = selected_system(simulation.simulation.state().selected) else {
        return;
    };
    let Some(system) = simulation.simulation.universe().system(system_id) else {
        return;
    };

    navigation.universe_focus =
        projected_universe_position(system.position, navigation.projection_mix);
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
    let systems = systems_for_universe_view(simulation.simulation(), debug_full_graph)
        .into_iter()
        .filter(|entry| entry.tier != UniverseSystemTier::Observed || debug_full_graph)
        .collect::<Vec<_>>();
    if systems.is_empty() {
        return;
    }

    let current = selected_system(simulation.simulation.state().selected);
    let current_index =
        current.and_then(|current_id| systems.iter().position(|entry| entry.id == current_id));
    let next_index = current_index
        .map(|index| (index + 1) % systems.len())
        .unwrap_or(0);
    let next_system = systems[next_index].id;

    apply_simulation_command(simulation, GameAction::SelectSystem(next_system));
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

    apply_simulation_command(
        simulation,
        GameAction::SelectPlanet {
            system_id,
            planet_id,
        },
    );
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

#[derive(SystemParam)]
struct StrategicCameraInput<'w> {
    time: Res<'w, Time>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    mouse_buttons: Res<'w, ButtonInput<MouseButton>>,
    mouse_motion: Res<'w, AccumulatedMouseMotion>,
    mouse_scroll: Res<'w, AccumulatedMouseScroll>,
    management: Res<'w, ColonyManagementState>,
    research: Res<'w, research_ui::ResearchUiState>,
    craft: Res<'w, craft_ui::CraftUiState>,
}

fn update_strategic_camera(
    input: StrategicCameraInput,
    mut navigation: ResMut<StrategicNavigation>,
    mut query: Query<&mut Transform, With<StrategicCamera>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };
    if input.management.open || input.research.open || input.craft.open {
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
        if matches!(event.kind, GameEventKind::ProductionRefreshed(_)) {
            continue;
        }
        if matches!(event.kind, GameEventKind::KnowledgeChanged(_)) {
            rebuild.0 = true;
        }
        log.last_event = Some(event);
    }
}

fn update_projection_transition(time: Res<Time>, mut navigation: ResMut<StrategicNavigation>) {
    const TRANSITION_SECONDS: f32 = 0.65;
    let current = navigation.projection_mix;
    let target = navigation.projection.target_mix();
    if current == target {
        return;
    }
    let step = time.delta_secs() / TRANSITION_SECONDS;
    let next = advance_projection_mix(current, target, step);
    if next != current {
        navigation.projection_mix = next;
    }
}

fn advance_projection_mix(current: f32, target: f32, maximum_step: f32) -> f32 {
    let maximum_step = maximum_step.max(0.0);
    if current < target {
        (current + maximum_step).min(target)
    } else {
        (current - maximum_step).max(target)
    }
    .clamp(0.0, 1.0)
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
        if let Some(system) = simulation.simulation().universe().system(visual.id) {
            let next = projected_universe_position(system.position, navigation.projection_mix);
            if transform.translation != next {
                transform.translation = next;
            }
        }
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
        let visibility_multiplier = match visual.tier {
            UniverseSystemTier::Known => 1.0,
            UniverseSystemTier::Detected => 0.84,
            UniverseSystemTier::Observed => 0.72,
        };

        let next_scale =
            visual.base_scale * selected_multiplier * lod_multiplier * visibility_multiplier;
        if transform.scale != next_scale {
            transform.scale = next_scale;
        }
    }
}

fn update_orbiting_visuals(
    time: Res<Time>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&OrbitingVisual, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::System(_)) {
        return;
    }

    let elapsed = time.elapsed_secs();
    for (orbit, mut transform) in &mut query {
        let next = orbit.translation_at(elapsed);
        if transform.translation != next {
            transform.translation = next;
        }
    }
}

fn update_planet_spins(
    time: Res<Time>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&AxialSpin, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::System(_)) {
        return;
    }

    let delta = time.delta_secs().min(0.1);
    for (spin, mut transform) in &mut query {
        transform.rotate_y(spin.radians_per_second * delta);
    }
}

fn update_system_labels(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SystemLabel, &mut Transform, &mut Visibility)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let state = simulation.simulation().state();
    let selected = selected_system(state.selected);

    for (label, mut transform, mut visibility) in &mut query {
        if let Some(system) = simulation.simulation().universe().system(label.id) {
            let next = projected_universe_position(system.position, navigation.projection_mix)
                + Vec3::new(0.0, 1.8, 0.0);
            if transform.translation != next {
                transform.translation = next;
            }
        }
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

        let next_visibility = if should_show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
    }
}

fn update_sector_labels(
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SectorLabel, &mut Transform, &mut Visibility)>,
) {
    let should_show = matches!(navigation.mode, StrategicViewMode::Universe)
        && navigation.lod == UniverseLod::Overview;
    for (label, mut transform, mut visibility) in &mut query {
        let next = projected_universe_position(label.position, navigation.projection_mix)
            + Vec3::new(0.0, 4.2, 0.0);
        if transform.translation != next {
            transform.translation = next;
        }
        let next_visibility = if should_show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
    }
}

fn update_pointer_halo_positions(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut halos: Query<(&PointerHalo, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    for (halo, mut transform) in &mut halos {
        let PickTarget::System(system_id) = halo.target else {
            continue;
        };
        if let Some(system) = simulation.simulation().universe().system(system_id) {
            let next = projected_universe_position(system.position, navigation.projection_mix);
            if transform.translation != next {
                transform.translation = next;
            }
        }
    }
}

fn draw_strategic_overlays(
    mut gizmos: Gizmos,
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    time: Res<Time>,
) {
    match navigation.mode {
        StrategicViewMode::Universe => {
            draw_universe_routes(&mut gizmos, simulation.simulation(), &navigation);
            draw_universe_missions(&mut gizmos, simulation.simulation(), &navigation);
        }
        StrategicViewMode::System(system_id) => {
            draw_system_orbits(&mut gizmos, simulation.simulation(), system_id);
            draw_system_missions(
                &mut gizmos,
                simulation.simulation(),
                system_id,
                time.elapsed_secs(),
            );
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
                navigation.projection_mix,
                RouteVisualStyle {
                    color: Color::srgba(0.42, 0.24, 0.62, 0.28),
                    dash_length: 1.25,
                    gap_length: 1.10,
                },
            );
        }
        return;
    }

    for route in state.visible_routes(simulation.universe_repository()) {
        let both_known = state.is_system_known(route.from) && state.is_system_known(route.to);
        let crosses_sector = simulation
            .universe_repository()
            .sector_for_system(route.from)
            .zip(simulation.universe_repository().sector_for_system(route.to))
            .is_some_and(|(from, to)| from.id != to.id);
        let color = if crosses_sector && both_known {
            Color::srgba(0.96, 0.66, 0.26, 0.72)
        } else if crosses_sector {
            Color::srgba(0.72, 0.48, 0.24, 0.46)
        } else if both_known {
            Color::srgba(0.28, 0.62, 0.94, 0.58)
        } else {
            Color::srgba(0.30, 0.48, 0.66, 0.38)
        };
        let (dash_length, gap_length) = if crosses_sector {
            (2.10, 0.90)
        } else if both_known {
            (1.60, 0.72)
        } else {
            (1.15, 1.00)
        };
        draw_route(
            gizmos,
            universe,
            route.from,
            route.to,
            navigation.projection_mix,
            RouteVisualStyle {
                color,
                dash_length,
                gap_length,
            },
        );
    }
}

fn draw_route(
    gizmos: &mut Gizmos,
    universe: &galactic_domain::UniverseDefinition,
    from_id: SystemId,
    to_id: SystemId,
    projection_mix: f32,
    style: RouteVisualStyle,
) {
    let Some(from) = universe.system(from_id) else {
        return;
    };
    let Some(to) = universe.system(to_id) else {
        return;
    };
    draw_dashed_line(
        gizmos,
        projected_universe_position(from.position, projection_mix),
        projected_universe_position(to.position, projection_mix),
        style.dash_length,
        style.gap_length,
        style.color,
    );
}

fn draw_dashed_line(
    gizmos: &mut Gizmos,
    start: Vec3,
    end: Vec3,
    dash_length: f32,
    gap_length: f32,
    color: Color,
) {
    let delta = end - start;
    let length = delta.length();
    let dash_length = dash_length.max(0.05);
    let gap_length = gap_length.max(0.0);
    if length <= f32::EPSILON {
        return;
    }

    let direction = delta / length;
    let step = dash_length + gap_length;
    let segment_count = dashed_segment_count(length, dash_length, gap_length);
    for segment in 0..segment_count {
        let offset = segment as f32 * step;
        let dash_end = (offset + dash_length).min(length);
        gizmos.line(
            start + direction * offset,
            start + direction * dash_end,
            color,
        );
    }
}

fn dashed_segment_count(length: f32, dash_length: f32, gap_length: f32) -> usize {
    if length <= 0.0 || dash_length <= 0.0 {
        return 0;
    }
    (length / (dash_length + gap_length.max(0.0))).ceil() as usize
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

fn draw_universe_missions(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    navigation: &StrategicNavigation,
) {
    let current_tick = simulation.state().clock.current_tick();
    for mission in simulation
        .state()
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal() && mission.plan.route.len() > 1)
    {
        let Some(progress) = mission_route_progress(mission, current_tick) else {
            continue;
        };
        let Some(position) = mission_route_position(
            simulation,
            &mission.plan.route,
            progress,
            navigation.projection_mix,
        ) else {
            continue;
        };
        draw_probe_marker(gizmos, position, 1.15);
    }
}

fn draw_system_missions(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    system_id: SystemId,
    elapsed_seconds: f32,
) {
    let current_tick = simulation.state().clock.current_tick();
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };
    for mission in simulation.state().player_missions().filter(|mission| {
        !mission.phase.is_terminal()
            && mission.order.origin == system_id
            && matches!(
                mission.order.target,
                MissionTarget::Planet {
                    system_id: target_system,
                    ..
                } if target_system == system_id
            )
    }) {
        let MissionTarget::Planet { planet_id, .. } = mission.order.target else {
            continue;
        };
        let Some(origin_colony) = simulation.state().colony(mission.origin_colony_id) else {
            continue;
        };
        let Some(origin_index) = system
            .planets
            .iter()
            .position(|planet| planet.id == origin_colony.planet_id)
        else {
            continue;
        };
        let Some(target_index) = system
            .planets
            .iter()
            .position(|planet| planet.id == planet_id)
        else {
            continue;
        };
        let Some(progress) = mission_route_progress(mission, current_tick) else {
            continue;
        };
        let origin_radius = 6.0 + origin_index as f32 * 4.8;
        let target_radius = 6.0 + target_index as f32 * 4.8;
        let origin =
            planet_orbit(origin_index, origin_radius, 0.32).translation_at(elapsed_seconds);
        let target =
            planet_orbit(target_index, target_radius, 0.32).translation_at(elapsed_seconds);
        gizmos.line(origin, target, Color::srgba(0.32, 0.88, 0.96, 0.24));
        draw_probe_marker(gizmos, origin.lerp(target, progress), 0.62);
    }
}

fn mission_route_progress(
    mission: &galactic_sim::MissionState,
    current_tick: galactic_sim::StrategicTick,
) -> Option<f32> {
    let ratio = |start: galactic_sim::StrategicTick, end: galactic_sim::StrategicTick| {
        let duration = end.value().saturating_sub(start.value());
        if duration == 0 {
            return 1.0;
        }
        current_tick.value().saturating_sub(start.value()) as f32 / duration as f32
    };
    match mission.phase {
        MissionPhase::Preparation => Some(0.0),
        MissionPhase::Outbound => Some(
            ratio(mission.order.departure_at, mission.plan.outbound_arrival_at).clamp(0.0, 1.0),
        ),
        MissionPhase::OnSite => Some(1.0),
        MissionPhase::Returning => Some(
            1.0 - ratio(
                mission.plan.return_departure_at,
                mission.plan.return_arrival_at,
            )
            .clamp(0.0, 1.0),
        ),
        MissionPhase::Completed | MissionPhase::Cancelled | MissionPhase::Failed => None,
    }
}

fn mission_route_position(
    simulation: &Simulation,
    route: &[SystemId],
    progress: f32,
    projection_mix: f32,
) -> Option<Vec3> {
    let segments = route.len().checked_sub(1)?;
    if segments == 0 {
        return None;
    }
    let scaled = progress.clamp(0.0, 1.0) * segments as f32;
    let segment = (scaled.floor() as usize).min(segments - 1);
    let local = (scaled - segment as f32).clamp(0.0, 1.0);
    let from = simulation.universe().system(route[segment])?;
    let to = simulation.universe().system(route[segment + 1])?;
    Some(
        projected_universe_position(from.position, projection_mix).lerp(
            projected_universe_position(to.position, projection_mix),
            local,
        ),
    )
}

fn draw_probe_marker(gizmos: &mut Gizmos, position: Vec3, radius: f32) {
    let color = Color::srgba(0.42, 0.96, 1.0, 0.96);
    gizmos.line(
        position - Vec3::X * radius,
        position + Vec3::X * radius,
        color,
    );
    gizmos.line(
        position - Vec3::Y * radius,
        position + Vec3::Y * radius,
        color,
    );
    gizmos.line(
        position - Vec3::Z * radius,
        position + Vec3::Z * radius,
        color,
    );
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
    let active_colony = state
        .active_player_colony()
        .map(|colony| format!("C{} {}", colony.id.raw(), colony.name))
        .unwrap_or_else(|| "aucune".to_string());
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
    let known_sector_count = known_sector_labels(simulation).len();
    let knowledge = state.system_knowledge_counts();
    let mission_status = mission_status_line(simulation);
    let view_label = match navigation.mode {
        StrategicViewMode::Universe => format!(
            "univers {:?} | projection {}",
            navigation.lod,
            navigation.projection.label(),
        ),
        StrategicViewMode::System(system_id) => {
            format!("système {}", system_id.index())
        }
    };

    let next = format!(
        "Galactic MVP | échelle {} ({}) | graphique {:?} | {} | tick {} | vitesse {} | colonie active {} | cible {}\nSystèmes {}/{} | Secteurs connus {}/{} | Routes {}/{} | Détectés/Sondés/Analysés/Colonisés {}/{}/{}/{} | debug {} | {}\n{}",
        navigation.scale_preset.label(),
        navigation.scale_preset.system_count(),
        navigation.preset,
        view_label,
        state.clock.current_tick(),
        state.clock.speed(),
        active_colony,
        selected,
        visible_system_count,
        universe.systems.len(),
        known_sector_count,
        universe.sectors.len(),
        visible_route_count,
        universe.routes.len(),
        knowledge.detected,
        knowledge.probed,
        knowledge.analyzed,
        knowledge.colonized,
        navigation.debug_full_graph,
        last_event,
        mission_status,
    );
    if text.0 != next {
        text.0 = next;
    }
}

fn mission_status_line(simulation: &Simulation) -> String {
    let state = simulation.state();
    let Some(mission) = state
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal())
        .min_by_key(|mission| mission.id)
    else {
        return "Missions : aucune mission active".to_string();
    };
    let target = mission_target_label(simulation, mission.order.target);
    let phase = match mission.phase {
        MissionPhase::Preparation => "préparation",
        MissionPhase::Outbound => "transit aller",
        MissionPhase::OnSite => "sur place",
        MissionPhase::Returning => "transit retour",
        MissionPhase::Completed => "terminée",
        MissionPhase::Cancelled => "annulée",
        MissionPhase::Failed => "échec",
    };
    let deadline = match mission.phase {
        MissionPhase::Preparation => mission.order.departure_at,
        MissionPhase::Outbound => mission.plan.outbound_arrival_at,
        MissionPhase::OnSite => mission.plan.return_departure_at,
        MissionPhase::Returning => mission.plan.return_arrival_at,
        MissionPhase::Completed | MissionPhase::Cancelled | MissionPhase::Failed => {
            state.clock.current_tick()
        }
    };
    let remaining = deadline
        .value()
        .saturating_sub(state.clock.current_tick().value());
    let kind = match mission.order.kind {
        MissionKind::Probe => "Reconnaissance",
        MissionKind::Attack => "Attaque",
        MissionKind::Transport => "Transport",
        MissionKind::Harvest => "Récolte",
        MissionKind::Colonize => "Colonisation",
    };

    format!(
        "Mission {} • {} vers {} • {} • prochaine étape dans {}",
        mission.id.raw(),
        kind,
        target,
        phase,
        format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(remaining)),
    )
}

fn mission_target_label(simulation: &Simulation, target: MissionTarget) -> String {
    let state = simulation.state();
    match target {
        MissionTarget::System(system_id) => simulation
            .universe()
            .system(system_id)
            .map(|system| {
                if state.system_knowledge_level(system_id).reveals_identity() {
                    system.name.clone()
                } else {
                    format!("Signal {}", system_id.index())
                }
            })
            .unwrap_or_else(|| format!("Système {}", system_id.index())),
        MissionTarget::Planet {
            system_id,
            planet_id,
        } => simulation
            .universe()
            .system(system_id)
            .and_then(|system| {
                system
                    .planets
                    .iter()
                    .position(|planet| planet.id == planet_id)
                    .map(|index| {
                        if state.planet_knowledge_level(planet_id).reveals_identity() {
                            system.planets[index].name.clone()
                        } else {
                            provisional_planet_label(&system.name, index)
                        }
                    })
            })
            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),
    }
}

fn update_info_panel(
    simulation: Res<SimulationResource>,
    mut query: Query<(&mut Text, &mut TextColor), With<InfoPanelText>>,
) {
    let Ok((mut text, mut color)) = query.single_mut() else {
        return;
    };
    let content = information_panel_content(simulation.simulation());
    let next_text = content.render();
    if text.0 != next_text {
        text.0 = next_text;
    }
    let next_color = knowledge_color(content.level);
    if color.0 != next_color {
        color.0 = next_color;
    }
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
    let Some(colony) = state.active_player_colony() else {
        return inspector_error("Colonie active introuvable");
    };
    let Some(system) = simulation.universe().system(colony.system_id) else {
        return inspector_error("Système de la colonie active introuvable");
    };
    let Some(planet) = simulation.universe_repository().planet(colony.planet_id) else {
        return inspector_error("Planète de la colonie active introuvable");
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
{}",
            faction.name,
            planet.habitability,
            colony_economy_text(colony),
            colony.resource_profile.metal,
            colony.resource_profile.crystal,
            colony.resource_profile.fuel,
            colony.resource_profile.energy,
            colony_buildings_text(colony),
        ),
        hint: "Colonie active : ressources et énergie sont exactes.".to_string(),
    }
}

fn colony_economy_text(colony: &galactic_sim::ColonyState) -> String {
    let available = colony.resources.available();
    let production = galactic_sim::colony_production_snapshot(colony);
    let construction = colony
        .construction_queue
        .active()
        .map(|order| {
            let name = &galactic_sim::default_building_catalog()
                .definition(order.kind)
                .name;
            format!(
                "{} niveau {} — {}",
                name,
                order.target_level,
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    order.remaining_ticks,
                ),),
            )
        })
        .unwrap_or_else(|| "aucune".to_string());

    format!(
        "ÉCONOMIE — RÉSUMÉ
Disponible : {} métal, {} cristal, {} carburant
Production : +{:.2} / +{:.2} / +{:.2} par seconde
Énergie : {} produite, {} consommée
Construction : {}

Gestion complète : touche C",
        available.metal,
        available.crystal,
        available.fuel,
        production.effective_rate.metal_per_second(),
        production.effective_rate.crystal_per_second(),
        production.effective_rate.fuel_per_second(),
        production.effective_energy_production,
        production.energy_consumption,
        construction,
    )
}

fn colony_buildings_text(colony: &galactic_sim::ColonyState) -> String {
    galactic_sim::default_building_catalog()
        .definitions()
        .map(|definition| {
            format!(
                "{} : {}",
                definition.name,
                colony.buildings.level(definition.kind),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    let orbit_index = system
        .planets
        .iter()
        .position(|candidate| candidate.id == planet_id)
        .unwrap_or_default();

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
            provisional_planet_label(&system.name, orbit_index),
            format!(
                "Système : {}
Identité : non déterminée
Orbite : {}
Type : ???
Habitabilité : ???
Potentiel : analyse requise
Lunes : non recensées
{}",
                system_label,
                orbit_index + 1,
                selection_note,
            ),
        ),
        KnowledgeLevel::Probed => (
            planet.name.clone(),
            format!(
                "Système : {}
Type : {:?}
Habitabilité estimée : {}
Potentiel : analyse requise

{}

Lunes : non recensées
{}",
                system_label,
                planet.kind,
                habitability_estimate(planet.habitability),
                planetary_intelligence_text(simulation, planet_id),
                selection_note,
            ),
        ),
        KnowledgeLevel::Analyzed => (
            planet.name.clone(),
            analyzed_planet_text(simulation, &system_label, planet, selection_note),
        ),
        KnowledgeLevel::Colonized => (
            planet.name.clone(),
            format!(
                "Système : {}
Type : {:?}
Habitabilité exacte : {}%
Statut : {}

{}

Lunes : aucune donnée disponible
{}",
                system_label,
                planet.kind,
                planet.habitability,
                colony
                    .map(|value| value.name.as_str())
                    .unwrap_or("colonie non référencée"),
                planetary_intelligence_text(simulation, planet_id),
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
{}",
            colony_economy_text(colony),
            colony.resource_profile.metal,
            colony.resource_profile.crystal,
            colony.resource_profile.fuel,
            colony.resource_profile.energy,
            colony_buildings_text(colony),
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

fn analyzed_planet_text(
    simulation: &Simulation,
    system_label: &str,
    planet: &galactic_domain::Planet,
    selection_note: &str,
) -> String {
    let Some(report) = simulation.state().planet_analysis_report(planet.id) else {
        return format!(
            "Système : {system_label}
Type : {:?}
Habitabilité exacte : {}%
Rapport d'analyse : manquant
{selection_note}",
            planet.kind, planet.habitability,
        );
    };
    let constraints = report
        .constraints
        .iter()
        .map(installation_constraint_label)
        .collect::<Vec<_>>();
    let constraints = if constraints.is_empty() {
        "aucune".to_string()
    } else {
        constraints.join(", ")
    };
    let assessment = assess_planet_colonizability(
        simulation.state(),
        simulation.universe_repository(),
        simulation.state().player_faction,
        planet.id,
    );

    let mut body = format!(
        "Système : {system_label}
Type : {:?}
Environnement : {}
Habitabilité exacte : {}%
Contraintes : {constraints}
Rapport établi au tick {}

POTENTIEL EXACT
Métal : {}
Cristal : {}
Carburant : {}
Énergie : {}

{}

{}

Lunes : aucune donnée disponible
{selection_note}",
        planet.kind,
        planet_environment_label(report.environment),
        report.habitability,
        report.analyzed_at.value(),
        report.resource_profile.metal,
        report.resource_profile.crystal,
        report.resource_profile.fuel,
        report.resource_profile.energy,
        planetary_intelligence_text(simulation, planet.id),
        colonizability_text(&assessment, simulation.state()),
    );
    body.push_str("\n\n");
    body.push_str(&extraction_site_text(simulation, planet.id));
    body
}

fn extraction_site_text(simulation: &Simulation, planet_id: PlanetId) -> String {
    let Some(site) = simulation.state().extraction_site_on_planet(planet_id) else {
        return "SITE D'EXTRACTION\nAucun gisement recensé".to_string();
    };
    let resource = match site.resource {
        ResourceKind::Metal => "métal",
        ResourceKind::Crystal => "cristal",
        ResourceKind::Fuel => "carburant",
        ResourceKind::Energy => "énergie",
    };
    let status = if site.is_depleted() {
        "épuisé".to_string()
    } else if let Some(mission_id) = site.reserved_by {
        format!("réservé par la mission {}", mission_id.raw())
    } else if simulation.state().colony_on_planet(planet_id).is_some() {
        "intégré à la colonie".to_string()
    } else if !simulation
        .state()
        .research
        .has_unlock(TechnologyUnlock::RemoteExtraction)
    {
        "Prospection autonome requise".to_string()
    } else {
        "disponible — H pour lancer la récolte".to_string()
    };
    let planet = simulation
        .universe_repository()
        .planet(planet_id)
        .expect("an extraction site references a generated planet");
    let rule = default_ruleset().extraction().rule_for(planet.kind);

    format!(
        "SITE D'EXTRACTION\nRessource : {resource}\nRéserve : {}\nRendement : {}/tick pendant {} ticks\nStatut : {status}",
        site.remaining, rule.yield_per_tick, rule.harvest_ticks,
    )
}

fn planetary_intelligence_text(simulation: &Simulation, planet_id: PlanetId) -> String {
    let state = simulation.state();
    let Some(report) = state.planetary_intelligence_report(planet_id) else {
        return "RENSEIGNEMENT PLANÉTAIRE\nRapport indisponible".to_string();
    };

    let observed = format!("Observation au tick {}", report.observed_at.value());
    let intelligence = match report.precision {
        PlanetaryIntelPrecision::Contact => {
            let presence = match report.occupancy {
                PlanetaryOccupancyIntel::Unoccupied => {
                    "aucune présence organisée détectée".to_string()
                }
                PlanetaryOccupancyIntel::OccupiedUnknown => {
                    "signature occupante détectée, identité inconnue".to_string()
                }
                PlanetaryOccupancyIntel::Occupied(faction_id) => {
                    format!("signature attribuée à la faction {}", faction_id.raw())
                }
            };
            format!(
                "RENSEIGNEMENT PLANÉTAIRE — CONTACT\n{observed}\nPrésence : {presence}\nForces terrestres : {}\nDéfenses orbitales : {}\nUne analyse est requise pour identifier les unités et leurs effectifs.",
                strategic_signal_label(report.ground_strength),
                strategic_signal_label(report.orbital_strength),
            )
        }
        PlanetaryIntelPrecision::Surveyed | PlanetaryIntelPrecision::Exact => {
            let precision_label = if report.precision == PlanetaryIntelPrecision::Exact {
                "DONNÉES LOCALES"
            } else {
                "ESTIMATION"
            };
            let presence = match report.occupancy {
                PlanetaryOccupancyIntel::Unoccupied => "aucune présence organisée".to_string(),
                PlanetaryOccupancyIntel::OccupiedUnknown => {
                    "présence occupante non attribuée".to_string()
                }
                PlanetaryOccupancyIntel::Occupied(faction_id) => {
                    let name = state
                        .faction(faction_id)
                        .map(|faction| faction.name.as_str())
                        .unwrap_or("faction inconnue");
                    let relation = state
                        .relation_between(state.player_faction, faction_id)
                        .unwrap_or(DiplomaticRelation::Unknown);
                    format!("{name} — relation {}", diplomatic_relation_label(relation))
                }
            };
            let population = report
                .population
                .map(estimate_range_text)
                .unwrap_or_else(|| "aucune".to_string());
            let force_catalog = default_ruleset().planetary_presence();
            let forces = if report.forces.is_empty() {
                "• aucune unité recensée".to_string()
            } else {
                report
                    .forces
                    .iter()
                    .map(|force| {
                        let Some(definition) = force_catalog.definition(force.definition_id) else {
                            return format!(
                                "• unité inconnue {} : {}",
                                force.definition_id,
                                estimate_range_text(force.quantity),
                            );
                        };
                        format!(
                            "• {} ({}) : {}",
                            definition.name,
                            planetary_force_domain_label(definition.domain),
                            estimate_range_text(force.quantity),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                "RENSEIGNEMENT PLANÉTAIRE — {precision_label}\n{observed}\nOccupant : {presence}\nPopulation : {population}\nIndice terrestre : {}\nIndice orbital : {}\n{forces}",
                estimate_range_text(report.ground_strength),
                estimate_range_text(report.orbital_strength),
            )
        }
    };
    state
        .latest_combat_report_for_planet(planet_id)
        .map(|combat| format!("{intelligence}\n\n{}", combat_report_text(combat)))
        .unwrap_or(intelligence)
}

fn combat_report_text(report: &galactic_sim::CombatReport) -> String {
    match &report.status {
        CombatReportStatus::TargetInvalid(reason) => format!(
            "RAPPORT DE COMBAT — CIBLE INVALIDÉE\nMission {} • tick {}\n{}.\nAucune donnée défensive supplémentaire n'a été révélée.",
            report.mission_id.raw(),
            report.resolved_at.value(),
            attack_invalid_reason_label(*reason),
        ),
        CombatReportStatus::Resolved(resolution) => {
            let control = match resolution.control {
                CombatControlChange::Unchanged => "contrôle territorial inchangé",
                CombatControlChange::Secured { .. } => "orbite et surface sécurisées par le joueur",
            };
            format!(
                "RAPPORT DE COMBAT — {}\nMission {} • tick {} • {} round(s)\nAttaquants engagés : {}\nAttaquants survivants : {}\nDéfense engagée : {}\nDéfense survivante : {}\nDommages subis/infligés : {} / {}\nRécupérable : {} métal, {} cristal, {} carburant\nRécupéré : {} métal, {} cristal, {} carburant\nContrôle : {}",
                combat_outcome_label(resolution.outcome).to_uppercase(),
                report.mission_id.raw(),
                report.resolved_at.value(),
                resolution.rounds,
                combat_ship_stacks_text(&report.attacker.ships),
                combat_ship_stacks_text(&resolution.attacker_survivors),
                planetary_force_stacks_text(&report.defender.forces),
                planetary_force_stacks_text(&resolution.defender_survivors),
                resolution.attacker_damage,
                resolution.defender_damage,
                resolution.salvage_recoverable.metal,
                resolution.salvage_recoverable.crystal,
                resolution.salvage_recoverable.fuel,
                resolution.salvage_recovered.metal,
                resolution.salvage_recovered.crystal,
                resolution.salvage_recovered.fuel,
                control,
            )
        }
    }
}

fn combat_ship_stacks_text(stacks: &[galactic_sim::CombatShipStack]) -> String {
    if stacks.is_empty() {
        return "aucun".to_string();
    }
    stacks
        .iter()
        .map(|stack| {
            format!(
                "{} × {}",
                stack.quantity,
                galactic_sim::craftable_definition(stack.craftable).name,
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn planetary_force_stacks_text(stacks: &[galactic_sim::PlanetaryForceStack]) -> String {
    if stacks.is_empty() {
        return "aucune".to_string();
    }
    let catalog = default_ruleset().planetary_presence();
    stacks
        .iter()
        .map(|stack| {
            let name = catalog
                .definition(stack.definition_id)
                .map(|definition| definition.name)
                .unwrap_or("unité inconnue");
            format!("{} × {name}", stack.quantity)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

const fn combat_outcome_label(outcome: CombatOutcome) -> &'static str {
    match outcome {
        CombatOutcome::AttackerVictory => "victoire attaquante",
        CombatOutcome::DefenderVictory => "victoire défensive",
        CombatOutcome::Stalemate => "affrontement indécis",
        CombatOutcome::MutualDestruction => "destruction mutuelle",
    }
}

const fn attack_invalid_reason_label(reason: galactic_sim::AttackInvalidReason) -> &'static str {
    match reason {
        galactic_sim::AttackInvalidReason::TargetOwnerChanged => {
            "le contrôle de la planète a changé pendant le trajet"
        }
        galactic_sim::AttackInvalidReason::TargetPresenceChanged => {
            "les forces présentes ont changé pendant le trajet"
        }
        galactic_sim::AttackInvalidReason::AttackerFleetChanged => {
            "la flotte attaquante ne correspond plus à l'engagement"
        }
    }
}

fn estimate_range_text(range: EstimateRange) -> String {
    if range.is_exact() {
        range.minimum.to_string()
    } else {
        format!("{}–{}", range.minimum, range.maximum)
    }
}

fn strategic_signal_label(range: EstimateRange) -> &'static str {
    match range.maximum {
        0 => "aucune signature",
        1..=749 => "faible",
        750..=1_999 => "modérée",
        _ => "forte",
    }
}

const fn planetary_force_domain_label(domain: PlanetaryForceDomain) -> &'static str {
    match domain {
        PlanetaryForceDomain::Ground => "sol",
        PlanetaryForceDomain::Orbital => "orbite",
    }
}

const fn diplomatic_relation_label(relation: DiplomaticRelation) -> &'static str {
    match relation {
        DiplomaticRelation::Unknown => "inconnue",
        DiplomaticRelation::Neutral => "neutre",
        DiplomaticRelation::Hostile => "hostile",
        DiplomaticRelation::Allied => "alliée",
    }
}

fn planet_environment_label(environment: PlanetEnvironment) -> &'static str {
    match environment {
        PlanetEnvironment::Temperate => "tempéré",
        PlanetEnvironment::Oceanic => "océanique",
        PlanetEnvironment::Arid => "aride",
        PlanetEnvironment::Frozen => "gelé",
        PlanetEnvironment::Volcanic => "volcanique",
        PlanetEnvironment::Gaseous => "gazeux",
    }
}

fn installation_constraint_label(constraint: InstallationConstraint) -> &'static str {
    match constraint {
        InstallationConstraint::ThinAtmosphere => "atmosphère ténue",
        InstallationConstraint::GlobalOcean => "océan global",
        InstallationConstraint::AridClimate => "climat aride",
        InstallationConstraint::CryogenicClimate => "climat cryogénique",
        InstallationConstraint::ExtremeVolcanism => "volcanisme extrême",
        InstallationConstraint::NoSolidSurface => "absence de surface solide",
    }
}

fn colonizability_text(
    assessment: &galactic_sim::ColonizabilityAssessment,
    state: &galactic_sim::GameState,
) -> String {
    let cost = assessment.foundation_cost;
    if assessment.is_colonizable() {
        let colony_ship = default_ruleset().planetary_analysis().colony_ship();
        let active_colony = state.active_player_colony();
        let origin = active_colony
            .map(|colony| colony.name.as_str())
            .unwrap_or("aucune colonie active");
        let ship_ready = active_colony.is_some_and(|colony| {
            colony.inventory.quantity(colony_ship) > 0
                || state.fleets.iter().any(|fleet| {
                    fleet.is_idle()
                        && fleet.location == galactic_sim::FleetLocation::Docked(colony.id)
                        && fleet.composition.total_ships() == 1
                        && fleet.composition.quantity(colony_ship) == 1
                })
        });
        let ship_status = if ship_ready {
            format!(
                "Depuis {origin} : Arche Pionnière disponible — appuyez sur N pour lancer la mission."
            )
        } else {
            format!(
                "Depuis {origin} : Arche Pionnière manquante — construisez-en une au chantier orbital."
            )
        };
        return format!(
            "COLONISABILITÉ — ÉLIGIBLE
Conditions remplies : analyse, environnement, habitabilité, route, technologie, limite et cargaison.
Investissement requis : {} métal, {} cristal, {} carburant
{ship_status}",
            cost.metal, cost.crystal, cost.fuel,
        );
    }

    let blockers = assessment
        .blockers
        .iter()
        .map(|blocker| format!("• {}", colonization_blocker_label(*blocker, state)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "COLONISABILITÉ — BLOQUÉE
Conditions manquantes :
{blockers}
Investissement requis : {} métal, {} cristal, {} carburant",
        cost.metal, cost.crystal, cost.fuel,
    )
}

fn colonization_blocker_label(
    blocker: ColonizationBlocker,
    state: &galactic_sim::GameState,
) -> String {
    match blocker {
        ColonizationBlocker::UnknownPlanet(_) => "planète inconnue".to_string(),
        ColonizationBlocker::NotAnalyzed { current } => {
            format!("analyse complète requise (niveau actuel : {current})")
        }
        ColonizationBlocker::MissingAnalysisReport => {
            "rapport d'analyse persistant introuvable".to_string()
        }
        ColonizationBlocker::AlreadyColonized => "planète déjà colonisée".to_string(),
        ColonizationBlocker::FoundationAlreadyPrepared => {
            "une fondation coloniale attend son initialisation".to_string()
        }
        ColonizationBlocker::OccupiedPlanet { occupant, relation } => {
            let name = state
                .faction(occupant)
                .map(|faction| faction.name.as_str())
                .unwrap_or("faction inconnue");
            format!(
                "présence {} non sécurisée : {name}",
                diplomatic_relation_label(relation),
            )
        }
        ColonizationBlocker::MissingTechnology(TechnologyUnlock::FoundColonies) => {
            "technologie Ingénierie d'implantation manquante".to_string()
        }
        ColonizationBlocker::MissingTechnology(unlock) => {
            format!("technologie requise manquante : {unlock:?}")
        }
        ColonizationBlocker::UnsupportedEnvironment(environment) => format!(
            "environnement {} incompatible avec une implantation au sol",
            planet_environment_label(environment),
        ),
        ColonizationBlocker::HabitabilityTooLow { minimum, found } => {
            format!("habitabilité insuffisante : {found}% (minimum {minimum}%)")
        }
        ColonizationBlocker::NoAccessibleRoute => {
            "aucune route connue depuis une colonie du joueur".to_string()
        }
        ColonizationBlocker::ColonyLimitReached { maximum } => {
            format!("limite de {maximum} colonies déjà atteinte")
        }
        ColonizationBlocker::InsufficientFoundationResources { cost } => format!(
            "aucune colonie ne dispose de la cargaison de fondation ({} métal, {} cristal, {} carburant)",
            cost.metal, cost.crystal, cost.fuel,
        ),
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
        KnowledgeLevel::Analyzed => "[ANALYSÉ — RAPPORT COMPLET]",
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

fn projected_universe_position(position: WorldPosition, projection_mix: f32) -> Vec3 {
    let spatial = Vec3::new(
        position.x,
        position.y * UNIVERSE_VERTICAL_EXAGGERATION,
        position.z,
    );
    let flattened = Vec3::new(position.x, 0.0, position.z);
    spatial.lerp(flattened, projection_mix.clamp(0.0, 1.0))
}

fn star_material(class: StarClass) -> StandardMaterial {
    StandardMaterial {
        base_color: star_color(class),
        emissive: star_emissive(class),
        unlit: true,
        ..default()
    }
}

fn star_halo_material(class: StarClass) -> StandardMaterial {
    let color = star_color(class).with_alpha(0.18);
    StandardMaterial {
        base_color: color,
        emissive: star_emissive(class) * 0.22,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

fn planet_material(kind: PlanetKind, texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        perceptual_roughness: match kind {
            PlanetKind::Ocean => 0.56,
            PlanetKind::Ice => 0.48,
            PlanetKind::GasGiant => 0.72,
            PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => 0.88,
        },
        metallic: 0.0,
        ..default()
    }
}

fn atmosphere_material(kind: PlanetKind) -> StandardMaterial {
    let base_color = match kind {
        PlanetKind::Ocean => Color::srgba(0.20, 0.66, 1.0, 0.16),
        PlanetKind::Ice => Color::srgba(0.64, 0.86, 1.0, 0.12),
        PlanetKind::GasGiant => Color::srgba(0.84, 0.72, 0.94, 0.10),
        PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => Color::NONE,
    };
    StandardMaterial {
        base_color,
        emissive: LinearRgba::from(base_color) * 0.34,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

fn procedural_planet_texture(kind: PlanetKind) -> Image {
    let mut texture =
        Vec::with_capacity((PLANET_TEXTURE_WIDTH * PLANET_TEXTURE_HEIGHT * 4) as usize);
    for y in 0..PLANET_TEXTURE_HEIGHT {
        for x in 0..PLANET_TEXTURE_WIDTH {
            texture.extend_from_slice(&procedural_planet_pixel(kind, x, y));
        }
    }

    Image::new_fill(
        Extent3d {
            width: PLANET_TEXTURE_WIDTH,
            height: PLANET_TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn procedural_planet_pixel(kind: PlanetKind, x: u32, y: u32) -> [u8; 4] {
    let noise = visual_hash(x, y, planet_kind_seed(kind));
    let color = match kind {
        PlanetKind::Rocky => {
            if noise > 205 {
                [126, 112, 94]
            } else if noise > 92 {
                [92, 82, 72]
            } else {
                [58, 54, 52]
            }
        }
        PlanetKind::Ocean => {
            let polar_cap = y < 2 || y + 2 >= PLANET_TEXTURE_HEIGHT;
            let land = noise > 208 && (x + y * 3).is_multiple_of(2);
            if polar_cap {
                [220, 239, 246]
            } else if land {
                [54, 126, 88]
            } else if noise > 118 {
                [26, 104, 166]
            } else {
                [15, 61, 126]
            }
        }
        PlanetKind::Desert => {
            if (y + x / 9).is_multiple_of(7) {
                [218, 160, 72]
            } else if noise > 148 {
                [184, 118, 51]
            } else {
                [128, 74, 38]
            }
        }
        PlanetKind::Ice => {
            let ridge = (x * 3 + y * 5 + u32::from(noise)).is_multiple_of(19);
            if ridge {
                [118, 178, 205]
            } else if noise > 128 {
                [218, 239, 244]
            } else {
                [164, 207, 224]
            }
        }
        PlanetKind::GasGiant => {
            let storm_x = x.abs_diff(46);
            let storm_y = y.abs_diff(20);
            if storm_x * storm_x + storm_y * storm_y < 18 {
                [184, 106, 76]
            } else {
                match (y / 3 + u32::from(noise > 190)) % 4 {
                    0 => [188, 154, 176],
                    1 => [126, 104, 148],
                    2 => [218, 184, 150],
                    _ => [154, 124, 166],
                }
            }
        }
        PlanetKind::Volcanic => {
            let lava = (x * 7 + y * 11 + u32::from(noise)).is_multiple_of(17);
            if lava {
                [246, 101, 24]
            } else if noise > 150 {
                [105, 35, 25]
            } else {
                [40, 28, 28]
            }
        }
    };
    [color[0], color[1], color[2], 255]
}

fn visual_hash(x: u32, y: u32, seed: u32) -> u8 {
    let mut value = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    (value >> 24) as u8
}

const fn planet_kind_seed(kind: PlanetKind) -> u32 {
    match kind {
        PlanetKind::Rocky => 1,
        PlanetKind::Ocean => 2,
        PlanetKind::Desert => 3,
        PlanetKind::Ice => 4,
        PlanetKind::GasGiant => 5,
        PlanetKind::Volcanic => 6,
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
    match event.kind {
        GameEventKind::CommandRejected(error) => format!("commande refusée : {:?}", error),
        GameEventKind::SpeedChanged(speed) => format!("speed {}", speed),
        GameEventKind::SelectionChanged(selection) => {
            format!("selection {}", selection_label(selection))
        }
        GameEventKind::ActiveColonyChanged(colony_id) => {
            format!("colonie active : C{}", colony_id.raw())
        }
        GameEventKind::ActiveColonySelectionRejected(rejected) => format!(
            "sélection de la colonie C{} refusée : {:?}",
            rejected.colony_id.raw(),
            rejected.error,
        ),
        GameEventKind::KnowledgeChanged(change) => {
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
        GameEventKind::PlanetAnalyzed(report) => format!(
            "analyse terminée : planète {} caractérisée, colonisabilité évaluée",
            report.planet_id.index(),
        ),
        GameEventKind::PlanetAnalysisRejected(rejected) => {
            format!(
                "analyse refusée : {}",
                planet_analysis_error_text(rejected.error)
            )
        }
        GameEventKind::TicksAdvanced {
            ticks,
            current_tick,
        } => format!("+{} ticks -> {}", ticks.ticks(), current_tick),
        GameEventKind::ProductionRefreshed(_) => "production actualisée".to_string(),
        GameEventKind::ConstructionQueued(queued) => format!(
            "construction {:?} niveau {} ajoutée ({})",
            queued.order.kind, queued.order.target_level, queued.queue_length,
        ),
        GameEventKind::ConstructionCompleted(done) => format!(
            "construction {:?} niveau {} terminée",
            done.kind, done.new_level,
        ),
        GameEventKind::ConstructionRejected(rejected) => format!(
            "construction {:?} refusée : {:?}",
            rejected.kind, rejected.error,
        ),
        GameEventKind::ResearchQueued(queued) => format!(
            "recherche {:?} ajoutée ({})",
            queued.project.technology, queued.queue_length,
        ),
        GameEventKind::ResearchCompleted(completed) => {
            format!("recherche {:?} terminée", completed.technology,)
        }
        GameEventKind::ResearchRejected(rejected) => format!(
            "recherche {:?} refusée : {:?}",
            rejected.technology, rejected.error,
        ),
        GameEventKind::CraftQueued(queued) => format!(
            "craft {:?} ajouté ({})",
            queued.order.craftable, queued.queue_length,
        ),
        GameEventKind::CraftCompleted(completed) => format!(
            "craft {:?} terminé (stock {})",
            completed.craftable, completed.inventory_quantity,
        ),
        GameEventKind::CraftRejected(rejected) => format!(
            "craft {:?} refusé : {:?}",
            rejected.craftable, rejected.error,
        ),
        GameEventKind::FleetCreated(created) => {
            format!("flotte {:?} formée", created.fleet_id)
        }
        GameEventKind::FleetCreationRejected(rejected) => {
            format!("formation de flotte refusée : {:?}", rejected.error)
        }
        GameEventKind::MissionLaunched(launched) => format!(
            "mission {:?} lancée vers {:?}",
            launched.kind, launched.target,
        ),
        GameEventKind::MissionLaunchRejected(rejected) => {
            format!("mission refusée : {}", mission_error_text(rejected.error))
        }
        GameEventKind::MissionTransitioned(transition) => format!(
            "mission {:?} : {:?} -> {:?}",
            transition.mission_id, transition.from, transition.to,
        ),
        GameEventKind::MissionResolved(resolution) => match resolution.result {
            MissionResult::Probe(result) => match result.target {
                MissionTarget::System(system_id) => format!(
                    "reconnaissance terminée : système {} sondé, {} nouveaux signaux, {} routes et {} planètes révélées",
                    system_id.index(),
                    result.newly_detected_systems,
                    result.revealed_routes,
                    result.revealed_planets,
                ),
                MissionTarget::Planet { planet_id, .. } => format!(
                    "reconnaissance planétaire terminée : corps {} identifié",
                    planet_id.index(),
                ),
            },
            MissionResult::Attack(result) => match result.outcome {
                AttackMissionOutcome::Resolved(outcome) => format!(
                    "combat terminé sur le corps {} : {}{}",
                    result.target.index(),
                    combat_outcome_label(outcome),
                    if result.secured {
                        ", planète sécurisée"
                    } else {
                        ""
                    },
                ),
                AttackMissionOutcome::TargetInvalid(_) => format!(
                    "attaque annulée sur le corps {} : cible devenue invalide",
                    result.target.index(),
                ),
            },
            MissionResult::Transport(result) => transport_result_label(result),
            MissionResult::Harvest(result) => format!(
                "récolte terminée sur le site {} : {}, {} livré, {} conservé en soute, réserve restante {}",
                result.site_id.raw(),
                transport_cargo_label(result.collected),
                transport_cargo_label(result.delivered),
                transport_cargo_label(result.retained),
                result.site_remaining,
            ),
            MissionResult::Colonize(result) => match result.outcome {
                ColonizationMissionOutcome::FoundationPrepared => format!(
                    "fondation prête sur le corps {} : Arche Pionnière et chargement déployés",
                    result.target.index(),
                ),
                ColonizationMissionOutcome::TargetInvalid(blocker) => format!(
                    "colonisation annulée sur le corps {} : {}",
                    result.target.index(),
                    colonization_arrival_failure_label(blocker),
                ),
            },
        },
        GameEventKind::ColonyFoundationPrepared(foundation) => format!(
            "fondation coloniale préparée sur le corps {} au tick {}",
            foundation.planet_id.index(),
            foundation.prepared_at.value(),
        ),
        GameEventKind::ColonyEstablished(colony) => format!(
            "nouvelle colonie établie sur le corps {} : colonie {} opérationnelle",
            colony.planet_id.index(),
            colony.colony_id.raw(),
        ),
        GameEventKind::MissionReported(report) => format!(
            "rapport mission {:?} : {:?}",
            report.mission_id, report.outcome,
        ),
        GameEventKind::MissionCancellationRejected(rejected) => {
            format!("annulation de mission refusée : {:?}", rejected.error)
        }
    }
}

fn planet_analysis_error_text(error: PlanetAnalysisError) -> String {
    match error {
        PlanetAnalysisError::Access(_) => {
            "la faction active ne peut pas effectuer cette analyse".to_string()
        }
        PlanetAnalysisError::UnknownPlanet(_) => "la planète sélectionnée est inconnue".to_string(),
        PlanetAnalysisError::PlanetNotProbed { .. } => {
            "la planète doit d'abord être identifiée par une Sonde Luciole".to_string()
        }
        PlanetAnalysisError::MissingTechnology(TechnologyUnlock::AnalyzePlanets) => {
            "recherchez Spectrométrie planétaire avant de lancer l'analyse".to_string()
        }
        PlanetAnalysisError::MissingTechnology(unlock) => {
            format!("technologie d'analyse manquante : {unlock:?}")
        }
        PlanetAnalysisError::AlreadyAnalyzed(_) => {
            "cette planète possède déjà un rapport d'analyse exact".to_string()
        }
    }
}

fn colonization_arrival_failure_label(blocker: ColonizationBlocker) -> &'static str {
    match blocker {
        ColonizationBlocker::UnknownPlanet(_) => "planète inconnue",
        ColonizationBlocker::NotAnalyzed { .. } | ColonizationBlocker::MissingAnalysisReport => {
            "analyse planétaire invalide"
        }
        ColonizationBlocker::AlreadyColonized => "planète déjà colonisée",
        ColonizationBlocker::FoundationAlreadyPrepared => "fondation déjà préparée",
        ColonizationBlocker::OccupiedPlanet { .. } => "présence étrangère détectée",
        ColonizationBlocker::MissingTechnology(_) => "technologie de colonisation manquante",
        ColonizationBlocker::UnsupportedEnvironment(_) => "environnement non compatible",
        ColonizationBlocker::HabitabilityTooLow { .. } => "habitabilité insuffisante",
        ColonizationBlocker::NoAccessibleRoute => "route devenue inaccessible",
        ColonizationBlocker::ColonyLimitReached { .. } => "limite de colonies atteinte",
        ColonizationBlocker::InsufficientFoundationResources { .. } => {
            "chargement de fondation indisponible"
        }
    }
}

fn mission_error_text(error: galactic_sim::MissionError) -> String {
    match error {
        galactic_sim::MissionError::ProbeUnavailable(_) => {
            "aucune Sonde Luciole disponible ; construisez-en une au chantier orbital".to_string()
        }
        galactic_sim::MissionError::ProbeRequired(_) => {
            "la flotte sélectionnée ne contient aucune Sonde Luciole".to_string()
        }
        galactic_sim::MissionError::ProbeTargetNotDetected { .. } => {
            "la reconnaissance exige un système ou une planète actuellement détecté".to_string()
        }
        galactic_sim::MissionError::AttackFleetUnavailable(_) => {
            "aucune Frégate Rempart disponible ; construisez-en au chantier orbital".to_string()
        }
        galactic_sim::MissionError::AttackTargetNotAnalyzed { .. } => {
            "la cible doit être sondée puis analysée avant une attaque".to_string()
        }
        galactic_sim::MissionError::AttackPlanetTargetRequired => {
            "une attaque doit cibler une planète".to_string()
        }
        galactic_sim::MissionError::Attack(
            galactic_sim::CombatSnapshotError::UnoccupiedTarget(_),
        ) => "la planète est inoccupée et ne peut pas être attaquée".to_string(),
        galactic_sim::MissionError::Attack(galactic_sim::CombatSnapshotError::FriendlyTarget(
            _,
        )) => "la planète est déjà sous contrôle allié".to_string(),
        galactic_sim::MissionError::Attack(
            galactic_sim::CombatSnapshotError::FleetNotCombatCapable(_),
        ) => "la flotte doit être composée de vaisseaux militaires".to_string(),
        galactic_sim::MissionError::TransportOrderRequired => {
            "utilisez l'ordre logistique avec une origine, une destination et une cargaison"
                .to_string()
        }
        galactic_sim::MissionError::TransportCargoEmpty => {
            "la cargaison de transport ne peut pas être vide".to_string()
        }
        galactic_sim::MissionError::TransportCargoAmountOverflow => {
            "la cargaison demandée est trop importante".to_string()
        }
        galactic_sim::MissionError::UnknownTransportDestination(_) => {
            "la colonie de destination n'existe plus".to_string()
        }
        galactic_sim::MissionError::TransportDestinationIsOrigin(_) => {
            "l'origine et la destination du transport doivent être différentes".to_string()
        }
        galactic_sim::MissionError::TransportDestinationTargetMismatch { .. } => {
            "la destination ne correspond plus à la colonie choisie".to_string()
        }
        galactic_sim::MissionError::TransportFleetUnavailable {
            required_capacity,
            available_capacity,
            ..
        } => format!(
            "capacité cargo insuffisante : {required_capacity} requise, {available_capacity} disponible ; construisez des Caboteurs Sillage"
        ),
        galactic_sim::MissionError::TransportFleetHasCargo(_) => {
            "la flotte sélectionnée transporte déjà une cargaison".to_string()
        }
        galactic_sim::MissionError::TransportCargoExceedsCapacity { capacity, .. } => {
            format!("la cargaison dépasse la capacité de la flotte ({capacity})")
        }
        galactic_sim::MissionError::HarvestOrderRequired => {
            "utilisez l'ordre de récolte avec une colonie d'origine et un site analysé".to_string()
        }
        galactic_sim::MissionError::UnknownExtractionSite(_) => {
            "le site d'extraction sélectionné n'existe plus".to_string()
        }
        galactic_sim::MissionError::HarvestTargetMismatch { .. } => {
            "le site ne correspond plus à la planète sélectionnée".to_string()
        }
        galactic_sim::MissionError::HarvestPlanetNotAnalyzed { .. } => {
            "la planète doit être sondée puis analysée avant toute récolte".to_string()
        }
        galactic_sim::MissionError::MissingHarvestTechnology(_) => {
            "recherchez Prospection autonome avant de lancer une récolte".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteOnColony(_) => {
            "ce gisement appartient déjà à une colonie et n'est pas un site distant".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteDepleted(_) => {
            "ce site d'extraction est épuisé".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteBusy { .. } => {
            "ce site est déjà réservé par une autre mission".to_string()
        }
        galactic_sim::MissionError::HarvestFleetUnavailable(_) => {
            "aucun Caboteur Sillage disponible ; construisez-en au chantier orbital".to_string()
        }
        galactic_sim::MissionError::HarvestFleetHasCargo(_) => {
            "la flotte de récolte transporte déjà une cargaison".to_string()
        }
        galactic_sim::MissionError::ColonizationPlanetTargetRequired => {
            "une colonisation doit cibler une planète".to_string()
        }
        galactic_sim::MissionError::ColonizationShipUnavailable(_) => {
            "aucune Arche Pionnière disponible ; construisez-en une au chantier orbital".to_string()
        }
        galactic_sim::MissionError::ColonizationFleetRequired(_) => {
            "la flotte de colonisation doit contenir exactement une Arche Pionnière".to_string()
        }
        galactic_sim::MissionError::ColonizationBlocked(blocker) => {
            format!(
                "colonisation impossible : {}",
                colonization_arrival_failure_label(blocker)
            )
        }
        galactic_sim::MissionError::NoAccessibleRoute { .. } => {
            "aucune route connue ne permet d'atteindre cette destination".to_string()
        }
        galactic_sim::MissionError::InsufficientRange {
            required_hops,
            available_hops,
        } => format!(
            "portée insuffisante : {required_hops} sauts requis, {available_hops} disponibles"
        ),
        galactic_sim::MissionError::Resources(_) => {
            "ressources insuffisantes dans la colonie d'origine (carburant ou cargaison)"
                .to_string()
        }
        galactic_sim::MissionError::FleetBusy { .. } => {
            "la flotte est déjà affectée à une mission".to_string()
        }
        galactic_sim::MissionError::FleetNotDocked(_) => {
            "la flotte doit être amarrée à la colonie d'origine".to_string()
        }
        galactic_sim::MissionError::UnknownTarget(_)
        | galactic_sim::MissionError::UnknownPlanetTarget(_)
        | galactic_sim::MissionError::UnknownOrigin(_) => {
            "origine ou destination inconnue".to_string()
        }
        galactic_sim::MissionError::PlanetTargetSystemMismatch { .. } => {
            "la planète ne correspond pas au système sélectionné".to_string()
        }
        galactic_sim::MissionError::SameSystem(_) => {
            "l'origine et la destination doivent être différentes".to_string()
        }
        _ => format!("{error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::{any::TypeId, collections::HashSet};

    use super::*;
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, visibility::VisibleEntities};
    use bevy::sprite::update_text2d_layout;
    use bevy::text::{
        LayoutCx, RemSize, ScaleCx, TextIterScratch, TextPipeline, detect_text_needs_rerender,
    };

    #[test]
    fn transport_management_queries_are_disjoint() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(update_colony_management_transport);

        system.initialize(&mut world);
    }

    #[test]
    fn renderer_favors_bounded_memory_allocations() {
        let RenderCreation::Automatic(settings) = low_memory_render_plugin().render_creation else {
            panic!("the client renderer must use automatic creation");
        };

        assert!(matches!(settings.memory_hints, MemoryHints::MemoryUsage));
    }

    #[test]
    fn linux_memory_status_parser_extracts_process_counters() {
        let snapshot = parse_linux_process_status(
            "\
VmRSS:\t  512000 kB
RssAnon:\t  480000 kB
RssFile:\t   12000 kB
RssShmem:\t   4000 kB
VmSwap:\t      2048 kB
",
        );

        assert_eq!(
            snapshot,
            ProcessMemorySnapshot {
                rss_kib: 512_000,
                anonymous_kib: 480_000,
                file_kib: 12_000,
                shared_kib: 4_000,
                swap_kib: 2_048,
            }
        );
    }

    #[test]
    fn scale_cli_defaults_to_mvp_and_accepts_all_presets() {
        assert_eq!(
            universe_scale_preset_from_args(Vec::<String>::new()),
            Ok(UniverseScalePreset::Mvp),
        );
        assert_eq!(
            universe_scale_preset_from_args(["--scale", "test"].map(str::to_string)),
            Ok(UniverseScalePreset::Test),
        );
        assert_eq!(
            universe_scale_preset_from_args(["--scale=stress"].map(str::to_string)),
            Ok(UniverseScalePreset::Stress),
        );
        assert!(universe_scale_preset_from_args(["--scale=huge"].map(str::to_string)).is_err());
    }

    #[test]
    fn flattened_projection_is_interpolated_and_round_trips_without_drift() {
        let position = WorldPosition::new(12.0, 4.0, -7.0);
        let spatial = projected_universe_position(position, 0.0);
        let halfway = projected_universe_position(position, 0.5);
        let flattened = projected_universe_position(position, 1.0);

        assert_eq!(spatial, Vec3::new(12.0, 13.6, -7.0));
        assert_eq!(halfway, Vec3::new(12.0, 6.8, -7.0));
        assert_eq!(flattened, Vec3::new(12.0, 0.0, -7.0));
        assert_eq!(projected_universe_position(position, 0.0), spatial);
    }

    #[test]
    fn projection_transition_reaches_exact_endpoints() {
        let flattened = (0..10).fold(0.0, |mix, _| advance_projection_mix(mix, 1.0, 0.17));
        let spatial = (0..10).fold(flattened, |mix, _| advance_projection_mix(mix, 0.0, 0.17));

        assert_eq!(flattened, 1.0);
        assert_eq!(spatial, 0.0);
    }

    #[test]
    fn low_graphics_mvp_scale_stays_inside_the_render_budget() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let universe = simulation.universe();

        assert_eq!(GraphicsPreset::default(), GraphicsPreset::Low);
        assert_eq!(
            universe.systems.len(),
            UniverseScalePreset::Mvp.system_count(),
        );
        assert_eq!(universe.sectors.len(), 8);
        assert!(universe.routes.len() <= universe.systems.len() * 3);
    }

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
    fn resource_fill_ratio_is_clamped() {
        assert_eq!(resource_fill_ratio(0, 100), 0.0);
        assert_eq!(resource_fill_ratio(50, 100), 0.5);
        assert_eq!(resource_fill_ratio(150, 100), 1.0);
        assert_eq!(resource_fill_ratio(0, 0), 0.0);
        assert_eq!(resource_fill_ratio(1, 0), 1.0);
    }

    #[test]
    fn resource_status_distinguishes_normal_nearly_full_and_full() {
        assert_eq!(resource_hud_status(40, 100), ResourceHudStatus::Normal);
        assert_eq!(resource_hud_status(90, 100), ResourceHudStatus::NearlyFull);
        assert_eq!(resource_hud_status(100, 100), ResourceHudStatus::Full);
    }

    #[test]
    fn construction_resource_labels_use_names_and_omit_zeroes() {
        assert_eq!(
            construction_cost_label(galactic_domain::ResourceCost::new(0, 315, 0)),
            "315 cristal"
        );
        assert_eq!(
            construction_missing_resources_label(
                ResourceStock::new(120, 96, 3),
                galactic_domain::ResourceCost::new(120, 300, 10),
            ),
            "204 cristal, 7 carburant"
        );

        let text =
            construction_error_text(galactic_sim::ConstructionError::InsufficientResources {
                available: ResourceStock::new(120, 96, 3),
                cost: galactic_domain::ResourceCost::new(120, 300, 10),
            });

        assert_eq!(text, "Manque: 204 cristal, 7 carburant");
        assert!(!text.contains("M0"));
        assert!(!text.contains("C0"));
        assert!(!text.contains("F0"));
    }

    #[test]
    fn energy_deficit_uses_a_full_warning_gauge() {
        assert_eq!(energy_fill_ratio(60, 40), 1.0);
        assert_eq!(energy_fill_ratio(0, 0), 0.0);
    }

    #[test]
    fn management_uses_the_persisted_active_colony() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        assert_eq!(
            active_management_colony(&simulation).map(|colony| colony.id),
            simulation
                .state()
                .active_player_colony()
                .map(|colony| colony.id),
        );
        assert!(colony_list_label(&simulation).contains("● C0 Port-Sillage"));
    }

    #[test]
    fn management_colony_cycle_wraps() {
        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut management = ColonyManagementState::default();
        let initial = simulation.simulation().state().active_colony_id;

        cycle_management_colony(&mut management, &mut simulation, false);

        assert_eq!(simulation.simulation().state().active_colony_id, initial);
    }

    #[test]
    fn management_colony_cycle_updates_the_shared_selection() {
        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut second = simulation.simulation().state().colonies[0].clone();
        second.id = galactic_domain::ColonyId::new(1);
        second.name = "Relais Boréal".to_string();
        simulation.simulation.state_mut().colonies.push(second);
        let mut management = ColonyManagementState::default();

        cycle_management_colony(&mut management, &mut simulation, false);

        assert_eq!(
            simulation.simulation().state().active_colony_id,
            Some(galactic_domain::ColonyId::new(1)),
        );
        assert!(colony_list_label(simulation.simulation()).contains("● C1 Relais Boréal"));
    }

    #[test]
    fn building_detail_uses_catalog_and_simulation_values() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists");
        let quote = galactic_sim::building_upgrade_quote(
            simulation.state(),
            simulation.state().player_faction,
            colony.id,
            galactic_sim::BuildingKind::METAL_MINE,
        );

        let text =
            building_management_detail_text(colony, galactic_sim::BuildingKind::METAL_MINE, quote);

        assert!(text.contains("FOSSE SIDÉRURGIQUE"));
        assert!(text.contains("Niveau actif"));
        assert!(text.contains("Coût"));
        assert!(text.contains("Actuel"));
    }

    #[test]
    fn queue_progress_is_clamped_and_empty_is_zero() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        assert_eq!(
            construction_progress_ratio(
                simulation.state().colony(colony_id).expect("colony exists"),
            ),
            0.0,
        );

        simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
            colony_id,
            kind: galactic_sim::BuildingKind::METAL_MINE,
        });
        let ratio = construction_progress_ratio(
            simulation.state().colony(colony_id).expect("colony exists"),
        );
        assert!((0.0..=1.0).contains(&ratio));
    }

    #[test]
    fn colony_management_screen_uses_global_overlay_layer() {
        fn spawn_management_for_test(mut commands: Commands) {
            spawn_colony_management_screen(&mut commands);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Startup, spawn_management_for_test);

        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&GlobalZIndex, With<ColonyManagementRoot>>();
        let z_index = query
            .single(app.world())
            .expect("management root should have a global z-index");

        assert_eq!(*z_index, GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX));
        assert!(z_index.0 > 0);
    }

    #[test]
    fn ui_font_uses_the_stable_default_asset() {
        assert!(matches!(
            ui_text_font(14.0).font,
            FontSource::Handle(handle) if handle == Handle::default()
        ));
    }

    #[test]
    fn changing_french_text_reuses_its_font_atlas() {
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<TextureAtlasLayout>>()
            .init_resource::<FontAtlasSet>()
            .init_resource::<TextPipeline>()
            .init_resource::<FontCx>()
            .init_resource::<LayoutCx>()
            .init_resource::<ScaleCx>()
            .init_resource::<TextIterScratch>()
            .init_resource::<RemSize>()
            .add_systems(
                Update,
                (detect_text_needs_rerender, update_text2d_layout).chain(),
            );

        let font_data = {
            let mut font_cx = app.world_mut().resource_mut::<FontCx>();
            stable_system_sans_serif_data(&mut font_cx)
                .unwrap_or_else(|| bevy::text::DEFAULT_FONT_DATA.to_vec())
        };
        app.world_mut()
            .resource_mut::<Assets<Font>>()
            .insert(AssetId::default(), Font::from_bytes(font_data))
            .expect("default font handle should be available");
        let stable_font_data = {
            let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
            let mut font = fonts
                .get_mut(AssetId::default())
                .expect("the stable font was just inserted");
            font.alias = "Galactic Stable Sans".into();
            font.data.clone()
        };
        app.world_mut()
            .resource_mut::<FontCx>()
            .collection
            .register_fonts(stable_font_data, None);

        let mut visible_entities = VisibleEntities::default();
        visible_entities.push(Entity::PLACEHOLDER, TypeId::of::<Sprite>());
        app.world_mut().spawn((
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::splat(1_000),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
            visible_entities,
        ));
        let text_entity = app
            .world_mut()
            .spawn((Text2d::new("Hélianthe 0"), ui_text_font(18.0)))
            .id();

        app.update();
        let initial_images = app.world().resource::<Assets<Image>>().len();
        assert!(initial_images > 0);

        for sample in 1..120 {
            app.world_mut()
                .entity_mut(text_entity)
                .get_mut::<Text2d>()
                .expect("text entity should still exist")
                .0 = format!("Hélianthe {}", sample % 10);
            app.update();
        }

        let final_images = app.world().resource::<Assets<Image>>().len();
        assert!(
            final_images <= initial_images + 1,
            "font atlas images grew from {initial_images} to {final_images}",
        );
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
        simulation.apply_player_action(GameAction::SelectSystem(detected));
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);

        let probed = system_inspector_content(&simulation, detected).render();
        assert!(probed.contains("SONDÉ"));
        assert!(probed.contains("Luminosité estimée"));

        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
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
        let system = simulation
            .universe()
            .system(system_id)
            .expect("detected planet system exists");
        let orbit_index = system
            .planets
            .iter()
            .position(|candidate| candidate.id == detected)
            .expect("detected planet belongs to its system");

        assert!(rendered.contains("DÉTECTÉ"));
        assert!(rendered.contains(&provisional_planet_label(&system.name, orbit_index)));
        assert!(rendered.contains("Identité : non déterminée"));
        assert!(rendered.contains("Habitabilité : ???"));
        assert!(!rendered.contains(&planet.name));
        assert!(!rendered.contains(&format!("{:?}", planet.kind)));
    }

    #[test]
    fn analyzed_planet_inspector_shows_exact_report_and_colonization_status() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, planet) = simulation
            .universe_repository()
            .planet_location(planet_id)
            .expect("detected planet exists");
        let planet_name = planet.name.clone();
        let habitability = planet.habitability;
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
            galactic_sim::TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.apply_player_action(GameAction::AnalyzePlanet { planet_id });

        let rendered = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert!(rendered.contains("ANALYSÉ"));
        assert!(rendered.contains(&planet_name));
        assert!(rendered.contains(&format!("Habitabilité exacte : {habitability}%")));
        assert!(rendered.contains("Rapport établi au tick"));
        assert!(rendered.contains("POTENTIEL EXACT"));
        assert!(rendered.contains("COLONISABILITÉ — BLOQUÉE"));
        assert!(rendered.contains("SITE D'EXTRACTION"));
        assert!(rendered.contains("Prospection autonome requise"));
        assert!(!rendered.contains("Potentiel : analyse requise"));
    }

    #[test]
    fn planetary_intelligence_progresses_without_leaking_real_forces() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let presence = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                presence.occupant != galactic_domain::Owner::Unowned
                    && !presence.forces.is_empty()
                    && simulation
                        .state()
                        .colony_on_planet(presence.planet_id)
                        .is_none()
            })
            .expect("the MVP seed contains an occupied remote planet")
            .clone();
        let planet_id = presence.planet_id;
        let system_id = planet_id.system_id();
        let occupant = presence
            .occupant
            .faction()
            .expect("selected presence is occupied");
        let occupant_name = simulation
            .state()
            .faction(occupant)
            .expect("occupant exists")
            .name
            .clone();
        let force_names = presence
            .forces
            .iter()
            .map(|force| {
                default_ruleset()
                    .planetary_presence()
                    .definition(force.definition_id)
                    .expect("force definition exists")
                    .name
            })
            .collect::<Vec<_>>();
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            planet_id,
            KnowledgeLevel::Probed,
        );
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            planet_id,
            PlanetaryIntelPrecision::Contact,
            galactic_sim::StrategicTick::ZERO,
        )
        .expect("contact report");

        let contact = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert!(contact.contains("RENSEIGNEMENT PLANÉTAIRE — CONTACT"));
        assert!(contact.contains("identité inconnue"));
        assert!(!contact.contains(&occupant_name));
        assert!(force_names.iter().all(|name| !contact.contains(name)));
        assert!(!contact.contains("RAPPORT DE COMBAT"));
        assert_eq!(selected_attack_context(&simulation), None);

        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
            galactic_sim::TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.apply_player_action(GameAction::AnalyzePlanet { planet_id });
        let report = simulation
            .state()
            .planetary_intelligence_report(planet_id)
            .expect("analysis refreshes intelligence");
        let surveyed = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert_eq!(report.precision, PlanetaryIntelPrecision::Surveyed);
        assert!(surveyed.contains("RENSEIGNEMENT PLANÉTAIRE — ESTIMATION"));
        assert!(surveyed.contains(&occupant_name));
        assert!(report.forces.iter().all(|force| {
            !force.quantity.is_exact() && surveyed.contains(&estimate_range_text(force.quantity))
        }));
        assert!(!surveyed.contains("DONNÉES LOCALES"));
        assert!(!surveyed.contains("RAPPORT DE COMBAT"));
        assert_eq!(
            selected_attack_context(&simulation),
            Some((
                simulation
                    .state()
                    .player_home_colony()
                    .expect("the player home colony exists")
                    .id,
                MissionTarget::Planet {
                    system_id,
                    planet_id,
                },
            ))
        );
    }

    #[test]
    fn semantic_lod_uses_stable_distance_bands() {
        assert_eq!(UniverseLod::from_distance(120.0), UniverseLod::Overview);
        assert_eq!(UniverseLod::from_distance(64.0), UniverseLod::Regional);
        assert_eq!(UniverseLod::from_distance(32.0), UniverseLod::Local);
    }

    #[test]
    fn initial_observation_is_bounded_and_does_not_reveal_gameplay_knowledge() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let routes_before = simulation
            .state()
            .visible_routes(simulation.universe_repository())
            .len();
        let entries = systems_for_universe_view(&simulation, false);
        let observed = entries
            .iter()
            .filter(|entry| entry.tier == UniverseSystemTier::Observed)
            .collect::<Vec<_>>();

        assert!(entries.len() < simulation.universe().systems.len());
        assert!(entries.len() >= INITIAL_OBSERVATION_SYSTEM_LIMIT);
        assert!(!observed.is_empty());
        assert!(observed.iter().all(|entry| {
            simulation.state().system_knowledge_level(entry.id) == KnowledgeLevel::Unknown
        }));
        assert_eq!(
            simulation
                .state()
                .visible_routes(simulation.universe_repository())
                .len(),
            routes_before
        );
    }

    #[test]
    fn selection_cycle_skips_observed_signals_outside_debug_mode() {
        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };

        for _ in 0..24 {
            cycle_visible_selection(&mut simulation, false);
            let selected = selected_system(simulation.simulation().state().selected)
                .expect("the normal cycle selects a system");
            assert!(
                simulation
                    .simulation()
                    .state()
                    .system_knowledge_level(selected)
                    .is_visible()
            );
        }
    }

    #[test]
    fn dashed_routes_use_multiple_separated_segments() {
        assert_eq!(dashed_segment_count(0.0, 1.0, 1.0), 0);
        assert_eq!(dashed_segment_count(0.5, 1.0, 1.0), 1);
        assert_eq!(dashed_segment_count(10.0, 1.5, 0.5), 5);
    }

    #[test]
    fn procedural_planet_textures_are_small_varied_and_deterministic() {
        for kind in PlanetKind::ALL {
            let first = procedural_planet_texture(kind);
            let second = procedural_planet_texture(kind);
            let first_data = first.data.expect("generated texture keeps its source data");
            let second_data = second
                .data
                .expect("generated texture keeps its source data");
            let colors = first_data
                .chunks_exact(4)
                .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
                .collect::<HashSet<_>>();

            assert_eq!(
                first_data.len(),
                (PLANET_TEXTURE_WIDTH * PLANET_TEXTURE_HEIGHT * 4) as usize
            );
            assert_eq!(first_data, second_data);
            assert!(colors.len() >= 3, "{kind:?} texture is not varied");
        }
    }

    #[test]
    fn visual_orbit_is_slow_and_preserves_radius_and_label_height() {
        let orbit = planet_orbit(2, 15.6, 1.35);
        let start = orbit.translation_at(0.0);
        let later = orbit.translation_at(10.0);

        assert!((Vec2::new(start.x, start.z).length() - 15.6).abs() < 0.001);
        assert!((Vec2::new(later.x, later.z).length() - 15.6).abs() < 0.001);
        assert_eq!(start.y, 1.35);
        assert_eq!(later.y, 1.35);
        assert!(start.distance(later) < 1.5);
    }

    #[test]
    fn detected_planets_receive_stable_orbital_designations() {
        assert_eq!(
            provisional_planet_label("Port-Sillage", 0),
            "Port-Sillage I"
        );
        assert_eq!(
            provisional_planet_label("Port-Sillage", 7),
            "Port-Sillage VIII"
        );
        assert_eq!(
            provisional_planet_label("Port-Sillage", 12),
            "Port-Sillage 13"
        );
    }

    #[test]
    fn mission_marker_progresses_outward_then_returns() {
        let mut mission = galactic_sim::MissionState {
            id: galactic_domain::MissionId::new(0),
            owner: galactic_domain::Owner::Faction(galactic_domain::FactionId::new(0)),
            order: galactic_sim::MissionOrder {
                fleet_id: galactic_domain::FleetId::new(0),
                origin: SystemId::new(0),
                target: MissionTarget::System(SystemId::new(1)),
                kind: galactic_sim::MissionKind::Probe,
                departure_at: galactic_sim::StrategicTick::new(10),
            },
            origin_colony_id: galactic_domain::ColonyId::new(0),
            plan: galactic_sim::MissionPlan {
                route: vec![SystemId::new(0), SystemId::new(1)],
                hops: 1,
                travel_duration: galactic_sim::StrategicDuration::from_ticks(10),
                resolution_duration: galactic_sim::StrategicDuration::from_ticks(1),
                fuel_cost: galactic_domain::ResourceCost::new(0, 0, 2),
                outbound_arrival_at: galactic_sim::StrategicTick::new(20),
                return_departure_at: galactic_sim::StrategicTick::new(30),
                return_arrival_at: galactic_sim::StrategicTick::new(40),
            },
            phase: MissionPhase::Outbound,
            phase_started_at: galactic_sim::StrategicTick::new(10),
            fuel_reservation: None,
            foundation_reservation: None,
            cargo_reservation: None,
            attack: None,
            colonization: None,
            transport: None,
            harvest: None,
            result: None,
        };

        assert_eq!(
            mission_route_progress(&mission, galactic_sim::StrategicTick::new(15)),
            Some(0.5)
        );
        mission.phase = MissionPhase::Returning;
        assert_eq!(
            mission_route_progress(&mission, galactic_sim::StrategicTick::new(35)),
            Some(0.5)
        );
        mission.phase = MissionPhase::Completed;
        assert_eq!(
            mission_route_progress(&mission, galactic_sim::StrategicTick::new(40)),
            None
        );
    }

    #[test]
    fn overview_sector_labels_only_use_known_members() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let labels = known_sector_labels(&simulation);
        let home_sector = simulation
            .universe_repository()
            .sector_for_system(MVP_HOME_SYSTEM_ID)
            .expect("home system belongs to a sector");
        let hidden_names = simulation
            .universe()
            .sectors
            .iter()
            .filter(|sector| sector.id != home_sector.id)
            .map(|sector| sector.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, home_sector.name);
        assert_eq!(labels[0].position, WorldPosition::ZERO);
        assert!(
            hidden_names
                .iter()
                .all(|name| labels.iter().all(|label| label.text != *name))
        );
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
    fn projection_shortcut_uses_p() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyP);

        assert_eq!(view_shortcut(&keyboard), Some(UiAction::ToggleProjection));
    }

    #[test]
    fn reconnaissance_shortcut_uses_k() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyK);

        assert_eq!(simulation_shortcut(&keyboard), Some(UiAction::LaunchProbe));
    }

    #[test]
    fn planetary_analysis_shortcut_uses_l() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyL);

        assert_eq!(
            simulation_shortcut(&keyboard),
            Some(UiAction::AnalyzePlanet)
        );
    }

    #[test]
    fn attack_shortcut_uses_m() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyM);

        assert_eq!(simulation_shortcut(&keyboard), Some(UiAction::LaunchAttack));
    }

    #[test]
    fn harvest_shortcut_uses_h() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyH);

        assert_eq!(
            simulation_shortcut(&keyboard),
            Some(UiAction::LaunchHarvest)
        );
    }

    #[test]
    fn colonization_shortcut_uses_n() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyN);

        assert_eq!(
            simulation_shortcut(&keyboard),
            Some(UiAction::LaunchColonization),
        );
    }

    #[test]
    fn colony_creation_event_is_player_facing() {
        let label = event_label(GameEvent::new(
            galactic_domain::FactionId::new(0),
            galactic_sim::StrategicTick::new(420),
            GameEventKind::ColonyEstablished(galactic_sim::ColonyEstablished {
                colony_id: galactic_domain::ColonyId::new(1),
                mission_id: galactic_domain::MissionId::new(3),
                owner: galactic_domain::FactionId::new(0),
                system_id: SystemId::new(4),
                planet_id: PlanetId::from_system_index(SystemId::new(4), 2),
                established_at: galactic_sim::StrategicTick::new(420),
            }),
        ));

        assert!(label.contains("nouvelle colonie"));
        assert!(label.contains("colonie 1 opérationnelle"));
    }

    #[test]
    fn active_probe_mission_is_visible_in_the_hud() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = simulation.universe_repository().neighboring_systems(origin)[0];
        let colony = &mut simulation.state_mut().colonies[0];
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::CONSTRUCTION_CENTER, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::METAL_MINE, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::CRYSTAL_EXTRACTOR, 2);
        colony
            .buildings
            .set_level(galactic_sim::BuildingKind::SHIPYARD, 1);
        colony.energy =
            galactic_sim::default_building_catalog().energy_grid_for_levels(colony.buildings);
        colony
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test funding fits");
        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
        ]);
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: galactic_sim::CraftableId::LIGHT_PROBE,
        });
        simulation.advance(Duration::from_secs(50));
        simulation.apply_player_action(GameAction::LaunchProbe {
            colony_id,
            target: MissionTarget::System(target),
        });
        simulation.advance(Duration::from_secs(1));

        let status = mission_status_line(&simulation);

        assert!(status.contains("Reconnaissance"));
        assert!(status.contains("Signal"));
        assert!(status.contains("transit aller"));
    }

    #[test]
    fn missing_probe_has_a_player_facing_error() {
        let message = mission_error_text(galactic_sim::MissionError::ProbeUnavailable(
            galactic_domain::ColonyId::new(0),
        ));

        assert!(message.contains("Sonde Luciole"));
        assert!(message.contains("chantier orbital"));
        assert!(!message.contains("ProbeUnavailable"));
    }

    #[test]
    fn reconnaissance_result_announces_the_new_frontier() {
        let label = event_label(GameEvent::new(
            galactic_domain::FactionId::new(0),
            galactic_sim::StrategicTick::new(100),
            GameEventKind::MissionResolved(galactic_sim::MissionResolution {
                mission_id: galactic_domain::MissionId::new(0),
                result: MissionResult::Probe(galactic_sim::ProbeMissionResult {
                    target: MissionTarget::System(SystemId::new(4)),
                    previous: KnowledgeLevel::Detected,
                    current: KnowledgeLevel::Probed,
                    revealed_systems: 3,
                    newly_detected_systems: 2,
                    revealed_routes: 2,
                    revealed_planets: 4,
                }),
                occurred_at: galactic_sim::StrategicTick::new(100),
            }),
        ));

        assert!(label.contains("2 nouveaux signaux"));
        assert!(label.contains("2 routes"));
        assert!(label.contains("4 planètes"));
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
        apply_simulation_command(&mut simulation, GameAction::SelectSystem(neighbor));

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
        assert!(rendered.contains("Port-Sillage"));
        assert!(rendered.contains("ÉCONOMIE — RÉSUMÉ"));
        assert!(rendered.contains("Gestion complète : touche C"));
        assert!(rendered.contains("INFRASTRUCTURE"));
    }
}

mod benchmark;
mod combat_ui;
mod craft_ui;
mod fleet_ui;
mod graphics_settings_ui;
mod mission_wizard;
mod navigation_ui;
mod notifications_ui;
mod objectives_ui;
mod presentation;
mod research_ui;
mod save_load_ui;

use benchmark::{
    BenchmarkConfig, BenchmarkResolution, drive_benchmark_sequence, graphics_preset_from_slug,
    sample_benchmark_metrics,
};
use combat_ui::CombatUiPlugin;
use craft_ui::CraftUiPlugin;
use fleet_ui::FleetUiPlugin;
use graphics_settings_ui::GraphicsSettingsUiPlugin;
use mission_wizard::MissionWizardPlugin;
use navigation_ui::NavigationUiPlugin;
use notifications_ui::NotificationsUiPlugin;
use objectives_ui::ObjectivesUiPlugin;
use presentation::colony_management_ui::{
    capture_colony_management_feedback, cycle_management_colony, handle_colony_management_buttons,
    update_action_buttons, update_colony_management_buildings, update_colony_management_detail,
    update_colony_management_queue, update_colony_management_resources,
    update_colony_management_visibility,
};
use presentation::components::{
    ColonyManagementState, DebugOverlayState, HelpUiState, InspectorContent, InspectorPanelRoot,
    InspectorSection, InspectorTabBarRoot, InspectorTabButton, InspectorTabButtonQuery,
    InspectorTabLabelQuery, InspectorTabState, InspectorTextQuery, InspectorTextRole,
    IntroPitchUiState, OpenPanel, PointerSelectionState, SelectedMission, StrategicViewEntity,
    TopBarText, UiPointerBlocker, VictoryUiState,
};
use presentation::icons::IconAssets;
use presentation::input::{
    handle_action_buttons, handle_pointer_selection, handle_simulation_input, handle_view_input,
    provisional_planet_label, toggle_debug_overlay, update_ambiguity_panel,
    update_debug_overlay_visibility, update_pointer_candidates, update_pointer_halos,
    update_pointer_tooltip,
};
use presentation::inspector_panel::{
    combat_report_text, format_strategic_duration, handle_inspector_tab_buttons,
    mission_error_text, mission_kind_label, mission_next_deadline, mission_phase_label_for_kind,
    mission_result_text, mission_target_label, update_info_panel, update_ui,
};
use presentation::overlays::{
    FleetTrailSpawnTimer, advance_fleet_trail_particles, compute_label_budget,
    draw_strategic_overlays, spawn_fleet_trail_particles, update_orbiting_visuals,
    update_planet_spins, update_pointer_halo_positions, update_projection_transition,
    update_sector_labels, update_system_labels, update_system_visuals,
};
use presentation::procedural_materials::{
    atmosphere_material, planet_material, procedural_planet_texture, star_halo_material,
    star_material, territory_tint_material,
};
use presentation::scene::{
    accent_craft_amber, accent_fleet_blue, accent_research_violet, action_button_color,
    action_button_outline, handle_help_toggle_button, handle_intro_pitch_buttons,
    handle_scroll_areas, handle_tab_bar_galaxy_button, handle_victory_modal_buttons,
    panel_background, panel_outline, rebuild_strategic_view_if_requested, spawn_scene,
    spawn_strategic_view, spawn_ui, ui_text_font, update_camera_graphics_preset,
    update_help_visibility, update_intro_pitch_visibility, update_resource_bar,
    update_scroll_indicators, update_sun_light_preset, update_victory_modal, update_victory_state,
    update_window_resolution_preset,
};
use presentation::shortcuts::{apply_simulation_command, apply_ui_action, replace_simulation};
use presentation::strategic_camera::{tick_simulation, update_strategic_camera};
use presentation::strategic_navigation::{
    BreadcrumbKind, NavigationHistory, StrategicNavigation, ViewRebuildRequest,
    breadcrumb_segments, navigate_to_galaxy, navigate_to_sector, navigate_to_selection,
};
use presentation::universe_labels::LabelBudgetState;
use research_ui::ResearchUiPlugin;
use save_load_ui::SaveLoadUiPlugin;
use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::{
    RenderPlugin,
    settings::{MemoryHints, RenderCreation, WgpuSettings},
};
use bevy::text::{FontAtlasSet, FontCx, FontSource};
use bevy::window::PresentMode;
use galactic_domain::{PlanetKind, StarClass, SystemId, UniverseConfig, UniverseScalePreset};
use galactic_sim::{GameEvent, GameEventKind, Simulation, SystemVisibility};

#[cfg(test)]
use galactic_domain::{PlanetId, ResourceStock, WorldPosition};
#[cfg(test)]
use galactic_sim::{
    GameAction, KnowledgeLevel, MVP_HOME_SYSTEM_ID, MissionPhase, MissionResult, MissionTarget,
    SelectionTarget,
};
#[cfg(test)]
use presentation::colony_management_ui::{active_management_colony, colony_list_label};
#[cfg(test)]
use presentation::components::{
    AmbiguitySelection, ColonyManagementRoot, PickTarget, PointerCandidate, PointerClickRecord,
    UiAction,
};
#[cfg(test)]
use presentation::input::{
    pick_target_is_visible, pick_target_label, pointer_double_click, rank_pointer_candidates,
    screen_space_hit,
};
#[cfg(test)]
use presentation::inspector_panel::mission_status_line;
#[cfg(test)]
use presentation::overlays::{
    advance_projection_mix, dashed_segment_count, mission_route_progress,
};
#[cfg(test)]
use presentation::procedural_materials::{event_label, selection_label};
#[cfg(test)]
use presentation::scene::{
    known_sector_labels, planet_orbit, spawn_colony_management_screen, systems_for_universe_view,
};
#[cfg(test)]
use presentation::shortcuts::{
    action_available, cycle_visible_selection, selected_attack_context, selected_system,
    simulation_shortcut, view_shortcut,
};
#[cfg(test)]
use presentation::strategic_camera::{apply_orbit_drag, apply_scroll_zoom, keyboard_pan_direction};
#[cfg(test)]
use presentation::strategic_navigation::{
    HistoryDirection, StrategicViewMode, UniverseLod, history_shortcut, navigate_history,
    projected_universe_position,
};
#[cfg(test)]
use std::time::Duration;

pub(crate) const UNIVERSE_VERTICAL_EXAGGERATION: f32 = 3.4;
const INITIAL_OBSERVATION_SYSTEM_LIMIT: usize = 14;

pub fn run() {
    let (scale_preset, benchmark) = match parse_launch_args(std::env::args().skip(1)) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return;
        }
    };
    let mut plugin = ClientPlugin::new(scale_preset);
    if let Some(config) = benchmark {
        plugin = plugin.with_benchmark(config);
    }
    App::new().add_plugins(plugin).run();
}

pub struct ClientPlugin {
    scale_preset: UniverseScalePreset,
    benchmark: Option<BenchmarkConfig>,
}

impl ClientPlugin {
    pub const fn new(scale_preset: UniverseScalePreset) -> Self {
        Self {
            scale_preset,
            benchmark: None,
        }
    }

    pub(crate) fn with_benchmark(mut self, config: BenchmarkConfig) -> Self {
        self.benchmark = Some(config);
        self
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
        let graphics_settings = presentation::graphics_settings::GraphicsSettings {
            preset: galactic_persistence::load_settings(
                &galactic_persistence::default_settings_path(),
            ),
        };

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
        .insert_resource(graphics_settings)
        .init_resource::<PresentationLog>()
        .init_resource::<VisualAssets>()
        .init_resource::<IconAssets>()
        .insert_resource(navigation)
        .init_resource::<ViewRebuildRequest>()
        .init_resource::<NavigationHistory>()
        .init_resource::<LabelBudgetState>()
        .init_resource::<SelectedMission>()
        .init_resource::<PointerSelectionState>()
        .init_resource::<ColonyManagementState>()
        .init_resource::<OpenPanel>()
        .init_resource::<MemoryDiagnostics>()
        .init_resource::<DebugOverlayState>()
        .init_resource::<HelpUiState>()
        .init_resource::<IntroPitchUiState>()
        .init_resource::<VictoryUiState>()
        .init_resource::<InspectorTabState>()
        .init_resource::<FleetTrailSpawnTimer>();

        if let Some(config) = self.benchmark.clone() {
            app.insert_resource(benchmark::BenchmarkState::new(config));
        }

        app.add_plugins(SimulationBridgePlugin)
            .add_plugins(PresentationPlugin)
            .add_plugins(ResearchUiPlugin)
            .add_plugins(CraftUiPlugin)
            .add_plugins(FleetUiPlugin)
            .add_plugins(MissionWizardPlugin)
            .add_plugins(NavigationUiPlugin)
            .add_plugins(ObjectivesUiPlugin)
            .add_plugins(SaveLoadUiPlugin)
            .add_plugins(GraphicsSettingsUiPlugin)
            .add_plugins(CombatUiPlugin)
            .add_plugins(NotificationsUiPlugin)
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

/// Returns `Some(value)` and consumes it from `args` if `argument` is either
/// `flag` (value taken from the next argument) or `flag=value`. Returns
/// `None` (leaving `args` untouched) if `argument` doesn't match `flag` at
/// all.
fn take_flag_value(
    argument: &str,
    flag: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<Option<String>, String> {
    if argument == flag {
        let value = args
            .next()
            .ok_or_else(|| format!("Option {flag} sans valeur."))?;
        Ok(Some(value))
    } else if let Some(value) = argument.strip_prefix(&format!("{flag}=")) {
        Ok(Some(value.to_string()))
    } else {
        Ok(None)
    }
}

fn parse_launch_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(UniverseScalePreset, Option<BenchmarkConfig>), String> {
    let mut args = args.into_iter();
    let mut preset = UniverseScalePreset::default();
    let mut benchmark_enabled = false;
    let mut benchmark_resolution = None;
    let mut benchmark_preset = None;
    let mut benchmark_export_dir = None;

    while let Some(argument) = args.next() {
        if let Some(value) = take_flag_value(&argument, "--scale", &mut args)? {
            preset = UniverseScalePreset::from_slug(&value).ok_or_else(|| {
                format!("Preset inconnu « {value} ». Valeurs acceptées : test, mvp, stress.")
            })?;
        } else if argument == "--benchmark" {
            benchmark_enabled = true;
        } else if let Some(value) = take_flag_value(&argument, "--benchmark-resolution", &mut args)?
        {
            benchmark_resolution = Some(BenchmarkResolution::from_slug(&value).ok_or_else(|| {
                format!("Résolution de benchmark inconnue « {value} ». Valeurs acceptées : 720p, 1080p.")
            })?);
        } else if let Some(value) = take_flag_value(&argument, "--benchmark-preset", &mut args)? {
            benchmark_preset = Some(graphics_preset_from_slug(&value).ok_or_else(|| {
                format!(
                    "Preset de benchmark inconnu « {value} ». Valeurs acceptées : low, medium, high."
                )
            })?);
        } else if let Some(value) = take_flag_value(&argument, "--benchmark-export", &mut args)? {
            benchmark_export_dir = Some(std::path::PathBuf::from(value));
        } else {
            return Err(format!(
                "Option inconnue « {argument} ». Utiliser --scale test|mvp|stress, --benchmark, \
                 --benchmark-resolution 720p|1080p, --benchmark-preset low|medium|high, \
                 --benchmark-export <dossier>."
            ));
        }
    }

    let benchmark = benchmark_enabled.then(|| {
        let mut config = BenchmarkConfig::default();
        if let Some(resolution) = benchmark_resolution {
            config.resolutions = vec![resolution];
        }
        if let Some(preset) = benchmark_preset {
            config.presets = vec![preset];
        }
        if let Some(export_dir) = benchmark_export_dir {
            config.export_dir = export_dir;
        }
        config
    });

    Ok((preset, benchmark))
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
                (
                    rebuild_strategic_view_if_requested,
                    update_camera_graphics_preset,
                    update_sun_light_preset,
                    update_window_resolution_preset,
                    update_planet_texture_quality,
                    spawn_fleet_trail_particles,
                    advance_fleet_trail_particles,
                    update_projection_transition,
                    update_system_visuals,
                    update_orbiting_visuals,
                    update_planet_spins,
                )
                    .chain(),
                (
                    compute_label_budget,
                    update_system_labels,
                    update_sector_labels,
                    update_pointer_halo_positions,
                    sample_benchmark_metrics,
                    drive_benchmark_sequence,
                    update_strategic_camera,
                    update_pointer_candidates,
                    handle_pointer_selection,
                    capture_colony_management_feedback,
                    collect_presentation_events,
                    update_pointer_halos,
                    draw_strategic_overlays,
                )
                    .chain(),
            )
                .chain()
                .in_set(PresentationUpdateSet::View),
        )
        .add_systems(
            Update,
            (
                handle_action_buttons,
                handle_colony_management_buttons,
                handle_tab_bar_galaxy_button,
                handle_help_toggle_button,
                handle_intro_pitch_buttons,
                handle_victory_modal_buttons,
                handle_inspector_tab_buttons,
                toggle_debug_overlay,
            )
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
            )
                .chain()
                .in_set(PresentationUpdateSet::Management),
        )
        .add_systems(
            Update,
            (
                update_resource_bar,
                update_ui,
                update_victory_state,
                handle_scroll_areas,
                update_scroll_indicators,
                update_help_visibility,
                update_intro_pitch_visibility,
                update_victory_modal,
                update_info_panel,
                update_debug_overlay_visibility,
            )
                .chain()
                .in_set(PresentationUpdateSet::Ui),
        );
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PresentationUpdateSet {
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
pub(crate) struct PresentationLog {
    last_event: Option<GameEvent>,
}

#[derive(Resource)]
pub(crate) struct MemoryDiagnostics {
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
pub(crate) struct ProcessMemorySnapshot {
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

#[derive(Resource)]
pub(crate) struct VisualAssets {
    system_mesh: Handle<Mesh>,
    planet_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    colony_ring_mesh: Handle<Mesh>,
    pub(crate) particle_mesh: Handle<Mesh>,
    pub(crate) particle_material: Handle<StandardMaterial>,
    known_star_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    star_halo_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    detected_material: Handle<StandardMaterial>,
    observed_material: Handle<StandardMaterial>,
    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
    planet_textures: HashMap<PlanetKind, Handle<Image>>,
    atmosphere_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
    ring_material: Handle<StandardMaterial>,
    hover_material: Handle<StandardMaterial>,
    territory_materials: HashMap<presentation::territory::TerritoryTint, Handle<StandardMaterial>>,
}

impl FromWorld for VisualAssets {
    fn from_world(world: &mut World) -> Self {
        // Low preset: every geometry and material is shared by all matching bodies.
        let (system_mesh, planet_mesh, ring_mesh, colony_ring_mesh, particle_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Sphere::default().mesh().ico(1).unwrap()),
                meshes.add(Sphere::default().mesh().uv(32, 18)),
                meshes.add(Annulus::new(WIDE_RING_INNER_RADIUS, WIDE_RING_OUTER_RADIUS)),
                meshes.add(Annulus::new(
                    COLONY_RING_INNER_RADIUS,
                    COLONY_RING_OUTER_RADIUS,
                )),
                meshes.add(Sphere::new(0.5).mesh().ico(0).unwrap()),
            )
        };
        let preset = world
            .resource::<presentation::graphics_settings::GraphicsSettings>()
            .preset;
        let planet_textures = {
            let mut images = world.resource_mut::<Assets<Image>>();
            PlanetKind::ALL
                .into_iter()
                .map(|kind| (kind, images.add(procedural_planet_texture(kind, preset))))
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
        let territory_materials = presentation::territory::TerritoryTint::ALL
            .into_iter()
            .map(|tint| (tint, materials.add(territory_tint_material(tint))))
            .collect();
        let particle_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.84, 1.0, 0.85),
            emissive: LinearRgba::rgb(0.62, 0.94, 1.4),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        Self {
            system_mesh,
            planet_mesh,
            ring_mesh,
            colony_ring_mesh,
            particle_mesh,
            particle_material,
            known_star_materials,
            star_halo_materials,
            detected_material,
            observed_material,
            planet_materials,
            planet_textures,
            atmosphere_materials,
            ring_material,
            hover_material,
            territory_materials,
        }
    }
}

/// Regenerates the shared per-`PlanetKind` textures in place (via their
/// existing `Handle<Image>`) on a preset change — every already-spawned
/// planet mesh references these same handles through its material, so this
/// alone is enough to update every visible planet's texture resolution with
/// no despawn/respawn needed.
fn update_planet_texture_quality(
    graphics: Res<presentation::graphics_settings::GraphicsSettings>,
    visual_assets: Res<VisualAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    if !graphics.is_changed() {
        return;
    }
    for (kind, handle) in &visual_assets.planet_textures {
        if let Some(mut image) = images.get_mut(handle) {
            *image = procedural_planet_texture(*kind, graphics.preset);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UniverseSystemTier {
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
pub(crate) struct UniverseSystemEntry {
    id: SystemId,
    tier: UniverseSystemTier,
}

const COLONY_MANAGEMENT_Z_INDEX: i32 = 100;
const WIDE_RING_INNER_RADIUS: f32 = 1.55;
const WIDE_RING_OUTER_RADIUS: f32 = 2.25;
const COLONY_RING_INNER_RADIUS: f32 = 1.72;
const COLONY_RING_OUTER_RADIUS: f32 = 1.94;

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
            parse_launch_args(Vec::<String>::new()).map(|(preset, _)| preset),
            Ok(UniverseScalePreset::Mvp),
        );
        assert_eq!(
            parse_launch_args(["--scale", "test"].map(str::to_string)).map(|(preset, _)| preset),
            Ok(UniverseScalePreset::Test),
        );
        assert_eq!(
            parse_launch_args(["--scale=stress"].map(str::to_string)).map(|(preset, _)| preset),
            Ok(UniverseScalePreset::Stress),
        );
        assert!(parse_launch_args(["--scale=huge"].map(str::to_string)).is_err());
    }

    #[test]
    fn benchmark_flag_absent_means_no_benchmark_config() {
        let (_, benchmark) = parse_launch_args(Vec::<String>::new()).expect("parses");
        assert!(benchmark.is_none());
    }

    #[test]
    fn benchmark_flag_alone_runs_the_full_matrix() {
        let (_, benchmark) =
            parse_launch_args(["--benchmark"].map(str::to_string)).expect("parses");
        let config = benchmark.expect("benchmark enabled");
        assert_eq!(config.resolutions.len(), 2);
        assert_eq!(config.presets.len(), 3);
    }

    #[test]
    fn benchmark_resolution_and_preset_flags_restrict_the_matrix() {
        let (_, benchmark) = parse_launch_args(
            [
                "--benchmark",
                "--benchmark-resolution",
                "720p",
                "--benchmark-preset=low",
            ]
            .map(str::to_string),
        )
        .expect("parses");
        let config = benchmark.expect("benchmark enabled");
        assert_eq!(config.resolutions, vec![BenchmarkResolution::Hd720]);
        assert_eq!(
            config.presets,
            vec![galactic_persistence::GraphicsPreset::Low]
        );
    }

    #[test]
    fn benchmark_export_flag_overrides_the_output_directory() {
        let (_, benchmark) = parse_launch_args(
            ["--benchmark", "--benchmark-export", "/tmp/my-benchmarks"].map(str::to_string),
        )
        .expect("parses");
        let config = benchmark.expect("benchmark enabled");
        assert_eq!(
            config.export_dir,
            std::path::PathBuf::from("/tmp/my-benchmarks")
        );
    }

    #[test]
    fn unknown_benchmark_resolution_is_rejected() {
        assert!(parse_launch_args(["--benchmark-resolution=4k"].map(str::to_string)).is_err());
    }

    #[test]
    fn unknown_benchmark_preset_is_rejected() {
        assert!(parse_launch_args(["--benchmark-preset=ultra"].map(str::to_string)).is_err());
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse_launch_args(["--nope"].map(str::to_string)).is_err());
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
    fn mvp_scale_stays_inside_the_render_budget() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let universe = simulation.universe();

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
    fn management_uses_the_persisted_active_colony() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        assert_eq!(
            active_management_colony(&simulation).map(|colony| colony.id),
            simulation
                .state()
                .active_player_colony()
                .map(|colony| colony.id),
        );
        assert!(colony_list_label(&simulation).contains("* C0 Port-Sillage"));
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
        assert!(colony_list_label(simulation.simulation()).contains("* C1 Relais Boréal"));
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
    fn colony_ring_is_thinner_than_wide_visual_ring() {
        let wide_width = WIDE_RING_OUTER_RADIUS - WIDE_RING_INNER_RADIUS;
        let colony_width = COLONY_RING_OUTER_RADIUS - COLONY_RING_INNER_RADIUS;

        assert!(colony_width < wide_width / 3.0);
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
        use presentation::graphics_settings::GraphicsPreset;
        use presentation::procedural_materials::planet_texture_dimensions;

        for kind in PlanetKind::ALL {
            let first = procedural_planet_texture(kind, GraphicsPreset::Medium);
            let second = procedural_planet_texture(kind, GraphicsPreset::Medium);
            let first_data = first.data.expect("generated texture keeps its source data");
            let second_data = second
                .data
                .expect("generated texture keeps its source data");
            let colors = first_data
                .chunks_exact(4)
                .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
                .collect::<HashSet<_>>();

            let (width, height) = planet_texture_dimensions(GraphicsPreset::Medium);
            assert_eq!(first_data.len(), (width * height * 4) as usize);
            assert_eq!(first_data, second_data);
            assert!(colors.len() >= 3, "{kind:?} texture is not varied");
        }
    }

    #[test]
    fn low_preset_textures_are_smaller_than_high_preset_textures() {
        use presentation::graphics_settings::GraphicsPreset;
        use presentation::procedural_materials::planet_texture_dimensions;

        let (low_width, low_height) = planet_texture_dimensions(GraphicsPreset::Low);
        let (high_width, high_height) = planet_texture_dimensions(GraphicsPreset::High);

        assert!(low_width < high_width);
        assert!(low_height < high_height);
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
    fn navigation_history_round_trip_restores_view_and_selection() {
        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut navigation = StrategicNavigation {
            mode: StrategicViewMode::Universe,
            universe_focus: Vec3::new(4.0, 0.0, -2.0),
            universe_distance: 70.0,
            ..default()
        };
        let mut history = NavigationHistory::default();
        let mut rebuild = ViewRebuildRequest::default();

        apply_simulation_command(
            &mut simulation,
            GameAction::SelectSystem(MVP_HOME_SYSTEM_ID),
        );
        let start_focus = navigation.universe_focus;
        let start_distance = navigation.universe_distance;
        let start_selected = simulation.simulation.state().selected;

        apply_ui_action(
            UiAction::EnterSystem,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
            &mut history,
        );
        assert_eq!(
            navigation.mode,
            StrategicViewMode::System(MVP_HOME_SYSTEM_ID)
        );

        apply_ui_action(
            UiAction::ExitSystem,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
            &mut history,
        );
        assert_eq!(navigation.mode, StrategicViewMode::Universe);

        navigate_history(
            HistoryDirection::Back,
            &mut simulation,
            &mut navigation,
            &mut history,
            &mut rebuild,
        );
        assert_eq!(
            navigation.mode,
            StrategicViewMode::System(MVP_HOME_SYSTEM_ID)
        );

        navigate_history(
            HistoryDirection::Back,
            &mut simulation,
            &mut navigation,
            &mut history,
            &mut rebuild,
        );
        assert_eq!(navigation.mode, StrategicViewMode::Universe);
        assert_eq!(navigation.universe_focus, start_focus);
        assert_eq!(navigation.universe_distance, start_distance);
        assert_eq!(simulation.simulation.state().selected, start_selected);

        navigate_history(
            HistoryDirection::Forward,
            &mut simulation,
            &mut navigation,
            &mut history,
            &mut rebuild,
        );
        assert_eq!(
            navigation.mode,
            StrategicViewMode::System(MVP_HOME_SYSTEM_ID)
        );

        navigate_history(
            HistoryDirection::Forward,
            &mut simulation,
            &mut navigation,
            &mut history,
            &mut rebuild,
        );
        assert_eq!(navigation.mode, StrategicViewMode::Universe);
    }

    #[test]
    fn history_shortcuts_use_backspace_and_bracket_right() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Backspace);
        assert_eq!(history_shortcut(&keyboard), Some(HistoryDirection::Back));

        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::BracketRight);
        assert_eq!(history_shortcut(&keyboard), Some(HistoryDirection::Forward));
    }

    #[test]
    fn breadcrumb_reaches_planet_level_when_a_known_planet_is_selected() {
        let mut simulation_resource = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let planet_id = simulation_resource
            .simulation
            .universe()
            .system(MVP_HOME_SYSTEM_ID)
            .and_then(|system| system.planets.first())
            .expect("home system has at least one planet")
            .id;
        apply_simulation_command(
            &mut simulation_resource,
            GameAction::SelectPlanet {
                system_id: MVP_HOME_SYSTEM_ID,
                planet_id,
            },
        );
        let navigation = StrategicNavigation {
            mode: StrategicViewMode::System(MVP_HOME_SYSTEM_ID),
            ..default()
        };

        let segments = breadcrumb_segments(&simulation_resource.simulation, &navigation);
        assert!(matches!(segments[0].kind, BreadcrumbKind::Galaxy));
        assert!(segments.iter().any(
            |segment| matches!(segment.kind, BreadcrumbKind::System(id) if id == MVP_HOME_SYSTEM_ID)
        ));
        assert!(
            segments
                .iter()
                .any(|segment| matches!(segment.kind, BreadcrumbKind::Planet))
        );
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
    fn planetary_analysis_no_longer_uses_an_instant_shortcut() {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyL);

        assert_eq!(simulation_shortcut(&keyboard), None);
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
            quantity: 1,
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

        assert!(message.contains(
            galactic_sim::craftable_definition(galactic_sim::CraftableId::LIGHT_PROBE).name
        ));
        assert!(
            message.contains(
                galactic_sim::default_building_catalog()
                    .definition(galactic_sim::BuildingKind::SHIPYARD)
                    .name
            )
        );
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
}

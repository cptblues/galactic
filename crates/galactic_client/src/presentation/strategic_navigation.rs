use bevy::input::ButtonInput;
use bevy::prelude::{KeyCode, Resource, Vec2, Vec3};
use galactic_domain::{SystemId, UniverseScalePreset, WorldPosition};
use galactic_sim::{GameAction, MVP_HOME_SYSTEM_ID, SelectionTarget, Simulation};

use crate::presentation::shortcuts::{apply_simulation_command, selected_system};
use crate::{GraphicsPreset, SimulationResource, UNIVERSE_VERTICAL_EXAGGERATION};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StrategicViewMode {
    Universe,
    System(SystemId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UniverseLod {
    Overview,
    Regional,
    Local,
}

impl UniverseLod {
    pub(crate) fn from_distance(distance: f32) -> Self {
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
pub(crate) enum UniverseProjection {
    #[default]
    Spatial,
    Flattened,
}

impl UniverseProjection {
    pub(crate) const fn target_mix(self) -> f32 {
        match self {
            Self::Spatial => 0.0,
            Self::Flattened => 1.0,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Spatial => "3D",
            Self::Flattened => "2,5D",
        }
    }
}

#[derive(Resource)]
pub(crate) struct StrategicNavigation {
    pub(crate) mode: StrategicViewMode,
    pub(crate) scale_preset: UniverseScalePreset,
    pub(crate) universe_focus: Vec3,
    pub(crate) universe_distance: f32,
    pub(crate) universe_max_distance: f32,
    pub(crate) universe_yaw: f32,
    pub(crate) universe_pitch: f32,
    pub(crate) system_focus: Vec3,
    pub(crate) system_distance: f32,
    pub(crate) system_yaw: f32,
    pub(crate) system_pitch: f32,
    pub(crate) lod: UniverseLod,
    pub(crate) debug_full_graph: bool,
    pub(crate) preset: GraphicsPreset,
    pub(crate) projection: UniverseProjection,
    pub(crate) projection_mix: f32,
}

impl Default for StrategicNavigation {
    fn default() -> Self {
        Self::new(UniverseScalePreset::default(), 108.0)
    }
}

impl StrategicNavigation {
    pub(crate) fn for_universe(
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

    pub(crate) fn enter_system(&mut self, system_id: SystemId) {
        self.mode = StrategicViewMode::System(system_id);
    }

    pub(crate) fn exit_system(&mut self) {
        self.mode = StrategicViewMode::Universe;
    }

    pub(crate) fn toggle_projection(&mut self) {
        self.projection = match self.projection {
            UniverseProjection::Spatial => UniverseProjection::Flattened,
            UniverseProjection::Flattened => UniverseProjection::Spatial,
        };
    }

    pub(crate) fn snapshot(&self, selected: SelectionTarget) -> ViewSnapshot {
        ViewSnapshot {
            mode: self.mode,
            universe_focus: self.universe_focus,
            universe_distance: self.universe_distance,
            universe_yaw: self.universe_yaw,
            universe_pitch: self.universe_pitch,
            system_focus: self.system_focus,
            system_distance: self.system_distance,
            system_yaw: self.system_yaw,
            system_pitch: self.system_pitch,
            selected,
        }
    }

    fn restore(&mut self, snapshot: &ViewSnapshot) {
        self.mode = snapshot.mode;
        self.universe_focus = snapshot.universe_focus;
        self.universe_distance = snapshot.universe_distance;
        self.universe_yaw = snapshot.universe_yaw;
        self.universe_pitch = snapshot.universe_pitch;
        self.system_focus = snapshot.system_focus;
        self.system_distance = snapshot.system_distance;
        self.system_yaw = snapshot.system_yaw;
        self.system_pitch = snapshot.system_pitch;
        self.lod = UniverseLod::from_distance(self.universe_distance);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewSnapshot {
    mode: StrategicViewMode,
    universe_focus: Vec3,
    universe_distance: f32,
    universe_yaw: f32,
    universe_pitch: f32,
    system_focus: Vec3,
    system_distance: f32,
    system_yaw: f32,
    system_pitch: f32,
    pub(crate) selected: SelectionTarget,
}

const MAX_NAVIGATION_HISTORY: usize = 50;

#[derive(Resource, Default)]
pub(crate) struct NavigationHistory {
    back: Vec<ViewSnapshot>,
    forward: Vec<ViewSnapshot>,
}

impl NavigationHistory {
    pub(crate) fn push(&mut self, snapshot: ViewSnapshot) {
        self.back.push(snapshot);
        if self.back.len() > MAX_NAVIGATION_HISTORY {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    fn navigate_back(&mut self, current: ViewSnapshot) -> Option<ViewSnapshot> {
        let previous = self.back.pop()?;
        self.forward.push(current);
        Some(previous)
    }

    fn navigate_forward(&mut self, current: ViewSnapshot) -> Option<ViewSnapshot> {
        let next = self.forward.pop()?;
        self.back.push(current);
        Some(next)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryDirection {
    Back,
    Forward,
}

pub(crate) fn history_shortcut(keyboard: &ButtonInput<KeyCode>) -> Option<HistoryDirection> {
    if keyboard.just_pressed(KeyCode::Backspace) {
        Some(HistoryDirection::Back)
    } else if keyboard.just_pressed(KeyCode::BracketRight) {
        Some(HistoryDirection::Forward)
    } else {
        None
    }
}

pub(crate) fn navigate_history(
    direction: HistoryDirection,
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    history: &mut NavigationHistory,
    rebuild: &mut ViewRebuildRequest,
) {
    let current_selected = simulation.simulation.state().selected;
    let current = navigation.snapshot(current_selected);
    let target = match direction {
        HistoryDirection::Back => history.navigate_back(current),
        HistoryDirection::Forward => history.navigate_forward(current),
    };
    let Some(target) = target else {
        return;
    };
    navigation.restore(&target);
    apply_selection(simulation, target.selected);
    rebuild.0 = true;
}

fn apply_selection(simulation: &mut SimulationResource, selected: SelectionTarget) {
    let action = match selected {
        SelectionTarget::None => return,
        SelectionTarget::System(system_id) => GameAction::SelectSystem(system_id),
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => GameAction::SelectPlanet {
            system_id,
            planet_id,
        },
    };
    apply_simulation_command(simulation, action);
}

const SEARCH_JUMP_DISTANCE: f32 = 62.0;
const SECTOR_FOCUS_DISTANCE: f32 = 96.0;

pub(crate) fn navigate_to_selection(
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    history: &mut NavigationHistory,
    rebuild: &mut ViewRebuildRequest,
    target: SelectionTarget,
    enter_system: bool,
) {
    let target_system = match target {
        SelectionTarget::None => return,
        SelectionTarget::System(system_id) | SelectionTarget::Planet { system_id, .. } => system_id,
    };
    let Some(system) = simulation.simulation.universe().system(target_system) else {
        return;
    };
    let position = system.position;
    let current_selected = simulation.simulation.state().selected;
    let snapshot = navigation.snapshot(current_selected);

    apply_selection(simulation, target);

    if enter_system {
        navigation.enter_system(target_system);
    } else {
        navigation.exit_system();
        navigation.universe_focus =
            projected_universe_position(position, navigation.projection_mix);
        navigation.universe_distance = SEARCH_JUMP_DISTANCE;
        navigation.lod = UniverseLod::from_distance(navigation.universe_distance);
    }

    history.push(snapshot);
    rebuild.0 = true;
}

pub(crate) fn navigate_to_sector(
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    history: &mut NavigationHistory,
    rebuild: &mut ViewRebuildRequest,
    sector_center: WorldPosition,
) {
    let current_selected = simulation.simulation.state().selected;
    let snapshot = navigation.snapshot(current_selected);

    navigation.exit_system();
    navigation.universe_focus =
        projected_universe_position(sector_center, navigation.projection_mix);
    navigation.universe_distance = navigation.universe_max_distance.min(SECTOR_FOCUS_DISTANCE);
    navigation.lod = UniverseLod::from_distance(navigation.universe_distance);

    history.push(snapshot);
    rebuild.0 = true;
}

pub(crate) fn navigate_to_galaxy(
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    history: &mut NavigationHistory,
    rebuild: &mut ViewRebuildRequest,
) {
    if matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }
    let current_selected = simulation.simulation.state().selected;
    let snapshot = navigation.snapshot(current_selected);
    navigation.exit_system();
    history.push(snapshot);
    rebuild.0 = true;
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BreadcrumbKind {
    Galaxy,
    Sector(WorldPosition),
    System(SystemId),
    Planet,
}

#[derive(Debug, Clone)]
pub(crate) struct BreadcrumbSegment {
    pub(crate) label: String,
    pub(crate) kind: BreadcrumbKind,
}

pub(crate) fn breadcrumb_segments(
    simulation: &Simulation,
    navigation: &StrategicNavigation,
) -> Vec<BreadcrumbSegment> {
    let mut segments = vec![BreadcrumbSegment {
        label: "Galaxie".to_string(),
        kind: BreadcrumbKind::Galaxy,
    }];
    let state = simulation.state();

    match navigation.mode {
        StrategicViewMode::Universe => {
            if let Some(system_id) = selected_system(state.selected)
                && let Some(sector) = simulation
                    .universe_repository()
                    .sector_for_system(system_id)
            {
                segments.push(BreadcrumbSegment {
                    label: sector.name.clone(),
                    kind: BreadcrumbKind::Sector(sector.center),
                });
            }
        }
        StrategicViewMode::System(system_id) => {
            if let Some(sector) = simulation
                .universe_repository()
                .sector_for_system(system_id)
            {
                segments.push(BreadcrumbSegment {
                    label: sector.name.clone(),
                    kind: BreadcrumbKind::Sector(sector.center),
                });
            }
            if let Some(system) = simulation.universe().system(system_id) {
                segments.push(BreadcrumbSegment {
                    label: system.name.clone(),
                    kind: BreadcrumbKind::System(system_id),
                });
                if let SelectionTarget::Planet {
                    system_id: selected_system_id,
                    planet_id,
                } = state.selected
                    && selected_system_id == system_id
                    && state.planet_knowledge_level(planet_id).reveals_identity()
                    && let Some(planet) = system.planet(planet_id)
                {
                    segments.push(BreadcrumbSegment {
                        label: planet.name.clone(),
                        kind: BreadcrumbKind::Planet,
                    });
                }
            }
        }
    }

    segments
}

#[derive(Resource, Default)]
pub(crate) struct ViewRebuildRequest(pub(crate) bool);

pub(crate) fn to_vec3(position: WorldPosition) -> Vec3 {
    Vec3::new(position.x, position.y, position.z)
}

pub(crate) fn projected_universe_position(position: WorldPosition, projection_mix: f32) -> Vec3 {
    let spatial = Vec3::new(
        position.x,
        position.y * UNIVERSE_VERTICAL_EXAGGERATION,
        position.z,
    );
    let flattened = Vec3::new(position.x, 0.0, position.z);
    spatial.lerp(flattened, projection_mix.clamp(0.0, 1.0))
}

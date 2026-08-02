use std::time::Duration;

use bevy::prelude::{
    BackgroundColor, Button, Changed, Color, Component, Interaction, Outline, Query, Resource,
    Vec2, Vec3, With, Without,
};
use galactic_domain::{PlanetId, ResourceStock, SectorId, SystemId, WorldPosition};
use galactic_sim::{KnowledgeLevel, SystemVisibility, TimeSpeed};

use crate::UniverseSystemTier;

#[derive(Resource, Default)]
pub(crate) struct SelectedMission(pub(crate) Option<galactic_domain::MissionId>);

// MVP technical refactor: single source of truth for which HUD panel is open,
// replacing the previous per-panel `open` booleans that had to be cross-cleared
// by hand from every other panel's toggle handler.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenPanel {
    #[default]
    None,
    Fleet,
    Craft,
    Research,
    Navigation,
    Colony,
}

#[derive(Component)]
pub(crate) struct StrategicViewEntity;

#[derive(Component)]
pub(crate) struct StrategicCamera;

#[derive(Component)]
pub(crate) struct SystemVisual {
    pub(crate) id: SystemId,
    pub(crate) tier: UniverseSystemTier,
    pub(crate) base_scale: Vec3,
}

#[derive(Component)]
pub(crate) struct SystemLabel {
    pub(crate) id: SystemId,
    pub(crate) visibility: SystemVisibility,
}

#[derive(Component)]
pub(crate) struct SectorLabel {
    pub(crate) id: SectorId,
    pub(crate) base_text: String,
    pub(crate) position: WorldPosition,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct OrbitingVisual {
    pub(crate) radius: f32,
    pub(crate) phase: f32,
    pub(crate) angular_speed: f32,
    pub(crate) vertical_offset: f32,
}

impl OrbitingVisual {
    pub(crate) fn translation_at(self, elapsed_seconds: f32) -> Vec3 {
        let angle = self.phase + elapsed_seconds * self.angular_speed;
        Vec3::new(
            angle.cos() * self.radius,
            self.vertical_offset,
            angle.sin() * self.radius,
        )
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AxialSpin {
    pub(crate) radians_per_second: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RouteVisualStyle {
    pub(crate) color: Color,
    pub(crate) dash_length: f32,
    pub(crate) gap_length: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KnownSectorLabel {
    pub(crate) id: SectorId,
    pub(crate) text: String,
    pub(crate) position: WorldPosition,
}

#[derive(Component)]
pub(crate) struct TopBarText;

#[derive(Component)]
pub(crate) struct HelpText;

#[derive(Component)]
pub(crate) struct InfoPanelText;

#[derive(Component)]
pub(crate) struct SelectableVisual {
    pub(crate) target: PickTarget,
    pub(crate) pick_radius_px: f32,
    pub(crate) priority: u8,
}

#[derive(Component)]
pub(crate) struct PointerHalo {
    pub(crate) target: PickTarget,
}

#[derive(Component)]
pub(crate) struct UiPointerBlocker;

#[derive(Component)]
pub(crate) struct PointerTooltipText;

#[derive(Component)]
pub(crate) struct AmbiguityPanelText;

// MVP-015: dedicated colony management screen.
#[derive(Resource)]
pub(crate) struct ColonyManagementState {
    pub(crate) selected_building: galactic_sim::BuildingKind,
    pub(crate) transport_destination_id: Option<galactic_domain::ColonyId>,
    pub(crate) transport_cargo: TransportCargoPreset,
    pub(crate) feedback: String,
}

impl Default for ColonyManagementState {
    fn default() -> Self {
        Self {
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
pub(crate) enum TransportCargoPreset {
    Metal,
    Crystal,
    Fuel,
    Mixed,
}

impl TransportCargoPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Mixed];

    pub(crate) const fn cargo(self) -> ResourceStock {
        match self {
            Self::Metal => ResourceStock::new(400, 0, 0),
            Self::Crystal => ResourceStock::new(0, 400, 0),
            Self::Fuel => ResourceStock::new(0, 0, 300),
            Self::Mixed => ResourceStock::new(200, 150, 100),
        }
    }

    pub(crate) const fn short_label(self) -> &'static str {
        match self {
            Self::Metal => "M 400",
            Self::Crystal => "C 400",
            Self::Fuel => "F 300",
            Self::Mixed => "Mixte",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceHudKind {
    Metal,
    Crystal,
    Fuel,
    Energy,
}

impl ResourceHudKind {
    pub(crate) const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Energy];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Metal => "MÉTAL",
            Self::Crystal => "CRISTAL",
            Self::Fuel => "CARBURANT",
            Self::Energy => "ÉNERGIE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceHudStatus {
    Normal,
    NearlyFull,
    Full,
    Deficit,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagementButtonAction {
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

pub(crate) type ManagementButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ManagementButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
pub(crate) struct ColonyManagementRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagementTextRole {
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
pub(crate) struct ManagementResourceCardText {
    pub(crate) kind: ResourceHudKind,
}

#[derive(Component)]
pub(crate) struct ManagementResourceGaugeFill {
    pub(crate) kind: ResourceHudKind,
}

#[derive(Component)]
pub(crate) struct ManagementBuildingButton {
    pub(crate) kind: galactic_sim::BuildingKind,
}

#[derive(Component)]
pub(crate) struct ManagementBuildingButtonText {
    pub(crate) kind: galactic_sim::BuildingKind,
}

#[derive(Component)]
pub(crate) struct ManagementUpgradeButton;

#[derive(Component)]
pub(crate) struct ManagementTransportLaunchButton;

#[derive(Component)]
pub(crate) struct ManagementTransportPresetButton {
    pub(crate) preset: TransportCargoPreset,
}

pub(crate) type ManagementTransportLaunchStyleQuery<'w, 's> = Query<
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

pub(crate) type ManagementTransportPresetStyleQuery<'w, 's> = Query<
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
pub(crate) struct ManagementQueueProgressFill;
// MVP-010: partial-information inspectors must never reveal hidden data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorContent {
    pub(crate) level: Option<KnowledgeLevel>,
    pub(crate) badge: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) hint: String,
}

impl InspectorContent {
    pub(crate) fn render(&self) -> String {
        format!(
            "{}\n{}\n\n{}\n\n{}",
            self.badge, self.title, self.body, self.hint,
        )
    }
}

// MVP-010-B: screen-space picking uses displayed transforms, not domain positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PickTarget {
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

impl PickTarget {
    pub(crate) const fn sort_key(self) -> (u8, u64, u64) {
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
pub(crate) struct PointerCandidate {
    pub(crate) target: PickTarget,
    pub(crate) screen_position: Vec2,
    pub(crate) screen_distance: f32,
    pub(crate) depth: f32,
    pub(crate) priority: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct AmbiguitySelection {
    pub(crate) targets: Vec<PickTarget>,
    pub(crate) active_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PointerClickRecord {
    pub(crate) target: PickTarget,
    pub(crate) at: Duration,
    pub(crate) cursor_position: Vec2,
}

#[derive(Resource, Default)]
pub(crate) struct PointerSelectionState {
    pub(crate) hovered: Option<PickTarget>,
    pub(crate) hovered_screen_position: Option<Vec2>,
    pub(crate) candidates: Vec<PointerCandidate>,
    pub(crate) ambiguity: Option<AmbiguitySelection>,
    pub(crate) last_click: Option<PointerClickRecord>,
}

impl PointerSelectionState {
    pub(crate) fn clear_hover(&mut self) {
        self.hovered = None;
        self.hovered_screen_position = None;
        self.candidates.clear();
    }

    pub(crate) fn cycle_ambiguity(&mut self, reverse: bool) -> Option<PickTarget> {
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
pub(crate) enum UiAction {
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
pub(crate) struct ActionButton {
    pub(crate) action: UiAction,
}

pub(crate) type ActionButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ActionButton),
    (Changed<Interaction>, With<Button>),
>;
pub(crate) type ActionButtonStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ActionButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
>;

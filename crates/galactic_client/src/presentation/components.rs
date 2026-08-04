use std::time::Duration;

use bevy::prelude::{
    BackgroundColor, Button, Changed, Children, Color, Component, Interaction, Node, Outline,
    Query, Resource, Text, TextColor, Vec2, Vec3, With, Without,
};
use galactic_domain::{PlanetId, SectorId, SystemId, WorldPosition};
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
pub(crate) struct TabBarGalaxyButton;

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

/// Marks UI nodes that are only meant for the developer's own testing (tick/debug telemetry,
/// pause/speed controls, debug graph, forced rebuild) and are hidden by default in the shipped
/// game. Visibility is toggled as a group via `DebugOverlayState`.
#[derive(Component)]
pub(crate) struct DebugOverlayRoot;

#[derive(Resource, Default)]
pub(crate) struct DebugOverlayState {
    pub(crate) visible: bool,
}

#[derive(Resource, Default)]
pub(crate) struct HelpUiState {
    pub(crate) shortcuts_visible: bool,
}

#[derive(Component)]
pub(crate) struct ResourceBarRoot;

#[derive(Component)]
pub(crate) struct ResourceBarCardText {
    pub(crate) kind: ResourceHudKind,
}

#[derive(Component)]
pub(crate) struct HelpText;

#[derive(Component)]
pub(crate) struct HelpToggleText;

/// Distinguishes the 3 fixed text blocks of the inspector panel within a single query, avoiding
/// the unprovable-disjointness query conflicts that separate marker components would cause once
/// several of them mutate `Text` in the same system.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorTextRole {
    /// Badge + title, always visible.
    Title,
    /// The scrollable body of whichever inspector tab is currently active.
    Body,
    /// The always-visible note (moons/selection consistency, then the knowledge-level hint)
    /// shown under the tab content, outside the scroll area.
    Footer,
}

/// Wraps the fixed pool of inspector tab buttons so the whole row can be hidden when the
/// current selection only has a single section (no tabs needed).
#[derive(Component)]
pub(crate) struct InspectorTabBarRoot;

#[derive(Component)]
pub(crate) struct InspectorTabButton {
    pub(crate) index: usize,
}

#[derive(Component)]
pub(crate) struct InspectorTabButtonLabel;

#[derive(Resource, Default)]
pub(crate) struct InspectorTabState {
    pub(crate) active: usize,
    last_section_titles: Vec<String>,
}

impl InspectorTabState {
    /// Resets to the first tab whenever the set of available sections changes (new selection,
    /// new knowledge level) so a stale tab index from a previous target is never carried over.
    pub(crate) fn sync(&mut self, sections: &[InspectorSection]) {
        let titles: Vec<String> = sections
            .iter()
            .map(|section| section.title.clone())
            .collect();
        if titles != self.last_section_titles {
            self.active = 0;
            self.last_section_titles = titles;
        } else if self.active >= sections.len() {
            self.active = 0;
        }
    }
}

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
            feedback: String::new(),
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
    SelectBuilding(galactic_sim::BuildingKind),
    UpgradeSelected,
    CancelConstruction,
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
pub(crate) struct ManagementQueueProgressFill;

#[derive(Component)]
pub(crate) struct CancelConstructionButton;
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorSection {
    pub(crate) title: String,
    pub(crate) body: String,
}

// MVP-010: partial-information inspectors must never reveal hidden data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorContent {
    pub(crate) level: Option<KnowledgeLevel>,
    pub(crate) badge: String,
    pub(crate) title: String,
    /// Switchable tab content. A single section means no tabs are shown at all.
    pub(crate) sections: Vec<InspectorSection>,
    /// Short note always shown under the active section (e.g. moons/selection consistency),
    /// outside the scroll area and common to every tab.
    pub(crate) footer: Option<String>,
    pub(crate) hint: String,
}

impl InspectorContent {
    /// Full concatenation of every section, used by tests that assert on the union of what a
    /// given knowledge level ever reveals, regardless of which tab happens to be active on
    /// screen.
    #[cfg(test)]
    pub(crate) fn render(&self) -> String {
        let sections = self
            .sections
            .iter()
            .map(|section| format!("{}\n{}", section.title, section.body))
            .collect::<Vec<_>>()
            .join("\n\n");
        let footer = self.footer.as_deref().unwrap_or_default();
        format!(
            "{}\n{}\n\n{}\n\n{}\n\n{}",
            self.badge, self.title, sections, footer, self.hint,
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
        &'static mut Node,
    ),
>;

pub(crate) type InspectorTextQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static InspectorTextRole,
        &'static mut Text,
        &'static mut TextColor,
    ),
    Without<InspectorTabButtonLabel>,
>;
pub(crate) type InspectorTabLabelQuery<'w, 's> =
    Query<'w, 's, &'static mut Text, (With<InspectorTabButtonLabel>, Without<InspectorTextRole>)>;
pub(crate) type InspectorTabButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static InspectorTabButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
        &'static mut Node,
        &'static Children,
    ),
    Without<InspectorTabBarRoot>,
>;

#[cfg(test)]
mod tests {
    use super::*;

    fn section(title: &str) -> InspectorSection {
        InspectorSection {
            title: title.to_string(),
            body: String::new(),
        }
    }

    #[test]
    fn tab_state_resets_to_the_first_tab_when_the_section_set_changes() {
        let mut state = InspectorTabState {
            active: 2,
            last_section_titles: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        };
        let sections = [section("A"), section("B"), section("C")];
        state.sync(&sections);
        assert_eq!(state.active, 2, "same sections must keep the active tab");

        let new_sections = [section("Aperçu")];
        state.sync(&new_sections);
        assert_eq!(
            state.active, 0,
            "a different section set must reset to the first tab"
        );
    }

    #[test]
    fn tab_state_clamps_a_stale_index_left_over_from_a_longer_section_set() {
        let mut state = InspectorTabState {
            active: 4,
            last_section_titles: vec!["Aperçu".to_string()],
        };
        let sections = [section("Aperçu")];
        state.sync(&sections);
        assert_eq!(state.active, 0);
    }
}

// MVP-030-B: global search, filters and navigation breadcrumb.
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use galactic_domain::{ColonyId, FleetId, MissionId, PlanetId, SectorId, SystemId};
use galactic_sim::{
    FleetLocation, FleetState, GameState, KnowledgeLevel, MissionKind, Simulation,
    craftable_definition,
};

use super::{
    BreadcrumbKind, NavigationHistory, OpenPanel, PresentationUpdateSet, SimulationResource,
    StrategicNavigation, UiPointerBlocker, ViewRebuildRequest, accent_fleet_blue,
    action_button_color, action_button_outline, breadcrumb_segments, mission_kind_label,
    mission_target_label, navigate_to_galaxy, navigate_to_sector, navigate_to_selection,
    panel_background, panel_outline, ui_text_font,
};

const NAVIGATION_Z_INDEX: i32 = 140;
const MAX_SEARCH_ROWS: usize = 12;
const MAX_BREADCRUMB_SEGMENTS: usize = 4;
const MAX_QUERY_LENGTH: usize = 48;

pub(crate) struct NavigationUiPlugin;

impl Plugin for NavigationUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NavigationUiState>()
            .add_systems(Startup, spawn_navigation_panels)
            .add_systems(
                Update,
                (
                    handle_navigation_shortcuts,
                    handle_search_text_input,
                    handle_navigation_toggle_buttons,
                    handle_search_result_buttons,
                    handle_breadcrumb_buttons,
                    handle_filter_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_breadcrumb_rows,
                    update_navigation_visibility,
                    update_search_results,
                    update_search_query_text,
                    update_filters_display,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NavigationFilters {
    min_knowledge: KnowledgeLevel,
    sector: Option<SectorId>,
    colonies_only: bool,
    mission_kind: Option<MissionKind>,
}

impl Default for NavigationFilters {
    fn default() -> Self {
        Self {
            min_knowledge: KnowledgeLevel::Unknown,
            sector: None,
            colonies_only: false,
            mission_kind: None,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct NavigationUiState {
    pub(crate) search_open: bool,
    pub(crate) filters_open: bool,
    query: String,
    filters: NavigationFilters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchEntryKind {
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
    Colony(ColonyId),
    Fleet(FleetId),
    Mission(MissionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchEntry {
    label: String,
    kind: SearchEntryKind,
}

fn sector_matches(
    repository: &galactic_sim::UniverseRepository,
    system_id: SystemId,
    filter: Option<SectorId>,
) -> bool {
    match filter {
        None => true,
        Some(sector_id) => repository
            .sector_for_system(system_id)
            .is_some_and(|sector| sector.id == sector_id),
    }
}

fn push_if_matches(
    entries: &mut Vec<SearchEntry>,
    label: &str,
    kind: SearchEntryKind,
    query_lower: &str,
) {
    if query_lower.is_empty() || label.to_lowercase().contains(query_lower) {
        entries.push(SearchEntry {
            label: label.to_string(),
            kind,
        });
    }
}

fn fleet_system_id(state: &GameState, fleet: &FleetState) -> Option<SystemId> {
    match fleet.location {
        FleetLocation::Docked(colony_id) => state.colony(colony_id).map(|colony| colony.system_id),
        FleetLocation::InSystem(system_id) => Some(system_id),
    }
}

fn fleet_composition_label(fleet: &FleetState) -> String {
    fleet
        .composition
        .entries()
        .map(|stack| {
            format!(
                "{} x{}",
                craftable_definition(stack.craftable).name,
                stack.quantity
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_search_index(
    simulation: &Simulation,
    filters: &NavigationFilters,
    query: &str,
) -> Vec<SearchEntry> {
    let state = simulation.state();
    let universe = simulation.universe();
    let repository = simulation.universe_repository();
    let query_lower = query.to_lowercase();
    let mut entries = Vec::new();

    if !filters.colonies_only {
        for system in &universe.systems {
            if !state.is_system_known(system.id) {
                continue;
            }
            if state.system_knowledge_level(system.id) < filters.min_knowledge {
                continue;
            }
            if !sector_matches(repository, system.id, filters.sector) {
                continue;
            }
            push_if_matches(
                &mut entries,
                &system.name,
                SearchEntryKind::System(system.id),
                &query_lower,
            );
        }

        for system in &universe.systems {
            for planet in &system.planets {
                let level = state.planet_knowledge_level(planet.id);
                if !level.reveals_identity() || level < filters.min_knowledge {
                    continue;
                }
                if !sector_matches(repository, system.id, filters.sector) {
                    continue;
                }
                push_if_matches(
                    &mut entries,
                    &planet.name,
                    SearchEntryKind::Planet {
                        system_id: system.id,
                        planet_id: planet.id,
                    },
                    &query_lower,
                );
            }
        }
    }

    for colony in &state.colonies {
        if !state.can_manage(state.player_faction, colony.owner) {
            continue;
        }
        if !sector_matches(repository, colony.system_id, filters.sector) {
            continue;
        }
        let label = format!("Colonie C{} {}", colony.id.raw(), colony.name);
        push_if_matches(
            &mut entries,
            &label,
            SearchEntryKind::Colony(colony.id),
            &query_lower,
        );
    }

    if !filters.colonies_only {
        for fleet in &state.fleets {
            if !state.can_manage(state.player_faction, fleet.owner) {
                continue;
            }
            let Some(system_id) = fleet_system_id(state, fleet) else {
                continue;
            };
            if !sector_matches(repository, system_id, filters.sector) {
                continue;
            }
            let label = format!(
                "Flotte #{} ({})",
                fleet.id.raw(),
                fleet_composition_label(fleet)
            );
            push_if_matches(
                &mut entries,
                &label,
                SearchEntryKind::Fleet(fleet.id),
                &query_lower,
            );
        }

        for mission in state.player_missions() {
            if let Some(kind) = filters.mission_kind
                && mission.order.kind != kind
            {
                continue;
            }
            if !sector_matches(repository, mission.order.origin, filters.sector) {
                continue;
            }
            let label = format!(
                "#{} {} → {}",
                mission.id.raw(),
                mission_kind_label(mission.order.kind),
                mission_target_label(simulation, mission.order.target),
            );
            push_if_matches(
                &mut entries,
                &label,
                SearchEntryKind::Mission(mission.id),
                &query_lower,
            );
        }
    }

    entries
}

fn known_sector_ids(simulation: &Simulation) -> Vec<SectorId> {
    let state = simulation.state();
    simulation
        .universe()
        .sectors
        .iter()
        .filter(|sector| sector.systems.iter().any(|id| state.is_system_known(*id)))
        .map(|sector| sector.id)
        .collect()
}

fn sector_name(simulation: &Simulation, sector_id: SectorId) -> String {
    simulation
        .universe()
        .sector(sector_id)
        .map(|sector| sector.name.clone())
        .unwrap_or_else(|| "secteur inconnu".to_string())
}

const MISSION_KIND_CYCLE: [MissionKind; 5] = [
    MissionKind::Probe,
    MissionKind::Attack,
    MissionKind::Transport,
    MissionKind::Harvest,
    MissionKind::Colonize,
];

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum NavAction {
    ToggleSearch,
    ToggleFilters,
    SelectResult(usize),
    CycleMinKnowledge,
    CycleSector,
    ToggleColoniesOnly,
    CycleMissionKind,
    ResetFilters,
}

type NavButtonInteractionQuery<'w, 's> =
    Query<'w, 's, (&'static Interaction, &'static NavAction), (Changed<Interaction>, With<Button>)>;

#[derive(Component)]
struct BreadcrumbButton {
    slot: usize,
    kind: Option<BreadcrumbKind>,
}

#[derive(Component)]
struct SearchResultRow {
    slot: usize,
    kind: Option<SearchEntryKind>,
}

#[derive(Component)]
struct SearchQueryText;

#[derive(Component)]
struct SearchRoot;

#[derive(Component)]
struct FiltersRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterField {
    MinKnowledge,
    Sector,
    ColoniesOnly,
    MissionKind,
}

#[derive(Component)]
struct FilterValueText(FilterField);

pub(crate) fn spawn_search_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_fleet_blue()),
            NavAction::ToggleSearch,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Rechercher  [/]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
        });
}

pub(crate) fn spawn_filters_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_fleet_blue()),
            NavAction::ToggleFilters,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Filtres  [B]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
        });
}

fn spawn_navigation_panels(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(300.0),
                right: Val::Px(370.0),
                top: Val::Px(64.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
            GlobalZIndex(NAVIGATION_Z_INDEX),
        ))
        .with_children(|bar| {
            for slot in 0..MAX_BREADCRUMB_SEGMENTS {
                spawn_breadcrumb_segment(bar, slot);
            }
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(300.0),
                right: Val::Px(370.0),
                top: Val::Px(102.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Hidden,
            GlobalZIndex(NAVIGATION_Z_INDEX),
            SearchRoot,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("RECHERCHE — tapez un nom, Entrée pour sélectionner, Échap pour fermer"),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            panel.spawn((
                Text::new("_"),
                ui_text_font(13.0),
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                SearchQueryText,
            ));
            for slot in 0..MAX_SEARCH_ROWS {
                spawn_search_row(panel, slot);
            }
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(300.0),
                right: Val::Px(370.0),
                top: Val::Px(102.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Hidden,
            GlobalZIndex(NAVIGATION_Z_INDEX),
            FiltersRoot,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("FILTRES"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            spawn_filter_row(
                panel,
                "Connaissance",
                NavAction::CycleMinKnowledge,
                FilterField::MinKnowledge,
            );
            spawn_filter_row(
                panel,
                "Secteur",
                NavAction::CycleSector,
                FilterField::Sector,
            );
            spawn_filter_row(
                panel,
                "Colonies uniquement",
                NavAction::ToggleColoniesOnly,
                FilterField::ColoniesOnly,
            );
            spawn_filter_row(
                panel,
                "Type de mission",
                NavAction::CycleMissionKind,
                FilterField::MissionKind,
            );
            spawn_small_action_button(panel, "Réinitialiser", NavAction::ResetFilters);
        });
}

fn spawn_filter_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: NavAction,
    field: FilterField,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Button,
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                action_button_outline(true, false, &Interaction::None),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                ui_text_font(11.5),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
            row.spawn((
                Text::new(""),
                ui_text_font(11.5),
                TextColor(Color::srgb(0.62, 0.86, 0.78)),
                FilterValueText(field),
            ));
        });
}

fn spawn_small_action_button(parent: &mut ChildSpawnerCommands, label: &str, action: NavAction) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.94)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.40, 0.56, 0.86, 0.50),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
        });
}

fn spawn_breadcrumb_segment(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.90)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.40, 0.56, 0.86, 0.40),
            ),
            Visibility::Hidden,
            BreadcrumbButton { slot, kind: None },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.86, 0.90, 0.98)),
            ));
        });
}

fn spawn_search_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(20.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.9)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.34, 0.46, 0.62, 0.40),
            ),
            Visibility::Hidden,
            NavAction::SelectResult(slot),
            SearchResultRow { slot, kind: None },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.86, 0.90, 0.98)),
            ));
        });
}

fn handle_navigation_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<NavigationUiState>,
    mut open_panel: ResMut<OpenPanel>,
) {
    if keyboard.just_pressed(KeyCode::Slash) && !ui.search_open {
        ui.search_open = true;
        ui.filters_open = false;
        ui.query.clear();
        *open_panel = OpenPanel::Navigation;
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyB) && !ui.search_open {
        ui.filters_open = !ui.filters_open;
        if ui.filters_open {
            ui.search_open = false;
            *open_panel = OpenPanel::Navigation;
        } else {
            *open_panel = OpenPanel::None;
        }
        return;
    }

    if ui.search_open && keyboard.just_pressed(KeyCode::Escape) {
        ui.search_open = false;
        *open_panel = OpenPanel::None;
        return;
    }
    if ui.filters_open && keyboard.just_pressed(KeyCode::Escape) {
        ui.filters_open = false;
        *open_panel = OpenPanel::None;
    }
}

fn handle_search_text_input(
    mut events: MessageReader<KeyboardInput>,
    mut ui: ResMut<NavigationUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut history: ResMut<NavigationHistory>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    if !ui.search_open {
        events.clear();
        return;
    }

    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Backspace => {
                ui.query.pop();
            }
            KeyCode::Escape | KeyCode::Slash => {}
            KeyCode::Enter => {
                let filters = ui.filters;
                let results = build_search_index(simulation.simulation(), &filters, &ui.query);
                if let Some(entry) = results.first() {
                    select_search_entry(
                        &entry.kind,
                        &mut simulation,
                        &mut navigation,
                        &mut history,
                        &mut rebuild,
                    );
                    ui.search_open = false;
                    *open_panel = OpenPanel::None;
                }
            }
            _ => {
                if let Some(text) = &event.text
                    && ui.query.chars().count() < MAX_QUERY_LENGTH
                {
                    for ch in text.chars() {
                        if !ch.is_control() {
                            ui.query.push(ch);
                        }
                    }
                }
            }
        }
    }
}

fn select_search_entry(
    kind: &SearchEntryKind,
    simulation: &mut SimulationResource,
    navigation: &mut StrategicNavigation,
    history: &mut NavigationHistory,
    rebuild: &mut ViewRebuildRequest,
) {
    match *kind {
        SearchEntryKind::System(system_id) => {
            navigate_to_selection(
                simulation,
                navigation,
                history,
                rebuild,
                galactic_sim::SelectionTarget::System(system_id),
                true,
            );
        }
        SearchEntryKind::Planet {
            system_id,
            planet_id,
        } => {
            navigate_to_selection(
                simulation,
                navigation,
                history,
                rebuild,
                galactic_sim::SelectionTarget::Planet {
                    system_id,
                    planet_id,
                },
                true,
            );
        }
        SearchEntryKind::Colony(colony_id) => {
            if let Some(colony) = simulation.simulation().state().colony(colony_id) {
                let system_id = colony.system_id;
                navigate_to_selection(
                    simulation,
                    navigation,
                    history,
                    rebuild,
                    galactic_sim::SelectionTarget::System(system_id),
                    true,
                );
            }
        }
        SearchEntryKind::Fleet(fleet_id) => {
            let target = simulation
                .simulation()
                .state()
                .fleets
                .iter()
                .find(|fleet| fleet.id == fleet_id)
                .and_then(|fleet| fleet_system_id(simulation.simulation().state(), fleet));
            if let Some(system_id) = target {
                navigate_to_selection(
                    simulation,
                    navigation,
                    history,
                    rebuild,
                    galactic_sim::SelectionTarget::System(system_id),
                    true,
                );
            }
        }
        SearchEntryKind::Mission(mission_id) => {
            if let Some(mission) = simulation.simulation().state().mission(mission_id) {
                let origin = mission.order.origin;
                navigate_to_selection(
                    simulation,
                    navigation,
                    history,
                    rebuild,
                    galactic_sim::SelectionTarget::System(origin),
                    false,
                );
            }
        }
    }
}

fn handle_navigation_toggle_buttons(
    mut ui: ResMut<NavigationUiState>,
    mut open_panel: ResMut<OpenPanel>,
    interactions: NavButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            NavAction::ToggleSearch => {
                ui.search_open = !ui.search_open;
                if ui.search_open {
                    ui.filters_open = false;
                    ui.query.clear();
                    *open_panel = OpenPanel::Navigation;
                } else {
                    *open_panel = OpenPanel::None;
                }
            }
            NavAction::ToggleFilters => {
                ui.filters_open = !ui.filters_open;
                if ui.filters_open {
                    ui.search_open = false;
                    *open_panel = OpenPanel::Navigation;
                } else {
                    *open_panel = OpenPanel::None;
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_search_result_buttons(
    mut ui: ResMut<NavigationUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut history: ResMut<NavigationHistory>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    interactions: NavButtonInteractionQuery,
    rows: Query<&SearchResultRow>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let NavAction::SelectResult(slot) = *action
            && let Some(row) = rows.iter().find(|row| row.slot == slot)
            && let Some(kind) = row.kind
        {
            select_search_entry(
                &kind,
                &mut simulation,
                &mut navigation,
                &mut history,
                &mut rebuild,
            );
            ui.search_open = false;
            *open_panel = OpenPanel::None;
        }
    }
}

fn handle_filter_buttons(
    mut ui: ResMut<NavigationUiState>,
    simulation: Res<SimulationResource>,
    interactions: NavButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            NavAction::CycleMinKnowledge => {
                ui.filters.min_knowledge = match ui.filters.min_knowledge {
                    KnowledgeLevel::Unknown | KnowledgeLevel::Detected => KnowledgeLevel::Probed,
                    KnowledgeLevel::Probed => KnowledgeLevel::Analyzed,
                    KnowledgeLevel::Analyzed => KnowledgeLevel::Colonized,
                    KnowledgeLevel::Colonized => KnowledgeLevel::Unknown,
                };
            }
            NavAction::CycleSector => {
                let known = known_sector_ids(simulation.simulation());
                ui.filters.sector = match ui.filters.sector {
                    None => known.first().copied(),
                    Some(current) => {
                        let index = known.iter().position(|id| *id == current);
                        let next_index = index.map(|index| index + 1).unwrap_or(0);
                        known.get(next_index).copied()
                    }
                };
            }
            NavAction::ToggleColoniesOnly => {
                ui.filters.colonies_only = !ui.filters.colonies_only;
            }
            NavAction::CycleMissionKind => {
                ui.filters.mission_kind = match ui.filters.mission_kind {
                    None => Some(MISSION_KIND_CYCLE[0]),
                    Some(current) => {
                        let index = MISSION_KIND_CYCLE.iter().position(|kind| *kind == current);
                        let next_index = index.map(|index| index + 1).unwrap_or(0);
                        MISSION_KIND_CYCLE.get(next_index).copied()
                    }
                };
            }
            NavAction::ResetFilters => {
                ui.filters = NavigationFilters::default();
            }
            _ => {}
        }
    }
}

fn handle_breadcrumb_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut history: ResMut<NavigationHistory>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    interactions: Query<(&Interaction, &BreadcrumbButton), Changed<Interaction>>,
) {
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button.kind {
            Some(BreadcrumbKind::Galaxy) => {
                navigate_to_galaxy(&mut simulation, &mut navigation, &mut history, &mut rebuild);
            }
            Some(BreadcrumbKind::Sector(center)) => {
                navigate_to_sector(
                    &mut simulation,
                    &mut navigation,
                    &mut history,
                    &mut rebuild,
                    center,
                );
            }
            Some(BreadcrumbKind::System(system_id)) => {
                navigate_to_selection(
                    &mut simulation,
                    &mut navigation,
                    &mut history,
                    &mut rebuild,
                    galactic_sim::SelectionTarget::System(system_id),
                    true,
                );
            }
            Some(BreadcrumbKind::Planet) | None => {}
        }
    }
}

fn update_breadcrumb_rows(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut rows: Query<(&mut BreadcrumbButton, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let segments = breadcrumb_segments(simulation.simulation(), &navigation);
    for (mut button, mut visibility, children) in &mut rows {
        if let Some(segment) = segments.get(button.slot) {
            button.kind = Some(segment.kind);
            *visibility = Visibility::Inherited;
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != segment.label
                {
                    text.0 = segment.label.clone();
                }
            }
        } else {
            button.kind = None;
            *visibility = Visibility::Hidden;
        }
    }
}

fn update_navigation_visibility(
    ui: Res<NavigationUiState>,
    mut search_roots: Query<&mut Visibility, (With<SearchRoot>, Without<FiltersRoot>)>,
    mut filters_roots: Query<&mut Visibility, (With<FiltersRoot>, Without<SearchRoot>)>,
) {
    for mut visibility in &mut search_roots {
        let next = if ui.search_open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for mut visibility in &mut filters_roots {
        let next = if ui.filters_open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

fn update_search_query_text(
    ui: Res<NavigationUiState>,
    mut texts: Query<&mut Text, With<SearchQueryText>>,
) {
    if !ui.search_open {
        return;
    }
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let next = format!("{}_", ui.query);
    if text.0 != next {
        text.0 = next;
    }
}

fn update_search_results(
    ui: Res<NavigationUiState>,
    simulation: Res<SimulationResource>,
    mut rows: Query<(&mut SearchResultRow, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text>,
) {
    if !ui.search_open {
        for (mut row, mut visibility, _) in &mut rows {
            row.kind = None;
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let entries = if ui.query.trim().is_empty() {
        Vec::new()
    } else {
        build_search_index(simulation.simulation(), &ui.filters, &ui.query)
    };

    for (mut row, mut visibility, children) in &mut rows {
        if let Some(entry) = entries.get(row.slot) {
            row.kind = Some(entry.kind);
            *visibility = Visibility::Inherited;
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != entry.label
                {
                    text.0 = entry.label.clone();
                }
            }
        } else {
            row.kind = None;
            *visibility = Visibility::Hidden;
        }
    }
}

fn filter_field_label(
    field: FilterField,
    ui: &NavigationUiState,
    simulation: &Simulation,
) -> String {
    match field {
        FilterField::MinKnowledge => match ui.filters.min_knowledge {
            KnowledgeLevel::Unknown | KnowledgeLevel::Detected => "Tous".to_string(),
            KnowledgeLevel::Probed => "Sondé+".to_string(),
            KnowledgeLevel::Analyzed => "Analysé+".to_string(),
            KnowledgeLevel::Colonized => "Colonisé".to_string(),
        },
        FilterField::Sector => match ui.filters.sector {
            None => "Tous".to_string(),
            Some(sector_id) => sector_name(simulation, sector_id),
        },
        FilterField::ColoniesOnly => {
            if ui.filters.colonies_only {
                "Oui".to_string()
            } else {
                "Non".to_string()
            }
        }
        FilterField::MissionKind => ui
            .filters
            .mission_kind
            .map(mission_kind_label)
            .unwrap_or("Tous")
            .to_string(),
    }
}

fn update_filters_display(
    ui: Res<NavigationUiState>,
    simulation: Res<SimulationResource>,
    mut texts: Query<(&FilterValueText, &mut Text)>,
) {
    if !ui.filters_open {
        return;
    }
    for (marker, mut text) in &mut texts {
        let label = filter_field_label(marker.0, &ui, simulation.simulation());
        if text.0 != label {
            text.0 = label;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galactic_domain::UniverseConfig;

    #[test]
    fn search_excludes_undetected_systems_and_includes_probed_ones() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let filters = NavigationFilters::default();

        let home_name = simulation
            .universe()
            .system(galactic_sim::MVP_HOME_SYSTEM_ID)
            .expect("home system exists")
            .name
            .clone();

        let results = build_search_index(&simulation, &filters, &home_name.to_lowercase());
        assert!(results.iter().any(|entry| entry.label == home_name));

        let hidden_system = simulation
            .universe()
            .systems
            .iter()
            .find(|system| !simulation.state().is_system_known(system.id))
            .expect("an undiscovered system exists in the reference universe");

        let hidden_results =
            build_search_index(&simulation, &filters, &hidden_system.name.to_lowercase());
        assert!(hidden_results.is_empty());
    }

    #[test]
    fn min_knowledge_filter_excludes_merely_probed_systems_when_set_to_analyzed() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let filters = NavigationFilters {
            min_knowledge: KnowledgeLevel::Analyzed,
            ..Default::default()
        };

        let home_level = simulation
            .state()
            .system_knowledge_level(galactic_sim::MVP_HOME_SYSTEM_ID);
        assert!(home_level >= KnowledgeLevel::Colonized);

        let home_name = simulation
            .universe()
            .system(galactic_sim::MVP_HOME_SYSTEM_ID)
            .expect("home system exists")
            .name
            .clone();
        let results = build_search_index(&simulation, &filters, &home_name.to_lowercase());
        assert!(results.iter().any(|entry| entry.label == home_name));
    }

    #[test]
    fn colonies_only_filter_hides_systems_and_planets() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let filters = NavigationFilters {
            colonies_only: true,
            ..Default::default()
        };

        let home_name = simulation
            .universe()
            .system(galactic_sim::MVP_HOME_SYSTEM_ID)
            .expect("home system exists")
            .name
            .clone();
        let results = build_search_index(&simulation, &filters, &home_name.to_lowercase());
        assert!(
            results
                .iter()
                .all(|entry| matches!(entry.kind, SearchEntryKind::Colony(_)))
        );
    }

    #[test]
    fn breadcrumb_starts_with_galaxy_segment() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let navigation = StrategicNavigation::default();
        let segments = breadcrumb_segments(&simulation, &navigation);
        assert!(!segments.is_empty());
        assert!(matches!(segments[0].kind, BreadcrumbKind::Galaxy));
    }
}

// MVP-030: dedicated fleets and missions screen kept outside the main client module.
use std::collections::BTreeMap;

use bevy::prelude::*;
use galactic_domain::SystemId;
use galactic_sim::{
    CraftableId, FleetAssignment, FleetComposition, FleetCompositionError, FleetError,
    FleetLocation, FleetState, GameAction, GameEventKind, MissionTarget, ShipStack, Simulation,
    StrategicDuration, craftable_catalog, craftable_definition,
};

use super::{
    OpenPanel, PresentationUpdateSet, SelectedMission, SimulationResource, UiPointerBlocker,
    accent_fleet_blue, apply_simulation_command, collect_presentation_events,
    format_strategic_duration, mission_error_text, mission_kind_label, mission_next_deadline,
    mission_phase_label, mission_result_text, mission_target_label, panel_background,
    panel_outline, ui_text_font,
};

const FLEET_Z_INDEX: i32 = 130;
const MAX_FLEET_ROWS: usize = 16;
const MAX_MISSION_ROWS: usize = 16;
const MAX_REPORT_ROWS: usize = 14;

pub(crate) struct FleetUiPlugin;

impl Plugin for FleetUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FleetUiState>()
            .add_systems(Startup, spawn_fleet_screen)
            .add_systems(
                Update,
                capture_fleet_feedback
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (
                    handle_fleet_shortcuts,
                    handle_fleet_tab_buttons,
                    handle_ship_stepper_buttons,
                    handle_form_fleet_button,
                    handle_active_mission_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_fleet_visibility,
                    update_fleet_list_rows,
                    update_ship_stepper_rows,
                    update_active_mission_rows,
                    update_report_rows,
                    update_feedback_text,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FleetUiTab {
    Fleets,
    Launch,
    Active,
    Reports,
}

#[derive(Resource)]
pub(crate) struct FleetUiState {
    pub(crate) tab: FleetUiTab,
    pending_composition: BTreeMap<CraftableId, u64>,
    feedback: String,
}

impl Default for FleetUiState {
    fn default() -> Self {
        Self {
            tab: FleetUiTab::Fleets,
            pending_composition: BTreeMap::new(),
            feedback: String::new(),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum FleetButtonAction {
    Toggle,
    Close,
    SelectTab(FleetUiTab),
    Ship(CraftableId, i64),
    FormFleet,
    CancelMission(usize),
    FocusOrigin(usize),
    FocusTarget(usize),
    HighlightMission(usize),
}

type FleetButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static FleetButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct FleetRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum FleetTextRole {
    Toggle,
    Feedback,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TabContent(pub(crate) FleetUiTab);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct TabButton(FleetUiTab);

#[derive(Component)]
struct FleetListRow(usize);

#[derive(Component)]
struct ShipStepperRow {
    craftable: CraftableId,
}

#[derive(Component)]
struct MissionRow {
    slot: usize,
    mission_id: Option<galactic_domain::MissionId>,
    origin: Option<SystemId>,
    target: Option<MissionTarget>,
    can_cancel: bool,
}

#[derive(Component)]
struct ReportRow(usize);

#[derive(Component)]
struct MissionCancelButton(usize);

pub(crate) fn spawn_fleet_toggle(parent: &mut ChildSpawnerCommands) {
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
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_fleet_blue()),
            FleetButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Flottes & missions  [V]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
                FleetTextRole::Toggle,
            ));
        });
}

fn spawn_fleet_screen(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(112.0),
                bottom: Val::Px(14.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(9.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.010, 0.014, 0.022, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.42, 0.62, 0.94, 0.74),
            ),
            Visibility::Hidden,
            GlobalZIndex(FLEET_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            FleetRoot,
        ))
        .with_children(|root| {
            spawn_fleet_header(root);
            spawn_tab_bar(root);
            spawn_fleets_tab(root);
            crate::mission_wizard::spawn_mission_wizard_tab(root);
            spawn_active_tab(root);
            spawn_reports_tab(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.70, 0.86, 1.0)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                FleetTextRole::Feedback,
            ));
        });
}

fn spawn_fleet_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("FLOTTES & MISSIONS"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.80, 0.88, 1.0)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            spawn_small_button(
                header,
                "Fermer  [V / Échap]",
                FleetButtonAction::Close,
                160.0,
            );
        });
}

fn spawn_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: FleetButtonAction,
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
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.40, 0.56, 0.86, 0.60),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.88, 0.98)),
            ));
        });
}

fn spawn_tab_bar(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(34.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(6.0),
        ..default()
    },))
        .with_children(|bar| {
            spawn_tab_button(bar, FleetUiTab::Fleets, "FLOTTES");
            spawn_tab_button(bar, FleetUiTab::Launch, "LANCER UNE MISSION");
            spawn_tab_button(bar, FleetUiTab::Active, "MISSIONS ACTIVES");
            spawn_tab_button(bar, FleetUiTab::Reports, "RAPPORTS");
        });
}

fn spawn_tab_button(parent: &mut ChildSpawnerCommands, tab: FleetUiTab, label: &str) {
    parent
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.98)),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            FleetButtonAction::SelectTab(tab),
            TabButton(tab),
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.88, 0.98)),
            ));
        });
}

fn spawn_fleets_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(9.0),
            ..default()
        },
        TabContent(FleetUiTab::Fleets),
    ))
    .with_children(|row| {
        spawn_fleet_list_panel(row);
        spawn_fleet_composer_panel(row);
    });
}

fn spawn_fleet_list_panel(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|list| {
        list.spawn((
            Text::new("FLOTTES CONTRÔLÉES"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.78, 0.86, 1.0)),
        ));
        for slot in 0..MAX_FLEET_ROWS {
            list.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.88, 0.92, 0.98)),
                Node {
                    min_height: Val::Px(16.0),
                    ..default()
                },
                Visibility::Hidden,
                FleetListRow(slot),
            ));
        }
    });
}

fn spawn_fleet_composer_panel(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(360.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|composer| {
        composer.spawn((
            Text::new("COMPOSER UNE NOUVELLE FLOTTE"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.78, 0.86, 1.0)),
        ));
        for definition in craftable_catalog().definitions() {
            let Some(_) = definition.ship else { continue };
            spawn_ship_stepper_row(composer, definition.id);
        }
        composer
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(38.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.20, 0.36, 0.98)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.40, 0.66, 0.98, 0.78),
                ),
                FleetButtonAction::FormFleet,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("FORMER LA FLOTTE"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.86, 0.92, 1.0)),
                ));
            });
    });
}

fn spawn_ship_stepper_row(parent: &mut ChildSpawnerCommands, craftable: CraftableId) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(28.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ShipStepperRow { craftable },
            ));
            spawn_stepper_button(row, "-", FleetButtonAction::Ship(craftable, -1));
            spawn_stepper_button(row, "+", FleetButtonAction::Ship(craftable, 1));
        });
}

fn spawn_stepper_button(parent: &mut ChildSpawnerCommands, label: &str, action: FleetButtonAction) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(28.0),
                min_height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.98)),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.86, 0.92, 1.0)),
            ));
        });
}

fn spawn_active_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        TabContent(FleetUiTab::Active),
    ))
    .with_children(|list| {
        list.spawn((Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(24.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },))
            .with_children(|header| {
                header.spawn((
                    Text::new("MISSIONS ACTIVES"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.78, 0.86, 1.0)),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                spawn_small_button(
                    header,
                    "Voir les rapports terminés",
                    FleetButtonAction::SelectTab(FleetUiTab::Reports),
                    200.0,
                );
            });
        for slot in 0..MAX_MISSION_ROWS {
            spawn_mission_row(list, slot);
        }
    });
}

fn spawn_mission_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            },
            Visibility::Hidden,
            MissionRow {
                slot,
                mission_id: None,
                origin: None,
                target: None,
                can_cancel: false,
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.88, 0.92, 0.98)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            spawn_row_action_button(row, "Origine", FleetButtonAction::FocusOrigin(slot));
            spawn_row_action_button(row, "Cible", FleetButtonAction::FocusTarget(slot));
            spawn_row_action_button(row, "Surligner", FleetButtonAction::HighlightMission(slot));
            row.spawn((
                Button,
                Node {
                    width: Val::Px(58.0),
                    min_height: Val::Px(20.0),
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
                Visibility::Hidden,
                FleetButtonAction::CancelMission(slot),
                MissionCancelButton(slot),
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Annuler"),
                    ui_text_font(9.0),
                    TextColor(Color::srgb(0.84, 0.90, 0.98)),
                ));
            });
        });
}

fn spawn_row_action_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: FleetButtonAction,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(58.0),
                min_height: Val::Px(20.0),
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
                ui_text_font(9.0),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
        });
}

fn spawn_reports_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        TabContent(FleetUiTab::Reports),
    ))
    .with_children(|list| {
        list.spawn((
            Text::new("RAPPORTS DE MISSION"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.78, 0.86, 1.0)),
        ));
        for slot in 0..MAX_REPORT_ROWS {
            list.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.86, 0.90, 0.98)),
                Node {
                    min_height: Val::Px(16.0),
                    ..default()
                },
                Visibility::Hidden,
                ReportRow(slot),
            ));
        }
    });
}

fn handle_fleet_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<FleetUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        let opening = *open_panel != OpenPanel::Fleet;
        *open_panel = if opening {
            OpenPanel::Fleet
        } else {
            OpenPanel::None
        };
        ui.feedback.clear();
        if opening {
            navigation_ui.search_open = false;
            navigation_ui.filters_open = false;
        }
        return;
    }

    if *open_panel == OpenPanel::Fleet && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
    }
}

fn handle_fleet_tab_buttons(
    mut ui: ResMut<FleetUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: FleetButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            FleetButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::Fleet;
                *open_panel = if opening {
                    OpenPanel::Fleet
                } else {
                    OpenPanel::None
                };
                ui.feedback.clear();
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            FleetButtonAction::Close => *open_panel = OpenPanel::None,
            FleetButtonAction::SelectTab(tab) => ui.tab = tab,
            _ => {}
        }
    }
}

fn handle_ship_stepper_buttons(
    simulation: Res<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
    interactions: FleetButtonInteractionQuery,
) {
    let available_by_craftable =
        active_colony(simulation.simulation()).map(|colony| &colony.inventory);
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let FleetButtonAction::Ship(craftable, delta) = *action else {
            continue;
        };
        let available = available_by_craftable.map_or(0, |inventory| inventory.quantity(craftable));
        let entry = ui.pending_composition.entry(craftable).or_insert(0);
        let next = if delta.is_negative() {
            entry.saturating_sub(1)
        } else {
            (*entry + 1).min(available)
        };
        *entry = next;
        ui.feedback.clear();
    }
}

fn handle_form_fleet_button(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
    interactions: FleetButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed || *action != FleetButtonAction::FormFleet {
            continue;
        }
        form_selected_fleet(&mut simulation, &mut ui);
    }
}

fn handle_active_mission_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
    mut selected_mission: ResMut<SelectedMission>,
    interactions: FleetButtonInteractionQuery,
    rows: Query<&MissionRow>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            FleetButtonAction::FocusOrigin(slot) => {
                if let Some(row) = rows.iter().find(|row| row.slot == slot)
                    && let Some(origin) = row.origin
                {
                    apply_simulation_command(&mut simulation, GameAction::SelectSystem(origin));
                }
            }
            FleetButtonAction::FocusTarget(slot) => {
                if let Some(row) = rows.iter().find(|row| row.slot == slot)
                    && let Some(target) = row.target
                {
                    let select = match target {
                        MissionTarget::System(system_id) => GameAction::SelectSystem(system_id),
                        MissionTarget::Planet {
                            system_id,
                            planet_id,
                        } => GameAction::SelectPlanet {
                            system_id,
                            planet_id,
                        },
                    };
                    apply_simulation_command(&mut simulation, select);
                }
            }
            FleetButtonAction::HighlightMission(slot) => {
                if let Some(row) = rows.iter().find(|row| row.slot == slot)
                    && let Some(mission_id) = row.mission_id
                {
                    selected_mission.0 = if selected_mission.0 == Some(mission_id) {
                        None
                    } else {
                        Some(mission_id)
                    };
                }
            }
            FleetButtonAction::CancelMission(slot) => {
                if let Some(row) = rows.iter().find(|row| row.slot == slot)
                    && row.can_cancel
                    && let Some(mission_id) = row.mission_id
                {
                    apply_simulation_command(
                        &mut simulation,
                        GameAction::CancelMission { mission_id },
                    );
                    ui.feedback.clear();
                }
            }
            _ => {}
        }
    }
}

fn capture_fleet_feedback(simulation: Res<SimulationResource>, mut ui: ResMut<FleetUiState>) {
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::FleetCreated(created) => {
                ui.feedback = format!("Flotte {} formée.", created.fleet_id.raw());
            }
            GameEventKind::FleetCreationRejected(rejected) => {
                ui.feedback = format!(
                    "Formation de flotte refusée : {}",
                    fleet_error_text(rejected.error)
                );
            }
            GameEventKind::MissionLaunched(launched) => {
                ui.feedback = format!(
                    "Mission {} ({}) lancée.",
                    launched.mission_id.raw(),
                    mission_kind_label(launched.kind),
                );
            }
            GameEventKind::MissionLaunchRejected(rejected) => {
                ui.feedback = format!("Mission refusée : {}", mission_error_text(rejected.error));
            }
            GameEventKind::MissionCancellationRejected(rejected) => {
                ui.feedback = format!(
                    "Annulation refusée : {}",
                    mission_error_text(rejected.error)
                );
            }
            GameEventKind::MissionReported(report) => {
                if let Some(result) = report.result {
                    ui.feedback = mission_result_text(result);
                }
            }
            _ => {}
        }
    }
}

fn update_fleet_visibility(
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, (With<FleetRoot>, Without<TabContent>)>,
    mut tabs: Query<(&TabContent, &mut Visibility, &mut Node), Without<FleetRoot>>,
    mut texts: Query<(&FleetTextRole, &mut Text)>,
) {
    let is_open = *open_panel == OpenPanel::Fleet;
    for mut visibility in &mut roots {
        let next = if is_open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (content, mut visibility, mut node) in &mut tabs {
        let active = is_open && content.0 == ui.tab;
        let next = if active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
        let next_display = if active { Display::Flex } else { Display::None };
        if node.display != next_display {
            node.display = next_display;
        }
    }
    for (role, mut text) in &mut texts {
        if *role == FleetTextRole::Toggle {
            let next = if is_open {
                "Fermer flottes".to_string()
            } else {
                "Flottes & missions  [V]".to_string()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
    }
}

fn update_feedback_text(
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&FleetTextRole, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (role, mut text) in &mut texts {
        if *role == FleetTextRole::Feedback {
            text.0 = ui.feedback.clone();
        }
    }
}

fn update_fleet_list_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&FleetListRow, &mut Text, &mut Visibility)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let simulation = simulation.simulation();
    let fleets = simulation.state().player_fleets().collect::<Vec<_>>();

    for (row, mut text, mut visibility) in &mut rows {
        if let Some(fleet) = fleets.get(row.0) {
            let label = format!(
                "#{} {} — {} — {}",
                fleet.id.raw(),
                fleet_composition_summary(fleet),
                fleet_location_label(simulation, fleet.location),
                fleet_assignment_label(fleet.assignment),
            );
            if text.0 != label {
                text.0 = label;
            }
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn update_ship_stepper_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&ShipStepperRow, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let colony = active_colony(simulation.simulation());
    for (row, mut text) in &mut rows {
        let definition = craftable_definition(row.craftable);
        let available = colony.map_or(0, |colony| colony.inventory.quantity(row.craftable));
        let selected = ui
            .pending_composition
            .get(&row.craftable)
            .copied()
            .unwrap_or(0);
        let label = format!("{} — {} / {} au dock", definition.name, selected, available);
        if text.0 != label {
            text.0 = label;
        }
    }
}

fn update_active_mission_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&mut MissionRow, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text>,
    mut cancel_buttons: Query<(&MissionCancelButton, &mut Visibility), Without<MissionRow>>,
) {
    if *open_panel != OpenPanel::Fleet || ui.tab != FleetUiTab::Active {
        return;
    }
    let simulation = simulation.simulation();
    let state = simulation.state();
    let current_tick = state.clock.current_tick();
    let mut missions = state
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal())
        .collect::<Vec<_>>();
    missions.sort_by_key(|mission| mission.id);

    let mut cancellable = [false; MAX_MISSION_ROWS];
    for (mut row, mut visibility, children) in &mut rows {
        if let Some(mission) = missions.get(row.slot) {
            row.mission_id = Some(mission.id);
            row.origin = Some(mission.order.origin);
            row.target = Some(mission.order.target);
            row.can_cancel = mission.phase == galactic_sim::MissionPhase::Preparation;
            if let Some(slot) = cancellable.get_mut(row.slot) {
                *slot = row.can_cancel;
            }
            let deadline = mission_next_deadline(mission, current_tick);
            let remaining = deadline.value().saturating_sub(current_tick.value());
            let label = format!(
                "#{} {} — {} • {} • ETA {}\n{} saut(s) • aller {} • sur place {} • retour {}",
                mission.id.raw(),
                mission_kind_label(mission.order.kind),
                mission_target_label(simulation, mission.order.target),
                mission_phase_label(mission.phase),
                format_strategic_duration(StrategicDuration::from_ticks(remaining)),
                mission.plan.hops,
                format_strategic_duration(mission.plan.travel_duration),
                format_strategic_duration(mission.plan.resolution_duration),
                format_strategic_duration(mission.plan.travel_duration),
            );
            *visibility = Visibility::Inherited;
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != label
                {
                    text.0 = label.clone();
                }
            }
        } else {
            row.mission_id = None;
            row.origin = None;
            row.target = None;
            row.can_cancel = false;
            *visibility = Visibility::Hidden;
        }
    }

    for (button, mut visibility) in &mut cancel_buttons {
        let can_cancel = cancellable.get(button.0).copied().unwrap_or(false);
        *visibility = if can_cancel {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn update_report_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&ReportRow, &mut Text, &mut Visibility)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.tab != FleetUiTab::Reports {
        return;
    }
    let reports = simulation
        .simulation()
        .state()
        .mission_reports
        .iter()
        .rev()
        .take(MAX_REPORT_ROWS)
        .collect::<Vec<_>>();

    for (row, mut text, mut visibility) in &mut rows {
        if let Some(report) = reports.get(row.0) {
            let summary = report
                .result
                .map(mission_result_text)
                .unwrap_or_else(|| format!("{:?}", report.outcome));
            let label = format!(
                "#{} {} (tick {}) — {}",
                report.mission_id.raw(),
                mission_kind_label(report.kind),
                report.occurred_at.value(),
                summary,
            );
            if text.0 != label {
                text.0 = label;
            }
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn form_selected_fleet(simulation: &mut SimulationResource, ui: &mut FleetUiState) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        ui.feedback = "Aucune colonie active.".to_string();
        return;
    };
    let stacks = ui
        .pending_composition
        .iter()
        .filter(|(_, quantity)| **quantity > 0)
        .map(|(craftable, quantity)| ShipStack::new(*craftable, *quantity))
        .collect::<Vec<_>>();
    match FleetComposition::from_stacks(stacks) {
        Ok(composition) => {
            apply_simulation_command(
                simulation,
                GameAction::FormFleet {
                    colony_id,
                    composition,
                },
            );
            ui.pending_composition.clear();
        }
        Err(error) => ui.feedback = fleet_composition_error_text(error),
    }
}

pub(crate) fn fleet_composition_summary(fleet: &FleetState) -> String {
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

pub(crate) fn fleet_location_label(simulation: &Simulation, location: FleetLocation) -> String {
    match location {
        FleetLocation::Docked(colony_id) => simulation
            .state()
            .colony(colony_id)
            .map(|colony| format!("amarrée C{} {}", colony_id.raw(), colony.name))
            .unwrap_or_else(|| "amarrée — colonie inconnue".to_string()),
        FleetLocation::InSystem(system_id) => simulation
            .universe_repository()
            .system(system_id)
            .map(|system| format!("en transit — {}", system.name))
            .unwrap_or_else(|| "en transit".to_string()),
    }
}

fn fleet_assignment_label(assignment: FleetAssignment) -> String {
    match assignment {
        FleetAssignment::Idle => "disponible".to_string(),
        FleetAssignment::Mission(mission_id) => format!("mission {}", mission_id.raw()),
    }
}

fn fleet_error_text(error: FleetError) -> String {
    match error {
        FleetError::UnknownColony(_) => "Colonie introuvable".to_string(),
        FleetError::Access(_) => "Colonie non contrôlée".to_string(),
        FleetError::InvalidComposition(error) => fleet_composition_error_text(error),
        FleetError::InsufficientDockedShips {
            craftable,
            requested,
            available,
        } => format!(
            "{} : {} demandé(s), {} disponible(s) au dock",
            craftable_definition(craftable).name,
            requested,
            available,
        ),
        FleetError::FleetIdOverflow => "Trop de flottes ont été formées.".to_string(),
    }
}

fn fleet_composition_error_text(error: FleetCompositionError) -> String {
    match error {
        FleetCompositionError::Empty => "Sélectionnez au moins une unité.".to_string(),
        FleetCompositionError::TooManyShipStacks { maximum, .. } => {
            format!("Trop de types de vaisseaux (maximum {maximum}).")
        }
        FleetCompositionError::ZeroQuantity(craftable) => {
            format!(
                "Quantité nulle pour {}",
                craftable_definition(craftable).name
            )
        }
        FleetCompositionError::DuplicateShip(craftable) => {
            format!("{} en double", craftable_definition(craftable).name)
        }
        FleetCompositionError::UnknownCraftable(_) => "Unité inconnue".to_string(),
        FleetCompositionError::NotShip(craftable) => {
            format!(
                "{} n'est pas un vaisseau",
                craftable_definition(craftable).name
            )
        }
        FleetCompositionError::ShipCountOverflow
        | FleetCompositionError::CargoCapacityOverflow
        | FleetCompositionError::FuelConsumptionOverflow => {
            "Composition trop importante.".to_string()
        }
    }
}

pub(crate) fn active_colony(simulation: &Simulation) -> Option<&galactic_sim::ColonyState> {
    simulation.state().active_player_colony()
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use galactic_domain::{ColonyId, UniverseConfig};

    use super::*;

    #[test]
    fn fleet_assignment_label_distinguishes_idle_from_mission() {
        assert_eq!(fleet_assignment_label(FleetAssignment::Idle), "disponible");
        assert!(
            fleet_assignment_label(FleetAssignment::Mission(galactic_domain::MissionId::new(3)))
                .contains('3')
        );
    }

    #[test]
    fn fleet_composition_summary_lists_ship_stacks_with_quantities() {
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 3)])
                .expect("light probe is a valid ship stack");
        let fleet = FleetState {
            id: galactic_domain::FleetId::new(0),
            owner: galactic_domain::Owner::Faction(galactic_domain::FactionId::new(0)),
            location: FleetLocation::Docked(ColonyId::new(0)),
            composition,
            cargo: galactic_domain::ResourceStock::default(),
            assignment: FleetAssignment::Idle,
        };

        let summary = fleet_composition_summary(&fleet);

        assert!(summary.contains("x3"));
        assert!(summary.contains(craftable_definition(CraftableId::LIGHT_PROBE).name));
    }

    #[test]
    fn fleet_location_label_reports_docked_colony() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = active_colony(&simulation).expect("home colony exists");

        let label = fleet_location_label(&simulation, FleetLocation::Docked(colony.id));

        assert!(label.contains("amarrée"));
        assert!(label.contains(&colony.name));
    }

    #[test]
    fn fleet_error_text_translates_every_variant_to_french() {
        assert_eq!(
            fleet_error_text(FleetError::FleetIdOverflow),
            "Trop de flottes ont été formées."
        );
        assert_eq!(
            fleet_error_text(FleetError::InvalidComposition(FleetCompositionError::Empty)),
            "Sélectionnez au moins une unité."
        );
    }

    #[test]
    fn fleet_composition_error_text_translates_every_variant_to_french() {
        assert_eq!(
            fleet_composition_error_text(FleetCompositionError::Empty),
            "Sélectionnez au moins une unité."
        );
        assert_eq!(
            fleet_composition_error_text(FleetCompositionError::ShipCountOverflow),
            "Composition trop importante."
        );
    }

    #[test]
    fn active_colony_returns_the_player_home_colony() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        let colony = active_colony(&simulation).expect("home colony exists");

        assert_eq!(Some(colony.id), simulation.state().active_colony_id);
    }

    #[test]
    fn opening_fleet_panel_overrides_another_open_panel() {
        let mut world = World::new();
        world.insert_resource(OpenPanel::Craft);
        world.insert_resource(FleetUiState::default());
        world.insert_resource(super::super::navigation_ui::NavigationUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyV);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_fleet_shortcuts)
            .expect("handle_fleet_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::Fleet);
    }

    #[test]
    fn escape_closes_the_fleet_panel() {
        let mut world = World::new();
        world.insert_resource(OpenPanel::Fleet);
        world.insert_resource(FleetUiState::default());
        world.insert_resource(super::super::navigation_ui::NavigationUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_fleet_shortcuts)
            .expect("handle_fleet_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::None);
    }
}

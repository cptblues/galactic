// MVP-030: dedicated fleets and missions screen kept outside the main client module.
use std::collections::BTreeMap;

use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use galactic_domain::{FleetId, MissionId, SystemId};
use galactic_sim::{
    CraftableId, FleetAssignment, FleetComposition, FleetCompositionError, FleetError,
    FleetLocation, FleetState, GameAction, GameEventKind, MAX_FLEET_NAME_CHARS, MissionTarget,
    ShipStack, Simulation, StrategicDuration, craftable_catalog, craftable_definition,
};

use crate::presentation::{
    components::{ScrollIndicatorArea, ScrollIndicatorId},
    entity_visuals::EntityVisualCatalog,
    scene::spawn_scroll_indicator,
};

use super::{
    CommandDockButton, CommandDockGroup, CommandDockTarget, GameWindowKind, GameWindowRoot,
    GameWindowTitleBar, OpenPanel, OpenWindows, PresentationUpdateSet, SelectedMission,
    SimulationResource, UI_ICON_SIZE_LARGE, UI_ICON_SIZE_SMALL, UiPointerBlocker,
    accent_fleet_blue, apply_simulation_command, collect_presentation_events, combat_report_text,
    format_strategic_duration, mission_error_text, mission_kind_label, mission_next_deadline,
    mission_phase_label_for_kind, mission_result_text, mission_target_label, panel_background,
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
                    handle_fleet_quantity_input,
                    handle_form_fleet_button,
                    handle_fleet_management_buttons,
                    handle_fleet_name_input,
                    handle_active_mission_buttons,
                    handle_report_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_fleet_visibility,
                    update_fleet_list_rows,
                    update_fleet_list_icons,
                    update_fleet_name_editor,
                    update_fleet_rename_buttons,
                    update_ship_stepper_rows,
                    update_quantity_editor_text,
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
    selected_fleet_id: Option<FleetId>,
    selected_report_id: Option<MissionId>,
    rename_buffer: String,
    rename_editing: bool,
    /// Which stepper row's quantity box is being typed into, if any — a
    /// numeric text-entry alternative to the `+`/`-` steppers, mirroring the
    /// fleet-rename text-input pattern (playtest feedback: typing a quantity
    /// directly is faster than repeated clicking for large numbers).
    quantity_editing: Option<CraftableId>,
    quantity_buffer: String,
    feedback: String,
}

impl Default for FleetUiState {
    fn default() -> Self {
        Self {
            tab: FleetUiTab::Fleets,
            pending_composition: BTreeMap::new(),
            selected_fleet_id: None,
            selected_report_id: None,
            rename_buffer: String::new(),
            rename_editing: false,
            quantity_editing: None,
            quantity_buffer: String::new(),
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
    ShipMax(CraftableId),
    StartQuantityEdit(CraftableId),
    FormFleet,
    SelectFleet(usize),
    StartFleetRename,
    ApplyFleetRename,
    DisbandFleet(usize),
    SelectReport(usize),
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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct TabButtonLabel(FleetUiTab);

#[derive(Component)]
struct FleetListRow(usize);

#[derive(Component)]
struct FleetListRowText(usize);

#[derive(Component)]
struct FleetListRowIcon(usize);

#[derive(Component)]
struct FleetDisbandButton(usize);

#[derive(Component)]
struct FleetNameEditorText;

#[derive(Component)]
struct ShipStepperRow {
    craftable: CraftableId,
}

#[derive(Component)]
struct ShipStepperIcon {
    craftable: CraftableId,
}

#[derive(Component)]
struct QuantityEditorText(CraftableId);

#[derive(Component)]
struct MissionRow {
    slot: usize,
    mission_id: Option<MissionId>,
    origin: Option<SystemId>,
    target: Option<MissionTarget>,
    can_cancel: bool,
}

#[derive(Component)]
struct ReportRow {
    slot: usize,
    mission_id: Option<MissionId>,
}

#[derive(Component)]
struct ReportDetailText;

#[derive(Component)]
struct MissionCancelButton(usize);

pub(crate) fn spawn_fleet_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(42.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.18, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_fleet_blue()),
            FleetButtonAction::Toggle,
            CommandDockButton {
                target: CommandDockTarget::Panel(OpenPanel::Fleet),
                group: CommandDockGroup::Operations,
            },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Flottes  [V]"),
                ui_text_font(12.5),
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
                bottom: Val::Px(74.0),
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
            GameWindowRoot {
                kind: GameWindowKind::Fleet,
            },
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
                    width: Val::Percent(100.0),
                    min_height: Val::Px(18.0),
                    ..default()
                },
                FleetTextRole::Feedback,
            ));
        });
}

fn spawn_fleet_header(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(42.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(0.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.16, 0.24, 0.38, 0.55)),
        Interaction::None,
        GameWindowTitleBar {
            kind: GameWindowKind::Fleet,
        },
        UiPointerBlocker,
    ))
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
                TabButtonLabel(tab),
            ));
        });
}

fn spawn_fleets_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
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
    row.spawn(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(0.0),
        min_height: Val::Px(0.0),
        position_type: PositionType::Relative,
        ..default()
    })
    .with_children(|frame| {
        frame
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    padding: UiRect::all(Val::Px(9.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                ScrollIndicatorArea {
                    id: ScrollIndicatorId::FleetList,
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
                list.spawn((
                    Text::new(""),
                    ui_text_font(10.5),
                    TextColor(Color::srgb(0.76, 0.86, 0.98)),
                    Node {
                        min_height: Val::Px(18.0),
                        ..default()
                    },
                    FleetNameEditorText,
                ));
                list.spawn((Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                },))
                    .with_children(|actions| {
                        spawn_small_button(
                            actions,
                            "Éditer nom",
                            FleetButtonAction::StartFleetRename,
                            118.0,
                        );
                        spawn_small_button(
                            actions,
                            "Valider nom",
                            FleetButtonAction::ApplyFleetRename,
                            118.0,
                        );
                    });
                for slot in 0..MAX_FLEET_ROWS {
                    list.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(UI_ICON_SIZE_SMALL + 12.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        },
                        Visibility::Hidden,
                        FleetListRow(slot),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Button,
                            Node {
                                flex_grow: 1.0,
                                min_height: Val::Px(UI_ICON_SIZE_SMALL + 8.0),
                                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(7.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.9)),
                            Outline::new(
                                Val::Px(1.0),
                                Val::ZERO,
                                Color::srgba(0.34, 0.46, 0.62, 0.40),
                            ),
                            FleetButtonAction::SelectFleet(slot),
                            UiPointerBlocker,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                ImageNode {
                                    image: Handle::default(),
                                    color: Color::WHITE,
                                    ..default()
                                },
                                Node {
                                    width: Val::Px(UI_ICON_SIZE_SMALL),
                                    height: Val::Px(UI_ICON_SIZE_SMALL),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                FleetListRowIcon(slot),
                            ));
                            button.spawn((
                                Text::new(""),
                                ui_text_font(10.5),
                                TextColor(Color::srgb(0.88, 0.92, 0.98)),
                                Node {
                                    flex_grow: 1.0,
                                    min_width: Val::Px(0.0),
                                    ..default()
                                },
                                FleetListRowText(slot),
                            ));
                        });
                        row.spawn((
                            Button,
                            Node {
                                width: Val::Px(94.0),
                                min_height: Val::Px(38.0),
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.18, 0.07, 0.08, 0.96)),
                            Outline::new(
                                Val::Px(1.0),
                                Val::ZERO,
                                Color::srgba(0.82, 0.34, 0.34, 0.64),
                            ),
                            FleetButtonAction::DisbandFleet(slot),
                            FleetDisbandButton(slot),
                            UiPointerBlocker,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Dissoudre"),
                                ui_text_font(10.0),
                                TextColor(Color::srgb(1.0, 0.82, 0.82)),
                            ));
                        });
                    });
                }
            });
        spawn_scroll_indicator(frame, ScrollIndicatorId::FleetList);
    });
}

fn spawn_fleet_composer_panel(row: &mut ChildSpawnerCommands) {
    row.spawn(Node {
        width: Val::Px(540.0),
        min_height: Val::Px(0.0),
        position_type: PositionType::Relative,
        ..default()
    })
    .with_children(|frame| {
        frame
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    padding: UiRect::all(Val::Px(11.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(7.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                ScrollIndicatorArea {
                    id: ScrollIndicatorId::FleetComposer,
                },
                BackgroundColor(panel_background()),
                Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            ))
            .with_children(|composer| {
                composer.spawn((
                    Text::new("COMPOSER UNE NOUVELLE FLOTTE"),
                    ui_text_font(13.0),
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
        spawn_scroll_indicator(frame, ScrollIndicatorId::FleetComposer);
    });
}

fn spawn_ship_stepper_row(parent: &mut ChildSpawnerCommands, craftable: CraftableId) {
    parent
        .spawn((Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(UI_ICON_SIZE_LARGE + 6.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            ..default()
        },))
        .with_children(|row| {
            row.spawn((
                ImageNode {
                    image: Handle::default(),
                    color: Color::WHITE,
                    ..default()
                },
                Node {
                    width: Val::Px(UI_ICON_SIZE_LARGE),
                    height: Val::Px(UI_ICON_SIZE_LARGE),
                    flex_shrink: 0.0,
                    ..default()
                },
                ShipStepperIcon { craftable },
            ));
            row.spawn((
                Text::new(""),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    ..default()
                },
                ShipStepperRow { craftable },
            ));
            spawn_stepper_button(row, "-1", FleetButtonAction::Ship(craftable, -1));
            spawn_quantity_editor(row, craftable);
            spawn_stepper_button(row, "+1", FleetButtonAction::Ship(craftable, 1));
            spawn_stepper_button(row, "MAX", FleetButtonAction::ShipMax(craftable));
        });
}

fn spawn_stepper_button(parent: &mut ChildSpawnerCommands, label: &str, action: FleetButtonAction) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(36.0),
                min_height: Val::Px(34.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)),
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
                ui_text_font(13.0),
                TextColor(Color::srgb(0.86, 0.92, 1.0)),
            ));
        });
}

/// A click-to-edit numeric box between the `-`/`+` steppers — types a
/// quantity directly instead of repeated clicking (playtest feedback).
/// Reuses the fleet-rename text-input convention (`handle_fleet_quantity_input`
/// mirrors `handle_fleet_name_input`'s shape exactly).
fn spawn_quantity_editor(parent: &mut ChildSpawnerCommands, craftable: CraftableId) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(64.0),
                min_height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.07, 0.12, 0.98)),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            FleetButtonAction::StartQuantityEdit(craftable),
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("0"),
                ui_text_font(13.0),
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                QuantityEditorText(craftable),
            ));
        });
}

fn spawn_active_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        ScrollPosition::default(),
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
                    min_width: Val::Px(0.0),
                    ..default()
                },
            ));
            spawn_row_action_button(row, "Origine", FleetButtonAction::FocusOrigin(slot));
            spawn_row_action_button(row, "Cible", FleetButtonAction::FocusTarget(slot));
            spawn_row_action_button(row, "Suivre", FleetButtonAction::HighlightMission(slot));
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
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        },
        TabContent(FleetUiTab::Reports),
    ))
    .with_children(|row| {
        row.spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                align_self: AlignSelf::Stretch,
                min_height: Val::Px(0.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|frame| {
            frame
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_height: Val::Px(0.0),
                        padding: UiRect {
                            left: Val::Px(9.0),
                            right: Val::Px(16.0),
                            top: Val::Px(9.0),
                            bottom: Val::Px(9.0),
                        },
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    ScrollIndicatorArea {
                        id: ScrollIndicatorId::MissionReportList,
                    },
                ))
                .with_children(|list| {
                    list.spawn((
                        Text::new("RAPPORTS DE MISSION"),
                        ui_text_font(12.0),
                        TextColor(Color::srgb(0.78, 0.86, 1.0)),
                    ));
                    for slot in 0..MAX_REPORT_ROWS {
                        spawn_report_row(list, slot);
                    }
                });
            spawn_scroll_indicator(frame, ScrollIndicatorId::MissionReportList);
        });

        row.spawn((
            Node {
                flex_grow: 1.2,
                flex_basis: Val::Px(0.0),
                align_self: AlignSelf::Stretch,
                min_height: Val::Px(0.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|frame| {
            frame
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_height: Val::Px(0.0),
                        padding: UiRect {
                            left: Val::Px(9.0),
                            right: Val::Px(16.0),
                            top: Val::Px(9.0),
                            bottom: Val::Px(9.0),
                        },
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    ScrollIndicatorArea {
                        id: ScrollIndicatorId::MissionReportDetail,
                    },
                ))
                .with_children(|detail| {
                    detail.spawn((
                        Text::new("Aucun rapport sélectionné."),
                        ui_text_font(10.5),
                        TextColor(Color::srgb(0.86, 0.90, 0.98)),
                        ReportDetailText,
                    ));
                });
            spawn_scroll_indicator(frame, ScrollIndicatorId::MissionReportDetail);
        });
    });
}

fn spawn_report_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.90)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.34, 0.46, 0.62, 0.40),
            ),
            Visibility::Hidden,
            FleetButtonAction::SelectReport(slot),
            ReportRow {
                slot,
                mission_id: None,
            },
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

fn handle_fleet_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<FleetUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut windows: ResMut<OpenWindows>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    save_load_ui: Res<crate::save_load_ui::SaveLoadUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || ui.rename_editing
        || crate::save_load_ui::save_name_is_editing(&save_load_ui)
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyV) {
        let opening = !windows.is_visible(GameWindowKind::Fleet);
        if opening {
            windows.open(GameWindowKind::Fleet);
            *open_panel = OpenPanel::None;
        } else {
            windows.close(GameWindowKind::Fleet);
            ui.rename_editing = false;
            ui.quantity_editing = None;
            ui.quantity_buffer.clear();
        }
        ui.feedback.clear();
        if opening {
            navigation_ui.search_open = false;
            navigation_ui.filters_open = false;
        }
        return;
    }

    if windows.topmost() == Some(GameWindowKind::Fleet) && keyboard.just_pressed(KeyCode::Escape) {
        windows.close(GameWindowKind::Fleet);
        ui.rename_editing = false;
        ui.quantity_editing = None;
        ui.quantity_buffer.clear();
    }
}

fn handle_fleet_tab_buttons(
    mut ui: ResMut<FleetUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut windows: ResMut<OpenWindows>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: FleetButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            FleetButtonAction::Toggle => {
                let opening = !windows.is_visible(GameWindowKind::Fleet);
                if opening {
                    windows.open(GameWindowKind::Fleet);
                    *open_panel = OpenPanel::None;
                } else {
                    windows.close(GameWindowKind::Fleet);
                    ui.rename_editing = false;
                    ui.quantity_editing = None;
                    ui.quantity_buffer.clear();
                }
                ui.feedback.clear();
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            FleetButtonAction::Close => {
                ui.rename_editing = false;
                ui.quantity_editing = None;
                ui.quantity_buffer.clear();
                windows.close(GameWindowKind::Fleet);
            }
            FleetButtonAction::SelectTab(tab) => {
                ui.rename_editing = false;
                ui.quantity_editing = None;
                ui.quantity_buffer.clear();
                ui.tab = tab;
            }
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
        match *action {
            FleetButtonAction::Ship(craftable, delta) => {
                // A stepper click is a clear "use the buttons" signal —
                // cancel any in-progress numeric text entry rather than
                // silently applying it alongside the click.
                ui.quantity_editing = None;
                ui.quantity_buffer.clear();
                let available =
                    available_by_craftable.map_or(0, |inventory| inventory.quantity(craftable));
                let entry = ui.pending_composition.entry(craftable).or_insert(0);
                let next = if delta.is_negative() {
                    entry.saturating_sub(1)
                } else {
                    (*entry + 1).min(available)
                };
                *entry = next;
                ui.feedback.clear();
            }
            FleetButtonAction::ShipMax(craftable) => {
                ui.quantity_editing = None;
                ui.quantity_buffer.clear();
                let available =
                    available_by_craftable.map_or(0, |inventory| inventory.quantity(craftable));
                ui.pending_composition.insert(craftable, available);
                ui.feedback.clear();
            }
            FleetButtonAction::StartQuantityEdit(craftable) => {
                if let Some(previous) = ui.quantity_editing
                    && previous != craftable
                {
                    commit_quantity_edit(&simulation, &mut ui, previous);
                }
                let current = ui.pending_composition.get(&craftable).copied().unwrap_or(0);
                ui.quantity_buffer = current.to_string();
                ui.quantity_editing = Some(craftable);
                ui.feedback.clear();
            }
            _ => {}
        }
    }
}

/// Parses `ui.quantity_buffer` and applies it to `craftable`'s pending
/// composition, clamped to what the active colony's dock actually has —
/// same bound the `+` stepper already enforces.
fn commit_quantity_edit(
    simulation: &SimulationResource,
    ui: &mut FleetUiState,
    craftable: CraftableId,
) {
    let available = active_colony(simulation.simulation())
        .map_or(0, |colony| colony.inventory.quantity(craftable));
    let requested: u64 = ui.quantity_buffer.parse().unwrap_or(0);
    ui.pending_composition
        .insert(craftable, requested.min(available));
    ui.quantity_editing = None;
    ui.quantity_buffer.clear();
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
        if let Some(craftable) = ui.quantity_editing {
            commit_quantity_edit(&simulation, &mut ui, craftable);
        }
        form_selected_fleet(&mut simulation, &mut ui);
    }
}

fn handle_fleet_management_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
    interactions: FleetButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            FleetButtonAction::SelectFleet(slot) => {
                if let Some((fleet_id, name)) =
                    fleet_at_slot(&simulation, slot).map(|fleet| (fleet.id, fleet.name.clone()))
                {
                    ui.selected_fleet_id = Some(fleet_id);
                    ui.rename_buffer = name;
                    ui.rename_editing = false;
                    ui.feedback.clear();
                }
            }
            FleetButtonAction::StartFleetRename => {
                if ui.rename_editing {
                    cancel_fleet_rename(&simulation, &mut ui);
                    continue;
                }
                if let Some(fleet_id) = ui.selected_fleet_id {
                    if let Some(fleet) = simulation.simulation().state().fleet(fleet_id) {
                        ui.rename_buffer = fleet.name.clone();
                        ui.rename_editing = true;
                        ui.feedback.clear();
                    } else {
                        ui.selected_fleet_id = None;
                        ui.rename_editing = false;
                        ui.feedback = "Flotte sélectionnée introuvable.".to_string();
                    }
                } else {
                    ui.feedback = "Sélectionnez une flotte à renommer.".to_string();
                }
            }
            FleetButtonAction::ApplyFleetRename => {
                if let Some(fleet_id) = ui.selected_fleet_id {
                    if !ui.rename_editing {
                        ui.feedback = "Activez l'édition avant de valider le nom.".to_string();
                        continue;
                    }
                    let name = ui.rename_buffer.clone();
                    apply_simulation_command(
                        &mut simulation,
                        GameAction::RenameFleet { fleet_id, name },
                    );
                    ui.rename_editing = false;
                } else {
                    ui.feedback = "Sélectionnez une flotte à renommer.".to_string();
                }
            }
            FleetButtonAction::DisbandFleet(slot) => {
                if let Some(fleet_id) = fleet_at_slot(&simulation, slot).map(|fleet| fleet.id) {
                    apply_simulation_command(
                        &mut simulation,
                        GameAction::DisbandFleet { fleet_id },
                    );
                }
            }
            _ => {}
        }
    }
}

fn handle_fleet_name_input(
    mut events: MessageReader<KeyboardInput>,
    windows: Res<OpenWindows>,
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
) {
    if !windows.is_visible(GameWindowKind::Fleet)
        || ui.tab != FleetUiTab::Fleets
        || !ui.rename_editing
    {
        return;
    }

    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Backspace => {
                ui.rename_buffer.pop();
            }
            KeyCode::Enter => {
                if let Some(fleet_id) = ui.selected_fleet_id {
                    let name = ui.rename_buffer.clone();
                    apply_simulation_command(
                        &mut simulation,
                        GameAction::RenameFleet { fleet_id, name },
                    );
                    ui.rename_editing = false;
                }
            }
            KeyCode::Escape => {
                cancel_fleet_rename(&simulation, &mut ui);
            }
            _ => {
                if let Some(text) = &event.text
                    && ui.rename_buffer.chars().count() < MAX_FLEET_NAME_CHARS
                {
                    for ch in text.chars() {
                        if !ch.is_control()
                            && ui.rename_buffer.chars().count() < MAX_FLEET_NAME_CHARS
                        {
                            ui.rename_buffer.push(ch);
                        }
                    }
                }
            }
        }
    }
}

fn cancel_fleet_rename(simulation: &SimulationResource, ui: &mut FleetUiState) {
    if let Some(fleet_id) = ui.selected_fleet_id {
        if let Some(fleet) = simulation.simulation().state().fleet(fleet_id) {
            ui.rename_buffer = fleet.name.clone();
        } else {
            ui.selected_fleet_id = None;
            ui.rename_buffer.clear();
        }
    } else {
        ui.rename_buffer.clear();
    }
    ui.rename_editing = false;
    ui.feedback.clear();
}

const MAX_QUANTITY_DIGITS: usize = 6;

/// Mirrors `handle_fleet_name_input`'s shape exactly (`Backspace`/`Enter`/
/// `Escape` special-cased, everything else pushes filtered characters) —
/// only the character filter (ASCII digits instead of "not a control
/// character") and the commit action differ.
fn handle_fleet_quantity_input(
    mut events: MessageReader<KeyboardInput>,
    windows: Res<OpenWindows>,
    simulation: Res<SimulationResource>,
    mut ui: ResMut<FleetUiState>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let Some(craftable) = ui.quantity_editing else {
        return;
    };

    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Backspace => {
                ui.quantity_buffer.pop();
            }
            KeyCode::Enter => {
                commit_quantity_edit(&simulation, &mut ui, craftable);
            }
            KeyCode::Escape => {
                ui.quantity_editing = None;
                ui.quantity_buffer.clear();
            }
            _ => {
                if let Some(text) = &event.text {
                    for ch in text.chars() {
                        if ch.is_ascii_digit()
                            && ui.quantity_buffer.chars().count() < MAX_QUANTITY_DIGITS
                        {
                            ui.quantity_buffer.push(ch);
                        }
                    }
                }
            }
        }
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

fn handle_report_buttons(
    mut ui: ResMut<FleetUiState>,
    interactions: FleetButtonInteractionQuery,
    rows: Query<&ReportRow>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let FleetButtonAction::SelectReport(slot) = *action else {
            continue;
        };
        if let Some(row) = rows.iter().find(|row| row.slot == slot)
            && let Some(mission_id) = row.mission_id
        {
            ui.selected_report_id = Some(mission_id);
            ui.feedback.clear();
        }
    }
}

fn capture_fleet_feedback(simulation: Res<SimulationResource>, mut ui: ResMut<FleetUiState>) {
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::FleetCreated(created) => {
                ui.selected_fleet_id = Some(created.fleet_id);
                ui.rename_buffer = simulation
                    .simulation()
                    .state()
                    .fleet(created.fleet_id)
                    .map(|fleet| fleet.name.clone())
                    .unwrap_or_else(|| format!("Flotte {}", created.fleet_id.raw() + 1));
                ui.rename_editing = false;
                ui.feedback = format!("{} formée.", ui.rename_buffer);
            }
            GameEventKind::FleetCreationRejected(rejected) => {
                ui.feedback = format!(
                    "Formation de flotte refusée : {}",
                    fleet_error_text(rejected.error)
                );
            }
            GameEventKind::FleetRenamed(renamed) => {
                ui.rename_buffer = simulation
                    .simulation()
                    .state()
                    .fleet(renamed.fleet_id)
                    .map(|fleet| fleet.name.clone())
                    .unwrap_or_default();
                ui.feedback = "Nom de flotte mis à jour.".to_string();
            }
            GameEventKind::FleetRenameRejected(rejected) => {
                ui.feedback = format!("Renommage refusé : {}", fleet_error_text(rejected.error));
            }
            GameEventKind::FleetDisbanded(disbanded) => {
                if ui.selected_fleet_id == Some(disbanded.fleet_id) {
                    ui.selected_fleet_id = None;
                    ui.rename_buffer.clear();
                    ui.rename_editing = false;
                }
                ui.feedback = format!("Flotte {} dissoute.", disbanded.fleet_id.raw());
            }
            GameEventKind::FleetDisbandRejected(rejected) => {
                ui.feedback = format!("Dissolution refusée : {}", fleet_error_text(rejected.error));
            }
            GameEventKind::MissionLaunched(launched) => {
                ui.tab = FleetUiTab::Active;
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
                ui.selected_report_id = Some(report.mission_id);
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
    windows: Res<OpenWindows>,
    mut roots: Query<&mut Visibility, (With<FleetRoot>, Without<TabContent>)>,
    mut tabs: Query<(&TabContent, &mut Visibility, &mut Node), Without<FleetRoot>>,
    mut texts: Query<(&FleetTextRole, &mut Text)>,
    mut tab_buttons: Query<(&TabButton, &Interaction, &mut BackgroundColor, &mut Outline)>,
    mut tab_labels: Query<(&TabButtonLabel, &mut TextColor)>,
) {
    let is_open = windows.is_visible(GameWindowKind::Fleet);
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
                "Flottes  [V]".to_string()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
    }
    for (button, interaction, mut background, mut outline) in &mut tab_buttons {
        let active = is_open && button.0 == ui.tab;
        background.0 = fleet_tab_button_color(active, interaction);
        outline.color = fleet_tab_button_outline(active);
    }
    for (label, mut color) in &mut tab_labels {
        color.0 = if is_open && label.0 == ui.tab {
            Color::srgb(0.98, 1.0, 1.0)
        } else {
            Color::srgb(0.72, 0.80, 0.92)
        };
    }
}

fn update_feedback_text(
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    mut texts: Query<(&FleetTextRole, &mut Text)>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) {
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
    windows: Res<OpenWindows>,
    mut rows: Query<(&FleetListRow, &mut Visibility), Without<FleetDisbandButton>>,
    mut labels: Query<(&FleetListRowText, &mut Text)>,
    mut disband_buttons: Query<(&FleetDisbandButton, &mut Visibility), Without<FleetListRow>>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let simulation = simulation.simulation();
    let fleets = simulation.state().player_fleets().collect::<Vec<_>>();

    for (row, mut visibility) in &mut rows {
        *visibility = if fleets.get(row.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (row, mut text) in &mut labels {
        if let Some(fleet) = fleets.get(row.0) {
            let marker = if ui.selected_fleet_id == Some(fleet.id) {
                "> "
            } else {
                ""
            };
            let label = format!(
                "{}{}  #{} — {} — {} — {}",
                marker,
                fleet.name,
                fleet.id.raw(),
                fleet_composition_summary(fleet),
                fleet_location_label(simulation, fleet.location),
                fleet_assignment_label(fleet.assignment),
            );
            if text.0 != label {
                text.0 = label;
            }
        }
    }
    for (button, mut visibility) in &mut disband_buttons {
        *visibility = if fleets
            .get(button.0)
            .is_some_and(|fleet| can_disband_fleet(fleet))
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn update_fleet_list_icons(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut icons: Query<(&FleetListRowIcon, &mut ImageNode)>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let fleets = simulation
        .simulation()
        .state()
        .player_fleets()
        .collect::<Vec<_>>();
    for (row, mut icon) in &mut icons {
        if let Some(fleet) = fleets.get(row.0)
            && let Some(craftable) = fleet_primary_visual(fleet)
        {
            icon.image = entity_visuals.ship(craftable);
            icon.color = if ui.selected_fleet_id == Some(fleet.id) {
                Color::WHITE
            } else {
                Color::srgba(0.82, 0.90, 0.98, 0.88)
            };
        }
    }
}

fn update_fleet_name_editor(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    mut texts: Query<&mut Text, With<FleetNameEditorText>>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let selected = ui
        .selected_fleet_id
        .and_then(|fleet_id| simulation.simulation().state().fleet(fleet_id));
    let label = if selected.is_some() {
        let cursor = if ui.rename_editing { "_" } else { "" };
        format!("Nom : {}{}", ui.rename_buffer, cursor)
    } else {
        "Nom : aucune flotte sélectionnée".to_string()
    };
    for mut text in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_fleet_rename_buttons(
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    buttons: Query<(&FleetButtonAction, &Children), With<Button>>,
    mut button_visibility: Query<(&FleetButtonAction, &mut Visibility), With<Button>>,
    mut texts: Query<&mut Text>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    for (action, children) in &buttons {
        if *action != FleetButtonAction::StartFleetRename {
            continue;
        }
        let label = if ui.rename_editing {
            "Annuler"
        } else {
            "Éditer nom"
        };
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != label
            {
                text.0 = label.to_string();
            }
        }
    }
    for (action, mut visibility) in &mut button_visibility {
        match action {
            FleetButtonAction::ApplyFleetRename => {
                *visibility = if ui.rename_editing {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            FleetButtonAction::StartFleetRename => {
                *visibility = if ui.selected_fleet_id.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            _ => {}
        }
    }
}

fn update_ship_stepper_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut rows: Query<(&ShipStepperRow, &mut Text)>,
    mut icons: Query<(&ShipStepperIcon, &mut ImageNode)>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    let colony = active_colony(simulation.simulation());
    for (row, mut text) in &mut rows {
        let definition = craftable_definition(row.craftable);
        let available = colony.map_or(0, |colony| colony.inventory.quantity(row.craftable));
        let label = format!("{} — {} au dock", definition.name, available);
        if text.0 != label {
            text.0 = label;
        }
    }
    for (marker, mut icon) in &mut icons {
        let selected = ui
            .pending_composition
            .get(&marker.craftable)
            .copied()
            .unwrap_or(0)
            > 0;
        icon.image = entity_visuals.ship(marker.craftable);
        icon.color = if selected {
            Color::WHITE
        } else {
            Color::srgba(0.82, 0.90, 0.98, 0.82)
        };
    }
}

fn update_quantity_editor_text(
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    mut texts: Query<(&QuantityEditorText, &mut Text)>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Fleets {
        return;
    }
    for (marker, mut text) in &mut texts {
        let label = if ui.quantity_editing == Some(marker.0) {
            format!("{}_", ui.quantity_buffer)
        } else {
            ui.pending_composition
                .get(&marker.0)
                .copied()
                .unwrap_or(0)
                .to_string()
        };
        if text.0 != label {
            text.0 = label;
        }
    }
}

fn update_active_mission_rows(
    simulation: Res<SimulationResource>,
    ui: Res<FleetUiState>,
    windows: Res<OpenWindows>,
    mut rows: Query<(&mut MissionRow, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text>,
    mut cancel_buttons: Query<(&MissionCancelButton, &mut Visibility), Without<MissionRow>>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Active {
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
                mission_phase_label_for_kind(mission.order.kind, mission.phase),
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
    windows: Res<OpenWindows>,
    mut rows: Query<(
        &mut ReportRow,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
        &mut Visibility,
        &Children,
    )>,
    mut row_texts: Query<&mut Text, Without<ReportDetailText>>,
    mut detail_texts: Query<&mut Text, With<ReportDetailText>>,
) {
    if !windows.is_visible(GameWindowKind::Fleet) || ui.tab != FleetUiTab::Reports {
        return;
    }
    let state = simulation.simulation().state();
    let reports = state
        .mission_reports
        .iter()
        .rev()
        .take(MAX_REPORT_ROWS)
        .collect::<Vec<_>>();
    let selected_report_id = ui
        .selected_report_id
        .filter(|mission_id| {
            reports
                .iter()
                .any(|report| report.mission_id == *mission_id)
        })
        .or_else(|| reports.first().map(|report| report.mission_id));

    for (mut row, interaction, mut background, mut outline, mut visibility, children) in &mut rows {
        if let Some(report) = reports.get(row.slot) {
            row.mission_id = Some(report.mission_id);
            let selected = selected_report_id == Some(report.mission_id);
            background.0 = report_row_background(selected, interaction);
            outline.color = report_row_outline(selected);
            let label = report_summary_label(report);
            for child in children {
                if let Ok(mut text) = row_texts.get_mut(*child)
                    && text.0 != label
                {
                    text.0 = label.clone();
                }
            }
            *visibility = Visibility::Inherited;
        } else {
            row.mission_id = None;
            *visibility = Visibility::Hidden;
        }
    }

    let detail_label = selected_report_id
        .and_then(|mission_id| {
            reports
                .iter()
                .copied()
                .find(|report| report.mission_id == mission_id)
        })
        .map(|report| report_detail_label(report, state.combat_report(report.mission_id)))
        .unwrap_or_else(|| "Aucun rapport terminé.".to_string());
    for mut text in &mut detail_texts {
        if text.0 != detail_label {
            text.0 = detail_label.clone();
        }
    }
}

fn report_summary_label(report: &galactic_sim::MissionReport) -> String {
    let summary = report
        .result
        .map(mission_result_text)
        .unwrap_or_else(|| format!("{:?}", report.outcome));
    format!(
        "#{} {}\ntick {} — {}",
        report.mission_id.raw(),
        mission_kind_label(report.kind),
        report.occurred_at.value(),
        summary,
    )
}

fn report_detail_label(
    report: &galactic_sim::MissionReport,
    combat: Option<&galactic_sim::CombatReport>,
) -> String {
    let header = report_summary_label(report).replace('\n', " — ");
    if let Some(combat) = combat {
        format!("{header}\n\n{}", combat_report_text(combat))
    } else {
        format!("{header}\n\nAucun rapport tactique détaillé pour cette mission.")
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

fn fleet_at_slot(simulation: &SimulationResource, slot: usize) -> Option<&FleetState> {
    simulation.simulation().state().player_fleets().nth(slot)
}

fn can_disband_fleet(fleet: &FleetState) -> bool {
    fleet.is_idle() && matches!(fleet.location, FleetLocation::Docked(_))
}

fn fleet_primary_visual(fleet: &FleetState) -> Option<CraftableId> {
    let mut selected = None;
    for stack in fleet.composition.entries() {
        match selected {
            Some((_, quantity)) if quantity >= stack.quantity => {}
            _ => selected = Some((stack.craftable, stack.quantity)),
        }
    }
    selected.map(|(craftable, _)| craftable)
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
        FleetError::UnknownFleet(_) => "Flotte introuvable".to_string(),
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
        FleetError::FleetNameEmpty => "Nom de flotte vide".to_string(),
        FleetError::FleetNameTooLong { maximum, .. } => {
            format!("Nom de flotte trop long (maximum {maximum} caractères).")
        }
        FleetError::FleetNameContainsControlCharacter => "Nom de flotte invalide".to_string(),
        FleetError::FleetNotIdle(_) => "La flotte est déjà affectée.".to_string(),
        FleetError::FleetNotDocked(_) => "La flotte n'est pas amarrée.".to_string(),
        FleetError::FleetCargoNotEmpty(_) => "La flotte transporte une cargaison.".to_string(),
        FleetError::FleetDockOwnerMismatch { .. } => {
            "La flotte n'est pas amarrée dans une colonie compatible.".to_string()
        }
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

/// True while the fleet screen is capturing free-text keyboard input —
/// either the rename field or a stepper row's numeric quantity box — so
/// every other screen's shortcut guard (already consulting this under its
/// old, rename-only name) also suspends its own shortcuts during quantity
/// entry without each call site needing a second check.
pub(crate) fn fleet_text_input_is_active(ui: &FleetUiState) -> bool {
    ui.rename_editing || ui.quantity_editing.is_some()
}

fn fleet_tab_button_color(active: bool, interaction: &Interaction) -> Color {
    if active {
        Color::srgba(0.10, 0.24, 0.42, 0.98)
    } else if *interaction == Interaction::Hovered {
        Color::srgba(0.07, 0.11, 0.19, 0.98)
    } else {
        Color::srgba(0.04, 0.06, 0.11, 0.98)
    }
}

fn fleet_tab_button_outline(active: bool) -> Color {
    if active {
        Color::srgba(0.58, 0.82, 1.0, 0.90)
    } else {
        panel_outline()
    }
}

fn report_row_background(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        Color::srgba(0.09, 0.20, 0.34, 0.98)
    } else if *interaction == Interaction::Hovered {
        Color::srgba(0.07, 0.11, 0.18, 0.98)
    } else {
        Color::srgba(0.04, 0.06, 0.10, 0.90)
    }
}

fn report_row_outline(selected: bool) -> Color {
    if selected {
        Color::srgba(0.48, 0.74, 1.0, 0.85)
    } else {
        Color::srgba(0.34, 0.46, 0.62, 0.40)
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::{IntoSystem, RunSystemOnce};
    use galactic_domain::{ColonyId, UniverseConfig};

    use super::*;

    macro_rules! assert_disjoint_queries {
        ($name:ident, $system:expr) => {
            #[test]
            fn $name() {
                let mut world = World::new();
                let mut system = IntoSystem::into_system($system);
                system.initialize(&mut world);
            }
        };
    }

    assert_disjoint_queries!(
        update_fleet_visibility_queries_are_disjoint,
        update_fleet_visibility
    );
    assert_disjoint_queries!(
        update_fleet_list_rows_queries_are_disjoint,
        update_fleet_list_rows
    );
    assert_disjoint_queries!(
        update_fleet_list_icons_queries_are_disjoint,
        update_fleet_list_icons
    );
    assert_disjoint_queries!(
        update_fleet_rename_buttons_queries_are_disjoint,
        update_fleet_rename_buttons
    );
    assert_disjoint_queries!(update_report_rows_queries_are_disjoint, update_report_rows);
    assert_disjoint_queries!(
        update_quantity_editor_text_queries_are_disjoint,
        update_quantity_editor_text
    );

    fn fresh_simulation_resource() -> SimulationResource {
        SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        }
    }

    #[test]
    fn commit_quantity_edit_clamps_to_what_the_dock_actually_has() {
        // A freshly-started colony has 0 of every craftable docked (nothing
        // built yet) — `.min(0)` still exercises the real clamp path, just
        // against a zero bound: no public test-only way to seed inventory
        // exists outside `galactic_sim` (`CraftInventory::add` is
        // `pub(crate)`), and that's an intentional boundary, not a gap to
        // work around.
        let simulation = fresh_simulation_resource();
        let mut ui = FleetUiState {
            quantity_buffer: "999".to_string(),
            quantity_editing: Some(CraftableId::FRIGATE_BULWARK),
            ..Default::default()
        };

        commit_quantity_edit(&simulation, &mut ui, CraftableId::FRIGATE_BULWARK);

        assert_eq!(
            ui.pending_composition.get(&CraftableId::FRIGATE_BULWARK),
            Some(&0)
        );
        assert_eq!(ui.quantity_editing, None);
        assert!(ui.quantity_buffer.is_empty());
    }

    #[test]
    fn commit_quantity_edit_treats_an_empty_buffer_as_zero() {
        let simulation = fresh_simulation_resource();
        let mut ui = FleetUiState::default();
        ui.quantity_buffer.clear();

        commit_quantity_edit(&simulation, &mut ui, CraftableId::FRIGATE_BULWARK);

        assert_eq!(
            ui.pending_composition.get(&CraftableId::FRIGATE_BULWARK),
            Some(&0)
        );
    }

    #[test]
    fn commit_quantity_edit_ignores_non_numeric_garbage() {
        let simulation = fresh_simulation_resource();
        // The keyboard filter only ever pushes ASCII digits, but the parse
        // fallback is still worth locking down directly.
        let mut ui = FleetUiState {
            quantity_buffer: "abc".to_string(),
            ..Default::default()
        };

        commit_quantity_edit(&simulation, &mut ui, CraftableId::FRIGATE_BULWARK);

        assert_eq!(
            ui.pending_composition.get(&CraftableId::FRIGATE_BULWARK),
            Some(&0)
        );
    }

    #[test]
    fn fleet_text_input_is_active_covers_both_rename_and_quantity_editing() {
        let mut ui = FleetUiState::default();
        assert!(!fleet_text_input_is_active(&ui));

        ui.rename_editing = true;
        assert!(fleet_text_input_is_active(&ui));
        ui.rename_editing = false;

        ui.quantity_editing = Some(CraftableId::FRIGATE_BULWARK);
        assert!(fleet_text_input_is_active(&ui));
    }

    #[test]
    fn active_fleet_tab_uses_a_distinct_button_style() {
        assert_ne!(
            fleet_tab_button_color(true, &Interaction::None),
            fleet_tab_button_color(false, &Interaction::None),
        );
        assert_ne!(
            fleet_tab_button_outline(true),
            fleet_tab_button_outline(false)
        );
        assert_ne!(
            report_row_background(true, &Interaction::None),
            report_row_background(false, &Interaction::None),
        );
        assert_ne!(report_row_outline(true), report_row_outline(false));
    }

    #[test]
    fn report_summary_stays_compact_and_detail_mentions_missing_tactical_report() {
        let report = galactic_sim::MissionReport {
            mission_id: MissionId::new(7),
            fleet_id: galactic_domain::FleetId::new(2),
            kind: galactic_sim::MissionKind::Attack,
            outcome: galactic_sim::MissionReportOutcome::Completed,
            occurred_at: galactic_sim::StrategicTick::new(42),
            result: None,
        };

        let summary = report_summary_label(&report);
        let detail = report_detail_label(&report, None);

        assert!(!summary.contains("RAPPORT DE COMBAT"));
        assert!(detail.contains("Aucun rapport tactique détaillé"));
    }

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
            name: "Recon Alpha".to_string(),
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
    fn fleet_primary_visual_uses_largest_stack_and_stable_tie_break() {
        let colony_id = ColonyId::new(0);
        let fleet = FleetState {
            id: FleetId::new(0),
            name: "Ligne Alpha".to_string(),
            owner: galactic_domain::Owner::Faction(galactic_domain::FactionId::new(0)),
            location: FleetLocation::Docked(colony_id),
            composition: FleetComposition::from_stacks([
                ShipStack::new(CraftableId::LIGHT_PROBE, 1),
                ShipStack::new(CraftableId::FRIGATE_BULWARK, 4),
            ])
            .expect("composition is valid"),
            cargo: galactic_domain::ResourceStock::ZERO,
            assignment: FleetAssignment::Idle,
        };
        assert_eq!(
            fleet_primary_visual(&fleet),
            Some(CraftableId::FRIGATE_BULWARK)
        );

        let tied_composition = FleetComposition::from_stacks([
            ShipStack::new(CraftableId::FRIGATE_BULWARK, 2),
            ShipStack::new(CraftableId::LIGHT_PROBE, 2),
        ])
        .expect("composition is valid");
        let expected = tied_composition
            .entries()
            .next()
            .expect("composition has a first stable entry")
            .craftable;
        let tied_fleet = FleetState {
            id: FleetId::new(1),
            name: "Mixte".to_string(),
            owner: galactic_domain::Owner::Faction(galactic_domain::FactionId::new(0)),
            location: FleetLocation::Docked(colony_id),
            composition: tied_composition,
            cargo: galactic_domain::ResourceStock::ZERO,
            assignment: FleetAssignment::Idle,
        };

        assert_eq!(fleet_primary_visual(&tied_fleet), Some(expected));
    }

    #[test]
    fn fleet_composer_rows_use_ship_visuals() {
        let mut windows = OpenWindows::default();
        windows.open(GameWindowKind::Fleet);
        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .insert_resource(fresh_simulation_resource())
            .insert_resource(OpenPanel::None)
            .insert_resource(windows)
            .insert_resource(FleetUiState::default())
            .add_systems(bevy::app::Startup, spawn_fleet_screen)
            .add_systems(bevy::app::Update, update_ship_stepper_rows);
        let entity_visuals = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EntityVisualCatalog::for_tests(&mut images)
        };
        app.insert_resource(entity_visuals);

        app.update();

        let world = app.world_mut();
        let expected = craftable_catalog()
            .definitions()
            .filter(|definition| definition.ship.is_some())
            .map(|definition| {
                (
                    definition.id,
                    world.resource::<EntityVisualCatalog>().ship(definition.id),
                )
            })
            .collect::<Vec<_>>();
        let mut icons = world.query::<(&ShipStepperIcon, &ImageNode)>();
        let rendered = icons.iter(world).collect::<Vec<_>>();
        assert_eq!(rendered.len(), expected.len());
        for (craftable, expected_image) in expected {
            let (_, icon) = rendered
                .iter()
                .find(|(marker, _)| marker.craftable == craftable)
                .expect("every ship composer row has an icon");
            assert_eq!(icon.image, expected_image);
            assert_ne!(icon.image, Handle::<Image>::default());
        }
    }

    #[test]
    fn fleet_list_rows_use_the_primary_ship_visual() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .active_colony_id
            .expect("home colony is active");
        let actor = simulation.state().player_faction;
        let fleet_id = FleetId::new(0);
        simulation.state_mut().fleets.push(FleetState {
            id: fleet_id,
            name: "Escorte Alpha".to_string(),
            owner: galactic_domain::Owner::Faction(actor),
            location: FleetLocation::Docked(colony_id),
            composition: FleetComposition::from_stacks([
                ShipStack::new(CraftableId::LIGHT_PROBE, 1),
                ShipStack::new(CraftableId::FRIGATE_BULWARK, 3),
            ])
            .expect("composition is valid"),
            cargo: galactic_domain::ResourceStock::ZERO,
            assignment: FleetAssignment::Idle,
        });
        let mut windows = OpenWindows::default();
        windows.open(GameWindowKind::Fleet);
        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(OpenPanel::None)
            .insert_resource(windows)
            .insert_resource(FleetUiState {
                selected_fleet_id: Some(fleet_id),
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_fleet_screen)
            .add_systems(
                bevy::app::Update,
                (update_fleet_list_rows, update_fleet_list_icons).chain(),
            );
        let entity_visuals = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EntityVisualCatalog::for_tests(&mut images)
        };
        app.insert_resource(entity_visuals);

        app.update();

        let world = app.world_mut();
        let expected = world
            .resource::<EntityVisualCatalog>()
            .ship(CraftableId::FRIGATE_BULWARK);
        let mut icons = world.query::<(&FleetListRowIcon, &ImageNode)>();
        let (_, icon) = icons
            .iter(world)
            .find(|(marker, _)| marker.0 == 0)
            .expect("fleet slot 0 has an icon");
        assert_eq!(icon.image, expected);
        assert_ne!(icon.image, Handle::<Image>::default());
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
    fn opening_fleet_window_keeps_other_game_windows_open() {
        let mut world = World::new();
        let mut windows = OpenWindows::default();
        windows.open(GameWindowKind::Craft);
        world.insert_resource(OpenPanel::None);
        world.insert_resource(windows);
        world.insert_resource(FleetUiState::default());
        world.insert_resource(super::super::navigation_ui::NavigationUiState::default());
        world.insert_resource(super::super::save_load_ui::SaveLoadUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyV);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_fleet_shortcuts)
            .expect("handle_fleet_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::None);
        let windows = world.resource::<OpenWindows>();
        assert!(windows.is_visible(GameWindowKind::Craft));
        assert!(windows.is_visible(GameWindowKind::Fleet));
    }

    #[test]
    fn fleet_shortcuts_are_ignored_while_renaming_a_fleet() {
        let mut world = World::new();
        let mut windows = OpenWindows::default();
        windows.open(GameWindowKind::Fleet);
        world.insert_resource(OpenPanel::None);
        world.insert_resource(windows);
        world.insert_resource(FleetUiState {
            rename_editing: true,
            ..default()
        });
        world.insert_resource(super::super::navigation_ui::NavigationUiState::default());
        world.insert_resource(super::super::save_load_ui::SaveLoadUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyV);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_fleet_shortcuts)
            .expect("handle_fleet_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::None);
        assert!(
            world
                .resource::<OpenWindows>()
                .is_visible(GameWindowKind::Fleet)
        );
        assert!(world.resource::<FleetUiState>().rename_editing);
    }

    #[test]
    fn cancelling_fleet_rename_restores_the_selected_fleet_name() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .active_colony_id
            .expect("home colony is active");
        let actor = simulation.state().player_faction;
        let fleet_id = FleetId::new(0);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("composition is valid");
        simulation.state_mut().fleets.push(FleetState {
            id: fleet_id,
            name: "Recon Alpha".to_string(),
            owner: galactic_domain::Owner::Faction(actor),
            location: FleetLocation::Docked(colony_id),
            composition,
            cargo: galactic_domain::ResourceStock::ZERO,
            assignment: FleetAssignment::Idle,
        });
        let resource = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = FleetUiState {
            selected_fleet_id: Some(fleet_id),
            rename_buffer: "Brouillon".to_string(),
            rename_editing: true,
            ..default()
        };

        cancel_fleet_rename(&resource, &mut ui);

        assert_eq!(ui.rename_buffer, "Recon Alpha");
        assert!(!ui.rename_editing);
    }

    #[test]
    fn mission_launch_feedback_switches_to_active_missions_tab() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let mut world = World::new();
        world.insert_resource(FleetUiState {
            tab: FleetUiTab::Launch,
            ..default()
        });
        world.insert_resource(SimulationResource {
            simulation,
            pending_events: vec![galactic_sim::GameEvent::new(
                actor,
                galactic_sim::StrategicTick::ZERO,
                GameEventKind::MissionLaunched(galactic_sim::MissionLaunched {
                    mission_id: galactic_domain::MissionId::new(3),
                    fleet_id: FleetId::new(1),
                    kind: galactic_sim::MissionKind::Probe,
                    target: MissionTarget::System(SystemId::new(0)),
                    departure_at: galactic_sim::StrategicTick::ZERO,
                    return_arrival_at: galactic_sim::StrategicTick::new(10),
                    fuel_cost: galactic_domain::ResourceCost::ZERO,
                }),
            )],
        });

        world
            .run_system_once(capture_fleet_feedback)
            .expect("capture_fleet_feedback runs");

        assert_eq!(world.resource::<FleetUiState>().tab, FleetUiTab::Active);
    }

    #[test]
    fn escape_closes_the_fleet_panel() {
        let mut world = World::new();
        let mut windows = OpenWindows::default();
        windows.open(GameWindowKind::Fleet);
        world.insert_resource(OpenPanel::None);
        world.insert_resource(windows);
        world.insert_resource(FleetUiState::default());
        world.insert_resource(super::super::navigation_ui::NavigationUiState::default());
        world.insert_resource(super::super::save_load_ui::SaveLoadUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_fleet_shortcuts)
            .expect("handle_fleet_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::None);
        assert!(
            !world
                .resource::<OpenWindows>()
                .is_visible(GameWindowKind::Fleet)
        );
    }
}

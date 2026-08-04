// MVP-030-A4: a step-by-step mission planner replacing the previous auto-pick launch tab.
// The player must explicitly choose a fleet at every step; nothing is auto-formed or
// auto-selected on their behalf.
use bevy::prelude::*;
use galactic_domain::{ColonyId, ExtractionSiteId, FleetId, PlanetId, ResourceStock, SystemId};
use galactic_sim::{
    GameAction, KnowledgeLevel, MissionKind, MissionOrder, MissionTarget, PlanetaryOccupancyIntel,
    Simulation, TechnologyUnlock, assess_planet_colonizability, eligible_fleets_for_mission,
    extraction_rules, harvest_recoverable_quantity, plan_mission,
};

use super::{
    OpenPanel, PresentationUpdateSet, SimulationResource, UiPointerBlocker, action_button_color,
    action_button_outline, apply_simulation_command, collect_presentation_events,
    format_strategic_duration, mission_error_text, mission_kind_label, panel_background,
    panel_outline, provisional_planet_label, ui_text_font,
};
use crate::fleet_ui::{
    FleetUiTab, TabContent, active_colony, fleet_composition_summary, fleet_location_label,
};

const MAX_TARGET_ROWS: usize = 16;
const MAX_FLEET_ROWS: usize = 8;
const CARGO_STEP: u64 = 50;

pub(crate) struct MissionWizardPlugin;

impl Plugin for MissionWizardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissionWizardState>()
            .add_systems(
                Update,
                capture_mission_wizard_feedback
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (
                    handle_wizard_kind_buttons,
                    handle_wizard_target_buttons,
                    handle_wizard_fleet_buttons,
                    handle_wizard_cargo_buttons,
                    handle_wizard_step_buttons,
                    handle_wizard_confirm_button,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_wizard_step_visibility,
                    update_wizard_origin_text,
                    update_wizard_kind_buttons,
                    update_wizard_target_rows,
                    update_wizard_fleet_rows,
                    update_wizard_params_text,
                    update_wizard_preview_text,
                    update_wizard_feedback_text,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Kind,
    Destination,
    Fleet,
    Params,
    Preview,
}

const WIZARD_STEPS: [WizardStep; 5] = [
    WizardStep::Kind,
    WizardStep::Destination,
    WizardStep::Fleet,
    WizardStep::Params,
    WizardStep::Preview,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardTarget {
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
    Colony(ColonyId),
    Site(ExtractionSiteId),
}

#[derive(Resource)]
pub(crate) struct MissionWizardState {
    step: WizardStep,
    kind: MissionKind,
    target: Option<WizardTarget>,
    fleet_id: Option<FleetId>,
    cargo: ResourceStock,
    feedback: String,
}

impl Default for MissionWizardState {
    fn default() -> Self {
        Self {
            step: WizardStep::Kind,
            kind: MissionKind::Probe,
            target: None,
            fleet_id: None,
            cargo: ResourceStock::ZERO,
            feedback: String::new(),
        }
    }
}

impl MissionWizardState {
    fn step_index(&self) -> usize {
        WIZARD_STEPS
            .iter()
            .position(|step| *step == self.step)
            .unwrap_or(0)
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum WizardButtonAction {
    SelectKind(MissionKind),
    SelectTarget(usize),
    SelectFleet(usize),
    AdjustMetal(i64),
    AdjustCrystal(i64),
    AdjustFuel(i64),
    MaxMetal,
    MaxCrystal,
    MaxFuel,
    ClearCargo,
    PreviousStep,
    NextStep,
    Confirm,
}

type WizardButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static WizardButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct WizardStepBlock(WizardStep);

#[derive(Component)]
struct WizardOriginText;

#[derive(Component)]
struct WizardKindButton(MissionKind);

#[derive(Component)]
struct WizardTargetRow {
    slot: usize,
    binding: Option<WizardTarget>,
}

#[derive(Component)]
struct WizardFleetRow {
    slot: usize,
    binding: Option<FleetId>,
}

#[derive(Component)]
struct WizardFleetHintText;

#[derive(Component)]
struct WizardDestinationHintText;

#[derive(Component)]
struct WizardParamsText;

#[derive(Component)]
struct WizardPreviewText;

#[derive(Component)]
struct WizardFeedbackText;

#[derive(Component)]
struct WizardStepLabel;

#[derive(Component)]
struct WizardPreviousButton;

#[derive(Component)]
struct WizardNextButton;

#[derive(Component)]
struct WizardConfirmButton;

pub(crate) fn spawn_mission_wizard_tab(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },
        TabContent(FleetUiTab::Launch),
    ))
    .with_children(|column| {
        column.spawn((
            Text::new(""),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.74, 0.84, 0.96)),
            WizardOriginText,
        ));
        column.spawn((
            Text::new(""),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.80, 0.88, 1.0)),
            WizardStepLabel,
        ));

        spawn_wizard_kind_block(column);
        spawn_wizard_destination_block(column);
        spawn_wizard_fleet_block(column);
        spawn_wizard_params_block(column);
        spawn_wizard_preview_block(column);

        column
            .spawn((Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(38.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            },))
            .with_children(|row| {
                spawn_wizard_nav_button(
                    row,
                    "< Précédente",
                    WizardButtonAction::PreviousStep,
                    WizardPreviousButton,
                );
                spawn_wizard_nav_button(
                    row,
                    "Suivante >",
                    WizardButtonAction::NextStep,
                    WizardNextButton,
                );
            });

        column.spawn((
            Text::new(""),
            ui_text_font(10.5),
            TextColor(Color::srgb(0.94, 0.72, 0.68)),
            WizardFeedbackText,
        ));
    });
}

fn spawn_wizard_nav_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: WizardButtonAction,
    marker: impl Bundle,
) {
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
            action,
            marker,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
        });
}

fn spawn_wizard_kind_block(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            },
            WizardStepBlock(WizardStep::Kind),
        ))
        .with_children(|bar| {
            spawn_wizard_kind_button(bar, MissionKind::Probe, "Reconnaissance");
            spawn_wizard_kind_button(bar, MissionKind::Attack, "Attaque");
            spawn_wizard_kind_button(bar, MissionKind::Harvest, "Récolte");
            spawn_wizard_kind_button(bar, MissionKind::Colonize, "Colonisation");
            spawn_wizard_kind_button(bar, MissionKind::Transport, "Transport");
        });
}

fn spawn_wizard_kind_button(parent: &mut ChildSpawnerCommands, kind: MissionKind, label: &str) {
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
            WizardButtonAction::SelectKind(kind),
            WizardKindButton(kind),
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
            ));
        });
}

fn spawn_wizard_destination_block(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            WizardStepBlock(WizardStep::Destination),
        ))
        .with_children(|list| {
            list.spawn((
                Text::new("DESTINATION"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            list.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.94, 0.72, 0.68)),
                WizardDestinationHintText,
            ));
            for slot in 0..MAX_TARGET_ROWS {
                spawn_wizard_target_row(list, slot);
            }
        });
}

fn spawn_wizard_target_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
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
            WizardButtonAction::SelectTarget(slot),
            WizardTargetRow {
                slot,
                binding: None,
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

fn spawn_wizard_fleet_block(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            WizardStepBlock(WizardStep::Fleet),
        ))
        .with_children(|list| {
            list.spawn((
                Text::new("FLOTTE (aucun choix automatique)"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            list.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.94, 0.72, 0.68)),
                WizardFleetHintText,
            ));
            for slot in 0..MAX_FLEET_ROWS {
                spawn_wizard_fleet_row(list, slot);
            }
        });
}

fn spawn_wizard_fleet_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
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
            WizardButtonAction::SelectFleet(slot),
            WizardFleetRow {
                slot,
                binding: None,
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

fn spawn_wizard_params_block(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
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
            WizardStepBlock(WizardStep::Params),
        ))
        .with_children(|column| {
            column.spawn((
                Text::new("CARGAISON / PARAMÈTRES"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            column.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.86, 0.90, 0.98)),
                WizardParamsText,
            ));
            spawn_wizard_cargo_row(
                column,
                "Métal",
                WizardButtonAction::AdjustMetal(-(CARGO_STEP as i64)),
                WizardButtonAction::AdjustMetal(CARGO_STEP as i64),
                WizardButtonAction::MaxMetal,
            );
            spawn_wizard_cargo_row(
                column,
                "Cristal",
                WizardButtonAction::AdjustCrystal(-(CARGO_STEP as i64)),
                WizardButtonAction::AdjustCrystal(CARGO_STEP as i64),
                WizardButtonAction::MaxCrystal,
            );
            spawn_wizard_cargo_row(
                column,
                "Carburant",
                WizardButtonAction::AdjustFuel(-(CARGO_STEP as i64)),
                WizardButtonAction::AdjustFuel(CARGO_STEP as i64),
                WizardButtonAction::MaxFuel,
            );
            spawn_wizard_nav_button(
                column,
                "Vider la cargaison",
                WizardButtonAction::ClearCargo,
                (),
            );
        });
}

fn spawn_wizard_cargo_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    minus: WizardButtonAction,
    plus: WizardButtonAction,
    max: WizardButtonAction,
) {
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
                Text::new(label.to_string()),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.84, 0.90, 0.98)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            spawn_wizard_small_button(row, "-50", minus);
            spawn_wizard_small_button(row, "+50", plus);
            spawn_wizard_small_button(row, "MAX", max);
        });
}

fn spawn_wizard_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: WizardButtonAction,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(44.0),
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
                ui_text_font(10.0),
                TextColor(Color::srgb(0.86, 0.92, 1.0)),
            ));
        });
}

fn spawn_wizard_preview_block(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            WizardStepBlock(WizardStep::Preview),
        ))
        .with_children(|column| {
            column.spawn((
                Text::new("ROUTE ET DURÉE"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
            ));
            column.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.86, 0.90, 0.98)),
                WizardPreviewText,
            ));
            column
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(40.0),
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
                    WizardButtonAction::Confirm,
                    WizardConfirmButton,
                    UiPointerBlocker,
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("LANCER LA MISSION"),
                        ui_text_font(13.0),
                        TextColor(Color::srgb(0.88, 0.94, 1.0)),
                    ));
                });
        });
}

fn handle_wizard_kind_buttons(
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let WizardButtonAction::SelectKind(kind) = *action {
            ui.kind = kind;
            ui.target = None;
            ui.fleet_id = None;
            ui.cargo = ResourceStock::ZERO;
            ui.feedback.clear();
        }
    }
}

fn handle_wizard_target_buttons(
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
    rows: Query<&WizardTargetRow>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let WizardButtonAction::SelectTarget(slot) = *action
            && let Some(row) = rows.iter().find(|row| row.slot == slot)
            && let Some(target) = row.binding
        {
            ui.target = Some(target);
            ui.fleet_id = None;
            ui.feedback.clear();
        }
    }
}

fn handle_wizard_fleet_buttons(
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
    rows: Query<&WizardFleetRow>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let WizardButtonAction::SelectFleet(slot) = *action
            && let Some(row) = rows.iter().find(|row| row.slot == slot)
            && let Some(fleet_id) = row.binding
        {
            ui.fleet_id = Some(fleet_id);
            ui.cargo = ResourceStock::ZERO;
            ui.feedback.clear();
        }
    }
}

fn handle_wizard_cargo_buttons(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    let capacity = wizard_selected_fleet_capacity(&simulation, &ui);
    let available = active_colony(simulation.simulation())
        .map(|colony| colony.resources.available())
        .unwrap_or(ResourceStock::ZERO);
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            WizardButtonAction::AdjustMetal(delta) => {
                ui.cargo.metal = adjust_cargo_value(ui.cargo, ui.cargo.metal, delta, capacity);
            }
            WizardButtonAction::AdjustCrystal(delta) => {
                ui.cargo.crystal = adjust_cargo_value(ui.cargo, ui.cargo.crystal, delta, capacity);
            }
            WizardButtonAction::AdjustFuel(delta) => {
                ui.cargo.fuel = adjust_cargo_value(ui.cargo, ui.cargo.fuel, delta, capacity);
            }
            WizardButtonAction::MaxMetal => {
                let remaining = capacity.saturating_sub(ui.cargo.crystal + ui.cargo.fuel);
                ui.cargo.metal = remaining.min(available.metal);
            }
            WizardButtonAction::MaxCrystal => {
                let remaining = capacity.saturating_sub(ui.cargo.metal + ui.cargo.fuel);
                ui.cargo.crystal = remaining.min(available.crystal);
            }
            WizardButtonAction::MaxFuel => {
                let remaining = capacity.saturating_sub(ui.cargo.metal + ui.cargo.crystal);
                ui.cargo.fuel = remaining.min(available.fuel);
            }
            WizardButtonAction::ClearCargo => {
                ui.cargo = ResourceStock::ZERO;
            }
            _ => continue,
        }
        ui.feedback.clear();
    }
}

fn adjust_cargo_value(cargo: ResourceStock, current: u64, delta: i64, capacity: u64) -> u64 {
    let other_total = cargo.metal + cargo.crystal + cargo.fuel - current;
    let remaining_capacity = capacity.saturating_sub(other_total);
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current.saturating_add(delta.unsigned_abs())).min(remaining_capacity)
    }
}

fn handle_wizard_step_buttons(
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            WizardButtonAction::PreviousStep => {
                let index = ui.step_index();
                if index > 0 {
                    ui.step = WIZARD_STEPS[index - 1];
                    ui.feedback.clear();
                }
            }
            WizardButtonAction::NextStep => {
                let index = ui.step_index();
                if index + 1 < WIZARD_STEPS.len() {
                    ui.step = WIZARD_STEPS[index + 1];
                    ui.feedback.clear();
                }
            }
            _ => {}
        }
    }
}

fn handle_wizard_confirm_button(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<MissionWizardState>,
    interactions: WizardButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed || *action != WizardButtonAction::Confirm {
            continue;
        }
        confirm_selected_mission(&mut simulation, &mut ui);
    }
}

fn confirm_selected_mission(simulation: &mut SimulationResource, ui: &mut MissionWizardState) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        ui.feedback = "Aucune colonie active.".to_string();
        return;
    };
    let Some(fleet_id) = ui.fleet_id else {
        ui.feedback = "Sélectionnez une flotte.".to_string();
        return;
    };
    let Some(target) = ui.target else {
        ui.feedback = "Sélectionnez une destination.".to_string();
        return;
    };
    let state = simulation.simulation().state();
    let Some(origin) = state.colony(colony_id).map(|colony| colony.system_id) else {
        ui.feedback = "Colonie introuvable.".to_string();
        return;
    };
    let Some(mission_target) = wizard_mission_target(state, target) else {
        ui.feedback = "Cible introuvable.".to_string();
        return;
    };
    let departure_at = state.clock.current_tick();
    let order = MissionOrder {
        fleet_id,
        origin,
        target: mission_target,
        kind: ui.kind,
        departure_at,
    };
    match ui.kind {
        MissionKind::Transport => {
            let WizardTarget::Colony(destination_colony_id) = target else {
                ui.feedback = "Sélectionnez une colonie de destination.".to_string();
                return;
            };
            if ui.cargo.is_zero() {
                ui.feedback = "Saisissez une cargaison avant de lancer le transport.".to_string();
                return;
            }
            apply_simulation_command(
                simulation,
                GameAction::LaunchTransport {
                    origin_colony_id: colony_id,
                    destination_colony_id,
                    fleet_id,
                    cargo: ui.cargo,
                },
            );
        }
        MissionKind::Harvest => {
            let WizardTarget::Site(site_id) = target else {
                ui.feedback = "Sélectionnez un site d'extraction.".to_string();
                return;
            };
            apply_simulation_command(
                simulation,
                GameAction::LaunchHarvest {
                    colony_id,
                    fleet_id,
                    site_id,
                },
            );
        }
        MissionKind::Probe | MissionKind::Attack | MissionKind::Colonize => {
            apply_simulation_command(simulation, GameAction::LaunchMission(order));
        }
    }
}

fn capture_mission_wizard_feedback(
    simulation: Res<SimulationResource>,
    mut ui: ResMut<MissionWizardState>,
) {
    for event in &simulation.pending_events {
        if let galactic_sim::GameEventKind::MissionLaunched(_) = event.kind {
            *ui = MissionWizardState::default();
        }
    }
}

type WizardPreviousButtonQuery<'w, 's> =
    Query<'w, 's, &'static mut Visibility, (With<WizardPreviousButton>, Without<WizardStepBlock>)>;

type WizardNextButtonQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<WizardNextButton>,
        Without<WizardStepBlock>,
        Without<WizardPreviousButton>,
    ),
>;

fn update_wizard_step_visibility(
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut blocks: Query<(&WizardStepBlock, &mut Visibility, &mut Node)>,
    mut previous_buttons: WizardPreviousButtonQuery,
    mut next_buttons: WizardNextButtonQuery,
    mut texts: Query<(&WizardStepLabel, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (block, mut visibility, mut node) in &mut blocks {
        let active = block.0 == ui.step;
        let next = if active {
            Visibility::Inherited
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
    let index = ui.step_index();
    for mut visibility in &mut previous_buttons {
        *visibility = if index > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut visibility in &mut next_buttons {
        *visibility = if index + 1 < WIZARD_STEPS.len() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let label = format!(
        "Étape {}/{} : {}",
        index + 1,
        WIZARD_STEPS.len(),
        wizard_step_label(ui.step),
    );
    for (_, mut text) in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn wizard_step_label(step: WizardStep) -> &'static str {
    match step {
        WizardStep::Kind => "type de mission",
        WizardStep::Destination => "destination",
        WizardStep::Fleet => "sélection de flotte",
        WizardStep::Params => "cargaison / paramètres",
        WizardStep::Preview => "route, durée et validation",
    }
}

fn wizard_no_destination_hint(kind: MissionKind) -> &'static str {
    match kind {
        MissionKind::Probe => {
            "Aucune cible de reconnaissance : il faut d'abord détecter un système ou une planète \
             (portée de détection, technologies de détection)."
        }
        MissionKind::Attack => {
            "Aucune cible d'attaque : il faut une planète analysée et occupée par une faction hostile."
        }
        MissionKind::Colonize => {
            "Aucune planète colonisable : il faut une planète analysée, libre, habitable et accessible."
        }
        MissionKind::Harvest => {
            "Aucun site d'extraction disponible : il faut une planète analysée avec un site non \
             colonisé, non réservé et la technologie Prospection autonome."
        }
        MissionKind::Transport => "Une deuxième colonie est nécessaire pour un transport.",
    }
}

fn update_wizard_origin_text(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&WizardOriginText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    let label = active_colony(simulation.simulation())
        .map(|colony| format!("Origine : C{} {}", colony.id.raw(), colony.name))
        .unwrap_or_else(|| "Origine : aucune colonie active".to_string());
    for (_, mut text) in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_wizard_kind_buttons(
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut buttons: Query<(
        &WizardKindButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (button, interaction, mut background, mut outline) in &mut buttons {
        let selected = button.0 == ui.kind;
        background.0 = action_button_color(true, selected, interaction);
        outline.color = action_button_outline(true, selected, interaction);
    }
}

fn update_wizard_target_rows(
    simulation: Res<SimulationResource>,
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&mut WizardTargetRow, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text, Without<WizardDestinationHintText>>,
    mut hints: Query<(&WizardDestinationHintText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.step != WizardStep::Destination {
        return;
    }
    let candidates = wizard_target_candidates(simulation.simulation(), ui.kind);

    let hint = if candidates.is_empty() {
        wizard_no_destination_hint(ui.kind)
    } else {
        ""
    };
    for (_, mut text) in &mut hints {
        if text.0 != hint {
            text.0 = hint.to_string();
        }
    }

    for (mut row, mut visibility, children) in &mut rows {
        if let Some((target, label)) = candidates.get(row.slot) {
            row.binding = Some(*target);
            *visibility = Visibility::Inherited;
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != *label
                {
                    text.0 = label.clone();
                }
            }
        } else {
            row.binding = None;
            *visibility = Visibility::Hidden;
        }
    }
}

fn update_wizard_fleet_rows(
    simulation: Res<SimulationResource>,
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(&mut WizardFleetRow, &mut Visibility, &Children)>,
    mut texts: Query<&mut Text, Without<WizardFleetHintText>>,
    mut hints: Query<(&WizardFleetHintText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.step != WizardStep::Fleet {
        return;
    }
    let sim = simulation.simulation();
    let state = sim.state();
    let Some(colony_id) = state.active_colony_id else {
        for (mut row, mut visibility, _) in &mut rows {
            row.binding = None;
            *visibility = Visibility::Hidden;
        }
        set_wizard_fleet_hint(&mut hints, "Aucune colonie active.");
        return;
    };
    let Some(target) = ui
        .target
        .and_then(|target| wizard_mission_target(state, target))
    else {
        for (mut row, mut visibility, _) in &mut rows {
            row.binding = None;
            *visibility = Visibility::Hidden;
        }
        set_wizard_fleet_hint(
            &mut hints,
            "Sélectionnez une destination à l'étape précédente.",
        );
        return;
    };
    let fleets = eligible_fleets_for_mission(
        state,
        sim.universe_repository(),
        state.player_faction,
        colony_id,
        target,
        ui.kind,
    );
    let labels = fleets
        .iter()
        .map(|fleet_id| {
            let fleet = state.fleet(*fleet_id).expect("eligible fleet exists");
            format!(
                "#{} {} — {}",
                fleet_id.raw(),
                fleet_composition_summary(fleet),
                fleet_location_label(sim, fleet.location),
            )
        })
        .collect::<Vec<_>>();

    if fleets.is_empty() {
        set_wizard_fleet_hint(
            &mut hints,
            "Aucune flotte disponible pour ce type de mission. Formez d'abord une flotte adaptée (dockée à l'origine, idle, sans cargaison en cours) dans l'onglet FLOTTES.",
        );
    } else {
        set_wizard_fleet_hint(&mut hints, "");
    }

    for (mut row, mut visibility, children) in &mut rows {
        if let Some(fleet_id) = fleets.get(row.slot) {
            row.binding = Some(*fleet_id);
            *visibility = Visibility::Inherited;
            let label = &labels[row.slot];
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != *label
                {
                    text.0 = label.clone();
                }
            }
        } else {
            row.binding = None;
            *visibility = Visibility::Hidden;
        }
    }
}

fn set_wizard_fleet_hint(hints: &mut Query<(&WizardFleetHintText, &mut Text)>, message: &str) {
    for (_, mut text) in hints.iter_mut() {
        if text.0 != message {
            text.0 = message.to_string();
        }
    }
}

fn update_wizard_params_text(
    simulation: Res<SimulationResource>,
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&WizardParamsText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.step != WizardStep::Params {
        return;
    }
    let sim = simulation.simulation();
    let state = sim.state();
    let capacity = wizard_selected_fleet_capacity(&simulation, &ui);
    let used = ui.cargo.metal + ui.cargo.crystal + ui.cargo.fuel;

    let label = match ui.kind {
        MissionKind::Transport => {
            format!(
                "Capacité utilisée : {used} / {capacity}\nMétal {}, Cristal {}, Carburant {}",
                ui.cargo.metal, ui.cargo.crystal, ui.cargo.fuel,
            )
        }
        MissionKind::Harvest => match ui.target {
            Some(WizardTarget::Site(site_id)) => {
                match (state.extraction_site(site_id), ui.fleet_id) {
                    (Some(site), Some(fleet_id)) => {
                        let rule = sim
                            .universe_repository()
                            .planet(site.planet_id)
                            .map(|planet| extraction_rules().rule_for(planet.kind));
                        let recoverable = harvest_recoverable_quantity(
                            state,
                            sim.universe_repository(),
                            site_id,
                            fleet_id,
                        )
                        .unwrap_or(0);
                        match rule {
                            Some(rule) => format!(
                                "Réserve du site : {}\nRendement : {} / tick sur {} ticks\nQuantité récupérable : {}",
                                site.remaining,
                                rule.yield_per_tick,
                                rule.harvest_ticks,
                                recoverable,
                            ),
                            None => format!("Réserve du site : {}", site.remaining),
                        }
                    }
                    (Some(site), None) => format!(
                        "Réserve du site : {}\nSélectionnez une flotte pour estimer la quantité récupérable.",
                        site.remaining,
                    ),
                    (None, _) => "Site d'extraction introuvable.".to_string(),
                }
            }
            _ => "Sélectionnez un site à l'étape précédente.".to_string(),
        },
        MissionKind::Probe | MissionKind::Attack | MissionKind::Colonize => {
            "Aucun paramètre requis pour ce type de mission.".to_string()
        }
    };
    for (_, mut text) in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_wizard_preview_text(
    simulation: Res<SimulationResource>,
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&WizardPreviewText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet || ui.step != WizardStep::Preview {
        return;
    }
    let sim = simulation.simulation();
    let state = sim.state();
    let label = wizard_preview_label(sim, state, &ui);
    for (_, mut text) in &mut texts {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn wizard_preview_label(
    sim: &Simulation,
    state: &galactic_sim::GameState,
    ui: &MissionWizardState,
) -> String {
    let Some(colony_id) = state.active_colony_id else {
        return "Aucune colonie active.".to_string();
    };
    let Some(fleet_id) = ui.fleet_id else {
        return "Sélectionnez une flotte à l'étape précédente.".to_string();
    };
    let Some(target) = ui.target else {
        return "Sélectionnez une destination.".to_string();
    };
    let Some(origin) = state.colony(colony_id).map(|colony| colony.system_id) else {
        return "Colonie introuvable.".to_string();
    };
    let Some(mission_target) = wizard_mission_target(state, target) else {
        return "Cible introuvable.".to_string();
    };
    let order = MissionOrder {
        fleet_id,
        origin,
        target: mission_target,
        kind: ui.kind,
        departure_at: state.clock.current_tick(),
    };
    match plan_mission(
        state,
        sim.universe_repository(),
        state.player_faction,
        order,
    ) {
        Ok((_, plan)) => format!(
            "Type : {}\nSauts : {}\nDurée aller : {}\nDurée sur place : {}\nDurée retour : {}\nCoût carburant : {}\nArrivée prévue : tick {}",
            mission_kind_label(ui.kind),
            plan.hops,
            format_strategic_duration(plan.travel_duration),
            format_strategic_duration(plan.resolution_duration),
            format_strategic_duration(plan.travel_duration),
            plan.fuel_cost.fuel,
            plan.outbound_arrival_at.value(),
        ),
        Err(error) => format!("Erreur : {}", mission_error_text(error)),
    }
}

fn update_wizard_feedback_text(
    ui: Res<MissionWizardState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&WizardFeedbackText, &mut Text)>,
) {
    if *open_panel != OpenPanel::Fleet {
        return;
    }
    for (_, mut text) in &mut texts {
        if text.0 != ui.feedback {
            text.0 = ui.feedback.clone();
        }
    }
}

fn wizard_selected_fleet_capacity(simulation: &SimulationResource, ui: &MissionWizardState) -> u64 {
    ui.fleet_id
        .and_then(|fleet_id| simulation.simulation().state().fleet(fleet_id))
        .and_then(|fleet| fleet.capabilities().ok())
        .map(|capabilities| capabilities.cargo_capacity)
        .unwrap_or(0)
}

fn wizard_mission_target(
    state: &galactic_sim::GameState,
    target: WizardTarget,
) -> Option<MissionTarget> {
    match target {
        WizardTarget::System(system_id) => Some(MissionTarget::System(system_id)),
        WizardTarget::Planet {
            system_id,
            planet_id,
        } => Some(MissionTarget::Planet {
            system_id,
            planet_id,
        }),
        WizardTarget::Colony(colony_id) => {
            let colony = state.colony(colony_id)?;
            Some(MissionTarget::Planet {
                system_id: colony.system_id,
                planet_id: colony.planet_id,
            })
        }
        WizardTarget::Site(site_id) => {
            let site = state.extraction_site(site_id)?;
            Some(MissionTarget::Planet {
                system_id: site.system_id,
                planet_id: site.planet_id,
            })
        }
    }
}

fn wizard_target_candidates(
    simulation: &Simulation,
    kind: MissionKind,
) -> Vec<(WizardTarget, String)> {
    let mut candidates = match kind {
        MissionKind::Probe => probe_candidates(simulation),
        MissionKind::Attack => attack_candidates(simulation),
        MissionKind::Colonize => colonize_candidates(simulation),
        MissionKind::Harvest => harvest_candidates(simulation),
        MissionKind::Transport => transport_candidates(simulation),
    };
    candidates.truncate(MAX_TARGET_ROWS);
    candidates
}

fn probe_candidates(simulation: &Simulation) -> Vec<(WizardTarget, String)> {
    let state = simulation.state();
    let universe = simulation.universe_repository();
    let mut candidates = Vec::new();
    for system in &universe.definition().systems {
        if state.system_knowledge_level(system.id) == KnowledgeLevel::Detected {
            candidates.push((
                WizardTarget::System(system.id),
                format!("Signal {} (système détecté)", system.id.index()),
            ));
        }
        for (orbit_index, planet) in system.planets.iter().enumerate() {
            if state.planet_knowledge_level(planet.id) == KnowledgeLevel::Detected {
                candidates.push((
                    WizardTarget::Planet {
                        system_id: system.id,
                        planet_id: planet.id,
                    },
                    format!(
                        "{} — {}",
                        provisional_planet_label(&system.name, orbit_index),
                        system.name,
                    ),
                ));
            }
        }
    }
    candidates
}

fn attack_candidates(simulation: &Simulation) -> Vec<(WizardTarget, String)> {
    let state = simulation.state();
    let universe = simulation.universe_repository();
    let mut candidates = Vec::new();
    for system in &universe.definition().systems {
        for planet in &system.planets {
            if state.planet_knowledge_level(planet.id) < KnowledgeLevel::Analyzed {
                continue;
            }
            let Some(report) = state.planetary_intelligence_report(planet.id) else {
                continue;
            };
            let PlanetaryOccupancyIntel::Occupied(occupant) = report.occupancy else {
                continue;
            };
            if occupant == state.player_faction {
                continue;
            }
            candidates.push((
                WizardTarget::Planet {
                    system_id: system.id,
                    planet_id: planet.id,
                },
                format!("{} — {} (occupée, hostile)", planet.name, system.name),
            ));
        }
    }
    candidates
}

fn colonize_candidates(simulation: &Simulation) -> Vec<(WizardTarget, String)> {
    let state = simulation.state();
    let universe = simulation.universe_repository();
    let actor = state.player_faction;
    let mut candidates = Vec::new();
    for system in &universe.definition().systems {
        for planet in &system.planets {
            if state.planet_knowledge_level(planet.id) < KnowledgeLevel::Analyzed {
                continue;
            }
            if !assess_planet_colonizability(state, universe, actor, planet.id).is_colonizable() {
                continue;
            }
            candidates.push((
                WizardTarget::Planet {
                    system_id: system.id,
                    planet_id: planet.id,
                },
                format!("{} — {} (libre)", planet.name, system.name),
            ));
        }
    }
    candidates
}

fn harvest_candidates(simulation: &Simulation) -> Vec<(WizardTarget, String)> {
    let state = simulation.state();
    let universe = simulation.universe_repository();
    let mut candidates = Vec::new();
    for site in &state.extraction_sites {
        if site.is_depleted() || site.reserved_by.is_some() {
            continue;
        }
        if state.planet_knowledge_level(site.planet_id) < KnowledgeLevel::Analyzed {
            continue;
        }
        if state.colony_on_planet(site.planet_id).is_some() {
            continue;
        }
        if !state
            .research
            .has_unlock(TechnologyUnlock::RemoteExtraction)
        {
            continue;
        }
        let Some((_, planet)) = universe.planet_location(site.planet_id) else {
            continue;
        };
        let system_name = universe
            .system(site.system_id)
            .map(|system| system.name.clone())
            .unwrap_or_default();
        candidates.push((
            WizardTarget::Site(site.id),
            format!(
                "{} — {} ({}, libre)",
                planet.name,
                system_name,
                resource_kind_label(site.resource),
            ),
        ));
    }
    candidates
}

fn transport_candidates(simulation: &Simulation) -> Vec<(WizardTarget, String)> {
    let state = simulation.state();
    let active = state.active_colony_id;
    state
        .player_colony_ids()
        .into_iter()
        .filter(|colony_id| Some(*colony_id) != active)
        .filter_map(|colony_id| {
            state.colony(colony_id).map(|colony| {
                (
                    WizardTarget::Colony(colony_id),
                    format!("C{} {}", colony_id.raw(), colony.name),
                )
            })
        })
        .collect()
}

fn resource_kind_label(kind: galactic_domain::ResourceKind) -> &'static str {
    match kind {
        galactic_domain::ResourceKind::Metal => "métal",
        galactic_domain::ResourceKind::Crystal => "cristal",
        galactic_domain::ResourceKind::Fuel => "carburant",
        galactic_domain::ResourceKind::Energy => "énergie",
    }
}

#[cfg(test)]
mod tests {
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
        capture_mission_wizard_feedback_queries_are_disjoint,
        capture_mission_wizard_feedback
    );
    assert_disjoint_queries!(
        handle_wizard_kind_buttons_queries_are_disjoint,
        handle_wizard_kind_buttons
    );
    assert_disjoint_queries!(
        handle_wizard_target_buttons_queries_are_disjoint,
        handle_wizard_target_buttons
    );
    assert_disjoint_queries!(
        handle_wizard_fleet_buttons_queries_are_disjoint,
        handle_wizard_fleet_buttons
    );
    assert_disjoint_queries!(
        handle_wizard_cargo_buttons_queries_are_disjoint,
        handle_wizard_cargo_buttons
    );
    assert_disjoint_queries!(
        handle_wizard_step_buttons_queries_are_disjoint,
        handle_wizard_step_buttons
    );
    assert_disjoint_queries!(
        handle_wizard_confirm_button_queries_are_disjoint,
        handle_wizard_confirm_button
    );
    assert_disjoint_queries!(
        update_wizard_step_visibility_queries_are_disjoint,
        update_wizard_step_visibility
    );
    assert_disjoint_queries!(
        update_wizard_origin_text_queries_are_disjoint,
        update_wizard_origin_text
    );
    assert_disjoint_queries!(
        update_wizard_kind_buttons_queries_are_disjoint,
        update_wizard_kind_buttons
    );
    assert_disjoint_queries!(
        update_wizard_target_rows_queries_are_disjoint,
        update_wizard_target_rows
    );
    assert_disjoint_queries!(
        update_wizard_fleet_rows_queries_are_disjoint,
        update_wizard_fleet_rows
    );
    assert_disjoint_queries!(
        update_wizard_params_text_queries_are_disjoint,
        update_wizard_params_text
    );
    assert_disjoint_queries!(
        update_wizard_preview_text_queries_are_disjoint,
        update_wizard_preview_text
    );
    assert_disjoint_queries!(
        update_wizard_feedback_text_queries_are_disjoint,
        update_wizard_feedback_text
    );
}

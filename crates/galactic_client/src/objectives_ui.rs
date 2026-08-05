// MVP-032-A: session-local Consortium objectives. Persistence is intentionally
// deferred until MVP-031 is resumed.
use std::collections::BTreeSet;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use galactic_domain::ResourceStock;
use galactic_sim::{
    AttackMissionOutcome, BuildingKind, ColonizationBlocker, CombatOutcome, CraftableId, GameState,
    KnowledgeLevel, MissionKind, MissionReportOutcome, MissionResult, PlanetaryOccupancyIntel,
    TechnologyId, UniverseRepository, VictoryConditionProgress, VictoryProgress,
    assess_planet_colonizability, combat_rules, craftable_definition, evaluate_victory_progress,
    storage_capacity, technology_definition, victory_rules,
};

use crate::presentation::{
    components::{ScrollIndicatorArea, ScrollIndicatorId},
    scene::spawn_scroll_indicator,
};

use super::{
    OpenPanel, PresentationUpdateSet, SimulationResource, UiPointerBlocker,
    collect_presentation_events, panel_background, panel_outline, ui_text_font,
};

const OBJECTIVES_Z_INDEX: i32 = 125;

pub(crate) struct ObjectivesUiPlugin;

impl Plugin for ObjectivesUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjectiveProgressState>()
            .init_resource::<ObjectiveUiState>()
            .add_systems(Startup, spawn_objectives_screen)
            .add_systems(
                Update,
                sync_objective_progress
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (handle_objective_shortcuts, handle_objective_buttons)
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_objectives_visibility,
                    update_objective_rows,
                    update_objective_texts,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectiveCategory {
    Campaign,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ObjectiveId {
    OpenObjectivesPanel,
    UpgradeProduction,
    BuildLaboratory,
    StartResearch,
    BuildProbe,
    ProbePlanet,
    BuildSatellite,
    AnalyzePlanet,
    HarvestSite,
    BuildMilitaryFleet,
    ResolveObstacle,
    UnlockColonization,
    BuildColonyShip,
    FoundSecondColony,
    ProbeDistantSystem,
    AnalyzeOccupiedDistant,
    HarvestValuableSite,
    WinMixedFleetCombat,
    FoundRiskColony,
}

const CAMPAIGN_OBJECTIVES: [ObjectiveId; 14] = [
    ObjectiveId::OpenObjectivesPanel,
    ObjectiveId::UpgradeProduction,
    ObjectiveId::BuildLaboratory,
    ObjectiveId::StartResearch,
    ObjectiveId::BuildProbe,
    ObjectiveId::ProbePlanet,
    ObjectiveId::BuildSatellite,
    ObjectiveId::AnalyzePlanet,
    ObjectiveId::HarvestSite,
    ObjectiveId::BuildMilitaryFleet,
    ObjectiveId::ResolveObstacle,
    ObjectiveId::UnlockColonization,
    ObjectiveId::BuildColonyShip,
    ObjectiveId::FoundSecondColony,
];

const OPTIONAL_OBJECTIVES: [ObjectiveId; 5] = [
    ObjectiveId::ProbeDistantSystem,
    ObjectiveId::AnalyzeOccupiedDistant,
    ObjectiveId::HarvestValuableSite,
    ObjectiveId::WinMixedFleetCombat,
    ObjectiveId::FoundRiskColony,
];

#[derive(Debug, Clone, Copy)]
struct ObjectiveDefinition {
    id: ObjectiveId,
    category: ObjectiveCategory,
    title: &'static str,
    briefing: &'static str,
    condition: &'static str,
    reward: Option<ObjectiveReward>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectiveReward {
    stock: ResourceStock,
}

#[derive(Resource)]
pub(crate) struct ObjectiveProgressState {
    completed_objectives: BTreeSet<ObjectiveId>,
    claimed_rewards: BTreeSet<ObjectiveId>,
    selected_objective: ObjectiveId,
    opened_objectives_panel: bool,
    feedback: String,
}

impl Default for ObjectiveProgressState {
    fn default() -> Self {
        Self {
            completed_objectives: BTreeSet::new(),
            claimed_rewards: BTreeSet::new(),
            selected_objective: ObjectiveId::OpenObjectivesPanel,
            opened_objectives_panel: false,
            feedback: String::new(),
        }
    }
}

#[derive(Resource)]
pub(crate) struct ObjectiveUiState {
    hints_visible: bool,
}

impl Default for ObjectiveUiState {
    fn default() -> Self {
        Self {
            hints_visible: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectiveButtonAction {
    Toggle,
    Close,
    ToggleHints,
    Select(ObjectiveId),
}

type ObjectiveButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ObjectiveButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct ObjectivesRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectiveTextRole {
    Toggle,
    Title,
    Briefing,
    VictoryDirective,
    Current,
    Detail,
    Hints,
    HintsButton,
    CloseButton,
    Feedback,
}

#[derive(Component)]
struct ObjectiveRow {
    id: ObjectiveId,
}

#[derive(Component)]
struct ObjectiveRowText {
    id: ObjectiveId,
}

pub(crate) fn spawn_objectives_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.13, 0.11, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, objective_accent()),
            ObjectiveButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Objectifs  [O]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.82, 0.96, 0.88)),
                ObjectiveTextRole::Toggle,
            ));
        });
}

fn spawn_objectives_screen(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(96.0),
                bottom: Val::Px(74.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(9.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.010, 0.020, 0.018, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.48, 0.76, 0.58, 0.72),
            ),
            Visibility::Hidden,
            GlobalZIndex(OBJECTIVES_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            ObjectivesRoot,
        ))
        .with_children(|root| {
            spawn_objectives_header(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.74, 0.88, 0.78)),
                Node {
                    min_height: Val::Px(24.0),
                    ..default()
                },
                ObjectiveTextRole::Briefing,
            ));
            root.spawn((
                Text::new(""),
                ui_text_font(10.5),
                TextColor(Color::srgb(0.78, 0.92, 0.80)),
                Node {
                    min_height: Val::Px(52.0),
                    ..default()
                },
                ObjectiveTextRole::VictoryDirective,
            ));
            spawn_objectives_main(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.94, 0.78, 0.44)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ObjectiveTextRole::Feedback,
            ));
        });
}

fn spawn_objectives_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("OBJECTIFS DU CONSORTIUM"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.84, 0.96, 0.88)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ObjectiveTextRole::Title,
            ));
            spawn_objective_small_button(
                header,
                "Conseils",
                ObjectiveButtonAction::ToggleHints,
                112.0,
                ObjectiveTextRole::HintsButton,
            );
            spawn_objective_small_button(
                header,
                "Fermer  [O / Échap]",
                ObjectiveButtonAction::Close,
                170.0,
                ObjectiveTextRole::CloseButton,
            );
        });
}

fn spawn_objective_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ObjectiveButtonAction,
    width: f32,
    text_role: ObjectiveTextRole,
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
            BackgroundColor(Color::srgba(0.04, 0.08, 0.06, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.42, 0.78, 0.54, 0.58),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.84, 0.94, 0.86)),
                text_role,
            ));
        });
}

fn spawn_objectives_main(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(9.0),
        ..default()
    },))
        .with_children(|row| {
            spawn_objective_list(row);
            spawn_objective_detail(row);
        });
}

fn spawn_objective_list(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(420.0),
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
                    row_gap: Val::Px(7.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                ScrollIndicatorArea {
                    id: ScrollIndicatorId::ObjectiveList,
                },
            ))
            .with_children(|list| {
                list.spawn((
                    Text::new("MANDAT PRINCIPAL"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.78, 0.94, 0.82)),
                ));
                for id in CAMPAIGN_OBJECTIVES {
                    spawn_objective_row(list, id);
                }
                list.spawn((
                    Text::new("MANDATS FACULTATIFS"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.94, 0.86, 0.62)),
                    Node {
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    },
                ));
                for id in OPTIONAL_OBJECTIVES {
                    spawn_objective_row(list, id);
                }
            });
        spawn_scroll_indicator(frame, ScrollIndicatorId::ObjectiveList);
    });
}

fn spawn_objective_row(parent: &mut ChildSpawnerCommands, id: ObjectiveId) {
    let definition = objective_definition(id);
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(42.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.040, 0.036, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.22, 0.34, 0.26, 0.76),
            ),
            ObjectiveButtonAction::Select(id),
            ObjectiveRow { id },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(definition.title),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.78, 0.88, 0.80)),
                ObjectiveRowText { id },
            ));
        });
}

fn spawn_objective_detail(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.018, 0.028, 0.026, 0.96)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.34, 0.58, 0.42, 0.60),
        ),
    ))
    .with_children(|detail| {
        detail.spawn((
            Text::new(""),
            ui_text_font(14.0),
            TextColor(Color::srgb(0.88, 0.98, 0.90)),
            ObjectiveTextRole::Current,
        ));
        detail
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
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
                            padding: UiRect::right(Val::Px(12.0)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollPosition::default(),
                        RelativeCursorPosition::default(),
                        ScrollIndicatorArea {
                            id: ScrollIndicatorId::ObjectiveDetail,
                        },
                    ))
                    .with_children(|scroll| {
                        scroll.spawn((
                            Text::new(""),
                            ui_text_font(12.0),
                            TextColor(Color::srgb(0.78, 0.88, 0.82)),
                            Node {
                                width: Val::Percent(100.0),
                                ..default()
                            },
                            ObjectiveTextRole::Detail,
                        ));
                    });
                spawn_scroll_indicator(frame, ScrollIndicatorId::ObjectiveDetail);
            });
        detail.spawn((
            Text::new(""),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.70, 0.82, 0.74)),
            ObjectiveTextRole::Hints,
        ));
    });
}

fn handle_objective_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut progress: ResMut<ObjectiveProgressState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    save_load_ui: Res<crate::save_load_ui::SaveLoadUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
        || crate::save_load_ui::save_name_is_editing(&save_load_ui)
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        let opening = *open_panel != OpenPanel::Objectives;
        *open_panel = if opening {
            OpenPanel::Objectives
        } else {
            OpenPanel::None
        };
        if opening {
            progress.opened_objectives_panel = true;
            progress.feedback.clear();
            navigation_ui.search_open = false;
            navigation_ui.filters_open = false;
        }
        return;
    }

    if *open_panel == OpenPanel::Objectives && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
    }
}

fn handle_objective_buttons(
    mut progress: ResMut<ObjectiveProgressState>,
    mut ui: ResMut<ObjectiveUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: ObjectiveButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            ObjectiveButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::Objectives;
                *open_panel = if opening {
                    OpenPanel::Objectives
                } else {
                    OpenPanel::None
                };
                if opening {
                    progress.opened_objectives_panel = true;
                    progress.feedback.clear();
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            ObjectiveButtonAction::Close => {
                *open_panel = OpenPanel::None;
            }
            ObjectiveButtonAction::ToggleHints => {
                ui.hints_visible = !ui.hints_visible;
            }
            ObjectiveButtonAction::Select(id) => {
                progress.selected_objective = id;
            }
        }
    }
}

fn sync_objective_progress(
    mut simulation: ResMut<SimulationResource>,
    mut progress: ResMut<ObjectiveProgressState>,
) {
    let completed_now = {
        let simulation = simulation.simulation();
        let state = simulation.state();
        let universe = simulation.universe_repository();
        all_objective_ids()
            .iter()
            .copied()
            .filter(|id| {
                !progress.completed_objectives.contains(id)
                    && objective_is_complete(state, universe, &progress, *id)
            })
            .collect::<Vec<_>>()
    };

    if completed_now.is_empty() {
        return;
    }

    for id in completed_now.iter().copied() {
        progress.completed_objectives.insert(id);
    }
    let current_selected = progress.selected_objective;
    if progress.completed_objectives.contains(&current_selected) {
        progress.selected_objective =
            current_campaign_objective(&progress).unwrap_or(current_selected);
    }

    let mut feedback = Vec::new();
    for id in completed_now {
        let definition = objective_definition(id);
        if let Some(reward) = definition.reward {
            if progress.claimed_rewards.insert(id) {
                let credited = grant_reward(&mut simulation, reward);
                feedback.push(format!(
                    "{} validé — dotation accordée : {}.",
                    definition.title,
                    format_stock(credited),
                ));
            }
        } else {
            feedback.push(format!("{} validé.", definition.title));
        }
    }
    progress.feedback = feedback.join(" ");
}

fn update_objectives_visibility(
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<ObjectivesRoot>>,
    mut texts: Query<(&ObjectiveTextRole, &mut Text)>,
) {
    let is_open = *open_panel == OpenPanel::Objectives;
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
    for (role, mut text) in &mut texts {
        if *role == ObjectiveTextRole::Toggle {
            let next = if is_open {
                "Fermer objectifs".to_string()
            } else {
                "Objectifs  [O]".to_string()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
    }
}

fn update_objective_rows(
    progress: Res<ObjectiveProgressState>,
    open_panel: Res<OpenPanel>,
    mut rows: Query<(
        &ObjectiveRow,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
    mut labels: Query<(&ObjectiveRowText, &mut Text, &mut TextColor)>,
) {
    if *open_panel != OpenPanel::Objectives {
        return;
    }

    for (row, interaction, mut background, mut outline) in &mut rows {
        let selected = row.id == progress.selected_objective;
        let completed = progress.completed_objectives.contains(&row.id);
        background.0 = objective_row_color(selected, completed, interaction);
        outline.color = objective_row_outline(selected, completed);
    }

    for (label, mut text, mut color) in &mut labels {
        let definition = objective_definition(label.id);
        let marker = if progress.completed_objectives.contains(&label.id) {
            "[VALIDÉ]"
        } else if label.id == progress.selected_objective {
            "[EN COURS]"
        } else {
            "[À TRAITER]"
        };
        text.0 = format!("{marker} {}", definition.title);
        color.0 = if progress.completed_objectives.contains(&label.id) {
            Color::srgb(0.58, 0.94, 0.68)
        } else if label.id == progress.selected_objective {
            Color::srgb(0.90, 0.98, 0.92)
        } else {
            Color::srgb(0.74, 0.84, 0.76)
        };
    }
}

fn update_objective_texts(
    simulation: Res<SimulationResource>,
    progress: Res<ObjectiveProgressState>,
    ui: Res<ObjectiveUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ObjectiveTextRole, &mut Text, &mut TextColor)>,
) {
    if *open_panel != OpenPanel::Objectives {
        return;
    }
    let state = simulation.simulation().state();
    let universe = simulation.simulation().universe_repository();
    let victory_progress = evaluate_victory_progress(state, universe, victory_rules());
    let current = current_campaign_objective(&progress);
    let selected = objective_definition(progress.selected_objective);
    let completed_campaign = CAMPAIGN_OBJECTIVES
        .iter()
        .filter(|id| progress.completed_objectives.contains(id))
        .count();
    let completed_optional = OPTIONAL_OBJECTIVES
        .iter()
        .filter(|id| progress.completed_objectives.contains(id))
        .count();

    for (role, mut text, mut color) in &mut texts {
        match role {
            ObjectiveTextRole::Title => {
                text.0 = format!(
                    "OBJECTIFS DU CONSORTIUM — {}/{} mandat(s)",
                    completed_campaign,
                    CAMPAIGN_OBJECTIVES.len(),
                );
                color.0 = Color::srgb(0.84, 0.96, 0.88);
            }
            ObjectiveTextRole::Briefing => {
                text.0 = "Directive : convertir l'incertitude galactique en stabilité exploitable. Les besoins du Consortium restent prioritaires, surtout lorsqu'ils deviennent commodes.".to_string();
            }
            ObjectiveTextRole::VictoryDirective => {
                text.0 = victory_objectives_text(victory_progress);
                color.0 = if victory_progress.is_complete() {
                    Color::srgb(0.90, 1.0, 0.72)
                } else {
                    Color::srgb(0.78, 0.92, 0.80)
                };
            }
            ObjectiveTextRole::Current => {
                text.0 = current
                    .map(|id| format!("Mandat courant : {}", objective_definition(id).title))
                    .unwrap_or_else(|| {
                        "Mandat principal accompli : deuxième prospérité déclarée.".to_string()
                    });
            }
            ObjectiveTextRole::Detail => {
                text.0 = objective_detail_text(state, &progress, selected);
                color.0 = Color::srgb(0.80, 0.90, 0.84);
            }
            ObjectiveTextRole::Hints => {
                text.0 = if ui.hints_visible {
                    objective_hint_text(selected.id)
                } else {
                    "Conseils masqués par décision administrative.".to_string()
                };
                color.0 = if ui.hints_visible {
                    Color::srgb(0.70, 0.82, 0.74)
                } else {
                    Color::srgb(0.54, 0.62, 0.56)
                };
            }
            ObjectiveTextRole::HintsButton => {
                text.0 = if ui.hints_visible {
                    "Masquer conseils".to_string()
                } else {
                    "Afficher conseils".to_string()
                };
            }
            ObjectiveTextRole::CloseButton => {}
            ObjectiveTextRole::Feedback => {
                text.0 = if progress.feedback.is_empty() {
                    format!(
                        "Mandats facultatifs validés : {}/{}.",
                        completed_optional,
                        OPTIONAL_OBJECTIVES.len(),
                    )
                } else {
                    progress.feedback.clone()
                };
            }
            ObjectiveTextRole::Toggle => {}
        }
    }
}

fn victory_objectives_text(progress: VictoryProgress) -> String {
    let completed = victory_completed_conditions(progress);
    let technology = technology_definition(victory_rules().required_technology);
    format!(
        "Directive régionale : {completed}/6 critères validés.\n{} · {} · {} {}\n{} · {} · {}",
        compact_progress("colonies", progress.colonies),
        compact_progress("systèmes sondés", progress.probed_systems),
        technology.name,
        if progress.required_technology.complete() {
            "validée"
        } else {
            "en attente"
        },
        compact_progress("récoltes", progress.completed_harvests),
        compact_progress("analyses Sylves", progress.sylve_analysis_reports),
        compact_progress("sécurisations Sylves", progress.sylve_attack_victories),
    )
}

fn compact_progress(label: &str, progress: VictoryConditionProgress) -> String {
    format!("{label} {}/{}", progress.current, progress.required)
}

fn victory_completed_conditions(progress: VictoryProgress) -> usize {
    usize::from(progress.colonies.complete())
        + usize::from(progress.probed_systems.complete())
        + usize::from(progress.required_technology.complete())
        + usize::from(progress.completed_harvests.complete())
        + usize::from(progress.sylve_analysis_reports.complete())
        + usize::from(progress.sylve_attack_victories.complete())
}

fn grant_reward(simulation: &mut SimulationResource, reward: ObjectiveReward) -> ResourceStock {
    let state = simulation.simulation.state_mut();
    let colony_id = state
        .active_player_colony()
        .or_else(|| state.player_home_colony())
        .map(|colony| colony.id);
    let Some(colony_id) = colony_id else {
        return ResourceStock::ZERO;
    };
    let Some(colony) = state.colony_mut(colony_id) else {
        return ResourceStock::ZERO;
    };
    let capacity = storage_capacity(colony.buildings);
    colony.resources.credit_capped(reward.stock, capacity)
}

fn objective_detail_text(
    state: &GameState,
    progress: &ObjectiveProgressState,
    definition: ObjectiveDefinition,
) -> String {
    let status = if progress.completed_objectives.contains(&definition.id) {
        "Statut : validé"
    } else {
        "Statut : en attente"
    };
    let reward = definition
        .reward
        .map(|reward| format_stock(reward.stock))
        .unwrap_or_else(|| "aucune dotation".to_string());
    let category = match definition.category {
        ObjectiveCategory::Campaign => "Mandat principal",
        ObjectiveCategory::Optional => "Mandat facultatif",
    };
    format!(
        "{category}\n\
{status}\n\
\n\
{}\n\
\n\
Condition : {}\n\
Récompense : {}\n\
\n\
Situation actuelle : {}",
        definition.briefing,
        definition.condition,
        reward,
        objective_status_text(state, progress, definition.id),
    )
}

fn objective_status_text(
    state: &GameState,
    progress: &ObjectiveProgressState,
    id: ObjectiveId,
) -> String {
    match id {
        ObjectiveId::OpenObjectivesPanel => {
            if progress.opened_objectives_panel {
                "panneau Objectifs ouvert".to_string()
            } else {
                "panneau Objectifs non consulté".to_string()
            }
        }
        ObjectiveId::UpgradeProduction => {
            let best = player_best_level(
                state,
                [
                    BuildingKind::METAL_MINE,
                    BuildingKind::CRYSTAL_EXTRACTOR,
                    BuildingKind::FUEL_REFINERY,
                    BuildingKind::POWER_PLANT,
                ],
            );
            format!("meilleur niveau de production : {best}")
        }
        ObjectiveId::BuildLaboratory => format!(
            "meilleur Laboratoire : niveau {}",
            player_best_level(state, [BuildingKind::RESEARCH_LAB]),
        ),
        ObjectiveId::StartResearch => format!(
            "recherches acquises : {} ; file : {}",
            state.research.completed_count(),
            state.research.queue_len(),
        ),
        ObjectiveId::BuildProbe => format!(
            "{} disponibles ou affectés : {}",
            craftable_definition(CraftableId::LIGHT_PROBE).name,
            player_ship_count(state, CraftableId::LIGHT_PROBE),
        ),
        ObjectiveId::ProbePlanet => {
            format!("planètes sondées : {}", probed_non_colonized_count(state))
        }
        ObjectiveId::BuildSatellite => format!(
            "{} disponibles ou affectés : {}",
            craftable_definition(CraftableId::CARTOGRAPHER_SATELLITE).name,
            player_ship_count(state, CraftableId::CARTOGRAPHER_SATELLITE),
        ),
        ObjectiveId::AnalyzePlanet => format!(
            "rapports d'analyse planétaire : {}",
            state.planet_analysis_reports.len(),
        ),
        ObjectiveId::HarvestSite => format!(
            "missions de récolte rapportées : {}",
            harvest_report_count(state)
        ),
        ObjectiveId::BuildMilitaryFleet => format!(
            "flottes militaires constituées : {}",
            state
                .player_fleets()
                .filter(|fleet| combat_rules().is_combat_fleet(fleet))
                .count(),
        ),
        ObjectiveId::ResolveObstacle => {
            if attack_victory_reported(state) {
                "attaque victorieuse enregistrée".to_string()
            } else {
                "aucune attaque victorieuse enregistrée".to_string()
            }
        }
        ObjectiveId::UnlockColonization => {
            let tech = technology_definition(TechnologyId::COLONIZATION);
            if state.research.has_completed(TechnologyId::COLONIZATION) {
                format!("{} acquise", tech.name)
            } else {
                format!("{} non acquise", tech.name)
            }
        }
        ObjectiveId::BuildColonyShip => format!(
            "{} disponibles ou affectés : {}",
            craftable_definition(CraftableId::COLONY_SHIP).name,
            player_ship_count(state, CraftableId::COLONY_SHIP),
        ),
        ObjectiveId::FoundSecondColony => {
            format!("colonies du joueur : {}", state.player_colonies().count())
        }
        ObjectiveId::ProbeDistantSystem => {
            "objectif mesuré sur le graphe réel depuis Port-Sillage".to_string()
        }
        ObjectiveId::AnalyzeOccupiedDistant => {
            "rapports analysés avec occupation connue requis".to_string()
        }
        ObjectiveId::HarvestValuableSite => {
            "récolte distante avec cargaison significative requise".to_string()
        }
        ObjectiveId::WinMixedFleetCombat => {
            "rapport de combat victorieux avec flotte mixte requis".to_string()
        }
        ObjectiveId::FoundRiskColony => {
            "deuxième colonie hors voisinage immédiat ou après combat requis".to_string()
        }
    }
}

fn objective_hint_text(id: ObjectiveId) -> String {
    match id {
        ObjectiveId::OpenObjectivesPanel => {
            "Ouvre Objectifs [O] ou le bouton bas pour confirmer la prise de commandement.".to_string()
        }
        ObjectiveId::UpgradeProduction => {
            "Ouvre Gestion colonie [C], choisis une installation de production, puis lance une amélioration.".to_string()
        }
        ObjectiveId::BuildLaboratory => {
            "Le Laboratoire se construit depuis Gestion colonie après les prérequis économiques nécessaires.".to_string()
        }
        ObjectiveId::StartResearch => {
            "Ouvre Recherche techno [T] et lance une doctrine disponible.".to_string()
        }
        ObjectiveId::BuildProbe => {
            "Ouvre Chantier flotte [Y] et assemble une Sonde — Œil.".to_string()
        }
        ObjectiveId::ProbePlanet => {
            "Crée une flotte de sonde puis utilise Lancer mission depuis Flottes [V].".to_string()
        }
        ObjectiveId::BuildSatellite => {
            "Le Satellite — Veilleur demande la recherche d'analyse planétaire et un chantier prêt.".to_string()
        }
        ObjectiveId::AnalyzePlanet => {
            "Envoie le Satellite — Veilleur sur une planète déjà sondée pour produire un rapport complet.".to_string()
        }
        ObjectiveId::HarvestSite => {
            "Après analyse, choisis une mission Récolte avec une flotte cargo vide.".to_string()
        }
        ObjectiveId::BuildMilitaryFleet => {
            "Constitue une flotte avec Intercepteur — Riposte, Frégate — Garde ou Croiseur — Verdict.".to_string()
        }
        ObjectiveId::ResolveObstacle => {
            "Si une planète prioritaire est occupée, l'onglet mission peut estimer puis lancer l'attaque.".to_string()
        }
        ObjectiveId::UnlockColonization => {
            "Poursuis l'arbre Recherche techno jusqu'à Colonisation avancée.".to_string()
        }
        ObjectiveId::BuildColonyShip => {
            "Assemble une Arche coloniale — Essor quand la technologie et les ressources sont prêtes.".to_string()
        }
        ObjectiveId::FoundSecondColony => {
            "Envoie l'Arche coloniale — Essor vers une planète analysée et réglementairement compatible.".to_string()
        }
        ObjectiveId::ProbeDistantSystem => {
            "Explore au-delà du voisinage immédiat : trois sauts suffisent pour satisfaire la cartographie supérieure.".to_string()
        }
        ObjectiveId::AnalyzeOccupiedDistant => {
            "Cherche une planète occupée hors des premières routes, puis produis un rapport orbital complet.".to_string()
        }
        ObjectiveId::HarvestValuableSite => {
            "Les sites qui rapportent beaucoup justifient les formulaires de priorité logistique.".to_string()
        }
        ObjectiveId::WinMixedFleetCombat => {
            "Mélange au moins deux classes militaires dans la flotte avant d'obtenir une victoire.".to_string()
        }
        ObjectiveId::FoundRiskColony => {
            "Une colonie éloignée ou fondée après sécurisation armée prouve une saine ambition administrative.".to_string()
        }
    }
}

fn objective_is_complete(
    state: &GameState,
    universe: &UniverseRepository,
    progress: &ObjectiveProgressState,
    id: ObjectiveId,
) -> bool {
    match id {
        ObjectiveId::OpenObjectivesPanel => progress.opened_objectives_panel,
        ObjectiveId::UpgradeProduction => {
            player_best_level(
                state,
                [
                    BuildingKind::METAL_MINE,
                    BuildingKind::CRYSTAL_EXTRACTOR,
                    BuildingKind::FUEL_REFINERY,
                    BuildingKind::POWER_PLANT,
                ],
            ) >= 2
        }
        ObjectiveId::BuildLaboratory => player_best_level(state, [BuildingKind::RESEARCH_LAB]) >= 1,
        ObjectiveId::StartResearch => {
            state.research.queue_len() > 0 || state.research.completed_count() > 0
        }
        ObjectiveId::BuildProbe => player_ship_count(state, CraftableId::LIGHT_PROBE) > 0,
        ObjectiveId::ProbePlanet => probed_non_colonized_count(state) > 0,
        ObjectiveId::BuildSatellite => {
            player_ship_count(state, CraftableId::CARTOGRAPHER_SATELLITE) > 0
        }
        ObjectiveId::AnalyzePlanet => !state.planet_analysis_reports.is_empty(),
        ObjectiveId::HarvestSite => harvest_report_count(state) > 0,
        ObjectiveId::BuildMilitaryFleet => state
            .player_fleets()
            .any(|fleet| combat_rules().is_combat_fleet(fleet)),
        ObjectiveId::ResolveObstacle => {
            attack_victory_reported(state) || analyzed_clear_target_exists(state, universe)
        }
        ObjectiveId::UnlockColonization => state.research.has_completed(TechnologyId::COLONIZATION),
        ObjectiveId::BuildColonyShip => player_ship_count(state, CraftableId::COLONY_SHIP) > 0,
        ObjectiveId::FoundSecondColony => state.player_colonies().count() >= 2,
        ObjectiveId::ProbeDistantSystem => probed_distant_system_exists(state, universe, 3),
        ObjectiveId::AnalyzeOccupiedDistant => {
            analyzed_occupied_distant_planet_exists(state, universe, 2)
        }
        ObjectiveId::HarvestValuableSite => valuable_harvest_reported(state),
        ObjectiveId::WinMixedFleetCombat => mixed_fleet_victory_reported(state),
        ObjectiveId::FoundRiskColony => risky_colony_exists(state, universe),
    }
}

fn objective_definition(id: ObjectiveId) -> ObjectiveDefinition {
    match id {
        ObjectiveId::OpenObjectivesPanel => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Prise de commandement",
            briefing: "Un mandat ignoré reste techniquement un mandat, mais le Consortium préfère les amiraux qui ouvrent leurs dossiers.",
            condition: "Ouvrir le panneau Objectifs au moins une fois.",
            reward: None,
        },
        ObjectiveId::UpgradeProduction => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Productivité réglementaire",
            briefing: "La survie réclame une amélioration visible. Les citoyens préfèrent les promesses ; le Consortium préfère les niveaux de bâtiment.",
            condition: "Améliorer une installation de production au niveau 2.",
            reward: Some(reward(120, 60, 20)),
        },
        ObjectiveId::BuildLaboratory => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Science conforme",
            briefing: "Toute expansion durable commence par une salle où les hypothèses sont autorisées à condition de finir utiles.",
            condition: "Construire un Laboratoire.",
            reward: Some(reward(80, 120, 40)),
        },
        ObjectiveId::StartResearch => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Doctrine d'expansion",
            briefing: "Une recherche lancée transforme l'inconnu en budget défendable.",
            condition: "Avoir une recherche en file ou déjà terminée.",
            reward: Some(reward(100, 80, 60)),
        },
        ObjectiveId::BuildProbe => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Observation préventive",
            briefing: "La Sonde — Œil établit la différence essentielle entre un mystère et un dossier en retard.",
            condition: "Construire ou affecter au moins une Sonde — Œil.",
            reward: Some(reward(90, 70, 90)),
        },
        ObjectiveId::ProbePlanet => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Premier dossier planétaire",
            briefing: "Une planète sondée cesse d'être une rumeur astronomique et devient une opportunité sous examen.",
            condition: "Faire passer une planète non colonisée au niveau Sondé.",
            reward: Some(reward(120, 80, 80)),
        },
        ObjectiveId::BuildSatellite => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Surveillance orbitale",
            briefing: "Le Satellite — Veilleur ne juge pas. Il observe jusqu'à ce qu'une décision devienne évidente.",
            condition: "Construire ou affecter au moins un Satellite — Veilleur.",
            reward: Some(reward(120, 120, 80)),
        },
        ObjectiveId::AnalyzePlanet => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Pré-administration",
            briefing: "Une planète correctement analysée est déjà partiellement administrée, même si elle n'a pas encore été informée.",
            condition: "Obtenir un rapport d'analyse planétaire complet.",
            reward: Some(reward(150, 100, 100)),
        },
        ObjectiveId::HarvestSite => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Réquisition distante",
            briefing: "La ressource locale est pratique. La ressource lointaine est patriotique.",
            condition: "Terminer une mission de récolte distante avec cargaison livrée.",
            reward: Some(reward(80, 80, 160)),
        },
        ObjectiveId::BuildMilitaryFleet => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Dissuasion locale",
            briefing: "Une flotte militaire est un argument diplomatique qui a pris soin d'apporter ses propres conclusions.",
            condition: "Former une flotte composée de vaisseaux militaires.",
            reward: Some(reward(150, 80, 120)),
        },
        ObjectiveId::ResolveObstacle => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Résolution d'obstacle",
            briefing: "Si un monde utile est occupé, le Consortium recommande une clarification rapide des responsabilités locales.",
            condition: "Gagner une attaque ou identifier une cible analysée sans occupant à déloger.",
            reward: Some(reward(160, 100, 120)),
        },
        ObjectiveId::UnlockColonization => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Mandat d'implantation",
            briefing: "La Colonisation avancée autorise les nouveaux départs, les formulaires anticipés et l'optimisme contraignant.",
            condition: "Terminer la recherche Colonisation avancée.",
            reward: Some(reward(200, 140, 160)),
        },
        ObjectiveId::BuildColonyShip => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Bureau mobile",
            briefing: "L'Arche coloniale — Essor transporte les citoyens, les machines et la certitude qu'un bureau arrivera avant les plaintes.",
            condition: "Construire ou affecter une Arche coloniale — Essor.",
            reward: Some(reward(220, 160, 180)),
        },
        ObjectiveId::FoundSecondColony => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Campaign,
            title: "Nouvelle prospérité déclarée",
            briefing: "Deux colonies constituent une présence. Une présence constitue un précédent. Un précédent constitue une politique.",
            condition: "Posséder au moins deux colonies.",
            reward: Some(reward(300, 220, 180)),
        },
        ObjectiveId::ProbeDistantSystem => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Optional,
            title: "Cartographie supérieure",
            briefing: "Les systèmes proches rassurent les timides. Les systèmes lointains rassurent les services de planification.",
            condition: "Sonder un système situé à au moins trois sauts de Port-Sillage.",
            reward: Some(reward(180, 120, 120)),
        },
        ObjectiveId::AnalyzeOccupiedDistant => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Optional,
            title: "Recensement contrariant",
            briefing: "Un occupant lointain n'est pas forcément hostile. Il est surtout mal renseigné sur nos besoins.",
            condition: "Analyser une planète occupée hors du voisinage immédiat.",
            reward: Some(reward(160, 160, 120)),
        },
        ObjectiveId::HarvestValuableSite => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Optional,
            title: "Réquisition prioritaire",
            briefing: "Une cargaison substantielle confirme que le déplacement était nécessaire, donc moralement reposant.",
            condition: "Livrer au moins 500 unités via une mission de récolte.",
            reward: Some(reward(120, 120, 220)),
        },
        ObjectiveId::WinMixedFleetCombat => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Optional,
            title: "Doctrine combinée",
            briefing: "La victoire est plus pédagogique lorsqu'elle implique plusieurs services armés capables de se féliciter séparément.",
            condition: "Gagner un combat avec une flotte comprenant au moins deux types de vaisseaux militaires.",
            reward: Some(reward(220, 120, 180)),
        },
        ObjectiveId::FoundRiskColony => ObjectiveDefinition {
            id,
            category: ObjectiveCategory::Optional,
            title: "Implantation exemplaire",
            briefing: "Une colonie fondée loin du confort administratif prouve que la prudence sait parfois remplir un formulaire de congé.",
            condition: "Fonder une colonie à deux sauts ou plus, ou après sécurisation militaire.",
            reward: Some(reward(260, 180, 220)),
        },
    }
}

fn reward(metal: u64, crystal: u64, fuel: u64) -> ObjectiveReward {
    ObjectiveReward {
        stock: ResourceStock::new(metal, crystal, fuel),
    }
}

fn current_campaign_objective(progress: &ObjectiveProgressState) -> Option<ObjectiveId> {
    CAMPAIGN_OBJECTIVES
        .iter()
        .copied()
        .find(|id| !progress.completed_objectives.contains(id))
}

fn all_objective_ids() -> &'static [ObjectiveId] {
    const ALL: [ObjectiveId; 19] = [
        ObjectiveId::OpenObjectivesPanel,
        ObjectiveId::UpgradeProduction,
        ObjectiveId::BuildLaboratory,
        ObjectiveId::StartResearch,
        ObjectiveId::BuildProbe,
        ObjectiveId::ProbePlanet,
        ObjectiveId::BuildSatellite,
        ObjectiveId::AnalyzePlanet,
        ObjectiveId::HarvestSite,
        ObjectiveId::BuildMilitaryFleet,
        ObjectiveId::ResolveObstacle,
        ObjectiveId::UnlockColonization,
        ObjectiveId::BuildColonyShip,
        ObjectiveId::FoundSecondColony,
        ObjectiveId::ProbeDistantSystem,
        ObjectiveId::AnalyzeOccupiedDistant,
        ObjectiveId::HarvestValuableSite,
        ObjectiveId::WinMixedFleetCombat,
        ObjectiveId::FoundRiskColony,
    ];
    &ALL
}

fn player_best_level(state: &GameState, kinds: impl IntoIterator<Item = BuildingKind>) -> u8 {
    let kinds = kinds.into_iter().collect::<Vec<_>>();
    state
        .player_colonies()
        .flat_map(|colony| kinds.iter().map(move |kind| colony.buildings.level(*kind)))
        .max()
        .unwrap_or(0)
}

fn player_ship_count(state: &GameState, craftable: CraftableId) -> u64 {
    let docked = state
        .player_colonies()
        .map(|colony| colony.inventory.quantity(craftable))
        .fold(0_u64, u64::saturating_add);
    let fleeted = state
        .player_fleets()
        .map(|fleet| fleet.composition.quantity(craftable))
        .fold(0_u64, u64::saturating_add);
    docked.saturating_add(fleeted)
}

fn probed_non_colonized_count(state: &GameState) -> usize {
    state
        .planet_knowledge
        .iter()
        .filter(|knowledge| {
            knowledge.level >= KnowledgeLevel::Probed
                && knowledge.level != KnowledgeLevel::Colonized
        })
        .count()
}

fn harvest_report_count(state: &GameState) -> usize {
    state
        .mission_reports
        .iter()
        .filter(|report| {
            report.kind == MissionKind::Harvest
                && report.outcome == MissionReportOutcome::Completed
                && matches!(
                    report.result,
                    Some(MissionResult::Harvest(result)) if !result.delivered.is_zero()
                )
        })
        .count()
}

fn attack_victory_reported(state: &GameState) -> bool {
    state.mission_reports.iter().any(|report| {
        matches!(
            report.result,
            Some(MissionResult::Attack(result))
                if matches!(
                    result.outcome,
                    AttackMissionOutcome::Resolved(CombatOutcome::AttackerVictory)
                )
        )
    })
}

fn analyzed_clear_target_exists(state: &GameState, universe: &UniverseRepository) -> bool {
    state.planet_analysis_reports.iter().any(|report| {
        let assessment =
            assess_planet_colonizability(state, universe, state.player_faction, report.planet_id);
        !assessment
            .blockers
            .iter()
            .any(|blocker| matches!(blocker, ColonizationBlocker::OccupiedPlanet { .. }))
    })
}

fn probed_distant_system_exists(
    state: &GameState,
    universe: &UniverseRepository,
    minimum_hops: usize,
) -> bool {
    let Some(home) = state.player_home_colony() else {
        return false;
    };
    state.system_knowledge.iter().any(|knowledge| {
        knowledge.level >= KnowledgeLevel::Probed
            && universe
                .shortest_path(home.system_id, knowledge.system_id)
                .is_some_and(|path| path.len().saturating_sub(1) >= minimum_hops)
    })
}

fn analyzed_occupied_distant_planet_exists(
    state: &GameState,
    universe: &UniverseRepository,
    minimum_hops: usize,
) -> bool {
    let Some(home) = state.player_home_colony() else {
        return false;
    };
    state.planet_analysis_reports.iter().any(|report| {
        let Some((system_id, _)) = universe.planet_location(report.planet_id) else {
            return false;
        };
        let distant = universe
            .shortest_path(home.system_id, system_id)
            .is_some_and(|path| path.len().saturating_sub(1) >= minimum_hops);
        distant
            && state
                .planetary_intelligence_report(report.planet_id)
                .is_some_and(|intel| {
                    matches!(intel.occupancy, PlanetaryOccupancyIntel::Occupied(_))
                })
    })
}

fn valuable_harvest_reported(state: &GameState) -> bool {
    state.mission_reports.iter().any(|report| {
        matches!(
            report.result,
            Some(MissionResult::Harvest(result)) if stock_total(result.delivered) >= 500
        )
    })
}

fn mixed_fleet_victory_reported(state: &GameState) -> bool {
    state.mission_reports.iter().any(|report| {
        let Some(MissionResult::Attack(result)) = report.result else {
            return false;
        };
        if !matches!(
            result.outcome,
            AttackMissionOutcome::Resolved(CombatOutcome::AttackerVictory)
        ) {
            return false;
        }
        state
            .combat_report(report.mission_id)
            .is_some_and(|combat| {
                combat
                    .attacker
                    .ships
                    .iter()
                    .filter(|ship| ship.quantity > 0)
                    .count()
                    >= 2
            })
    })
}

fn risky_colony_exists(state: &GameState, universe: &UniverseRepository) -> bool {
    let Some(home) = state.player_home_colony() else {
        return false;
    };
    state.player_colonies().any(|colony| {
        colony.id != home.id
            && (attack_victory_reported(state)
                || universe
                    .shortest_path(home.system_id, colony.system_id)
                    .is_some_and(|path| path.len().saturating_sub(1) >= 2))
    })
}

fn stock_total(stock: ResourceStock) -> u64 {
    stock
        .metal
        .saturating_add(stock.crystal)
        .saturating_add(stock.fuel)
}

fn format_stock(stock: ResourceStock) -> String {
    if stock.is_zero() {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    if stock.metal > 0 {
        parts.push(format!("{} métal", stock.metal));
    }
    if stock.crystal > 0 {
        parts.push(format!("{} cristal", stock.crystal));
    }
    if stock.fuel > 0 {
        parts.push(format!("{} carburant", stock.fuel));
    }
    parts.join(", ")
}

fn objective_row_color(selected: bool, completed: bool, interaction: &Interaction) -> Color {
    match (*interaction, selected, completed) {
        (Interaction::Pressed, _, _) => Color::srgba(0.12, 0.24, 0.17, 0.98),
        (Interaction::Hovered, true, _) => Color::srgba(0.09, 0.20, 0.14, 0.98),
        (Interaction::Hovered, false, true) => Color::srgba(0.07, 0.16, 0.11, 0.98),
        (Interaction::Hovered, false, false) => Color::srgba(0.05, 0.10, 0.08, 0.98),
        (_, true, _) => Color::srgba(0.07, 0.17, 0.12, 0.98),
        (_, false, true) => Color::srgba(0.035, 0.095, 0.060, 0.98),
        (_, false, false) => Color::srgba(0.025, 0.040, 0.036, 0.98),
    }
}

fn objective_row_outline(selected: bool, completed: bool) -> Color {
    if selected {
        Color::srgba(0.52, 0.88, 0.62, 0.78)
    } else if completed {
        Color::srgba(0.36, 0.72, 0.46, 0.58)
    } else {
        Color::srgba(0.22, 0.34, 0.26, 0.76)
    }
}

fn objective_accent() -> Color {
    Color::srgba(0.42, 0.82, 0.54, 0.70)
}

#[cfg(test)]
mod tests {
    use super::*;
    use galactic_domain::{ResourceLedger, UniverseConfig};
    use galactic_sim::{Simulation, StartingScenario, default_ruleset};

    fn simulation() -> Simulation {
        Simulation::new(UniverseConfig::default())
    }

    #[test]
    fn first_objective_requires_opening_the_objectives_panel() {
        let simulation = simulation();
        let mut progress = ObjectiveProgressState::default();

        assert!(!objective_is_complete(
            simulation.state(),
            simulation.universe_repository(),
            &progress,
            ObjectiveId::OpenObjectivesPanel,
        ));
        progress.opened_objectives_panel = true;
        assert!(objective_is_complete(
            simulation.state(),
            simulation.universe_repository(),
            &progress,
            ObjectiveId::OpenObjectivesPanel,
        ));
        assert_eq!(
            current_campaign_objective(&progress),
            Some(ObjectiveId::OpenObjectivesPanel),
        );
        progress
            .completed_objectives
            .insert(ObjectiveId::OpenObjectivesPanel);
        assert_eq!(
            current_campaign_objective(&progress),
            Some(ObjectiveId::UpgradeProduction),
        );
        assert!(!objective_is_complete(
            simulation.state(),
            simulation.universe_repository(),
            &progress,
            ObjectiveId::BuildLaboratory,
        ));
    }

    #[test]
    fn production_upgrade_requires_a_real_level_increase() {
        let mut simulation = simulation();
        let progress = ObjectiveProgressState::default();
        assert!(!objective_is_complete(
            simulation.state(),
            simulation.universe_repository(),
            &progress,
            ObjectiveId::UpgradeProduction,
        ));

        let home = simulation
            .state()
            .player_home_colony()
            .expect("home colony")
            .id;
        simulation
            .state_mut()
            .colony_mut(home)
            .expect("home colony")
            .buildings
            .set_level(BuildingKind::METAL_MINE, 2);

        assert!(objective_is_complete(
            simulation.state(),
            simulation.universe_repository(),
            &progress,
            ObjectiveId::UpgradeProduction,
        ));
    }

    #[test]
    fn rewards_are_session_local_and_claimed_once() {
        let mut simulation = SimulationResource {
            simulation: simulation(),
            pending_events: Vec::new(),
        };
        let reward = ObjectiveReward {
            stock: ResourceStock::new(10, 20, 30),
        };
        let home = simulation
            .simulation()
            .state()
            .player_home_colony()
            .expect("home colony")
            .id;
        let before = simulation
            .simulation()
            .state()
            .colony(home)
            .expect("home colony")
            .resources
            .stock();

        let credited = grant_reward(&mut simulation, reward);
        let after = simulation
            .simulation()
            .state()
            .colony(home)
            .expect("home colony")
            .resources
            .stock();

        assert_eq!(credited, ResourceStock::new(10, 20, 30));
        assert_eq!(after, before + credited);
    }

    #[test]
    fn current_campaign_objective_skips_completed_steps() {
        let mut progress = ObjectiveProgressState::default();
        progress
            .completed_objectives
            .insert(ObjectiveId::OpenObjectivesPanel);
        progress
            .completed_objectives
            .insert(ObjectiveId::UpgradeProduction);

        assert_eq!(
            current_campaign_objective(&progress),
            Some(ObjectiveId::BuildLaboratory),
        );
    }

    #[test]
    fn objective_text_keeps_consortium_tone_without_external_branding() {
        let definition = objective_definition(ObjectiveId::AnalyzePlanet);

        assert!(definition.briefing.contains("administrée"));
        assert!(!definition.briefing.contains("Helldivers"));
    }

    #[test]
    fn objective_hints_start_hidden() {
        assert!(!ObjectiveUiState::default().hints_visible);
    }

    #[test]
    fn format_stock_omits_zero_components() {
        assert_eq!(
            format_stock(ResourceStock::new(0, 50, 12)),
            "50 cristal, 12 carburant",
        );
    }

    #[test]
    fn optional_objectives_are_not_part_of_the_main_campaign() {
        assert!(!CAMPAIGN_OBJECTIVES.contains(&ObjectiveId::ProbeDistantSystem));
        assert!(OPTIONAL_OBJECTIVES.contains(&ObjectiveId::ProbeDistantSystem));
    }

    #[test]
    fn first_campaign_rewards_are_modest_against_starting_storage() {
        let scenario = StartingScenario::mvp();
        let capacity = default_ruleset()
            .buildings()
            .storage_capacity_for_levels(scenario.home_colony.buildings);
        let reward = objective_definition(ObjectiveId::UpgradeProduction)
            .reward
            .expect("reward");

        assert!(reward.stock.metal < capacity.metal / 10);
        assert!(reward.stock.crystal < capacity.crystal / 10);
        assert!(reward.stock.fuel < capacity.fuel / 10);
    }

    #[test]
    fn reward_credit_respects_storage_capacity() {
        let mut simulation = SimulationResource {
            simulation: simulation(),
            pending_events: Vec::new(),
        };
        let home = simulation
            .simulation()
            .state()
            .player_home_colony()
            .expect("home colony")
            .id;
        {
            let state = simulation.simulation.state_mut();
            let colony = state.colony_mut(home).expect("home colony");
            let capacity = storage_capacity(colony.buildings);
            colony.resources = ResourceLedger::new(ResourceStock::new(
                capacity.metal - 1,
                capacity.crystal,
                capacity.fuel - 2,
            ));
        }

        let credited = grant_reward(
            &mut simulation,
            ObjectiveReward {
                stock: ResourceStock::new(10, 10, 10),
            },
        );

        assert_eq!(credited, ResourceStock::new(1, 0, 2));
    }
}

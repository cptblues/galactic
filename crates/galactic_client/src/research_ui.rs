// MVP-016: dedicated research screen kept outside the main client module.
use bevy::prelude::*;
use galactic_sim::{
    BuildingKind, GameAction, GameEventKind, ResearchError, ResearchQuote,
    STRATEGIC_TICKS_PER_SECOND, StrategicDuration, TechnologyId, default_building_catalog,
    max_research_queue, research_lab_level_total, research_output_milli_points_per_tick,
    research_output_points_per_second, research_progress_ratio, research_quote, technology_catalog,
    technology_definition,
};

use super::{
    OpenPanel, PresentationUpdateSet, SimulationResource, UiPointerBlocker, accent_research_violet,
    action_button_color, action_button_outline, apply_simulation_command,
    collect_presentation_events, format_strategic_duration, panel_background, panel_outline,
    ui_text_font,
};

const RESEARCH_Z_INDEX: i32 = 110;

pub(crate) struct ResearchUiPlugin;

impl Plugin for ResearchUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResearchUiState>()
            .add_systems(Startup, spawn_research_screen)
            .add_systems(
                Update,
                capture_research_feedback
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (handle_research_shortcuts, handle_research_buttons)
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_research_visibility,
                    update_research_summary,
                    update_research_technology_buttons,
                    update_research_detail,
                    update_research_queue,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Resource)]
pub(crate) struct ResearchUiState {
    selected: TechnologyId,
    feedback: String,
}

impl Default for ResearchUiState {
    fn default() -> Self {
        Self {
            selected: technology_catalog()
                .ids()
                .next()
                .expect("validated ruleset contains at least one technology"),
            feedback: String::new(),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ResearchButtonAction {
    Toggle,
    Close,
    Select(TechnologyId),
    QueueSelected,
    CancelActive,
}

type ResearchButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ResearchButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct ResearchRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum ResearchTextRole {
    Toggle,
    Title,
    Summary,
    Detail,
    Queue,
    QueueButton,
    Feedback,
}

#[derive(Component)]
struct TechnologyButton {
    technology: TechnologyId,
}

#[derive(Component)]
struct TechnologyButtonText {
    technology: TechnologyId,
}

#[derive(Component)]
struct QueueResearchButton;

#[derive(Component)]
struct CancelResearchButton;

#[derive(Component)]
struct ResearchProgressFill;

pub(crate) fn spawn_research_toggle(parent: &mut ChildSpawnerCommands) {
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
            BackgroundColor(Color::srgba(0.11, 0.13, 0.24, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_research_violet()),
            ResearchButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Recherche techno  [T]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.84, 0.86, 1.0)),
                ResearchTextRole::Toggle,
            ));
        });
}

fn spawn_research_screen(mut commands: Commands) {
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
            BackgroundColor(Color::srgba(0.008, 0.012, 0.026, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.48, 0.54, 0.94, 0.72),
            ),
            Visibility::Hidden,
            GlobalZIndex(RESEARCH_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            ResearchRoot,
        ))
        .with_children(|root| {
            spawn_research_header(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.74, 0.82, 0.96)),
                Node {
                    min_height: Val::Px(28.0),
                    ..default()
                },
                ResearchTextRole::Summary,
            ));
            spawn_research_main_row(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.94, 0.72, 0.44)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ResearchTextRole::Feedback,
            ));
        });
}

fn spawn_research_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("RECHERCHE TECHNO"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.86, 0.88, 1.0)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ResearchTextRole::Title,
            ));
            spawn_research_small_button(
                header,
                "Fermer  [T / Échap]",
                ResearchButtonAction::Close,
                160.0,
            );
        });
}

fn spawn_research_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ResearchButtonAction,
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
            BackgroundColor(Color::srgba(0.07, 0.08, 0.16, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.46, 0.52, 0.84, 0.58),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.84, 0.88, 0.98)),
            ));
        });
}

fn spawn_research_main_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(450.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(9.0),
        ..default()
    },))
        .with_children(|row| {
            spawn_technology_list(row);
            spawn_technology_detail(row);
            spawn_research_queue(row);
        });
}

fn spawn_technology_list(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(330.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|list| {
        list.spawn((
            Text::new("ARBRE TECHNOLOGIQUE"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.76, 0.82, 1.0)),
        ));
        for technology in technology_catalog().ids() {
            spawn_technology_button(list, technology);
        }
    });
}

fn spawn_technology_button(parent: &mut ChildSpawnerCommands, technology: TechnologyId) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(52.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.05, 0.10, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.32, 0.36, 0.62, 0.46),
            ),
            ResearchButtonAction::Select(technology),
            TechnologyButton { technology },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.80, 0.84, 0.94)),
                TechnologyButtonText { technology },
            ));
        });
}

fn spawn_technology_detail(row: &mut ChildSpawnerCommands) {
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
            Text::new("Sélectionne une technologie."),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.84, 0.88, 0.96)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ResearchTextRole::Detail,
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
                BackgroundColor(Color::srgba(0.12, 0.18, 0.42, 0.98)),
                Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.52, 0.62, 1.0, 0.76)),
                ResearchButtonAction::QueueSelected,
                QueueResearchButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("LANCER LA RECHERCHE"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.88, 0.92, 1.0)),
                    ResearchTextRole::QueueButton,
                ));
            });
    });
}

fn spawn_research_queue(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(320.0),
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
            Text::new("FILE DE RECHERCHE TECHNO"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.76, 0.82, 1.0)),
        ));
        queue
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.09, 0.16, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.52, 0.62, 1.0)),
                    ResearchProgressFill,
                ));
            });
        queue.spawn((
            Text::new("File vide."),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.78, 0.82, 0.92)),
            ResearchTextRole::Queue,
        ));
        queue
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(32.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.30, 0.09, 0.14, 0.94)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.86, 0.36, 0.42, 0.60),
                ),
                ResearchButtonAction::CancelActive,
                CancelResearchButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Annuler la recherche en cours"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(1.0, 0.80, 0.82)),
                ));
            });
    });
}

fn handle_research_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<ResearchUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        let opening = *open_panel != OpenPanel::Research;
        *open_panel = if opening {
            OpenPanel::Research
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

    if *open_panel == OpenPanel::Research && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
    }
}

fn handle_research_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<ResearchUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: ResearchButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            ResearchButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::Research;
                *open_panel = if opening {
                    OpenPanel::Research
                } else {
                    OpenPanel::None
                };
                ui.feedback.clear();
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            ResearchButtonAction::Close => {
                *open_panel = OpenPanel::None;
            }
            ResearchButtonAction::Select(technology) => {
                ui.selected = technology;
                ui.feedback.clear();
            }
            ResearchButtonAction::QueueSelected => {
                queue_selected_research(&mut simulation, &mut ui);
            }
            ResearchButtonAction::CancelActive => {
                apply_simulation_command(&mut simulation, GameAction::CancelResearch);
            }
        }
    }
}

fn capture_research_feedback(simulation: Res<SimulationResource>, mut ui: ResMut<ResearchUiState>) {
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::ResearchQueued(queued) => {
                let definition = technology_definition(queued.project.technology);
                ui.feedback = format!("{} ajouté à la file.", definition.name,);
            }
            GameEventKind::ResearchCompleted(completed) => {
                let definition = technology_definition(completed.technology);
                ui.feedback = format!(
                    "{} terminé — {} débloqué.",
                    definition.name, definition.unlock_label,
                );
            }
            GameEventKind::ResearchRejected(rejected) => {
                ui.feedback = format!(
                    "Recherche refusée : {}",
                    research_error_text(rejected.error),
                );
            }
            GameEventKind::ResearchCancelled(cancelled) => {
                let definition = technology_definition(cancelled.technology);
                ui.feedback = format!(
                    "{} annulée ({:.1} points perdus).",
                    definition.name,
                    cancelled.accumulated_milli_points as f64 / 1_000.0,
                );
            }
            GameEventKind::ResearchCancellationRejected(rejected) => {
                ui.feedback = format!(
                    "Annulation refusée : {}",
                    research_error_text(rejected.error),
                );
            }
            _ => {}
        }
    }
}

fn update_research_visibility(
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<ResearchRoot>>,
    mut texts: Query<(&ResearchTextRole, &mut Text)>,
) {
    let is_open = *open_panel == OpenPanel::Research;
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
        if *role == ResearchTextRole::Toggle {
            let next = if is_open {
                "Fermer recherche".to_string()
            } else {
                "Recherche techno  [T]".to_string()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
    }
}

fn update_research_summary(
    simulation: Res<SimulationResource>,
    ui: Res<ResearchUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ResearchTextRole, &mut Text)>,
) {
    if *open_panel != OpenPanel::Research {
        return;
    }
    let state = simulation.simulation().state();
    let labs = research_lab_level_total(state, state.player_faction);
    let output = research_output_points_per_second(state, state.player_faction);
    let completed = state.research.completed_count();

    for (role, mut text) in &mut texts {
        match role {
            ResearchTextRole::Title => {
                text.0 = format!(
                    "RECHERCHE TECHNO — {} / {} technologie(s)",
                    completed,
                    technology_catalog().ids().count(),
                );
            }
            ResearchTextRole::Summary => {
                text.0 = format!(
                    "Instituts d'analyse cumulés : niveau {}  •  production scientifique : {:.2} point(s)/s  •  file : {}/{}",
                    labs,
                    output,
                    state.research.queue_len(),
                    max_research_queue(),
                );
            }
            ResearchTextRole::Feedback => {
                text.0 = ui.feedback.clone();
            }
            _ => {}
        }
    }
}

fn update_research_technology_buttons(
    simulation: Res<SimulationResource>,
    ui: Res<ResearchUiState>,
    open_panel: Res<OpenPanel>,
    mut buttons: Query<(
        &TechnologyButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
    mut labels: Query<(&TechnologyButtonText, &mut Text, &mut TextColor)>,
) {
    if *open_panel != OpenPanel::Research {
        return;
    }
    let state = simulation.simulation().state();

    for (button, interaction, mut background, mut outline) in &mut buttons {
        let selected = button.technology == ui.selected;
        background.0 = technology_button_color(selected, interaction);
        outline.color = technology_button_outline(selected);
    }

    for (label, mut text, mut color) in &mut labels {
        let definition = technology_definition(label.technology);
        let status = technology_status_label(state, label.technology);
        text.0 = format!("{}\n{}", definition.name, status,);
        color.0 = if state.research.has_completed(label.technology) {
            Color::srgb(0.54, 0.94, 0.74)
        } else if label.technology == ui.selected {
            Color::srgb(0.90, 0.92, 1.0)
        } else {
            Color::srgb(0.78, 0.82, 0.92)
        };
    }
}

fn update_research_detail(
    simulation: Res<SimulationResource>,
    ui: Res<ResearchUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ResearchTextRole, &mut Text, &mut TextColor)>,
    mut button: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<QueueResearchButton>,
    >,
) {
    if *open_panel != OpenPanel::Research {
        return;
    }
    let state = simulation.simulation().state();
    let quote = research_quote(state, state.player_faction, ui.selected);
    let available = quote.is_ok();
    let detail = research_detail_text(state, ui.selected, quote);
    let button_label = match quote {
        Ok(_) => "LANCER LA RECHERCHE".to_string(),
        Err(error) => research_error_text(error),
    };

    for (role, mut text, mut color) in &mut texts {
        match role {
            ResearchTextRole::Detail => {
                text.0 = detail.clone();
                color.0 = Color::srgb(0.84, 0.88, 0.96);
            }
            ResearchTextRole::QueueButton => {
                text.0 = button_label.clone();
                color.0 = if available {
                    Color::srgb(0.88, 0.92, 1.0)
                } else {
                    Color::srgb(0.62, 0.64, 0.70)
                };
            }
            _ => {}
        }
    }

    for (interaction, mut background, mut outline) in &mut button {
        background.0 = action_button_color(available, false, interaction);
        outline.color = action_button_outline(available, false, interaction);
    }
}

fn update_research_queue(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ResearchTextRole, &mut Text)>,
    mut progress: Query<&mut Node, With<ResearchProgressFill>>,
    mut cancel_button: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<CancelResearchButton>,
    >,
) {
    if *open_panel != OpenPanel::Research {
        return;
    }
    let state = simulation.simulation().state();
    let label = research_queue_text(state);

    for (role, mut text) in &mut texts {
        if *role == ResearchTextRole::Queue {
            text.0 = label.clone();
        }
    }

    let active_project = state.research.active();
    let ratio = active_project
        .copied()
        .map(research_progress_ratio)
        .unwrap_or(0.0);
    for mut node in &mut progress {
        node.width = Val::Percent((ratio * 100.0).clamp(0.0, 100.0));
    }

    let can_cancel = active_project.is_some();
    for (interaction, mut background, mut outline) in &mut cancel_button {
        background.0 = action_button_color(can_cancel, false, interaction);
        outline.color = action_button_outline(can_cancel, false, interaction);
    }
}

fn queue_selected_research(simulation: &mut SimulationResource, ui: &mut ResearchUiState) {
    let technology = ui.selected;
    let state = simulation.simulation().state();
    match research_quote(state, state.player_faction, technology) {
        Ok(_) => {
            apply_simulation_command(simulation, GameAction::QueueResearch { technology });
        }
        Err(error) => {
            ui.feedback = research_error_text(error);
        }
    }
}

fn technology_status_label(state: &galactic_sim::GameState, technology: TechnologyId) -> String {
    if state.research.has_completed(technology) {
        return "ACQUISE".to_string();
    }
    if let Some(position) = state
        .research
        .queue()
        .position(|project| project.technology == technology)
    {
        return if position == 0 {
            "EN COURS".to_string()
        } else {
            format!("EN ATTENTE — position {}", position + 1)
        };
    }

    match research_quote(state, state.player_faction, technology) {
        Ok(_) => "DISPONIBLE".to_string(),
        Err(error) => research_error_text(error),
    }
}

fn research_detail_text(
    state: &galactic_sim::GameState,
    technology: TechnologyId,
    quote: Result<ResearchQuote, ResearchError>,
) -> String {
    let definition = technology_definition(technology);
    let prerequisites = if definition.prerequisites.is_empty() {
        "aucun".to_string()
    } else {
        definition
            .prerequisites
            .iter()
            .map(|prerequisite| technology_definition(*prerequisite).name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let cost_points = definition.required_milli_points as f64 / 1_000.0;

    let mut lines = vec![
        definition.name.to_uppercase(),
        String::new(),
        definition.description.to_string(),
        String::new(),
        format!("Prérequis : {prerequisites}"),
    ];
    if !definition.building_prerequisites.is_empty() {
        let catalog = galactic_sim::default_building_catalog();
        let buildings = definition
            .building_prerequisites
            .iter()
            .map(|prerequisite| {
                format!(
                    "{} niveau {}",
                    catalog.definition(prerequisite.kind).name,
                    prerequisite.level,
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Bâtiments requis : {buildings}"));
    }
    lines.extend([
        format!("Coût scientifique : {cost_points:.1} points"),
        format!("Déblocage : {}", definition.unlock_label,),
    ]);

    if state.research.has_completed(technology) {
        lines.push(String::new());
        lines.push("TECHNOLOGIE ACQUISE".to_string());
        return lines.join("\n");
    }

    if let Some(project) = state
        .research
        .queue()
        .find(|project| project.technology == technology)
        .copied()
    {
        lines.extend([
            String::new(),
            format!(
                "Progression : {:.1} / {:.1} points",
                project.accumulated_milli_points as f64 / 1_000.0,
                project.required_milli_points as f64 / 1_000.0,
            ),
            format!("État : {}", technology_status_label(state, technology),),
        ]);
        return lines.join("\n");
    }

    match quote {
        Ok(value) => {
            lines.extend([
                String::new(),
                format!(
                    "Production actuelle : {:.2} point(s)/s",
                    value.output_milli_points_per_tick as f64
                        * f64::from(STRATEGIC_TICKS_PER_SECOND,)
                        / 1_000.0,
                ),
                format!(
                    "Durée estimée : {}",
                    format_strategic_duration(
                        StrategicDuration::from_ticks(value.estimated_ticks,),
                    ),
                ),
                String::new(),
                "Prête à être ajoutée à la file.".to_string(),
            ]);
        }
        Err(error) => {
            lines.extend([
                String::new(),
                format!("BLOCAGE : {}", research_error_text(error),),
            ]);
        }
    }

    lines.join("\n")
}

fn research_queue_text(state: &galactic_sim::GameState) -> String {
    if state.research.is_queue_empty() {
        let has_output = research_output_milli_points_per_tick(state, state.player_faction) > 0;
        let hint = if !has_output {
            format!(
                "Construis un {} pour produire des points de recherche.",
                research_lab_building_name()
            )
        } else {
            "Sélectionne une technologie disponible.".to_string()
        };
        return format!(
            "File vide\n\n{}\n\n{} emplacement(s) disponible(s).",
            hint,
            max_research_queue(),
        );
    }

    let output = research_output_milli_points_per_tick(state, state.player_faction);
    let mut lines = Vec::new();
    for (index, project) in state.research.queue().enumerate() {
        let definition = technology_definition(project.technology);
        if index == 0 {
            let remaining_ticks = if output == 0 {
                None
            } else {
                Some(project.remaining_milli_points().div_ceil(output))
            };
            let remaining = remaining_ticks
                .map(|ticks| format_strategic_duration(StrategicDuration::from_ticks(ticks)))
                .unwrap_or_else(|| "en pause — aucun laboratoire".to_string());
            lines.push(format!(
                "EN COURS\n{}. {}\n{:.1} / {:.1} points\n{} restante(s)",
                index + 1,
                definition.name,
                project.accumulated_milli_points as f64 / 1_000.0,
                project.required_milli_points as f64 / 1_000.0,
                remaining,
            ));
        } else {
            lines.push(format!(
                "\nEN ATTENTE\n{}. {} — {:.1} points",
                index + 1,
                definition.name,
                project.required_milli_points as f64 / 1_000.0,
            ));
        }
    }
    lines.push(format!(
        "\n\n{} / {} emplacement(s) utilisé(s)",
        state.research.queue_len(),
        max_research_queue(),
    ));
    lines.join("\n")
}

fn research_error_text(error: ResearchError) -> String {
    match error {
        ResearchError::Access(_) => "Recherche non autorisée".to_string(),
        ResearchError::NoResearchCapacity => format!("{} requis", research_lab_building_name()),
        ResearchError::AlreadyCompleted(technology) => {
            format!("{} déjà acquise", technology_definition(technology).name,)
        }
        ResearchError::AlreadyQueued(technology) => {
            format!(
                "{} déjà dans la file",
                technology_definition(technology).name,
            )
        }
        ResearchError::QueueFull { maximum } => {
            format!("File pleine ({maximum})")
        }
        ResearchError::MissingPrerequisite { prerequisite, .. } => {
            format!("Requiert {}", technology_definition(prerequisite).name,)
        }
        ResearchError::MissingBuildingPrerequisite {
            building,
            required,
            found,
        } => {
            let name = galactic_sim::default_building_catalog()
                .definition(building)
                .name;
            format!("Requiert {name} niveau {required} (actuel : {found})")
        }
        ResearchError::NoActiveProject => "Aucune recherche en cours".to_string(),
    }
}

fn research_lab_building_name() -> &'static str {
    default_building_catalog()
        .definition(BuildingKind::RESEARCH_LAB)
        .name
}

fn technology_button_color(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        return Color::srgba(0.16, 0.20, 0.48, 0.98);
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.14, 0.18, 0.40, 0.98),
        Interaction::Hovered => Color::srgba(0.09, 0.12, 0.26, 0.98),
        Interaction::None => Color::srgba(0.04, 0.05, 0.10, 0.98),
    }
}

fn technology_button_outline(selected: bool) -> Color {
    if selected {
        Color::srgba(0.58, 0.68, 1.0, 0.88)
    } else {
        Color::srgba(0.32, 0.36, 0.62, 0.46)
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use galactic_domain::UniverseConfig;
    use galactic_sim::{BuildingKind, Simulation, default_building_catalog};

    use super::*;

    fn simulation_with_lab() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        simulation
    }

    #[test]
    fn root_technology_explains_missing_laboratory() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        let text = research_detail_text(
            simulation.state(),
            TechnologyId::SPATIAL_DETECTION,
            research_quote(
                simulation.state(),
                simulation.state().player_faction,
                TechnologyId::SPATIAL_DETECTION,
            ),
        );

        assert!(text.contains(&format!("{} requis", research_lab_building_name())));
        assert!(
            text.contains(
                &technology_definition(TechnologyId::SPATIAL_DETECTION)
                    .name
                    .to_uppercase()
            )
        );
    }

    #[test]
    fn queue_text_distinguishes_active_and_waiting() {
        let mut simulation = simulation_with_lab();
        simulation.apply_player_action(GameAction::QueueResearch {
            technology: TechnologyId::SPATIAL_DETECTION,
        });
        simulation.apply_player_action(GameAction::QueueResearch {
            technology: TechnologyId::PROPULSION,
        });

        let text = research_queue_text(simulation.state());
        assert!(text.contains("EN COURS"));
        assert!(text.contains("EN ATTENTE"));
    }

    #[test]
    fn research_overlay_is_above_colony_management() {
        const { assert!(RESEARCH_Z_INDEX > 100) };
    }

    #[test]
    fn research_shortcut_ignores_navigation_text_input() {
        let mut world = World::new();
        world.insert_resource(OpenPanel::Navigation);
        world.insert_resource(ResearchUiState::default());
        let mut navigation_ui = super::super::navigation_ui::NavigationUiState::default();
        navigation_ui.search_open = true;
        world.insert_resource(navigation_ui);
        world.insert_resource(crate::fleet_ui::FleetUiState::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::KeyT);
        world.insert_resource(keyboard);

        world
            .run_system_once(handle_research_shortcuts)
            .expect("handle_research_shortcuts runs");

        assert_eq!(*world.resource::<OpenPanel>(), OpenPanel::Navigation);
    }
}

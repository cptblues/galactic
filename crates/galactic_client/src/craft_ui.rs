// MVP-017: dedicated minimal shipyard screen.
use bevy::prelude::*;
use galactic_sim::{
    BuildingKind, CraftError, CraftQuote, CraftableId, GameAction, GameEventKind,
    MAX_CRAFT_BATCH_QUANTITY, StrategicDuration, craft_progress_ratio, craft_quote,
    craftable_catalog, craftable_definition, default_building_catalog, max_affordable_quantity,
    max_craft_queue, ship_class_label, shipyard_output_milli_per_tick,
    shipyard_output_points_per_second, technology_definition,
};

use super::{
    OpenPanel, PresentationUpdateSet, SimulationResource, UiPointerBlocker, accent_craft_amber,
    action_button_color, action_button_outline, apply_simulation_command,
    collect_presentation_events, format_strategic_duration, panel_background, panel_outline,
    ui_text_font,
};

const CRAFT_Z_INDEX: i32 = 120;

pub(crate) struct CraftUiPlugin;

impl Plugin for CraftUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftUiState>()
            .add_systems(Startup, spawn_craft_screen)
            .add_systems(
                Update,
                capture_craft_feedback
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (handle_craft_shortcuts, handle_craft_buttons)
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_craft_visibility,
                    update_craft_summary,
                    update_craftable_buttons,
                    update_craft_detail,
                    update_craft_queue,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Resource)]
pub(crate) struct CraftUiState {
    selected: CraftableId,
    quantity: u64,
    feedback: String,
}

impl Default for CraftUiState {
    fn default() -> Self {
        Self {
            selected: craftable_catalog()
                .ids()
                .next()
                .expect("validated ruleset contains at least one craftable"),
            quantity: 1,
            feedback: String::new(),
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum CraftButtonAction {
    Toggle,
    Close,
    PreviousColony,
    NextColony,
    Select(CraftableId),
    AdjustQuantity(i64),
    SetQuantityMax,
    QueueSelected,
    CancelActive,
}

type CraftButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static CraftButtonAction),
    (Changed<Interaction>, With<Button>),
>;

type CraftCancelButtonStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    (With<CancelCraftButton>, Without<QueueCraftButton>),
>;

#[derive(Component)]
struct CraftRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum CraftTextRole {
    Toggle,
    Title,
    Summary,
    Detail,
    Quantity,
    Queue,
    QueueButton,
    Feedback,
}

#[derive(Component)]
struct CraftableButton {
    craftable: CraftableId,
}

#[derive(Component)]
struct CraftableButtonText {
    craftable: CraftableId,
}

#[derive(Component)]
struct QueueCraftButton;

#[derive(Component)]
struct CancelCraftButton;

#[derive(Component)]
struct CraftProgressFill;

pub(crate) fn spawn_craft_toggle(parent: &mut ChildSpawnerCommands) {
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
            BackgroundColor(Color::srgba(0.17, 0.11, 0.05, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_craft_amber()),
            CraftButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Chantier flotte  [Y]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(1.0, 0.86, 0.66)),
                CraftTextRole::Toggle,
            ));
        });
}

fn spawn_craft_screen(mut commands: Commands) {
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
            BackgroundColor(Color::srgba(0.018, 0.011, 0.006, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.94, 0.54, 0.20, 0.74),
            ),
            Visibility::Hidden,
            GlobalZIndex(CRAFT_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            CraftRoot,
        ))
        .with_children(|root| {
            spawn_craft_header(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.94, 0.82, 0.68)),
                Node {
                    min_height: Val::Px(28.0),
                    ..default()
                },
                CraftTextRole::Summary,
            ));
            spawn_craft_main_row(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(1.0, 0.70, 0.34)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                CraftTextRole::Feedback,
            ));
        });
}

fn spawn_craft_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("CHANTIER SPATIAL"),
                ui_text_font(18.0),
                TextColor(Color::srgb(1.0, 0.86, 0.68)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                CraftTextRole::Title,
            ));
            spawn_craft_small_button(
                header,
                "< Précédente",
                CraftButtonAction::PreviousColony,
                92.0,
            );
            spawn_craft_small_button(header, "Suivante >", CraftButtonAction::NextColony, 92.0);
            spawn_craft_small_button(
                header,
                "Fermer  [Y / Échap]",
                CraftButtonAction::Close,
                160.0,
            );
        });
}

fn spawn_craft_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: CraftButtonAction,
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
            BackgroundColor(Color::srgba(0.15, 0.09, 0.04, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.86, 0.48, 0.18, 0.60),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.96, 0.86, 0.74)),
            ));
        });
}

fn spawn_craft_main_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(450.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(9.0),
        ..default()
    },))
        .with_children(|row| {
            spawn_craftable_list(row);
            spawn_craftable_detail(row);
            spawn_craft_queue(row);
        });
}

fn spawn_craftable_list(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(310.0),
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
            Text::new("CATALOGUE DE FABRICATION"),
            ui_text_font(12.0),
            TextColor(Color::srgb(1.0, 0.82, 0.58)),
        ));
        for craftable in craftable_catalog().ids() {
            spawn_craftable_button(list, craftable);
        }
    });
}

fn spawn_craftable_button(parent: &mut ChildSpawnerCommands, craftable: CraftableId) {
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
            BackgroundColor(Color::srgba(0.08, 0.05, 0.025, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.54, 0.34, 0.15, 0.48),
            ),
            CraftButtonAction::Select(craftable),
            CraftableButton { craftable },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.94, 0.84, 0.72)),
                CraftableButtonText { craftable },
            ));
        });
}

fn spawn_craftable_detail(row: &mut ChildSpawnerCommands) {
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
            Text::new("Sélectionne un objet."),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.94, 0.88, 0.80)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            CraftTextRole::Detail,
        ));
        detail
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                spawn_quantity_button(row, "-10", CraftButtonAction::AdjustQuantity(-10));
                spawn_quantity_button(row, "-1", CraftButtonAction::AdjustQuantity(-1));
                row.spawn((
                    Text::new("1"),
                    ui_text_font(13.0),
                    TextColor(Color::srgb(1.0, 0.92, 0.80)),
                    Node {
                        width: Val::Px(64.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    CraftTextRole::Quantity,
                ));
                spawn_quantity_button(row, "+1", CraftButtonAction::AdjustQuantity(1));
                spawn_quantity_button(row, "+10", CraftButtonAction::AdjustQuantity(10));
                spawn_quantity_button(row, "MAX", CraftButtonAction::SetQuantityMax);
            });
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
                BackgroundColor(Color::srgba(0.38, 0.18, 0.05, 0.98)),
                Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(1.0, 0.62, 0.22, 0.78)),
                CraftButtonAction::QueueSelected,
                QueueCraftButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("AJOUTER À LA FILE"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(1.0, 0.90, 0.76)),
                    CraftTextRole::QueueButton,
                ));
            });
    });
}

fn spawn_quantity_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: CraftButtonAction,
) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(42.0),
                min_height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.09, 0.04, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.94, 0.54, 0.20, 0.60),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.94, 0.84, 0.72)),
            ));
        });
}

fn spawn_craft_queue(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(340.0),
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
            Text::new("FILE ET UNITÉS DISPONIBLES"),
            ui_text_font(12.0),
            TextColor(Color::srgb(1.0, 0.82, 0.58)),
        ));
        queue
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.08, 0.035, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.56, 0.18)),
                    CraftProgressFill,
                ));
            });
        queue.spawn((
            Text::new("File vide."),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.92, 0.84, 0.74)),
            CraftTextRole::Queue,
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
                BackgroundColor(Color::srgba(0.34, 0.08, 0.06, 0.94)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.86, 0.36, 0.28, 0.60),
                ),
                CraftButtonAction::CancelActive,
                CancelCraftButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Annuler la fabrication en cours"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(1.0, 0.82, 0.78)),
                ));
            });
    });
}

fn handle_craft_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<CraftUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyY) {
        let opening = *open_panel != OpenPanel::Craft;
        *open_panel = if opening {
            OpenPanel::Craft
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

    if *open_panel == OpenPanel::Craft && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
    }
}

fn handle_craft_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<CraftUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: CraftButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            CraftButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::Craft;
                *open_panel = if opening {
                    OpenPanel::Craft
                } else {
                    OpenPanel::None
                };
                ui.feedback.clear();
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            CraftButtonAction::Close => *open_panel = OpenPanel::None,
            CraftButtonAction::PreviousColony => {
                cycle_craft_colony(&mut ui, &mut simulation, true);
            }
            CraftButtonAction::NextColony => {
                cycle_craft_colony(&mut ui, &mut simulation, false);
            }
            CraftButtonAction::Select(craftable) => {
                ui.selected = craftable;
                ui.quantity = 1;
                ui.feedback.clear();
            }
            CraftButtonAction::AdjustQuantity(delta) => {
                adjust_craft_quantity(&mut ui, &simulation, delta);
            }
            CraftButtonAction::SetQuantityMax => {
                set_craft_quantity_max(&mut ui, &simulation);
            }
            CraftButtonAction::QueueSelected => queue_selected_craft(&mut simulation, &mut ui),
            CraftButtonAction::CancelActive => cancel_active_craft(&mut simulation, &mut ui),
        }
    }
}

fn capture_craft_feedback(simulation: Res<SimulationResource>, mut ui: ResMut<CraftUiState>) {
    let active_colony_id = simulation.simulation().state().active_colony_id;
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::CraftQueued(queued) if Some(queued.colony_id) == active_colony_id => {
                let definition = craftable_definition(queued.craftable);
                ui.feedback = format!(
                    "{} × {} ajouté à la file.",
                    queued.quantity_requested, definition.name,
                );
            }
            GameEventKind::CraftCompleted(completed)
                if Some(completed.colony_id) == active_colony_id =>
            {
                let definition = craftable_definition(completed.craftable);
                ui.feedback = if completed.quantity_remaining > 0 {
                    format!(
                        "{} : {}/{} unité(s) — {} disponible(s).",
                        definition.name,
                        completed.quantity_completed,
                        completed.quantity_completed + completed.quantity_remaining,
                        completed.inventory_quantity,
                    )
                } else {
                    format!(
                        "{} terminé — {} unité(s) disponible(s).",
                        definition.name, completed.inventory_quantity,
                    )
                };
            }
            GameEventKind::CraftRejected(rejected)
                if Some(rejected.colony_id) == active_colony_id =>
            {
                ui.feedback = format!("Fabrication refusée : {}", craft_error_text(rejected.error));
            }
            GameEventKind::CraftCancelled(cancelled)
                if Some(cancelled.colony_id) == active_colony_id =>
            {
                let definition = craftable_definition(cancelled.craftable);
                ui.feedback = format!(
                    "{} annulé — {} unité(s) conservée(s), {} remboursée(s).",
                    definition.name, cancelled.quantity_completed, cancelled.quantity_refunded,
                );
            }
            GameEventKind::CraftCancellationRejected(rejected)
                if Some(rejected.colony_id) == active_colony_id =>
            {
                ui.feedback = format!("Annulation refusée : {}", craft_error_text(rejected.error));
            }
            _ => {}
        }
    }
}

fn update_craft_visibility(
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<CraftRoot>>,
    mut texts: Query<(&CraftTextRole, &mut Text)>,
) {
    let is_open = *open_panel == OpenPanel::Craft;
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
        if *role == CraftTextRole::Toggle {
            let next = if is_open {
                "Fermer chantier".to_string()
            } else {
                "Chantier flotte  [Y]".to_string()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
    }
}

fn update_craft_summary(
    simulation: Res<SimulationResource>,
    ui: Res<CraftUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&CraftTextRole, &mut Text)>,
) {
    if *open_panel != OpenPanel::Craft {
        return;
    }
    let colony = active_colony(simulation.simulation());

    for (role, mut text) in &mut texts {
        match role {
            CraftTextRole::Title => {
                text.0 = colony.map_or_else(
                    || "CHANTIER SPATIAL".to_string(),
                    |colony| format!("CHANTIER SPATIAL — {}", colony.name),
                );
            }
            CraftTextRole::Summary => {
                text.0 = colony.map_or_else(
                    || "Aucune colonie contrôlée.".to_string(),
                    |colony| {
                        format!(
                            "Cadence : {:.2} point(s)/s  •  file : {}/{}  •  ressources disponibles : {} métal, {} cristal, {} carburant",
                            shipyard_output_points_per_second(colony),
                            colony.craft_queue.len(),
                            max_craft_queue(),
                            colony.resources.available().metal,
                            colony.resources.available().crystal,
                            colony.resources.available().fuel,
                        )
                    },
                );
            }
            CraftTextRole::Feedback => text.0 = ui.feedback.clone(),
            _ => {}
        }
    }
}

fn update_craftable_buttons(
    simulation: Res<SimulationResource>,
    ui: Res<CraftUiState>,
    open_panel: Res<OpenPanel>,
    mut buttons: Query<(
        &CraftableButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
    mut labels: Query<(&CraftableButtonText, &mut Text, &mut TextColor)>,
) {
    if *open_panel != OpenPanel::Craft {
        return;
    }
    let colony = active_colony(simulation.simulation());

    for (button, interaction, mut background, mut outline) in &mut buttons {
        let selected = button.craftable == ui.selected;
        background.0 = craftable_button_color(selected, interaction);
        outline.color = craftable_button_outline(selected);
    }

    for (label, mut text, mut color) in &mut labels {
        let definition = craftable_definition(label.craftable);
        let quantity = colony.map_or(0, |colony| colony.inventory.quantity(label.craftable));
        text.0 = format!(
            "{}\n{}  •  en stock : {}",
            definition.name,
            definition.category.label(),
            quantity,
        );
        color.0 = if label.craftable == ui.selected {
            Color::srgb(1.0, 0.92, 0.80)
        } else {
            Color::srgb(0.92, 0.84, 0.74)
        };
    }
}

fn update_craft_detail(
    simulation: Res<SimulationResource>,
    ui: Res<CraftUiState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&CraftTextRole, &mut Text, &mut TextColor)>,
    mut button: Query<(&Interaction, &mut BackgroundColor, &mut Outline), With<QueueCraftButton>>,
    mut cancel_button: CraftCancelButtonStyleQuery,
) {
    if *open_panel != OpenPanel::Craft {
        return;
    }
    let state = simulation.simulation().state();
    let quote = state
        .active_colony_id
        .map(|colony_id| {
            craft_quote(
                state,
                state.player_faction,
                colony_id,
                ui.selected,
                ui.quantity,
            )
        })
        .unwrap_or(Err(CraftError::NoShipyardCapacity));
    let available = quote.is_ok();
    let detail = craft_detail_text(ui.selected, ui.quantity, quote);
    let button_label = match quote {
        Ok(_) => "AJOUTER À LA FILE".to_string(),
        Err(error) => craft_error_text(error),
    };
    let can_cancel = state
        .active_colony_id
        .and_then(|id| state.colony(id))
        .is_some_and(|colony| !colony.craft_queue.is_empty());

    for (role, mut text, mut color) in &mut texts {
        match role {
            CraftTextRole::Detail => {
                text.0 = detail.clone();
                color.0 = Color::srgb(0.94, 0.88, 0.80);
            }
            CraftTextRole::Quantity => {
                text.0 = ui.quantity.to_string();
            }
            CraftTextRole::QueueButton => {
                text.0 = button_label.clone();
                color.0 = if available {
                    Color::srgb(1.0, 0.90, 0.76)
                } else {
                    Color::srgb(0.64, 0.62, 0.58)
                };
            }
            _ => {}
        }
    }

    for (interaction, mut background, mut outline) in &mut button {
        background.0 = action_button_color(available, false, interaction);
        outline.color = action_button_outline(available, false, interaction);
    }

    for (interaction, mut background, mut outline) in &mut cancel_button {
        background.0 = action_button_color(can_cancel, false, interaction);
        outline.color = action_button_outline(can_cancel, false, interaction);
    }
}

fn update_craft_queue(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&CraftTextRole, &mut Text)>,
    mut progress: Query<&mut Node, With<CraftProgressFill>>,
) {
    if *open_panel != OpenPanel::Craft {
        return;
    }
    let colony = active_colony(simulation.simulation());
    let label = colony.map_or_else(|| "Aucune colonie contrôlée.".to_string(), craft_queue_text);

    for (role, mut text) in &mut texts {
        if *role == CraftTextRole::Queue {
            text.0 = label.clone();
        }
    }

    let ratio = colony
        .and_then(|colony| colony.craft_queue.active())
        .map(craft_progress_ratio)
        .unwrap_or(0.0);
    for mut node in &mut progress {
        node.width = Val::Percent((ratio * 100.0).clamp(0.0, 100.0));
    }
}

fn queue_selected_craft(simulation: &mut SimulationResource, ui: &mut CraftUiState) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        ui.feedback = "Aucune colonie contrôlée.".to_string();
        return;
    };
    let state = simulation.simulation().state();
    match craft_quote(
        state,
        state.player_faction,
        colony_id,
        ui.selected,
        ui.quantity,
    ) {
        Ok(_) => {
            apply_simulation_command(
                simulation,
                GameAction::QueueCraft {
                    colony_id,
                    craftable: ui.selected,
                    quantity: ui.quantity,
                },
            );
            ui.quantity = 1;
        }
        Err(error) => ui.feedback = craft_error_text(error),
    }
}

/// The largest batch the active colony can currently afford for `craftable` — used to clamp
/// both the stepper buttons and the MAX control (MVP-030-A3).
fn max_affordable_craft_quantity(simulation: &SimulationResource, craftable: CraftableId) -> u64 {
    let state = simulation.simulation().state();
    let Some(colony) = state.active_colony_id.and_then(|id| state.colony(id)) else {
        return 1;
    };
    max_affordable_quantity(
        colony.resources.available(),
        craftable_definition(craftable).cost,
    )
    .max(1)
}

fn adjust_craft_quantity(ui: &mut CraftUiState, simulation: &SimulationResource, delta: i64) {
    let max = max_affordable_craft_quantity(simulation, ui.selected).min(MAX_CRAFT_BATCH_QUANTITY);
    let next = (ui.quantity as i64).saturating_add(delta).max(1) as u64;
    ui.quantity = next.min(max);
}

fn set_craft_quantity_max(ui: &mut CraftUiState, simulation: &SimulationResource) {
    ui.quantity =
        max_affordable_craft_quantity(simulation, ui.selected).min(MAX_CRAFT_BATCH_QUANTITY);
}

fn cancel_active_craft(simulation: &mut SimulationResource, ui: &mut CraftUiState) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        ui.feedback = "Aucune colonie contrôlée.".to_string();
        return;
    };
    apply_simulation_command(simulation, GameAction::CancelCraft { colony_id });
}

fn craft_detail_text(
    craftable: CraftableId,
    quantity: u64,
    quote: Result<CraftQuote, CraftError>,
) -> String {
    let definition = craftable_definition(craftable);
    let buildings = definition
        .building_prerequisites
        .iter()
        .map(|prerequisite| {
            let building = galactic_sim::default_building_catalog().definition(prerequisite.kind);
            format!("{} niv. {}", building.name, prerequisite.level)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let technologies = if definition.technology_prerequisites.is_empty() {
        "aucune".to_string()
    } else {
        definition
            .technology_prerequisites
            .iter()
            .map(|technology| technology_definition(*technology).name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let capabilities = if definition.capabilities.is_empty() {
        "aucune".to_string()
    } else {
        definition
            .capabilities
            .iter()
            .map(|capability| format!("{} : {}", capability.id, capability.value))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut lines = vec![
        definition.name.to_uppercase(),
        definition.category.label().to_uppercase(),
        String::new(),
        definition.description.to_string(),
        String::new(),
        format!(
            "Coût unitaire : {} métal • {} cristal • {} carburant",
            definition.cost.metal, definition.cost.crystal, definition.cost.fuel,
        ),
        format!(
            "Durée unitaire : {}",
            format_strategic_duration(StrategicDuration::from_ticks(
                definition.base_duration_ticks,
            )),
        ),
        format!("Bâtiments requis : {buildings}"),
        format!("Technologies requises : {technologies}"),
        ship_stats_text(definition.ship),
        format!("Capacités : {capabilities}"),
        String::new(),
        format!("Quantité demandée : {quantity}"),
        format!(
            "Coût total : {} métal • {} cristal • {} carburant",
            definition.cost.metal * quantity,
            definition.cost.crystal * quantity,
            definition.cost.fuel * quantity,
        ),
    ];

    match quote {
        Ok(value) => lines.extend([
            format!(
                "Durée totale estimée : {}",
                format_strategic_duration(StrategicDuration::from_ticks(
                    value
                        .total_required_work_milli
                        .div_ceil(value.output_milli_per_tick.max(1)),
                )),
            ),
            format!(
                "Quantité finançable actuellement : {}",
                value.max_affordable_quantity
            ),
            String::new(),
            format!(
                "Cadence actuelle : {:.2} point(s)/s",
                value.output_milli_per_tick as f64
                    * f64::from(galactic_sim::STRATEGIC_TICKS_PER_SECOND)
                    / 1_000.0,
            ),
            format!(
                "Résultat : {} unité(s) par exemplaire construit",
                value.result_quantity_per_unit
            ),
        ]),
        Err(error) => lines.extend([
            String::new(),
            format!("BLOCAGE : {}", craft_error_text(error)),
        ]),
    }

    lines.join("\n")
}

fn ship_stats_text(ship: Option<galactic_sim::ShipDefinition>) -> String {
    let Some(ship) = ship else {
        return "Châssis : aucun".to_string();
    };
    let mut lines = vec![
        format!("Rôle : {}", ship_class_label(ship.class)),
        format!(
            "Transit : vitesse {} • portée {} saut(s) • carburant {} / saut",
            ship.cruise_speed, ship.range_hops, ship.fuel_per_hop,
        ),
        format!("Soute : {} unité(s)", ship.cargo_capacity),
    ];
    if let Some(combat) = ship.combat {
        lines.push(format!(
            "Combat : cible {} • attaque {} • défense {} • structure {}",
            combat.target_class.label(),
            combat.offense,
            combat.defense,
            combat.durability,
        ));
        lines.push(format!(
            "Spécialisation : {}",
            combat_bonus_text(combat.bonuses)
        ));
    }
    lines.join("\n")
}

fn combat_bonus_text(bonuses: galactic_sim::CombatTargetBonuses) -> String {
    let entries = bonuses
        .entries()
        .into_iter()
        .filter(|(_, multiplier)| *multiplier != galactic_sim::NEUTRAL_COMBAT_BONUS_PER_MILLE)
        .map(|(target_class, multiplier)| {
            format!(
                "x{:.2} vs {}",
                f64::from(multiplier) / 1_000.0,
                target_class.label(),
            )
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "aucune".to_string()
    } else {
        entries.join(", ")
    }
}

fn craft_queue_text(colony: &galactic_sim::ColonyState) -> String {
    let mut lines = Vec::new();
    if colony.craft_queue.is_empty() {
        let hint = if shipyard_output_milli_per_tick(colony) == 0 {
            format!(
                "Construis un {} pour fabriquer des unités.",
                shipyard_building_name()
            )
        } else {
            "Sélectionne une fabrication disponible.".to_string()
        };
        lines.push(format!(
            "FILE VIDE\n\n{}\n\n{} emplacement(s) disponible(s).",
            hint,
            max_craft_queue(),
        ));
    } else {
        let output = shipyard_output_milli_per_tick(colony);
        for (index, order) in colony.craft_queue.orders().enumerate() {
            let definition = craftable_definition(order.craftable);
            if index == 0 {
                let remaining = if output == 0 {
                    "en pause — aucun chantier actif".to_string()
                } else {
                    format_strategic_duration(StrategicDuration::from_ticks(
                        order.remaining_work_milli().div_ceil(output),
                    ))
                };
                lines.push(format!(
                    "EN COURS\n{}. {} : {}/{} unité(s)\n{:.1} % de l'unité en cours\n{} restante(s)",
                    index + 1,
                    definition.name,
                    order.quantity_completed,
                    order.quantity_requested,
                    craft_progress_ratio(order) * 100.0,
                    remaining,
                ));
            } else {
                lines.push(format!(
                    "\nEN ATTENTE\n{}. {} : {} unité(s)",
                    index + 1,
                    definition.name,
                    order.quantity_requested,
                ));
            }
        }
        lines.push(format!(
            "\n\n{} / {} emplacement(s) utilisé(s)",
            colony.craft_queue.len(),
            max_craft_queue(),
        ));
    }

    lines.push("\n\nINVENTAIRE".to_string());
    let inventory = craftable_catalog()
        .ids()
        .filter_map(|craftable| {
            let quantity = colony.inventory.quantity(craftable);
            (quantity > 0)
                .then(|| format!("{} : {}", craftable_definition(craftable).name, quantity,))
        })
        .collect::<Vec<_>>();
    if inventory.is_empty() {
        lines.push("Aucune unité disponible.".to_string());
    } else {
        lines.extend(inventory);
    }
    lines.join("\n")
}

fn craft_error_text(error: CraftError) -> String {
    match error {
        CraftError::UnknownColony(_) => "Colonie introuvable".to_string(),
        CraftError::Access(_) => "Colonie non contrôlée".to_string(),
        CraftError::UnknownCraftable(craftable) => {
            format!("Fabrication inconnue ({})", craftable.key())
        }
        CraftError::QueueFull { maximum } => format!("File pleine ({maximum})"),
        CraftError::MissingBuilding {
            building,
            required,
            found,
        } => {
            let name = galactic_sim::default_building_catalog()
                .definition(building)
                .name;
            format!("Requiert {name} niv. {required} — actuel {found}")
        }
        CraftError::MissingTechnology(technology) => {
            format!("Requiert {}", technology_definition(technology).name)
        }
        CraftError::NoShipyardCapacity => format!("{} requis", shipyard_building_name()),
        CraftError::InsufficientResources { available, cost } => format!(
            "Ressources insuffisantes — dispo {} métal, {} cristal, {} carburant • coût {} métal, {} cristal, {} carburant",
            available.metal, available.crystal, available.fuel, cost.metal, cost.crystal, cost.fuel,
        ),
        CraftError::InventoryOverflow(_) => "Capacité d'inventaire dépassée".to_string(),
        CraftError::Reservation(_) => "Réservation des ressources impossible".to_string(),
        CraftError::InvalidQuantity { requested, maximum } => {
            format!("Quantité invalide ({requested}, maximum {maximum})")
        }
        CraftError::NoActiveOrder => "Aucune fabrication en cours".to_string(),
    }
}

fn shipyard_building_name() -> &'static str {
    default_building_catalog()
        .definition(BuildingKind::SHIPYARD)
        .name
}

fn active_colony(simulation: &galactic_sim::Simulation) -> Option<&galactic_sim::ColonyState> {
    simulation.state().active_player_colony()
}

fn cycle_craft_colony(ui: &mut CraftUiState, simulation: &mut SimulationResource, reverse: bool) {
    let colonies = simulation.simulation().state().player_colony_ids();
    if colonies.is_empty() {
        return;
    }
    let current = simulation
        .simulation()
        .state()
        .active_colony_id
        .and_then(|active| colonies.iter().position(|colony| *colony == active))
        .unwrap_or(0);
    let next = if reverse {
        current.checked_sub(1).unwrap_or(colonies.len() - 1)
    } else {
        (current + 1) % colonies.len()
    };
    ui.feedback.clear();
    apply_simulation_command(
        simulation,
        GameAction::SelectColony {
            colony_id: colonies[next],
        },
    );
}

fn craftable_button_color(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        return Color::srgba(0.38, 0.18, 0.05, 0.98);
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.31, 0.15, 0.045, 0.98),
        Interaction::Hovered => Color::srgba(0.18, 0.09, 0.03, 0.98),
        Interaction::None => Color::srgba(0.08, 0.05, 0.025, 0.98),
    }
}

fn craftable_button_outline(selected: bool) -> Color {
    if selected {
        Color::srgba(1.0, 0.62, 0.22, 0.82)
    } else {
        Color::srgba(0.54, 0.34, 0.15, 0.48)
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::Simulation;

    use super::*;

    fn first_craftable() -> CraftableId {
        craftable_catalog()
            .ids()
            .next()
            .expect("ruleset defines at least one craftable")
    }

    #[test]
    fn active_colony_returns_the_player_home_colony() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        let colony = active_colony(&simulation).expect("home colony exists");

        assert_eq!(
            Some(colony.id),
            simulation.state().active_player_colony().map(|c| c.id)
        );
    }

    #[test]
    fn craft_error_text_translates_every_variant_to_french() {
        assert_eq!(
            craft_error_text(CraftError::NoShipyardCapacity),
            format!("{} requis", shipyard_building_name())
        );
        assert_eq!(
            craft_error_text(CraftError::QueueFull { maximum: 5 }),
            "File pleine (5)"
        );
    }

    #[test]
    fn craft_detail_text_reports_blocking_error_when_quote_fails() {
        let craftable = first_craftable();
        let quote = Err(CraftError::NoShipyardCapacity);

        let text = craft_detail_text(craftable, 1, quote);

        assert!(text.contains("BLOCAGE"));
        assert!(text.contains(&format!("{} requis", shipyard_building_name())));
    }

    fn simulation_with_shipyard() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
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
            .credit(galactic_domain::ResourceStock::new(1_000, 1_000, 1_000))
            .expect("resource credit fits");
        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
        ]);
        simulation
    }

    #[test]
    fn craft_detail_text_reports_estimate_when_quote_succeeds() {
        let simulation = simulation_with_shipyard();
        let colony_id = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists")
            .id;
        let craftable = CraftableId::LIGHT_PROBE;
        let quote = craft_quote(
            simulation.state(),
            simulation.state().player_faction,
            colony_id,
            craftable,
            1,
        );

        let text = craft_detail_text(craftable, 1, quote);

        assert!(text.contains("Durée totale estimée"));
        assert!(text.contains("Résultat"));
    }

    #[test]
    fn craft_detail_text_shows_military_role_and_target_bonus() {
        let text = craft_detail_text(
            CraftableId::NEEDLE_INTERCEPTOR,
            1,
            Err(CraftError::NoShipyardCapacity),
        );

        assert!(text.contains("Combat : cible Léger"));
        assert!(text.contains("Spécialisation : x1.40 vs Léger"));
    }

    #[test]
    fn craft_detail_text_shows_the_total_cost_of_a_batch() {
        let simulation = simulation_with_shipyard();
        let colony_id = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists")
            .id;
        let craftable = CraftableId::LIGHT_PROBE;
        let unit_cost = craftable_definition(craftable).cost;
        let quantity = 4;
        let quote = craft_quote(
            simulation.state(),
            simulation.state().player_faction,
            colony_id,
            craftable,
            quantity,
        );

        let text = craft_detail_text(craftable, quantity, quote);

        assert!(text.contains(&format!("Quantité demandée : {quantity}")));
        assert!(text.contains(&format!(
            "Coût total : {} métal • {} cristal • {} carburant",
            unit_cost.metal * quantity,
            unit_cost.crystal * quantity,
            unit_cost.fuel * quantity,
        )));
    }

    #[test]
    fn craft_queue_text_shows_empty_queue_hint() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists");

        let text = craft_queue_text(colony);

        assert!(text.contains("FILE VIDE"));
    }

    #[test]
    fn craft_queue_text_reports_active_order_progress() {
        let mut simulation = simulation_with_shipyard();
        let colony_id = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists")
            .id;
        let craftable = CraftableId::LIGHT_PROBE;

        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable,
            quantity: 1,
        });
        let colony = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists");

        let text = craft_queue_text(colony);

        assert!(text.contains("EN COURS"));
    }

    #[test]
    fn craftable_button_color_prioritizes_selected_state() {
        let selected = craftable_button_color(true, &Interaction::Hovered);
        let unselected = craftable_button_color(false, &Interaction::None);

        assert_ne!(selected, unselected);
    }

    #[test]
    fn craftable_button_outline_differs_when_selected() {
        assert_ne!(
            craftable_button_outline(true),
            craftable_button_outline(false)
        );
    }
}

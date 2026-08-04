use bevy::prelude::*;
use galactic_domain::ResourceStock;
use galactic_sim::{GameAction, GameEventKind, SelectionTarget, Simulation};

use crate::presentation::components::*;
use crate::presentation::resource_hud::*;
use crate::presentation::shortcuts::{action_active, action_available};
use crate::presentation::strategic_navigation::*;
use crate::*;

pub(crate) fn handle_colony_management_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut management: ResMut<ColonyManagementState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<navigation_ui::NavigationUiState>,
    interactions: ManagementButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            ManagementButtonAction::Toggle => {
                toggle_colony_management(
                    &mut management,
                    &mut open_panel,
                    &mut simulation,
                    &mut navigation_ui,
                );
            }
            ManagementButtonAction::Close => {
                *open_panel = OpenPanel::None;
            }
            ManagementButtonAction::PreviousColony => {
                cycle_management_colony(&mut management, &mut simulation, true);
            }
            ManagementButtonAction::NextColony => {
                cycle_management_colony(&mut management, &mut simulation, false);
            }
            ManagementButtonAction::SelectBuilding(kind) => {
                management.selected_building = kind;
                management.feedback.clear();
            }
            ManagementButtonAction::UpgradeSelected => {
                queue_selected_management_upgrade(&mut management, &mut simulation);
            }
            ManagementButtonAction::CancelConstruction => {
                cancel_active_construction(&mut management, &mut simulation);
            }
        }
    }
}

pub(crate) fn cancel_active_construction(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        management.feedback = "Aucune colonie active.".to_string();
        return;
    };
    apply_simulation_command(simulation, GameAction::CancelConstruction { colony_id });
}

pub(crate) fn capture_colony_management_feedback(
    simulation: Res<SimulationResource>,
    mut management: ResMut<ColonyManagementState>,
) {
    let active_colony_id = simulation.simulation().state().active_colony_id;
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::ConstructionQueued(queued)
                if Some(queued.colony_id) == active_colony_id =>
            {
                let name = &galactic_sim::default_building_catalog()
                    .definition(queued.order.kind)
                    .name;
                management.feedback = format!(
                    "{} niveau {} ajouté à la file.",
                    name, queued.order.target_level,
                );
            }
            GameEventKind::ConstructionCompleted(completed)
                if Some(completed.colony_id) == active_colony_id =>
            {
                let name = &galactic_sim::default_building_catalog()
                    .definition(completed.kind)
                    .name;
                management.feedback = format!("{} niveau {} terminé.", name, completed.new_level,);
            }
            GameEventKind::ConstructionRejected(rejected)
                if Some(rejected.colony_id) == active_colony_id =>
            {
                management.feedback = format!(
                    "Amélioration refusée : {}",
                    construction_error_text(rejected.error),
                );
            }
            GameEventKind::ConstructionCancelled(cancelled)
                if Some(cancelled.colony_id) == active_colony_id =>
            {
                let name = &galactic_sim::default_building_catalog()
                    .definition(cancelled.kind)
                    .name;
                management.feedback =
                    format!("Amélioration de {name} annulée, ressources remboursées.",);
            }
            GameEventKind::ConstructionCancellationRejected(rejected)
                if Some(rejected.colony_id) == active_colony_id =>
            {
                management.feedback = format!(
                    "Annulation refusée : {}",
                    construction_error_text(rejected.error),
                );
            }
            _ => {}
        }
    }
}

pub(crate) fn update_colony_management_visibility(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<ColonyManagementRoot>>,
    mut texts: Query<(&ManagementTextRole, &mut Text)>,
) {
    let is_open = *open_panel == OpenPanel::Colony;
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

    let colony = active_management_colony(simulation.simulation());
    let colonies = player_colony_ids(simulation.simulation());
    let current_index = colony.and_then(|active| {
        colonies
            .iter()
            .position(|candidate| *candidate == active.id)
    });

    for (role, mut text) in &mut texts {
        let next = match role {
            ManagementTextRole::ToggleLabel => Some(if is_open {
                "Fermer gestion colonie".to_string()
            } else {
                "Gestion colonie  [C]".to_string()
            }),
            ManagementTextRole::Title => Some(
                colony
                    .map(|active| format!("GESTION PLANÉTAIRE — {}", active.name,))
                    .unwrap_or_else(|| "GESTION PLANÉTAIRE".to_string()),
            ),
            ManagementTextRole::Colony => Some(
                colony
                    .map(|active| {
                        let index = current_index.unwrap_or(0) + 1;
                        let planet = simulation
                            .simulation()
                            .universe_repository()
                            .planet(active.planet_id)
                            .map(|value| value.name.as_str())
                            .unwrap_or("Planète");
                        format!("{} / {}  •  {}", index, colonies.len().max(1), planet,)
                    })
                    .unwrap_or_else(|| "Aucune colonie".to_string()),
            ),
            ManagementTextRole::ColonyList => Some(colony_list_label(simulation.simulation())),
            ManagementTextRole::Feedback => Some(management.feedback.clone()),
            ManagementTextRole::BuildingDetail
            | ManagementTextRole::UpgradeLabel
            | ManagementTextRole::Queue => None,
        };
        if let Some(next) = next
            && text.0 != next
        {
            text.0 = next;
        }
    }
}

pub(crate) fn update_colony_management_resources(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ManagementResourceCardText, &mut Text, &mut TextColor)>,
    mut gauges: Query<(
        &ManagementResourceGaugeFill,
        &mut Node,
        &mut BackgroundColor,
    )>,
) {
    if *open_panel != OpenPanel::Colony {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };
    let production = galactic_sim::colony_production_snapshot(colony);

    for (card, mut text, mut color) in &mut texts {
        let view = resource_hud_view(card.kind, colony, production);
        text.0 = view.text;
        color.0 = status_text_color(card.kind, view.status);
    }
    for (gauge, mut node, mut color) in &mut gauges {
        let view = resource_hud_view(gauge.kind, colony, production);
        node.width = Val::Percent((view.fill_ratio * 100.0).clamp(0.0, 100.0));
        color.0 = status_gauge_color(gauge.kind, view.status);
    }
}

pub(crate) fn update_colony_management_buildings(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    open_panel: Res<OpenPanel>,
    mut buttons: Query<(
        &ManagementBuildingButton,
        &Interaction,
        &mut BackgroundColor,
        &mut Outline,
    )>,
    mut labels: Query<(&ManagementBuildingButtonText, &mut Text, &mut TextColor)>,
) {
    if *open_panel != OpenPanel::Colony {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };
    let catalog = galactic_sim::default_building_catalog();
    let projected = galactic_sim::projected_building_levels(colony);

    for (button, interaction, mut background, mut outline) in &mut buttons {
        let selected = button.kind == management.selected_building;
        background.0 = management_building_button_color(selected, interaction);
        outline.color = management_building_button_outline(selected);
    }

    for (label, mut text, mut color) in &mut labels {
        let definition = catalog.definition(label.kind);
        let active_level = colony.buildings.level(label.kind);
        let projected_level = projected.level(label.kind);
        let queue_suffix = if projected_level > active_level {
            format!("  -> {} en file", projected_level)
        } else {
            String::new()
        };
        text.0 = format!(
            "{}
Niveau {}{}",
            definition.name, active_level, queue_suffix,
        );
        color.0 = if label.kind == management.selected_building {
            Color::srgb(0.86, 0.98, 0.94)
        } else {
            Color::srgb(0.78, 0.84, 0.88)
        };
    }
}

pub(crate) fn update_colony_management_detail(
    simulation: Res<SimulationResource>,
    management: Res<ColonyManagementState>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ManagementTextRole, &mut Text, &mut TextColor)>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<ManagementUpgradeButton>,
    >,
) {
    if *open_panel != OpenPanel::Colony {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };

    let kind = management.selected_building;
    let state = simulation.simulation().state();
    let quote = galactic_sim::building_upgrade_quote(state, state.player_faction, colony.id, kind);
    let available = quote.is_ok();
    let detail = building_management_detail_text(colony, kind, quote);
    let upgrade_label = match quote {
        Ok(value) => format!("AMÉLIORER VERS LE NIVEAU {}", value.target_level,),
        Err(error) => construction_error_text(error),
    };

    for (role, mut text, mut color) in &mut texts {
        match role {
            ManagementTextRole::BuildingDetail => {
                text.0 = detail.clone();
                color.0 = Color::srgb(0.84, 0.90, 0.94);
            }
            ManagementTextRole::UpgradeLabel => {
                text.0 = upgrade_label.clone();
                color.0 = if available {
                    Color::srgb(0.86, 0.98, 0.92)
                } else {
                    Color::srgb(0.64, 0.66, 0.66)
                };
            }
            _ => {}
        }
    }

    for (interaction, mut background, mut outline) in &mut buttons {
        background.0 = action_button_color(available, false, interaction);
        outline.color = action_button_outline(available, false, interaction);
    }
}

pub(crate) fn update_colony_management_queue(
    simulation: Res<SimulationResource>,
    open_panel: Res<OpenPanel>,
    mut texts: Query<(&ManagementTextRole, &mut Text)>,
    mut progress: Query<&mut Node, With<ManagementQueueProgressFill>>,
    mut cancel_button: Query<
        (&Interaction, &mut BackgroundColor, &mut Outline),
        With<CancelConstructionButton>,
    >,
) {
    if *open_panel != OpenPanel::Colony {
        return;
    }
    let Some(colony) = active_management_colony(simulation.simulation()) else {
        return;
    };

    let label = construction_queue_detail_label(colony);
    for (role, mut text) in &mut texts {
        if *role == ManagementTextRole::Queue {
            text.0 = label.clone();
        }
    }

    let ratio = construction_progress_ratio(colony);
    for mut node in &mut progress {
        node.width = Val::Percent((ratio * 100.0).clamp(0.0, 100.0));
    }

    let can_cancel = !colony.construction_queue.is_empty();
    for (interaction, mut background, mut outline) in &mut cancel_button {
        background.0 = action_button_color(can_cancel, false, interaction);
        outline.color = action_button_outline(can_cancel, false, interaction);
    }
}

pub(crate) fn toggle_colony_management(
    management: &mut ColonyManagementState,
    open_panel: &mut OpenPanel,
    simulation: &mut SimulationResource,
    navigation_ui: &mut navigation_ui::NavigationUiState,
) {
    let opening = *open_panel != OpenPanel::Colony;
    *open_panel = if opening {
        OpenPanel::Colony
    } else {
        OpenPanel::None
    };
    management.feedback.clear();
    if opening {
        navigation_ui.search_open = false;
        navigation_ui.filters_open = false;
        if let Some(colony_id) = selected_player_colony_id(simulation.simulation()) {
            apply_simulation_command(simulation, GameAction::SelectColony { colony_id });
        }
    }
}

pub(crate) fn selected_player_colony_id(
    simulation: &Simulation,
) -> Option<galactic_domain::ColonyId> {
    let state = simulation.state();
    let colony = match state.selected {
        SelectionTarget::Planet { planet_id, .. } => state.colony_on_planet(planet_id),
        SelectionTarget::System(system_id) => state
            .player_colonies()
            .find(|colony| colony.system_id == system_id),
        SelectionTarget::None => state.active_player_colony(),
    }?;
    state
        .can_manage(state.player_faction, colony.owner)
        .then_some(colony.id)
}

pub(crate) fn player_colony_ids(simulation: &Simulation) -> Vec<galactic_domain::ColonyId> {
    simulation.state().player_colony_ids()
}

pub(crate) fn active_management_colony(
    simulation: &Simulation,
) -> Option<&galactic_sim::ColonyState> {
    simulation.state().active_player_colony()
}

pub(crate) fn colony_list_label(simulation: &Simulation) -> String {
    let state = simulation.state();
    let active = state.active_colony_id;
    let entries = state
        .player_colony_ids()
        .into_iter()
        .filter_map(|colony_id| {
            state.colony(colony_id).map(|colony| {
                let marker = if Some(colony_id) == active { "*" } else { "-" };
                format!("{marker} C{} {}", colony_id.raw(), colony.name)
            })
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        "COLONIES : aucune".to_string()
    } else {
        format!("COLONIES : {}", entries.join("   "))
    }
}

pub(crate) fn transport_cargo_label(cargo: ResourceStock) -> String {
    format!(
        "{} métal, {} cristal, {} carburant",
        cargo.metal, cargo.crystal, cargo.fuel,
    )
}

pub(crate) fn transport_result_label(result: galactic_sim::TransportMissionResult) -> String {
    match result.status {
        galactic_sim::TransportDeliveryStatus::Delivered => format!(
            "Livraison terminée vers C{} : {}.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.delivered),
        ),
        galactic_sim::TransportDeliveryStatus::PartiallyDelivered => format!(
            "Livraison partielle vers C{} : {} livrés, {} revenus, {} encore en soute.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.delivered),
            transport_cargo_label(result.returned),
            transport_cargo_label(result.retained),
        ),
        galactic_sim::TransportDeliveryStatus::DestinationInvalid => format!(
            "Destination C{} invalide : {} revenus, {} encore en soute.",
            result.destination_colony_id.raw(),
            transport_cargo_label(result.returned),
            transport_cargo_label(result.retained),
        ),
        galactic_sim::TransportDeliveryStatus::Pending => "Transport encore en cours.".to_string(),
    }
}

pub(crate) fn cycle_management_colony(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
    reverse: bool,
) {
    let colonies = player_colony_ids(simulation.simulation());
    if colonies.is_empty() {
        return;
    }

    let current = simulation
        .simulation()
        .state()
        .active_colony_id
        .and_then(|active| colonies.iter().position(|id| *id == active))
        .unwrap_or(0);
    let next = if reverse {
        current.checked_sub(1).unwrap_or(colonies.len() - 1)
    } else {
        (current + 1) % colonies.len()
    };
    management.feedback.clear();
    apply_simulation_command(
        simulation,
        GameAction::SelectColony {
            colony_id: colonies[next],
        },
    );
}

pub(crate) fn queue_selected_management_upgrade(
    management: &mut ColonyManagementState,
    simulation: &mut SimulationResource,
) {
    let Some(colony_id) = simulation.simulation().state().active_colony_id else {
        management.feedback = "Aucune colonie active.".to_string();
        return;
    };
    let kind = management.selected_building;
    let state = simulation.simulation().state();
    match galactic_sim::building_upgrade_quote(state, state.player_faction, colony_id, kind) {
        Ok(_) => {
            apply_simulation_command(
                simulation,
                GameAction::QueueBuildingUpgrade { colony_id, kind },
            );
        }
        Err(error) => {
            management.feedback = construction_error_text(error);
        }
    }
}

pub(crate) fn update_action_buttons(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut buttons: ActionButtonStyleQuery,
) {
    for (button, interaction, mut background, mut outline, mut node) in &mut buttons {
        let available = action_available(button.action, &simulation, &navigation);
        let active = action_active(button.action, &simulation, &navigation);
        let next_background = action_button_color(available, active, interaction);
        if background.0 != next_background {
            background.0 = next_background;
        }
        let next_outline = action_button_outline(available, active, interaction);
        if outline.color != next_outline {
            outline.color = next_outline;
        }
        let next_display = if available {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != next_display {
            node.display = next_display;
        }
    }
}

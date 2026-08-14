// Playtest feedback: no way to know when something finishes without keeping
// the relevant panel open and watching it. A small, always-visible toast
// stack — not gated by any `OpenPanel` state, unlike every other screen in
// this module family — fixes that without duplicating any panel's own
// detailed queue view.
//
// Covers construction (building upgrades), craft (ship batches, only once
// the whole batch is done — `quantity_remaining == 0`, not per-unit) and
// research completions, across *all* of the player's colonies (not just the
// currently active one). Research has no colony to jump to, so its toasts
// carry `colony_id: None` and are dismiss-only.
//
// Placed above every other screen's `GlobalZIndex`, including the combat
// modal — playtest feedback was explicit that this must stay visible
// regardless of which screen the player is on.

use std::time::Duration;

use bevy::prelude::*;
use galactic_domain::ColonyId;
use galactic_sim::{
    GameEventKind, SelectionTarget, craftable_definition, default_building_catalog,
    technology_definition,
};

use super::{
    NavigationHistory, PresentationUpdateSet, SimulationResource, StrategicNavigation,
    UiPointerBlocker, ViewRebuildRequest, collect_presentation_events, navigate_to_selection,
    panel_background, panel_outline, ui_text_font,
};

const MAX_TOASTS: usize = 5;
const TOAST_DURATION_SECS: f32 = 6.0;
/// Above the resource bar (top ≈ 10-56px) and the top-left corner is
/// otherwise unused — the inspector panel occupies the mirrored top-right
/// spot, nothing currently spawns anything on the left below the resource
/// bar. Stacks downward, capped at `MAX_TOASTS`, well clear of any
/// full-screen panel (those all start at `top: 112px`).
const NOTIFICATIONS_TOP_PX: f32 = 64.0;
/// Above `combat_ui.rs`'s 200 — the highest z-index anywhere else in the
/// client — so a toast survives even while the combat modal is open.
const NOTIFICATIONS_Z_INDEX: i32 = 210;

pub(crate) struct NotificationsUiPlugin;

impl Plugin for NotificationsUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NotificationsUiState>()
            .add_systems(Startup, spawn_notifications_screen)
            .add_systems(
                Update,
                capture_completion_notifications
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (expire_toasts, handle_toast_buttons)
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                update_toast_rows.in_set(PresentationUpdateSet::Management),
            );
    }
}

struct Toast {
    colony_id: Option<ColonyId>,
    message: String,
    expires_at: Duration,
}

#[derive(Resource, Default)]
pub(crate) struct NotificationsUiState {
    toasts: Vec<Toast>,
}

#[derive(Component)]
struct ToastRow(usize);

#[derive(Component)]
struct ToastRowText(usize);

fn spawn_notifications_screen(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(NOTIFICATIONS_TOP_PX),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            GlobalZIndex(NOTIFICATIONS_Z_INDEX),
        ))
        .with_children(|root| {
            for slot in 0..MAX_TOASTS {
                spawn_toast_row(root, slot);
            }
        });
}

fn spawn_toast_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Visibility::Hidden,
            ToastRow(slot),
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.86, 0.94, 0.90)),
                ToastRowText(slot),
            ));
        });
}

fn capture_completion_notifications(
    mut ui: ResMut<NotificationsUiState>,
    simulation: Res<SimulationResource>,
    time: Res<Time>,
) {
    let expires_at = time.elapsed() + Duration::from_secs_f32(TOAST_DURATION_SECS);
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::ConstructionCompleted(completed) => {
                let Some(colony) = simulation.simulation().state().colony(completed.colony_id)
                else {
                    continue;
                };
                let building_name = default_building_catalog().definition(completed.kind).name;
                push_toast(
                    &mut ui,
                    Some(completed.colony_id),
                    format!(
                        "{} : {} niveau {} terminé.",
                        colony.name, building_name, completed.new_level
                    ),
                    expires_at,
                );
            }
            GameEventKind::CraftCompleted(completed) if completed.quantity_remaining == 0 => {
                let Some(colony) = simulation.simulation().state().colony(completed.colony_id)
                else {
                    continue;
                };
                let definition = craftable_definition(completed.craftable);
                push_toast(
                    &mut ui,
                    Some(completed.colony_id),
                    format!(
                        "{} : {} × {} terminé.",
                        colony.name, completed.quantity_completed, definition.name
                    ),
                    expires_at,
                );
            }
            GameEventKind::ResearchCompleted(completed) => {
                let definition = technology_definition(completed.technology);
                push_toast(
                    &mut ui,
                    None,
                    format!(
                        "{} terminé — {} débloqué.",
                        definition.name, definition.unlock_label
                    ),
                    expires_at,
                );
            }
            _ => {}
        }
    }
}

fn push_toast(
    ui: &mut NotificationsUiState,
    colony_id: Option<ColonyId>,
    message: String,
    expires_at: Duration,
) {
    ui.toasts.push(Toast {
        colony_id,
        message,
        expires_at,
    });
    if ui.toasts.len() > MAX_TOASTS {
        ui.toasts.remove(0);
    }
}

fn expire_toasts(mut ui: ResMut<NotificationsUiState>, time: Res<Time>) {
    let now = time.elapsed();
    ui.toasts.retain(|toast| toast.expires_at > now);
}

type ToastInteractionQuery<'w, 's> =
    Query<'w, 's, (&'static Interaction, &'static ToastRow), Changed<Interaction>>;

fn handle_toast_buttons(
    mut ui: ResMut<NotificationsUiState>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut history: ResMut<NavigationHistory>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    interactions: ToastInteractionQuery,
) {
    for (interaction, row) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(toast) = ui.toasts.get(row.0) else {
            continue;
        };
        let colony = toast
            .colony_id
            .and_then(|colony_id| simulation.simulation().state().colony(colony_id));
        if let Some(colony) = colony {
            let target = SelectionTarget::Planet {
                system_id: colony.system_id,
                planet_id: colony.planet_id,
            };
            navigate_to_selection(
                &mut simulation,
                &mut navigation,
                &mut history,
                &mut rebuild,
                target,
                true,
            );
        }
        if row.0 < ui.toasts.len() {
            ui.toasts.remove(row.0);
        }
        // At most one toast handled per frame — clicking several in the same
        // frame is not a realistic input, and handling more than one here
        // would shift the remaining slots' indices mid-iteration.
        return;
    }
}

type ToastRowVisibilityQuery<'w, 's> = Query<'w, 's, (&'static ToastRow, &'static mut Visibility)>;
type ToastRowTextQuery<'w, 's> = Query<'w, 's, (&'static ToastRowText, &'static mut Text)>;

fn update_toast_rows(
    ui: Res<NotificationsUiState>,
    mut rows: ToastRowVisibilityQuery,
    mut texts: ToastRowTextQuery,
) {
    for (row, mut visibility) in &mut rows {
        let next = if ui.toasts.get(row.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (marker, mut text) in &mut texts {
        let next = ui
            .toasts
            .get(marker.0)
            .map(|toast| toast.message.clone())
            .unwrap_or_default();
        if text.0 != next {
            text.0 = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use galactic_domain::UniverseConfig;
    use galactic_sim::{
        BuildingKind, CraftCompleted, CraftableId, GameEvent, ResearchCompleted, Simulation,
        StrategicTick, TechnologyId, TechnologyUnlock,
    };

    use super::*;

    // A real end-to-end run — Startup spawn, then the capture and row-update
    // systems, on a shared `World` — rather than only asserting on
    // `NotificationsUiState` in isolation. This module previously had zero
    // coverage of its own entity-spawning/query wiring, unlike every other
    // screen in this family.
    #[test]
    fn full_pipeline_shows_a_toast_for_each_completion_kind() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .active_colony_id
            .expect("a freshly started game has an active home colony");
        let actor = simulation.state().player_faction;

        let mut world = World::new();
        world.insert_resource(NotificationsUiState::default());
        world.insert_resource(Time::<()>::default());
        world.insert_resource(SimulationResource {
            simulation,
            pending_events: vec![
                GameEvent::new(
                    actor,
                    StrategicTick::ZERO,
                    GameEventKind::ConstructionCompleted(construction_completed(colony_id)),
                ),
                GameEvent::new(
                    actor,
                    StrategicTick::ZERO,
                    GameEventKind::CraftCompleted(craft_completed(colony_id)),
                ),
                GameEvent::new(
                    actor,
                    StrategicTick::ZERO,
                    GameEventKind::ResearchCompleted(research_completed()),
                ),
            ],
        });

        world
            .run_system_once(spawn_notifications_screen)
            .expect("spawn_notifications_screen runs");
        world
            .run_system_once(capture_completion_notifications)
            .expect("capture_completion_notifications runs");
        world
            .run_system_once(update_toast_rows)
            .expect("update_toast_rows runs");

        let ui = world.resource::<NotificationsUiState>();
        assert_eq!(ui.toasts.len(), 3);
        assert_eq!(ui.toasts[2].colony_id, None, "research has no colony");

        let mut rows = world.query::<(&ToastRow, &Visibility)>();
        let visible = rows
            .iter(&world)
            .filter(|(_, visibility)| **visibility == Visibility::Inherited)
            .count();
        assert_eq!(visible, 3, "exactly the 3 pushed toasts become visible");

        let mut texts = world.query::<(&ToastRowText, &Text)>();
        let messages: Vec<String> = texts.iter(&world).map(|(_, text)| text.0.clone()).collect();
        assert!(messages.iter().any(|message| message.contains("niveau 2")));
        assert!(messages.iter().any(|message| {
            message.contains(
                &craftable_definition(CraftableId::FRIGATE_BULWARK)
                    .name
                    .to_string(),
            )
        }));
        assert!(messages.iter().any(|message| message.contains("débloqué")));
    }

    #[test]
    fn a_craft_batch_still_in_progress_does_not_push_a_toast() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .active_colony_id
            .expect("a freshly started game has an active home colony");
        let actor = simulation.state().player_faction;

        let mut world = World::new();
        world.insert_resource(NotificationsUiState::default());
        world.insert_resource(Time::<()>::default());
        world.insert_resource(SimulationResource {
            simulation,
            pending_events: vec![GameEvent::new(
                actor,
                StrategicTick::ZERO,
                GameEventKind::CraftCompleted(CraftCompleted {
                    colony_id,
                    craftable: CraftableId::FRIGATE_BULWARK,
                    quantity_completed: 1,
                    quantity_remaining: 2,
                    inventory_quantity: 1,
                }),
            )],
        });

        world
            .run_system_once(capture_completion_notifications)
            .expect("capture_completion_notifications runs");

        assert!(world.resource::<NotificationsUiState>().toasts.is_empty());
    }

    fn construction_completed(
        colony_id: galactic_domain::ColonyId,
    ) -> galactic_sim::ConstructionCompleted {
        galactic_sim::ConstructionCompleted {
            colony_id,
            kind: BuildingKind::RESEARCH_LAB,
            new_level: 2,
        }
    }

    fn craft_completed(colony_id: galactic_domain::ColonyId) -> CraftCompleted {
        CraftCompleted {
            colony_id,
            craftable: CraftableId::FRIGATE_BULWARK,
            quantity_completed: 3,
            quantity_remaining: 0,
            inventory_quantity: 3,
        }
    }

    fn research_completed() -> ResearchCompleted {
        ResearchCompleted {
            technology: TechnologyId::SPATIAL_DETECTION,
            unlock: TechnologyUnlock::DetectUnknownSystems,
        }
    }
}

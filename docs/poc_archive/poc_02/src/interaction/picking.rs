use bevy::{ecs::system::SystemParam, prelude::*};

use crate::data::{ActiveSystem, Notifications, OrbitAnimation, SelectableId, Selection};
use crate::interaction::{LastClick, Selectable};
use crate::state::ViewState;
use crate::usability::UsabilityMetrics;

pub fn selectable_over(
    event: On<Pointer<Over>>,
    mut selection: ResMut<Selection>,
    query: Query<&Selectable>,
) {
    if let Ok(selectable) = query.get(event.event_target()) {
        selection.hovered = Some(selectable.id);
    }
}

pub fn selectable_out(
    event: On<Pointer<Out>>,
    mut selection: ResMut<Selection>,
    query: Query<&Selectable>,
) {
    if let Ok(selectable) = query.get(event.event_target())
        && selection.hovered == Some(selectable.id)
    {
        selection.hovered = None;
    }
}

#[derive(SystemParam)]
pub struct ClickParams<'w> {
    time: Res<'w, Time>,
    state: Res<'w, State<ViewState>>,
    selection: ResMut<'w, Selection>,
    last_click: ResMut<'w, LastClick>,
    active_system: ResMut<'w, ActiveSystem>,
    next_state: ResMut<'w, NextState<ViewState>>,
    notifications: ResMut<'w, Notifications>,
    animation: ResMut<'w, OrbitAnimation>,
    metrics: ResMut<'w, UsabilityMetrics>,
}

pub fn selectable_click(
    event: On<Pointer<Click>>,
    mut params: ClickParams,
    query: Query<&Selectable>,
) {
    let Ok(selectable) = query.get(event.event_target()) else {
        return;
    };

    let now = params.time.elapsed_secs_f64();
    let double_click =
        params.last_click.id == Some(selectable.id) && now - params.last_click.time <= 0.35;
    params.last_click.id = Some(selectable.id);
    params.last_click.time = now;
    params.selection.selected = Some(selectable.id);
    params.metrics.selection_count += 1;

    match (params.state.get(), selectable.id) {
        (ViewState::Galaxy, SelectableId::System(system_id)) if double_click => {
            params.active_system.id = Some(system_id);
            params.next_state.set(ViewState::System);
            params.animation.paused = false;
            params.metrics.view_transition_count += 1;
            params.notifications.show("Ouverture du systeme");
        }
        (ViewState::System, SelectableId::Star(_)) => {
            params.notifications.show("Etoile selectionnee");
        }
        (ViewState::System, SelectableId::Planet(_, _)) => {
            params.notifications.show("Planete selectionnee");
        }
        (ViewState::System, SelectableId::Moon(_, _, _)) => {
            params.notifications.show("Lune selectionnee");
        }
        _ => {}
    }
}

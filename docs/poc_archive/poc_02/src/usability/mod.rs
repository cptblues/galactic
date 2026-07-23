pub mod metrics;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::data::{GalaxyData, Notifications, SelectableId, Selection};
use crate::strategic::{
    MissionState, MissionTarget, StrategicGalaxyData, validate_planet_for_mission,
};

pub use metrics::*;

pub struct UsabilityPlugin;

impl Plugin for UsabilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UsabilityMetrics>()
            .init_resource::<MissionState>()
            .add_systems(Update, handle_mission_keys);
    }
}

#[derive(SystemParam)]
struct MissionParams<'w> {
    time: Res<'w, Time>,
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    selection: Res<'w, Selection>,
    mission: ResMut<'w, MissionState>,
    metrics: ResMut<'w, UsabilityMetrics>,
    notifications: ResMut<'w, Notifications>,
}

fn handle_mission_keys(keys: Res<ButtonInput<KeyCode>>, mut params: MissionParams) {
    if keys.just_pressed(KeyCode::KeyM) {
        params.mission.active = true;
        params.mission.completed = false;
        params.mission.result = Some(
            "Objectif: trouvez une planete oceanique non colonisee, a trois routes ou moins d'un allie, hors territoire hostile.".to_string(),
        );
        let now = params.time.elapsed_secs_f64();
        params.metrics.reset_for_mission(now);
        params.notifications.show("Mission lancee");
    }

    if keys.just_pressed(KeyCode::KeyY) {
        let Some(SelectableId::Planet(system, planet)) = params.selection.selected else {
            params.mission.result = Some("Selectionnez une planete avant validation.".to_string());
            params.notifications.show("Mission: cible invalide");
            return;
        };
        let target = MissionTarget { system, planet };
        match validate_planet_for_mission(&params.galaxy, &params.strategic, target) {
            crate::strategic::MissionValidation::Success => {
                params.mission.completed = true;
                params.mission.result = Some(format!(
                    "Succes mission en {:.1}s - selections {} filtres {} retours {} ambigus {}",
                    params
                        .metrics
                        .mission_started_at_secs
                        .map(|start| params.time.elapsed_secs_f64() - start)
                        .unwrap_or(0.0),
                    params.metrics.selection_count,
                    params.metrics.filter_change_count,
                    params.metrics.navigation_back_count,
                    params.metrics.ambiguous_selection_count
                ));
                params.notifications.show("Mission reussie");
            }
            crate::strategic::MissionValidation::Failure(reasons) => {
                params.mission.result = Some(format!("Echec mission: {}", reasons.join(", ")));
                params.notifications.show("Mission: mauvaise cible");
            }
        }
    }
}

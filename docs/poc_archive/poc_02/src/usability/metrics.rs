use bevy::prelude::*;

#[derive(Resource, Default, Clone, Debug)]
pub struct UsabilityMetrics {
    pub mission_started_at_secs: Option<f64>,
    pub selection_count: u32,
    pub ambiguous_selection_count: u32,
    pub navigation_back_count: u32,
    pub filter_change_count: u32,
    pub search_count: u32,
    pub view_transition_count: u32,
}

impl UsabilityMetrics {
    pub fn reset_for_mission(&mut self, now: f64) {
        *self = Self {
            mission_started_at_secs: Some(now),
            ..default()
        };
    }
}

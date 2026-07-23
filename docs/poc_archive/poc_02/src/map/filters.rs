use bevy::prelude::*;

use crate::data::{FactionId, SelectableId};
use crate::interaction::Selectable;
use crate::strategic::StrategicGalaxyData;
use crate::strategic::{ExplorationState, SystemStrategicState};

#[derive(Resource, Clone, Debug)]
pub struct MapFilters {
    pub labels: bool,
    pub major_routes: bool,
    pub minor_routes: bool,
    pub borders: bool,
    pub influence: bool,
    pub fleets: bool,
    pub alerts: bool,
    pub anomalies: bool,
    pub habitable_worlds: bool,
    pub unknown_systems: bool,
    pub faction_filter: Option<FactionId>,
    pub exploration_filter: Option<ExplorationState>,
}

impl Default for MapFilters {
    fn default() -> Self {
        Self {
            labels: true,
            major_routes: true,
            minor_routes: true,
            borders: true,
            influence: true,
            fleets: true,
            alerts: true,
            anomalies: true,
            habitable_worlds: true,
            unknown_systems: true,
            faction_filter: None,
            exploration_filter: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterPreset {
    Exploration,
    Diplomacy,
    Navigation,
    Minimal,
}

impl MapFilters {
    pub fn system_visible(&self, state: Option<&SystemStrategicState>) -> bool {
        let Some(state) = state else {
            return true;
        };
        if !self.unknown_systems && state.exploration == ExplorationState::Unknown {
            return false;
        }
        if self
            .faction_filter
            .is_some_and(|faction| state.control.controlling_faction() != Some(faction))
        {
            return false;
        }
        if self
            .exploration_filter
            .is_some_and(|exploration| state.exploration != exploration)
        {
            return false;
        }
        true
    }

    pub fn apply_preset(&mut self, preset: FilterPreset) {
        *self = match preset {
            FilterPreset::Exploration => Self {
                labels: true,
                major_routes: true,
                minor_routes: false,
                borders: false,
                influence: false,
                fleets: false,
                alerts: true,
                anomalies: true,
                habitable_worlds: true,
                unknown_systems: true,
                faction_filter: None,
                exploration_filter: None,
            },
            FilterPreset::Diplomacy => Self {
                labels: true,
                major_routes: true,
                minor_routes: false,
                borders: true,
                influence: true,
                fleets: false,
                alerts: true,
                anomalies: false,
                habitable_worlds: false,
                unknown_systems: false,
                faction_filter: None,
                exploration_filter: None,
            },
            FilterPreset::Navigation => Self {
                labels: true,
                major_routes: true,
                minor_routes: true,
                borders: false,
                influence: false,
                fleets: true,
                alerts: false,
                anomalies: false,
                habitable_worlds: false,
                unknown_systems: false,
                faction_filter: None,
                exploration_filter: None,
            },
            FilterPreset::Minimal => Self {
                labels: true,
                major_routes: false,
                minor_routes: false,
                borders: false,
                influence: false,
                fleets: false,
                alerts: false,
                anomalies: false,
                habitable_worlds: false,
                unknown_systems: false,
                faction_filter: None,
                exploration_filter: None,
            },
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicsPreset {
    Low,
    Medium,
    High,
}

#[derive(Resource, Clone, Debug)]
pub struct GraphicsSettings {
    pub preset: GraphicsPreset,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            preset: GraphicsPreset::High,
        }
    }
}

impl GraphicsSettings {
    pub fn cycle(&mut self) {
        self.preset = match self.preset {
            GraphicsPreset::Low => GraphicsPreset::Medium,
            GraphicsPreset::Medium => GraphicsPreset::High,
            GraphicsPreset::High => GraphicsPreset::Low,
        };
    }
}

pub fn apply_system_visibility(
    filters: Res<MapFilters>,
    strategic: Res<StrategicGalaxyData>,
    mut query: Query<(&Selectable, &mut Visibility)>,
) {
    if !filters.is_changed() && !strategic.is_changed() {
        return;
    }
    for (selectable, mut visibility) in &mut query {
        let SelectableId::System(system_id) = selectable.id else {
            continue;
        };
        *visibility = if filters.system_visible(strategic.system_states.get(&system_id)) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

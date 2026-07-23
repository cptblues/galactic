use bevy::prelude::*;

use crate::camera::{MainCamera, OrbitCamera};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum SemanticZoomLevel {
    #[default]
    GalaxyOverview,
    Regional,
    Local,
    SystemApproach,
}

impl SemanticZoomLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::GalaxyOverview => "Galaxie",
            Self::Regional => "Secteur",
            Self::Local => "Local",
            Self::SystemApproach => "Approche systeme",
        }
    }

    pub fn label_budget(self) -> usize {
        match self {
            Self::GalaxyOverview => 12,
            Self::Regional => 25,
            Self::Local => 50,
            Self::SystemApproach => 20,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SemanticZoomThresholds {
    pub system_approach: f32,
    pub local: f32,
    pub regional: f32,
    pub hysteresis: f32,
}

impl Default for SemanticZoomThresholds {
    fn default() -> Self {
        Self {
            system_approach: 38.0,
            local: 92.0,
            regional: 185.0,
            hysteresis: 0.08,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct SemanticZoomState {
    pub level: SemanticZoomLevel,
    pub thresholds: SemanticZoomThresholds,
}

impl Default for SemanticZoomState {
    fn default() -> Self {
        Self {
            level: SemanticZoomLevel::GalaxyOverview,
            thresholds: SemanticZoomThresholds::default(),
        }
    }
}

pub fn update_semantic_zoom(
    camera: Query<&OrbitCamera, With<MainCamera>>,
    mut zoom: ResMut<SemanticZoomState>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    zoom.level = level_for_distance(camera.distance, zoom.level, zoom.thresholds);
}

pub fn level_for_distance(
    distance: f32,
    current: SemanticZoomLevel,
    thresholds: SemanticZoomThresholds,
) -> SemanticZoomLevel {
    let margin = thresholds.hysteresis.clamp(0.0, 0.25);
    let enter_system = thresholds.system_approach;
    let exit_system = enter_system * (1.0 + margin);
    let enter_local = thresholds.local;
    let exit_local = enter_local * (1.0 + margin);
    let enter_regional = thresholds.regional;
    let exit_regional = enter_regional * (1.0 + margin);

    match current {
        SemanticZoomLevel::SystemApproach if distance <= exit_system => current,
        SemanticZoomLevel::Local if distance > enter_system && distance <= exit_local => current,
        SemanticZoomLevel::Regional if distance > enter_local && distance <= exit_regional => {
            current
        }
        SemanticZoomLevel::GalaxyOverview if distance > enter_regional * (1.0 - margin) => current,
        _ if distance <= enter_system => SemanticZoomLevel::SystemApproach,
        _ if distance <= enter_local => SemanticZoomLevel::Local,
        _ if distance <= enter_regional => SemanticZoomLevel::Regional,
        _ => SemanticZoomLevel::GalaxyOverview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_zoom_uses_thresholds() {
        let thresholds = SemanticZoomThresholds::default();
        assert_eq!(
            level_for_distance(20.0, SemanticZoomLevel::GalaxyOverview, thresholds),
            SemanticZoomLevel::SystemApproach
        );
        assert_eq!(
            level_for_distance(120.0, SemanticZoomLevel::GalaxyOverview, thresholds),
            SemanticZoomLevel::Regional
        );
        assert_eq!(
            level_for_distance(240.0, SemanticZoomLevel::Local, thresholds),
            SemanticZoomLevel::GalaxyOverview
        );
    }

    #[test]
    fn semantic_zoom_has_hysteresis() {
        let thresholds = SemanticZoomThresholds::default();
        assert_eq!(
            level_for_distance(39.0, SemanticZoomLevel::SystemApproach, thresholds),
            SemanticZoomLevel::SystemApproach
        );
    }
}

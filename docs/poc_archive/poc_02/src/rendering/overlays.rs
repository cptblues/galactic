use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

use crate::data::GalaxyData;
use crate::map::{
    MapFilters, MapProjectionState, SemanticZoomLevel, SemanticZoomState, projected_position,
};
use crate::strategic::{ControlState, FactionColor, FleetImportance, StrategicGalaxyData};

pub fn draw_territory_halos(
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    filters: Res<MapFilters>,
    projection: Res<MapProjectionState>,
    zoom: Res<SemanticZoomState>,
    mut gizmos: Gizmos,
) {
    if !filters.influence && !filters.borders {
        return;
    }
    let alpha = match zoom.level {
        SemanticZoomLevel::GalaxyOverview => 0.28,
        SemanticZoomLevel::Regional => 0.22,
        SemanticZoomLevel::Local => 0.14,
        SemanticZoomLevel::SystemApproach => 0.08,
    };
    for system in &galaxy.systems {
        let Some(state) = strategic.system_states.get(&system.id) else {
            continue;
        };
        let Some(faction) = state
            .control
            .controlling_faction()
            .and_then(|id| strategic.faction(id))
        else {
            continue;
        };
        let radius = match state.control {
            ControlState::Capital(_) => 2.7,
            ControlState::Colonized(_) => 2.1,
            ControlState::Outpost(_) => 1.65,
            ControlState::Contested(_, _) => 2.35,
            ControlState::Unclaimed => 0.0,
        };
        if radius <= 0.0 {
            continue;
        }
        let center = projected_position(system.position, projection.blend);
        draw_ring(
            &mut gizmos,
            center,
            radius,
            color_with_alpha(faction.ui_color, alpha),
        );
        if faction.disposition.is_hostile() {
            draw_ring(
                &mut gizmos,
                center + Vec3::Y * 0.05,
                radius * 1.18,
                Color::srgba(1.0, 0.16, 0.2, alpha * 0.85),
            );
        }
    }
}

pub fn draw_map_markers(
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    filters: Res<MapFilters>,
    projection: Res<MapProjectionState>,
    mut gizmos: Gizmos,
) {
    for system in &galaxy.systems {
        let base = projected_position(system.position, projection.blend);
        if filters.habitable_worlds && system.tags.has_habitable_world {
            gizmos.line(
                base + Vec3::new(-0.55, 1.15, 0.0),
                base + Vec3::new(0.55, 1.15, 0.0),
                Color::srgba(0.26, 1.0, 0.58, 0.82),
            );
            gizmos.line(
                base + Vec3::new(0.0, 0.68, -0.55),
                base + Vec3::new(0.0, 0.68, 0.55),
                Color::srgba(0.26, 1.0, 0.58, 0.62),
            );
        }
        if filters.anomalies && system.tags.anomaly_detected {
            draw_ring(
                &mut gizmos,
                base + Vec3::Y * 0.2,
                1.05,
                Color::srgba(0.9, 0.45, 1.0, 0.7),
            );
        }
        if filters.alerts
            && strategic
                .system_states
                .get(&system.id)
                .map(|state| !state.alerts.is_empty())
                .unwrap_or(false)
        {
            gizmos.line(
                base + Vec3::Y * 0.2,
                base + Vec3::Y * 2.1,
                Color::srgba(1.0, 0.72, 0.2, 0.82),
            );
        }
    }
}

pub fn draw_fleets(
    time: Res<Time>,
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    filters: Res<MapFilters>,
    projection: Res<MapProjectionState>,
    zoom: Res<SemanticZoomState>,
    mut gizmos: Gizmos,
) {
    if !filters.fleets {
        return;
    }

    if matches!(zoom.level, SemanticZoomLevel::GalaxyOverview) {
        let mut sector_counts = HashMap::<_, usize>::new();
        for fleet in &strategic.fleets {
            if let Some(system_id) = fleet.route.first()
                && let Some(sector) = strategic
                    .system_states
                    .get(system_id)
                    .map(|state| state.sector)
            {
                *sector_counts.entry(sector).or_default() += 1;
            }
        }
        for (sector_id, count) in sector_counts {
            let Some(sector) = strategic
                .sectors
                .iter()
                .find(|sector| sector.id == sector_id)
            else {
                continue;
            };
            let center = projected_position(sector.center, projection.blend);
            draw_ring(
                &mut gizmos,
                center + Vec3::Y * 1.0,
                1.0 + count as f32 * 0.12,
                Color::srgba(0.95, 0.95, 1.0, 0.52),
            );
        }
        return;
    }

    for fleet in &strategic.fleets {
        let Some(position) = fleet_position(&galaxy, fleet, time.elapsed_secs()) else {
            continue;
        };
        let Some(faction) = strategic.faction(fleet.faction) else {
            continue;
        };
        let projected = projected_position(position, projection.blend);
        let size = if fleet.importance == FleetImportance::Major {
            0.8
        } else {
            0.48
        };
        gizmos.line(
            projected + Vec3::new(-size, 0.9, 0.0),
            projected + Vec3::new(size, 0.9, 0.0),
            color_with_alpha(faction.ui_color, 0.78),
        );
        gizmos.line(
            projected + Vec3::new(0.0, 0.9, -size),
            projected + Vec3::new(0.0, 0.9, size),
            color_with_alpha(faction.ui_color, 0.78),
        );
    }
}

fn fleet_position(
    galaxy: &GalaxyData,
    fleet: &crate::strategic::FleetData,
    elapsed: f32,
) -> Option<Vec3> {
    if fleet.route.len() < 2 {
        return None;
    }
    let segment_count = fleet.route.len() - 1;
    let travel = (fleet.progress + elapsed * 0.035) * segment_count as f32;
    let segment = travel.floor() as usize % segment_count;
    let t = travel.fract();
    let a = galaxy.find_system(fleet.route[segment])?.position;
    let b = galaxy.find_system(fleet.route[segment + 1])?.position;
    Some(a.lerp(b, t))
}

pub fn color_with_alpha(color: FactionColor, alpha: f32) -> Color {
    Color::srgba(color.r, color.g, color.b, alpha)
}

fn draw_ring(gizmos: &mut Gizmos, center: Vec3, radius: f32, color: Color) {
    let segments = 32;
    for index in 0..segments {
        let a = index as f32 / segments as f32 * TAU;
        let b = (index + 1) as f32 / segments as f32 * TAU;
        gizmos.line(
            center + Vec3::new(radius * a.cos(), 0.0, radius * a.sin()),
            center + Vec3::new(radius * b.cos(), 0.0, radius * b.sin()),
            color,
        );
    }
}

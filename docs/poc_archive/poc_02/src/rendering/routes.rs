use bevy::prelude::*;

use crate::data::{GalaxyData, SelectableId, Selection};
use crate::map::{MapFilters, MapProjectionState, projected_position};
use crate::navigation::HighlightedPath;
use crate::strategic::{RouteKind, StrategicGalaxyData, route_key};

pub fn draw_galaxy_routes(
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    filters: Res<MapFilters>,
    projection: Res<MapProjectionState>,
    highlighted_path: Res<HighlightedPath>,
    selection: Res<Selection>,
    mut gizmos: Gizmos,
) {
    let selected_system = selection.selected.map(SelectableId::system_id);
    for route in &galaxy.routes {
        let kind = strategic
            .route_kinds
            .get(&route_key(route.a, route.b))
            .copied()
            .unwrap_or(RouteKind::Minor);
        if matches!(kind, RouteKind::Major) && !filters.major_routes {
            continue;
        }
        if matches!(kind, RouteKind::Minor) && !filters.minor_routes {
            continue;
        }
        let Some(a) = galaxy.find_system(route.a) else {
            continue;
        };
        let Some(b) = galaxy.find_system(route.b) else {
            continue;
        };
        let highlighted = selected_system
            .map(|id| route.a == id || route.b == id)
            .unwrap_or(false);
        let color = if highlighted {
            Color::srgba(0.55, 0.95, 1.0, 0.58)
        } else if matches!(kind, RouteKind::Major) {
            Color::srgba(0.34, 0.56, 0.96, 0.34)
        } else {
            Color::srgba(0.18, 0.32, 0.56, 0.14)
        };
        gizmos.line(
            projected_position(a.position, projection.blend),
            projected_position(b.position, projection.blend),
            color,
        );
    }

    for pair in highlighted_path.systems.windows(2) {
        let Some(a) = galaxy.find_system(pair[0]) else {
            continue;
        };
        let Some(b) = galaxy.find_system(pair[1]) else {
            continue;
        };
        gizmos.line(
            projected_position(a.position, projection.blend) + Vec3::Y * 0.28,
            projected_position(b.position, projection.blend) + Vec3::Y * 0.28,
            Color::srgba(1.0, 0.92, 0.26, 0.92),
        );
    }
}

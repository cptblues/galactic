use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

use crate::data::{ActiveSystem, GalaxyData, PlanetId, SelectableId, Selection, ViewOptions};
use crate::views::PlanetVisual;

pub fn draw_system_orbits(
    active_system: Res<ActiveSystem>,
    galaxy: Res<GalaxyData>,
    options: Res<ViewOptions>,
    selection: Res<Selection>,
    planet_transforms: Query<(&PlanetVisual, &Transform)>,
    mut gizmos: Gizmos,
) {
    if !options.show_orbits {
        return;
    }
    let Some(system_id) = active_system.id else {
        return;
    };
    let Some(system) = galaxy.find_system(system_id) else {
        return;
    };

    let selected = selection.selected;
    let mut planet_positions = HashMap::<PlanetId, Vec3>::new();
    for (visual, transform) in &planet_transforms {
        planet_positions.insert(visual.id, transform.translation);
    }

    for planet in &system.planets {
        let planet_selected =
            matches!(selected, Some(SelectableId::Planet(_, id)) if id == planet.id);
        draw_orbit(
            &mut gizmos,
            Vec3::ZERO,
            planet.orbit_radius,
            planet.orbit_inclination,
            64,
            if planet_selected {
                Color::srgba(0.8, 1.0, 0.55, 0.72)
            } else {
                Color::srgba(0.55, 0.72, 0.95, 0.32)
            },
        );

        let Some(planet_position) = planet_positions.get(&planet.id).copied() else {
            continue;
        };
        for moon in &planet.moons {
            let moon_selected =
                matches!(selected, Some(SelectableId::Moon(_, _, id)) if id == moon.id);
            draw_orbit(
                &mut gizmos,
                planet_position,
                moon.orbit_radius,
                moon.orbit_inclination,
                32,
                if moon_selected {
                    Color::srgba(1.0, 0.96, 0.62, 0.68)
                } else {
                    Color::srgba(0.72, 0.76, 0.86, 0.25)
                },
            );
        }
    }
}

fn draw_orbit(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    inclination: f32,
    segments: usize,
    color: Color,
) {
    for index in 0..segments {
        let a = orbit_point(radius, inclination, index as f32 / segments as f32 * TAU) + center;
        let b = orbit_point(
            radius,
            inclination,
            (index + 1) as f32 / segments as f32 * TAU,
        ) + center;
        gizmos.line(a, b, color);
    }
}

fn orbit_point(radius: f32, inclination: f32, angle: f32) -> Vec3 {
    let local = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
    Quat::from_rotation_x(inclination) * local
}

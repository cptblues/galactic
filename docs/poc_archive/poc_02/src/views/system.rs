use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use std::f32::consts::TAU;

use crate::data::{ActiveSystem, GalaxyData, PlanetId, SelectableId};
use crate::interaction::{Selectable, selectable_click, selectable_out, selectable_over};
use crate::rendering::{BaseScale, VisualAssets, VisualMaterialSet};
use crate::views::{MoonVisual, OrbitMotion, PlanetVisual, StarVisual, SystemViewEntity};

pub fn spawn_system_view(
    mut commands: Commands,
    galaxy: Res<GalaxyData>,
    active_system: Res<ActiveSystem>,
    assets: Res<VisualAssets>,
    existing: Query<Entity, With<SystemViewEntity>>,
) {
    despawn_entities(&mut commands, &existing);

    let Some(system_id) = active_system.id else {
        warn!("cannot spawn system view without active system");
        return;
    };
    let Some(system) = galaxy.find_system(system_id) else {
        warn!("active system {:?} not found", system_id);
        return;
    };

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.28, 0.34, 0.46),
        brightness: 0.08,
        affects_lightmapped_meshes: true,
    });

    let star_scale = Vec3::splat(system.star.visual_radius * 1.9);
    let star_material = assets
        .star_materials
        .get(&system.star.class)
        .expect("star class material exists")
        .clone();
    commands
        .spawn((
            Mesh3d(assets.star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_scale(star_scale),
            StarVisual,
            Selectable {
                id: SelectableId::Star(system_id),
            },
            VisualMaterialSet::new(star_material, &assets),
            BaseScale(star_scale),
            SystemViewEntity,
        ))
        .observe(selectable_over)
        .observe(selectable_out)
        .observe(selectable_click);

    commands.spawn((
        PointLight {
            intensity: 4_800.0 * system.star.luminosity,
            range: 180.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        SystemViewEntity,
    ));

    for planet in &system.planets {
        let position = orbit_position(
            planet.orbit_radius,
            planet.orbit_phase,
            planet.orbit_inclination,
            Vec3::ZERO,
        );
        let scale = Vec3::splat(planet.visual_radius);
        let planet_material = assets
            .planet_materials
            .get(&planet.kind)
            .expect("planet kind material exists")
            .clone();
        commands
            .spawn((
                Mesh3d(assets.planet_mesh.clone()),
                MeshMaterial3d(planet_material.clone()),
                Transform::from_translation(position).with_scale(scale),
                PlanetVisual { id: planet.id },
                OrbitMotion {
                    radius: planet.orbit_radius,
                    speed: planet.orbit_speed,
                    phase: planet.orbit_phase,
                    inclination: planet.orbit_inclination,
                },
                Selectable {
                    id: SelectableId::Planet(system_id, planet.id),
                },
                VisualMaterialSet::new(planet_material, &assets),
                BaseScale(scale),
                SystemViewEntity,
            ))
            .observe(selectable_over)
            .observe(selectable_out)
            .observe(selectable_click);

        for moon in &planet.moons {
            let moon_position = orbit_position(
                moon.orbit_radius,
                moon.orbit_phase,
                moon.orbit_inclination,
                position,
            );
            let scale = Vec3::splat(moon.visual_radius);
            commands
                .spawn((
                    Mesh3d(assets.moon_mesh.clone()),
                    MeshMaterial3d(assets.moon_material.clone()),
                    Transform::from_translation(moon_position).with_scale(scale),
                    MoonVisual {
                        planet_id: planet.id,
                    },
                    OrbitMotion {
                        radius: moon.orbit_radius,
                        speed: moon.orbit_speed,
                        phase: moon.orbit_phase,
                        inclination: moon.orbit_inclination,
                    },
                    Selectable {
                        id: SelectableId::Moon(system_id, planet.id, moon.id),
                    },
                    VisualMaterialSet::new(assets.moon_material.clone(), &assets),
                    BaseScale(scale),
                    SystemViewEntity,
                ))
                .observe(selectable_over)
                .observe(selectable_out)
                .observe(selectable_click);
        }
    }

    if let Some(belt) = &system.asteroid_belt {
        spawn_asteroid_belt(&mut commands, &assets, galaxy.seed, system.id.0, belt);
    }

    info!(
        "spawned system view {} planets={} moons={}",
        system.name,
        system.planets.len(),
        system.moon_count()
    );
}

pub fn cleanup_system_view(mut commands: Commands, query: Query<Entity, With<SystemViewEntity>>) {
    despawn_entities(&mut commands, &query);
}

pub fn animate_system_bodies(
    animation: Res<crate::data::OrbitAnimation>,
    mut planets: Query<(&PlanetVisual, &OrbitMotion, &mut Transform), Without<MoonVisual>>,
    mut moons: Query<(&MoonVisual, &OrbitMotion, &mut Transform), Without<PlanetVisual>>,
) {
    let elapsed = animation.elapsed;
    let mut planet_positions = HashMap::<PlanetId, Vec3>::new();

    for (visual, motion, mut transform) in &mut planets {
        let angle = motion.phase + elapsed * motion.speed;
        transform.translation =
            orbit_position(motion.radius, angle, motion.inclination, Vec3::ZERO);
        planet_positions.insert(visual.id, transform.translation);
    }

    for (visual, motion, mut transform) in &mut moons {
        let Some(parent_position) = planet_positions.get(&visual.planet_id).copied() else {
            continue;
        };
        let angle = motion.phase + elapsed * motion.speed;
        transform.translation =
            orbit_position(motion.radius, angle, motion.inclination, parent_position);
    }
}

fn spawn_asteroid_belt(
    commands: &mut Commands,
    assets: &VisualAssets,
    seed: u64,
    system_index: u32,
    belt: &crate::data::AsteroidBeltData,
) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ (system_index as u64).wrapping_mul(0x9E37_79B9));
    for _ in 0..belt.asteroid_count {
        let angle = rng.random_range(0.0..TAU);
        let radius = rng.random_range(belt.inner_radius..belt.outer_radius);
        let height = rng.random_range(-0.35..0.35);
        let position = Vec3::new(radius * angle.cos(), height, radius * angle.sin());
        let scale = Vec3::splat(rng.random_range(0.12..0.36));
        commands.spawn((
            Mesh3d(assets.asteroid_mesh.clone()),
            MeshMaterial3d(assets.asteroid_material.clone()),
            Transform::from_translation(position)
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.random_range(0.0..TAU),
                    rng.random_range(0.0..TAU),
                    rng.random_range(0.0..TAU),
                ))
                .with_scale(scale),
            Pickable::IGNORE,
            SystemViewEntity,
        ));
    }
}

fn orbit_position(radius: f32, angle: f32, inclination: f32, center: Vec3) -> Vec3 {
    let local = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
    center + Quat::from_rotation_x(inclination) * local
}

fn despawn_entities(commands: &mut Commands, query: &Query<Entity, With<SystemViewEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

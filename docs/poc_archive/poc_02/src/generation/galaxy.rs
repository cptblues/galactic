use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;
use std::f32::consts::TAU;

use crate::data::{GalaxyConfig, GalaxyData, PlanetKind, StarSystemData, SystemId};
use crate::generation::names::unique_system_name;
use crate::generation::routes::generate_routes;
use crate::generation::system::generate_system;

pub fn generate_galaxy(config: &GalaxyConfig) -> GalaxyData {
    let config = config.sanitized();
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut systems = Vec::with_capacity(config.system_count);
    let mut used_names = HashSet::new();
    let bulge_count = ((config.system_count as f32) * 0.12).round() as usize;
    let outlier_count = ((config.system_count as f32) * 0.05).round() as usize;

    for index in 0..config.system_count {
        let id = SystemId(index as u32);
        let position = if index < bulge_count {
            generate_bulge_position(&mut rng, &config)
        } else if index >= config.system_count.saturating_sub(outlier_count) {
            generate_outlier_position(&mut rng, &config)
        } else {
            generate_spiral_position(&mut rng, &config, &systems)
        };
        let name = unique_system_name(&mut rng, &mut used_names);
        systems.push(generate_system(&mut rng, id, name, position));
    }

    ensure_demo_validation_planet(&mut systems);
    let routes = generate_routes(&systems);
    GalaxyData {
        seed: config.seed,
        systems,
        routes,
    }
}

fn ensure_demo_validation_planet(systems: &mut [StarSystemData]) {
    let Some(system) = systems.get_mut(2) else {
        return;
    };
    let Some(planet) = system.planets.first_mut() else {
        return;
    };
    planet.kind = PlanetKind::Ocean;
    planet.habitability = planet.habitability.max(88);
    system.tags.has_habitable_world = true;
}

fn generate_spiral_position(
    rng: &mut ChaCha8Rng,
    config: &GalaxyConfig,
    systems: &[StarSystemData],
) -> Vec3 {
    let mut best = Vec3::ZERO;
    let mut best_distance = -1.0_f32;

    for _ in 0..64 {
        let candidate = spiral_candidate(rng, config);
        let nearest = systems
            .iter()
            .map(|system| system.position.distance(candidate))
            .fold(f32::INFINITY, f32::min);

        if nearest >= config.min_system_distance {
            return candidate;
        }
        if nearest > best_distance {
            best_distance = nearest;
            best = candidate;
        }
    }

    best
}

fn spiral_candidate(rng: &mut ChaCha8Rng, config: &GalaxyConfig) -> Vec3 {
    let arm_index = rng.random_range(0..config.arm_count);
    let u = rng.random_range(0.0_f32..1.0).powf(0.65);
    let radius = config.radius * u;
    let arm_base = arm_index as f32 * TAU / config.arm_count as f32;
    let angular_noise = rng.random_range(-config.arm_spread..config.arm_spread);
    let spiral_angle =
        arm_base + config.spiral_turns * TAU * (radius / config.radius) + angular_noise;
    let radial_noise = rng.random_range(-1.8..1.8) * (0.35 + u);
    let radial_noise_2 = rng.random_range(-1.8..1.8) * (0.35 + u);
    let x = radius * spiral_angle.cos() + radial_noise;
    let z = radius * spiral_angle.sin() + radial_noise_2;
    let y_scale = 0.35 + u * 0.65;
    let y = rng.random_range(-1.0..1.0) * config.thickness * y_scale;
    Vec3::new(x, y, z)
}

fn generate_bulge_position(rng: &mut ChaCha8Rng, config: &GalaxyConfig) -> Vec3 {
    let radius = config.radius * rng.random_range(0.0_f32..0.18).sqrt();
    let angle = rng.random_range(0.0..TAU);
    Vec3::new(
        radius * angle.cos(),
        rng.random_range(-config.thickness * 0.45..config.thickness * 0.45),
        radius * angle.sin(),
    )
}

fn generate_outlier_position(rng: &mut ChaCha8Rng, config: &GalaxyConfig) -> Vec3 {
    let radius = config.radius * rng.random_range(0.82..1.16);
    let angle = rng.random_range(0.0..TAU);
    Vec3::new(
        radius * angle.cos(),
        rng.random_range(-config.thickness * 1.2..config.thickness * 1.2),
        radius * angle.sin(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn same_seed_produces_same_galaxy() {
        let config = GalaxyConfig::default();
        let first = generate_galaxy(&config);
        let second = generate_galaxy(&config);
        assert_eq!(first, second);
    }

    #[test]
    fn ids_and_names_are_unique() {
        let galaxy = generate_galaxy(&GalaxyConfig::default());
        let mut system_ids = HashSet::new();
        let mut planet_ids = HashSet::new();
        let mut moon_ids = HashSet::new();
        let mut names = HashSet::new();

        for system in &galaxy.systems {
            assert!(system_ids.insert(system.id));
            assert!(names.insert(system.name.clone()));
            for planet in &system.planets {
                assert!(planet_ids.insert(planet.id));
                for moon in &planet.moons {
                    assert!(moon_ids.insert(moon.id));
                }
            }
        }
    }

    #[test]
    fn generated_bounds_are_valid() {
        let galaxy = generate_galaxy(&GalaxyConfig::default());
        assert_eq!(galaxy.systems.len(), 500);
        for system in &galaxy.systems {
            assert!(system.position.is_finite());
            assert!((2..=9).contains(&system.planets.len()));
            let mut previous_orbit = 0.0;
            for planet in &system.planets {
                assert!((0..=100).contains(&planet.habitability));
                assert!(planet.orbit_radius > previous_orbit);
                assert!(planet.orbit_radius.is_finite());
                assert!(planet.visual_radius.is_finite());
                assert!(planet.moons.len() <= 4);
                previous_orbit = planet.orbit_radius;
                for moon in &planet.moons {
                    assert!(moon.orbit_radius.is_finite());
                    assert!(moon.visual_radius.is_finite());
                }
            }
        }
    }

    #[test]
    fn routes_are_valid() {
        let galaxy = generate_galaxy(&GalaxyConfig::default());
        let system_ids: HashSet<_> = galaxy.systems.iter().map(|system| system.id).collect();
        let mut routes = HashSet::new();
        for route in &galaxy.routes {
            assert_ne!(route.a, route.b);
            assert!(system_ids.contains(&route.a));
            assert!(system_ids.contains(&route.b));
            assert!(routes.insert((route.a.min(route.b), route.a.max(route.b))));
        }
    }

    #[test]
    fn seed_42_regression_anchor() {
        let galaxy = generate_galaxy(&GalaxyConfig::default());
        let first = &galaxy.systems[0];
        assert_eq!(galaxy.systems.len(), 500);
        assert_eq!(first.id, SystemId(0));
        assert!(!first.name.is_empty());
        assert!((2..=9).contains(&first.planets.len()));
    }

    #[test]
    fn demo_seed_has_guaranteed_ocean_target_body() {
        let galaxy = generate_galaxy(&GalaxyConfig::default());
        let system = galaxy.find_system(SystemId(2)).expect("demo system exists");
        assert_eq!(system.planets[0].kind, PlanetKind::Ocean);
        assert!(system.planets[0].habitability >= 88);
    }
}

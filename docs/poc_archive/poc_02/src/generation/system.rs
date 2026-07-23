use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::data::{
    AsteroidBeltData, MoonData, MoonId, PlanetData, PlanetId, PlanetKind, StarClass, StarData,
    StarSystemData, SystemId, SystemTags,
};
use crate::generation::names::roman;

pub fn generate_system(
    rng: &mut ChaCha8Rng,
    id: SystemId,
    name: String,
    position: bevy::prelude::Vec3,
) -> StarSystemData {
    let star = generate_star(rng);
    let planet_count = rng.random_range(2..=9);
    let mut planets = Vec::with_capacity(planet_count);
    let mut orbit = 7.0 + star.visual_radius;

    for planet_index in 0..planet_count {
        orbit += rng.random_range(4.0..9.0) + planet_index as f32 * 0.45;
        let normalized_distance = (planet_index as f32 / planet_count as f32).clamp(0.0, 1.0);
        let kind = generate_planet_kind(rng, star.class, normalized_distance);
        let visual_radius = planet_radius(rng, kind);
        let habitability = habitability(rng, kind, star.class, normalized_distance);
        let moon_count = moon_count(rng, kind);
        let planet_id = PlanetId(id.0 * 16 + planet_index as u32);
        let planet_name = format!("{name} {}", roman(planet_index));

        let mut moons = Vec::with_capacity(moon_count);
        let mut moon_orbit = visual_radius + 1.2;
        for moon_index in 0..moon_count {
            moon_orbit += rng.random_range(0.7..1.8);
            moons.push(MoonData {
                id: MoonId(id.0 * 128 + planet_index as u32 * 8 + moon_index as u32),
                name: format!("{planet_name}-{}", (b'a' + moon_index as u8) as char),
                visual_radius: rng.random_range(0.12..0.34),
                orbit_radius: moon_orbit,
                orbit_speed: rng.random_range(0.22..0.62),
                orbit_phase: rng.random_range(0.0..std::f32::consts::TAU),
                orbit_inclination: rng.random_range(-0.22..0.22),
            });
        }

        planets.push(PlanetData {
            id: planet_id,
            name: planet_name,
            kind,
            visual_radius,
            orbit_radius: orbit,
            orbit_speed: rng.random_range(0.025..0.085) / (1.0 + planet_index as f32 * 0.24),
            orbit_phase: rng.random_range(0.0..std::f32::consts::TAU),
            orbit_inclination: rng.random_range(-0.28..0.28),
            habitability,
            moons,
        });
    }

    let asteroid_belt = if rng.random_bool(0.38) && planets.len() > 3 {
        let belt_index = rng.random_range(2..planets.len());
        let center = planets[belt_index].orbit_radius + rng.random_range(2.2..5.5);
        Some(AsteroidBeltData {
            inner_radius: center,
            outer_radius: center + rng.random_range(2.0..5.5),
            asteroid_count: rng.random_range(100..=300),
        })
    } else {
        None
    };

    let tags = SystemTags {
        has_habitable_world: planets.iter().any(|planet| planet.habitability >= 62),
        mineral_rich: rng.random_bool(0.22),
        anomaly_detected: rng.random_bool(0.08),
    };

    StarSystemData {
        id,
        name,
        position,
        star,
        planets,
        asteroid_belt,
        tags,
    }
}

fn generate_star(rng: &mut ChaCha8Rng) -> StarData {
    let roll = rng.random_range(0.0..1.0);
    let class = if roll < 0.08 {
        StarClass::Blue
    } else if roll < 0.22 {
        StarClass::White
    } else if roll < 0.48 {
        StarClass::Yellow
    } else if roll < 0.68 {
        StarClass::Orange
    } else {
        StarClass::Red
    };

    match class {
        StarClass::Blue => StarData {
            class,
            visual_radius: rng.random_range(1.7..2.25),
            luminosity: rng.random_range(2.2..3.4),
            temperature_kelvin: rng.random_range(11_000..22_000),
        },
        StarClass::White => StarData {
            class,
            visual_radius: rng.random_range(1.25..1.65),
            luminosity: rng.random_range(1.5..2.2),
            temperature_kelvin: rng.random_range(7_500..10_500),
        },
        StarClass::Yellow => StarData {
            class,
            visual_radius: rng.random_range(1.0..1.35),
            luminosity: rng.random_range(0.9..1.5),
            temperature_kelvin: rng.random_range(5_200..6_500),
        },
        StarClass::Orange => StarData {
            class,
            visual_radius: rng.random_range(0.85..1.2),
            luminosity: rng.random_range(0.55..1.0),
            temperature_kelvin: rng.random_range(3_900..5_200),
        },
        StarClass::Red => StarData {
            class,
            visual_radius: rng.random_range(0.65..1.05),
            luminosity: rng.random_range(0.25..0.7),
            temperature_kelvin: rng.random_range(2_600..3_900),
        },
    }
}

fn generate_planet_kind(
    rng: &mut ChaCha8Rng,
    star_class: StarClass,
    normalized_distance: f32,
) -> PlanetKind {
    let roll = rng.random_range(0.0..1.0);
    if normalized_distance < 0.18 {
        if roll < 0.42 {
            PlanetKind::Volcanic
        } else if roll < 0.74 {
            PlanetKind::Rocky
        } else {
            PlanetKind::Desert
        }
    } else if normalized_distance > 0.76 {
        if roll < 0.45 {
            PlanetKind::Ice
        } else if roll < 0.82 {
            PlanetKind::GasGiant
        } else {
            PlanetKind::Rocky
        }
    } else {
        match star_class {
            StarClass::Blue if roll < 0.32 => PlanetKind::Desert,
            StarClass::Red if roll < 0.34 => PlanetKind::Ice,
            _ if roll < 0.24 => PlanetKind::Ocean,
            _ if roll < 0.46 => PlanetKind::Rocky,
            _ if roll < 0.64 => PlanetKind::Desert,
            _ if roll < 0.82 => PlanetKind::GasGiant,
            _ => PlanetKind::Ice,
        }
    }
}

fn planet_radius(rng: &mut ChaCha8Rng, kind: PlanetKind) -> f32 {
    match kind {
        PlanetKind::GasGiant => rng.random_range(1.25..2.2),
        PlanetKind::Ocean => rng.random_range(0.62..1.08),
        PlanetKind::Rocky => rng.random_range(0.45..0.88),
        PlanetKind::Desert => rng.random_range(0.5..0.95),
        PlanetKind::Ice => rng.random_range(0.42..0.9),
        PlanetKind::Volcanic => rng.random_range(0.48..0.92),
    }
}

fn moon_count(rng: &mut ChaCha8Rng, kind: PlanetKind) -> usize {
    match kind {
        PlanetKind::GasGiant => rng.random_range(1..=4),
        PlanetKind::Ocean | PlanetKind::Rocky | PlanetKind::Desert => rng.random_range(0..=2),
        PlanetKind::Ice | PlanetKind::Volcanic => rng.random_range(0..=1),
    }
}

fn habitability(
    rng: &mut ChaCha8Rng,
    kind: PlanetKind,
    star_class: StarClass,
    normalized_distance: f32,
) -> u8 {
    let base = match kind {
        PlanetKind::Ocean => 62,
        PlanetKind::Rocky => 42,
        PlanetKind::Desert => 22,
        PlanetKind::Ice => 16,
        PlanetKind::Volcanic => 4,
        PlanetKind::GasGiant => 0,
    };
    let distance_bonus = (1.0 - (normalized_distance - 0.48).abs() * 2.4).max(0.0) * 26.0;
    let star_penalty = match star_class {
        StarClass::Blue => 18,
        StarClass::White => 6,
        StarClass::Yellow => 0,
        StarClass::Orange => 2,
        StarClass::Red => 10,
    };
    (base + distance_bonus as i32 - star_penalty + rng.random_range(0..=12)).clamp(0, 100) as u8
}

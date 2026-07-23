use std::collections::BTreeSet;
use std::f32::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{PlanetId, SystemId, WorldPosition};

const DEFAULT_SEED: u64 = 42;
const DEFAULT_SYSTEM_COUNT: usize = 16;
const MAX_SYSTEM_COUNT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniverseConfig {
    pub seed: u64,
    pub system_count: usize,
}

impl UniverseConfig {
    pub const fn new(seed: u64, system_count: usize) -> Self {
        Self { seed, system_count }
    }

    pub fn sanitized(self) -> Self {
        Self {
            seed: self.seed,
            system_count: self.system_count.clamp(1, MAX_SYSTEM_COUNT),
        }
    }
}

impl Default for UniverseConfig {
    fn default() -> Self {
        Self::new(DEFAULT_SEED, DEFAULT_SYSTEM_COUNT)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UniverseDefinition {
    pub seed: u64,
    pub systems: Vec<StarSystem>,
    pub routes: Vec<Route>,
}

impl UniverseDefinition {
    pub fn system(&self, id: SystemId) -> Option<&StarSystem> {
        self.systems.iter().find(|system| system.id == id)
    }

    pub fn neighboring_systems(&self, id: SystemId) -> Vec<SystemId> {
        self.routes
            .iter()
            .filter_map(|route| route.other(id))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StarSystem {
    pub id: SystemId,
    pub name: String,
    pub position: WorldPosition,
    pub star: Star,
    pub planets: Vec<Planet>,
}

impl StarSystem {
    pub fn planet(&self, id: PlanetId) -> Option<&Planet> {
        self.planets.iter().find(|planet| planet.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    pub class: StarClass,
    pub luminosity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StarClass {
    Blue,
    White,
    Yellow,
    Orange,
    Red,
}

impl StarClass {
    pub const ALL: [Self; 5] = [
        Self::Blue,
        Self::White,
        Self::Yellow,
        Self::Orange,
        Self::Red,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub struct Planet {
    pub id: PlanetId,
    pub name: String,
    pub kind: PlanetKind,
    pub habitability: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanetKind {
    Rocky,
    Ocean,
    Desert,
    Ice,
    GasGiant,
    Volcanic,
}

impl PlanetKind {
    pub const ALL: [Self; 6] = [
        Self::Rocky,
        Self::Ocean,
        Self::Desert,
        Self::Ice,
        Self::GasGiant,
        Self::Volcanic,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Route {
    pub from: SystemId,
    pub to: SystemId,
}

impl Route {
    pub fn new(a: SystemId, b: SystemId) -> Self {
        if a <= b {
            Self { from: a, to: b }
        } else {
            Self { from: b, to: a }
        }
    }

    pub fn other(self, id: SystemId) -> Option<SystemId> {
        if self.from == id {
            Some(self.to)
        } else if self.to == id {
            Some(self.from)
        } else {
            None
        }
    }
}

pub fn generate_universe(config: UniverseConfig) -> UniverseDefinition {
    let config = config.sanitized();
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let systems = (0..config.system_count)
        .map(|index| generate_system(index, &mut rng))
        .collect::<Vec<_>>();
    let routes = generate_routes(&systems);

    UniverseDefinition {
        seed: config.seed,
        systems,
        routes,
    }
}

fn generate_system(index: usize, rng: &mut ChaCha8Rng) -> StarSystem {
    let id = SystemId::new(index as u32);
    let is_home = index == 0;
    let position = if is_home {
        WorldPosition::ZERO
    } else {
        spiral_position(index, rng)
    };
    let star = if is_home {
        Star {
            class: StarClass::Yellow,
            luminosity: 1.0,
        }
    } else {
        random_star(rng)
    };

    StarSystem {
        id,
        name: system_name(index, rng),
        position,
        star,
        planets: generate_planets(index, rng),
    }
}

fn spiral_position(index: usize, rng: &mut ChaCha8Rng) -> WorldPosition {
    let arm = index % 4;
    let arm_angle = arm as f32 * TAU / 4.0;
    let radial_step = 9.0 + index as f32 * 2.8;
    let angle = arm_angle + radial_step * 0.045 + rng.random_range(-0.42..0.42);
    let radius = radial_step + rng.random_range(-3.5..3.5);

    WorldPosition::new(
        angle.cos() * radius,
        rng.random_range(-2.2..2.2),
        angle.sin() * radius,
    )
}

fn random_star(rng: &mut ChaCha8Rng) -> Star {
    let roll = rng.random_range(0.0..1.0);
    let class = if roll < 0.08 {
        StarClass::Blue
    } else if roll < 0.2 {
        StarClass::White
    } else if roll < 0.55 {
        StarClass::Yellow
    } else if roll < 0.82 {
        StarClass::Orange
    } else {
        StarClass::Red
    };
    let luminosity = match class {
        StarClass::Blue => rng.random_range(2.6..4.8),
        StarClass::White => rng.random_range(1.6..2.4),
        StarClass::Yellow => rng.random_range(0.8..1.5),
        StarClass::Orange => rng.random_range(0.45..0.9),
        StarClass::Red => rng.random_range(0.18..0.55),
    };

    Star { class, luminosity }
}

fn generate_planets(system_index: usize, rng: &mut ChaCha8Rng) -> Vec<Planet> {
    let count = if system_index == 0 {
        3
    } else {
        rng.random_range(1..=5)
    };

    (0..count)
        .map(|index| {
            let id = PlanetId::new(index as u32);
            if system_index == 0 && index == 0 {
                return Planet {
                    id,
                    name: "Aster Prime".to_string(),
                    kind: PlanetKind::Ocean,
                    habitability: 92,
                };
            }

            let kind = random_planet_kind(rng);
            Planet {
                id,
                name: planet_name(system_index, index),
                kind,
                habitability: habitability_for(kind, rng),
            }
        })
        .collect()
}

fn random_planet_kind(rng: &mut ChaCha8Rng) -> PlanetKind {
    match rng.random_range(0..6) {
        0 => PlanetKind::Rocky,
        1 => PlanetKind::Ocean,
        2 => PlanetKind::Desert,
        3 => PlanetKind::Ice,
        4 => PlanetKind::GasGiant,
        _ => PlanetKind::Volcanic,
    }
}

fn habitability_for(kind: PlanetKind, rng: &mut ChaCha8Rng) -> u8 {
    let range = match kind {
        PlanetKind::Ocean => 55..=96,
        PlanetKind::Rocky => 25..=82,
        PlanetKind::Desert => 12..=62,
        PlanetKind::Ice => 8..=52,
        PlanetKind::Volcanic => 0..=38,
        PlanetKind::GasGiant => 0..=8,
    };
    rng.random_range(range)
}

fn generate_routes(systems: &[StarSystem]) -> Vec<Route> {
    let mut unique = BTreeSet::new();

    for system in systems {
        let mut neighbors = systems
            .iter()
            .filter(|candidate| candidate.id != system.id)
            .map(|candidate| {
                (
                    system.position.distance_squared(candidate.position),
                    candidate.id,
                )
            })
            .collect::<Vec<_>>();
        neighbors.sort_by(|a, b| a.0.total_cmp(&b.0));

        for (_, neighbor_id) in neighbors.into_iter().take(2) {
            let route = Route::new(system.id, neighbor_id);
            unique.insert((route.from.index(), route.to.index()));
        }
    }

    unique
        .into_iter()
        .map(|(from, to)| Route::new(SystemId::new(from), SystemId::new(to)))
        .collect()
}

fn system_name(index: usize, rng: &mut ChaCha8Rng) -> String {
    const PREFIXES: &[&str] = &[
        "Aster", "Nova", "Kepler", "Vega", "Orion", "Lyra", "Cygni", "Helio",
    ];
    const SUFFIXES: &[&str] = &[
        "Reach", "Gate", "Hold", "Bastion", "Drift", "Crown", "Harbor", "Span",
    ];

    if index == 0 {
        "Aster".to_string()
    } else {
        format!(
            "{} {}",
            PREFIXES[rng.random_range(0..PREFIXES.len())],
            SUFFIXES[rng.random_range(0..SUFFIXES.len())]
        )
    }
}

fn planet_name(system_index: usize, planet_index: usize) -> String {
    format!("P{}-{}", system_index + 1, planet_index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_for_same_seed() {
        let config = UniverseConfig::new(7, 16);

        assert_eq!(generate_universe(config), generate_universe(config));
    }

    #[test]
    fn default_world_matches_mvp_scope() {
        let universe = generate_universe(UniverseConfig::default());

        assert!((12..=20).contains(&universe.systems.len()));
        assert!(!universe.routes.is_empty());
    }

    #[test]
    fn home_system_has_habitable_planet() {
        let universe = generate_universe(UniverseConfig::default());
        let home = universe
            .system(SystemId::new(0))
            .expect("home system exists");
        let planet = home.planet(PlanetId::new(0)).expect("home planet exists");

        assert_eq!(planet.kind, PlanetKind::Ocean);
        assert!(planet.habitability >= 90);
    }

    #[test]
    fn routes_reference_existing_systems() {
        let universe = generate_universe(UniverseConfig::new(11, 18));

        for route in &universe.routes {
            assert!(universe.system(route.from).is_some());
            assert!(universe.system(route.to).is_some());
            assert_ne!(route.from, route.to);
        }
    }
}

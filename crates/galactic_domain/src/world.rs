use std::collections::BTreeSet;
use std::f32::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{PlanetId, StarId, SystemId, UniverseId, WorldPosition};

pub const MVP_UNIVERSE_SEED: u64 = 42;
pub const MVP_SYSTEM_COUNT: usize = 16;
pub const GENERATION_VERSION: u32 = 2;
pub const MVP_REFERENCE_FINGERPRINT: u64 = 12539308657388844103;

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

    pub const fn mvp() -> Self {
        Self::new(MVP_UNIVERSE_SEED, MVP_SYSTEM_COUNT)
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
        Self::mvp()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UniverseDefinition {
    pub id: UniverseId,
    pub seed: u64,
    pub generation_version: u32,
    pub generation_fingerprint: u64,
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
    pub id: StarId,
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

    const fn fingerprint_tag(self) -> u64 {
        match self {
            Self::Blue => 1,
            Self::White => 2,
            Self::Yellow => 3,
            Self::Orange => 4,
            Self::Red => 5,
        }
    }
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

    const fn fingerprint_tag(self) -> u64 {
        match self {
            Self::Rocky => 1,
            Self::Ocean => 2,
            Self::Desert => 3,
            Self::Ice => 4,
            Self::GasGiant => 5,
            Self::Volcanic => 6,
        }
    }
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

    let mut universe = UniverseDefinition {
        id: UniverseId::MVP,
        seed: config.seed,
        generation_version: GENERATION_VERSION,
        generation_fingerprint: 0,
        systems,
        routes,
    };
    universe.generation_fingerprint = fingerprint_universe(&universe);
    universe
}

fn generate_system(index: usize, rng: &mut ChaCha8Rng) -> StarSystem {
    let id = SystemId::from_index(index as u32);
    let is_home = index == 0;
    let position = if is_home {
        WorldPosition::ZERO
    } else {
        spiral_position(index, rng)
    };
    let star = if is_home {
        Star {
            id: StarId::for_system(id),
            class: StarClass::Yellow,
            luminosity: 1.0,
        }
    } else {
        random_star(id, rng)
    };

    StarSystem {
        id,
        name: system_name(index, rng),
        position,
        star,
        planets: generate_planets(id, index, rng),
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

fn random_star(system_id: SystemId, rng: &mut ChaCha8Rng) -> Star {
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

    Star {
        id: StarId::for_system(system_id),
        class,
        luminosity,
    }
}

fn generate_planets(system_id: SystemId, system_index: usize, rng: &mut ChaCha8Rng) -> Vec<Planet> {
    let count = if system_index == 0 {
        3
    } else {
        rng.random_range(1..=5)
    };

    (0..count)
        .map(|index| {
            let id = PlanetId::from_system_index(system_id, index as u32);
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

// MVP-006: guarantee connectivity with a deterministic minimum spanning tree,
// then add local nearest-neighbor links to keep the map tactically interesting.
fn generate_routes(systems: &[StarSystem]) -> Vec<Route> {
    if systems.len() <= 1 {
        return Vec::new();
    }

    let mut unique = BTreeSet::new();
    let mut connected = vec![false; systems.len()];
    connected[0] = true;

    // Prim's algorithm over geometric distances. System IDs break equal-distance
    // ties so the same seed always yields the same route graph.
    for _ in 1..systems.len() {
        let mut best: Option<(f32, SystemId, SystemId, usize)> = None;

        for (from_index, from) in systems.iter().enumerate() {
            if !connected[from_index] {
                continue;
            }

            for (to_index, to) in systems.iter().enumerate() {
                if connected[to_index] {
                    continue;
                }

                let distance = from.position.distance_squared(to.position);
                let replace = match best {
                    None => true,
                    Some((best_distance, best_from, best_to, _)) => distance
                        .total_cmp(&best_distance)
                        .then_with(|| from.id.cmp(&best_from))
                        .then_with(|| to.id.cmp(&best_to))
                        .is_lt(),
                };

                if replace {
                    best = Some((distance, from.id, to.id, to_index));
                }
            }
        }

        let (_, from, to, to_index) =
            best.expect("a disconnected vertex must have an edge to the connected set");
        let route = Route::new(from, to);
        unique.insert((route.from.raw(), route.to.raw()));
        connected[to_index] = true;
    }

    // Add each system's two nearest neighbors. The BTreeSet preserves canonical
    // ordering and removes edges already provided by the spanning tree.
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
        neighbors.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        for (_, neighbor_id) in neighbors.into_iter().take(2) {
            let route = Route::new(system.id, neighbor_id);
            unique.insert((route.from.raw(), route.to.raw()));
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

pub fn fingerprint_universe(universe: &UniverseDefinition) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    mix_u64(&mut hash, universe.id.raw());
    mix_u64(&mut hash, universe.seed);
    mix_u64(&mut hash, universe.generation_version as u64);
    mix_u64(&mut hash, universe.systems.len() as u64);
    mix_u64(&mut hash, universe.routes.len() as u64);

    for system in &universe.systems {
        mix_u64(&mut hash, system.id.raw());
        mix_bytes(&mut hash, system.name.as_bytes());
        mix_u64(&mut hash, system.position.x.to_bits() as u64);
        mix_u64(&mut hash, system.position.y.to_bits() as u64);
        mix_u64(&mut hash, system.position.z.to_bits() as u64);
        mix_u64(&mut hash, system.star.id.raw());
        mix_u64(&mut hash, system.star.class.fingerprint_tag());
        mix_u64(&mut hash, system.star.luminosity.to_bits() as u64);
        mix_u64(&mut hash, system.planets.len() as u64);

        for planet in &system.planets {
            mix_u64(&mut hash, planet.id.raw());
            mix_bytes(&mut hash, planet.name.as_bytes());
            mix_u64(&mut hash, planet.kind.fingerprint_tag());
            mix_u64(&mut hash, planet.habitability as u64);
        }
    }

    for route in &universe.routes {
        mix_u64(&mut hash, route.from.raw());
        mix_u64(&mut hash, route.to.raw());
    }

    hash
}

fn mix_u64(hash: &mut u64, value: u64) {
    mix_bytes(hash, &value.to_le_bytes());
}

fn mix_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(0x100000001b3);
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
        assert_eq!(universe.seed, MVP_UNIVERSE_SEED);
        assert_eq!(universe.generation_version, GENERATION_VERSION);
        assert!((12..=20).contains(&universe.systems.len()));
        assert!(!universe.routes.is_empty());
        assert_ne!(universe.generation_fingerprint, 0);
    }

    #[test]
    fn home_system_has_habitable_planet() {
        let universe = generate_universe(UniverseConfig::default());
        let home_system_id = SystemId::from_index(0);
        let home = universe.system(home_system_id).expect("home system exists");
        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);
        let planet = home.planet(home_planet_id).expect("home planet exists");

        assert_eq!(home.star.id, StarId::for_system(home_system_id));
        assert_eq!(planet.kind, PlanetKind::Ocean);
        assert!(planet.habitability >= 90);
    }

    #[test]
    fn planet_ids_are_unique_across_systems() {
        let universe = generate_universe(UniverseConfig::default());
        let ids = universe
            .systems
            .iter()
            .flat_map(|system| system.planets.iter().map(|planet| planet.id))
            .collect::<BTreeSet<_>>();
        let planet_count = universe
            .systems
            .iter()
            .map(|system| system.planets.len())
            .sum::<usize>();
        assert_eq!(ids.len(), planet_count);
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

    #[test]
    fn route_graph_is_connected_from_home() {
        let universe = generate_universe(UniverseConfig::mvp());
        let mut visited = BTreeSet::new();
        let mut frontier = vec![SystemId::from_index(0)];

        while let Some(system_id) = frontier.pop() {
            if !visited.insert(system_id) {
                continue;
            }
            frontier.extend(
                universe
                    .neighboring_systems(system_id)
                    .into_iter()
                    .filter(|neighbor| !visited.contains(neighbor)),
            );
        }

        assert_eq!(visited.len(), universe.systems.len());
    }

    #[test]
    fn routes_are_unique_canonical_and_deterministic() {
        let first = generate_universe(UniverseConfig::mvp());
        let second = generate_universe(UniverseConfig::mvp());
        let mut unique = BTreeSet::new();

        assert_eq!(first.routes, second.routes);
        for route in &first.routes {
            assert!(route.from < route.to);
            assert!(unique.insert((route.from, route.to)));
        }
    }

    #[test]
    fn reference_seed_fingerprint_is_stable() {
        assert_ne!(
            MVP_REFERENCE_FINGERPRINT, 0,
            "run tools/apply_mvp_003.py once to bootstrap the reference fingerprint"
        );
        let universe = generate_universe(UniverseConfig::mvp());
        assert_eq!(
            universe.generation_fingerprint, MVP_REFERENCE_FINGERPRINT,
            "the generated MVP universe changed; increment GENERATION_VERSION only if intentional"
        );
    }

    #[test]
    #[ignore = "used by tools/apply_mvp_003.py to bootstrap the snapshot"]
    fn print_reference_seed_fingerprint() {
        let universe = generate_universe(UniverseConfig::mvp());
        println!("MVP_FINGERPRINT={}", universe.generation_fingerprint);
    }
}

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f32::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{PlanetId, SectorId, StarId, SystemId, UniverseId, WorldPosition};

pub const MVP_UNIVERSE_SEED: u64 = 42;
pub const TEST_SYSTEM_COUNT: usize = 16;
pub const MVP_SYSTEM_COUNT: usize = 64;
pub const STRESS_SYSTEM_COUNT: usize = 128;
pub const GENERATION_VERSION: u32 = 5;
pub const TEST_REFERENCE_FINGERPRINT: u64 = 7965321313134283584;

const MAX_SYSTEM_COUNT: usize = 256;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UniverseScalePreset {
    Test,
    #[default]
    Mvp,
    Stress,
}

impl UniverseScalePreset {
    pub const ALL: [Self; 3] = [Self::Test, Self::Mvp, Self::Stress];

    pub const fn system_count(self) -> usize {
        match self {
            Self::Test => TEST_SYSTEM_COUNT,
            Self::Mvp => MVP_SYSTEM_COUNT,
            Self::Stress => STRESS_SYSTEM_COUNT,
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Mvp => "mvp",
            Self::Stress => "stress",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Test => "Test",
            Self::Mvp => "MVP",
            Self::Stress => "Stress",
        }
    }

    pub fn from_slug(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "test" | "16" => Some(Self::Test),
            "mvp" | "64" => Some(Self::Mvp),
            "stress" | "128" => Some(Self::Stress),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniverseConfig {
    pub seed: u64,
    pub system_count: usize,
}

impl UniverseConfig {
    pub const fn new(seed: u64, system_count: usize) -> Self {
        Self { seed, system_count }
    }

    pub const fn for_preset(seed: u64, preset: UniverseScalePreset) -> Self {
        Self::new(seed, preset.system_count())
    }

    pub const fn test() -> Self {
        Self::for_preset(MVP_UNIVERSE_SEED, UniverseScalePreset::Test)
    }

    pub const fn mvp() -> Self {
        Self::for_preset(MVP_UNIVERSE_SEED, UniverseScalePreset::Mvp)
    }

    pub const fn stress() -> Self {
        Self::for_preset(MVP_UNIVERSE_SEED, UniverseScalePreset::Stress)
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
    pub sectors: Vec<SectorDefinition>,
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

    pub fn sector(&self, id: SectorId) -> Option<&SectorDefinition> {
        self.sectors.iter().find(|sector| sector.id == id)
    }

    pub fn sector_for_system(&self, id: SystemId) -> Option<&SectorDefinition> {
        self.sectors
            .iter()
            .find(|sector| sector.systems.binary_search(&id).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectorDefinition {
    pub id: SectorId,
    pub name: String,
    pub center: WorldPosition,
    pub systems: Vec<SystemId>,
    pub gateway_routes: Vec<Route>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    let sectors = generate_sectors(config.seed, &systems, &routes);

    let mut universe = UniverseDefinition {
        id: UniverseId::MVP,
        seed: config.seed,
        generation_version: GENERATION_VERSION,
        generation_fingerprint: 0,
        systems,
        routes,
        sectors,
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

    let name = system_name(index, rng);
    let planets = generate_planets(id, index, &name, rng);

    StarSystem {
        id,
        name,
        position,
        star,
        planets,
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

fn generate_planets(
    system_id: SystemId,
    system_index: usize,
    system_name: &str,
    rng: &mut ChaCha8Rng,
) -> Vec<Planet> {
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
                    name: "Nacre".to_string(),
                    kind: PlanetKind::Ocean,
                    habitability: 92,
                };
            }

            let kind = random_planet_kind(rng);
            Planet {
                id,
                name: planet_name(system_name, index),
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

fn generate_sectors(seed: u64, systems: &[StarSystem], routes: &[Route]) -> Vec<SectorDefinition> {
    if systems.is_empty() {
        return Vec::new();
    }

    let sector_count = sector_count_for(systems.len());
    let centers = select_sector_centers(seed, systems, sector_count);
    let adjacency = sector_adjacency(systems, routes);
    let mut assignments = BTreeMap::<SystemId, usize>::new();
    let mut frontier = VecDeque::<(SystemId, usize)>::new();

    for (sector_index, center) in centers.iter().copied().enumerate() {
        assignments.insert(center, sector_index);
        frontier.push_back((center, sector_index));
    }

    // Multi-source BFS produces graph-connected sectors. Center order and
    // sorted adjacency provide stable tie-breaking at equal hop distance.
    while let Some((system_id, sector_index)) = frontier.pop_front() {
        let Some(neighbors) = adjacency.get(&system_id) else {
            continue;
        };
        for neighbor in neighbors {
            if assignments.contains_key(neighbor) {
                continue;
            }
            assignments.insert(*neighbor, sector_index);
            frontier.push_back((*neighbor, sector_index));
        }
    }

    // The generated route graph is connected, but keep a deterministic
    // geometric fallback so this pure generator remains total for custom data.
    for system in systems {
        assignments.entry(system.id).or_insert_with(|| {
            centers
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| {
                    let left_position = systems
                        .iter()
                        .find(|candidate| candidate.id == **left)
                        .expect("sector center belongs to the universe")
                        .position;
                    let right_position = systems
                        .iter()
                        .find(|candidate| candidate.id == **right)
                        .expect("sector center belongs to the universe")
                        .position;
                    system
                        .position
                        .distance_squared(left_position)
                        .total_cmp(&system.position.distance_squared(right_position))
                        .then_with(|| left.cmp(right))
                })
                .map(|(index, _)| index)
                .expect("at least one sector center exists")
        });
    }

    let mut members = vec![Vec::<SystemId>::new(); sector_count];
    for (system_id, sector_index) in &assignments {
        members[*sector_index].push(*system_id);
    }

    let mut gateways = vec![Vec::<Route>::new(); sector_count];
    for route in routes {
        let from_sector = assignments[&route.from];
        let to_sector = assignments[&route.to];
        if from_sector == to_sector {
            continue;
        }
        gateways[from_sector].push(*route);
        gateways[to_sector].push(*route);
    }

    members
        .into_iter()
        .zip(gateways)
        .enumerate()
        .map(|(index, (mut systems_in_sector, mut gateway_routes))| {
            systems_in_sector.sort();
            gateway_routes.sort_by_key(|route| (route.from, route.to));
            let center = sector_center(systems, &systems_in_sector);

            SectorDefinition {
                id: SectorId::from_index(index as u32),
                name: sector_name(seed, index),
                center,
                systems: systems_in_sector,
                gateway_routes,
            }
        })
        .collect()
}

fn sector_count_for(system_count: usize) -> usize {
    (system_count as f64)
        .sqrt()
        .round()
        .max(1.0)
        .min(system_count as f64) as usize
}

fn select_sector_centers(seed: u64, systems: &[StarSystem], sector_count: usize) -> Vec<SystemId> {
    let first_index = (splitmix64(seed ^ 0x5345_4354_4f52_5331) as usize) % systems.len();
    let mut centers = vec![systems[first_index].id];

    while centers.len() < sector_count {
        let next = systems
            .iter()
            .filter(|system| !centers.contains(&system.id))
            .max_by(|left, right| {
                let left_distance = minimum_center_distance(left, systems, &centers);
                let right_distance = minimum_center_distance(right, systems, &centers);
                left_distance
                    .total_cmp(&right_distance)
                    .then_with(|| {
                        seeded_system_rank(seed, right.id).cmp(&seeded_system_rank(seed, left.id))
                    })
                    .then_with(|| right.id.cmp(&left.id))
            })
            .expect("sector count cannot exceed system count");
        centers.push(next.id);
    }

    centers
}

fn minimum_center_distance(
    system: &StarSystem,
    systems: &[StarSystem],
    centers: &[SystemId],
) -> f32 {
    centers
        .iter()
        .map(|center_id| {
            let center = systems
                .iter()
                .find(|candidate| candidate.id == *center_id)
                .expect("selected sector center belongs to the universe");
            system.position.distance_squared(center.position)
        })
        .min_by(f32::total_cmp)
        .expect("at least one sector center exists")
}

fn sector_adjacency(systems: &[StarSystem], routes: &[Route]) -> BTreeMap<SystemId, Vec<SystemId>> {
    let mut adjacency = systems
        .iter()
        .map(|system| (system.id, Vec::<SystemId>::new()))
        .collect::<BTreeMap<_, _>>();

    for route in routes {
        if let Some(neighbors) = adjacency.get_mut(&route.from) {
            neighbors.push(route.to);
        }
        if let Some(neighbors) = adjacency.get_mut(&route.to) {
            neighbors.push(route.from);
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }
    adjacency
}

fn sector_center(systems: &[StarSystem], members: &[SystemId]) -> WorldPosition {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    for member in members {
        let position = systems
            .iter()
            .find(|system| system.id == *member)
            .expect("sector member belongs to the universe")
            .position;
        x += position.x;
        y += position.y;
        z += position.z;
    }
    let divisor = members.len() as f32;
    WorldPosition::new(x / divisor, y / divisor, z / divisor)
}

fn sector_name(seed: u64, index: usize) -> String {
    const NAMES: &[&str] = &[
        "Marche d’Orphée",
        "Voile de Nérée",
        "Couronne d’Aster",
        "Étendue de Vesper",
        "Lisière de Cyrène",
        "Détroit de Talos",
        "Arc de Sélène",
        "Front d’Ilyr",
        "Brèche d’Ophira",
        "Dérive de Praxia",
        "Cadran de Thémis",
        "Sillage de Méroé",
        "Confins d’Eidolon",
        "Veille d’Arkan",
        "Traverse de Calder",
        "Lointains de Nacréon",
    ];
    let offset = (splitmix64(seed ^ 0x4e4f_4d53_5345_4354) as usize) % NAMES.len();
    NAMES[(offset + index) % NAMES.len()].to_string()
}

fn seeded_system_rank(seed: u64, system_id: SystemId) -> u64 {
    splitmix64(seed ^ system_id.raw().wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn system_name(index: usize, rng: &mut ChaCha8Rng) -> String {
    const NAMES: &[&str] = &[
        "Hélianthe",
        "Vespera",
        "Néréide",
        "Talos",
        "Cyrène",
        "Ophira",
        "Méroé",
        "Eidolon",
        "Sélène",
        "Praxia",
        "Ilyr",
        "Calder",
        "Thémis",
        "Orphéon",
        "Nacréon",
        "Arkan",
        "Solédris",
        "Noctavéa",
        "Thaléryne",
        "Brontar",
        "Lyséa",
        "Oradis",
        "Kéméris",
        "Phanéon",
        "Lunévar",
        "Axoria",
        "Ilvaris",
        "Cendral",
        "Dikéa",
        "Cantéor",
        "Opalys",
        "Varkane",
        "Aurélys",
        "Crépuscor",
        "Pélagis",
        "Colosséa",
        "Myrion",
        "Zéphara",
        "Sabaël",
        "Mnémoris",
        "Nysséa",
        "Ordalis",
        "Vaelune",
        "Braséon",
        "Noméria",
        "Mélodran",
        "Iridys",
        "Kharéon",
        "Héméria",
        "Ombrelis",
        "Abyssara",
        "Gravéon",
        "Elarque",
        "Oryssia",
        "Aksomar",
        "Spectéon",
        "Artélys",
        "Agréon",
        "Ylvane",
        "Ferrélys",
        "Équoria",
        "Harméon",
        "Perléa",
        "Arcandor",
    ];

    if index == 0 {
        return NAMES[0].to_string();
    }

    // Preserve the two random draws used by generation version 2 so that this
    // editorial migration changes identities, not physical world properties.
    let _ = rng.random_range(0..8);
    let _ = rng.random_range(0..8);

    let base = NAMES[index % NAMES.len()];
    let cycle = index / NAMES.len();
    if cycle == 0 {
        base.to_string()
    } else {
        format!("{base}-{}", cycle + 1)
    }
}

fn planet_name(system_name: &str, planet_index: usize) -> String {
    const DESIGNATORS: &[&str] = &["b", "c", "d", "e", "f", "g", "h", "i"];
    let designator = DESIGNATORS.get(planet_index).copied().unwrap_or("x");
    format!("{system_name} {designator}")
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

    for sector in &universe.sectors {
        mix_u64(&mut hash, sector.id.raw());
        mix_bytes(&mut hash, sector.name.as_bytes());
        mix_u64(&mut hash, sector.center.x.to_bits() as u64);
        mix_u64(&mut hash, sector.center.y.to_bits() as u64);
        mix_u64(&mut hash, sector.center.z.to_bits() as u64);
        mix_u64(&mut hash, sector.systems.len() as u64);
        for system_id in &sector.systems {
            mix_u64(&mut hash, system_id.raw());
        }
        mix_u64(&mut hash, sector.gateway_routes.len() as u64);
        for route in &sector.gateway_routes {
            mix_u64(&mut hash, route.from.raw());
            mix_u64(&mut hash, route.to.raw());
        }
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
        assert_eq!(universe.systems.len(), MVP_SYSTEM_COUNT);
        assert_eq!(universe.sectors.len(), 8);
        assert!(!universe.routes.is_empty());
        assert_ne!(universe.generation_fingerprint, 0);
    }

    #[test]
    fn scale_presets_have_stable_distinct_sizes() {
        assert_eq!(UniverseScalePreset::default(), UniverseScalePreset::Mvp);
        assert_eq!(UniverseConfig::test().system_count, TEST_SYSTEM_COUNT);
        assert_eq!(UniverseConfig::mvp().system_count, MVP_SYSTEM_COUNT);
        assert_eq!(UniverseConfig::stress().system_count, STRESS_SYSTEM_COUNT);
        assert_eq!(
            UniverseScalePreset::from_slug("128"),
            Some(UniverseScalePreset::Stress),
        );
        assert_eq!(UniverseScalePreset::from_slug("unknown"), None);
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
    fn canonical_test_names_are_stable_and_unique() {
        let universe = generate_universe(UniverseConfig::test());
        let system_names = universe
            .systems
            .iter()
            .map(|system| system.name.as_str())
            .collect::<Vec<_>>();
        let unique_names = system_names.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(
            system_names,
            [
                "Hélianthe",
                "Vespera",
                "Néréide",
                "Talos",
                "Cyrène",
                "Ophira",
                "Méroé",
                "Eidolon",
                "Sélène",
                "Praxia",
                "Ilyr",
                "Calder",
                "Thémis",
                "Orphéon",
                "Nacréon",
                "Arkan",
            ],
        );
        assert_eq!(unique_names.len(), system_names.len());
        assert_eq!(universe.systems[0].planets[0].name, "Nacre");
        assert_eq!(universe.systems[0].planets[1].name, "Hélianthe c");
        assert_eq!(universe.systems[1].planets[0].name, "Vespera b");
    }

    #[test]
    fn mvp_system_names_replace_cycle_suffixes_with_canonical_names() {
        let universe = generate_universe(UniverseConfig::mvp());
        let system_names = universe
            .systems
            .iter()
            .map(|system| system.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(system_names[16], "Solédris");
        assert_eq!(system_names[32], "Aurélys");
        assert_eq!(system_names[48], "Héméria");
        assert_eq!(system_names[63], "Arcandor");
        assert!(!system_names.iter().any(|name| name.ends_with("-2")));
        assert_eq!(universe.systems[16].planets[0].name, "Solédris b");
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
        let universe = generate_universe(UniverseConfig::test());
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
        let first = generate_universe(UniverseConfig::test());
        let second = generate_universe(UniverseConfig::test());
        let mut unique = BTreeSet::new();

        assert_eq!(first.routes, second.routes);
        for route in &first.routes {
            assert!(route.from < route.to);
            assert!(unique.insert((route.from, route.to)));
        }
    }

    #[test]
    fn sectors_are_deterministic_complete_and_disjoint() {
        let first = generate_universe(UniverseConfig::test());
        let second = generate_universe(UniverseConfig::test());
        let mut memberships = BTreeMap::<SystemId, SectorId>::new();

        assert_eq!(first.sectors, second.sectors);
        assert_eq!(first.sectors.len(), 4);
        for sector in &first.sectors {
            assert!(!sector.systems.is_empty());
            assert!(sector.center.x.is_finite());
            assert!(sector.center.y.is_finite());
            assert!(sector.center.z.is_finite());

            let members = sector.systems.iter().copied().collect::<BTreeSet<_>>();
            let mut connected_members = BTreeSet::new();
            let mut frontier = vec![sector.systems[0]];
            while let Some(system_id) = frontier.pop() {
                if !connected_members.insert(system_id) {
                    continue;
                }
                frontier.extend(
                    first
                        .neighboring_systems(system_id)
                        .into_iter()
                        .filter(|neighbor| members.contains(neighbor)),
                );
            }
            assert_eq!(connected_members, members);

            for system_id in &sector.systems {
                assert!(first.system(*system_id).is_some());
                assert_eq!(memberships.insert(*system_id, sector.id), None);
                assert_eq!(
                    first.sector_for_system(*system_id).map(|found| found.id),
                    Some(sector.id),
                );
            }
        }
        assert_eq!(memberships.len(), first.systems.len());
    }

    #[test]
    fn sector_gateway_routes_are_exactly_the_intersector_edges() {
        let universe = generate_universe(UniverseConfig::test());

        for route in &universe.routes {
            let from_sector = universe
                .sector_for_system(route.from)
                .expect("route origin belongs to a sector");
            let to_sector = universe
                .sector_for_system(route.to)
                .expect("route destination belongs to a sector");
            let gateway_owners = universe
                .sectors
                .iter()
                .filter(|sector| sector.gateway_routes.contains(route))
                .map(|sector| sector.id)
                .collect::<BTreeSet<_>>();

            if from_sector.id == to_sector.id {
                assert!(gateway_owners.is_empty());
            } else {
                assert_eq!(
                    gateway_owners,
                    BTreeSet::from([from_sector.id, to_sector.id]),
                );
            }
        }
    }

    #[test]
    fn extended_mvp_scale_targets_six_to_ten_sectors() {
        let universe = generate_universe(UniverseConfig::mvp());

        assert_eq!(universe.systems.len(), MVP_SYSTEM_COUNT);
        assert!((6..=10).contains(&universe.sectors.len()));
        assert_eq!(universe.sectors.len(), 8);
    }

    #[test]
    fn stress_scale_is_available_without_becoming_the_default() {
        let universe = generate_universe(UniverseConfig::stress());

        assert_eq!(universe.systems.len(), STRESS_SYSTEM_COUNT);
        assert_ne!(
            UniverseConfig::default().system_count,
            UniverseConfig::stress().system_count,
        );
    }

    #[test]
    fn test_seed_fingerprint_is_stable() {
        assert_ne!(
            TEST_REFERENCE_FINGERPRINT, 0,
            "run tools/apply_mvp_003.py once to bootstrap the reference fingerprint"
        );
        let universe = generate_universe(UniverseConfig::test());
        assert_eq!(
            universe.generation_fingerprint, TEST_REFERENCE_FINGERPRINT,
            "the generated reference universe changed; increment GENERATION_VERSION only if intentional"
        );
    }

    #[test]
    #[ignore = "used by tools/apply_mvp_003.py to bootstrap the snapshot"]
    fn print_reference_seed_fingerprint() {
        let universe = generate_universe(UniverseConfig::test());
        println!("TEST_FINGERPRINT={}", universe.generation_fingerprint);
    }
}

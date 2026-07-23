#!/usr/bin/env python3
# Applique MVP-003 au dépôt Galactic.
from __future__ import annotations
import argparse
import difflib
import re
import shutil
import subprocess
from datetime import datetime
from pathlib import Path

IDS_RS = r'''use std::fmt;

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(raw: u64) -> Self { Self(raw) }
            pub const fn raw(self) -> u64 { self.0 }
            pub const fn index(self) -> u64 { self.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

stable_id!(UniverseId);
stable_id!(SystemId);
stable_id!(StarId);
stable_id!(PlanetId);
stable_id!(MoonId);
stable_id!(FactionId);
stable_id!(ColonyId);
stable_id!(FleetId);
stable_id!(MissionId);

impl UniverseId {
    pub const MVP: Self = Self::new(0);
}

impl SystemId {
    pub const fn from_index(index: u32) -> Self { Self::new(index as u64) }
}

impl StarId {
    pub const fn for_system(system_id: SystemId) -> Self { Self::new(system_id.raw()) }
}

impl PlanetId {
    pub const fn from_system_index(system_id: SystemId, planet_index: u32) -> Self {
        Self::new((system_id.raw() << 32) | planet_index as u64)
    }

    pub const fn system_id(self) -> SystemId { SystemId::new(self.raw() >> 32) }
    pub const fn local_index(self) -> u32 { self.raw() as u32 }
}

impl MoonId {
    pub const fn from_planet_index(planet_id: PlanetId, moon_index: u16) -> Self {
        Self::new((planet_id.raw() << 16) | moon_index as u64)
    }

    pub const fn local_index(self) -> u16 { self.raw() as u16 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchical_ids_are_stable_and_globally_distinct() {
        let system_a = SystemId::from_index(2);
        let system_b = SystemId::from_index(3);
        let planet_a0 = PlanetId::from_system_index(system_a, 0);
        let planet_a1 = PlanetId::from_system_index(system_a, 1);
        let planet_b0 = PlanetId::from_system_index(system_b, 0);

        assert_ne!(planet_a0, planet_a1);
        assert_ne!(planet_a0, planet_b0);
        assert_eq!(planet_a1.system_id(), system_a);
        assert_eq!(planet_a1.local_index(), 1);

        let moon = MoonId::from_planet_index(planet_a1, 4);
        assert_eq!(moon.local_index(), 4);
    }

    #[test]
    fn star_identity_is_derived_from_its_system() {
        let system = SystemId::from_index(7);
        assert_eq!(StarId::for_system(system).raw(), system.raw());
    }
}
'''


def world_rs(reference_fingerprint: int) -> str:
    template = r'''use std::collections::BTreeSet;
use std::f32::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::{PlanetId, StarId, SystemId, UniverseId, WorldPosition};

pub const MVP_UNIVERSE_SEED: u64 = 42;
pub const MVP_SYSTEM_COUNT: usize = 16;
pub const GENERATION_VERSION: u32 = 1;
pub const MVP_REFERENCE_FINGERPRINT: u64 = __REFERENCE_FINGERPRINT__;

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
        self.routes.iter().filter_map(|route| route.other(id)).collect()
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
        if a <= b { Self { from: a, to: b } } else { Self { from: b, to: a } }
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

fn generate_planets(
    system_id: SystemId,
    system_index: usize,
    rng: &mut ChaCha8Rng,
) -> Vec<Planet> {
    let count = if system_index == 0 { 3 } else { rng.random_range(1..=5) };

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

fn generate_routes(systems: &[StarSystem]) -> Vec<Route> {
    let mut unique = BTreeSet::new();

    for system in systems {
        let mut neighbors = systems
            .iter()
            .filter(|candidate| candidate.id != system.id)
            .map(|candidate| (
                system.position.distance_squared(candidate.position),
                candidate.id,
            ))
            .collect::<Vec<_>>();
        neighbors.sort_by(|a, b| a.0.total_cmp(&b.0));

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
'''
    return template.replace("__REFERENCE_FINGERPRINT__", str(reference_fingerprint))


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_domain/src/world.rs").exists()
            and (candidate / "crates/galactic_client/src/lib.rs").exists()
        ):
            return candidate
    raise SystemExit("Racine Galactic introuvable. Utilise --root.")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_if_changed(path: Path, content: str, dry_run: bool, backup_root: Path | None) -> bool:
    old = read(path) if path.exists() else ""
    content = content.rstrip() + "\n"
    if old == content:
        print(f"= inchangé : {path}")
        return False
    if dry_run:
        print("".join(difflib.unified_diff(
            old.splitlines(keepends=True),
            content.splitlines(keepends=True),
            fromfile=str(path),
            tofile=str(path),
        )))
        return True
    if path.exists() and backup_root:
        backup = backup_root / path.relative_to(ROOT)
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, backup)
    path.write_text(content, encoding="utf-8")
    print(f"+ mis à jour : {path}")
    return True


def patch_client(source: str) -> str:
    old_fmt = '"Galactic MVP | Bevy 0.19 | systems {} | routes {} | colonies {} | known {} | t {:.1}s | speed {} | selected {} | event {}",'
    new_fmt = '"Galactic MVP | Bevy 0.19 | seed {} | gen v{} | fp {:016x} | systems {} | routes {} | colonies {} | known {} | t {:.1}s | speed {} | selected {} | event {}",'
    if new_fmt not in source:
        if old_fmt not in source:
            raise SystemExit("Format HUD attendu introuvable dans galactic_client.")
        source = source.replace(old_fmt, new_fmt, 1)
        old_args = (
            "        state.universe.systems.len(),\n"
            "        state.universe.routes.len(),"
        )
        new_args = (
            "        state.universe.seed,\n"
            "        state.universe.generation_version,\n"
            "        state.universe.generation_fingerprint,\n"
            "        state.universe.systems.len(),\n"
            "        state.universe.routes.len(),"
        )
        if old_args not in source:
            raise SystemExit("Arguments HUD introuvables.")
        source = source.replace(old_args, new_args, 1)
    return source


def patch_state(source: str) -> str:
    old = (
        "        let home_system_id = SystemId::new(0);\n"
        "        let home_planet_id = PlanetId::new(0);"
    )
    new = (
        "        let home_system_id = SystemId::from_index(0);\n"
        "        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);"
    )
    if new in source:
        return source
    if old not in source:
        raise SystemExit("IDs de la planète mère introuvables dans state.rs.")
    return source.replace(old, new, 1)


def run(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT
    )
    print(result.stdout)
    if result.returncode != 0:
        raise SystemExit(f"Commande en échec : {' '.join(command)}")
    return result


def bootstrap_fingerprint(root: Path, world_path: Path) -> int:
    source = read(world_path)
    match = re.search(r"pub const MVP_REFERENCE_FINGERPRINT: u64 = (\d+);", source)
    if not match:
        raise SystemExit("Constante de fingerprint introuvable.")
    value = int(match.group(1))
    if value:
        print(f"= fingerprint déjà initialisé : {value}")
        return value

    result = run(
        [
            "cargo", "test", "-p", "galactic_domain",
            "print_reference_seed_fingerprint", "--", "--ignored", "--nocapture",
        ],
        root,
    )
    found = re.search(r"MVP_FINGERPRINT=(\d+)", result.stdout)
    if not found:
        raise SystemExit("Fingerprint impossible à extraire.")
    value = int(found.group(1))
    source = source.replace(
        "pub const MVP_REFERENCE_FINGERPRINT: u64 = 0;",
        f"pub const MVP_REFERENCE_FINGERPRINT: u64 = {value};",
        1,
    )
    world_path.write_text(source, encoding="utf-8")
    print(f"+ fingerprint initialisé : {value}")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description="Applique Galactic MVP-003")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    args = parser.parse_args()

    global ROOT
    ROOT = find_root(args.root.resolve())
    ids = ROOT / "crates/galactic_domain/src/ids.rs"
    world = ROOT / "crates/galactic_domain/src/world.rs"
    state = ROOT / "crates/galactic_sim/src/state.rs"
    client = ROOT / "crates/galactic_client/src/lib.rs"

    existing = read(world)
    fp_match = re.search(r"pub const MVP_REFERENCE_FINGERPRINT: u64 = (\d+);", existing)
    fingerprint = int(fp_match.group(1)) if fp_match else 0

    backup = None
    if not args.dry_run:
        backup = ROOT / ".mvp003-backup" / datetime.now().strftime("%Y%m%d-%H%M%S")

    changed = [
        write_if_changed(ids, IDS_RS, args.dry_run, backup),
        write_if_changed(world, world_rs(fingerprint), args.dry_run, backup),
        write_if_changed(state, patch_state(read(state)), args.dry_run, backup),
        write_if_changed(client, patch_client(read(client)), args.dry_run, backup),
    ]

    if args.dry_run:
        print(f"Dry-run : {sum(changed)} fichier(s) seraient modifiés.")
        return 0

    if any(changed):
        print(f"Sauvegarde : {backup}")

    if not args.skip_checks:
        bootstrap_fingerprint(ROOT, world)
        run(["cargo", "fmt", "--all"], ROOT)
        run([
            "cargo", "clippy", "--workspace", "--all-targets",
            "--all-features", "--", "-D", "warnings",
        ], ROOT)
        run(["cargo", "test", "--workspace"], ROOT)
    else:
        print("Checks ignorés ; le fingerprint restera à initialiser.")

    print("MVP-003 appliqué. Vérifie avec `git diff`, puis `cargo run --release`.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

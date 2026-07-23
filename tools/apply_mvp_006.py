#!/usr/bin/env python3
"""
Applique MVP-006 au dépôt Galactic.

Baseline analysée :
    930e26a03fdfcab352a653e34c5bee8ec96abe19
    feat mvp 5 add timer

Le script :
- garantit un graphe d'univers connexe et déterministe ;
- ajoute un index d'adjacence et le calcul des chemins minimaux ;
- masque les routes dont une extrémité est inconnue ;
- incrémente la version de génération ;
- recalcule automatiquement le fingerprint de la seed MVP ;
- crée des sauvegardes et exécute les contrôles Cargo.

Usage :
    python tools/apply_mvp_006.py --dry-run
    python tools/apply_mvp_006.py
    python tools/apply_mvp_006.py --skip-checks
    python tools/apply_mvp_006.py --root /chemin/vers/galactic

Le script est idempotent.
"""

from __future__ import annotations

import argparse
import difflib
import re
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = "930e26a03fdfcab352a653e34c5bee8ec96abe19"

UNIVERSE_RS = '// MVP-006: indexed connected graph and deterministic path finding\nuse std::collections::{HashMap, HashSet, VecDeque};\n\nuse galactic_domain::{\n    Planet, PlanetId, Route, StarSystem, SystemId, UniverseConfig, UniverseDefinition,\n    generate_universe,\n};\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum UniverseIndexError {\n    DuplicateSystem(SystemId),\n    DuplicatePlanet(PlanetId),\n    UnknownRouteEndpoint(SystemId),\n    SelfRoute(SystemId),\n    DuplicateRoute(Route),\n}\n\n/// Read-only repository around a generated universe.\n///\n/// The definition is owned by the simulation but has no mutable accessor. All\n/// runtime changes belong in `GameState` instead.\n#[derive(Debug, Clone)]\npub struct UniverseRepository {\n    definition: UniverseDefinition,\n    system_indices: HashMap<SystemId, usize>,\n    planet_indices: HashMap<PlanetId, (usize, usize)>,\n    adjacency: HashMap<SystemId, Vec<SystemId>>,\n}\n\nimpl UniverseRepository {\n    pub fn generate(config: UniverseConfig) -> Self {\n        Self::new(generate_universe(config))\n            .expect("the deterministic universe generator must produce a valid connected graph")\n    }\n\n    pub fn new(definition: UniverseDefinition) -> Result<Self, UniverseIndexError> {\n        let mut system_indices = HashMap::with_capacity(definition.systems.len());\n        let mut planet_indices = HashMap::new();\n\n        for (system_index, system) in definition.systems.iter().enumerate() {\n            if system_indices.insert(system.id, system_index).is_some() {\n                return Err(UniverseIndexError::DuplicateSystem(system.id));\n            }\n\n            for (planet_index, planet) in system.planets.iter().enumerate() {\n                if planet_indices\n                    .insert(planet.id, (system_index, planet_index))\n                    .is_some()\n                {\n                    return Err(UniverseIndexError::DuplicatePlanet(planet.id));\n                }\n            }\n        }\n\n        let mut adjacency = system_indices\n            .keys()\n            .copied()\n            .map(|system_id| (system_id, Vec::new()))\n            .collect::<HashMap<_, _>>();\n        let mut route_set = HashSet::with_capacity(definition.routes.len());\n\n        for route in &definition.routes {\n            if route.from == route.to {\n                return Err(UniverseIndexError::SelfRoute(route.from));\n            }\n            if !system_indices.contains_key(&route.from) {\n                return Err(UniverseIndexError::UnknownRouteEndpoint(route.from));\n            }\n            if !system_indices.contains_key(&route.to) {\n                return Err(UniverseIndexError::UnknownRouteEndpoint(route.to));\n            }\n\n            let canonical = Route::new(route.from, route.to);\n            if !route_set.insert(canonical) {\n                return Err(UniverseIndexError::DuplicateRoute(canonical));\n            }\n\n            adjacency\n                .get_mut(&canonical.from)\n                .expect("route endpoint was validated")\n                .push(canonical.to);\n            adjacency\n                .get_mut(&canonical.to)\n                .expect("route endpoint was validated")\n                .push(canonical.from);\n        }\n\n        for neighbors in adjacency.values_mut() {\n            neighbors.sort();\n            neighbors.dedup();\n        }\n\n        Ok(Self {\n            definition,\n            system_indices,\n            planet_indices,\n            adjacency,\n        })\n    }\n\n    pub fn definition(&self) -> &UniverseDefinition {\n        &self.definition\n    }\n\n    pub fn system(&self, id: SystemId) -> Option<&StarSystem> {\n        let index = *self.system_indices.get(&id)?;\n        self.definition.systems.get(index)\n    }\n\n    pub fn planet(&self, id: PlanetId) -> Option<&Planet> {\n        let (system_index, planet_index) = *self.planet_indices.get(&id)?;\n        self.definition\n            .systems\n            .get(system_index)?\n            .planets\n            .get(planet_index)\n    }\n\n    pub fn planet_location(&self, id: PlanetId) -> Option<(SystemId, &Planet)> {\n        let (system_index, planet_index) = *self.planet_indices.get(&id)?;\n        let system = self.definition.systems.get(system_index)?;\n        let planet = system.planets.get(planet_index)?;\n        Some((system.id, planet))\n    }\n\n    pub fn neighboring_systems(&self, id: SystemId) -> Vec<SystemId> {\n        self.adjacency.get(&id).cloned().unwrap_or_default()\n    }\n\n    pub fn route_exists(&self, from: SystemId, to: SystemId) -> bool {\n        self.adjacency\n            .get(&from)\n            .is_some_and(|neighbors| neighbors.binary_search(&to).is_ok())\n    }\n\n    /// Returns the deterministic shortest path by number of jumps.\n    ///\n    /// Both endpoints are included in the returned vector.\n    pub fn shortest_path(&self, from: SystemId, to: SystemId) -> Option<Vec<SystemId>> {\n        if !self.system_indices.contains_key(&from) || !self.system_indices.contains_key(&to) {\n            return None;\n        }\n        if from == to {\n            return Some(vec![from]);\n        }\n\n        let mut queue = VecDeque::new();\n        let mut visited = HashSet::new();\n        let mut previous = HashMap::<SystemId, SystemId>::new();\n\n        visited.insert(from);\n        queue.push_back(from);\n\n        while let Some(current) = queue.pop_front() {\n            let neighbors = self.adjacency.get(&current)?;\n            for neighbor in neighbors {\n                if !visited.insert(*neighbor) {\n                    continue;\n                }\n\n                previous.insert(*neighbor, current);\n                if *neighbor == to {\n                    return reconstruct_path(from, to, &previous);\n                }\n                queue.push_back(*neighbor);\n            }\n        }\n\n        None\n    }\n\n    pub fn hop_distance(&self, from: SystemId, to: SystemId) -> Option<u32> {\n        self.shortest_path(from, to)\n            .map(|path| path.len().saturating_sub(1) as u32)\n    }\n\n    pub fn all_systems_reachable_from(&self, start: SystemId) -> bool {\n        if !self.system_indices.contains_key(&start) {\n            return false;\n        }\n\n        let mut queue = VecDeque::new();\n        let mut visited = HashSet::new();\n        visited.insert(start);\n        queue.push_back(start);\n\n        while let Some(current) = queue.pop_front() {\n            if let Some(neighbors) = self.adjacency.get(&current) {\n                for neighbor in neighbors {\n                    if visited.insert(*neighbor) {\n                        queue.push_back(*neighbor);\n                    }\n                }\n            }\n        }\n\n        visited.len() == self.definition.systems.len()\n    }\n}\n\nfn reconstruct_path(\n    from: SystemId,\n    to: SystemId,\n    previous: &HashMap<SystemId, SystemId>,\n) -> Option<Vec<SystemId>> {\n    let mut path = vec![to];\n    let mut cursor = to;\n\n    while cursor != from {\n        cursor = *previous.get(&cursor)?;\n        path.push(cursor);\n    }\n\n    path.reverse();\n    Some(path)\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{PlanetId, Route, SystemId, UniverseConfig};\n\n    use super::*;\n\n    #[test]\n    fn repository_accesses_systems_and_planets_by_stable_id() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n        let home_system_id = SystemId::from_index(0);\n        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);\n\n        let system = repository\n            .system(home_system_id)\n            .expect("home system is indexed");\n        let planet = repository\n            .planet(home_planet_id)\n            .expect("home planet is indexed");\n        let (located_system_id, located_planet) = repository\n            .planet_location(home_planet_id)\n            .expect("planet location is indexed");\n\n        assert_eq!(system.id, home_system_id);\n        assert_eq!(planet.id, home_planet_id);\n        assert_eq!(located_system_id, home_system_id);\n        assert_eq!(located_planet.id, home_planet_id);\n    }\n\n    #[test]\n    fn regenerated_repository_matches_the_reference_universe() {\n        let left = UniverseRepository::generate(UniverseConfig::mvp());\n        let right = UniverseRepository::generate(UniverseConfig::mvp());\n\n        assert_eq!(left.definition(), right.definition());\n    }\n\n    #[test]\n    fn all_mvp_systems_are_reachable_from_home() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n\n        assert!(repository.all_systems_reachable_from(SystemId::from_index(0)));\n    }\n\n    #[test]\n    fn shortest_path_is_valid_and_deterministic() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n        let from = SystemId::from_index(0);\n        let to = SystemId::from_index(15);\n\n        let first = repository\n            .shortest_path(from, to)\n            .expect("the connected MVP graph has a path");\n        let second = repository\n            .shortest_path(from, to)\n            .expect("the same graph has the same path");\n\n        assert_eq!(first, second);\n        assert_eq!(first.first(), Some(&from));\n        assert_eq!(first.last(), Some(&to));\n        assert!(first\n            .windows(2)\n            .all(|edge| repository.route_exists(edge[0], edge[1])));\n        assert_eq!(\n            repository.hop_distance(from, to),\n            Some(first.len().saturating_sub(1) as u32)\n        );\n    }\n\n    #[test]\n    fn duplicate_routes_are_rejected() {\n        let mut definition = generate_universe(UniverseConfig::mvp());\n        let duplicate = *definition.routes.first().expect("MVP routes exist");\n        definition.routes.push(Route::new(duplicate.to, duplicate.from));\n\n        assert!(matches!(\n            UniverseRepository::new(definition),\n            Err(UniverseIndexError::DuplicateRoute(route)) if route == duplicate\n        ));\n    }\n}\n'
STATE_RS = '// MVP-006: mutable discoveries control which immutable routes are visible\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId, UniverseDefinition,\n};\n\nuse crate::{SelectionTarget, StrategicClock, UniverseRepository};\n\n/// Version of the mutable in-memory state contract.\n///\n/// Version 2 replaces floating elapsed seconds with a deterministic tick clock.\npub const GAME_STATE_VERSION: u32 = 2;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct GameState {\n    pub version: u32,\n    pub player_faction: FactionId,\n    pub colonies: Vec<ColonyState>,\n    pub known_systems: Vec<SystemId>,\n    pub selected: SelectionTarget,\n    pub clock: StrategicClock,\n}\n\nimpl GameState {\n    pub fn new(universe: &UniverseRepository) -> Self {\n        let home_system_id = SystemId::from_index(0);\n        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);\n        let player_faction = FactionId::new(0);\n        let mut known_systems = vec![home_system_id];\n        known_systems.extend(universe.neighboring_systems(home_system_id));\n        known_systems.sort();\n        known_systems.dedup();\n\n        debug_assert!(universe.system(home_system_id).is_some());\n        debug_assert!(universe.planet(home_planet_id).is_some());\n\n        Self {\n            version: GAME_STATE_VERSION,\n            player_faction,\n            colonies: vec![ColonyState {\n                id: ColonyId::new(0),\n                name: "Aster Prime Colony".to_string(),\n                faction: player_faction,\n                system_id: home_system_id,\n                planet_id: home_planet_id,\n                stock: ResourceStock::new(120, 45, 80, 30),\n            }],\n            known_systems,\n            selected: SelectionTarget::System(home_system_id),\n            clock: StrategicClock::new(),\n        }\n    }\n\n    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {\n        self.colonies.iter().find(|colony| colony.id == id)\n    }\n\n    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {\n        self.colonies.iter_mut().find(|colony| colony.id == id)\n    }\n\n    pub fn is_system_known(&self, system_id: SystemId) -> bool {\n        self.known_systems.contains(&system_id)\n    }\n\n    /// A route becomes visible only when both endpoint systems are known.\n    ///\n    /// Detailed knowledge levels are introduced later by MVP-009.\n    pub fn visible_routes<\'a>(\n        &\'a self,\n        universe: &\'a UniverseDefinition,\n    ) -> impl Iterator<Item = &\'a Route> + \'a {\n        universe.routes.iter().filter(|route| {\n            self.is_system_known(route.from) && self.is_system_known(route.to)\n        })\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonyState {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{ColonyId, SystemId, UniverseConfig};\n\n    use super::*;\n\n    #[test]\n    fn colony_is_accessible_by_stable_id() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        let colony = state\n            .colony(ColonyId::new(0))\n            .expect("home colony is indexed by its stable ID");\n\n        assert_eq!(colony.name, "Aster Prime Colony");\n    }\n\n    #[test]\n    fn new_game_starts_at_tick_zero_and_speed_one() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        assert_eq!(state.clock.current_tick().value(), 0);\n        assert_eq!(state.clock.speed(), crate::TimeSpeed::X1);\n    }\n\n    #[test]\n    fn visible_routes_never_reveal_unknown_endpoints() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n        let home = SystemId::from_index(0);\n        let neighbor = repository\n            .neighboring_systems(home)\n            .into_iter()\n            .next()\n            .expect("home system has a route");\n        let mut state = GameState::new(&repository);\n        state.known_systems = vec![home, neighbor];\n        let visible = state\n            .visible_routes(repository.definition())\n            .collect::<Vec<_>>();\n\n        assert_eq!(visible.len(), 1);\n        assert!(visible.iter().all(|route| {\n            state.is_system_known(route.from) && state.is_system_known(route.to)\n        }));\n    }\n}\n'
DOC_APPEND = "\n## MVP-006 — Graphe d'univers et routes\n\nL'univers MVP est maintenant un graphe connexe plutôt qu'un simple ensemble de\npositions :\n\n```text\nSystèmes générés\n        │\n        ├── arbre couvrant minimal déterministe\n        └── routes locales vers les voisins proches\n        ▼\nGraphe connexe sans doublon\n        ▼\nIndex d'adjacence UniverseRepository\n        ▼\nVoisinages / chemin minimal / distance en sauts\n```\n\nRègles :\n\n- la seed MVP contient toujours 16 systèmes, dans la fourchette cible 12–20 ;\n- un arbre couvrant minimal garantit que tous les systèmes sont accessibles\n  depuis `SystemId(0)` ;\n- des routes locales supplémentaires évitent un graphe réduit à un simple arbre ;\n- les routes sont canoniques, sans boucle et sans doublon ;\n- `UniverseRepository::shortest_path()` utilise un BFS déterministe ;\n- `UniverseRepository::hop_distance()` retourne le nombre minimal de sauts ;\n- l'univers complet reste immuable et est régénéré depuis la seed ;\n- la vue Univers ne trace que les routes dont les deux systèmes sont connus ;\n- l'instanciation limitée aux systèmes découverts sera traitée par `MVP-007`.\n\nLa modification volontaire du graphe incrémente `GENERATION_VERSION` et produit\nun nouveau fingerprint de référence pour la seed MVP.\n"
WORLD_NEW_ROUTES = '// MVP-006: guarantee connectivity with a deterministic minimum spanning tree,\n// then add local nearest-neighbor links to keep the map tactically interesting.\nfn generate_routes(systems: &[StarSystem]) -> Vec<Route> {\n    if systems.len() <= 1 {\n        return Vec::new();\n    }\n\n    let mut unique = BTreeSet::new();\n    let mut connected = vec![false; systems.len()];\n    connected[0] = true;\n\n    // Prim\'s algorithm over geometric distances. System IDs break equal-distance\n    // ties so the same seed always yields the same route graph.\n    for _ in 1..systems.len() {\n        let mut best: Option<(f32, SystemId, SystemId, usize)> = None;\n\n        for (from_index, from) in systems.iter().enumerate() {\n            if !connected[from_index] {\n                continue;\n            }\n\n            for (to_index, to) in systems.iter().enumerate() {\n                if connected[to_index] {\n                    continue;\n                }\n\n                let distance = from.position.distance_squared(to.position);\n                let replace = match best {\n                    None => true,\n                    Some((best_distance, best_from, best_to, _)) => distance\n                        .total_cmp(&best_distance)\n                        .then_with(|| from.id.cmp(&best_from))\n                        .then_with(|| to.id.cmp(&best_to))\n                        .is_lt(),\n                };\n\n                if replace {\n                    best = Some((distance, from.id, to.id, to_index));\n                }\n            }\n        }\n\n        let (_, from, to, to_index) =\n            best.expect("a disconnected vertex must have an edge to the connected set");\n        let route = Route::new(from, to);\n        unique.insert((route.from.raw(), route.to.raw()));\n        connected[to_index] = true;\n    }\n\n    // Add each system\'s two nearest neighbors. The BTreeSet preserves canonical\n    // ordering and removes edges already provided by the spanning tree.\n    for system in systems {\n        let mut neighbors = systems\n            .iter()\n            .filter(|candidate| candidate.id != system.id)\n            .map(|candidate| {\n                (\n                    system.position.distance_squared(candidate.position),\n                    candidate.id,\n                )\n            })\n            .collect::<Vec<_>>();\n        neighbors.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));\n\n        for (_, neighbor_id) in neighbors.into_iter().take(2) {\n            let route = Route::new(system.id, neighbor_id);\n            unique.insert((route.from.raw(), route.to.raw()));\n        }\n    }\n\n    unique\n        .into_iter()\n        .map(|(from, to)| Route::new(SystemId::new(from), SystemId::new(to)))\n        .collect()\n}\n'
WORLD_TESTS = '\n    #[test]\n    fn route_graph_is_connected_from_home() {\n        let universe = generate_universe(UniverseConfig::mvp());\n        let mut visited = BTreeSet::new();\n        let mut frontier = vec![SystemId::from_index(0)];\n\n        while let Some(system_id) = frontier.pop() {\n            if !visited.insert(system_id) {\n                continue;\n            }\n            frontier.extend(\n                universe\n                    .neighboring_systems(system_id)\n                    .into_iter()\n                    .filter(|neighbor| !visited.contains(neighbor)),\n            );\n        }\n\n        assert_eq!(visited.len(), universe.systems.len());\n    }\n\n    #[test]\n    fn routes_are_unique_canonical_and_deterministic() {\n        let first = generate_universe(UniverseConfig::mvp());\n        let second = generate_universe(UniverseConfig::mvp());\n        let mut unique = BTreeSet::new();\n\n        assert_eq!(first.routes, second.routes);\n        for route in &first.routes {\n            assert!(route.from < route.to);\n            assert!(unique.insert((route.from, route.to)));\n        }\n    }\n'


@dataclass(frozen=True)
class Update:
    path: Path
    before: str
    after: str


def run(
    command: list[str],
    *,
    cwd: Path,
    check: bool = True,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if capture and result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : {' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_domain/src/world.rs").exists()
            and (candidate / "crates/galactic_sim/src/universe.rs").exists()
            and (candidate / "crates/galactic_client/src/lib.rs").exists()
        ):
            return candidate
    raise SystemExit("Racine Galactic introuvable. Utilise --root.")


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def verify_baseline(root: Path, force: bool) -> None:
    head = run(["git", "rev-parse", "HEAD"], cwd=root).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return

    ancestor = run(
        ["git", "merge-base", "--is-ancestor", EXPECTED_BASELINE_COMMIT, "HEAD"],
        cwd=root,
        check=False,
    )
    if ancestor.returncode == 0:
        print(f"Baseline présente dans l'historique ; HEAD actuel : {head}")
        return
    if force:
        print("WARNING: baseline différente, poursuite autorisée par --force.")
        return

    raise SystemExit(
        "Le dépôt local ne correspond pas à la baseline MVP-005 analysée.\n"
        f"HEAD={head}\nAttendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après vérification."
    )


def verify_mvp5(root: Path) -> None:
    state = (root / "crates/galactic_sim/src/state.rs").read_text(encoding="utf-8")
    time_source = (root / "crates/galactic_sim/src/time.rs").read_text(encoding="utf-8")
    persistence = (
        root / "crates/galactic_persistence/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    if "pub clock: StrategicClock" not in state:
        failures.append("StrategicClock absent de GameState")
    if "STRATEGIC_TICKS_PER_SECOND" not in time_source:
        failures.append("horloge fixe MVP-005 absente")
    if "StrategicClockSave" not in persistence:
        failures.append("horloge stratégique absente de la sauvegarde")

    if failures:
        raise SystemExit(
            "Baseline MVP-005 incohérente :\n- " + "\n- ".join(failures)
        )


def replace_once(source: str, old: str, new: str, label: str) -> str:
    if new in source:
        return source
    if old not in source:
        raise SystemExit(
            f"Bloc attendu introuvable pour {label}. "
            "Le dépôt a probablement évolué."
        )
    return source.replace(old, new, 1)


def patch_world(source: str) -> str:
    if "// MVP-006: guarantee connectivity" in source:
        return normalize(source)

    updated = source
    updated = replace_once(
        updated,
        "pub const GENERATION_VERSION: u32 = 1;",
        "pub const GENERATION_VERSION: u32 = 2;",
        "version de génération",
    )
    updated = re.sub(
        r"pub const MVP_REFERENCE_FINGERPRINT: u64 = \d+;",
        "pub const MVP_REFERENCE_FINGERPRINT: u64 = 0;",
        updated,
        count=1,
    )

    old_routes = re.search(
        r"fn generate_routes\(systems: &\[StarSystem\]\) -> Vec<Route> \{.*?\n\}\n\nfn system_name",
        updated,
        flags=re.DOTALL,
    )
    if old_routes is None:
        raise SystemExit("Fonction generate_routes attendue introuvable.")

    updated = (
        updated[: old_routes.start()]
        + WORLD_NEW_ROUTES
        + "\n\nfn system_name"
        + updated[old_routes.end() :]
    )

    test_marker = "    #[test]\n    fn reference_seed_fingerprint_is_stable()"
    if test_marker not in updated:
        raise SystemExit("Point d'insertion des tests de graphe introuvable.")
    updated = updated.replace(test_marker, WORLD_TESTS + "\n" + test_marker, 1)
    return normalize(updated)


def patch_client(source: str) -> str:
    updated = source

    updated = updated.replace(
        "Space pause | 1 x1 | 2 x2 | 3 x4 | R rebuild views | immutable universe + mutable state live outside Bevy views",
        "Space pause | 1 x1 | 2 x2 | 3 x4 | R rebuild views | only discovered routes are visible",
        1,
    )

    old_event_setup = """    let last_event = log
        .last_event
        .map(event_label)
        .unwrap_or_else(|| "ready".to_string());

    text.0 = format!("""
    new_event_setup = """    let last_event = log
        .last_event
        .map(event_label)
        .unwrap_or_else(|| "ready".to_string());
    let visible_route_count = state.visible_routes(universe).count();

    text.0 = format!("""
    if "let visible_route_count = state.visible_routes(universe).count();" not in updated:
        updated = replace_once(
            updated,
            old_event_setup,
            new_event_setup,
            "compteur de routes visibles",
        )

    updated = updated.replace(
        "| systems {} | routes {} | colonies {} |",
        "| systems {} | routes {}/{} | colonies {} |",
        1,
    )
    updated = updated.replace(
        """        universe.systems.len(),
        universe.routes.len(),
        state.colonies.len(),""",
        """        universe.systems.len(),
        visible_route_count,
        universe.routes.len(),
        state.colonies.len(),""",
        1,
    )

    old_draw = """fn draw_routes(mut gizmos: Gizmos, simulation: Res<SimulationResource>) {
    let universe = simulation.simulation().universe();

    for route in &universe.routes {
        let Some(from) = universe.system(route.from) else {
            continue;
        };
        let Some(to) = universe.system(route.to) else {
            continue;
        };

        gizmos.line(
            to_vec3(from.position),
            to_vec3(to.position),
            Color::srgba(0.28, 0.62, 0.94, 0.35),
        );
    }
}"""
    new_draw = """fn draw_routes(mut gizmos: Gizmos, simulation: Res<SimulationResource>) {
    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let state = simulation.state();

    for route in state.visible_routes(universe) {
        let Some(from) = universe.system(route.from) else {
            continue;
        };
        let Some(to) = universe.system(route.to) else {
            continue;
        };

        gizmos.line(
            to_vec3(from.position),
            to_vec3(to.position),
            Color::srgba(0.28, 0.62, 0.94, 0.52),
        );
    }
}"""
    updated = replace_once(updated, old_draw, new_draw, "filtrage des routes")

    if "for route in &universe.routes" in updated:
        raise SystemExit("Le client trace encore toutes les routes de l'univers.")
    return normalize(updated)


def patch_docs(source: str) -> str:
    if "## MVP-006 — Graphe d'univers et routes" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_sim/src/universe.rs": normalize(UNIVERSE_RS),
        root / "crates/galactic_sim/src/state.rs": normalize(STATE_RS),
    }
    for path, after in replacements.items():
        before = path.read_text(encoding="utf-8")
        if before != after:
            updates.append(Update(path, before, after))

    world_path = root / "crates/galactic_domain/src/world.rs"
    world_before = world_path.read_text(encoding="utf-8")
    world_after = patch_world(world_before)
    if world_before != world_after:
        updates.append(Update(world_path, world_before, world_after))

    client_path = root / "crates/galactic_client/src/lib.rs"
    client_before = client_path.read_text(encoding="utf-8")
    client_after = patch_client(client_before)
    if client_before != client_after:
        updates.append(Update(client_path, client_before, client_after))

    docs_path = root / "docs/mvp_architecture.md"
    docs_before = docs_path.read_text(encoding="utf-8")
    docs_after = patch_docs(docs_before)
    if docs_before != docs_after:
        updates.append(Update(docs_path, docs_before, docs_after))

    return updates


def show_diff(update: Update, root: Path) -> None:
    relative = update.path.relative_to(root)
    print(
        "".join(
            difflib.unified_diff(
                update.before.splitlines(keepends=True),
                update.after.splitlines(keepends=True),
                fromfile=f"a/{relative}",
                tofile=f"b/{relative}",
            )
        ),
        end="",
    )


def apply_updates(updates: list[Update], root: Path, dry_run: bool) -> None:
    if not updates:
        print("MVP-006 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = root / ".mvp006-backup" / datetime.now().strftime("%Y%m%d-%H%M%S")
    for update in updates:
        relative = update.path.relative_to(root)
        backup = backup_root / relative
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(update.path, backup)
        update.path.write_text(update.after, encoding="utf-8")
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def bootstrap_fingerprint(root: Path) -> None:
    world_path = root / "crates/galactic_domain/src/world.rs"
    source = world_path.read_text(encoding="utf-8")
    match = re.search(
        r"pub const MVP_REFERENCE_FINGERPRINT: u64 = (\d+);",
        source,
    )
    if match is None:
        raise SystemExit("Constante MVP_REFERENCE_FINGERPRINT introuvable.")

    if int(match.group(1)) != 0:
        print(f"= fingerprint déjà initialisé : {match.group(1)}")
        return

    result = run(
        [
            "cargo",
            "test",
            "-p",
            "galactic_domain",
            "print_reference_seed_fingerprint",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=root,
    )
    fingerprint = re.search(r"MVP_FINGERPRINT=(\d+)", result.stdout)
    if fingerprint is None:
        raise SystemExit("Fingerprint impossible à extraire de la sortie Cargo.")

    value = fingerprint.group(1)
    source = source.replace(
        "pub const MVP_REFERENCE_FINGERPRINT: u64 = 0;",
        f"pub const MVP_REFERENCE_FINGERPRINT: u64 = {value};",
        1,
    )
    world_path.write_text(source, encoding="utf-8")
    print(f"+ fingerprint de référence initialisé : {value}")


def checks(root: Path) -> None:
    bootstrap_fingerprint(root)
    run(["cargo", "fmt", "--all"], cwd=root, capture=False)
    run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        cwd=root,
        capture=False,
    )
    run(["cargo", "test", "--workspace"], cwd=root, capture=False)
    run(["cargo", "build", "--release"], cwd=root, capture=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_mvp5(root)

    status = run(["git", "status", "--porcelain"], cwd=root).stdout
    if status.strip():
        print("WARNING: working tree already contains changes.")
        print(status, end="" if status.endswith("\n") else "\n")

    updates = collect_updates(root)
    apply_updates(updates, root, args.dry_run)

    if args.dry_run:
        print(f"\nDry-run complete: {len(updates)} file(s) would change.")
        return 0

    if args.skip_checks:
        print(
            "\nChecks ignorés. Le fingerprint reste éventuellement à initialiser.\n"
            "Relance sans --skip-checks ou exécute ensuite :\n"
            "  cargo test -p galactic_domain print_reference_seed_fingerprint -- --ignored --nocapture\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        checks(root)

    print(
        "\nMVP-006 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

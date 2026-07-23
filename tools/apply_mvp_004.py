#!/usr/bin/env python3
"""Apply MVP-004 to the Galactic repository.

MVP-004 separates the immutable universe generated from the seed from the
mutable game state.

Expected baseline:
    23ea668e81d6e8abb30d15b1049bb93e0f1869e9
    feat add mvp 3 seed

Usage from the repository root:
    python tools/apply_mvp_004.py --dry-run
    python tools/apply_mvp_004.py

Options:
    --root PATH      Explicit repository root.
    --dry-run        Show diffs without writing.
    --skip-checks    Do not run fmt, clippy and tests.
    --force          Allow a different Git HEAD after structural checks.

The script is idempotent and creates backups under .mvp004-backup/ before
replacing existing files.
"""

from __future__ import annotations

import argparse
import difflib
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path

EXPECTED_HEAD = "23ea668e81d6e8abb30d15b1049bb93e0f1869e9"
MVP004_MARKER = "MVP-004: immutable generated universe separated from mutable game state"


def _rust(template: str) -> str:
    return (
        template.replace("__MVP004_MARKER__", MVP004_MARKER)
        .replace("{{", "{")
        .replace("}}", "}")
    )

SIM_LIB_RS = _rust(r'''// __MVP004_MARKER__
pub mod command;
pub mod event;
pub mod simulation;
pub mod state;
pub mod universe;

pub use command::*;
pub use event::*;
pub use simulation::*;
pub use state::*;
pub use universe::*;
''')

UNIVERSE_RS = _rust(r'''// __MVP004_MARKER__
use std::collections::HashMap;

use galactic_domain::{
    Planet, PlanetId, StarSystem, SystemId, UniverseConfig, UniverseDefinition, generate_universe,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseIndexError {{
    DuplicateSystem(SystemId),
    DuplicatePlanet(PlanetId),
}}

/// Read-only repository around a generated universe.
///
/// The definition is owned by the simulation but has no mutable accessor. All
/// runtime changes belong in `GameState` instead.
#[derive(Debug, Clone)]
pub struct UniverseRepository {{
    definition: UniverseDefinition,
    system_indices: HashMap<SystemId, usize>,
    planet_indices: HashMap<PlanetId, (usize, usize)>,
}}

impl UniverseRepository {{
    pub fn generate(config: UniverseConfig) -> Self {{
        Self::new(generate_universe(config))
            .expect("the deterministic universe generator must produce unique stable IDs")
    }}

    pub fn new(definition: UniverseDefinition) -> Result<Self, UniverseIndexError> {{
        let mut system_indices = HashMap::with_capacity(definition.systems.len());
        let mut planet_indices = HashMap::new();

        for (system_index, system) in definition.systems.iter().enumerate() {{
            if system_indices.insert(system.id, system_index).is_some() {{
                return Err(UniverseIndexError::DuplicateSystem(system.id));
            }}

            for (planet_index, planet) in system.planets.iter().enumerate() {{
                if planet_indices
                    .insert(planet.id, (system_index, planet_index))
                    .is_some()
                {{
                    return Err(UniverseIndexError::DuplicatePlanet(planet.id));
                }}
            }}
        }}

        Ok(Self {{
            definition,
            system_indices,
            planet_indices,
        }})
    }}

    pub fn definition(&self) -> &UniverseDefinition {{
        &self.definition
    }}

    pub fn system(&self, id: SystemId) -> Option<&StarSystem> {{
        let index = *self.system_indices.get(&id)?;
        self.definition.systems.get(index)
    }}

    pub fn planet(&self, id: PlanetId) -> Option<&Planet> {{
        let (system_index, planet_index) = *self.planet_indices.get(&id)?;
        self.definition
            .systems
            .get(system_index)?
            .planets
            .get(planet_index)
    }}

    pub fn planet_location(&self, id: PlanetId) -> Option<(SystemId, &Planet)> {{
        let (system_index, planet_index) = *self.planet_indices.get(&id)?;
        let system = self.definition.systems.get(system_index)?;
        let planet = system.planets.get(planet_index)?;
        Some((system.id, planet))
    }}

    pub fn neighboring_systems(&self, id: SystemId) -> Vec<SystemId> {{
        self.definition.neighboring_systems(id)
    }}
}}

#[cfg(test)]
mod tests {{
    use galactic_domain::{{PlanetId, SystemId, UniverseConfig}};

    use super::*;

    #[test]
    fn repository_accesses_systems_and_planets_by_stable_id() {{
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let home_system_id = SystemId::from_index(0);
        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);

        let system = repository
            .system(home_system_id)
            .expect("home system is indexed");
        let planet = repository
            .planet(home_planet_id)
            .expect("home planet is indexed");
        let (located_system_id, located_planet) = repository
            .planet_location(home_planet_id)
            .expect("planet location is indexed");

        assert_eq!(system.id, home_system_id);
        assert_eq!(planet.id, home_planet_id);
        assert_eq!(located_system_id, home_system_id);
        assert_eq!(located_planet.id, home_planet_id);
    }}

    #[test]
    fn regenerated_repository_matches_the_reference_universe() {{
        let left = UniverseRepository::generate(UniverseConfig::mvp());
        let right = UniverseRepository::generate(UniverseConfig::mvp());

        assert_eq!(left.definition(), right.definition());
    }}
}}
''')

STATE_RS = _rust(r'''// __MVP004_MARKER__
use galactic_domain::{{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId,
}};

use crate::{{SelectionTarget, TimeSpeed, UniverseRepository}};

/// Version of the mutable in-memory state contract.
///
/// This version is independent from the generated universe version and from
/// the persistence envelope version.
pub const GAME_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {{
    pub version: u32,
    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub known_systems: Vec<SystemId>,
    pub selected: SelectionTarget,
    pub elapsed_seconds: f32,
    pub speed: TimeSpeed,
}}

impl GameState {{
    pub fn new(universe: &UniverseRepository) -> Self {{
        let home_system_id = SystemId::from_index(0);
        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);
        let player_faction = FactionId::new(0);
        let mut known_systems = vec![home_system_id];
        known_systems.extend(universe.neighboring_systems(home_system_id));
        known_systems.sort();
        known_systems.dedup();

        debug_assert!(universe.system(home_system_id).is_some());
        debug_assert!(universe.planet(home_planet_id).is_some());

        Self {{
            version: GAME_STATE_VERSION,
            player_faction,
            colonies: vec![ColonyState {{
                id: ColonyId::new(0),
                name: "Aster Prime Colony".to_string(),
                faction: player_faction,
                system_id: home_system_id,
                planet_id: home_planet_id,
                stock: ResourceStock::new(120, 45, 80, 30),
            }}],
            known_systems,
            selected: SelectionTarget::System(home_system_id),
            elapsed_seconds: 0.0,
            speed: TimeSpeed::X1,
        }}
    }}

    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {{
        self.colonies.iter().find(|colony| colony.id == id)
    }}

    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {{
        self.colonies.iter_mut().find(|colony| colony.id == id)
    }}
}}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonyState {{
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
}}

#[cfg(test)]
mod tests {{
    use galactic_domain::{{ColonyId, UniverseConfig}};

    use super::*;

    #[test]
    fn colony_is_accessible_by_stable_id() {{
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);

        let colony = state
            .colony(ColonyId::new(0))
            .expect("home colony is indexed by its stable ID");

        assert_eq!(colony.name, "Aster Prime Colony");
    }}
}}
''')

SIMULATION_RS = _rust(r'''// __MVP004_MARKER__
use std::collections::HashSet;

use galactic_domain::{{
    ColonyId, PlanetId, SystemId, UniverseConfig, UniverseDefinition,
}};

use crate::{{
    GAME_STATE_VERSION, GameCommand, GameEvent, GameState, SelectionTarget, TimeSpeed,
    UniverseIndexError, UniverseRepository,
}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationBuildError {{
    InvalidUniverse(UniverseIndexError),
    UnsupportedStateVersion {{ expected: u32, found: u32 }},
    DuplicateColony(ColonyId),
    UnknownKnownSystem(SystemId),
    UnknownColonySystem {{ colony_id: ColonyId, system_id: SystemId }},
    UnknownColonyPlanet {{ colony_id: ColonyId, planet_id: PlanetId }},
    ColonyPlanetSystemMismatch {{
        colony_id: ColonyId,
        system_id: SystemId,
        planet_id: PlanetId,
    }},
    InvalidSelectedSystem(SystemId),
    InvalidSelectedPlanet {{ system_id: SystemId, planet_id: PlanetId }},
}}

impl From<UniverseIndexError> for SimulationBuildError {{
    fn from(error: UniverseIndexError) -> Self {{
        Self::InvalidUniverse(error)
    }}
}}

#[derive(Debug, Clone)]
pub struct Simulation {{
    universe: UniverseRepository,
    state: GameState,
}}

impl Simulation {{
    pub fn new(config: UniverseConfig) -> Self {{
        let universe = UniverseRepository::generate(config);
        let state = GameState::new(&universe);
        Self {{ universe, state }}
    }}

    pub fn from_parts(
        universe: UniverseDefinition,
        state: GameState,
    ) -> Result<Self, SimulationBuildError> {{
        let universe = UniverseRepository::new(universe)?;
        validate_state(&universe, &state)?;
        Ok(Self {{ universe, state }})
    }}

    /// Immutable generated definition. No mutable universe accessor exists.
    pub fn universe(&self) -> &UniverseDefinition {{
        self.universe.definition()
    }}

    pub fn universe_repository(&self) -> &UniverseRepository {{
        &self.universe
    }}

    pub fn state(&self) -> &GameState {{
        &self.state
    }}

    pub fn state_mut(&mut self) -> &mut GameState {{
        &mut self.state
    }}

    pub fn apply_command(&mut self, command: GameCommand) -> Vec<GameEvent> {{
        match command {{
            GameCommand::TogglePause => {{
                let next_speed = if self.state.speed == TimeSpeed::Paused {{
                    TimeSpeed::X1
                }} else {{
                    TimeSpeed::Paused
                }};
                self.set_speed(next_speed)
            }}
            GameCommand::SetSpeed(speed) => self.set_speed(speed),
            GameCommand::SelectSystem(system_id) => self.select_system(system_id),
            GameCommand::SelectPlanet {{
                system_id,
                planet_id,
            }} => self.select_planet(system_id, planet_id),
            GameCommand::ClearSelection => self.set_selection(SelectionTarget::None),
        }}
    }}

    pub fn tick(&mut self, delta_seconds: f32) -> Vec<GameEvent> {{
        let scaled_delta = delta_seconds.max(0.0) * self.state.speed.multiplier();
        if scaled_delta == 0.0 {{
            return Vec::new();
        }}

        self.state.elapsed_seconds += scaled_delta;
        vec![GameEvent::TickAdvanced {{
            delta_seconds: scaled_delta,
            elapsed_seconds: self.state.elapsed_seconds,
        }}]
    }}

    fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEvent> {{
        if self.state.speed == speed {{
            return Vec::new();
        }}

        self.state.speed = speed;
        vec![GameEvent::SpeedChanged(speed)]
    }}

    fn select_system(&mut self, system_id: SystemId) -> Vec<GameEvent> {{
        if self.universe.system(system_id).is_none() {{
            return Vec::new();
        }}

        self.set_selection(SelectionTarget::System(system_id))
    }}

    fn select_planet(&mut self, system_id: SystemId, planet_id: PlanetId) -> Vec<GameEvent> {{
        let Some((planet_system_id, _)) = self.universe.planet_location(planet_id) else {{
            return Vec::new();
        }};
        if planet_system_id != system_id {{
            return Vec::new();
        }}

        self.set_selection(SelectionTarget::Planet {{
            system_id,
            planet_id,
        }})
    }}

    fn set_selection(&mut self, selection: SelectionTarget) -> Vec<GameEvent> {{
        if self.state.selected == selection {{
            return Vec::new();
        }}

        self.state.selected = selection;
        vec![GameEvent::SelectionChanged(selection)]
    }}
}}

fn validate_state(
    universe: &UniverseRepository,
    state: &GameState,
) -> Result<(), SimulationBuildError> {{
    if state.version != GAME_STATE_VERSION {{
        return Err(SimulationBuildError::UnsupportedStateVersion {{
            expected: GAME_STATE_VERSION,
            found: state.version,
        }});
    }}

    for system_id in &state.known_systems {{
        if universe.system(*system_id).is_none() {{
            return Err(SimulationBuildError::UnknownKnownSystem(*system_id));
        }}
    }}

    let mut colony_ids = HashSet::with_capacity(state.colonies.len());
    for colony in &state.colonies {{
        if !colony_ids.insert(colony.id) {{
            return Err(SimulationBuildError::DuplicateColony(colony.id));
        }}
        if universe.system(colony.system_id).is_none() {{
            return Err(SimulationBuildError::UnknownColonySystem {{
                colony_id: colony.id,
                system_id: colony.system_id,
            }});
        }}
        let Some((planet_system_id, _)) = universe.planet_location(colony.planet_id) else {{
            return Err(SimulationBuildError::UnknownColonyPlanet {{
                colony_id: colony.id,
                planet_id: colony.planet_id,
            }});
        }};
        if planet_system_id != colony.system_id {{
            return Err(SimulationBuildError::ColonyPlanetSystemMismatch {{
                colony_id: colony.id,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
            }});
        }}
    }}

    match state.selected {{
        SelectionTarget::None => {{}}
        SelectionTarget::System(system_id) => {{
            if universe.system(system_id).is_none() {{
                return Err(SimulationBuildError::InvalidSelectedSystem(system_id));
            }}
        }}
        SelectionTarget::Planet {{
            system_id,
            planet_id,
        }} => {{
            let Some((planet_system_id, _)) = universe.planet_location(planet_id) else {{
                return Err(SimulationBuildError::InvalidSelectedPlanet {{
                    system_id,
                    planet_id,
                }});
            }};
            if planet_system_id != system_id {{
                return Err(SimulationBuildError::InvalidSelectedPlanet {{
                    system_id,
                    planet_id,
                }});
            }}
        }}
    }}

    Ok(())
}}

#[cfg(test)]
mod tests {{
    use galactic_domain::{{PlanetId, ResourceStock, SystemId, UniverseConfig}};

    use super::*;

    #[test]
    fn simulation_advances_without_renderer() {{
        let mut simulation = Simulation::new(UniverseConfig::default());

        let events = simulation.tick(2.5);

        assert_eq!(simulation.state().elapsed_seconds, 2.5);
        assert_eq!(
            events,
            vec![GameEvent::TickAdvanced {{
                delta_seconds: 2.5,
                elapsed_seconds: 2.5,
            }}]
        );
    }}

    #[test]
    fn pause_blocks_time_progression() {{
        let mut simulation = Simulation::new(UniverseConfig::default());

        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::Paused));
        let events = simulation.tick(10.0);

        assert!(events.is_empty());
        assert_eq!(simulation.state().elapsed_seconds, 0.0);
    }}

    #[test]
    fn selection_events_use_domain_ids() {{
        let mut simulation = Simulation::new(UniverseConfig::default());
        let system_id = SystemId::from_index(0);
        let planet_id = PlanetId::from_system_index(system_id, 0);

        let events = simulation.apply_command(GameCommand::SelectPlanet {{
            system_id,
            planet_id,
        }});

        assert_eq!(
            events,
            vec![GameEvent::SelectionChanged(SelectionTarget::Planet {{
                system_id,
                planet_id,
            }})]
        );
    }}

    #[test]
    fn invalid_selection_is_ignored() {{
        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));

        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(
            simulation.state().selected,
            SelectionTarget::System(SystemId::from_index(0))
        );
    }}

    #[test]
    fn mutable_actions_do_not_change_generated_universe() {{
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let initial_universe = simulation.universe().clone();

        simulation.tick(42.0);
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));
        simulation
            .state_mut()
            .colony_mut(ColonyId::new(0))
            .expect("home colony exists")
            .stock = ResourceStock::new(999, 888, 777, 666);

        assert_eq!(simulation.universe(), &initial_universe);
        assert_ne!(
            simulation
                .state()
                .colony(ColonyId::new(0))
                .expect("home colony exists")
                .stock,
            ResourceStock::new(120, 45, 80, 30)
        );
    }}

    #[test]
    fn visual_world_inputs_are_available_as_definition_plus_state() {{
        let simulation = Simulation::new(UniverseConfig::mvp());

        assert!(!simulation.universe().systems.is_empty());
        assert!(!simulation.state().known_systems.is_empty());
        assert!(simulation.state().colony(ColonyId::new(0)).is_some());
    }}
}}
''')

PERSISTENCE_RS = _rust(r'''// __MVP004_MARKER__
use galactic_domain::{{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseId,
    generate_universe,
}};
use galactic_sim::{{
    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, Simulation,
    SimulationBuildError, TimeSpeed,
}};

pub const SAVE_VERSION: u32 = 2;

/// Persistence envelope: generated data is referenced, not duplicated.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {{
    pub version: u32,
    pub universe: UniverseReference,
    pub state: MutableGameSave,
}}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniverseReference {{
    pub id: UniverseId,
    pub seed: u64,
    pub system_count: usize,
    pub generation_version: u32,
    pub generation_fingerprint: u64,
}}

#[derive(Debug, Clone, PartialEq)]
pub struct MutableGameSave {{
    pub version: u32,
    pub player_faction: FactionId,
    pub elapsed_seconds: f32,
    pub speed: TimeSpeed,
    pub selected: SelectionTarget,
    pub known_systems: Vec<SystemId>,
    pub colonies: Vec<ColonySave>,
}}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonySave {{
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
}}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {{
    UnsupportedVersion(u32),
    UniverseIdMismatch {{ expected: UniverseId, found: UniverseId }},
    GenerationVersionMismatch {{ expected: u32, found: u32 }},
    GenerationFingerprintMismatch {{ expected: u64, found: u64 }},
    InvalidState(SimulationBuildError),
}}

pub fn snapshot_from_simulation(simulation: &Simulation) -> SaveGame {{
    let universe = simulation.universe();
    let state = simulation.state();

    SaveGame {{
        version: SAVE_VERSION,
        universe: UniverseReference {{
            id: universe.id,
            seed: universe.seed,
            system_count: universe.systems.len(),
            generation_version: universe.generation_version,
            generation_fingerprint: universe.generation_fingerprint,
        }},
        state: MutableGameSave {{
            version: state.version,
            player_faction: state.player_faction,
            elapsed_seconds: state.elapsed_seconds,
            speed: state.speed,
            selected: state.selected,
            known_systems: state.known_systems.clone(),
            colonies: state
                .colonies
                .iter()
                .map(|colony| ColonySave {{
                    id: colony.id,
                    name: colony.name.clone(),
                    faction: colony.faction,
                    system_id: colony.system_id,
                    planet_id: colony.planet_id,
                    stock: colony.stock,
                }})
                .collect(),
        }},
    }}
}}

pub fn restore_from_snapshot(save: &SaveGame) -> Result<Simulation, SaveError> {{
    if save.version != SAVE_VERSION {{
        return Err(SaveError::UnsupportedVersion(save.version));
    }}

    let universe = generate_universe(UniverseConfig::new(
        save.universe.seed,
        save.universe.system_count,
    ));

    if universe.id != save.universe.id {{
        return Err(SaveError::UniverseIdMismatch {{
            expected: universe.id,
            found: save.universe.id,
        }});
    }}
    if universe.generation_version != save.universe.generation_version {{
        return Err(SaveError::GenerationVersionMismatch {{
            expected: universe.generation_version,
            found: save.universe.generation_version,
        }});
    }}
    if universe.generation_fingerprint != save.universe.generation_fingerprint {{
        return Err(SaveError::GenerationFingerprintMismatch {{
            expected: universe.generation_fingerprint,
            found: save.universe.generation_fingerprint,
        }});
    }}

    let state = GameState {{
        version: save.state.version,
        player_faction: save.state.player_faction,
        colonies: save
            .state
            .colonies
            .iter()
            .map(|colony| ColonyState {{
                id: colony.id,
                name: colony.name.clone(),
                faction: colony.faction,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
                stock: colony.stock,
            }})
            .collect(),
        known_systems: save.state.known_systems.clone(),
        selected: save.state.selected,
        elapsed_seconds: save.state.elapsed_seconds,
        speed: save.state.speed,
    }};

    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)
}}

#[cfg(test)]
mod tests {{
    use galactic_domain::UniverseConfig;
    use galactic_sim::{{GameCommand, TimeSpeed}};

    use super::*;

    #[test]
    fn snapshot_round_trips_mutable_state_and_regenerates_universe() {{
        let mut simulation = Simulation::new(UniverseConfig::new(99, 14));
        simulation.tick(12.0);
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));

        let original_fingerprint = simulation.universe().generation_fingerprint;
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("save is compatible");

        assert_eq!(
            restored.universe().generation_fingerprint,
            original_fingerprint
        );
        assert_eq!(restored.state(), simulation.state());
    }}

    #[test]
    fn snapshot_contains_a_universe_reference_not_generated_objects() {{
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.universe.system_count, simulation.universe().systems.len());
        assert_eq!(
            save.universe.generation_fingerprint,
            simulation.universe().generation_fingerprint
        );
        assert_eq!(save.state.version, GAME_STATE_VERSION);
    }}

    #[test]
    fn modified_fingerprint_is_rejected() {{
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.universe.generation_fingerprint ^= 1;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::GenerationFingerprintMismatch {{ .. }})
        ));
    }}

    #[test]
    fn unsupported_save_version_is_rejected() {{
        let simulation = Simulation::new(UniverseConfig::default());
        let mut save = snapshot_from_simulation(&simulation);
        save.version = 999;

        assert_eq!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        );
    }}
}}
''')

DOC_SECTION = '''

## MVP-004 — Univers immuable et état mutable

Le moteur distingue désormais explicitement deux sources de données :

```text
UniverseDefinition (seed, systèmes, étoiles, planètes, routes)
        │ immuable et régénérable
        ▼
UniverseRepository (index SystemId / PlanetId en lecture seule)

GameState (temps, sélection, découvertes, colonies, stocks)
        │ mutable et sauvegardé
        ▼
Simulation = UniverseRepository + GameState
```

Règles :

- `GameState` ne contient plus `UniverseDefinition`.
- `Simulation::universe()` ne retourne qu'une référence immuable.
- `UniverseRepository` fournit les accès par `SystemId` et `PlanetId`.
- `GameState::colony()` fournit l'accès à une colonie par `ColonyId`.
- les commandes et ticks modifient uniquement `GameState` ;
- une sauvegarde contient une `UniverseReference` (seed, version, fingerprint)
  et un `MutableGameSave`, jamais une copie des systèmes et planètes ;
- la restauration régénère l'univers, vérifie son fingerprint, puis injecte
  l'état mutable validé.

Version de contrat mutable actuelle : `GAME_STATE_VERSION = 1`.
Version d'enveloppe de sauvegarde actuelle : `SAVE_VERSION = 2`.
'''


def find_root(start: Path) -> Path:
    for candidate in (start, *start.parents):
        if (
            (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_domain/src/world.rs").exists()
            and (candidate / "crates/galactic_sim/src/state.rs").exists()
            and (candidate / "crates/galactic_client/src/lib.rs").exists()
        ):
            return candidate
    raise SystemExit(
        "Repository root not found. Run from Galactic or pass --root /path/to/galactic."
    )


def run(command: list[str], cwd: Path, *, check: bool = True) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if check and result.returncode != 0:
        raise SystemExit(f"Command failed ({result.returncode}): {' '.join(command)}")
    return result


def git_head(root: Path) -> str | None:
    result = run(["git", "rev-parse", "HEAD"], root, check=False)
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def normalize(content: str) -> str:
    return content.rstrip() + "\n"


def diff_text(path: Path, before: str, after: str) -> str:
    return "".join(
        difflib.unified_diff(
            before.splitlines(keepends=True),
            after.splitlines(keepends=True),
            fromfile=str(path),
            tofile=str(path),
        )
    )


def write_file(
    root: Path,
    path: Path,
    content: str,
    *,
    dry_run: bool,
    backup_root: Path | None,
) -> bool:
    before = path.read_text(encoding="utf-8") if path.exists() else ""
    after = normalize(content)
    if before == after:
        print(f"= unchanged: {path.relative_to(root)}")
        return False

    if dry_run:
        print(diff_text(path.relative_to(root), before, after))
        return True

    if path.exists() and backup_root is not None:
        backup = backup_root / path.relative_to(root)
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, backup)

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(after, encoding="utf-8")
    print(f"+ updated: {path.relative_to(root)}")
    return True


def replace_once(source: str, old: str, new: str, label: str) -> str:
    if new in source:
        return source
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"Cannot safely patch {label}: expected one baseline block, found {count}."
        )
    return source.replace(old, new, 1)


def patch_client(source: str) -> str:
    source = source.replace(
        "simulation.simulation().state().universe.systems",
        "simulation.simulation().universe().systems",
    )

    source = replace_once(
        source,
        """    let state = simulation.simulation().state();
    let selected = selection_label(state.selected);""",
        """    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let state = simulation.state();
    let selected = selection_label(state.selected);""",
        "client update_ui state binding",
    )

    for old, new in (
        ("state.universe.seed", "universe.seed"),
        ("state.universe.generation_version", "universe.generation_version"),
        ("state.universe.generation_fingerprint", "universe.generation_fingerprint"),
        ("state.universe.systems.len()", "universe.systems.len()"),
        ("state.universe.routes.len()", "universe.routes.len()"),
    ):
        source = source.replace(old, new)

    source = source.replace(
        "let universe = &simulation.simulation().state().universe;",
        "let universe = simulation.simulation().universe();",
    )

    source = source.replace(
        "business state lives outside Bevy views",
        "immutable universe + mutable state live outside Bevy views",
    )
    return source


def patch_docs(source: str) -> str:
    if "## MVP-004 — Univers immuable et état mutable" in source:
        return source
    return source.rstrip() + DOC_SECTION


def baseline_checks(root: Path) -> None:
    state = (root / "crates/galactic_sim/src/state.rs").read_text(encoding="utf-8")
    world = (root / "crates/galactic_domain/src/world.rs").read_text(encoding="utf-8")

    if MVP004_MARKER in state:
        return
    required = (
        "pub struct GameState",
        "pub universe: UniverseDefinition",
        "pub const MVP_UNIVERSE_SEED: u64 = 42;",
        "pub const GENERATION_VERSION: u32 = 1;",
    )
    combined = state + "\n" + world
    missing = [item for item in required if item not in combined]
    if missing:
        raise SystemExit(
            "Repository does not match the MVP-003 baseline. Missing markers: "
            + ", ".join(missing)
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    baseline_checks(root)

    head = git_head(root)
    already_applied = MVP004_MARKER in (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")
    if head and head != EXPECTED_HEAD and not args.force and not already_applied:
        raise SystemExit(
            "Unexpected Git HEAD.\n"
            f"Expected: {EXPECTED_HEAD}\n"
            f"Found:    {head}\n"
            "Review the repository or rerun with --force after checking the dry-run."
        )

    if not args.dry_run:
        status = run(["git", "status", "--porcelain"], root, check=False)
        if status.returncode == 0 and status.stdout.strip():
            print(
                "WARNING: the working tree already contains changes. "
                "Backups will be created before writes.",
                file=sys.stderr,
            )

    backup_root = None
    if not args.dry_run:
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        backup_root = root / ".mvp004-backup" / stamp

    client_path = root / "crates/galactic_client/src/lib.rs"
    docs_path = root / "docs/mvp_architecture.md"

    targets = {
        root / "crates/galactic_sim/src/lib.rs": SIM_LIB_RS,
        root / "crates/galactic_sim/src/universe.rs": UNIVERSE_RS,
        root / "crates/galactic_sim/src/state.rs": STATE_RS,
        root / "crates/galactic_sim/src/simulation.rs": SIMULATION_RS,
        root / "crates/galactic_persistence/src/lib.rs": PERSISTENCE_RS,
        client_path: patch_client(client_path.read_text(encoding="utf-8")),
        docs_path: patch_docs(docs_path.read_text(encoding="utf-8")),
    }

    changed = 0
    for path, content in targets.items():
        changed += int(
            write_file(
                root,
                path,
                content,
                dry_run=args.dry_run,
                backup_root=backup_root,
            )
        )

    if args.dry_run:
        print(f"\nDry-run complete: {changed} file(s) would change.")
        return 0

    if changed and backup_root is not None:
        print(f"Backup directory: {backup_root}")

    if not args.skip_checks:
        run(["cargo", "fmt", "--all"], root)
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
            root,
        )
        run(["cargo", "test", "--workspace"], root)
    else:
        print(
            "\nChecks skipped. Run manually:\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace"
        )

    print(
        "\nMVP-004 applied. Review with `git diff`, then run "
        "`cargo run --release`."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

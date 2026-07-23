#!/usr/bin/env python3
"""
Applique MVP-008 au dépôt Galactic.

Baseline analysée :
    60c4f42e3980e81a58b531098b691285b56d30c4
    feat mvp 7

Le script :
- ajoute une configuration de nouvelle partie indépendante de la génération ;
- crée explicitement la faction joueur et la colonie initiale ;
- ajoute bâtiments de départ et profil de ressources planétaire ;
- ne marque comme connu que le système natal ;
- démarre avec la planète mère sélectionnée dans la vue Système ;
- sauvegarde et restaure les nouvelles données ;
- met à jour la documentation et les tests.

Usage :
    python tools/apply_mvp_008.py --dry-run
    python tools/apply_mvp_008.py
    python tools/apply_mvp_008.py --skip-checks
    python tools/apply_mvp_008.py --root /chemin/vers/galactic

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

EXPECTED_BASELINE_COMMIT = (
    "60c4f42e3980e81a58b531098b691285b56d30c4"
)

STARTING_RS = '// MVP-008: configurable starting scenario, independent from universe generation\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, SystemId,\n};\n\nuse crate::UniverseRepository;\n\npub const MVP_HOME_SYSTEM_ID: SystemId = SystemId::from_index(0);\npub const MVP_HOME_PLANET_ID: PlanetId =\n    PlanetId::from_system_index(MVP_HOME_SYSTEM_ID, 0);\npub const MVP_PLAYER_FACTION_ID: FactionId = FactionId::new(0);\npub const MVP_HOME_COLONY_ID: ColonyId = ColonyId::new(0);\npub const MVP_MIN_HOME_HABITABILITY: u8 = 80;\n\npub const MVP_INITIAL_KNOWN_SYSTEMS: [SystemId; 1] =\n    [MVP_HOME_SYSTEM_ID];\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum BuildingKind {\n    MetalMine,\n    CrystalExtractor,\n    FuelRefinery,\n    PowerPlant,\n    Warehouse,\n    ConstructionCenter,\n    ResearchLab,\n    Shipyard,\n}\n\nimpl BuildingKind {\n    pub const ALL: [Self; 8] = [\n        Self::MetalMine,\n        Self::CrystalExtractor,\n        Self::FuelRefinery,\n        Self::PowerPlant,\n        Self::Warehouse,\n        Self::ConstructionCenter,\n        Self::ResearchLab,\n        Self::Shipyard,\n    ];\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct BuildingLevels {\n    pub metal_mine: u8,\n    pub crystal_extractor: u8,\n    pub fuel_refinery: u8,\n    pub power_plant: u8,\n    pub warehouse: u8,\n    pub construction_center: u8,\n    pub research_lab: u8,\n    pub shipyard: u8,\n}\n\nimpl BuildingLevels {\n    pub const EMPTY: Self = Self {\n        metal_mine: 0,\n        crystal_extractor: 0,\n        fuel_refinery: 0,\n        power_plant: 0,\n        warehouse: 0,\n        construction_center: 0,\n        research_lab: 0,\n        shipyard: 0,\n    };\n\n    pub const MVP_START: Self = Self {\n        metal_mine: 1,\n        crystal_extractor: 1,\n        fuel_refinery: 1,\n        power_plant: 1,\n        warehouse: 1,\n        construction_center: 1,\n        research_lab: 0,\n        shipyard: 0,\n    };\n\n    pub const fn level(self, kind: BuildingKind) -> u8 {\n        match kind {\n            BuildingKind::MetalMine => self.metal_mine,\n            BuildingKind::CrystalExtractor => self.crystal_extractor,\n            BuildingKind::FuelRefinery => self.fuel_refinery,\n            BuildingKind::PowerPlant => self.power_plant,\n            BuildingKind::Warehouse => self.warehouse,\n            BuildingKind::ConstructionCenter => {\n                self.construction_center\n            }\n            BuildingKind::ResearchLab => self.research_lab,\n            BuildingKind::Shipyard => self.shipyard,\n        }\n    }\n\n    pub fn total_levels(self) -> u32 {\n        BuildingKind::ALL\n            .into_iter()\n            .map(|kind| u32::from(self.level(kind)))\n            .sum()\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct PlanetResourceProfile {\n    /// Relative production potential, where 100 is the balanced baseline.\n    pub metal: u16,\n    pub crystal: u16,\n    pub fuel: u16,\n    pub energy: u16,\n}\n\nimpl PlanetResourceProfile {\n    pub const BALANCED: Self = Self::new(100, 100, 100, 100);\n\n    pub const fn new(\n        metal: u16,\n        crystal: u16,\n        fuel: u16,\n        energy: u16,\n    ) -> Self {\n        Self {\n            metal,\n            crystal,\n            fuel,\n            energy,\n        }\n    }\n\n    pub const fn is_viable(self) -> bool {\n        self.metal > 0\n            && self.crystal > 0\n            && self.fuel > 0\n            && self.energy > 0\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StartingFactionConfig {\n    pub id: FactionId,\n    pub name: &\'static str,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StartingColonyConfig {\n    pub id: ColonyId,\n    pub name: &\'static str,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub initial_stock: ResourceStock,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StartingScenario {\n    pub player_faction: StartingFactionConfig,\n    pub home_colony: StartingColonyConfig,\n    pub initially_known_systems: &\'static [SystemId],\n    pub minimum_home_habitability: u8,\n}\n\nimpl StartingScenario {\n    pub const fn mvp() -> Self {\n        Self {\n            player_faction: StartingFactionConfig {\n                id: MVP_PLAYER_FACTION_ID,\n                name: "Aster Expedition",\n            },\n            home_colony: StartingColonyConfig {\n                id: MVP_HOME_COLONY_ID,\n                name: "Aster Prime Colony",\n                system_id: MVP_HOME_SYSTEM_ID,\n                planet_id: MVP_HOME_PLANET_ID,\n                initial_stock: ResourceStock::new(600, 300, 220, 80),\n                buildings: BuildingLevels::MVP_START,\n                resource_profile: PlanetResourceProfile::BALANCED,\n            },\n            initially_known_systems: &MVP_INITIAL_KNOWN_SYSTEMS,\n            minimum_home_habitability: MVP_MIN_HOME_HABITABILITY,\n        }\n    }\n\n    pub fn validate(\n        self,\n        universe: &UniverseRepository,\n    ) -> Result<(), StartingScenarioError> {\n        if self.player_faction.name.trim().is_empty() {\n            return Err(StartingScenarioError::EmptyFactionName);\n        }\n        if self.home_colony.name.trim().is_empty() {\n            return Err(StartingScenarioError::EmptyColonyName);\n        }\n        if !self.home_colony.resource_profile.is_viable() {\n            return Err(StartingScenarioError::InvalidResourceProfile);\n        }\n\n        let Some(system) = universe.system(self.home_colony.system_id)\n        else {\n            return Err(StartingScenarioError::UnknownHomeSystem(\n                self.home_colony.system_id,\n            ));\n        };\n        let Some(planet) = universe.planet(self.home_colony.planet_id)\n        else {\n            return Err(StartingScenarioError::UnknownHomePlanet(\n                self.home_colony.planet_id,\n            ));\n        };\n        if planet.id.system_id() != system.id {\n            return Err(\n                StartingScenarioError::HomePlanetSystemMismatch {\n                    system_id: system.id,\n                    planet_id: planet.id,\n                },\n            );\n        }\n        if planet.habitability < self.minimum_home_habitability {\n            return Err(\n                StartingScenarioError::InsufficientHabitability {\n                    required: self.minimum_home_habitability,\n                    found: planet.habitability,\n                },\n            );\n        }\n\n        for system_id in self.initially_known_systems {\n            if universe.system(*system_id).is_none() {\n                return Err(\n                    StartingScenarioError::UnknownInitiallyKnownSystem(\n                        *system_id,\n                    ),\n                );\n            }\n        }\n        if !self\n            .initially_known_systems\n            .contains(&self.home_colony.system_id)\n        {\n            return Err(\n                StartingScenarioError::HomeSystemNotInitiallyKnown,\n            );\n        }\n\n        Ok(())\n    }\n}\n\nimpl Default for StartingScenario {\n    fn default() -> Self {\n        Self::mvp()\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum StartingScenarioError {\n    EmptyFactionName,\n    EmptyColonyName,\n    InvalidResourceProfile,\n    UnknownHomeSystem(SystemId),\n    UnknownHomePlanet(PlanetId),\n    HomePlanetSystemMismatch {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n    InsufficientHabitability {\n        required: u8,\n        found: u8,\n    },\n    UnknownInitiallyKnownSystem(SystemId),\n    HomeSystemNotInitiallyKnown,\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::UniverseConfig;\n\n    use super::*;\n\n    #[test]\n    fn mvp_starting_scenario_matches_reference_universe() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n\n        assert_eq!(\n            StartingScenario::mvp().validate(&universe),\n            Ok(())\n        );\n    }\n\n    #[test]\n    fn starting_data_is_configurable_without_mutating_universe() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n        let fingerprint =\n            universe.definition().generation_fingerprint;\n        let mut scenario = StartingScenario::mvp();\n        scenario.home_colony.initial_stock =\n            ResourceStock::new(999, 888, 777, 66);\n        scenario.home_colony.buildings.research_lab = 1;\n\n        assert_eq!(scenario.validate(&universe), Ok(()));\n        assert_eq!(\n            universe.definition().generation_fingerprint,\n            fingerprint\n        );\n    }\n}\n'
STATE_RS = '// MVP-008: configurable player faction, home world and starting colony\nuse std::collections::BTreeSet;\n\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId,\n};\n\nuse crate::{\n    BuildingLevels, PlanetResourceProfile, SelectionTarget,\n    StartingScenario, StartingScenarioError, StrategicClock,\n    UniverseRepository,\n};\n\n/// Version of the mutable in-memory state contract.\n///\n/// Version 3 adds factions and configurable colony foundation data.\npub const GAME_STATE_VERSION: u32 = 3;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SystemVisibility {\n    Known,\n    Detected,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum FactionKind {\n    Player,\n    Neutral,\n    FutureAi,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionState {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct GameState {\n    pub version: u32,\n    pub factions: Vec<FactionState>,\n    pub player_faction: FactionId,\n    pub colonies: Vec<ColonyState>,\n    pub known_systems: Vec<SystemId>,\n    pub selected: SelectionTarget,\n    pub clock: StrategicClock,\n}\n\nimpl GameState {\n    pub fn new(universe: &UniverseRepository) -> Self {\n        Self::from_starting_scenario(\n            universe,\n            StartingScenario::mvp(),\n        )\n        .expect(\n            "the MVP starting scenario must match the reference universe",\n        )\n    }\n\n    pub fn from_starting_scenario(\n        universe: &UniverseRepository,\n        scenario: StartingScenario,\n    ) -> Result<Self, StartingScenarioError> {\n        scenario.validate(universe)?;\n\n        let mut known_systems =\n            scenario.initially_known_systems.to_vec();\n        known_systems.sort();\n        known_systems.dedup();\n\n        let player_faction = scenario.player_faction.id;\n        let home = scenario.home_colony;\n\n        Ok(Self {\n            version: GAME_STATE_VERSION,\n            factions: vec![FactionState {\n                id: player_faction,\n                name: scenario.player_faction.name.to_string(),\n                kind: FactionKind::Player,\n            }],\n            player_faction,\n            colonies: vec![ColonyState {\n                id: home.id,\n                name: home.name.to_string(),\n                faction: player_faction,\n                system_id: home.system_id,\n                planet_id: home.planet_id,\n                stock: home.initial_stock,\n                buildings: home.buildings,\n                resource_profile: home.resource_profile,\n            }],\n            known_systems,\n            selected: SelectionTarget::Planet {\n                system_id: home.system_id,\n                planet_id: home.planet_id,\n            },\n            clock: StrategicClock::new(),\n        })\n    }\n\n    pub fn faction(&self, id: FactionId) -> Option<&FactionState> {\n        self.factions.iter().find(|faction| faction.id == id)\n    }\n\n    pub fn player_faction_state(&self) -> Option<&FactionState> {\n        self.faction(self.player_faction)\n    }\n\n    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {\n        self.colonies.iter().find(|colony| colony.id == id)\n    }\n\n    pub fn colony_mut(\n        &mut self,\n        id: ColonyId,\n    ) -> Option<&mut ColonyState> {\n        self.colonies\n            .iter_mut()\n            .find(|colony| colony.id == id)\n    }\n\n    pub fn colony_on_planet(\n        &self,\n        planet_id: PlanetId,\n    ) -> Option<&ColonyState> {\n        self.colonies\n            .iter()\n            .find(|colony| colony.planet_id == planet_id)\n    }\n\n    pub fn player_home_colony(&self) -> Option<&ColonyState> {\n        self.colonies\n            .iter()\n            .find(|colony| colony.faction == self.player_faction)\n    }\n\n    pub fn is_system_known(&self, system_id: SystemId) -> bool {\n        self.known_systems.contains(&system_id)\n    }\n\n    /// Systems directly adjacent to known systems form the current detection\n    /// frontier. MVP-009 will replace this with persisted knowledge levels.\n    pub fn detected_systems(\n        &self,\n        universe: &UniverseRepository,\n    ) -> Vec<SystemId> {\n        let mut detected = BTreeSet::new();\n\n        for known_system in &self.known_systems {\n            for neighbor in\n                universe.neighboring_systems(*known_system)\n            {\n                if !self.is_system_known(neighbor) {\n                    detected.insert(neighbor);\n                }\n            }\n        }\n\n        detected.into_iter().collect()\n    }\n\n    pub fn system_visibility(\n        &self,\n        universe: &UniverseRepository,\n        system_id: SystemId,\n    ) -> Option<SystemVisibility> {\n        if self.is_system_known(system_id) {\n            return Some(SystemVisibility::Known);\n        }\n\n        self.detected_systems(universe)\n            .binary_search(&system_id)\n            .ok()\n            .map(|_| SystemVisibility::Detected)\n    }\n\n    pub fn visible_systems(\n        &self,\n        universe: &UniverseRepository,\n    ) -> Vec<(SystemId, SystemVisibility)> {\n        let mut systems = self\n            .known_systems\n            .iter()\n            .copied()\n            .map(|system_id| {\n                (system_id, SystemVisibility::Known)\n            })\n            .collect::<Vec<_>>();\n\n        systems.extend(\n            self.detected_systems(universe)\n                .into_iter()\n                .map(|system_id| {\n                    (system_id, SystemVisibility::Detected)\n                }),\n        );\n        systems.sort_by_key(|(system_id, _)| *system_id);\n        systems\n    }\n\n    pub fn is_system_visible(\n        &self,\n        universe: &UniverseRepository,\n        system_id: SystemId,\n    ) -> bool {\n        self.system_visibility(universe, system_id).is_some()\n    }\n\n    pub fn visible_routes<\'a>(\n        &self,\n        universe: &\'a UniverseRepository,\n    ) -> Vec<&\'a Route> {\n        universe\n            .definition()\n            .routes\n            .iter()\n            .filter(|route| {\n                let from =\n                    self.system_visibility(universe, route.from);\n                let to =\n                    self.system_visibility(universe, route.to);\n\n                (from == Some(SystemVisibility::Known)\n                    && to.is_some())\n                    || (from == Some(SystemVisibility::Detected)\n                        && to == Some(SystemVisibility::Known))\n            })\n            .collect()\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonyState {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{ColonyId, UniverseConfig};\n\n    use super::*;\n\n    #[test]\n    fn new_game_uses_stable_home_world_and_player_faction() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n        let scenario = StartingScenario::mvp();\n        let colony = state\n            .colony(ColonyId::new(0))\n            .expect("home colony exists");\n\n        assert_eq!(\n            state.player_faction,\n            scenario.player_faction.id\n        );\n        assert_eq!(state.factions.len(), 1);\n        assert_eq!(\n            state.player_faction_state()\n                .expect("player faction exists")\n                .kind,\n            FactionKind::Player\n        );\n        assert_eq!(\n            colony.system_id,\n            scenario.home_colony.system_id\n        );\n        assert_eq!(\n            colony.planet_id,\n            scenario.home_colony.planet_id\n        );\n    }\n\n    #[test]\n    fn home_planet_supports_the_starting_loop() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n        let colony =\n            state.player_home_colony().expect("home colony exists");\n        let planet = universe\n            .planet(colony.planet_id)\n            .expect("home planet exists");\n\n        assert!(\n            planet.habitability\n                >= StartingScenario::mvp()\n                    .minimum_home_habitability\n        );\n        assert!(\n            colony\n                .stock\n                .can_cover(ResourceStock::new(100, 50, 25, 0))\n        );\n        assert!(colony.resource_profile.is_viable());\n        assert!(\n            colony.buildings.total_levels() >= 6,\n            "the colony starts with the six foundation buildings"\n        );\n    }\n\n    #[test]\n    fn only_home_system_is_known_at_start() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n        let home =\n            StartingScenario::mvp().home_colony.system_id;\n\n        assert_eq!(state.known_systems, vec![home]);\n        assert!(!state.detected_systems(&universe).is_empty());\n        assert!(state\n            .detected_systems(&universe)\n            .iter()\n            .all(|system_id| {\n                universe.route_exists(home, *system_id)\n            }));\n    }\n\n    #[test]\n    fn custom_starting_data_does_not_change_generated_universe() {\n        let universe =\n            UniverseRepository::generate(UniverseConfig::mvp());\n        let fingerprint =\n            universe.definition().generation_fingerprint;\n        let mut scenario = StartingScenario::mvp();\n        scenario.home_colony.initial_stock =\n            ResourceStock::new(900, 700, 500, 100);\n        scenario.home_colony.buildings.research_lab = 1;\n\n        let state =\n            GameState::from_starting_scenario(&universe, scenario)\n                .expect("custom starting scenario is valid");\n        let colony =\n            state.player_home_colony().expect("home colony exists");\n\n        assert_eq!(\n            colony.stock,\n            ResourceStock::new(900, 700, 500, 100)\n        );\n        assert_eq!(colony.buildings.research_lab, 1);\n        assert_eq!(\n            universe.definition().generation_fingerprint,\n            fingerprint\n        );\n    }\n}\n'
PERSISTENCE_RS = '// MVP-008: save the player faction and configurable colony foundation\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, SystemId,\n    UniverseConfig, UniverseId, generate_universe,\n};\nuse galactic_sim::{\n    BuildingLevels, ColonyState, FactionKind, FactionState,\n    GameState, PlanetResourceProfile, SelectionTarget,\n    Simulation, SimulationBuildError, StrategicClock,\n    StrategicClockError, StrategicTick, TimeSpeed,\n};\n\npub const SAVE_VERSION: u32 = 4;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub factions: Vec<FactionSave>,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub known_systems: Vec<SystemId>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionSave {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(\n    simulation: &Simulation,\n) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint:\n                universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            factions: state\n                .factions\n                .iter()\n                .map(|faction| FactionSave {\n                    id: faction.id,\n                    name: faction.name.clone(),\n                    kind: faction.kind,\n                })\n                .collect(),\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos: state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            known_systems: state.known_systems.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.stock,\n                    buildings: colony.buildings,\n                    resource_profile: colony.resource_profile,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(\n    save: &SaveGame,\n) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(SaveError::UnsupportedVersion(save.version));\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version\n        != save.universe.generation_version\n    {\n        return Err(SaveError::GenerationVersionMismatch {\n            expected: universe.generation_version,\n            found: save.universe.generation_version,\n        });\n    }\n    if universe.generation_fingerprint\n        != save.universe.generation_fingerprint\n    {\n        return Err(\n            SaveError::GenerationFingerprintMismatch {\n                expected: universe.generation_fingerprint,\n                found: save.universe.generation_fingerprint,\n            },\n        );\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let state = GameState {\n        version: save.state.version,\n        factions: save\n            .state\n            .factions\n            .iter()\n            .map(|faction| FactionState {\n                id: faction.id,\n                name: faction.name.clone(),\n                kind: faction.kind,\n            })\n            .collect(),\n        player_faction: save.state.player_faction,\n        colonies: save\n            .state\n            .colonies\n            .iter()\n            .map(|colony| ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                stock: colony.stock,\n                buildings: colony.buildings,\n                resource_profile: colony.resource_profile,\n            })\n            .collect(),\n        known_systems: save.state.known_systems.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state)\n        .map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::UniverseConfig;\n    use galactic_sim::{\n        GAME_STATE_VERSION, GameCommand, STRATEGIC_TICK_NANOS,\n        StartingScenario, StrategicTick, TimeSpeed,\n    };\n\n    use super::*;\n\n    #[test]\n    fn snapshot_round_trips_complete_starting_state() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::new(99, 14));\n        simulation.advance(Duration::from_millis(125));\n        simulation\n            .apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n\n        let original_fingerprint =\n            simulation.universe().generation_fingerprint;\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("save is compatible");\n\n        assert_eq!(\n            restored.universe().generation_fingerprint,\n            original_fingerprint\n        );\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(\n            restored.state().clock.current_tick(),\n            StrategicTick::new(1)\n        );\n        assert_eq!(\n            restored.state().clock.remainder_nanos(),\n            25_000_000\n        );\n    }\n\n    #[test]\n    fn snapshot_contains_player_faction_and_home_foundation() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let save = snapshot_from_simulation(&simulation);\n        let scenario = StartingScenario::mvp();\n        let colony = save\n            .state\n            .colonies\n            .first()\n            .expect("home colony is saved");\n\n        assert_eq!(save.state.version, GAME_STATE_VERSION);\n        assert_eq!(save.state.factions.len(), 1);\n        assert_eq!(\n            save.state.player_faction,\n            scenario.player_faction.id\n        );\n        assert_eq!(\n            colony.buildings,\n            scenario.home_colony.buildings\n        );\n        assert_eq!(\n            colony.resource_profile,\n            scenario.home_colony.resource_profile\n        );\n        assert_eq!(\n            save.state.known_systems.as_slice(),\n            scenario.initially_known_systems\n        );\n    }\n\n    #[test]\n    fn modified_fingerprint_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.universe.generation_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::GenerationFingerprintMismatch { .. })\n        ));\n    }\n\n    #[test]\n    fn invalid_clock_remainder_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.state.clock.remainder_nanos =\n            STRATEGIC_TICK_NANOS;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidClock(\n                StrategicClockError::RemainderOutOfRange(_)\n            ))\n        ));\n    }\n\n    #[test]\n    fn unsupported_save_version_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::default());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.version = 999;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::UnsupportedVersion(999))\n        ));\n    }\n}\n'
DOC_APPEND = "\n## MVP-008 — Système de départ et planète mère\n\nLes paramètres de nouvelle partie sont maintenant séparés de la génération de\nl'univers :\n\n```text\nUniverseConfig\n    seed / nombre de systèmes\n            │\n            ▼\nUniverseDefinition immuable\n\nStartingScenario\n    faction joueur\n    système et planète de départ\n    colonie et stocks\n    bâtiments initiaux\n    profil de ressources\n    connaissances initiales\n            │\n            ▼\nGameState mutable\n```\n\nConfiguration MVP :\n\n- système natal : `SystemId(0)` ;\n- planète mère : première planète de ce système, `Aster Prime` ;\n- habitabilité minimale validée : 80 ;\n- faction joueur : `Aster Expedition` ;\n- une colonie initiale ;\n- stocks initiaux : 600 métal, 300 cristal, 220 carburant, 80 énergie ;\n- profil planétaire équilibré : 100/100/100/100 ;\n- bâtiments niveau 1 :\n  - mine de métal ;\n  - extracteur de cristal ;\n  - raffinerie de carburant ;\n  - centrale énergétique ;\n  - entrepôt ;\n  - centre de construction ;\n- laboratoire et chantier spatial au niveau 0 ;\n- seul le système natal est connu ;\n- ses voisins apparaissent comme signaux détectés via la frontière MVP-007 ;\n- la sélection initiale vise directement la planète mère ;\n- la vue initiale du client est la vue Système.\n\n`StartingScenario` est configurable sans modifier la seed, la version de\ngénération ou le fingerprint de l'univers.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 3` ;\n- `SAVE_VERSION = 4`.\n"


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
        print(
            result.stdout,
            end="" if result.stdout.endswith("\n") else "\n",
        )
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : "
            f"{' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (
                candidate
                / "crates/galactic_sim/src/state.rs"
            ).exists()
            and (
                candidate
                / "crates/galactic_client/src/lib.rs"
            ).exists()
        ):
            return candidate

    raise SystemExit(
        "Racine Galactic introuvable. Utilise --root."
    )


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def format_rust_source(root: Path, source: str) -> str:
    result = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        cwd=root,
        input=source,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        details = result.stderr or result.stdout
        raise SystemExit(
            "Impossible de formatter la source Rust générée par MVP-008.\n"
            + details
        )
    return normalize(result.stdout)


def verify_baseline(root: Path, force: bool) -> None:
    head = run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
    ).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return

    ancestor = run(
        [
            "git",
            "merge-base",
            "--is-ancestor",
            EXPECTED_BASELINE_COMMIT,
            "HEAD",
        ],
        cwd=root,
        check=False,
    )
    if ancestor.returncode == 0:
        print(
            "Baseline présente dans l'historique ; "
            f"HEAD actuel : {head}"
        )
        return
    if force:
        print(
            "WARNING: baseline différente, poursuite "
            "autorisée par --force."
        )
        return

    raise SystemExit(
        "Le dépôt local ne correspond pas à la baseline "
        "MVP-007 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_mvp7(root: Path) -> None:
    state = (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")
    persistence = (
        root / "crates/galactic_persistence/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    if "SystemVisibility" not in state:
        failures.append("frontière visible MVP-007 absente")
    if "StrategicNavigation" not in client:
        failures.append("navigation stratégique absente")
    if "StrategicClockSave" not in persistence:
        failures.append("sauvegarde MVP-005 absente")

    if failures:
        raise SystemExit(
            "Baseline MVP-007 incohérente :\n- "
            + "\n- ".join(failures)
        )


def patch_sim_lib(source: str) -> str:
    if "pub mod starting;" not in source:
        source = source.replace(
            "pub mod simulation;\n",
            "pub mod simulation;\npub mod starting;\n",
            1,
        )
    if "pub use starting::*;" not in source:
        source = source.replace(
            "pub use simulation::*;\n",
            "pub use simulation::*;\npub use starting::*;\n",
            1,
        )
    return normalize(source)


def patch_simulation(source: str) -> str:
    updated = source

    updated = updated.replace(
        "use galactic_domain::{ColonyId, PlanetId, "
        "SystemId, UniverseConfig, UniverseDefinition};",
        "use galactic_domain::{\n"
        "    ColonyId, FactionId, PlanetId, SystemId, "
        "UniverseConfig,\n"
        "    UniverseDefinition,\n"
        "};",
        1,
    )

    updated = updated.replace(
        "    GAME_STATE_VERSION, GameCommand, GameEvent, "
        "GameState, SelectionTarget, TimeSpeed,\n"
        "    UniverseIndexError, UniverseRepository,\n",
        "    FactionKind, GAME_STATE_VERSION, GameCommand, "
        "GameEvent, GameState,\n"
        "    SelectionTarget, StartingScenario, "
        "StartingScenarioError, TimeSpeed,\n"
        "    UniverseIndexError, UniverseRepository,\n",
        1,
    )

    error_marker = """    DuplicateColony(ColonyId),
    UnknownKnownSystem(SystemId),"""
    error_replacement = """    InvalidStartingScenario(StartingScenarioError),
    DuplicateFaction(FactionId),
    UnknownPlayerFaction(FactionId),
    PlayerFactionIsNotPlayer(FactionId),
    DuplicateColony(ColonyId),
    UnknownColonyFaction {
        colony_id: ColonyId,
        faction_id: FactionId,
    },
    UnknownKnownSystem(SystemId),"""
    if "InvalidStartingScenario" not in updated:
        if error_marker not in updated:
            raise SystemExit(
                "Bloc SimulationBuildError attendu introuvable."
            )
        updated = updated.replace(
            error_marker,
            error_replacement,
            1,
        )

    old_new = """    pub fn new(config: UniverseConfig) -> Self {
        let universe = UniverseRepository::generate(config);
        let state = GameState::new(&universe);
        Self { universe, state }
    }
"""
    new_new = """    pub fn new(config: UniverseConfig) -> Self {
        Self::new_with_scenario(
            config,
            StartingScenario::mvp(),
        )
        .expect(
            "the MVP starting scenario must produce a valid simulation",
        )
    }

    pub fn new_with_scenario(
        config: UniverseConfig,
        scenario: StartingScenario,
    ) -> Result<Self, SimulationBuildError> {
        let universe = UniverseRepository::generate(config);
        let state = GameState::from_starting_scenario(
            &universe,
            scenario,
        )
        .map_err(
            SimulationBuildError::InvalidStartingScenario,
        )?;
        validate_state(&universe, &state)?;
        Ok(Self { universe, state })
    }
"""
    if "pub fn new_with_scenario" not in updated:
        if old_new not in updated:
            raise SystemExit(
                "Constructeur Simulation::new attendu introuvable."
            )
        updated = updated.replace(old_new, new_new, 1)

    validation_marker = """    for system_id in &state.known_systems {
        if universe.system(*system_id).is_none() {
            return Err(SimulationBuildError::UnknownKnownSystem(*system_id));
        }
    }

    let mut colony_ids"""
    validation_replacement = """    let mut faction_ids =
        HashSet::with_capacity(state.factions.len());
    for faction in &state.factions {
        if !faction_ids.insert(faction.id) {
            return Err(
                SimulationBuildError::DuplicateFaction(faction.id),
            );
        }
    }

    let Some(player_faction) =
        state.faction(state.player_faction)
    else {
        return Err(
            SimulationBuildError::UnknownPlayerFaction(
                state.player_faction,
            ),
        );
    };
    if player_faction.kind != FactionKind::Player {
        return Err(
            SimulationBuildError::PlayerFactionIsNotPlayer(
                state.player_faction,
            ),
        );
    }

    for system_id in &state.known_systems {
        if universe.system(*system_id).is_none() {
            return Err(
                SimulationBuildError::UnknownKnownSystem(
                    *system_id,
                ),
            );
        }
    }

    let mut colony_ids"""
    if "let mut faction_ids" not in updated:
        if validation_marker not in updated:
            raise SystemExit(
                "Point de validation des factions introuvable."
            )
        updated = updated.replace(
            validation_marker,
            validation_replacement,
            1,
        )

    colony_marker = """        if !colony_ids.insert(colony.id) {
            return Err(SimulationBuildError::DuplicateColony(colony.id));
        }
        if universe.system(colony.system_id).is_none() {"""
    colony_replacement = """        if !colony_ids.insert(colony.id) {
            return Err(
                SimulationBuildError::DuplicateColony(colony.id),
            );
        }
        if state.faction(colony.faction).is_none() {
            return Err(
                SimulationBuildError::UnknownColonyFaction {
                    colony_id: colony.id,
                    faction_id: colony.faction,
                },
            );
        }
        if universe.system(colony.system_id).is_none() {"""
    if "UnknownColonyFaction" in updated and (
        "state.faction(colony.faction).is_none()" not in updated
    ):
        if colony_marker not in updated:
            raise SystemExit(
                "Validation de colonie attendue introuvable."
            )
        updated = updated.replace(
            colony_marker,
            colony_replacement,
            1,
        )

    if "vec![GameEvent::SelectionChanged(SelectionTarget::None)]" not in updated:
        updated = updated.replace(
            """        let events = simulation.apply_command(GameCommand::SelectPlanet {
            system_id,
            planet_id,
        });""",
            """        assert_eq!(
            simulation.apply_command(GameCommand::ClearSelection),
            vec![GameEvent::SelectionChanged(SelectionTarget::None)]
        );
        let events = simulation.apply_command(GameCommand::SelectPlanet {
            system_id,
            planet_id,
        });""",
            1,
        )
    updated = updated.replace(
        """        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));

        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(
            simulation.state().selected,
            SelectionTarget::System(SystemId::from_index(0))
        );""",
        """        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));
        let initial_selection = simulation.state().selected;

        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(simulation.state().selected, initial_selection);""",
        1,
    )

    return normalize(updated)


def patch_client(source: str) -> str:
    updated = source

    updated = updated.replace(
        "    GameCommand, GameEvent, SelectionTarget, "
        "Simulation, SystemVisibility, TimeSpeed,\n",
        "    GameCommand, GameEvent, MVP_HOME_SYSTEM_ID, "
        "SelectionTarget,\n"
        "    Simulation, SystemVisibility, TimeSpeed,\n",
        1,
    )

    updated = updated.replace(
        "            mode: StrategicViewMode::Universe,",
        "            mode: StrategicViewMode::System("
        "MVP_HOME_SYSTEM_ID),",
        1,
    )

    if "struct HomeSummaryText;" not in updated:
        updated = updated.replace(
            "#[derive(Component)]\nstruct HelpText;",
            "#[derive(Component)]\nstruct HelpText;\n\n"
            "#[derive(Component)]\nstruct HomeSummaryText;",
            1,
        )

    if "                update_home_summary,\n" not in updated:
        updated = updated.replace(
            "                update_ui,\n",
            "                update_ui,\n"
            "                update_home_summary,\n",
            1,
        )

    old_loop = """    for (index, planet) in system.planets.iter().enumerate() {
        let radius = 6.0 + index as f32 * 4.8;
        let angle = index as f32 * 1.37;
        let position = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        let material = assets
            .planet_materials
            .get(&planet.kind)
            .expect("planet material exists")
            .clone();
        let scale = if planet.kind == PlanetKind::GasGiant {
            1.25
        } else {
            0.72
        };

        commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            StrategicViewEntity,
        ));

        commands.spawn((
            Text2d::new(planet.name.clone()),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),
            Transform::from_translation(position + Vec3::new(0.0, 1.35, 0.0))
                .with_scale(Vec3::splat(0.25)),
            StrategicViewEntity,
        ));
    }"""
    new_loop = """    let state = simulation.state();
    for (index, planet) in system.planets.iter().enumerate() {
        let radius = 6.0 + index as f32 * 4.8;
        let angle = index as f32 * 1.37;
        let position = Vec3::new(
            angle.cos() * radius,
            0.0,
            angle.sin() * radius,
        );
        let colonized = state.colony_on_planet(planet.id);
        let material = if colonized.is_some() {
            assets
                .planet_materials
                .get(&planet.kind)
                .expect("planet material exists")
                .clone()
        } else {
            assets.detected_material.clone()
        };
        let scale = if colonized.is_some()
            && planet.kind == PlanetKind::GasGiant
        {
            1.25
        } else {
            0.72
        };
        let label = if let Some(colony) = colonized {
            format!("{} — {}", planet.name, colony.name)
        } else {
            format!("Corps non sondé {}", index + 1)
        };

        commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_scale(Vec3::splat(scale)),
            StrategicViewEntity,
        ));

        commands.spawn((
            Text2d::new(label),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgba(
                0.72, 0.82, 0.92, 0.86,
            )),
            Transform::from_translation(
                position + Vec3::new(0.0, 1.35, 0.0),
            )
            .with_scale(Vec3::splat(0.25)),
            StrategicViewEntity,
        ));
    }"""
    if "Corps non sondé" not in updated:
        if old_loop not in updated:
            raise SystemExit(
                "Boucle de rendu des planètes introuvable."
            )
        updated = updated.replace(old_loop, new_loop, 1)

    summary_spawn = """    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.82, 0.90, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(78.0),
            width: Val::Px(330.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(
            0.014, 0.022, 0.034, 0.78,
        )),
        HomeSummaryText,
    ));

"""
    help_marker = """    commands.spawn((
        Text::new(
            "Space pause"""
    if "HomeSummaryText," not in updated:
        if help_marker not in updated:
            raise SystemExit(
                "Point d'insertion du résumé initial introuvable."
            )
        updated = updated.replace(
            help_marker,
            summary_spawn + help_marker,
            1,
        )

    summary_fn = r"""
fn update_home_summary(
    simulation: Res<SimulationResource>,
    mut query: Query<&mut Text, With<HomeSummaryText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let simulation = simulation.simulation();
    let state = simulation.state();
    let Some(faction) = state.player_faction_state() else {
        text.0 = "Faction joueur invalide".to_string();
        return;
    };
    let Some(colony) = state.player_home_colony() else {
        text.0 = "Colonie mère introuvable".to_string();
        return;
    };
    let Some(system) =
        simulation.universe().system(colony.system_id)
    else {
        return;
    };
    let Some(planet) =
        simulation.universe_repository().planet(colony.planet_id)
    else {
        return;
    };

    text.0 = format!(
        "{}\n{} / {}\nHabitabilité : {}%\n\nStocks\nMétal {}  Cristal {}\nCarburant {}  Énergie {}\n\nPotentiel planète\nM {}  C {}  F {}  E {}\n\nBâtiments\nMines {}/{}/{}  Centrale {}\nEntrepôt {}  Construction {}\nLaboratoire {}  Chantier {}",
        faction.name,
        system.name,
        planet.name,
        planet.habitability,
        colony.stock.metal,
        colony.stock.crystal,
        colony.stock.fuel,
        colony.stock.energy,
        colony.resource_profile.metal,
        colony.resource_profile.crystal,
        colony.resource_profile.fuel,
        colony.resource_profile.energy,
        colony.buildings.metal_mine,
        colony.buildings.crystal_extractor,
        colony.buildings.fuel_refinery,
        colony.buildings.power_plant,
        colony.buildings.warehouse,
        colony.buildings.construction_center,
        colony.buildings.research_lab,
        colony.buildings.shipyard,
    );
}

"""
    if "fn update_home_summary(" not in updated:
        marker = "\nfn to_vec3(position: WorldPosition) -> Vec3 {"
        if marker not in updated:
            raise SystemExit(
                "Point d'insertion update_home_summary introuvable."
            )
        updated = updated.replace(
            marker,
            "\n" + summary_fn + marker,
            1,
        )

    return normalize(updated)


def patch_docs(source: str) -> str:
    if "## MVP-008 — Système de départ et planète mère" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_sim/src/starting.rs":
            format_rust_source(root, STARTING_RS),
        root / "crates/galactic_sim/src/state.rs":
            format_rust_source(root, STATE_RS),
        root / "crates/galactic_persistence/src/lib.rs":
            format_rust_source(root, PERSISTENCE_RS),
    }
    for path, after in replacements.items():
        before = (
            path.read_text(encoding="utf-8")
            if path.exists()
            else ""
        )
        if before != after:
            updates.append(Update(path, before, after))

    lib_path = root / "crates/galactic_sim/src/lib.rs"
    lib_before = lib_path.read_text(encoding="utf-8")
    lib_after = patch_sim_lib(lib_before)
    if lib_before != lib_after:
        updates.append(Update(lib_path, lib_before, lib_after))

    simulation_path = (
        root / "crates/galactic_sim/src/simulation.rs"
    )
    simulation_before = simulation_path.read_text(
        encoding="utf-8"
    )
    simulation_after = patch_simulation(simulation_before)
    if simulation_before != simulation_after:
        updates.append(
            Update(
                simulation_path,
                simulation_before,
                simulation_after,
            )
        )

    client_path = root / "crates/galactic_client/src/lib.rs"
    client_before = client_path.read_text(encoding="utf-8")
    client_after = patch_client(client_before)
    if client_before != client_after:
        updates.append(
            Update(client_path, client_before, client_after)
        )

    docs_path = root / "docs/mvp_architecture.md"
    docs_before = docs_path.read_text(encoding="utf-8")
    docs_after = patch_docs(docs_before)
    if docs_before != docs_after:
        updates.append(
            Update(docs_path, docs_before, docs_after)
        )

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


def apply_updates(
    updates: list[Update],
    root: Path,
    dry_run: bool,
) -> None:
    if not updates:
        print("MVP-008 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp008-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        if update.path.exists():
            backup = backup_root / relative
            backup.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(update.path, backup)
        update.path.parent.mkdir(parents=True, exist_ok=True)
        update.path.write_text(
            update.after,
            encoding="utf-8",
        )
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def checks(root: Path) -> None:
    run(
        ["cargo", "fmt", "--all"],
        cwd=root,
        capture=False,
    )
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
    run(
        ["cargo", "test", "--workspace"],
        cwd=root,
        capture=False,
    )
    run(
        ["cargo", "build", "--release"],
        cwd=root,
        capture=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
    )
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_mvp7(root)

    status = run(
        ["git", "status", "--porcelain"],
        cwd=root,
    ).stdout
    if status.strip():
        print(
            "WARNING: working tree already contains changes."
        )
        print(
            status,
            end="" if status.endswith("\n") else "\n",
        )

    updates = collect_updates(root)
    apply_updates(updates, root, args.dry_run)

    if args.dry_run:
        print(
            f"\nDry-run complete: {len(updates)} "
            "file(s) would change."
        )
        return 0

    if args.skip_checks:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets "
            "--all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        checks(root)

    print(
        "\nMVP-008 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

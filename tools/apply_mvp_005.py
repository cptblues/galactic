#!/usr/bin/env python3
"""
Applique MVP-005 au dépôt Galactic.

Baseline analysée :
    fd36c676310da007fb332ea139387606cdbf3712
    feat mvp 4 ended

Usage :
    python tools/apply_mvp_005.py --dry-run
    python tools/apply_mvp_005.py
    python tools/apply_mvp_005.py --skip-checks
    python tools/apply_mvp_005.py --root /chemin/vers/galactic

Le script est idempotent, crée des sauvegardes et refuse une architecture
inattendue sauf utilisation explicite de --force.
"""

from __future__ import annotations

import argparse
import difflib
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = "fd36c676310da007fb332ea139387606cdbf3712"

FILES = {
    "crates/galactic_sim/src/time.rs": 'use std::fmt;\nuse std::time::Duration;\n\n/// Fréquence métier du MVP.\n///\n/// Tous les futurs systèmes temporels (production, construction, recherche,\n/// missions) doivent progresser sur ces ticks, jamais directement sur les FPS.\npub const STRATEGIC_TICKS_PER_SECOND: u32 = 10;\npub const STRATEGIC_TICK_NANOS: u64 =\n    1_000_000_000_u64 / STRATEGIC_TICKS_PER_SECOND as u64;\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct StrategicTick(u64);\n\nimpl StrategicTick {\n    pub const ZERO: Self = Self(0);\n\n    pub const fn new(value: u64) -> Self {\n        Self(value)\n    }\n\n    pub const fn value(self) -> u64 {\n        self.0\n    }\n\n    pub const fn saturating_add(self, ticks: u64) -> Self {\n        Self(self.0.saturating_add(ticks))\n    }\n\n    pub fn elapsed(self) -> Duration {\n        Duration::from_nanos(self.0.saturating_mul(STRATEGIC_TICK_NANOS))\n    }\n}\n\nimpl fmt::Display for StrategicTick {\n    fn fmt(&self, formatter: &mut fmt::Formatter<\'_>) -> fmt::Result {\n        self.0.fmt(formatter)\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct StrategicDuration {\n    ticks: u64,\n}\n\nimpl StrategicDuration {\n    pub const ZERO: Self = Self::from_ticks(0);\n\n    pub const fn from_ticks(ticks: u64) -> Self {\n        Self { ticks }\n    }\n\n    pub const fn ticks(self) -> u64 {\n        self.ticks\n    }\n\n    pub const fn is_zero(self) -> bool {\n        self.ticks == 0\n    }\n\n    pub fn as_duration(&self) -> Duration {\n        Duration::from_nanos(self.ticks.saturating_mul(STRATEGIC_TICK_NANOS))\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub enum TimeSpeed {\n    Paused,\n    #[default]\n    X1,\n    X2,\n    X4,\n}\n\nimpl TimeSpeed {\n    pub const fn factor(self) -> u32 {\n        match self {\n            Self::Paused => 0,\n            Self::X1 => 1,\n            Self::X2 => 2,\n            Self::X4 => 4,\n        }\n    }\n\n    pub const fn is_paused(self) -> bool {\n        matches!(self, Self::Paused)\n    }\n}\n\n\nimpl fmt::Display for TimeSpeed {\n    fn fmt(&self, formatter: &mut fmt::Formatter<\'_>) -> fmt::Result {\n        match self {\n            Self::Paused => formatter.write_str("pause"),\n            Self::X1 => formatter.write_str("x1"),\n            Self::X2 => formatter.write_str("x2"),\n            Self::X4 => formatter.write_str("x4"),\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum StrategicClockError {\n    RemainderOutOfRange(u64),\n    PausedResumeSpeed,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicAdvance {\n    pub ticks: StrategicDuration,\n    pub current_tick: StrategicTick,\n}\n\nimpl StrategicAdvance {\n    pub const fn none(current_tick: StrategicTick) -> Self {\n        Self {\n            ticks: StrategicDuration::ZERO,\n            current_tick,\n        }\n    }\n}\n\n/// Horloge mutable et sauvegardable de la partie.\n///\n/// `remainder_nanos` conserve la fraction de tick entre deux frames. Elle est\n/// exprimée en nanosecondes de temps stratégique déjà multiplié par la vitesse.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClock {\n    current_tick: StrategicTick,\n    remainder_nanos: u64,\n    speed: TimeSpeed,\n    resume_speed: TimeSpeed,\n}\n\nimpl StrategicClock {\n    pub const fn new() -> Self {\n        Self {\n            current_tick: StrategicTick::ZERO,\n            remainder_nanos: 0,\n            speed: TimeSpeed::X1,\n            resume_speed: TimeSpeed::X1,\n        }\n    }\n\n    pub fn from_parts(\n        current_tick: StrategicTick,\n        remainder_nanos: u64,\n        speed: TimeSpeed,\n        resume_speed: TimeSpeed,\n    ) -> Result<Self, StrategicClockError> {\n        if remainder_nanos >= STRATEGIC_TICK_NANOS {\n            return Err(StrategicClockError::RemainderOutOfRange(\n                remainder_nanos,\n            ));\n        }\n        if resume_speed.is_paused() {\n            return Err(StrategicClockError::PausedResumeSpeed);\n        }\n\n        Ok(Self {\n            current_tick,\n            remainder_nanos,\n            speed,\n            resume_speed: if speed.is_paused() {\n                resume_speed\n            } else {\n                speed\n            },\n        })\n    }\n\n    pub const fn current_tick(&self) -> StrategicTick {\n        self.current_tick\n    }\n\n    pub const fn remainder_nanos(&self) -> u64 {\n        self.remainder_nanos\n    }\n\n    pub const fn speed(&self) -> TimeSpeed {\n        self.speed\n    }\n\n    pub const fn resume_speed(&self) -> TimeSpeed {\n        self.resume_speed\n    }\n\n    pub fn elapsed(&self) -> Duration {\n        self.current_tick.elapsed()\n    }\n\n    pub fn elapsed_seconds(&self) -> f64 {\n        self.elapsed().as_secs_f64()\n    }\n\n    pub fn set_speed(&mut self, speed: TimeSpeed) -> bool {\n        if self.speed == speed {\n            return false;\n        }\n\n        self.speed = speed;\n        if !speed.is_paused() {\n            self.resume_speed = speed;\n        }\n        true\n    }\n\n    pub fn toggle_pause(&mut self) -> TimeSpeed {\n        let next = if self.speed.is_paused() {\n            self.resume_speed\n        } else {\n            TimeSpeed::Paused\n        };\n        self.set_speed(next);\n        next\n    }\n\n    pub fn advance(&mut self, real_delta: Duration) -> StrategicAdvance {\n        let factor = u64::from(self.speed.factor());\n        if factor == 0 || real_delta.is_zero() {\n            return StrategicAdvance::none(self.current_tick);\n        }\n\n        let real_nanos = real_delta.as_nanos().min(u128::from(u64::MAX)) as u64;\n        let scaled_nanos = real_nanos.saturating_mul(factor);\n        let total_nanos = self.remainder_nanos.saturating_add(scaled_nanos);\n        let advanced_ticks = total_nanos / STRATEGIC_TICK_NANOS;\n\n        self.remainder_nanos = total_nanos % STRATEGIC_TICK_NANOS;\n        self.current_tick = self.current_tick.saturating_add(advanced_ticks);\n\n        StrategicAdvance {\n            ticks: StrategicDuration::from_ticks(advanced_ticks),\n            current_tick: self.current_tick,\n        }\n    }\n}\n\nimpl Default for StrategicClock {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn partial_frames_accumulate_into_fixed_ticks() {\n        let mut clock = StrategicClock::new();\n\n        assert!(clock.advance(Duration::from_millis(40)).ticks.is_zero());\n        assert!(clock.advance(Duration::from_millis(40)).ticks.is_zero());\n\n        let result = clock.advance(Duration::from_millis(20));\n\n        assert_eq!(result.ticks, StrategicDuration::from_ticks(1));\n        assert_eq!(clock.current_tick(), StrategicTick::new(1));\n        assert_eq!(clock.remainder_nanos(), 0);\n    }\n\n    #[test]\n    fn pause_resumes_the_previous_speed() {\n        let mut clock = StrategicClock::new();\n        clock.set_speed(TimeSpeed::X4);\n\n        assert_eq!(clock.toggle_pause(), TimeSpeed::Paused);\n        assert_eq!(clock.toggle_pause(), TimeSpeed::X4);\n    }\n\n    #[test]\n    fn invalid_saved_remainder_is_rejected() {\n        assert_eq!(\n            StrategicClock::from_parts(\n                StrategicTick::ZERO,\n                STRATEGIC_TICK_NANOS,\n                TimeSpeed::X1,\n                TimeSpeed::X1,\n            ),\n            Err(StrategicClockError::RemainderOutOfRange(\n                STRATEGIC_TICK_NANOS\n            ))\n        );\n    }\n}\n',
    "crates/galactic_sim/src/command.rs": 'use galactic_domain::{PlanetId, SystemId};\n\nuse crate::TimeSpeed;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameCommand {\n    TogglePause,\n    SetSpeed(TimeSpeed),\n    SelectSystem(SystemId),\n    SelectPlanet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n    ClearSelection,\n}\n',
    "crates/galactic_sim/src/event.rs": 'use galactic_domain::{PlanetId, SystemId};\n\nuse crate::{StrategicDuration, StrategicTick, TimeSpeed};\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub enum SelectionTarget {\n    #[default]\n    None,\n    System(SystemId),\n    Planet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameEvent {\n    SpeedChanged(TimeSpeed),\n    SelectionChanged(SelectionTarget),\n    TicksAdvanced {\n        ticks: StrategicDuration,\n        current_tick: StrategicTick,\n    },\n}\n',
    "crates/galactic_sim/src/lib.rs": '// MVP-005: fixed strategic clock independent from rendering FPS\npub mod command;\npub mod event;\npub mod simulation;\npub mod state;\npub mod time;\npub mod universe;\n\npub use command::*;\npub use event::*;\npub use simulation::*;\npub use state::*;\npub use time::*;\npub use universe::*;\n',
    "crates/galactic_sim/src/state.rs": '// MVP-005: mutable game state owns a fixed, serializable strategic clock\nuse galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, SystemId};\n\nuse crate::{SelectionTarget, StrategicClock, UniverseRepository};\n\n/// Version of the mutable in-memory state contract.\n///\n/// Version 2 replaces floating elapsed seconds with a deterministic tick clock.\npub const GAME_STATE_VERSION: u32 = 2;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct GameState {\n    pub version: u32,\n    pub player_faction: FactionId,\n    pub colonies: Vec<ColonyState>,\n    pub known_systems: Vec<SystemId>,\n    pub selected: SelectionTarget,\n    pub clock: StrategicClock,\n}\n\nimpl GameState {\n    pub fn new(universe: &UniverseRepository) -> Self {\n        let home_system_id = SystemId::from_index(0);\n        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);\n        let player_faction = FactionId::new(0);\n        let mut known_systems = vec![home_system_id];\n        known_systems.extend(universe.neighboring_systems(home_system_id));\n        known_systems.sort();\n        known_systems.dedup();\n\n        debug_assert!(universe.system(home_system_id).is_some());\n        debug_assert!(universe.planet(home_planet_id).is_some());\n\n        Self {\n            version: GAME_STATE_VERSION,\n            player_faction,\n            colonies: vec![ColonyState {\n                id: ColonyId::new(0),\n                name: "Aster Prime Colony".to_string(),\n                faction: player_faction,\n                system_id: home_system_id,\n                planet_id: home_planet_id,\n                stock: ResourceStock::new(120, 45, 80, 30),\n            }],\n            known_systems,\n            selected: SelectionTarget::System(home_system_id),\n            clock: StrategicClock::new(),\n        }\n    }\n\n    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {\n        self.colonies.iter().find(|colony| colony.id == id)\n    }\n\n    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {\n        self.colonies.iter_mut().find(|colony| colony.id == id)\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonyState {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{ColonyId, UniverseConfig};\n\n    use super::*;\n\n    #[test]\n    fn colony_is_accessible_by_stable_id() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        let colony = state\n            .colony(ColonyId::new(0))\n            .expect("home colony is indexed by its stable ID");\n\n        assert_eq!(colony.name, "Aster Prime Colony");\n    }\n\n    #[test]\n    fn new_game_starts_at_tick_zero_and_speed_one() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        assert_eq!(state.clock.current_tick().value(), 0);\n        assert_eq!(state.clock.speed(), crate::TimeSpeed::X1);\n    }\n}\n',
    "crates/galactic_sim/src/simulation.rs": '// MVP-005: fixed strategic clock independent from rendering FPS\nuse std::collections::HashSet;\nuse std::time::Duration;\n\nuse galactic_domain::{ColonyId, PlanetId, SystemId, UniverseConfig, UniverseDefinition};\n\nuse crate::{\n    GAME_STATE_VERSION, GameCommand, GameEvent, GameState, SelectionTarget, TimeSpeed,\n    UniverseIndexError, UniverseRepository,\n};\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SimulationBuildError {\n    InvalidUniverse(UniverseIndexError),\n    UnsupportedStateVersion {\n        expected: u32,\n        found: u32,\n    },\n    DuplicateColony(ColonyId),\n    UnknownKnownSystem(SystemId),\n    UnknownColonySystem {\n        colony_id: ColonyId,\n        system_id: SystemId,\n    },\n    UnknownColonyPlanet {\n        colony_id: ColonyId,\n        planet_id: PlanetId,\n    },\n    ColonyPlanetSystemMismatch {\n        colony_id: ColonyId,\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n    InvalidSelectedSystem(SystemId),\n    InvalidSelectedPlanet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n}\n\nimpl From<UniverseIndexError> for SimulationBuildError {\n    fn from(error: UniverseIndexError) -> Self {\n        Self::InvalidUniverse(error)\n    }\n}\n\n#[derive(Debug, Clone)]\npub struct Simulation {\n    universe: UniverseRepository,\n    state: GameState,\n}\n\nimpl Simulation {\n    pub fn new(config: UniverseConfig) -> Self {\n        let universe = UniverseRepository::generate(config);\n        let state = GameState::new(&universe);\n        Self { universe, state }\n    }\n\n    pub fn from_parts(\n        universe: UniverseDefinition,\n        state: GameState,\n    ) -> Result<Self, SimulationBuildError> {\n        let universe = UniverseRepository::new(universe)?;\n        validate_state(&universe, &state)?;\n        Ok(Self { universe, state })\n    }\n\n    /// Immutable generated definition. No mutable universe accessor exists.\n    pub fn universe(&self) -> &UniverseDefinition {\n        self.universe.definition()\n    }\n\n    pub fn universe_repository(&self) -> &UniverseRepository {\n        &self.universe\n    }\n\n    pub fn state(&self) -> &GameState {\n        &self.state\n    }\n\n    pub fn state_mut(&mut self) -> &mut GameState {\n        &mut self.state\n    }\n\n    pub fn apply_command(&mut self, command: GameCommand) -> Vec<GameEvent> {\n        match command {\n            GameCommand::TogglePause => {\n                let next_speed = self.state.clock.toggle_pause();\n                vec![GameEvent::SpeedChanged(next_speed)]\n            }\n            GameCommand::SetSpeed(speed) => self.set_speed(speed),\n            GameCommand::SelectSystem(system_id) => self.select_system(system_id),\n            GameCommand::SelectPlanet {\n                system_id,\n                planet_id,\n            } => self.select_planet(system_id, planet_id),\n            GameCommand::ClearSelection => self.set_selection(SelectionTarget::None),\n        }\n    }\n\n    /// Advances simulation time from a real frame duration.\n    ///\n    /// The real duration is converted into fixed strategic ticks by the clock.\n    /// Rendering, UI and camera systems remain outside this method.\n    pub fn advance(&mut self, real_delta: Duration) -> Vec<GameEvent> {\n        let advance = self.state.clock.advance(real_delta);\n        if advance.ticks.is_zero() {\n            return Vec::new();\n        }\n\n        // Future production, construction, research and mission systems will be\n        // processed once per strategic tick here.\n        vec![GameEvent::TicksAdvanced {\n            ticks: advance.ticks,\n            current_tick: advance.current_tick,\n        }]\n    }\n\n    fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEvent> {\n        if !self.state.clock.set_speed(speed) {\n            return Vec::new();\n        }\n\n        vec![GameEvent::SpeedChanged(speed)]\n    }\n\n    fn select_system(&mut self, system_id: SystemId) -> Vec<GameEvent> {\n        if self.universe.system(system_id).is_none() {\n            return Vec::new();\n        }\n\n        self.set_selection(SelectionTarget::System(system_id))\n    }\n\n    fn select_planet(&mut self, system_id: SystemId, planet_id: PlanetId) -> Vec<GameEvent> {\n        let Some((planet_system_id, _)) = self.universe.planet_location(planet_id) else {\n            return Vec::new();\n        };\n        if planet_system_id != system_id {\n            return Vec::new();\n        }\n\n        self.set_selection(SelectionTarget::Planet {\n            system_id,\n            planet_id,\n        })\n    }\n\n    fn set_selection(&mut self, selection: SelectionTarget) -> Vec<GameEvent> {\n        if self.state.selected == selection {\n            return Vec::new();\n        }\n\n        self.state.selected = selection;\n        vec![GameEvent::SelectionChanged(selection)]\n    }\n}\n\nfn validate_state(\n    universe: &UniverseRepository,\n    state: &GameState,\n) -> Result<(), SimulationBuildError> {\n    if state.version != GAME_STATE_VERSION {\n        return Err(SimulationBuildError::UnsupportedStateVersion {\n            expected: GAME_STATE_VERSION,\n            found: state.version,\n        });\n    }\n\n    for system_id in &state.known_systems {\n        if universe.system(*system_id).is_none() {\n            return Err(SimulationBuildError::UnknownKnownSystem(*system_id));\n        }\n    }\n\n    let mut colony_ids = HashSet::with_capacity(state.colonies.len());\n    for colony in &state.colonies {\n        if !colony_ids.insert(colony.id) {\n            return Err(SimulationBuildError::DuplicateColony(colony.id));\n        }\n        if universe.system(colony.system_id).is_none() {\n            return Err(SimulationBuildError::UnknownColonySystem {\n                colony_id: colony.id,\n                system_id: colony.system_id,\n            });\n        }\n        let Some((planet_system_id, _)) = universe.planet_location(colony.planet_id) else {\n            return Err(SimulationBuildError::UnknownColonyPlanet {\n                colony_id: colony.id,\n                planet_id: colony.planet_id,\n            });\n        };\n        if planet_system_id != colony.system_id {\n            return Err(SimulationBuildError::ColonyPlanetSystemMismatch {\n                colony_id: colony.id,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n            });\n        }\n    }\n\n    match state.selected {\n        SelectionTarget::None => {}\n        SelectionTarget::System(system_id) => {\n            if universe.system(system_id).is_none() {\n                return Err(SimulationBuildError::InvalidSelectedSystem(system_id));\n            }\n        }\n        SelectionTarget::Planet {\n            system_id,\n            planet_id,\n        } => {\n            let Some((planet_system_id, _)) = universe.planet_location(planet_id) else {\n                return Err(SimulationBuildError::InvalidSelectedPlanet {\n                    system_id,\n                    planet_id,\n                });\n            };\n            if planet_system_id != system_id {\n                return Err(SimulationBuildError::InvalidSelectedPlanet {\n                    system_id,\n                    planet_id,\n                });\n            }\n        }\n    }\n\n    Ok(())\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{PlanetId, ResourceStock, SystemId, UniverseConfig};\n\n    use super::*;\n\n    fn advance_in_equal_frames(\n        simulation: &mut Simulation,\n        frame_count: u32,\n        frame_duration: Duration,\n    ) {\n        for _ in 0..frame_count {\n            simulation.advance(frame_duration);\n        }\n    }\n\n    #[test]\n    fn simulation_advances_without_renderer() {\n        let mut simulation = Simulation::new(UniverseConfig::default());\n\n        let events = simulation.advance(Duration::from_millis(250));\n\n        assert_eq!(simulation.state().clock.current_tick().value(), 2);\n        assert_eq!(\n            events,\n            vec![GameEvent::TicksAdvanced {\n                ticks: crate::StrategicDuration::from_ticks(2),\n                current_tick: crate::StrategicTick::new(2),\n            }]\n        );\n        assert_eq!(simulation.state().clock.remainder_nanos(), 50_000_000);\n    }\n\n    #[test]\n    fn different_frame_rates_produce_the_same_ticks() {\n        let mut fast_frames = Simulation::new(UniverseConfig::mvp());\n        let mut slow_frames = Simulation::new(UniverseConfig::mvp());\n\n        advance_in_equal_frames(&mut fast_frames, 100, Duration::from_millis(10));\n        advance_in_equal_frames(&mut slow_frames, 10, Duration::from_millis(100));\n\n        assert_eq!(fast_frames.state().clock, slow_frames.state().clock);\n        assert_eq!(fast_frames.state().clock.current_tick().value(), 10);\n    }\n\n    #[test]\n    fn pause_and_resume_do_not_duplicate_or_skip_ticks() {\n        let mut simulation = Simulation::new(UniverseConfig::default());\n\n        simulation.advance(Duration::from_millis(50));\n        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::Paused));\n        assert!(simulation.advance(Duration::from_secs(10)).is_empty());\n        assert_eq!(simulation.state().clock.current_tick().value(), 0);\n        assert_eq!(simulation.state().clock.remainder_nanos(), 50_000_000);\n\n        simulation.apply_command(GameCommand::TogglePause);\n        simulation.advance(Duration::from_millis(50));\n\n        assert_eq!(simulation.state().clock.current_tick().value(), 1);\n        assert_eq!(simulation.state().clock.remainder_nanos(), 0);\n    }\n\n    #[test]\n    fn speed_changes_simulation_tick_rate() {\n        let mut normal = Simulation::new(UniverseConfig::mvp());\n        let mut accelerated = Simulation::new(UniverseConfig::mvp());\n        accelerated.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n\n        normal.advance(Duration::from_millis(500));\n        accelerated.advance(Duration::from_millis(500));\n\n        assert_eq!(normal.state().clock.current_tick().value(), 5);\n        assert_eq!(accelerated.state().clock.current_tick().value(), 20);\n    }\n\n    #[test]\n    fn selection_events_use_domain_ids() {\n        let mut simulation = Simulation::new(UniverseConfig::default());\n        let system_id = SystemId::from_index(0);\n        let planet_id = PlanetId::from_system_index(system_id, 0);\n\n        let events = simulation.apply_command(GameCommand::SelectPlanet {\n            system_id,\n            planet_id,\n        });\n\n        assert_eq!(\n            events,\n            vec![GameEvent::SelectionChanged(SelectionTarget::Planet {\n                system_id,\n                planet_id,\n            })]\n        );\n    }\n\n    #[test]\n    fn invalid_selection_is_ignored() {\n        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));\n\n        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));\n\n        assert!(events.is_empty());\n        assert_eq!(\n            simulation.state().selected,\n            SelectionTarget::System(SystemId::from_index(0))\n        );\n    }\n\n    #[test]\n    fn mutable_actions_do_not_change_generated_universe() {\n        let mut simulation = Simulation::new(UniverseConfig::mvp());\n        let initial_universe = simulation.universe().clone();\n\n        simulation.advance(Duration::from_secs(42));\n        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n        simulation\n            .state_mut()\n            .colony_mut(ColonyId::new(0))\n            .expect("home colony exists")\n            .stock = ResourceStock::new(999, 888, 777, 666);\n\n        assert_eq!(simulation.universe(), &initial_universe);\n        assert_ne!(\n            simulation\n                .state()\n                .colony(ColonyId::new(0))\n                .expect("home colony exists")\n                .stock,\n            ResourceStock::new(120, 45, 80, 30)\n        );\n    }\n\n    #[test]\n    fn visual_world_inputs_are_available_as_definition_plus_state() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n\n        assert!(!simulation.universe().systems.is_empty());\n        assert!(!simulation.state().known_systems.is_empty());\n        assert!(simulation.state().colony(ColonyId::new(0)).is_some());\n    }\n}\n',
    "crates/galactic_persistence/src/lib.rs": '// MVP-005: strategic tick clock is persisted without frame-time floats\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseId,\n    generate_universe,\n};\nuse galactic_sim::{\n    ColonyState, GameState, SelectionTarget, Simulation, SimulationBuildError, StrategicClock,\n    StrategicClockError, StrategicTick, TimeSpeed,\n};\n\npub const SAVE_VERSION: u32 = 3;\n\n/// Persistence envelope: generated data is referenced, not duplicated.\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub known_systems: Vec<SystemId>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(simulation: &Simulation) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint: universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos: state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            known_systems: state.known_systems.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.stock,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(save: &SaveGame) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(SaveError::UnsupportedVersion(save.version));\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version != save.universe.generation_version {\n        return Err(SaveError::GenerationVersionMismatch {\n            expected: universe.generation_version,\n            found: save.universe.generation_version,\n        });\n    }\n    if universe.generation_fingerprint != save.universe.generation_fingerprint {\n        return Err(SaveError::GenerationFingerprintMismatch {\n            expected: universe.generation_fingerprint,\n            found: save.universe.generation_fingerprint,\n        });\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let state = GameState {\n        version: save.state.version,\n        player_faction: save.state.player_faction,\n        colonies: save\n            .state\n            .colonies\n            .iter()\n            .map(|colony| ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                stock: colony.stock,\n            })\n            .collect(),\n        known_systems: save.state.known_systems.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::UniverseConfig;\n    use galactic_sim::{\n        GAME_STATE_VERSION, GameCommand, STRATEGIC_TICK_NANOS, StrategicTick, TimeSpeed,\n    };\n\n    use super::*;\n\n    #[test]\n    fn snapshot_round_trips_mutable_state_and_regenerates_universe() {\n        let mut simulation = Simulation::new(UniverseConfig::new(99, 14));\n        simulation.advance(Duration::from_millis(125));\n        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n\n        let original_fingerprint = simulation.universe().generation_fingerprint;\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save).expect("save is compatible");\n\n        assert_eq!(\n            restored.universe().generation_fingerprint,\n            original_fingerprint\n        );\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(restored.state().clock.current_tick(), StrategicTick::new(1));\n        assert_eq!(restored.state().clock.remainder_nanos(), 25_000_000);\n    }\n\n    #[test]\n    fn snapshot_contains_a_universe_reference_and_strategic_clock() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let save = snapshot_from_simulation(&simulation);\n\n        assert_eq!(\n            save.universe.system_count,\n            simulation.universe().systems.len()\n        );\n        assert_eq!(\n            save.universe.generation_fingerprint,\n            simulation.universe().generation_fingerprint\n        );\n        assert_eq!(save.state.version, GAME_STATE_VERSION);\n        assert_eq!(save.state.clock.current_tick, StrategicTick::ZERO);\n    }\n\n    #[test]\n    fn modified_fingerprint_is_rejected() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.universe.generation_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::GenerationFingerprintMismatch { .. })\n        ));\n    }\n\n    #[test]\n    fn invalid_clock_remainder_is_rejected() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.state.clock.remainder_nanos = STRATEGIC_TICK_NANOS;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidClock(\n                StrategicClockError::RemainderOutOfRange(_)\n            ))\n        ));\n    }\n\n    #[test]\n    fn unsupported_save_version_is_rejected() {\n        let simulation = Simulation::new(UniverseConfig::default());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.version = 999;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::UnsupportedVersion(999))\n        ));\n    }\n}\n',
}

DOC_APPEND = "\n## MVP-005 — Temps stratégique déterministe\n\nLe temps métier est désormais indépendant du nombre d'images rendues :\n\n```text\nDurée réelle d'une frame\n        │ multipliée par x1 / x2 / x4\n        ▼\nStrategicClock\n        │ accumulation entière en nanosecondes\n        ▼\nTicks fixes à 10 Hz\n        ▼\nProduction / construction / recherche / missions\n```\n\nRègles :\n\n- `StrategicTick` est le timestamp métier sauvegardable ;\n- `StrategicDuration` exprime une durée en nombre entier de ticks ;\n- `StrategicClock` conserve le tick courant et la fraction de tick restante ;\n- la pause bloque uniquement l'horloge de simulation ;\n- la caméra et l'interface continuent d'utiliser le temps Bevy normal ;\n- `Simulation::advance(Duration)` remplace l'ancien avancement direct en `f32` ;\n- changer le framerate ne change pas le nombre de ticks obtenus sur une même durée ;\n- la sauvegarde conserve le tick courant, le reliquat et la vitesse ;\n- `GAME_STATE_VERSION = 2` et `SAVE_VERSION = 3`.\n\nFréquence stratégique actuelle : `10 ticks/seconde`.\n"


@dataclass(frozen=True)
class Update:
    path: Path
    before: str
    after: str


def run(command: list[str], *, cwd: Path, check: bool = True, capture: bool = True):
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
            and (candidate / "crates/galactic_sim/src/simulation.rs").exists()
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
        "Le dépôt local ne correspond pas à la baseline MVP-004 analysée.\n"
        f"HEAD={head}\nAttendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après vérification."
    )


def verify_mvp4(root: Path) -> None:
    state = (root / "crates/galactic_sim/src/state.rs").read_text()
    simulation = (root / "crates/galactic_sim/src/simulation.rs").read_text()
    persistence = (root / "crates/galactic_persistence/src/lib.rs").read_text()
    failures = []
    if "UniverseRepository" not in simulation:
        failures.append("UniverseRepository absent de Simulation")
    if "pub universe:" in state or "UniverseDefinition" in state:
        failures.append("GameState contient encore l'univers")
    if "UniverseReference" not in persistence or "MutableGameSave" not in persistence:
        failures.append("séparation de sauvegarde MVP-004 absente")
    if failures:
        raise SystemExit(
            "Baseline MVP-004 incohérente :\n- " + "\n- ".join(failures)
        )


def patch_client(source: str) -> str:
    updated = source.replace(
        "simulation.simulation.tick(time.delta_secs())",
        "simulation.simulation.advance(time.delta())",
    )

    old_setup = """    let state = simulation.simulation().state();
    let selected = selection_label(state.selected);"""
    new_setup = """    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let state = simulation.state();
    let selected = selection_label(state.selected);"""
    if old_setup in updated and "let universe = simulation.universe();" not in updated:
        updated = updated.replace(old_setup, new_setup, 1)

    old_format = """        "Galactic MVP | Bevy 0.19 | seed {} | gen v{} | fp {:016x} | systems {} | routes {} | colonies {} | known {} | t {:.1}s | speed {} | selected {} | event {}","""
    new_format = """        "Galactic MVP | Bevy 0.19 | seed {} | gen v{} | fp {:016x} | systems {} | routes {} | colonies {} | known {} | tick {} | t {:.1}s | speed {} | selected {} | event {}","""
    if old_format in updated:
        updated = updated.replace(old_format, new_format, 1)

    old_args = """        state.known_systems.len(),
        state.elapsed_seconds,
        state.speed,
        selected,"""
    new_args = """        state.known_systems.len(),
        state.clock.current_tick(),
        state.clock.elapsed_seconds(),
        state.clock.speed(),
        selected,"""
    if old_args in updated:
        updated = updated.replace(old_args, new_args, 1)

    old_event = """        GameEvent::TickAdvanced {
            elapsed_seconds, ..
        } => format!("tick {:.1}s", elapsed_seconds),"""
    new_event = """        GameEvent::TicksAdvanced {
            ticks,
            current_tick,
        } => format!("+{} ticks -> {}", ticks.ticks(), current_tick),"""
    if old_event in updated:
        updated = updated.replace(old_event, new_event, 1)

    if "simulation.simulation.tick(" in updated:
        raise SystemExit("L'ancien appel Simulation::tick est encore présent dans le client.")
    if "state.elapsed_seconds" in updated or "state.speed" in updated:
        raise SystemExit("Le HUD utilise encore les anciens champs temporels.")
    if "GameEvent::TickAdvanced" in updated:
        raise SystemExit("Le client utilise encore l'ancien événement TickAdvanced.")
    return normalize(updated)


def patch_docs(source: str) -> str:
    if "## MVP-005 — Temps stratégique déterministe" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []
    for relative, content in FILES.items():
        path = root / relative
        before = path.read_text(encoding="utf-8") if path.exists() else ""
        after = normalize(content)
        if before != after:
            updates.append(Update(path, before, after))

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


def apply(updates: list[Update], root: Path, dry_run: bool) -> None:
    if not updates:
        print("MVP-005 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = root / ".mvp005-backup" / datetime.now().strftime("%Y%m%d-%H%M%S")
    for update in updates:
        relative = update.path.relative_to(root)
        if update.path.exists():
            backup = backup_root / relative
            backup.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(update.path, backup)
        update.path.parent.mkdir(parents=True, exist_ok=True)
        update.path.write_text(update.after, encoding="utf-8")
        print(f"+ updated: {relative}")
    print(f"Backup directory: {backup_root}")


def checks(root: Path) -> None:
    run(["cargo", "fmt", "--all"], cwd=root, capture=False)
    run(
        [
            "cargo", "clippy", "--workspace", "--all-targets", "--all-features",
            "--", "-D", "warnings",
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
    verify_mvp4(root)

    status = run(["git", "status", "--porcelain"], cwd=root).stdout
    if status.strip():
        print("WARNING: working tree already contains changes.")
        print(status, end="" if status.endswith("\n") else "\n")

    updates = collect_updates(root)
    apply(updates, root, args.dry_run)

    if args.dry_run:
        print(f"\nDry-run complete: {len(updates)} file(s) would change.")
        return 0

    if not args.skip_checks:
        checks(root)
    else:
        print(
            "\nChecks skipped. Run:\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )

    print(
        "\nMVP-005 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

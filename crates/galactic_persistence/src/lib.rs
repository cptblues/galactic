// MVP-008: save the player faction and configurable colony foundation
use galactic_domain::{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseId,
    generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, FactionKind, FactionState, GameState, PlanetResourceProfile,
    SelectionTarget, Simulation, SimulationBuildError, StrategicClock, StrategicClockError,
    StrategicTick, TimeSpeed,
};

pub const SAVE_VERSION: u32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    pub universe: UniverseReference,
    pub state: MutableGameSave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UniverseReference {
    pub id: UniverseId,
    pub seed: u64,
    pub system_count: usize,
    pub generation_version: u32,
    pub generation_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MutableGameSave {
    pub version: u32,
    pub factions: Vec<FactionSave>,
    pub player_faction: FactionId,
    pub clock: StrategicClockSave,
    pub selected: SelectionTarget,
    pub known_systems: Vec<SystemId>,
    pub colonies: Vec<ColonySave>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactionSave {
    pub id: FactionId,
    pub name: String,
    pub kind: FactionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategicClockSave {
    pub current_tick: StrategicTick,
    pub remainder_nanos: u64,
    pub speed: TimeSpeed,
    pub resume_speed: TimeSpeed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonySave {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    UnsupportedVersion(u32),
    UniverseIdMismatch {
        expected: UniverseId,
        found: UniverseId,
    },
    GenerationVersionMismatch {
        expected: u32,
        found: u32,
    },
    GenerationFingerprintMismatch {
        expected: u64,
        found: u64,
    },
    InvalidClock(StrategicClockError),
    InvalidState(SimulationBuildError),
}

pub fn snapshot_from_simulation(simulation: &Simulation) -> SaveGame {
    let universe = simulation.universe();
    let state = simulation.state();

    SaveGame {
        version: SAVE_VERSION,
        universe: UniverseReference {
            id: universe.id,
            seed: universe.seed,
            system_count: universe.systems.len(),
            generation_version: universe.generation_version,
            generation_fingerprint: universe.generation_fingerprint,
        },
        state: MutableGameSave {
            version: state.version,
            factions: state
                .factions
                .iter()
                .map(|faction| FactionSave {
                    id: faction.id,
                    name: faction.name.clone(),
                    kind: faction.kind,
                })
                .collect(),
            player_faction: state.player_faction,
            clock: StrategicClockSave {
                current_tick: state.clock.current_tick(),
                remainder_nanos: state.clock.remainder_nanos(),
                speed: state.clock.speed(),
                resume_speed: state.clock.resume_speed(),
            },
            selected: state.selected,
            known_systems: state.known_systems.clone(),
            colonies: state
                .colonies
                .iter()
                .map(|colony| ColonySave {
                    id: colony.id,
                    name: colony.name.clone(),
                    faction: colony.faction,
                    system_id: colony.system_id,
                    planet_id: colony.planet_id,
                    stock: colony.stock,
                    buildings: colony.buildings,
                    resource_profile: colony.resource_profile,
                })
                .collect(),
        },
    }
}

pub fn restore_from_snapshot(save: &SaveGame) -> Result<Simulation, SaveError> {
    if save.version != SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion(save.version));
    }

    let universe = generate_universe(UniverseConfig::new(
        save.universe.seed,
        save.universe.system_count,
    ));

    if universe.id != save.universe.id {
        return Err(SaveError::UniverseIdMismatch {
            expected: universe.id,
            found: save.universe.id,
        });
    }
    if universe.generation_version != save.universe.generation_version {
        return Err(SaveError::GenerationVersionMismatch {
            expected: universe.generation_version,
            found: save.universe.generation_version,
        });
    }
    if universe.generation_fingerprint != save.universe.generation_fingerprint {
        return Err(SaveError::GenerationFingerprintMismatch {
            expected: universe.generation_fingerprint,
            found: save.universe.generation_fingerprint,
        });
    }

    let clock = StrategicClock::from_parts(
        save.state.clock.current_tick,
        save.state.clock.remainder_nanos,
        save.state.clock.speed,
        save.state.clock.resume_speed,
    )
    .map_err(SaveError::InvalidClock)?;

    let state = GameState {
        version: save.state.version,
        factions: save
            .state
            .factions
            .iter()
            .map(|faction| FactionState {
                id: faction.id,
                name: faction.name.clone(),
                kind: faction.kind,
            })
            .collect(),
        player_faction: save.state.player_faction,
        colonies: save
            .state
            .colonies
            .iter()
            .map(|colony| ColonyState {
                id: colony.id,
                name: colony.name.clone(),
                faction: colony.faction,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
                stock: colony.stock,
                buildings: colony.buildings,
                resource_profile: colony.resource_profile,
            })
            .collect(),
        known_systems: save.state.known_systems.clone(),
        selected: save.state.selected,
        clock,
    };

    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use galactic_domain::UniverseConfig;
    use galactic_sim::{
        GAME_STATE_VERSION, GameCommand, STRATEGIC_TICK_NANOS, StartingScenario, StrategicTick,
        TimeSpeed,
    };

    use super::*;

    #[test]
    fn snapshot_round_trips_complete_starting_state() {
        let mut simulation = Simulation::new(UniverseConfig::new(99, 14));
        simulation.advance(Duration::from_millis(125));
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));

        let original_fingerprint = simulation.universe().generation_fingerprint;
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("save is compatible");

        assert_eq!(
            restored.universe().generation_fingerprint,
            original_fingerprint
        );
        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().clock.current_tick(), StrategicTick::new(1));
        assert_eq!(restored.state().clock.remainder_nanos(), 25_000_000);
    }

    #[test]
    fn snapshot_contains_player_faction_and_home_foundation() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        let scenario = StartingScenario::mvp();
        let colony = save.state.colonies.first().expect("home colony is saved");

        assert_eq!(save.state.version, GAME_STATE_VERSION);
        assert_eq!(save.state.factions.len(), 1);
        assert_eq!(save.state.player_faction, scenario.player_faction.id);
        assert_eq!(colony.buildings, scenario.home_colony.buildings);
        assert_eq!(
            colony.resource_profile,
            scenario.home_colony.resource_profile
        );
        assert_eq!(
            save.state.known_systems.as_slice(),
            scenario.initially_known_systems
        );
    }

    #[test]
    fn modified_fingerprint_is_rejected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.universe.generation_fingerprint ^= 1;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::GenerationFingerprintMismatch { .. })
        ));
    }

    #[test]
    fn invalid_clock_remainder_is_rejected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.state.clock.remainder_nanos = STRATEGIC_TICK_NANOS;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::InvalidClock(
                StrategicClockError::RemainderOutOfRange(_)
            ))
        ));
    }

    #[test]
    fn unsupported_save_version_is_rejected() {
        let simulation = Simulation::new(UniverseConfig::default());
        let mut save = snapshot_from_simulation(&simulation);
        save.version = 999;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        ));
    }
}

// MVP-004: immutable generated universe separated from mutable game state
use galactic_domain::{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseId,
    generate_universe,
};
use galactic_sim::{
    ColonyState, GAME_STATE_VERSION, GameState, SelectionTarget, Simulation, SimulationBuildError,
    TimeSpeed,
};

pub const SAVE_VERSION: u32 = 2;

/// Persistence envelope: generated data is referenced, not duplicated.
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
    pub player_faction: FactionId,
    pub elapsed_seconds: f32,
    pub speed: TimeSpeed,
    pub selected: SelectionTarget,
    pub known_systems: Vec<SystemId>,
    pub colonies: Vec<ColonySave>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonySave {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
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
            player_faction: state.player_faction,
            elapsed_seconds: state.elapsed_seconds,
            speed: state.speed,
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

    let state = GameState {
        version: save.state.version,
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
            })
            .collect(),
        known_systems: save.state.known_systems.clone(),
        selected: save.state.selected,
        elapsed_seconds: save.state.elapsed_seconds,
        speed: save.state.speed,
    };

    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::{GAME_STATE_VERSION, GameCommand, TimeSpeed};

    use super::*;

    #[test]
    fn snapshot_round_trips_mutable_state_and_regenerates_universe() {
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
    }

    #[test]
    fn snapshot_contains_a_universe_reference_not_generated_objects() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(
            save.universe.system_count,
            simulation.universe().systems.len()
        );
        assert_eq!(
            save.universe.generation_fingerprint,
            simulation.universe().generation_fingerprint
        );
        assert_eq!(save.state.version, GAME_STATE_VERSION);
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

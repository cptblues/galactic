// MVP-009: persist progressive knowledge and the MVP-008 foundation
use galactic_domain::{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseId,
    generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, FactionKind, FactionState, GameState, PlanetKnowledge,
    PlanetResourceProfile, SelectionTarget, Simulation, SimulationBuildError, StrategicClock,
    StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed,
};

pub const SAVE_VERSION: u32 = 5;

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
    pub system_knowledge: Vec<SystemKnowledge>,
    pub planet_knowledge: Vec<PlanetKnowledge>,
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
            system_knowledge: state.system_knowledge.clone(),
            planet_knowledge: state.planet_knowledge.clone(),
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
        system_knowledge: save.state.system_knowledge.clone(),
        planet_knowledge: save.state.planet_knowledge.clone(),
        selected: save.state.selected,
        clock,
    };

    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use galactic_domain::{SystemId, UniverseConfig};
    use galactic_sim::{
        GAME_STATE_VERSION, GameCommand, KnowledgeLevel, STRATEGIC_TICK_NANOS, StrategicTick,
        TimeSpeed,
    };

    use super::*;

    #[test]
    fn snapshot_round_trips_knowledge_and_starting_state() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .universe_repository()
            .neighboring_systems(SystemId::from_index(0))
            .into_iter()
            .next()
            .expect("home has a neighbor");
        simulation.apply_command(GameCommand::SelectSystem(target));
        simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);
        simulation.advance(Duration::from_millis(125));
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored.state().system_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert_eq!(restored.state().clock.current_tick(), StrategicTick::new(1));
    }

    #[test]
    fn snapshot_contains_progressive_knowledge() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.state.version, GAME_STATE_VERSION);
        assert!(!save.state.system_knowledge.is_empty());
        assert!(!save.state.planet_knowledge.is_empty());
        assert!(
            save.state
                .system_knowledge
                .iter()
                .any(|entry| { entry.level == KnowledgeLevel::Colonized })
        );
        assert!(
            save.state
                .system_knowledge
                .iter()
                .any(|entry| { entry.level == KnowledgeLevel::Detected })
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

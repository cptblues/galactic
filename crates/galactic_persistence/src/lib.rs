// MVP-011: persist resource reservations and energy capacity.
use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, PlanetId, ResourceLedger, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId, generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, FactionKind, FactionState, GameState, PlanetKnowledge,
    PlanetResourceProfile, SelectionTarget, Simulation, SimulationBuildError, StrategicClock,
    StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed,
};

pub const SAVE_VERSION: u32 = 6;

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
    pub reservations: Vec<ResourceReservation>,
    pub next_reservation_id: u64,
    pub energy_production: u64,
    pub energy_consumption: u64,
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
    InvalidResourceLedger {
        colony_id: ColonyId,
        error: ResourceLedgerError,
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
                    stock: colony.resources.stock(),
                    reservations: colony.resources.reservations().to_vec(),
                    next_reservation_id: colony.resources.next_reservation_id(),
                    energy_production: colony.energy.production(),
                    energy_consumption: colony.energy.consumption(),
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

    let colonies = save
        .state
        .colonies
        .iter()
        .map(|colony| {
            let resources = ResourceLedger::from_parts(
                colony.stock,
                colony.reservations.clone(),
                colony.next_reservation_id,
            )
            .map_err(|error| SaveError::InvalidResourceLedger {
                colony_id: colony.id,
                error,
            })?;

            Ok(ColonyState {
                id: colony.id,
                name: colony.name.clone(),
                faction: colony.faction,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
                resources,
                energy: EnergyGrid::new(colony.energy_production, colony.energy_consumption),
                buildings: colony.buildings,
                resource_profile: colony.resource_profile,
            })
        })
        .collect::<Result<Vec<_>, SaveError>>()?;

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
        colonies,
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

    use galactic_domain::{
        ReservationId, ResourceCost, ResourceReservation, SystemId, UniverseConfig,
    };
    use galactic_sim::{
        GAME_STATE_VERSION, GameCommand, KnowledgeLevel, STRATEGIC_TICK_NANOS, StrategicTick,
        TimeSpeed,
    };

    use super::*;

    #[test]
    fn snapshot_round_trips_economy_knowledge_and_clock() {
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

        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony
            .resources
            .reserve(ResourceCost::new(50, 25, 10))
            .expect("test reservation is funded");
        colony
            .energy
            .allocate(10)
            .expect("energy capacity is available");

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
    fn snapshot_contains_ledger_and_energy_balance() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        let colony = save.state.colonies.first().expect("home colony is saved");

        assert_eq!(save.state.version, GAME_STATE_VERSION);
        assert_eq!(colony.stock, ResourceStock::new(600, 300, 220));
        assert_eq!(colony.energy_production, 80);
        assert_eq!(colony.energy_consumption, 30);
    }

    #[test]
    fn invalid_over_reserved_ledger_is_rejected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        let colony = save
            .state
            .colonies
            .first_mut()
            .expect("home colony is saved");
        colony.reservations.push(ResourceReservation::new(
            ReservationId::new(1),
            ResourceCost::new(700, 0, 0),
        ));
        colony.next_reservation_id = 2;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::InvalidResourceLedger { .. })
        ));
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

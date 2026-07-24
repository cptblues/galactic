// MVP-014: persist construction queues and reserved upgrade costs.
use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, PlanetId, ResourceLedger, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId, generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, ConstructionQueue, FactionKind, FactionState, GameState,
    PRODUCTION_REFRESH_TICKS, PlanetKnowledge, PlanetResourceProfile, ProductionRemainder,
    ProductionRemainderError, SelectionTarget, Simulation, SimulationBuildError, StrategicClock,
    StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed, default_building_catalog,
};

pub const SAVE_VERSION: u32 = 9;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    pub catalog_version: u32,
    pub catalog_fingerprint: u64,
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
    pub production_remainder_metal: u16,
    pub production_remainder_crystal: u16,
    pub production_remainder_fuel: u16,
    pub production_pending_ticks: u16,
    pub construction_queue: ConstructionQueue,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    UnsupportedVersion(u32),
    CatalogVersionMismatch {
        expected: u32,
        found: u32,
    },
    CatalogFingerprintMismatch {
        expected: u64,
        found: u64,
    },
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
    InvalidProductionRemainder {
        colony_id: ColonyId,
        error: ProductionRemainderError,
    },
    InvalidPendingProductionTicks {
        colony_id: ColonyId,
        found: u16,
    },
    InvalidState(SimulationBuildError),
}

pub fn snapshot_from_simulation(simulation: &Simulation) -> SaveGame {
    let universe = simulation.universe();
    let state = simulation.state();
    let catalog = default_building_catalog();

    SaveGame {
        version: SAVE_VERSION,
        catalog_version: catalog.version(),
        catalog_fingerprint: catalog.fingerprint(),
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
                    production_remainder_metal: colony.production_remainder.metal_milli(),
                    production_remainder_crystal: colony.production_remainder.crystal_milli(),
                    production_remainder_fuel: colony.production_remainder.fuel_milli(),
                    production_pending_ticks: colony.production_pending_ticks,
                    construction_queue: colony.construction_queue.clone(),
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

    let catalog = default_building_catalog();
    if save.catalog_version != catalog.version() {
        return Err(SaveError::CatalogVersionMismatch {
            expected: catalog.version(),
            found: save.catalog_version,
        });
    }
    if save.catalog_fingerprint != catalog.fingerprint() {
        return Err(SaveError::CatalogFingerprintMismatch {
            expected: catalog.fingerprint(),
            found: save.catalog_fingerprint,
        });
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
            let production_remainder = ProductionRemainder::from_parts(
                colony.production_remainder_metal,
                colony.production_remainder_crystal,
                colony.production_remainder_fuel,
            )
            .map_err(|error| SaveError::InvalidProductionRemainder {
                colony_id: colony.id,
                error,
            })?;
            if u64::from(colony.production_pending_ticks) >= PRODUCTION_REFRESH_TICKS {
                return Err(SaveError::InvalidPendingProductionTicks {
                    colony_id: colony.id,
                    found: colony.production_pending_ticks,
                });
            }

            Ok(ColonyState {
                id: colony.id,
                name: colony.name.clone(),
                faction: colony.faction,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
                resources,
                energy: EnergyGrid::new(colony.energy_production, colony.energy_consumption),
                production_remainder,
                production_pending_ticks: colony.production_pending_ticks,
                construction_queue: colony.construction_queue.clone(),
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

    use galactic_domain::UniverseConfig;
    use galactic_sim::{BuildingKind, GAME_STATE_VERSION, GameCommand};

    use super::*;

    #[test]
    fn construction_queue_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        simulation.apply_command(GameCommand::QueueBuildingUpgrade {
            colony_id,
            kind: BuildingKind::MetalMine,
        });
        simulation.advance(Duration::from_secs(2));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .construction_queue
                .len(),
            1
        );
    }

    #[test]
    fn catalog_changes_are_detected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.catalog_fingerprint ^= 1;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::CatalogFingerprintMismatch { .. })
        ));
    }

    #[test]
    fn state_and_save_versions_match_mvp_014() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.version, SAVE_VERSION);
        assert_eq!(save.state.version, GAME_STATE_VERSION);
    }
}

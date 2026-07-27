// MVP-017: persist construction, research, craft and active ruleset identity.
use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, PlanetId, ResourceLedger, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId, generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, ConstructionQueue, CraftInventory, CraftQueue, FactionKind,
    FactionState, GameState, PlanetKnowledge, PlanetResourceProfile, ProductionRemainder,
    ProductionRemainderError, ResearchState, SelectionTarget, Simulation, SimulationBuildError,
    StrategicClock, StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed,
    default_ruleset, production_refresh_ticks,
};

pub const SAVE_VERSION: u32 = 12;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    pub ruleset_id: String,
    pub ruleset_schema_version: u32,
    pub ruleset_content_version: u32,
    pub ruleset_structure_fingerprint: u64,
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
    pub research: ResearchState,
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
    pub craft_queue: CraftQueue,
    pub inventory: CraftInventory,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    UnsupportedVersion(u32),
    RulesetIdMismatch,
    RulesetSchemaVersionMismatch {
        expected: u32,
        found: u32,
    },
    RulesetStructureMismatch {
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
    let ruleset = default_ruleset();

    SaveGame {
        version: SAVE_VERSION,
        ruleset_id: ruleset.id().to_string(),
        ruleset_schema_version: ruleset.schema_version(),
        ruleset_content_version: ruleset.content_version(),
        ruleset_structure_fingerprint: ruleset.structure_fingerprint(),
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
                    craft_queue: colony.craft_queue.clone(),
                    inventory: colony.inventory.clone(),
                    buildings: colony.buildings,
                    resource_profile: colony.resource_profile,
                })
                .collect(),
            research: state.research.clone(),
        },
    }
}

pub fn restore_from_snapshot(save: &SaveGame) -> Result<Simulation, SaveError> {
    if save.version != SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion(save.version));
    }

    let ruleset = default_ruleset();
    if save.ruleset_id != ruleset.id() {
        return Err(SaveError::RulesetIdMismatch);
    }
    if save.ruleset_schema_version != ruleset.schema_version() {
        return Err(SaveError::RulesetSchemaVersionMismatch {
            expected: ruleset.schema_version(),
            found: save.ruleset_schema_version,
        });
    }
    if save.ruleset_structure_fingerprint != ruleset.structure_fingerprint() {
        return Err(SaveError::RulesetStructureMismatch {
            expected: ruleset.structure_fingerprint(),
            found: save.ruleset_structure_fingerprint,
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
            if u64::from(colony.production_pending_ticks) >= production_refresh_ticks() {
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
                craft_queue: colony.craft_queue.clone(),
                inventory: colony.inventory.clone(),
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
        research: save.state.research.clone(),
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
    use galactic_sim::{
        BuildingKind, CraftableId, GAME_STATE_VERSION, GameCommand, TechnologyId,
        default_building_catalog,
    };

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
            kind: BuildingKind::METAL_MINE,
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
    fn research_queue_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);

        simulation.apply_command(GameCommand::QueueResearch {
            technology: TechnologyId::SPATIAL_DETECTION,
        });
        simulation.advance(Duration::from_secs(12));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("research save is compatible");

        assert_eq!(restored.state().research, simulation.state().research,);
        assert_eq!(restored.state(), simulation.state(),);
    }

    #[test]
    fn catalog_changes_are_detected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.ruleset_structure_fingerprint ^= 1;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::RulesetStructureMismatch { .. })
        ));
    }

    #[test]
    fn craft_queue_and_inventory_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony
            .buildings
            .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
        colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
        colony
            .buildings
            .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
        colony.buildings.set_level(BuildingKind::SHIPYARD, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        colony
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("resource credit fits");
        simulation.state_mut().research =
            ResearchState::from_completed([TechnologyId::SPATIAL_DETECTION]);
        let colony_id = simulation.state().colonies[0].id;
        simulation.apply_command(GameCommand::QueueCraft {
            colony_id,
            craftable: CraftableId::LIGHT_PROBE,
        });
        simulation.advance(Duration::from_secs(12));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("craft save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .craft_queue
                .len(),
            1,
        );
    }

    #[test]
    fn state_and_save_versions_match_mvp_017() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.version, SAVE_VERSION);
        assert_eq!(save.state.version, GAME_STATE_VERSION);
    }
}

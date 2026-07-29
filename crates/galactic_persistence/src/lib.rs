// MVP-023: persist reconnaissance results and progressive discovery frontiers.
use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, FleetId, Owner, PlanetId, ResourceLedger, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId, generate_universe,
};
use galactic_sim::{
    BuildingLevels, ColonyState, ConstructionQueue, CraftInventory, CraftQueue, DiplomacyState,
    FactionData, FactionKind, FleetAssignment, FleetComposition, FleetLocation, FleetState,
    GameState, MissionReport, MissionState, PlanetAnalysisReport, PlanetKnowledge,
    PlanetResourceProfile, ProductionRemainder, ProductionRemainderError, ResearchState,
    SelectionTarget, Simulation, SimulationBuildError, StrategicClock, StrategicClockError,
    StrategicTick, SystemKnowledge, TimeSpeed, default_ruleset, production_refresh_ticks,
};

/// Version 20 persists dated planetary analysis reports.
pub const SAVE_VERSION: u32 = 20;

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
    pub diplomacy: DiplomacyState,
    pub player_faction: FactionId,
    pub clock: StrategicClockSave,
    pub selected: SelectionTarget,
    pub system_knowledge: Vec<SystemKnowledge>,
    pub planet_knowledge: Vec<PlanetKnowledge>,
    pub colonies: Vec<ColonySave>,
    pub fleets: Vec<FleetSave>,
    pub next_fleet_id: u64,
    pub missions: Vec<MissionState>,
    pub next_mission_id: u64,
    pub mission_reports: Vec<MissionReport>,
    pub planet_analysis_reports: Vec<PlanetAnalysisReport>,
    pub research: ResearchState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactionSave {
    pub id: FactionId,
    pub name: String,
    pub kind: FactionKind,
    pub active: bool,
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
    pub owner: Owner,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetSave {
    pub id: FleetId,
    pub owner: Owner,
    pub location: FleetLocation,
    pub composition: FleetComposition,
    pub cargo: ResourceStock,
    pub assignment: FleetAssignment,
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
                    active: faction.active,
                })
                .collect(),
            diplomacy: state.diplomacy.clone(),
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
                    owner: colony.owner,
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
            fleets: state
                .fleets
                .iter()
                .map(|fleet| FleetSave {
                    id: fleet.id,
                    owner: fleet.owner,
                    location: fleet.location,
                    composition: fleet.composition.clone(),
                    cargo: fleet.cargo,
                    assignment: fleet.assignment,
                })
                .collect(),
            next_fleet_id: state.next_fleet_id,
            missions: state.missions.clone(),
            next_mission_id: state.next_mission_id,
            mission_reports: state.mission_reports.clone(),
            planet_analysis_reports: state.planet_analysis_reports.clone(),
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
                owner: colony.owner,
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
            .map(|faction| FactionData {
                id: faction.id,
                name: faction.name.clone(),
                kind: faction.kind,
                active: faction.active,
            })
            .collect(),
        diplomacy: save.state.diplomacy.clone(),
        player_faction: save.state.player_faction,
        colonies,
        fleets: save
            .state
            .fleets
            .iter()
            .map(|fleet| FleetState {
                id: fleet.id,
                owner: fleet.owner,
                location: fleet.location,
                composition: fleet.composition.clone(),
                cargo: fleet.cargo,
                assignment: fleet.assignment,
            })
            .collect(),
        next_fleet_id: save.state.next_fleet_id,
        missions: save.state.missions.clone(),
        next_mission_id: save.state.next_mission_id,
        mission_reports: save.state.mission_reports.clone(),
        planet_analysis_reports: save.state.planet_analysis_reports.clone(),
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
        BuildingKind, CraftableId, FleetComposition, GAME_STATE_VERSION, GameAction,
        KnowledgeLevel, MissionKind, MissionOrder, MissionPhase, MissionResult, MissionTarget,
        ShipStack, TechnologyId, default_building_catalog,
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
        simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
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

        simulation.apply_player_action(GameAction::QueueResearch {
            technology: TechnologyId::SPATIAL_DETECTION,
        });
        simulation.advance(Duration::from_secs(12));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("research save is compatible");

        assert_eq!(restored.state().research, simulation.state().research,);
        assert_eq!(restored.state(), simulation.state(),);
    }

    #[test]
    fn planetary_analysis_report_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, _) = simulation
            .universe_repository()
            .planet_location(planet_id)
            .expect("detected planet exists");
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.apply_player_action(GameAction::AnalyzePlanet { planet_id });

        let report = *simulation
            .state()
            .planet_analysis_report(planet_id)
            .expect("analysis creates a persistent report");
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("analysis save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored.state().planet_analysis_report(planet_id),
            Some(&report)
        );
        assert_eq!(
            restored.state().planet_knowledge_level(planet_id),
            KnowledgeLevel::Analyzed
        );
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
        simulation.apply_player_action(GameAction::QueueCraft {
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
    fn faction_data_and_owner_survive_round_trip() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("faction save is compatible");

        assert_eq!(restored.state().factions, simulation.state().factions);
        assert_eq!(restored.state().diplomacy, simulation.state().diplomacy);
        assert_eq!(
            restored.state().colonies[0].owner,
            simulation.state().colonies[0].owner,
        );
        assert_eq!(restored.state().factions.len(), 3);
    }

    #[test]
    fn fleets_and_docked_inventory_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let colony = &mut simulation.state_mut().colonies[0];
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
            .expect("test funding fits");
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
        ]);
        for _ in 0..2 {
            let events = simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::LIGHT_CARGO,
            });
            assert!(matches!(
                events.as_slice(),
                [galactic_sim::GameEvent {
                    kind: galactic_sim::GameEventKind::CraftQueued(_),
                    ..
                }]
            ));
        }
        simulation.advance(Duration::from_secs(160));
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("composition is valid");
        simulation.apply_player_action(GameAction::FormFleet {
            colony_id,
            composition,
        });

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("fleet save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().player_fleets().count(), 1);
        assert_eq!(restored.state().fleets[0].owner, Owner::Faction(actor));
        assert_eq!(
            restored.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            1,
        );
    }

    #[test]
    fn active_mission_resumes_and_completes_at_the_same_tick() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .into_iter()
            .find(|candidate| {
                simulation
                    .universe_repository()
                    .neighboring_systems(*candidate)
                    .into_iter()
                    .any(|neighbor| {
                        simulation.state().system_knowledge_level(neighbor)
                            == KnowledgeLevel::Unknown
                    })
            })
            .expect("one initial signal opens a new frontier");
        let expected_frontier = simulation
            .universe_repository()
            .neighboring_systems(target)
            .into_iter()
            .filter(|neighbor| {
                simulation.state().system_knowledge_level(*neighbor) == KnowledgeLevel::Unknown
            })
            .collect::<Vec<_>>();
        let colony = &mut simulation.state_mut().colonies[0];
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
            .expect("test funding fits");
        simulation.state_mut().research =
            ResearchState::from_completed([TechnologyId::SPATIAL_DETECTION]);
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::LIGHT_PROBE,
        });
        simulation.advance(Duration::from_secs(50));
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("probe composition is valid");
        let actor = simulation.state().player_faction;
        let created =
            galactic_sim::form_fleet(simulation.state_mut(), actor, colony_id, composition)
                .expect("probe fleet can be formed");
        let departure_at = simulation.state().clock.current_tick();
        simulation.apply_player_action(GameAction::LaunchMission(MissionOrder {
            fleet_id: created.fleet_id,
            origin,
            target: MissionTarget::System(target),
            kind: MissionKind::Probe,
            departure_at,
        }));
        simulation.advance(Duration::from_secs(5));
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::Outbound);

        let save = snapshot_from_simulation(&simulation);
        let mut restored = restore_from_snapshot(&save).expect("active mission save is compatible");
        assert_eq!(restored.state(), simulation.state());

        simulation.advance(Duration::from_secs(16));
        restored.advance(Duration::from_secs(16));

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().missions[0].phase, MissionPhase::Completed);
        assert_eq!(
            restored.state().mission_reports[0].occurred_at,
            StrategicTick::new(701),
        );
        assert_eq!(
            restored.state().system_knowledge_level(target),
            KnowledgeLevel::Probed,
        );
        let Some(MissionResult::Probe(result)) = restored.state().mission_reports[0].result else {
            panic!("completed reconnaissance keeps its frontier result");
        };
        assert_eq!(result.target, MissionTarget::System(target));
        assert_eq!(result.current, KnowledgeLevel::Probed);
        assert_eq!(
            usize::from(result.newly_detected_systems),
            expected_frontier.len(),
        );
        assert_eq!(usize::from(result.revealed_routes), expected_frontier.len(),);
        assert!(expected_frontier.iter().all(|system_id| {
            restored.state().system_knowledge_level(*system_id) == KnowledgeLevel::Detected
        }));
    }

    #[test]
    fn state_and_save_versions_match_mvp_024() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.version, SAVE_VERSION);
        assert_eq!(save.state.version, GAME_STATE_VERSION);
    }
}

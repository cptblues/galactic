use galactic_domain::{EnergyGrid, ResourceLedger, UniverseConfig, generate_universe};
use galactic_sim::{
    ColonyState, FactionData, FleetState, GameState, ProductionRemainder, Simulation,
    StrategicClock, default_ruleset, production_refresh_ticks,
};

use crate::save::{SAVE_VERSION, SaveError, SaveGame};

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
                founding_mission_id: colony.founding_mission_id,
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
        next_colony_id: save.state.next_colony_id,
        active_colony_id: save.state.active_colony_id,
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
        combat_reports: save.state.combat_reports.clone(),
        colony_foundations: save.state.colony_foundations.clone(),
        planet_analysis_reports: save.state.planet_analysis_reports.clone(),
        extraction_sites: save.state.extraction_sites.clone(),
        planetary_presences: save.state.planetary_presences.clone(),
        planetary_intelligence_reports: save.state.planetary_intelligence_reports.clone(),
        research: save.state.research.clone(),
        system_knowledge: save.state.system_knowledge.clone(),
        planet_knowledge: save.state.planet_knowledge.clone(),
        selected: save.state.selected,
        clock,
    };

    Simulation::from_parts(universe, state).map_err(SaveError::InvalidState)
}

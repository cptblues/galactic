use galactic_sim::{Simulation, default_ruleset};

use crate::save::{ColonySave, FactionSave, FleetSave, MutableGameSave, SAVE_VERSION, SaveGame};
use crate::save::{StrategicClockSave, UniverseReference};

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
                    founding_mission_id: colony.founding_mission_id,
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
            next_colony_id: state.next_colony_id,
            active_colony_id: state.active_colony_id,
            fleets: state
                .fleets
                .iter()
                .map(|fleet| FleetSave {
                    id: fleet.id,
                    name: fleet.name.clone(),
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
            combat_reports: state.combat_reports.clone(),
            pending_combats: state.pending_combats.clone(),
            colony_foundations: state.colony_foundations.clone(),
            planet_analysis_reports: state.planet_analysis_reports.clone(),
            extraction_sites: state.extraction_sites.clone(),
            planetary_presences: state.planetary_presences.clone(),
            planetary_intelligence_reports: state.planetary_intelligence_reports.clone(),
            research: state.research.clone(),
        },
    }
}

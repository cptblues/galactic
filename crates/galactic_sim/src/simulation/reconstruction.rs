use std::collections::HashSet;

use galactic_domain::{Owner, ResourceStock};

use crate::{
    AttackMissionOutcome, ColonizationMissionOutcome, CombatReportStatus, DiplomacyState,
    FactionKind, FleetAssignment, GAME_STATE_VERSION, GameState, KnowledgeLevel, MissionKind,
    MissionPhase, MissionResult, SelectionTarget, UniverseRepository, default_building_catalog,
    storage_capacity, validate_colony_foundations, validate_construction_queue,
    validate_craft_state, validate_extraction_sites, validate_fleet_state, validate_mission_state,
    validate_planet_analysis_state, validate_planetary_presence_state, validate_research_state,
};

use super::build_error::SimulationBuildError;

pub(crate) fn validate_state(
    universe: &UniverseRepository,
    state: &GameState,
) -> Result<(), SimulationBuildError> {
    if state.version != GAME_STATE_VERSION {
        return Err(SimulationBuildError::UnsupportedStateVersion {
            expected: GAME_STATE_VERSION,
            found: state.version,
        });
    }

    let mut faction_ids = HashSet::with_capacity(state.factions.len());
    for faction in &state.factions {
        if !faction_ids.insert(faction.id) {
            return Err(SimulationBuildError::DuplicateFaction(faction.id));
        }
    }

    let Some(player_faction) = state.faction(state.player_faction) else {
        return Err(SimulationBuildError::UnknownPlayerFaction(
            state.player_faction,
        ));
    };
    if player_faction.kind != FactionKind::Player {
        return Err(SimulationBuildError::PlayerFactionIsNotPlayer(
            state.player_faction,
        ));
    }
    if !player_faction.active {
        return Err(SimulationBuildError::PlayerFactionIsInactive(
            state.player_faction,
        ));
    }

    DiplomacyState::new(
        state.diplomacy.default_relation(),
        state.diplomacy.relations().iter().copied(),
    )
    .map_err(SimulationBuildError::InvalidDiplomacy)?;
    for relation in state.diplomacy.relations() {
        if state.faction(relation.first).is_none() {
            return Err(SimulationBuildError::UnknownRelationFaction(relation.first));
        }
        if state.faction(relation.second).is_none() {
            return Err(SimulationBuildError::UnknownRelationFaction(
                relation.second,
            ));
        }
    }

    if let Err(error) = validate_research_state(state) {
        return Err(SimulationBuildError::InvalidResearchState(error));
    }

    let mut system_knowledge_ids = HashSet::with_capacity(state.system_knowledge.len());
    for knowledge in &state.system_knowledge {
        if !system_knowledge_ids.insert(knowledge.system_id) {
            return Err(SimulationBuildError::DuplicateSystemKnowledge(
                knowledge.system_id,
            ));
        }
        if knowledge.level == KnowledgeLevel::Unknown {
            return Err(SimulationBuildError::ExplicitUnknownSystemKnowledge(
                knowledge.system_id,
            ));
        }
        if universe.system(knowledge.system_id).is_none() {
            return Err(SimulationBuildError::UnknownKnowledgeSystem(
                knowledge.system_id,
            ));
        }
    }

    let mut planet_knowledge_ids = HashSet::with_capacity(state.planet_knowledge.len());
    for knowledge in &state.planet_knowledge {
        if !planet_knowledge_ids.insert(knowledge.planet_id) {
            return Err(SimulationBuildError::DuplicatePlanetKnowledge(
                knowledge.planet_id,
            ));
        }
        if knowledge.level == KnowledgeLevel::Unknown {
            return Err(SimulationBuildError::ExplicitUnknownPlanetKnowledge(
                knowledge.planet_id,
            ));
        }
        if universe.planet(knowledge.planet_id).is_none() {
            return Err(SimulationBuildError::UnknownKnowledgePlanet(
                knowledge.planet_id,
            ));
        }
    }

    validate_planet_analysis_state(state, universe)
        .map_err(SimulationBuildError::InvalidPlanetAnalysisState)?;
    validate_extraction_sites(state, universe)
        .map_err(SimulationBuildError::InvalidExtractionSiteState)?;

    let mut colony_ids = HashSet::with_capacity(state.colonies.len());
    let mut colony_planet_ids = HashSet::with_capacity(state.colonies.len());
    for colony in &state.colonies {
        if !colony_ids.insert(colony.id) {
            return Err(SimulationBuildError::DuplicateColony(colony.id));
        }
        if !colony_planet_ids.insert(colony.planet_id) {
            return Err(SimulationBuildError::DuplicateColonyPlanet(
                colony.planet_id,
            ));
        }
        if colony.id.raw() >= state.next_colony_id {
            return Err(SimulationBuildError::InvalidNextColonyId {
                next_colony_id: state.next_colony_id,
                existing_colony_id: colony.id,
            });
        }
        if colony.name.trim().is_empty() {
            return Err(SimulationBuildError::EmptyColonyName(colony.id));
        }
        if let Err(error) = default_building_catalog().validate_levels(colony.buildings) {
            return Err(SimulationBuildError::InvalidColonyBuildings {
                colony_id: colony.id,
                error,
            });
        }
        if colony.energy != default_building_catalog().energy_grid_for_levels(colony.buildings) {
            return Err(SimulationBuildError::InvalidColonyEnergy(colony.id));
        }
        if u64::from(colony.production_pending_ticks) >= crate::production_refresh_ticks() {
            return Err(SimulationBuildError::InvalidProductionWindow {
                colony_id: colony.id,
                pending_ticks: colony.production_pending_ticks,
            });
        }
        if let Err(error) = validate_construction_queue(colony) {
            return Err(SimulationBuildError::InvalidConstructionQueue {
                colony_id: colony.id,
                error,
            });
        }
        if let Err(error) = validate_craft_state(state, colony) {
            return Err(SimulationBuildError::InvalidCraftState {
                colony_id: colony.id,
                error,
            });
        }
        if let Err(error) = colony.resources.validate() {
            return Err(SimulationBuildError::InvalidColonyResourceLedger {
                colony_id: colony.id,
                error,
            });
        }
        let capacity = storage_capacity(colony.buildings);
        let stock = colony.resources.stock();
        if !stock.is_within(capacity) {
            return Err(SimulationBuildError::ColonyStockExceedsCapacity {
                colony_id: colony.id,
                stock,
                capacity,
            });
        }
        match colony.owner {
            Owner::Unowned => {
                return Err(SimulationBuildError::UnownedColony(colony.id));
            }
            Owner::Faction(faction_id) if state.faction(faction_id).is_none() => {
                return Err(SimulationBuildError::UnknownColonyFaction {
                    colony_id: colony.id,
                    faction_id,
                });
            }
            Owner::Faction(_) => {}
        }
        if universe.system(colony.system_id).is_none() {
            return Err(SimulationBuildError::UnknownColonySystem {
                colony_id: colony.id,
                system_id: colony.system_id,
            });
        }
        let Some((planet_system_id, _)) = universe.planet_location(colony.planet_id) else {
            return Err(SimulationBuildError::UnknownColonyPlanet {
                colony_id: colony.id,
                planet_id: colony.planet_id,
            });
        };
        if planet_system_id != colony.system_id {
            return Err(SimulationBuildError::ColonyPlanetSystemMismatch {
                colony_id: colony.id,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
            });
        }
        if state.system_knowledge_level(colony.system_id) != KnowledgeLevel::Colonized {
            return Err(SimulationBuildError::ColonySystemNotColonized {
                colony_id: colony.id,
                system_id: colony.system_id,
            });
        }
        if state.planet_knowledge_level(colony.planet_id) != KnowledgeLevel::Colonized {
            return Err(SimulationBuildError::ColonyPlanetNotColonized {
                colony_id: colony.id,
                planet_id: colony.planet_id,
            });
        }
    }

    match state.active_colony_id {
        None if state.player_colonies().next().is_some() => {
            return Err(SimulationBuildError::MissingActivePlayerColony);
        }
        None => {}
        Some(colony_id) => {
            let colony = state
                .colony(colony_id)
                .ok_or(SimulationBuildError::UnknownActiveColony(colony_id))?;
            state
                .authorize_management(state.player_faction, colony.owner)
                .map_err(|error| SimulationBuildError::InvalidActiveColonyAccess {
                    colony_id,
                    error,
                })?;
        }
    }

    validate_planetary_presence_state(state, universe)
        .map_err(SimulationBuildError::InvalidPlanetaryPresenceState)?;

    let mut fleet_ids = HashSet::with_capacity(state.fleets.len());
    for fleet in &state.fleets {
        if !fleet_ids.insert(fleet.id) {
            return Err(SimulationBuildError::DuplicateFleet(fleet.id));
        }
        if fleet.id.raw() >= state.next_fleet_id {
            return Err(SimulationBuildError::InvalidNextFleetId {
                next_fleet_id: state.next_fleet_id,
                existing_fleet_id: fleet.id,
            });
        }
        if let Err(error) = validate_fleet_state(fleet) {
            return Err(SimulationBuildError::InvalidFleetState {
                fleet_id: fleet.id,
                error,
            });
        }
        match fleet.owner {
            Owner::Unowned => {
                return Err(SimulationBuildError::UnownedFleet(fleet.id));
            }
            Owner::Faction(faction_id) if state.faction(faction_id).is_none() => {
                return Err(SimulationBuildError::UnknownFleetFaction {
                    fleet_id: fleet.id,
                    faction_id,
                });
            }
            Owner::Faction(_) => {}
        }
        match fleet.location {
            crate::FleetLocation::Docked(colony_id) => {
                let Some(colony) = state.colony(colony_id) else {
                    return Err(SimulationBuildError::UnknownFleetColony {
                        fleet_id: fleet.id,
                        colony_id,
                    });
                };
                if colony.owner != fleet.owner {
                    return Err(SimulationBuildError::DockedFleetOwnerMismatch {
                        fleet_id: fleet.id,
                        colony_id,
                    });
                }
            }
            crate::FleetLocation::InSystem(system_id) => {
                if universe.system(system_id).is_none() {
                    return Err(SimulationBuildError::UnknownFleetSystem {
                        fleet_id: fleet.id,
                        system_id,
                    });
                }
            }
        }
    }

    let mut mission_ids = HashSet::with_capacity(state.missions.len());
    for mission in &state.missions {
        if !mission_ids.insert(mission.id) {
            return Err(SimulationBuildError::DuplicateMission(mission.id));
        }
        if mission.id.raw() >= state.next_mission_id {
            return Err(SimulationBuildError::InvalidNextMissionId {
                next_mission_id: state.next_mission_id,
                existing_mission_id: mission.id,
            });
        }
        let has_pending_combat = state
            .pending_combats
            .iter()
            .any(|pending| pending.mission_id == mission.id);
        if let Err(error) = validate_mission_state(mission, universe, has_pending_combat) {
            return Err(SimulationBuildError::InvalidMissionState {
                mission_id: mission.id,
                error,
            });
        }
        match mission.owner {
            Owner::Unowned => {
                return Err(SimulationBuildError::UnownedMission(mission.id));
            }
            Owner::Faction(faction_id) if state.faction(faction_id).is_none() => {
                return Err(SimulationBuildError::UnknownMissionFaction {
                    mission_id: mission.id,
                    faction_id,
                });
            }
            Owner::Faction(_) => {}
        }
        let destroyed_attacker = matches!(
            mission.result,
            Some(MissionResult::Attack(crate::AttackMissionResult {
                attackers_destroyed: true,
                ..
            }))
        ) && mission.phase == MissionPhase::Failed;
        let consumed_colony_ship = matches!(
            mission.result,
            Some(MissionResult::Colonize(crate::ColonizationMissionResult {
                outcome: ColonizationMissionOutcome::FoundationPrepared,
                colony_ship_consumed: true,
                ..
            }))
        ) && mission.phase == MissionPhase::Completed;
        let fleet = state.fleet(mission.order.fleet_id);
        if fleet.is_none() && !destroyed_attacker && !consumed_colony_ship {
            return Err(SimulationBuildError::UnknownMissionFleet {
                mission_id: mission.id,
                fleet_id: mission.order.fleet_id,
            });
        }
        if let Some(fleet) = fleet {
            if fleet.owner != mission.owner {
                return Err(SimulationBuildError::MissionFleetOwnerMismatch {
                    mission_id: mission.id,
                    fleet_id: fleet.id,
                });
            }
            if !mission.phase.is_terminal()
                && fleet.assignment != FleetAssignment::Mission(mission.id)
            {
                return Err(SimulationBuildError::MissionFleetAssignmentMismatch {
                    mission_id: mission.id,
                    fleet_id: fleet.id,
                });
            }
            if let Some(transport) = mission.transport {
                let expected_cargo = match mission.phase {
                    MissionPhase::Preparation | MissionPhase::Cancelled => ResourceStock::ZERO,
                    MissionPhase::Outbound => transport.cargo,
                    MissionPhase::OnSite | MissionPhase::Returning => {
                        transport.cargo.saturating_sub(transport.delivered)
                    }
                    MissionPhase::Completed | MissionPhase::Failed => match mission.result {
                        Some(MissionResult::Transport(result)) => result.retained,
                        _ => ResourceStock::ZERO,
                    },
                };
                if fleet.cargo != expected_cargo {
                    return Err(SimulationBuildError::MissionFleetCargoMismatch {
                        mission_id: mission.id,
                        fleet_id: fleet.id,
                        expected: expected_cargo,
                        found: fleet.cargo,
                    });
                }
            } else if let Some(harvest) = mission.harvest {
                let expected_cargo = match mission.phase {
                    MissionPhase::Preparation
                    | MissionPhase::Outbound
                    | MissionPhase::OnSite
                    | MissionPhase::Cancelled => ResourceStock::ZERO,
                    MissionPhase::Returning => harvest.collected,
                    MissionPhase::Completed | MissionPhase::Failed => match mission.result {
                        Some(MissionResult::Harvest(result)) => result.retained,
                        _ => ResourceStock::ZERO,
                    },
                };
                if fleet.cargo != expected_cargo {
                    return Err(SimulationBuildError::MissionFleetCargoMismatch {
                        mission_id: mission.id,
                        fleet_id: fleet.id,
                        expected: expected_cargo,
                        found: fleet.cargo,
                    });
                }
            }
        }
        let expected_location = match mission.phase {
            MissionPhase::Preparation => {
                Some(crate::FleetLocation::Docked(mission.origin_colony_id))
            }
            MissionPhase::Outbound => Some(crate::FleetLocation::InSystem(mission.order.origin)),
            MissionPhase::OnSite | MissionPhase::Returning => Some(crate::FleetLocation::InSystem(
                mission.order.target.system_id(),
            )),
            MissionPhase::Completed | MissionPhase::Cancelled | MissionPhase::Failed => None,
        };
        if expected_location
            .is_some_and(|expected| fleet.is_none_or(|fleet| fleet.location != expected))
        {
            return Err(SimulationBuildError::MissionFleetLocationMismatch {
                mission_id: mission.id,
                fleet_id: mission.order.fleet_id,
            });
        }
        let Some(origin_colony) = state.colony(mission.origin_colony_id) else {
            return Err(SimulationBuildError::UnknownMissionOriginColony {
                mission_id: mission.id,
                colony_id: mission.origin_colony_id,
            });
        };
        if origin_colony.system_id != mission.order.origin {
            return Err(SimulationBuildError::MissionOriginMismatch {
                mission_id: mission.id,
                colony_id: mission.origin_colony_id,
                system_id: mission.order.origin,
            });
        }
        if mission.phase == MissionPhase::Preparation {
            let Some(reservation_id) = mission.fuel_reservation else {
                return Err(SimulationBuildError::MissingMissionFuelReservation {
                    mission_id: mission.id,
                });
            };
            let Some(reservation) = origin_colony
                .resources
                .reservations()
                .iter()
                .find(|reservation| reservation.id == reservation_id)
            else {
                return Err(SimulationBuildError::MissingMissionFuelReservation {
                    mission_id: mission.id,
                });
            };
            if reservation.cost != mission.plan.fuel_cost {
                return Err(SimulationBuildError::MissionFuelReservationMismatch {
                    mission_id: mission.id,
                });
            }
        }
        if let Some(reservation_id) = mission.foundation_reservation {
            let Some(reservation) = origin_colony
                .resources
                .reservations()
                .iter()
                .find(|reservation| reservation.id == reservation_id)
            else {
                return Err(SimulationBuildError::MissingMissionFoundationReservation {
                    mission_id: mission.id,
                });
            };
            let Some(commitment) = mission.colonization else {
                return Err(SimulationBuildError::MissionFoundationReservationMismatch {
                    mission_id: mission.id,
                });
            };
            if reservation.cost != commitment.foundation_cost {
                return Err(SimulationBuildError::MissionFoundationReservationMismatch {
                    mission_id: mission.id,
                });
            }
        }
        if let Some(reservation_id) = mission.cargo_reservation {
            let Some(reservation) = origin_colony
                .resources
                .reservations()
                .iter()
                .find(|reservation| reservation.id == reservation_id)
            else {
                return Err(SimulationBuildError::MissingMissionCargoReservation {
                    mission_id: mission.id,
                });
            };
            let Some(transport) = mission.transport else {
                return Err(SimulationBuildError::MissionCargoReservationMismatch {
                    mission_id: mission.id,
                });
            };
            if reservation.cost != transport.cargo.into() {
                return Err(SimulationBuildError::MissionCargoReservationMismatch {
                    mission_id: mission.id,
                });
            }
        }
        if let Some(harvest) = mission.harvest {
            let site = state.extraction_site(harvest.site_id).ok_or(
                SimulationBuildError::HarvestSiteReservationMismatch {
                    mission_id: mission.id,
                },
            )?;
            let reservation_expected = crate::extraction_rules().reserves_sites()
                && matches!(
                    mission.phase,
                    MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::OnSite
                );
            if reservation_expected != (site.reserved_by == Some(mission.id)) {
                return Err(SimulationBuildError::HarvestSiteReservationMismatch {
                    mission_id: mission.id,
                });
            }
        }
    }

    for site in &state.extraction_sites {
        let Some(mission_id) = site.reserved_by else {
            continue;
        };
        let Some(mission) = state.mission(mission_id) else {
            return Err(
                SimulationBuildError::ExtractionSiteReservedByUnknownMission {
                    site_id: site.id,
                    mission_id,
                },
            );
        };
        if mission.order.kind != MissionKind::Harvest
            || mission
                .harvest
                .is_none_or(|harvest| harvest.site_id != site.id)
            || !matches!(
                mission.phase,
                MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::OnSite
            )
        {
            return Err(
                SimulationBuildError::ExtractionSiteReservationMissionMismatch {
                    site_id: site.id,
                    mission_id,
                },
            );
        }
    }

    for fleet in &state.fleets {
        let FleetAssignment::Mission(mission_id) = fleet.assignment else {
            continue;
        };
        let Some(mission) = state.mission(mission_id) else {
            return Err(SimulationBuildError::FleetAssignedToUnknownMission {
                fleet_id: fleet.id,
                mission_id,
            });
        };
        if mission.order.fleet_id != fleet.id || mission.phase.is_terminal() {
            return Err(SimulationBuildError::FleetAssignedToDifferentMission {
                fleet_id: fleet.id,
                mission_id,
            });
        }
    }

    let mut reported_missions = HashSet::with_capacity(state.mission_reports.len());
    for report in &state.mission_reports {
        if !reported_missions.insert(report.mission_id) {
            return Err(SimulationBuildError::DuplicateMissionReport(
                report.mission_id,
            ));
        }
        if state.mission(report.mission_id).is_none() {
            return Err(SimulationBuildError::MissionReportWithoutMission(
                report.mission_id,
            ));
        }
    }

    let mut combat_missions = HashSet::with_capacity(state.combat_reports.len());
    for report in &state.combat_reports {
        if !combat_missions.insert(report.mission_id) {
            return Err(SimulationBuildError::DuplicateCombatReport(
                report.mission_id,
            ));
        }
        let Some(mission) = state.mission(report.mission_id) else {
            return Err(SimulationBuildError::CombatReportWithoutMission(
                report.mission_id,
            ));
        };
        let expected_planet = mission.order.target.planet_id();
        let Some(MissionResult::Attack(result)) = mission.result else {
            return Err(SimulationBuildError::CombatReportMissionMismatch(
                report.mission_id,
            ));
        };
        let summary_matches = match (&report.status, result.outcome) {
            (CombatReportStatus::Resolved(resolution), AttackMissionOutcome::Resolved(outcome)) => {
                resolution.outcome == outcome
                    && result.attackers_destroyed == resolution.attacker_survivors.is_empty()
                    && result.secured
                        == matches!(
                            resolution.control,
                            crate::CombatControlChange::Secured { .. }
                        )
            }
            (
                CombatReportStatus::TargetInvalid(found),
                AttackMissionOutcome::TargetInvalid(expected),
            ) => found == &expected && !result.attackers_destroyed && !result.secured,
            _ => false,
        };
        if mission.order.kind != MissionKind::Attack
            || expected_planet != Some(report.planet_id)
            || result.target != report.planet_id
            || !summary_matches
        {
            return Err(SimulationBuildError::CombatReportMissionMismatch(
                report.mission_id,
            ));
        }
        if report.resolved_at > state.clock.current_tick() {
            return Err(SimulationBuildError::CombatReportInFuture(
                report.mission_id,
            ));
        }
    }
    for mission in &state.missions {
        if mission.order.kind == MissionKind::Attack
            && mission.result.is_some()
            && !combat_missions.contains(&mission.id)
        {
            return Err(SimulationBuildError::MissingCombatReport(mission.id));
        }
    }

    let mut pending_combat_missions = HashSet::with_capacity(state.pending_combats.len());
    for pending in &state.pending_combats {
        if !pending_combat_missions.insert(pending.mission_id) {
            return Err(SimulationBuildError::DuplicatePendingCombat(
                pending.mission_id,
            ));
        }
        let Some(mission) = state.mission(pending.mission_id) else {
            return Err(SimulationBuildError::PendingCombatWithoutMission(
                pending.mission_id,
            ));
        };
        if mission.phase.is_terminal() {
            return Err(SimulationBuildError::PendingCombatForTerminalMission(
                pending.mission_id,
            ));
        }
        if pending.is_completed() {
            return Err(SimulationBuildError::PendingCombatAlreadyCompleted(
                pending.mission_id,
            ));
        }
        if pending.round() > pending.maximum_rounds() {
            return Err(SimulationBuildError::PendingCombatRoundExceedsMaximum(
                pending.mission_id,
            ));
        }
        if !pending.has_unique_stack_ids() {
            return Err(SimulationBuildError::PendingCombatStackIdCollision(
                pending.mission_id,
            ));
        }
        if !pending.every_stack_hull_within_bounds() {
            return Err(SimulationBuildError::PendingCombatHullExceedsMaximum(
                pending.mission_id,
            ));
        }
        if !pending.command_points_are_valid() {
            return Err(
                SimulationBuildError::PendingCombatCommandPointsExceedMaximum(pending.mission_id),
            );
        }
    }

    validate_colony_foundations(state, universe)
        .map_err(SimulationBuildError::InvalidColonyFoundationState)?;

    match state.selected {
        SelectionTarget::None => {}
        SelectionTarget::System(system_id) => {
            if universe.system(system_id).is_none() {
                return Err(SimulationBuildError::InvalidSelectedSystem(system_id));
            }
        }
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => {
            let Some((planet_system_id, _)) = universe.planet_location(planet_id) else {
                return Err(SimulationBuildError::InvalidSelectedPlanet {
                    system_id,
                    planet_id,
                });
            };
            if planet_system_id != system_id {
                return Err(SimulationBuildError::InvalidSelectedPlanet {
                    system_id,
                    planet_id,
                });
            }
        }
    }

    Ok(())
}

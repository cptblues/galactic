use galactic_domain::{ColonyId, ExtractionSiteId, FactionId, FleetId, ResourceStock};

use crate::{GameState, KnowledgeLevel, TechnologyUnlock, UniverseRepository};

use super::{
    MissionError, MissionKind, MissionLaunched, MissionOrder, MissionPhase, MissionResult,
    MissionState, MissionStateError, MissionTarget, launch_mission_with_payload,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarvestCollectionStatus {
    Pending,
    Collected,
    SiteDepleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarvestMissionState {
    pub site_id: ExtractionSiteId,
    pub collected: ResourceStock,
    pub site_remaining: u64,
    pub status: HarvestCollectionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarvestMissionResult {
    pub site_id: ExtractionSiteId,
    pub collected: ResourceStock,
    pub delivered: ResourceStock,
    pub retained: ResourceStock,
    pub site_remaining: u64,
    pub status: HarvestCollectionStatus,
}

pub(crate) fn validate_harvest_launch(
    state: &GameState,
    actor: FactionId,
    origin_colony_id: ColonyId,
    order: MissionOrder,
    harvest: HarvestMissionState,
) -> Result<(), MissionError> {
    if order.kind != MissionKind::Harvest
        || harvest.status != HarvestCollectionStatus::Pending
        || !harvest.collected.is_zero()
    {
        return Err(MissionError::HarvestOrderRequired);
    }
    let site = state
        .extraction_site(harvest.site_id)
        .ok_or(MissionError::UnknownExtractionSite(harvest.site_id))?;
    let expected_target = MissionTarget::Planet {
        system_id: site.system_id,
        planet_id: site.planet_id,
    };
    if order.target != expected_target {
        return Err(MissionError::HarvestTargetMismatch {
            site_id: site.id,
            target: order.target,
        });
    }
    let current = state.planet_knowledge_level(site.planet_id);
    if current < KnowledgeLevel::Analyzed {
        return Err(MissionError::HarvestPlanetNotAnalyzed {
            planet_id: site.planet_id,
            current,
        });
    }
    if !state
        .research
        .has_unlock(TechnologyUnlock::RemoteExtraction)
    {
        return Err(MissionError::MissingHarvestTechnology(
            TechnologyUnlock::RemoteExtraction,
        ));
    }
    if state.colony_on_planet(site.planet_id).is_some() {
        return Err(MissionError::ExtractionSiteOnColony(site.id));
    }
    if site.is_depleted() {
        return Err(MissionError::ExtractionSiteDepleted(site.id));
    }
    if let Some(mission_id) = site.reserved_by {
        return Err(MissionError::ExtractionSiteBusy {
            site_id: site.id,
            mission_id,
        });
    }
    if harvest.site_remaining != site.remaining {
        return Err(MissionError::HarvestOrderRequired);
    }
    let fleet = state
        .fleet(order.fleet_id)
        .ok_or(MissionError::UnknownFleet(order.fleet_id))?;
    if !fleet.cargo.is_zero() {
        return Err(MissionError::HarvestFleetHasCargo(fleet.id));
    }
    state
        .colony(origin_colony_id)
        .ok_or(MissionError::UnknownOriginColony(origin_colony_id))?;
    state
        .authorize_management(actor, fleet.owner)
        .map_err(MissionError::Access)?;
    Ok(())
}

/// Launches a cargo fleet toward one analyzed extraction site, with an
/// explicit fleet.
///
/// The site is reserved atomically with the mission. Collection occurs only
/// after the configured on-site duration, is capped by both the remaining
/// reserve and fleet capacity, and is credited at the origin after return.
pub fn launch_harvest_mission(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    origin_colony_id: ColonyId,
    fleet_id: FleetId,
    site_id: ExtractionSiteId,
) -> Result<MissionLaunched, MissionError> {
    let origin_colony = state
        .colony(origin_colony_id)
        .ok_or(MissionError::UnknownOriginColony(origin_colony_id))?;
    state
        .authorize_management(actor, origin_colony.owner)
        .map_err(MissionError::Access)?;
    let site = state
        .extraction_site(site_id)
        .ok_or(MissionError::UnknownExtractionSite(site_id))?;
    let target = MissionTarget::Planet {
        system_id: site.system_id,
        planet_id: site.planet_id,
    };
    let origin = origin_colony.system_id;
    let departure_at = state.clock.current_tick();
    let site_remaining = site.remaining;
    let mut candidate = state.clone();

    let launched = launch_mission_with_payload(
        &mut candidate,
        universe,
        actor,
        MissionOrder {
            fleet_id,
            origin,
            target,
            kind: MissionKind::Harvest,
            departure_at,
        },
        None,
        Some(HarvestMissionState {
            site_id,
            collected: ResourceStock::ZERO,
            site_remaining,
            status: HarvestCollectionStatus::Pending,
        }),
    )?;
    *state = candidate;
    Ok(launched)
}

pub(crate) fn validate_harvest_state(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.order.kind, mission.harvest) {
        (MissionKind::Harvest, Some(harvest)) => {
            let MissionTarget::Planet { planet_id, .. } = mission.order.target else {
                return Err(MissionStateError::HarvestPlanetTargetRequired);
            };
            let expected_site = ExtractionSiteId::for_planet(planet_id);
            if harvest.site_id != expected_site {
                return Err(MissionStateError::HarvestTargetMismatch {
                    expected: expected_site,
                    found: harvest.site_id,
                });
            }
            let pending_expected = matches!(
                mission.phase,
                MissionPhase::Preparation
                    | MissionPhase::Outbound
                    | MissionPhase::OnSite
                    | MissionPhase::Cancelled
            );
            if pending_expected != (harvest.status == HarvestCollectionStatus::Pending) {
                return Err(MissionStateError::InvalidHarvestStatus {
                    phase: mission.phase,
                    status: harvest.status,
                });
            }
            if pending_expected && !harvest.collected.is_zero() {
                return Err(MissionStateError::HarvestResultCargoMismatch);
            }
            if !pending_expected && harvest.collected.is_zero() {
                return Err(MissionStateError::EmptyHarvestCargo);
            }
            Ok(())
        }
        (MissionKind::Harvest, None) => Err(MissionStateError::MissingHarvestState),
        (_, Some(_)) => Err(MissionStateError::UnexpectedHarvestState),
        (_, None) => Ok(()),
    }
}

pub(crate) fn validate_harvest_result(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        (MissionPhase::Completed | MissionPhase::Failed, None) => {
            Err(MissionStateError::MissingHarvestResult)
        }
        (
            MissionPhase::Preparation
            | MissionPhase::Outbound
            | MissionPhase::OnSite
            | MissionPhase::Returning
            | MissionPhase::Cancelled,
            Some(_),
        ) => Err(MissionStateError::UnexpectedMissionResult),
        (_, Some(MissionResult::Harvest(result))) => {
            let harvest = mission
                .harvest
                .ok_or(MissionStateError::MissingHarvestState)?;
            if result.site_id != harvest.site_id {
                return Err(MissionStateError::HarvestResultSiteMismatch {
                    expected: harvest.site_id,
                    found: result.site_id,
                });
            }
            let accounted = result
                .delivered
                .checked_add(result.retained)
                .ok_or(MissionStateError::HarvestResultCargoMismatch)?;
            if result.collected != harvest.collected
                || accounted != harvest.collected
                || result.site_remaining != harvest.site_remaining
                || result.status != harvest.status
            {
                return Err(MissionStateError::HarvestResultCargoMismatch);
            }
            Ok(())
        }
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

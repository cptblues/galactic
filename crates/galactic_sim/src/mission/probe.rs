use galactic_domain::{ColonyId, FactionId};

use crate::{
    CraftableId, FleetComposition, FleetCreated, FleetError, GameState, KnowledgeLevel,
    UniverseRepository, form_fleet,
};

use super::{
    MissionError, MissionKind, MissionLaunched, MissionOrder, MissionPhase, MissionResult,
    MissionState, MissionStateError, MissionTarget, launch_mission, validate_mission_target,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeMissionResult {
    pub target: MissionTarget,
    pub previous: KnowledgeLevel,
    pub current: KnowledgeLevel,
    pub revealed_systems: u16,
    pub newly_detected_systems: u16,
    pub revealed_routes: u16,
    pub revealed_planets: u16,
}

/// Launches the minimal player-facing reconnaissance loop atomically.
///
/// The command reuses the lowest-id idle probe-only fleet docked at the
/// origin. If none exists, it forms one from a previously crafted Luciole in
/// the colony inventory. Any validation failure leaves the original state
/// untouched.
pub fn launch_probe_mission(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    origin_colony_id: ColonyId,
    target: MissionTarget,
) -> Result<(Option<FleetCreated>, MissionLaunched), MissionError> {
    let origin_colony = state
        .colony(origin_colony_id)
        .ok_or(MissionError::UnknownOriginColony(origin_colony_id))?;
    state
        .authorize_management(actor, origin_colony.owner)
        .map_err(MissionError::Access)?;
    let origin = origin_colony.system_id;
    let target_system = validate_mission_target(universe, target)?;
    let current = match target {
        MissionTarget::System(system_id) => state.system_knowledge_level(system_id),
        MissionTarget::Planet { planet_id, .. } => state.planet_knowledge_level(planet_id),
    };
    if current == KnowledgeLevel::Unknown {
        return Err(MissionError::NoAccessibleRoute {
            origin,
            target: target_system,
        });
    }
    if current != KnowledgeLevel::Detected {
        return Err(MissionError::ProbeTargetNotDetected { target, current });
    }

    let mut candidate = state.clone();
    let existing_fleet = candidate
        .fleets
        .iter()
        .filter(|fleet| {
            candidate.can_manage(actor, fleet.owner)
                && fleet.is_idle()
                && fleet.location == crate::FleetLocation::Docked(origin_colony_id)
                && fleet.composition.total_ships()
                    == fleet.composition.quantity(CraftableId::LIGHT_PROBE)
        })
        .map(|fleet| fleet.id)
        .min();

    let (fleet_id, created) = if let Some(fleet_id) = existing_fleet {
        (fleet_id, None)
    } else {
        let available = candidate
            .colony(origin_colony_id)
            .ok_or(MissionError::UnknownOriginColony(origin_colony_id))?
            .inventory
            .quantity(CraftableId::LIGHT_PROBE);
        if available == 0 {
            return Err(MissionError::ProbeUnavailable(origin_colony_id));
        }
        let composition =
            FleetComposition::from_stacks([crate::ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .map_err(FleetError::InvalidComposition)
                .map_err(MissionError::Fleet)?;
        let created = form_fleet(&mut candidate, actor, origin_colony_id, composition)
            .map_err(MissionError::Fleet)?;
        (created.fleet_id, Some(created))
    };

    let departure_at = candidate.clock.current_tick();
    let launched = launch_mission(
        &mut candidate,
        universe,
        actor,
        MissionOrder {
            fleet_id,
            origin,
            target,
            kind: MissionKind::Probe,
            departure_at,
        },
    )?;
    *state = candidate;
    Ok((created, launched))
}

pub(crate) fn validate_probe_result(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        (MissionPhase::OnSite | MissionPhase::Returning | MissionPhase::Completed, None) => {
            Err(MissionStateError::MissingProbeResult)
        }
        (MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled, Some(_)) => {
            Err(MissionStateError::UnexpectedMissionResult)
        }
        (_, Some(MissionResult::Probe(result))) if result.target != mission.order.target => {
            Err(MissionStateError::ProbeResultTargetMismatch {
                expected: mission.order.target,
                found: result.target,
            })
        }
        (_, Some(MissionResult::Probe(_))) => Ok(()),
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

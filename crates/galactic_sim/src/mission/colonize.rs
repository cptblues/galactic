use galactic_domain::{ColonyId, FactionId};

use crate::{
    ColonizationMissionOutcome, FleetComposition, FleetCreated, FleetError, GameState, ShipStack,
    UniverseRepository, assess_planet_colonizability, form_fleet, planetary_analysis_rules,
};

use super::{
    MissionError, MissionKind, MissionLaunched, MissionOrder, MissionPhase, MissionResult,
    MissionState, MissionStateError, MissionTarget, launch_mission,
};

/// Launches a colony ship atomically from an analyzed, eligible planet.
///
/// The foundation payload remains reserved at the origin while the ship is in
/// flight. It is committed together with the ship only when the target still
/// passes every arrival rule.
pub fn launch_colonization_mission(
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
    let Some(planet_id) = target.planet_id() else {
        return Err(MissionError::ColonizationPlanetTargetRequired);
    };
    if let Some(blocker) = assess_planet_colonizability(state, universe, actor, planet_id)
        .blockers
        .into_iter()
        .next()
    {
        return Err(MissionError::ColonizationBlocked(blocker));
    }

    let colony_ship = planetary_analysis_rules().colony_ship();
    let mut candidate = state.clone();
    let existing_fleet = candidate
        .fleets
        .iter()
        .filter(|fleet| {
            candidate.can_manage(actor, fleet.owner)
                && fleet.is_idle()
                && fleet.location == crate::FleetLocation::Docked(origin_colony_id)
                && fleet.composition.total_ships() == 1
                && fleet.composition.quantity(colony_ship) == 1
        })
        .map(|fleet| fleet.id)
        .min();

    let (fleet_id, created) = if let Some(fleet_id) = existing_fleet {
        (fleet_id, None)
    } else {
        let available = candidate
            .colony(origin_colony_id)
            .expect("the origin colony was validated")
            .inventory
            .quantity(colony_ship);
        if available == 0 {
            return Err(MissionError::ColonizationShipUnavailable(origin_colony_id));
        }
        let composition = FleetComposition::from_stacks([ShipStack::new(colony_ship, 1)])
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
            kind: MissionKind::Colonize,
            departure_at,
        },
    )?;
    *state = candidate;
    Ok((created, launched))
}

pub(crate) fn validate_colonize_result(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        (MissionPhase::OnSite | MissionPhase::Returning | MissionPhase::Completed, None) => {
            Err(MissionStateError::MissingColonizationResult)
        }
        (MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled, Some(_)) => {
            Err(MissionStateError::UnexpectedMissionResult)
        }
        (_, Some(MissionResult::Colonize(result))) => {
            let expected = mission
                .order
                .target
                .planet_id()
                .ok_or(MissionStateError::MissingColonizationCommitment)?;
            if result.target != expected {
                return Err(MissionStateError::ColonizationResultTargetMismatch {
                    expected,
                    found: result.target,
                });
            }
            if matches!(
                result.outcome,
                ColonizationMissionOutcome::FoundationPrepared
            ) && (!result.colony_ship_consumed || mission.phase != MissionPhase::Completed)
            {
                return Err(MissionStateError::UnexpectedMissionResult);
            }
            if matches!(result.outcome, ColonizationMissionOutcome::TargetInvalid(_))
                && result.colony_ship_consumed
            {
                return Err(MissionStateError::UnexpectedMissionResult);
            }
            Ok(())
        }
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

pub(crate) fn validate_colonization_commitment(
    mission: &MissionState,
) -> Result<(), MissionStateError> {
    match (mission.order.kind, mission.colonization) {
        (MissionKind::Colonize, Some(commitment)) => {
            let expected_planet = mission
                .order
                .target
                .planet_id()
                .ok_or(MissionStateError::MissingColonizationCommitment)?;
            if commitment.planet_id != expected_planet {
                return Err(MissionStateError::ColonizationCommitmentTargetMismatch {
                    expected: expected_planet,
                    found: commitment.planet_id,
                });
            }
            let rules = planetary_analysis_rules();
            if commitment.colony_ship != rules.colony_ship() {
                return Err(MissionStateError::ColonizationCommitmentShipMismatch {
                    expected: rules.colony_ship(),
                    found: commitment.colony_ship,
                });
            }
            if commitment.foundation_cost != rules.foundation_cost() {
                return Err(MissionStateError::ColonizationCommitmentCostMismatch);
            }
            Ok(())
        }
        (MissionKind::Colonize, None) => Err(MissionStateError::MissingColonizationCommitment),
        (_, Some(_)) => Err(MissionStateError::UnexpectedColonizationCommitment),
        (_, None) => Ok(()),
    }
}

use galactic_domain::{ColonyId, FactionId};

use crate::{
    FleetComposition, FleetCreated, FleetError, GameState, KnowledgeLevel, ShipStack,
    UniverseRepository, combat_rules, form_fleet,
};

use super::{
    MissionError, MissionKind, MissionLaunched, MissionOrder, MissionPhase, MissionResult,
    MissionState, MissionStateError, MissionTarget, launch_mission,
};

/// Launches a player-facing attack with an existing idle combat fleet, or
/// forms one atomically from every combat ship docked at the origin.
pub fn launch_attack_mission(
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
        return Err(MissionError::AttackPlanetTargetRequired);
    };
    let current = state.planet_knowledge_level(planet_id);
    if current < KnowledgeLevel::Analyzed {
        return Err(MissionError::AttackTargetNotAnalyzed { planet_id, current });
    }

    let mut candidate = state.clone();
    let existing_fleet = candidate
        .fleets
        .iter()
        .filter(|fleet| {
            candidate.can_manage(actor, fleet.owner)
                && fleet.is_idle()
                && fleet.location == crate::FleetLocation::Docked(origin_colony_id)
                && combat_rules().has_combat_ships(fleet)
        })
        .map(|fleet| fleet.id)
        .min();

    let (fleet_id, created) = if let Some(fleet_id) = existing_fleet {
        (fleet_id, None)
    } else {
        let composition = combat_rules()
            .ships()
            .filter_map(|definition| {
                let quantity = candidate
                    .colony(origin_colony_id)
                    .expect("the origin colony was validated")
                    .inventory
                    .quantity(definition.craftable);
                (quantity > 0).then_some(ShipStack::new(definition.craftable, quantity))
            })
            .collect::<Vec<_>>();
        if composition.is_empty() {
            return Err(MissionError::AttackFleetUnavailable(origin_colony_id));
        }
        let composition = FleetComposition::from_stacks(composition)
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
            kind: MissionKind::Attack,
            departure_at,
        },
    )?;
    *state = candidate;
    Ok((created, launched))
}

pub(crate) fn validate_attack_result(
    mission: &MissionState,
    has_pending_combat: bool,
) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        // COMBAT-001-C: an attack mission legitimately sits at `OnSite` with
        // no result yet while a `PendingCombat` awaits a decision — the
        // caller (`reconstruction.rs`) checks that this pairing is exactly
        // right (a pending combat exists iff the mission needs one).
        (MissionPhase::OnSite, None) if has_pending_combat => Ok(()),
        (
            MissionPhase::OnSite
            | MissionPhase::Returning
            | MissionPhase::Completed
            | MissionPhase::Failed,
            None,
        ) => Err(MissionStateError::MissingAttackResult),
        (MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled, Some(_)) => {
            Err(MissionStateError::UnexpectedMissionResult)
        }
        (_, Some(MissionResult::Attack(result))) => {
            let expected = mission
                .order
                .target
                .planet_id()
                .ok_or(MissionStateError::MissingAttackCommitment)?;
            if result.target != expected {
                return Err(MissionStateError::AttackResultTargetMismatch {
                    expected,
                    found: result.target,
                });
            }
            Ok(())
        }
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

pub(crate) fn validate_attack_commitment(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.order.kind, &mission.attack) {
        (MissionKind::Attack, Some(commitment)) => {
            let expected_planet = mission
                .order
                .target
                .planet_id()
                .ok_or(MissionStateError::MissingAttackCommitment)?;
            if commitment.defender.planet_id != expected_planet {
                return Err(MissionStateError::AttackCommitmentTargetMismatch {
                    expected: expected_planet,
                    found: commitment.defender.planet_id,
                });
            }
            if commitment.attacker.fleet_id != mission.order.fleet_id {
                return Err(MissionStateError::AttackCommitmentFleetMismatch {
                    expected: mission.order.fleet_id,
                    found: commitment.attacker.fleet_id,
                });
            }
            Ok(())
        }
        (MissionKind::Attack, None) => Err(MissionStateError::MissingAttackCommitment),
        (_, Some(_)) => Err(MissionStateError::UnexpectedAttackCommitment),
        (_, None) => Ok(()),
    }
}

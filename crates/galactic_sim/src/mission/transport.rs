use galactic_domain::{ColonyId, FactionId, FleetId, ResourceStock};

use crate::{GameState, UniverseRepository};

use super::{
    MissionError, MissionKind, MissionLaunched, MissionOrder, MissionPhase, MissionResult,
    MissionState, MissionStateError, MissionTarget, launch_mission_with_payload,
    resource_stock_total,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TransportDeliveryStatus {
    Pending,
    Delivered,
    PartiallyDelivered,
    DestinationInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransportMissionState {
    pub destination_colony_id: ColonyId,
    pub cargo: ResourceStock,
    pub delivered: ResourceStock,
    pub status: TransportDeliveryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransportMissionResult {
    pub destination_colony_id: ColonyId,
    pub requested: ResourceStock,
    pub delivered: ResourceStock,
    pub returned: ResourceStock,
    pub retained: ResourceStock,
    pub status: TransportDeliveryStatus,
}

pub(crate) fn validate_transport_launch(
    state: &GameState,
    actor: FactionId,
    origin_colony_id: ColonyId,
    order: MissionOrder,
    transport: TransportMissionState,
) -> Result<(), MissionError> {
    if order.kind != MissionKind::Transport {
        return Err(MissionError::TransportOrderRequired);
    }
    if transport.cargo.is_zero() {
        return Err(MissionError::TransportCargoEmpty);
    }
    if transport.status != TransportDeliveryStatus::Pending || !transport.delivered.is_zero() {
        return Err(MissionError::TransportOrderRequired);
    }
    if transport.destination_colony_id == origin_colony_id {
        return Err(MissionError::TransportDestinationIsOrigin(origin_colony_id));
    }
    let destination = state.colony(transport.destination_colony_id).ok_or(
        MissionError::UnknownTransportDestination(transport.destination_colony_id),
    )?;
    state
        .authorize_management(actor, destination.owner)
        .map_err(MissionError::Access)?;
    let expected_target = MissionTarget::Planet {
        system_id: destination.system_id,
        planet_id: destination.planet_id,
    };
    if order.target != expected_target {
        return Err(MissionError::TransportDestinationTargetMismatch {
            colony_id: destination.id,
            target: order.target,
        });
    }
    let fleet = state
        .fleet(order.fleet_id)
        .ok_or(MissionError::UnknownFleet(order.fleet_id))?;
    if !fleet.cargo.is_zero() {
        return Err(MissionError::TransportFleetHasCargo(fleet.id));
    }
    let cargo = resource_stock_total(transport.cargo)?;
    let capacity = fleet
        .capabilities()
        .map_err(MissionError::InvalidFleetComposition)?
        .cargo_capacity;
    if cargo > capacity {
        return Err(MissionError::TransportCargoExceedsCapacity {
            cargo: transport.cargo,
            capacity,
        });
    }
    Ok(())
}

/// Launches an atomic colony-to-colony transport with an explicit fleet.
///
/// Cargo is reserved at order time, committed into the fleet at departure,
/// delivered up to the destination's free storage, then returned to the
/// origin if it could not be delivered. Any remainder that no longer fits at
/// the origin stays visibly loaded on the docked fleet.
pub fn launch_transport_mission(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    origin_colony_id: ColonyId,
    destination_colony_id: ColonyId,
    fleet_id: FleetId,
    cargo: ResourceStock,
) -> Result<MissionLaunched, MissionError> {
    if cargo.is_zero() {
        return Err(MissionError::TransportCargoEmpty);
    }
    let origin_colony = state
        .colony(origin_colony_id)
        .ok_or(MissionError::UnknownOriginColony(origin_colony_id))?;
    state
        .authorize_management(actor, origin_colony.owner)
        .map_err(MissionError::Access)?;
    if destination_colony_id == origin_colony_id {
        return Err(MissionError::TransportDestinationIsOrigin(origin_colony_id));
    }
    let destination =
        state
            .colony(destination_colony_id)
            .ok_or(MissionError::UnknownTransportDestination(
                destination_colony_id,
            ))?;
    state
        .authorize_management(actor, destination.owner)
        .map_err(MissionError::Access)?;

    let origin = origin_colony.system_id;
    let target = MissionTarget::Planet {
        system_id: destination.system_id,
        planet_id: destination.planet_id,
    };
    let departure_at = state.clock.current_tick();
    let mut candidate = state.clone();

    let launched = launch_mission_with_payload(
        &mut candidate,
        universe,
        actor,
        MissionOrder {
            fleet_id,
            origin,
            target,
            kind: MissionKind::Transport,
            departure_at,
        },
        Some(TransportMissionState {
            destination_colony_id,
            cargo,
            delivered: ResourceStock::ZERO,
            status: TransportDeliveryStatus::Pending,
        }),
        None,
    )?;
    *state = candidate;
    Ok(launched)
}

pub(crate) fn validate_transport_state(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.order.kind, mission.transport) {
        (MissionKind::Transport, Some(transport)) => {
            if !matches!(mission.order.target, MissionTarget::Planet { .. }) {
                return Err(MissionStateError::TransportPlanetTargetRequired);
            }
            if transport.cargo.is_zero() {
                return Err(MissionStateError::EmptyTransportCargo);
            }
            if !transport.cargo.can_cover(transport.delivered) {
                return Err(MissionStateError::TransportResultCargoMismatch);
            }
            let pending_expected = matches!(
                mission.phase,
                MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled
            );
            if pending_expected != (transport.status == TransportDeliveryStatus::Pending) {
                return Err(MissionStateError::InvalidTransportStatus {
                    phase: mission.phase,
                    status: transport.status,
                });
            }
            if transport.status == TransportDeliveryStatus::Pending
                && !transport.delivered.is_zero()
            {
                return Err(MissionStateError::TransportResultCargoMismatch);
            }
            Ok(())
        }
        (MissionKind::Transport, None) => Err(MissionStateError::MissingTransportState),
        (_, Some(_)) => Err(MissionStateError::UnexpectedTransportState),
        (_, None) => Ok(()),
    }
}

pub(crate) fn validate_transport_result(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        (MissionPhase::Completed | MissionPhase::Failed, None) => {
            Err(MissionStateError::MissingTransportResult)
        }
        (
            MissionPhase::Preparation
            | MissionPhase::Outbound
            | MissionPhase::OnSite
            | MissionPhase::Returning
            | MissionPhase::Cancelled,
            Some(_),
        ) => Err(MissionStateError::UnexpectedMissionResult),
        (_, Some(MissionResult::Transport(result))) => {
            let transport = mission
                .transport
                .ok_or(MissionStateError::MissingTransportState)?;
            if result.destination_colony_id != transport.destination_colony_id {
                return Err(MissionStateError::TransportResultDestinationMismatch {
                    expected: transport.destination_colony_id,
                    found: result.destination_colony_id,
                });
            }
            let remaining = transport.cargo.saturating_sub(transport.delivered);
            let accounted = result
                .returned
                .checked_add(result.retained)
                .ok_or(MissionStateError::TransportResultCargoMismatch)?;
            if result.requested != transport.cargo
                || result.delivered != transport.delivered
                || result.status != transport.status
                || accounted != remaining
            {
                return Err(MissionStateError::TransportResultCargoMismatch);
            }
            Ok(())
        }
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

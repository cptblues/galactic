// MVP-023: deterministic missions with explicit discovery-frontier results.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, Owner, PlanetId, ReservationId, ResourceCost,
    ResourceLedgerError, SystemId,
};

use crate::{
    AttackMissionCommitment, AttackMissionOutcome, AttackMissionResult, AuthorizationError,
    CombatApplicationError, CombatSnapshotError, CraftableId, FleetAssignment, FleetComposition,
    FleetCompositionError, FleetCreated, FleetError, FleetLocation, GameState, KnowledgeChange,
    KnowledgeLevel, KnowledgeTarget, PlanetaryIntelPrecision, ShipStack, StrategicDuration,
    StrategicTick, UniverseRepository, combat_rules, form_fleet, prepare_attack_commitment,
    refresh_planetary_intelligence, resolve_and_apply_attack,
};

/// Abstract distance crossed by a fleet for one route hop.
///
/// Dividing this work by `cruise_speed` yields strategic ticks. With the
/// default ruleset, a Luciole crosses one hop in 100 ticks (10 seconds at x1).
pub const MISSION_TRAVEL_WORK_PER_HOP: u64 = 16_000;
pub const MISSION_LOCAL_TRAVEL_WORK_BASE: u64 = 2_400;
pub const MISSION_LOCAL_TRAVEL_WORK_PER_ORBIT: u64 = 1_200;
pub const MISSION_RESOLUTION_TICKS: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionKind {
    Probe,
    Attack,
    Transport,
    Harvest,
    Colonize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionPhase {
    Preparation,
    Outbound,
    OnSite,
    Returning,
    Completed,
    Cancelled,
    Failed,
}

impl MissionPhase {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissionTarget {
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

impl MissionTarget {
    pub const fn system_id(self) -> SystemId {
        match self {
            Self::System(system_id) | Self::Planet { system_id, .. } => system_id,
        }
    }

    pub const fn planet_id(self) -> Option<PlanetId> {
        match self {
            Self::System(_) => None,
            Self::Planet { planet_id, .. } => Some(planet_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionOrder {
    pub fleet_id: FleetId,
    pub origin: SystemId,
    pub target: MissionTarget,
    pub kind: MissionKind,
    pub departure_at: StrategicTick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionPlan {
    pub route: Vec<SystemId>,
    pub hops: u16,
    pub travel_duration: StrategicDuration,
    pub resolution_duration: StrategicDuration,
    pub fuel_cost: ResourceCost,
    pub outbound_arrival_at: StrategicTick,
    pub return_departure_at: StrategicTick,
    pub return_arrival_at: StrategicTick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionState {
    pub id: MissionId,
    pub owner: Owner,
    pub order: MissionOrder,
    pub origin_colony_id: ColonyId,
    pub plan: MissionPlan,
    pub phase: MissionPhase,
    pub phase_started_at: StrategicTick,
    pub fuel_reservation: Option<ReservationId>,
    pub attack: Option<AttackMissionCommitment>,
    pub result: Option<MissionResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionLaunched {
    pub mission_id: MissionId,
    pub fleet_id: FleetId,
    pub kind: MissionKind,
    pub target: MissionTarget,
    pub departure_at: StrategicTick,
    pub return_arrival_at: StrategicTick,
    pub fuel_cost: ResourceCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionLaunchRejected {
    pub fleet_id: Option<FleetId>,
    pub error: MissionError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionTransition {
    pub mission_id: MissionId,
    pub from: MissionPhase,
    pub to: MissionPhase,
    pub transitioned_at: StrategicTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionReportOutcome {
    Completed,
    Cancelled,
    Failed,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionResult {
    Probe(ProbeMissionResult),
    Attack(AttackMissionResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionResolution {
    pub mission_id: MissionId,
    pub result: MissionResult,
    pub occurred_at: StrategicTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionReport {
    pub mission_id: MissionId,
    pub fleet_id: FleetId,
    pub kind: MissionKind,
    pub outcome: MissionReportOutcome,
    pub occurred_at: StrategicTick,
    pub result: Option<MissionResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionCancellationRejected {
    pub mission_id: MissionId,
    pub error: MissionError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionError {
    UnknownOriginColony(ColonyId),
    UnknownFleet(FleetId),
    UnknownMission(MissionId),
    Access(AuthorizationError),
    FleetBusy {
        fleet_id: FleetId,
        mission_id: MissionId,
    },
    FleetNotDocked(FleetId),
    UnknownOrigin(SystemId),
    UnknownTarget(SystemId),
    UnknownPlanetTarget(PlanetId),
    PlanetTargetSystemMismatch {
        planet_id: PlanetId,
        expected: SystemId,
        found: SystemId,
    },
    OriginMismatch {
        expected: SystemId,
        found: SystemId,
    },
    SameSystem(SystemId),
    ProbeUnavailable(ColonyId),
    ProbeRequired(FleetId),
    ProbeTargetNotDetected {
        target: MissionTarget,
        current: KnowledgeLevel,
    },
    AttackPlanetTargetRequired,
    AttackTargetNotAnalyzed {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    AttackFleetUnavailable(ColonyId),
    Attack(CombatSnapshotError),
    DepartureInPast {
        current: StrategicTick,
        requested: StrategicTick,
    },
    NoAccessibleRoute {
        origin: SystemId,
        target: SystemId,
    },
    InsufficientRange {
        required_hops: u16,
        available_hops: u16,
    },
    InvalidFleetComposition(FleetCompositionError),
    TravelDurationOverflow,
    FuelCostOverflow,
    MissionIdOverflow,
    Fleet(FleetError),
    Resources(ResourceLedgerError),
    CannotCancelAfterDeparture(MissionId),
    InvalidTransition {
        from: MissionPhase,
        to: MissionPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionStateError {
    InvalidTransition {
        from: MissionPhase,
        to: MissionPhase,
    },
    EmptyRoute,
    RouteOriginMismatch {
        expected: SystemId,
        found: SystemId,
    },
    RouteTargetMismatch {
        expected: SystemId,
        found: SystemId,
    },
    UnknownRouteSystem(SystemId),
    UnknownTargetPlanet(PlanetId),
    TargetPlanetSystemMismatch {
        planet_id: PlanetId,
        expected: SystemId,
        found: SystemId,
    },
    MissingRoute {
        from: SystemId,
        to: SystemId,
    },
    HopCountMismatch {
        expected: u16,
        found: u16,
    },
    InvalidTimeline,
    InvalidFuelCost,
    MissingFuelReservation,
    UnexpectedFuelReservation,
    MissingProbeResult,
    UnexpectedMissionResult,
    ProbeResultTargetMismatch {
        expected: MissionTarget,
        found: MissionTarget,
    },
    MissingAttackCommitment,
    UnexpectedAttackCommitment,
    AttackCommitmentTargetMismatch {
        expected: PlanetId,
        found: PlanetId,
    },
    AttackCommitmentFleetMismatch {
        expected: FleetId,
        found: FleetId,
    },
    MissingAttackResult,
    AttackResultTargetMismatch {
        expected: PlanetId,
        found: PlanetId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MissionEngineEvent {
    Transition {
        recipient: FactionId,
        transition: MissionTransition,
    },
    Report {
        recipient: FactionId,
        report: MissionReport,
    },
    Knowledge {
        recipient: FactionId,
        change: KnowledgeChange,
        occurred_at: StrategicTick,
    },
    Resolution {
        recipient: FactionId,
        resolution: MissionResolution,
    },
}

pub fn plan_mission(
    state: &GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    order: MissionOrder,
) -> Result<(ColonyId, MissionPlan), MissionError> {
    let fleet = state
        .fleet(order.fleet_id)
        .ok_or(MissionError::UnknownFleet(order.fleet_id))?;
    state
        .authorize_management(actor, fleet.owner)
        .map_err(MissionError::Access)?;
    if let FleetAssignment::Mission(mission_id) = fleet.assignment {
        return Err(MissionError::FleetBusy {
            fleet_id: fleet.id,
            mission_id,
        });
    }
    let FleetLocation::Docked(origin_colony_id) = fleet.location else {
        return Err(MissionError::FleetNotDocked(fleet.id));
    };
    let origin_colony = state
        .colony(origin_colony_id)
        .expect("validated docked fleets always reference an existing colony");
    if origin_colony.system_id != order.origin {
        return Err(MissionError::OriginMismatch {
            expected: origin_colony.system_id,
            found: order.origin,
        });
    }
    if universe.system(order.origin).is_none() {
        return Err(MissionError::UnknownOrigin(order.origin));
    }
    let target_system = validate_mission_target(universe, order.target)?;
    if order.origin == target_system && matches!(order.target, MissionTarget::System(_)) {
        return Err(MissionError::SameSystem(order.origin));
    }
    let current_tick = state.clock.current_tick();
    if order.departure_at < current_tick {
        return Err(MissionError::DepartureInPast {
            current: current_tick,
            requested: order.departure_at,
        });
    }

    let route = if order.origin == target_system {
        vec![order.origin]
    } else {
        accessible_shortest_path(state, universe, order.origin, target_system).ok_or(
            MissionError::NoAccessibleRoute {
                origin: order.origin,
                target: target_system,
            },
        )?
    };
    if order.kind == MissionKind::Probe {
        let current = match order.target {
            MissionTarget::System(system_id) => state.system_knowledge_level(system_id),
            MissionTarget::Planet { planet_id, .. } => state.planet_knowledge_level(planet_id),
        };
        if current != KnowledgeLevel::Detected {
            return Err(MissionError::ProbeTargetNotDetected {
                target: order.target,
                current,
            });
        }
        if fleet.composition.quantity(CraftableId::LIGHT_PROBE) == 0 {
            return Err(MissionError::ProbeRequired(fleet.id));
        }
    }
    if order.kind == MissionKind::Attack {
        let Some(planet_id) = order.target.planet_id() else {
            return Err(MissionError::AttackPlanetTargetRequired);
        };
        let current = state.planet_knowledge_level(planet_id);
        if current < KnowledgeLevel::Analyzed {
            return Err(MissionError::AttackTargetNotAnalyzed { planet_id, current });
        }
        prepare_attack_commitment(state, fleet.id, planet_id, 0).map_err(MissionError::Attack)?;
    }
    let hops = u16::try_from(route.len().saturating_sub(1))
        .map_err(|_| MissionError::TravelDurationOverflow)?;
    let capabilities = fleet
        .capabilities()
        .map_err(MissionError::InvalidFleetComposition)?;
    if hops > capabilities.range_hops {
        return Err(MissionError::InsufficientRange {
            required_hops: hops,
            available_hops: capabilities.range_hops,
        });
    }

    let interstellar_work = MISSION_TRAVEL_WORK_PER_HOP
        .checked_mul(u64::from(hops))
        .ok_or(MissionError::TravelDurationOverflow)?;
    let local_work = local_target_work(universe, order.target)?;
    let work = interstellar_work
        .checked_add(local_work)
        .ok_or(MissionError::TravelDurationOverflow)?;
    let travel_ticks = work
        .checked_add(capabilities.cruise_speed.saturating_sub(1))
        .ok_or(MissionError::TravelDurationOverflow)?
        / capabilities.cruise_speed;
    if travel_ticks == 0 {
        return Err(MissionError::TravelDurationOverflow);
    }
    let outbound_fuel_legs = u64::from(hops)
        .checked_add(u64::from(order.target.planet_id().is_some()))
        .ok_or(MissionError::FuelCostOverflow)?;
    let round_trip_hops = outbound_fuel_legs
        .checked_mul(2)
        .ok_or(MissionError::FuelCostOverflow)?;
    let fuel = capabilities
        .fuel_per_hop
        .checked_mul(round_trip_hops)
        .ok_or(MissionError::FuelCostOverflow)?;
    let fuel_cost = ResourceCost::new(0, 0, fuel);

    let outbound_arrival_at = checked_tick_add(order.departure_at, travel_ticks)?;
    let return_departure_at = checked_tick_add(outbound_arrival_at, MISSION_RESOLUTION_TICKS)?;
    let return_arrival_at = checked_tick_add(return_departure_at, travel_ticks)?;

    Ok((
        origin_colony_id,
        MissionPlan {
            route,
            hops,
            travel_duration: StrategicDuration::from_ticks(travel_ticks),
            resolution_duration: StrategicDuration::from_ticks(MISSION_RESOLUTION_TICKS),
            fuel_cost,
            outbound_arrival_at,
            return_departure_at,
            return_arrival_at,
        },
    ))
}

fn validate_mission_target(
    universe: &UniverseRepository,
    target: MissionTarget,
) -> Result<SystemId, MissionError> {
    match target {
        MissionTarget::System(system_id) => universe
            .system(system_id)
            .map(|_| system_id)
            .ok_or(MissionError::UnknownTarget(system_id)),
        MissionTarget::Planet {
            system_id,
            planet_id,
        } => {
            let Some((found_system, _)) = universe.planet_location(planet_id) else {
                return Err(MissionError::UnknownPlanetTarget(planet_id));
            };
            if found_system != system_id {
                return Err(MissionError::PlanetTargetSystemMismatch {
                    planet_id,
                    expected: found_system,
                    found: system_id,
                });
            }
            Ok(system_id)
        }
    }
}

fn local_target_work(
    universe: &UniverseRepository,
    target: MissionTarget,
) -> Result<u64, MissionError> {
    let MissionTarget::Planet {
        system_id,
        planet_id,
    } = target
    else {
        return Ok(0);
    };
    let system = universe
        .system(system_id)
        .ok_or(MissionError::UnknownTarget(system_id))?;
    let orbit_index = system
        .planets
        .iter()
        .position(|planet| planet.id == planet_id)
        .ok_or(MissionError::UnknownPlanetTarget(planet_id))?;
    let orbit_index =
        u64::try_from(orbit_index).map_err(|_| MissionError::TravelDurationOverflow)?;
    MISSION_LOCAL_TRAVEL_WORK_PER_ORBIT
        .checked_mul(orbit_index)
        .and_then(|work| work.checked_add(MISSION_LOCAL_TRAVEL_WORK_BASE))
        .ok_or(MissionError::TravelDurationOverflow)
}

pub fn launch_mission(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    order: MissionOrder,
) -> Result<MissionLaunched, MissionError> {
    let (origin_colony_id, plan) = plan_mission(state, universe, actor, order)?;
    let next_mission_id = state
        .next_mission_id
        .checked_add(1)
        .ok_or(MissionError::MissionIdOverflow)?;
    let mission_id = MissionId::new(state.next_mission_id);
    let attack = if order.kind == MissionKind::Attack {
        let planet_id = order
            .target
            .planet_id()
            .expect("a planned attack always targets a planet");
        Some(
            prepare_attack_commitment(
                state,
                order.fleet_id,
                planet_id,
                attack_seed(universe.definition().seed, mission_id, planet_id),
            )
            .map_err(MissionError::Attack)?,
        )
    } else {
        None
    };
    let reservation = state
        .colony_mut(origin_colony_id)
        .expect("mission origin was validated")
        .resources
        .reserve(plan.fuel_cost)
        .map_err(MissionError::Resources)?;
    let owner = state
        .fleet(order.fleet_id)
        .expect("mission fleet was validated")
        .owner;
    state
        .fleet_mut(order.fleet_id)
        .expect("mission fleet was validated")
        .assignment = FleetAssignment::Mission(mission_id);
    state.next_mission_id = next_mission_id;
    state.missions.push(MissionState {
        id: mission_id,
        owner,
        order,
        origin_colony_id,
        plan: plan.clone(),
        phase: MissionPhase::Preparation,
        phase_started_at: state.clock.current_tick(),
        fuel_reservation: Some(reservation),
        attack,
        result: None,
    });
    state.missions.sort_by_key(|mission| mission.id);

    Ok(MissionLaunched {
        mission_id,
        fleet_id: order.fleet_id,
        kind: order.kind,
        target: order.target,
        departure_at: order.departure_at,
        return_arrival_at: plan.return_arrival_at,
        fuel_cost: plan.fuel_cost,
    })
}

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
                && fleet.location == FleetLocation::Docked(origin_colony_id)
                && combat_rules().is_combat_fleet(fleet)
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
                && fleet.location == FleetLocation::Docked(origin_colony_id)
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
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
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

pub fn cancel_mission(
    state: &mut GameState,
    actor: FactionId,
    mission_id: MissionId,
) -> Result<(MissionTransition, MissionReport), MissionError> {
    let index = state
        .missions
        .iter()
        .position(|mission| mission.id == mission_id)
        .ok_or(MissionError::UnknownMission(mission_id))?;
    let mission = &state.missions[index];
    state
        .authorize_management(actor, mission.owner)
        .map_err(MissionError::Access)?;
    if mission.phase != MissionPhase::Preparation {
        return Err(MissionError::CannotCancelAfterDeparture(mission_id));
    }
    validate_mission_transition(MissionPhase::Preparation, MissionPhase::Cancelled)?;

    let reservation = mission
        .fuel_reservation
        .expect("validated preparation mission must hold its fuel reservation");
    let origin_colony_id = mission.origin_colony_id;
    let fleet_id = mission.order.fleet_id;
    let kind = mission.order.kind;
    state
        .colony_mut(origin_colony_id)
        .expect("validated mission origin must exist")
        .resources
        .release(reservation)
        .map_err(MissionError::Resources)?;
    state
        .fleet_mut(fleet_id)
        .expect("validated mission fleet must exist")
        .assignment = FleetAssignment::Idle;

    let occurred_at = state.clock.current_tick();
    let mission = &mut state.missions[index];
    mission.phase = MissionPhase::Cancelled;
    mission.phase_started_at = occurred_at;
    mission.fuel_reservation = None;
    let transition = MissionTransition {
        mission_id,
        from: MissionPhase::Preparation,
        to: MissionPhase::Cancelled,
        transitioned_at: occurred_at,
    };
    let report = MissionReport {
        mission_id,
        fleet_id,
        kind,
        outcome: MissionReportOutcome::Cancelled,
        occurred_at,
        result: None,
    };
    state.mission_reports.push(report);
    Ok((transition, report))
}

pub const fn validate_mission_transition(
    from: MissionPhase,
    to: MissionPhase,
) -> Result<(), MissionError> {
    let valid = matches!(
        (from, to),
        (MissionPhase::Preparation, MissionPhase::Outbound)
            | (MissionPhase::Preparation, MissionPhase::Cancelled)
            | (MissionPhase::Preparation, MissionPhase::Failed)
            | (MissionPhase::Outbound, MissionPhase::OnSite)
            | (MissionPhase::Outbound, MissionPhase::Failed)
            | (MissionPhase::OnSite, MissionPhase::Returning)
            | (MissionPhase::OnSite, MissionPhase::Failed)
            | (MissionPhase::Returning, MissionPhase::Completed)
            | (MissionPhase::Returning, MissionPhase::Failed)
    );
    if valid {
        Ok(())
    } else {
        Err(MissionError::InvalidTransition { from, to })
    }
}

pub fn validate_mission_state(
    mission: &MissionState,
    universe: &UniverseRepository,
) -> Result<(), MissionStateError> {
    if let MissionTarget::Planet {
        system_id,
        planet_id,
    } = mission.order.target
    {
        let Some((expected, _)) = universe.planet_location(planet_id) else {
            return Err(MissionStateError::UnknownTargetPlanet(planet_id));
        };
        if expected != system_id {
            return Err(MissionStateError::TargetPlanetSystemMismatch {
                planet_id,
                expected,
                found: system_id,
            });
        }
    }
    let Some(first) = mission.plan.route.first().copied() else {
        return Err(MissionStateError::EmptyRoute);
    };
    if first != mission.order.origin {
        return Err(MissionStateError::RouteOriginMismatch {
            expected: mission.order.origin,
            found: first,
        });
    }
    let last = mission
        .plan
        .route
        .last()
        .copied()
        .expect("non-empty route has a last item");
    let target_system = mission.order.target.system_id();
    if last != target_system {
        return Err(MissionStateError::RouteTargetMismatch {
            expected: target_system,
            found: last,
        });
    }
    for system_id in &mission.plan.route {
        if universe.system(*system_id).is_none() {
            return Err(MissionStateError::UnknownRouteSystem(*system_id));
        }
    }
    for edge in mission.plan.route.windows(2) {
        if !universe.route_exists(edge[0], edge[1]) {
            return Err(MissionStateError::MissingRoute {
                from: edge[0],
                to: edge[1],
            });
        }
    }
    let found_hops = u16::try_from(mission.plan.route.len().saturating_sub(1))
        .map_err(|_| MissionStateError::InvalidTimeline)?;
    if found_hops != mission.plan.hops {
        return Err(MissionStateError::HopCountMismatch {
            expected: mission.plan.hops,
            found: found_hops,
        });
    }
    if mission.plan.travel_duration.is_zero()
        || mission.plan.resolution_duration.is_zero()
        || mission.plan.fuel_cost.is_zero()
    {
        return Err(MissionStateError::InvalidTimeline);
    }
    let expected_outbound = mission
        .order
        .departure_at
        .value()
        .checked_add(mission.plan.travel_duration.ticks());
    let expected_return_departure = expected_outbound
        .and_then(|tick| tick.checked_add(mission.plan.resolution_duration.ticks()));
    let expected_return_arrival = expected_return_departure
        .and_then(|tick| tick.checked_add(mission.plan.travel_duration.ticks()));
    if expected_outbound != Some(mission.plan.outbound_arrival_at.value())
        || expected_return_departure != Some(mission.plan.return_departure_at.value())
        || expected_return_arrival != Some(mission.plan.return_arrival_at.value())
    {
        return Err(MissionStateError::InvalidTimeline);
    }
    if mission.plan.fuel_cost.metal != 0
        || mission.plan.fuel_cost.crystal != 0
        || mission.plan.fuel_cost.fuel == 0
    {
        return Err(MissionStateError::InvalidFuelCost);
    }
    if mission.phase == MissionPhase::Preparation && mission.fuel_reservation.is_none() {
        return Err(MissionStateError::MissingFuelReservation);
    }
    if mission.phase != MissionPhase::Preparation && mission.fuel_reservation.is_some() {
        return Err(MissionStateError::UnexpectedFuelReservation);
    }
    match (mission.order.kind, mission.phase, mission.result) {
        (
            MissionKind::Probe,
            MissionPhase::OnSite | MissionPhase::Returning | MissionPhase::Completed,
            None,
        ) => return Err(MissionStateError::MissingProbeResult),
        (
            MissionKind::Probe,
            MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled,
            Some(_),
        ) => return Err(MissionStateError::UnexpectedMissionResult),
        (MissionKind::Probe, _, Some(MissionResult::Probe(result)))
            if result.target != mission.order.target =>
        {
            return Err(MissionStateError::ProbeResultTargetMismatch {
                expected: mission.order.target,
                found: result.target,
            });
        }
        (
            MissionKind::Attack,
            MissionPhase::OnSite
            | MissionPhase::Returning
            | MissionPhase::Completed
            | MissionPhase::Failed,
            None,
        ) => return Err(MissionStateError::MissingAttackResult),
        (
            MissionKind::Attack,
            MissionPhase::Preparation | MissionPhase::Outbound | MissionPhase::Cancelled,
            Some(_),
        ) => return Err(MissionStateError::UnexpectedMissionResult),
        (MissionKind::Attack, _, Some(MissionResult::Attack(result))) => {
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
        }
        (MissionKind::Attack, _, Some(MissionResult::Probe(_)))
        | (MissionKind::Probe, _, Some(MissionResult::Attack(_)))
        | (MissionKind::Transport | MissionKind::Harvest | MissionKind::Colonize, _, Some(_)) => {
            return Err(MissionStateError::UnexpectedMissionResult);
        }
        _ => {}
    }
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
        }
        (MissionKind::Attack, None) => return Err(MissionStateError::MissingAttackCommitment),
        (_, Some(_)) => return Err(MissionStateError::UnexpectedAttackCommitment),
        (_, None) => {}
    }
    Ok(())
}

pub(crate) fn advance_missions(
    state: &mut GameState,
    universe: &UniverseRepository,
    current_tick: StrategicTick,
) -> Vec<MissionEngineEvent> {
    let mut events = Vec::new();
    for index in 0..state.missions.len() {
        loop {
            let mission = &state.missions[index];
            let (next_phase, transition_at) = match mission.phase {
                MissionPhase::Preparation if current_tick >= mission.order.departure_at => {
                    (MissionPhase::Outbound, mission.order.departure_at)
                }
                MissionPhase::Outbound if current_tick >= mission.plan.outbound_arrival_at => {
                    (MissionPhase::OnSite, mission.plan.outbound_arrival_at)
                }
                MissionPhase::OnSite
                    if matches!(
                        mission.result,
                        Some(MissionResult::Attack(AttackMissionResult {
                            attackers_destroyed: true,
                            ..
                        }))
                    ) =>
                {
                    (MissionPhase::Failed, mission.phase_started_at)
                }
                MissionPhase::OnSite if current_tick >= mission.plan.return_departure_at => {
                    (MissionPhase::Returning, mission.plan.return_departure_at)
                }
                MissionPhase::Returning if current_tick >= mission.plan.return_arrival_at => {
                    (MissionPhase::Completed, mission.plan.return_arrival_at)
                }
                _ => break,
            };
            let from = mission.phase;
            let mission_id = mission.id;
            let fleet_id = mission.order.fleet_id;
            let origin = mission.order.origin;
            let target = mission.order.target;
            let target_system = target.system_id();
            let origin_colony_id = mission.origin_colony_id;
            let reservation = mission.fuel_reservation;
            let owner = mission
                .owner
                .faction()
                .expect("validated missions always have a faction owner");
            let kind = mission.order.kind;
            let mission_result = mission.result;
            let attack = mission.attack.clone();
            let mut knowledge_changes = Vec::new();
            let mut resolution = None;

            validate_mission_transition(from, next_phase)
                .expect("engine transitions must follow the mission state machine");
            match next_phase {
                MissionPhase::Outbound => {
                    state
                        .colony_mut(origin_colony_id)
                        .expect("validated mission origin exists")
                        .resources
                        .commit(reservation.expect("preparation reserves fuel"))
                        .expect("validated mission fuel reservation commits");
                    let fleet = state
                        .fleet_mut(fleet_id)
                        .expect("validated mission fleet exists");
                    fleet.location = FleetLocation::InSystem(origin);
                }
                MissionPhase::OnSite => {
                    state
                        .fleet_mut(fleet_id)
                        .expect("validated mission fleet exists")
                        .location = FleetLocation::InSystem(target_system);
                    if kind == MissionKind::Probe {
                        let (
                            previous,
                            revealed_systems,
                            newly_detected_systems,
                            revealed_routes,
                            revealed_planets,
                        ) = match target {
                            MissionTarget::System(system_id) => {
                                let previous = state.system_knowledge_level(system_id);
                                let frontier = state.probe_system(universe, system_id);
                                let revealed_systems = frontier
                                    .changes
                                    .iter()
                                    .filter(|change| {
                                        matches!(change.target, KnowledgeTarget::System(_))
                                    })
                                    .count()
                                    .min(usize::from(u16::MAX))
                                    as u16;
                                let revealed_planets = frontier
                                    .changes
                                    .iter()
                                    .filter(|change| {
                                        matches!(change.target, KnowledgeTarget::Planet(_))
                                    })
                                    .count()
                                    .min(usize::from(u16::MAX))
                                    as u16;
                                let newly_detected_systems = frontier
                                    .newly_detected_systems
                                    .len()
                                    .min(usize::from(u16::MAX))
                                    as u16;
                                let revealed_routes =
                                    frontier.revealed_routes.len().min(usize::from(u16::MAX))
                                        as u16;
                                knowledge_changes = frontier.changes;
                                (
                                    previous,
                                    revealed_systems,
                                    newly_detected_systems,
                                    revealed_routes,
                                    revealed_planets,
                                )
                            }
                            MissionTarget::Planet { planet_id, .. } => {
                                let previous = state.planet_knowledge_level(planet_id);
                                knowledge_changes = state.advance_planet_knowledge(
                                    universe,
                                    planet_id,
                                    KnowledgeLevel::Probed,
                                );
                                refresh_planetary_intelligence(
                                    state,
                                    planet_id,
                                    PlanetaryIntelPrecision::Contact,
                                    transition_at,
                                )
                                .expect("validated mission targets have a planetary presence");
                                let revealed_planets =
                                    u16::from(state.planet_knowledge_level(planet_id) > previous);
                                (previous, 0, 0, 0, revealed_planets)
                            }
                        };
                        let result = MissionResult::Probe(ProbeMissionResult {
                            target,
                            previous,
                            current: match target {
                                MissionTarget::System(system_id) => {
                                    state.system_knowledge_level(system_id)
                                }
                                MissionTarget::Planet { planet_id, .. } => {
                                    state.planet_knowledge_level(planet_id)
                                }
                            },
                            revealed_systems,
                            newly_detected_systems,
                            revealed_routes,
                            revealed_planets,
                        });
                        resolution = Some(MissionResolution {
                            mission_id,
                            result,
                            occurred_at: transition_at,
                        });
                    } else if kind == MissionKind::Attack {
                        let (_, result) = resolve_and_apply_attack(
                            state,
                            mission_id,
                            transition_at,
                            attack
                                .as_ref()
                                .expect("a validated attack has a commitment"),
                        )
                        .unwrap_or_else(|error| match error {
                            CombatApplicationError::AlreadyApplied(_) => {
                                panic!("a combat mission attempted to apply twice")
                            }
                            _ => panic!("validated combat application failed: {error:?}"),
                        });
                        resolution = Some(MissionResolution {
                            mission_id,
                            result: MissionResult::Attack(result),
                            occurred_at: transition_at,
                        });
                    }
                }
                MissionPhase::Returning => {}
                MissionPhase::Completed => {
                    let fleet = state
                        .fleet_mut(fleet_id)
                        .expect("validated mission fleet exists");
                    fleet.location = FleetLocation::Docked(origin_colony_id);
                    fleet.assignment = FleetAssignment::Idle;
                }
                MissionPhase::Failed => {}
                MissionPhase::Preparation | MissionPhase::Cancelled => {
                    unreachable!("automatic progression only follows the nominal path")
                }
            }

            let mission = &mut state.missions[index];
            mission.phase = next_phase;
            mission.phase_started_at = transition_at;
            if next_phase == MissionPhase::Outbound {
                mission.fuel_reservation = None;
            }
            if let Some(resolution) = resolution {
                mission.result = Some(resolution.result);
            }
            let transition = MissionTransition {
                mission_id,
                from,
                to: next_phase,
                transitioned_at: transition_at,
            };
            events.push(MissionEngineEvent::Transition {
                recipient: owner,
                transition,
            });
            events.extend(knowledge_changes.into_iter().map(|change| {
                MissionEngineEvent::Knowledge {
                    recipient: owner,
                    change,
                    occurred_at: transition_at,
                }
            }));
            if let Some(resolution) = resolution {
                events.push(MissionEngineEvent::Resolution {
                    recipient: owner,
                    resolution,
                });
            }
            if matches!(next_phase, MissionPhase::Completed | MissionPhase::Failed) {
                let outcome = if next_phase == MissionPhase::Failed
                    || matches!(
                        mission_result,
                        Some(MissionResult::Attack(AttackMissionResult {
                            outcome: AttackMissionOutcome::TargetInvalid(_),
                            ..
                        }))
                    ) {
                    MissionReportOutcome::Failed
                } else {
                    MissionReportOutcome::Completed
                };
                let report = MissionReport {
                    mission_id,
                    fleet_id,
                    kind,
                    outcome,
                    occurred_at: transition_at,
                    result: mission_result,
                };
                state.mission_reports.push(report);
                events.push(MissionEngineEvent::Report {
                    recipient: owner,
                    report,
                });
            }
        }
    }
    events
}

fn attack_seed(universe_seed: u64, mission_id: MissionId, planet_id: PlanetId) -> u64 {
    splitmix64(
        universe_seed
            ^ mission_id.raw().rotate_left(19)
            ^ planet_id.raw().rotate_left(37)
            ^ 0x434f_4d42_4154_5631,
    )
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn accessible_shortest_path(
    state: &GameState,
    universe: &UniverseRepository,
    origin: SystemId,
    target: SystemId,
) -> Option<Vec<SystemId>> {
    let mut adjacency = BTreeMap::<SystemId, BTreeSet<SystemId>>::new();
    for route in state.visible_routes(universe) {
        adjacency.entry(route.from).or_default().insert(route.to);
        adjacency.entry(route.to).or_default().insert(route.from);
    }
    if !adjacency.contains_key(&origin) || !adjacency.contains_key(&target) {
        return None;
    }

    let mut queue = VecDeque::from([origin]);
    let mut visited = BTreeSet::from([origin]);
    let mut previous = BTreeMap::<SystemId, SystemId>::new();
    while let Some(current) = queue.pop_front() {
        let neighbors = adjacency.get(&current)?;
        for neighbor in neighbors {
            if !visited.insert(*neighbor) {
                continue;
            }
            previous.insert(*neighbor, current);
            if *neighbor == target {
                let mut route = vec![target];
                let mut cursor = target;
                while cursor != origin {
                    cursor = *previous.get(&cursor)?;
                    route.push(cursor);
                }
                route.reverse();
                return Some(route);
            }
            queue.push_back(*neighbor);
        }
    }
    None
}

fn checked_tick_add(tick: StrategicTick, duration: u64) -> Result<StrategicTick, MissionError> {
    tick.value()
        .checked_add(duration)
        .map(StrategicTick::new)
        .ok_or(MissionError::TravelDurationOverflow)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use galactic_domain::{Owner, PlanetId, ResourceStock, UniverseConfig};

    use crate::{
        AttackInvalidReason, AttackMissionOutcome, CombatOutcome, CombatReportStatus, CraftableId,
        FleetComposition, GameAction, KnowledgeLevel, PlanetaryForceLoss, ShipStack, Simulation,
        apply_planetary_force_losses, form_fleet,
    };

    use super::*;

    fn simulation_with_probe_fleet() -> (Simulation, FleetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_PROBE, 1);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("probe fleet composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("probe fleet can be formed");
        (simulation, created.fleet_id)
    }

    fn neighboring_target(simulation: &Simulation) -> SystemId {
        let origin = simulation.state().colonies[0].system_id;
        simulation.universe_repository().neighboring_systems(origin)[0]
    }

    fn simulation_with_attack_target() -> (Simulation, PlanetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_systems = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .to_vec();
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                neighboring_systems.contains(&presence.planet_id.system_id())
                    && presence.occupant != Owner::Faction(actor)
                    && presence.occupant != Owner::Unowned
                    && !presence.forces.is_empty()
            })
            .expect("the home neighborhood guarantees a hostile outpost")
            .planet_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Analyzed,
        );
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::FRIGATE_BULWARK, 3);
        (simulation, target)
    }

    fn detected_planets_in_origin(simulation: &Simulation) -> Vec<(usize, PlanetId)> {
        let origin = simulation.state().colonies[0].system_id;
        simulation
            .universe()
            .system(origin)
            .expect("home system exists")
            .planets
            .iter()
            .enumerate()
            .filter(|(_, planet)| {
                simulation.state().planet_knowledge_level(planet.id) == KnowledgeLevel::Detected
            })
            .map(|(index, planet)| (index, planet.id))
            .collect()
    }

    #[test]
    fn non_combat_mission_kinds_use_the_generic_planner() {
        for kind in [
            MissionKind::Probe,
            MissionKind::Transport,
            MissionKind::Harvest,
            MissionKind::Colonize,
        ] {
            let (simulation, fleet_id) = simulation_with_probe_fleet();
            let origin = simulation.state().colonies[0].system_id;
            let target = neighboring_target(&simulation);
            let order = MissionOrder {
                fleet_id,
                origin,
                target: MissionTarget::System(target),
                kind,
                departure_at: simulation.state().clock.current_tick(),
            };

            let (_, plan) = plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                simulation.state().player_faction,
                order,
            )
            .expect("generic planner accepts every MVP mission kind");

            assert_eq!(plan.route.first(), Some(&origin));
            assert_eq!(plan.route.last(), Some(&target));
            assert_eq!(plan.hops, 1);
        }
    }

    #[test]
    fn launch_reserves_fuel_and_locks_the_fleet() {
        let (mut simulation, fleet_id) = simulation_with_probe_fleet();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let target = neighboring_target(&simulation);
        let order = MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::System(target),
            kind: MissionKind::Probe,
            departure_at: StrategicTick::new(2),
        };

        let launched = launch_mission(
            simulation.state_mut(),
            &UniverseRepository::generate(UniverseConfig::mvp()),
            actor,
            order,
        )
        .expect("mission can launch");

        assert_eq!(
            simulation.state().fleet(fleet_id).unwrap().assignment,
            FleetAssignment::Mission(launched.mission_id),
        );
        assert_eq!(
            simulation.state().colonies[0].resources.reserved_total(),
            launched.fuel_cost.as_stock(),
        );
        assert!(matches!(
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                order,
            ),
            Err(MissionError::FleetBusy { .. }),
        ));
    }

    #[test]
    fn probe_shortcut_requires_a_crafted_probe_and_is_atomic() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let target = neighboring_target(&simulation);
        let before = simulation.state().clone();
        let repository = UniverseRepository::generate(UniverseConfig::mvp());

        assert_eq!(
            launch_probe_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                MissionTarget::System(target),
            ),
            Err(MissionError::ProbeUnavailable(colony_id)),
        );
        assert_eq!(simulation.state(), &before);

        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_PROBE, 1);
        let (created, launched) = launch_probe_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            MissionTarget::System(target),
        )
        .expect("a crafted probe can launch");

        assert_eq!(
            created.map(|created| created.fleet_id),
            Some(launched.fleet_id),
        );
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_PROBE),
            0,
        );
        assert_eq!(simulation.state().missions.len(), 1);
    }

    #[test]
    fn local_probe_duration_grows_with_the_target_orbit() {
        let (simulation, fleet_id) = simulation_with_probe_fleet();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let detected = detected_planets_in_origin(&simulation);
        assert!(
            detected.len() >= 2,
            "the MVP home system needs two detectable planets"
        );
        let (near_index, near_planet) = detected[0];
        let (far_index, far_planet) = detected[detected.len() - 1];
        assert!(far_index > near_index);

        let plan_for = |planet_id| {
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                MissionOrder {
                    fleet_id,
                    origin,
                    target: MissionTarget::Planet {
                        system_id: origin,
                        planet_id,
                    },
                    kind: MissionKind::Probe,
                    departure_at: StrategicTick::ZERO,
                },
            )
            .expect("a detected local planet accepts a probe")
            .1
        };
        let near_plan = plan_for(near_planet);
        let far_plan = plan_for(far_planet);

        assert_eq!(near_plan.route, vec![origin]);
        assert_eq!(near_plan.hops, 0);
        assert!(near_plan.fuel_cost.fuel > 0);
        assert!(far_plan.travel_duration > near_plan.travel_duration);
    }

    #[test]
    fn local_probe_identifies_only_its_selected_planet() {
        let (mut simulation, fleet_id) = simulation_with_probe_fleet();
        let origin = simulation.state().colonies[0].system_id;
        let detected = detected_planets_in_origin(&simulation);
        assert!(
            detected.len() >= 2,
            "the MVP home system needs two detectable planets"
        );
        let target = detected[0].1;
        let sibling = detected[1].1;
        let mission_target = MissionTarget::Planet {
            system_id: origin,
            planet_id: target,
        };

        simulation.apply_player_action(GameAction::LaunchMission(MissionOrder {
            fleet_id,
            origin,
            target: mission_target,
            kind: MissionKind::Probe,
            departure_at: StrategicTick::ZERO,
        }));
        let travel = simulation.state().missions[0].plan.travel_duration;
        let events = simulation.advance(travel.as_duration());

        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert_eq!(
            simulation.state().planet_knowledge_level(sibling),
            KnowledgeLevel::Detected
        );
        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::GameEventKind::MissionResolved(MissionResolution {
                result: MissionResult::Probe(ProbeMissionResult {
                    target: resolved,
                    revealed_planets: 1,
                    ..
                }),
                ..
            }) if resolved == mission_target
        )));
    }

    #[test]
    fn probe_arrival_reveals_the_target_and_records_a_result() {
        let (mut simulation, fleet_id) = simulation_with_probe_fleet();
        let origin = simulation.state().colonies[0].system_id;
        let target = neighboring_target(&simulation);
        let events = simulation.apply_player_action(GameAction::LaunchMission(MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::System(target),
            kind: MissionKind::Probe,
            departure_at: StrategicTick::ZERO,
        }));
        assert!(matches!(
            events.as_slice(),
            [crate::GameEvent {
                kind: crate::GameEventKind::MissionLaunched(_),
                ..
            }],
        ));

        let events = simulation.advance(Duration::from_secs(10));

        assert_eq!(
            simulation.state().system_knowledge_level(target),
            KnowledgeLevel::Probed,
        );
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::OnSite);
        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::GameEventKind::KnowledgeChanged(KnowledgeChange {
                target: KnowledgeTarget::System(system_id),
                current: KnowledgeLevel::Probed,
                ..
            }) if system_id == target
        )));
        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::GameEventKind::MissionResolved(MissionResolution {
                result: MissionResult::Probe(ProbeMissionResult {
                    target: resolved,
                    current: KnowledgeLevel::Probed,
                    ..
                }),
                ..
            }) if resolved == MissionTarget::System(target)
        )));
    }

    #[test]
    fn invalid_transition_is_rejected_explicitly() {
        assert_eq!(
            validate_mission_transition(MissionPhase::Preparation, MissionPhase::Completed),
            Err(MissionError::InvalidTransition {
                from: MissionPhase::Preparation,
                to: MissionPhase::Completed,
            }),
        );
    }

    #[test]
    fn unknown_route_and_insufficient_range_block_departure() {
        let (simulation, fleet_id) = simulation_with_probe_fleet();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let hidden_target = simulation
            .universe()
            .systems
            .iter()
            .map(|system| system.id)
            .find(|system_id| !simulation.state().is_system_visible(*system_id))
            .expect("the MVP starts with hidden systems");
        let hidden_order = MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::System(hidden_target),
            kind: MissionKind::Probe,
            departure_at: StrategicTick::ZERO,
        };
        assert_eq!(
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                hidden_order,
            ),
            Err(MissionError::NoAccessibleRoute {
                origin,
                target: hidden_target,
            }),
        );

        let mut short_range = Simulation::new(UniverseConfig::mvp());
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let system_ids = repository
            .definition()
            .systems
            .iter()
            .map(|system| system.id)
            .collect::<Vec<_>>();
        for system_id in system_ids {
            short_range.state_mut().advance_system_knowledge(
                &repository,
                system_id,
                KnowledgeLevel::Probed,
            );
        }
        let origin = short_range.state().colonies[0].system_id;
        let target = repository
            .definition()
            .systems
            .iter()
            .map(|system| system.id)
            .find(|target| {
                repository
                    .hop_distance(origin, *target)
                    .is_some_and(|hops| hops > 2)
            })
            .expect("the MVP graph contains a system beyond colony-ship range");
        let actor = short_range.state().player_faction;
        let colony_id = short_range.state().colonies[0].id;
        short_range.state_mut().colonies[0]
            .inventory
            .add(CraftableId::COLONY_SHIP, 1);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::COLONY_SHIP, 1)])
                .expect("colony fleet composition is valid");
        let fleet_id = form_fleet(short_range.state_mut(), actor, colony_id, composition)
            .expect("colony fleet can be formed")
            .fleet_id;
        let required_hops = u16::try_from(
            repository
                .hop_distance(origin, target)
                .expect("selected target is reachable"),
        )
        .expect("MVP hop distance fits u16");

        assert_eq!(
            plan_mission(
                short_range.state(),
                &repository,
                actor,
                MissionOrder {
                    fleet_id,
                    origin,
                    target: MissionTarget::System(target),
                    kind: MissionKind::Colonize,
                    departure_at: StrategicTick::ZERO,
                },
            ),
            Err(MissionError::InsufficientRange {
                required_hops,
                available_hops: 2,
            }),
        );
    }

    #[test]
    fn cancellation_before_departure_releases_fuel_and_fleet() {
        let (mut simulation, fleet_id) = simulation_with_probe_fleet();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let target = neighboring_target(&simulation);
        let launched = launch_mission(
            simulation.state_mut(),
            &UniverseRepository::generate(UniverseConfig::mvp()),
            actor,
            MissionOrder {
                fleet_id,
                origin,
                target: MissionTarget::System(target),
                kind: MissionKind::Probe,
                departure_at: StrategicTick::new(10),
            },
        )
        .expect("mission can launch");

        let (_, report) = cancel_mission(simulation.state_mut(), actor, launched.mission_id)
            .expect("preparation can be cancelled");

        assert_eq!(report.outcome, MissionReportOutcome::Cancelled);
        assert!(
            simulation.state().colonies[0]
                .resources
                .reserved_total()
                .is_zero()
        );
        assert!(simulation.state().fleet(fleet_id).unwrap().is_idle());
    }

    #[test]
    fn attack_secures_the_guaranteed_neighboring_outpost_once() {
        let (mut simulation, target) = simulation_with_attack_target();
        let colony_id = simulation.state().colonies[0].id;
        let events = simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });

        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::GameEventKind::MissionLaunched(MissionLaunched {
                kind: MissionKind::Attack,
                ..
            })
        )));
        let mission_id = simulation.state().missions[0].id;
        simulation.advance(Duration::from_secs(120));

        let presence = simulation
            .state()
            .planetary_presence(target)
            .expect("the target presence remains addressable");
        assert_eq!(
            presence.occupant,
            Owner::Faction(simulation.state().player_faction)
        );
        assert!(presence.forces.is_empty());
        assert!(matches!(
            simulation
                .state()
                .combat_report(mission_id)
                .expect("combat produces one persistent report")
                .status,
            CombatReportStatus::Resolved(crate::CombatResolution {
                outcome: CombatOutcome::AttackerVictory,
                control: crate::CombatControlChange::Secured { .. },
                ..
            })
        ));
        assert!(matches!(
            simulation.state().missions[0].result,
            Some(MissionResult::Attack(AttackMissionResult {
                outcome: AttackMissionOutcome::Resolved(CombatOutcome::AttackerVictory),
                secured: true,
                ..
            }))
        ));
        assert_eq!(
            simulation
                .state()
                .combat_reports
                .iter()
                .filter(|report| report.mission_id == mission_id)
                .count(),
            1
        );
    }

    #[test]
    fn changed_target_is_rejected_without_applying_old_defenses() {
        let (mut simulation, target) = simulation_with_attack_target();
        let colony_id = simulation.state().colonies[0].id;
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        let mission_id = simulation.state().missions[0].id;
        let loss_target = simulation
            .state()
            .planetary_presence(target)
            .expect("the target presence exists")
            .forces[0];
        apply_planetary_force_losses(
            simulation.state_mut(),
            target,
            &[PlanetaryForceLoss {
                definition_id: loss_target.definition_id,
                quantity: 1,
            }],
        )
        .expect("an external battle can change the defenses in flight");
        let changed_presence = simulation
            .state()
            .planetary_presence(target)
            .expect("the changed target still exists")
            .clone();

        simulation.advance(Duration::from_secs(120));

        assert_eq!(
            simulation
                .state()
                .planetary_presence(target)
                .expect("the invalid target remains unchanged"),
            &changed_presence
        );
        assert!(matches!(
            simulation
                .state()
                .combat_report(mission_id)
                .expect("an invalid target still creates a report")
                .status,
            CombatReportStatus::TargetInvalid(AttackInvalidReason::TargetPresenceChanged)
        ));
        assert!(matches!(
            simulation.state().missions[0].result,
            Some(MissionResult::Attack(AttackMissionResult {
                outcome: AttackMissionOutcome::TargetInvalid(
                    AttackInvalidReason::TargetPresenceChanged
                ),
                secured: false,
                attackers_destroyed: false,
                ..
            }))
        ));
    }

    #[test]
    fn mission_progress_is_independent_from_frame_rate() {
        let (mut fast, fleet_id) = simulation_with_probe_fleet();
        let mut slow = fast.clone();
        let origin = fast.state().colonies[0].system_id;
        let target = neighboring_target(&fast);
        let action = GameAction::LaunchMission(MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::System(target),
            kind: MissionKind::Probe,
            departure_at: StrategicTick::ZERO,
        });
        fast.apply_player_action(action.clone());
        slow.apply_player_action(action);

        for _ in 0..210 {
            fast.advance(Duration::from_millis(100));
        }
        for _ in 0..21 {
            slow.advance(Duration::from_secs(1));
        }

        assert_eq!(fast.state(), slow.state());
        assert_eq!(fast.state().missions[0].phase, MissionPhase::Completed);
        assert_eq!(
            fast.state().fleet(fleet_id).unwrap().location,
            FleetLocation::Docked(fast.state().colonies[0].id),
        );
        assert_eq!(
            fast.state().mission_reports[0].outcome,
            MissionReportOutcome::Completed,
        );
        assert!(matches!(
            fast.state().mission_reports[0].result,
            Some(MissionResult::Probe(ProbeMissionResult {
                target: resolved,
                current: KnowledgeLevel::Probed,
                ..
            })) if resolved == MissionTarget::System(target)
        ));
        assert_eq!(
            fast.state().colonies[0].resources.stock(),
            ResourceStock::new(650, 325, 223),
        );
    }
}

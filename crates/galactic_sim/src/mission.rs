// MVP-023: deterministic missions with explicit discovery-frontier results.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use galactic_domain::{
    ColonyId, ExtractionSiteId, FactionId, FleetId, MissionId, Owner, PlanetId, ReservationId,
    ResourceCost, ResourceLedgerError, ResourceStock, SystemId,
};

use crate::{
    AttackBeginOutcome, AttackMissionCommitment, AttackMissionOutcome, AttackMissionResult,
    AuthorizationError, ColonizationBlocker, ColonizationMissionCommitment,
    ColonizationMissionOutcome, ColonizationMissionResult, ColonyEstablished, ColonyFoundation,
    CombatSnapshotError, CraftableId, FleetAssignment, FleetCompositionError, FleetError,
    FleetLocation, FleetState, GameState, KnowledgeChange, KnowledgeLevel, KnowledgeTarget,
    PlanetaryIntelPrecision, StrategicDuration, StrategicTick, TechnologyUnlock,
    UniverseRepository, assess_planet_colonizability, begin_attack, build_planet_analysis_report,
    colonization_arrival_blocker, combat_rules, extraction_rules,
    initialize_colony_from_foundation, planetary_analysis_rules, prepare_attack_commitment,
    refresh_planetary_intelligence, stock_for,
};

mod analyze;
mod attack;
mod colonize;
mod harvest;
mod probe;
mod transport;

pub use analyze::AnalyzeMissionResult;
pub use attack::launch_attack_mission;
pub use colonize::launch_colonization_mission;
pub use harvest::{
    HarvestCollectionStatus, HarvestMissionResult, HarvestMissionState, launch_harvest_mission,
};
pub use probe::{ProbeMissionResult, launch_probe_mission};
pub use transport::{
    TransportDeliveryStatus, TransportMissionResult, TransportMissionState,
    launch_transport_mission,
};

use analyze::validate_analyze_result;
use attack::{validate_attack_commitment, validate_attack_result};
use colonize::{validate_colonization_commitment, validate_colonize_result};
use harvest::{validate_harvest_launch, validate_harvest_result, validate_harvest_state};
use probe::validate_probe_result;
use transport::{validate_transport_launch, validate_transport_result, validate_transport_state};

/// Abstract distance crossed by a fleet for one route hop.
///
/// Dividing this work by `cruise_speed` yields strategic ticks. With the
/// default ruleset, a Luciole crosses one hop in 100 ticks (10 seconds at x1).
pub const MISSION_TRAVEL_WORK_PER_HOP: u64 = 16_000;
pub const MISSION_LOCAL_TRAVEL_WORK_BASE: u64 = 2_400;
pub const MISSION_LOCAL_TRAVEL_WORK_PER_ORBIT: u64 = 1_200;
pub const MISSION_RESOLUTION_TICKS: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MissionKind {
    Probe,
    Analyze,
    Attack,
    Transport,
    Harvest,
    Colonize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MissionOrder {
    pub fleet_id: FleetId,
    pub origin: SystemId,
    pub target: MissionTarget,
    pub kind: MissionKind,
    pub departure_at: StrategicTick,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MissionState {
    pub id: MissionId,
    pub owner: Owner,
    pub order: MissionOrder,
    pub origin_colony_id: ColonyId,
    pub plan: MissionPlan,
    pub phase: MissionPhase,
    pub phase_started_at: StrategicTick,
    pub fuel_reservation: Option<ReservationId>,
    pub foundation_reservation: Option<ReservationId>,
    pub cargo_reservation: Option<ReservationId>,
    pub attack: Option<AttackMissionCommitment>,
    pub colonization: Option<ColonizationMissionCommitment>,
    pub transport: Option<TransportMissionState>,
    pub harvest: Option<HarvestMissionState>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MissionReportOutcome {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MissionResult {
    Probe(ProbeMissionResult),
    Analyze(AnalyzeMissionResult),
    Attack(AttackMissionResult),
    Transport(TransportMissionResult),
    Harvest(HarvestMissionResult),
    Colonize(ColonizationMissionResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionResolution {
    pub mission_id: MissionId,
    pub result: MissionResult,
    pub occurred_at: StrategicTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    AnalyzePlanetTargetRequired,
    AnalyzeTargetNotProbed {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    MissingAnalyzeTechnology(TechnologyUnlock),
    AnalyzeSatelliteRequired(FleetId),
    AnalysisTargetBusy {
        planet_id: PlanetId,
        mission_id: MissionId,
    },
    AttackPlanetTargetRequired,
    AttackTargetNotAnalyzed {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    AttackFleetUnavailable(ColonyId),
    Attack(CombatSnapshotError),
    TransportOrderRequired,
    TransportCargoEmpty,
    TransportCargoAmountOverflow,
    UnknownTransportDestination(ColonyId),
    TransportDestinationIsOrigin(ColonyId),
    TransportDestinationTargetMismatch {
        colony_id: ColonyId,
        target: MissionTarget,
    },
    TransportFleetHasCargo(FleetId),
    TransportCargoExceedsCapacity {
        cargo: ResourceStock,
        capacity: u64,
    },
    HarvestOrderRequired,
    UnknownExtractionSite(ExtractionSiteId),
    HarvestTargetMismatch {
        site_id: ExtractionSiteId,
        target: MissionTarget,
    },
    HarvestPlanetNotAnalyzed {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    MissingHarvestTechnology(TechnologyUnlock),
    ExtractionSiteOnColony(ExtractionSiteId),
    ExtractionSiteDepleted(ExtractionSiteId),
    ExtractionSiteBusy {
        site_id: ExtractionSiteId,
        mission_id: MissionId,
    },
    HarvestFleetHasCargo(FleetId),
    ColonizationPlanetTargetRequired,
    ColonizationShipUnavailable(ColonyId),
    ColonizationFleetRequired(FleetId),
    ColonizationBlocked(ColonizationBlocker),
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
    MissingAnalyzeResult,
    AnalyzeResultTargetMismatch {
        expected: Option<PlanetId>,
        found: PlanetId,
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
    MissingCargoReservation,
    UnexpectedCargoReservation,
    MissingTransportState,
    UnexpectedTransportState,
    TransportPlanetTargetRequired,
    EmptyTransportCargo,
    InvalidTransportStatus {
        phase: MissionPhase,
        status: TransportDeliveryStatus,
    },
    TransportTargetMismatch {
        expected: MissionTarget,
        found: MissionTarget,
    },
    MissingTransportResult,
    TransportResultDestinationMismatch {
        expected: ColonyId,
        found: ColonyId,
    },
    TransportResultCargoMismatch,
    MissingHarvestState,
    UnexpectedHarvestState,
    HarvestPlanetTargetRequired,
    EmptyHarvestCargo,
    InvalidHarvestStatus {
        phase: MissionPhase,
        status: HarvestCollectionStatus,
    },
    HarvestTargetMismatch {
        expected: ExtractionSiteId,
        found: ExtractionSiteId,
    },
    MissingHarvestResult,
    HarvestResultSiteMismatch {
        expected: ExtractionSiteId,
        found: ExtractionSiteId,
    },
    HarvestResultCargoMismatch,
    MissingFoundationReservation,
    UnexpectedFoundationReservation,
    MissingColonizationCommitment,
    UnexpectedColonizationCommitment,
    ColonizationCommitmentTargetMismatch {
        expected: PlanetId,
        found: PlanetId,
    },
    ColonizationCommitmentShipMismatch {
        expected: CraftableId,
        found: CraftableId,
    },
    ColonizationCommitmentCostMismatch,
    MissingColonizationResult,
    ColonizationResultTargetMismatch {
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
    Foundation {
        recipient: FactionId,
        foundation: ColonyFoundation,
    },
    Colony {
        recipient: FactionId,
        colony: ColonyEstablished,
    },
    /// COMBAT-001-C: an attack mission arrived at a still-valid target and a
    /// `PendingCombat` was created — fired exactly once, at creation (never
    /// re-fired while the mission stays parked at `OnSite` awaiting a
    /// decision, see the `OnSite → Returning` guard).
    CombatPending {
        recipient: FactionId,
        mission_id: MissionId,
        planet_id: PlanetId,
        round: u16,
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
    if order.kind == MissionKind::Analyze {
        let Some(planet_id) = order.target.planet_id() else {
            return Err(MissionError::AnalyzePlanetTargetRequired);
        };
        let current = state.planet_knowledge_level(planet_id);
        if current != KnowledgeLevel::Probed {
            return Err(MissionError::AnalyzeTargetNotProbed { planet_id, current });
        }
        if !state.research.has_unlock(TechnologyUnlock::AnalyzePlanets) {
            return Err(MissionError::MissingAnalyzeTechnology(
                TechnologyUnlock::AnalyzePlanets,
            ));
        }
        if fleet.composition.total_ships()
            != fleet
                .composition
                .quantity(CraftableId::CARTOGRAPHER_SATELLITE)
        {
            return Err(MissionError::AnalyzeSatelliteRequired(fleet.id));
        }
        if let Some(existing) = state.missions.iter().find(|mission| {
            !mission.phase.is_terminal()
                && mission.order.kind == MissionKind::Analyze
                && mission.order.target.planet_id() == Some(planet_id)
        }) {
            return Err(MissionError::AnalysisTargetBusy {
                planet_id,
                mission_id: existing.id,
            });
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
    if order.kind == MissionKind::Colonize {
        let Some(planet_id) = order.target.planet_id() else {
            return Err(MissionError::ColonizationPlanetTargetRequired);
        };
        let colony_ship = planetary_analysis_rules().colony_ship();
        if fleet.composition.total_ships() != 1 || fleet.composition.quantity(colony_ship) != 1 {
            return Err(MissionError::ColonizationFleetRequired(fleet.id));
        }
        if let Some(blocker) = assess_planet_colonizability(state, universe, actor, planet_id)
            .blockers
            .into_iter()
            .next()
        {
            return Err(MissionError::ColonizationBlocked(blocker));
        }
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

    let resolution_ticks = match order.kind {
        MissionKind::Harvest => {
            let planet_id = order
                .target
                .planet_id()
                .ok_or(MissionError::HarvestOrderRequired)?;
            let planet = universe
                .planet(planet_id)
                .ok_or(MissionError::UnknownPlanetTarget(planet_id))?;
            extraction_rules().rule_for(planet.kind).harvest_ticks
        }
        MissionKind::Analyze => planetary_analysis_rules().analysis_duration().ticks(),
        MissionKind::Probe
        | MissionKind::Attack
        | MissionKind::Transport
        | MissionKind::Colonize => MISSION_RESOLUTION_TICKS,
    };
    let outbound_arrival_at = checked_tick_add(order.departure_at, travel_ticks)?;
    let return_departure_at = checked_tick_add(outbound_arrival_at, resolution_ticks)?;
    let return_arrival_at = checked_tick_add(return_departure_at, travel_ticks)?;

    Ok((
        origin_colony_id,
        MissionPlan {
            route,
            hops,
            travel_duration: StrategicDuration::from_ticks(travel_ticks),
            resolution_duration: StrategicDuration::from_ticks(resolution_ticks),
            fuel_cost,
            outbound_arrival_at,
            return_departure_at,
            return_arrival_at,
        },
    ))
}

pub fn fleet_supports_mission_kind(fleet: &FleetState, kind: MissionKind) -> bool {
    match kind {
        MissionKind::Probe => {
            fleet.composition.total_ships() == fleet.composition.quantity(CraftableId::LIGHT_PROBE)
        }
        MissionKind::Analyze => {
            fleet.composition.total_ships()
                == fleet
                    .composition
                    .quantity(CraftableId::CARTOGRAPHER_SATELLITE)
        }
        MissionKind::Attack => combat_rules().has_combat_ships(fleet),
        MissionKind::Colonize => {
            let colony_ship = planetary_analysis_rules().colony_ship();
            fleet.composition.total_ships() == 1 && fleet.composition.quantity(colony_ship) == 1
        }
        MissionKind::Transport | MissionKind::Harvest => {
            fleet.cargo.is_zero()
                && fleet
                    .capabilities()
                    .is_ok_and(|capabilities| capabilities.cargo_capacity > 0)
        }
    }
}

/// Lists every fleet the player could explicitly assign to a mission of the
/// given `kind` from `origin_colony_id` toward `target`.
///
/// A fleet is eligible when it is idle, docked at the origin, matches the
/// capability required by `kind` (a pure probe for [`MissionKind::Probe`], a
/// combat-capable fleet for [`MissionKind::Attack`], a single configured
/// colony ship for [`MissionKind::Colonize`], an empty cargo-capable fleet
/// for [`MissionKind::Transport`]/[`MissionKind::Harvest`]), and would
/// actually produce a valid [`MissionPlan`] (route, range and fuel all
/// checked via [`plan_mission`]). The result is never auto-picked or
/// auto-formed: it is only ever a list for the player to choose from.
pub fn eligible_fleets_for_mission(
    state: &GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    origin_colony_id: ColonyId,
    target: MissionTarget,
    kind: MissionKind,
) -> Vec<FleetId> {
    let Some(origin_colony) = state.colony(origin_colony_id) else {
        return Vec::new();
    };
    let origin = origin_colony.system_id;
    let departure_at = state.clock.current_tick();

    let mut eligible: Vec<FleetId> = state
        .fleets
        .iter()
        .filter(|fleet| {
            state.can_manage(actor, fleet.owner)
                && fleet.is_idle()
                && fleet.location == FleetLocation::Docked(origin_colony_id)
                && fleet_supports_mission_kind(fleet, kind)
        })
        .map(|fleet| fleet.id)
        .filter(|fleet_id| {
            plan_mission(
                state,
                universe,
                actor,
                MissionOrder {
                    fleet_id: *fleet_id,
                    origin,
                    target,
                    kind,
                    departure_at,
                },
            )
            .is_ok()
        })
        .collect();
    eligible.sort();
    eligible
}

/// Computes how many units of a site's resource a fleet could recover if a
/// harvest mission launched now, capped by the site's remaining reserve, the
/// extraction rule's per-mission maximum, and the fleet's cargo capacity.
///
/// Mirrors the formula applied at collection time when a harvest mission
/// actually returns, so the wizard can show the exact recoverable quantity
/// before launch.
pub fn harvest_recoverable_quantity(
    state: &GameState,
    universe: &UniverseRepository,
    site_id: ExtractionSiteId,
    fleet_id: FleetId,
) -> Result<u64, MissionError> {
    let site = state
        .extraction_site(site_id)
        .ok_or(MissionError::UnknownExtractionSite(site_id))?;
    let planet = universe
        .planet(site.planet_id)
        .ok_or(MissionError::UnknownPlanetTarget(site.planet_id))?;
    let rule = extraction_rules().rule_for(planet.kind);
    let capacity = state
        .fleet(fleet_id)
        .ok_or(MissionError::UnknownFleet(fleet_id))?
        .capabilities()
        .map_err(MissionError::InvalidFleetComposition)?
        .cargo_capacity;
    Ok(site.remaining.min(rule.maximum_harvest()).min(capacity))
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
    if order.kind == MissionKind::Transport {
        return Err(MissionError::TransportOrderRequired);
    }
    if order.kind == MissionKind::Harvest {
        return Err(MissionError::HarvestOrderRequired);
    }
    let mut candidate = state.clone();
    let launched = launch_mission_with_payload(&mut candidate, universe, actor, order, None, None)?;
    *state = candidate;
    Ok(launched)
}

fn launch_mission_with_payload(
    state: &mut GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    order: MissionOrder,
    transport: Option<TransportMissionState>,
    harvest: Option<HarvestMissionState>,
) -> Result<MissionLaunched, MissionError> {
    let (origin_colony_id, plan) = plan_mission(state, universe, actor, order)?;
    if let Some(transport) = transport {
        validate_transport_launch(state, actor, origin_colony_id, order, transport)?;
    }
    if let Some(harvest) = harvest {
        validate_harvest_launch(state, actor, origin_colony_id, order, harvest)?;
    }
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
    let colonization = if order.kind == MissionKind::Colonize {
        Some(ColonizationMissionCommitment {
            planet_id: order
                .target
                .planet_id()
                .expect("a planned colonization always targets a planet"),
            colony_ship: planetary_analysis_rules().colony_ship(),
            foundation_cost: planetary_analysis_rules().foundation_cost(),
        })
    } else {
        None
    };
    let fuel_reservation = state
        .colony_mut(origin_colony_id)
        .expect("mission origin was validated")
        .resources
        .reserve(plan.fuel_cost)
        .map_err(MissionError::Resources)?;
    let foundation_reservation = if let Some(commitment) = colonization {
        match state
            .colony_mut(origin_colony_id)
            .expect("mission origin was validated")
            .resources
            .reserve(commitment.foundation_cost)
        {
            Ok(reservation) => Some(reservation),
            Err(error) => {
                state
                    .colony_mut(origin_colony_id)
                    .expect("mission origin was validated")
                    .resources
                    .release(fuel_reservation)
                    .expect("the fresh fuel reservation can be rolled back");
                return Err(MissionError::Resources(error));
            }
        }
    } else {
        None
    };
    let cargo_reservation = if let Some(commitment) = transport {
        Some(
            state
                .colony_mut(origin_colony_id)
                .expect("mission origin was validated")
                .resources
                .reserve(commitment.cargo.into())
                .map_err(MissionError::Resources)?,
        )
    } else {
        None
    };
    let owner = state
        .fleet(order.fleet_id)
        .expect("mission fleet was validated")
        .owner;
    state
        .fleet_mut(order.fleet_id)
        .expect("mission fleet was validated")
        .assignment = FleetAssignment::Mission(mission_id);
    if let Some(harvest) = harvest
        && extraction_rules().reserves_sites()
    {
        state
            .extraction_site_mut(harvest.site_id)
            .expect("the harvest site was validated")
            .reserved_by = Some(mission_id);
    }
    state.next_mission_id = next_mission_id;
    state.missions.push(MissionState {
        id: mission_id,
        owner,
        order,
        origin_colony_id,
        plan: plan.clone(),
        phase: MissionPhase::Preparation,
        phase_started_at: state.clock.current_tick(),
        fuel_reservation: Some(fuel_reservation),
        foundation_reservation,
        cargo_reservation,
        attack,
        colonization,
        transport,
        harvest,
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
    let foundation_reservation = mission.foundation_reservation;
    let cargo_reservation = mission.cargo_reservation;
    let harvest_site_id = mission.harvest.map(|harvest| harvest.site_id);
    let origin_colony_id = mission.origin_colony_id;
    let fleet_id = mission.order.fleet_id;
    let kind = mission.order.kind;
    state
        .colony_mut(origin_colony_id)
        .expect("validated mission origin must exist")
        .resources
        .release(reservation)
        .map_err(MissionError::Resources)?;
    if let Some(reservation) = foundation_reservation {
        state
            .colony_mut(origin_colony_id)
            .expect("validated mission origin must exist")
            .resources
            .release(reservation)
            .map_err(MissionError::Resources)?;
    }
    if let Some(reservation) = cargo_reservation {
        state
            .colony_mut(origin_colony_id)
            .expect("validated mission origin must exist")
            .resources
            .release(reservation)
            .map_err(MissionError::Resources)?;
    }
    if let Some(site_id) = harvest_site_id
        && extraction_rules().reserves_sites()
    {
        state
            .extraction_site_mut(site_id)
            .expect("validated harvest site must exist")
            .reserved_by = None;
    }
    state
        .fleet_mut(fleet_id)
        .expect("validated mission fleet must exist")
        .assignment = FleetAssignment::Idle;

    let occurred_at = state.clock.current_tick();
    let mission = &mut state.missions[index];
    mission.phase = MissionPhase::Cancelled;
    mission.phase_started_at = occurred_at;
    mission.fuel_reservation = None;
    mission.foundation_reservation = None;
    mission.cargo_reservation = None;
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

fn resource_stock_total(stock: ResourceStock) -> Result<u64, MissionError> {
    stock
        .metal
        .checked_add(stock.crystal)
        .and_then(|total| total.checked_add(stock.fuel))
        .ok_or(MissionError::TransportCargoAmountOverflow)
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
            | (MissionPhase::OnSite, MissionPhase::Completed)
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
    has_pending_combat: bool,
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
    let colonization_reservation_expected = mission.order.kind == MissionKind::Colonize
        && matches!(
            mission.phase,
            MissionPhase::Preparation
                | MissionPhase::Outbound
                | MissionPhase::OnSite
                | MissionPhase::Returning
        );
    if colonization_reservation_expected && mission.foundation_reservation.is_none() {
        return Err(MissionStateError::MissingFoundationReservation);
    }
    if !colonization_reservation_expected && mission.foundation_reservation.is_some() {
        return Err(MissionStateError::UnexpectedFoundationReservation);
    }
    let cargo_reservation_expected =
        mission.order.kind == MissionKind::Transport && mission.phase == MissionPhase::Preparation;
    if cargo_reservation_expected && mission.cargo_reservation.is_none() {
        return Err(MissionStateError::MissingCargoReservation);
    }
    if !cargo_reservation_expected && mission.cargo_reservation.is_some() {
        return Err(MissionStateError::UnexpectedCargoReservation);
    }
    validate_transport_state(mission)?;
    validate_harvest_state(mission)?;
    match mission.order.kind {
        MissionKind::Probe => validate_probe_result(mission)?,
        MissionKind::Analyze => validate_analyze_result(mission)?,
        MissionKind::Attack => validate_attack_result(mission, has_pending_combat)?,
        MissionKind::Transport => validate_transport_result(mission)?,
        MissionKind::Harvest => validate_harvest_result(mission)?,
        MissionKind::Colonize => validate_colonize_result(mission)?,
    }
    validate_attack_commitment(mission)?;
    validate_colonization_commitment(mission)?;
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
                MissionPhase::OnSite
                    if matches!(
                        mission.result,
                        Some(MissionResult::Colonize(ColonizationMissionResult {
                            outcome: ColonizationMissionOutcome::FoundationPrepared,
                            ..
                        }))
                    ) =>
                {
                    (MissionPhase::Completed, mission.phase_started_at)
                }
                MissionPhase::OnSite
                    if current_tick >= mission.plan.return_departure_at
                        && state.pending_combat(mission.id).is_none() =>
                {
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
            let foundation_reservation = mission.foundation_reservation;
            let cargo_reservation = mission.cargo_reservation;
            let owner = mission
                .owner
                .faction()
                .expect("validated missions always have a faction owner");
            let kind = mission.order.kind;
            let mission_result = mission.result;
            let attack = mission.attack.clone();
            let colonization = mission.colonization;
            let transport = mission.transport;
            let harvest = mission.harvest;
            let mut knowledge_changes = Vec::new();
            let mut resolution = None;
            let mut prepared_foundation = None;
            let mut established_colony = None;
            let mut transport_update = None;
            let mut harvest_update = None;
            let mut combat_pending = None;

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
                    if let Some(transport) = transport {
                        state
                            .colony_mut(origin_colony_id)
                            .expect("validated mission origin exists")
                            .resources
                            .commit(
                                cargo_reservation
                                    .expect("a preparing transport reserves its cargo"),
                            )
                            .expect("validated transport cargo reservation commits");
                        state
                            .fleet_mut(fleet_id)
                            .expect("validated mission fleet exists")
                            .cargo = transport.cargo;
                    }
                }
                MissionPhase::OnSite => {
                    state
                        .fleet_mut(fleet_id)
                        .expect("validated mission fleet exists")
                        .location = FleetLocation::InSystem(target_system);
                    if kind == MissionKind::Transport {
                        let mut updated =
                            transport.expect("a validated transport has a transport state");
                        let destination_is_valid = state
                            .colony(updated.destination_colony_id)
                            .is_some_and(|destination| {
                                state.can_manage(owner, destination.owner)
                                    && target
                                        == (MissionTarget::Planet {
                                            system_id: destination.system_id,
                                            planet_id: destination.planet_id,
                                        })
                            });
                        let cargo = state
                            .fleet(fleet_id)
                            .expect("validated mission fleet exists")
                            .cargo;
                        let delivered = if destination_is_valid {
                            let destination = state
                                .colony(updated.destination_colony_id)
                                .expect("the destination was just validated");
                            let capacity = crate::storage_capacity(destination.buildings);
                            state
                                .colony_mut(updated.destination_colony_id)
                                .expect("the destination was just validated")
                                .resources
                                .credit_capped(cargo, capacity)
                        } else {
                            ResourceStock::ZERO
                        };
                        state
                            .fleet_mut(fleet_id)
                            .expect("validated mission fleet exists")
                            .cargo = cargo.saturating_sub(delivered);
                        updated.delivered = delivered;
                        updated.status = if !destination_is_valid {
                            TransportDeliveryStatus::DestinationInvalid
                        } else if delivered == updated.cargo {
                            TransportDeliveryStatus::Delivered
                        } else {
                            TransportDeliveryStatus::PartiallyDelivered
                        };
                        transport_update = Some(updated);
                    } else if kind == MissionKind::Probe {
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
                        // COMBAT-001-C: an invalid target still resolves
                        // instantly, exactly as before — no tactical screen
                        // is ever opened for it (doc §11). A valid target now
                        // starts a `PendingCombat` instead: `resolution` stays
                        // `None`, so the generic tail below does not set
                        // `mission.result` yet, and the `OnSite → Returning`
                        // guard a few match arms up keeps this mission
                        // parked at `OnSite` until a command
                        // (`ChooseCombatDoctrine`/`RetreatFromCombat`/
                        // `AutoResolveCombat`) finalizes it.
                        let commitment = attack
                            .as_ref()
                            .expect("a validated attack has a commitment");
                        let planet_id = commitment.defender.planet_id;
                        match begin_attack(state, mission_id, transition_at, commitment) {
                            AttackBeginOutcome::Invalid(result) => {
                                resolution = Some(MissionResolution {
                                    mission_id,
                                    result: MissionResult::Attack(result),
                                    occurred_at: transition_at,
                                });
                            }
                            AttackBeginOutcome::Pending => {
                                combat_pending = Some(planet_id);
                            }
                        }
                    } else if kind == MissionKind::Colonize {
                        let commitment =
                            colonization.expect("a validated colonization has a commitment");
                        let result = if let Some(blocker) = colonization_arrival_blocker(
                            state,
                            universe,
                            owner,
                            commitment.planet_id,
                        ) {
                            ColonizationMissionResult {
                                target: commitment.planet_id,
                                outcome: ColonizationMissionOutcome::TargetInvalid(blocker),
                                colony_ship_consumed: false,
                            }
                        } else {
                            state
                                .colony_mut(origin_colony_id)
                                .expect("validated mission origin exists")
                                .resources
                                .commit(
                                    foundation_reservation
                                        .expect("an active colonization reserves its payload"),
                                )
                                .expect("validated foundation reservation commits");
                            let foundation = ColonyFoundation {
                                mission_id,
                                owner,
                                source_colony_id: origin_colony_id,
                                system_id: target_system,
                                planet_id: commitment.planet_id,
                                payload: commitment.foundation_cost.as_stock(),
                                prepared_at: transition_at,
                            };
                            state.colony_foundations.push(foundation);
                            state
                                .colony_foundations
                                .sort_by_key(|entry| (entry.planet_id, entry.mission_id));
                            let (colony, colony_knowledge_changes) =
                                initialize_colony_from_foundation(state, universe, foundation);
                            knowledge_changes.extend(colony_knowledge_changes);
                            state.fleets.retain(|fleet| fleet.id != fleet_id);
                            prepared_foundation = Some(foundation);
                            established_colony = Some(colony);
                            ColonizationMissionResult {
                                target: commitment.planet_id,
                                outcome: ColonizationMissionOutcome::FoundationPrepared,
                                colony_ship_consumed: true,
                            }
                        };
                        resolution = Some(MissionResolution {
                            mission_id,
                            result: MissionResult::Colonize(result),
                            occurred_at: transition_at,
                        });
                    }
                }
                MissionPhase::Returning => {
                    if kind == MissionKind::Harvest {
                        let mut updated = harvest.expect("a validated harvest has a harvest state");
                        let amount = harvest_recoverable_quantity(
                            state,
                            universe,
                            updated.site_id,
                            fleet_id,
                        )
                        .expect("a validated harvest mission has a valid site and fleet");
                        let site = state
                            .extraction_site_mut(updated.site_id)
                            .expect("a validated harvest site exists");
                        site.remaining = site.remaining.saturating_sub(amount);
                        site.reserved_by = None;
                        let site_remaining = site.remaining;
                        let collected = stock_for(site.resource, amount);
                        state
                            .fleet_mut(fleet_id)
                            .expect("validated mission fleet exists")
                            .cargo = collected;
                        updated.collected = collected;
                        updated.site_remaining = site_remaining;
                        updated.status = if site_remaining == 0 {
                            HarvestCollectionStatus::SiteDepleted
                        } else {
                            HarvestCollectionStatus::Collected
                        };
                        harvest_update = Some(updated);
                    }
                }
                MissionPhase::Completed => {
                    if matches!(
                        mission_result,
                        Some(MissionResult::Colonize(ColonizationMissionResult {
                            outcome: ColonizationMissionOutcome::FoundationPrepared,
                            ..
                        }))
                    ) {
                        debug_assert!(state.fleet(fleet_id).is_none());
                    } else {
                        if kind == MissionKind::Colonize {
                            state
                                .colony_mut(origin_colony_id)
                                .expect("validated mission origin exists")
                                .resources
                                .release(
                                    foundation_reservation
                                        .expect("a returning colonization retains its payload"),
                                )
                                .expect("validated foundation reservation releases");
                        }
                        if kind == MissionKind::Analyze {
                            let planet_id = target
                                .planet_id()
                                .expect("a validated analysis targets a planet");
                            let planet = universe
                                .planet(planet_id)
                                .expect("a validated analysis target exists");
                            let previous = state.planet_knowledge_level(planet_id);
                            let report = state
                                .planet_analysis_report(planet_id)
                                .copied()
                                .unwrap_or_else(|| {
                                    build_planet_analysis_report(
                                        planet,
                                        target_system,
                                        transition_at,
                                        planetary_analysis_rules(),
                                    )
                                });
                            if !previous.reveals_exact_details() {
                                knowledge_changes = state.advance_planet_knowledge(
                                    universe,
                                    planet_id,
                                    KnowledgeLevel::Analyzed,
                                );
                                state.planet_analysis_reports.push(report);
                                state
                                    .planet_analysis_reports
                                    .sort_by_key(|entry| entry.planet_id);
                            }
                            refresh_planetary_intelligence(
                                state,
                                planet_id,
                                PlanetaryIntelPrecision::Surveyed,
                                transition_at,
                            )
                            .expect("a completed analysis has a validated planetary presence");
                            resolution = Some(MissionResolution {
                                mission_id,
                                result: MissionResult::Analyze(AnalyzeMissionResult {
                                    planet_id,
                                    previous,
                                    current: state.planet_knowledge_level(planet_id),
                                    report,
                                }),
                                occurred_at: transition_at,
                            });
                        } else if let Some(transport) = transport {
                            let cargo = state
                                .fleet(fleet_id)
                                .expect("validated mission fleet exists")
                                .cargo;
                            let origin = state
                                .colony(origin_colony_id)
                                .expect("validated mission origin exists");
                            let capacity = crate::storage_capacity(origin.buildings);
                            let returned = state
                                .colony_mut(origin_colony_id)
                                .expect("validated mission origin exists")
                                .resources
                                .credit_capped(cargo, capacity);
                            let retained = cargo.saturating_sub(returned);
                            state
                                .fleet_mut(fleet_id)
                                .expect("validated mission fleet exists")
                                .cargo = retained;
                            resolution = Some(MissionResolution {
                                mission_id,
                                result: MissionResult::Transport(TransportMissionResult {
                                    destination_colony_id: transport.destination_colony_id,
                                    requested: transport.cargo,
                                    delivered: transport.delivered,
                                    returned,
                                    retained,
                                    status: transport.status,
                                }),
                                occurred_at: transition_at,
                            });
                        } else if let Some(harvest) = harvest {
                            let cargo = state
                                .fleet(fleet_id)
                                .expect("validated mission fleet exists")
                                .cargo;
                            let origin = state
                                .colony(origin_colony_id)
                                .expect("validated mission origin exists");
                            let capacity = crate::storage_capacity(origin.buildings);
                            let delivered = state
                                .colony_mut(origin_colony_id)
                                .expect("validated mission origin exists")
                                .resources
                                .credit_capped(cargo, capacity);
                            let retained = cargo.saturating_sub(delivered);
                            state
                                .fleet_mut(fleet_id)
                                .expect("validated mission fleet exists")
                                .cargo = retained;
                            resolution = Some(MissionResolution {
                                mission_id,
                                result: MissionResult::Harvest(HarvestMissionResult {
                                    site_id: harvest.site_id,
                                    collected: harvest.collected,
                                    delivered,
                                    retained,
                                    site_remaining: harvest.site_remaining,
                                    status: harvest.status,
                                }),
                                occurred_at: transition_at,
                            });
                        }
                        let fleet = state
                            .fleet_mut(fleet_id)
                            .expect("validated mission fleet exists");
                        fleet.location = FleetLocation::Docked(origin_colony_id);
                        fleet.assignment = FleetAssignment::Idle;
                    }
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
                mission.cargo_reservation = None;
            }
            if next_phase.is_terminal() {
                mission.foundation_reservation = None;
                mission.cargo_reservation = None;
            }
            if let Some(transport) = transport_update {
                mission.transport = Some(transport);
            }
            if let Some(harvest) = harvest_update {
                mission.harvest = Some(harvest);
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
            if let Some(foundation) = prepared_foundation {
                events.push(MissionEngineEvent::Foundation {
                    recipient: owner,
                    foundation,
                });
            }
            if let Some(colony) = established_colony {
                events.push(MissionEngineEvent::Colony {
                    recipient: owner,
                    colony,
                });
            }
            if let Some(planet_id) = combat_pending {
                events.push(MissionEngineEvent::CombatPending {
                    recipient: owner,
                    mission_id,
                    planet_id,
                    round: 1,
                });
            }
            if matches!(next_phase, MissionPhase::Completed | MissionPhase::Failed) {
                let final_result = resolution.map(|value| value.result).or(mission_result);
                let outcome = if next_phase == MissionPhase::Failed
                    || matches!(
                        final_result,
                        Some(MissionResult::Attack(AttackMissionResult {
                            outcome: AttackMissionOutcome::TargetInvalid(_),
                            ..
                        })) | Some(MissionResult::Colonize(ColonizationMissionResult {
                            outcome: ColonizationMissionOutcome::TargetInvalid(_),
                            ..
                        })) | Some(MissionResult::Transport(TransportMissionResult {
                            status: TransportDeliveryStatus::DestinationInvalid,
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
                    result: final_result,
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

    use galactic_domain::{
        FactionId, Owner, PlanetId, ResourceLedger, ResourceStock, UniverseConfig,
    };

    use crate::{
        AttackInvalidReason, AttackMissionOutcome, CombatOutcome, CombatReportStatus, CraftableId,
        FleetComposition, GameAction, KnowledgeLevel, PlanetaryForceLoss, ResearchState,
        RetreatingSide, ShipStack, Simulation, TechnologyId, analyze_planet,
        apply_planetary_force_losses, default_building_catalog, disband_fleet,
        enqueue_building_upgrade, form_fleet, retreat_from_combat,
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

    fn advance_ticks(simulation: &mut Simulation, ticks: u64) {
        simulation.advance(Duration::from_millis(ticks.saturating_mul(100)));
    }

    fn first_detected_planet(simulation: &Simulation) -> PlanetId {
        simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains detected planets")
            .planet_id
    }

    fn simulation_with_analysis_fleet() -> (Simulation, PlanetId, FleetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let target = first_detected_planet(&simulation);
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.state_mut().colonies[0]
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test fuel fits the ledger");
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::CARTOGRAPHER_SATELLITE, 1);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::CARTOGRAPHER_SATELLITE, 1)])
                .expect("cartographer satellite fleet composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("analysis fleet can be formed");
        (simulation, target, created.fleet_id)
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

    fn simulation_with_colonization_target() -> (Simulation, PlanetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_systems = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .to_vec();
        let rules = planetary_analysis_rules();
        let target = simulation
            .universe()
            .systems
            .iter()
            .filter(|system| neighboring_systems.contains(&system.id))
            .flat_map(|system| system.planets.iter())
            .find(|planet| {
                rules.rule_for(planet.kind).colonizable
                    && planet.habitability >= rules.minimum_habitability()
            })
            .expect("a neighboring planet supports the colonization mission")
            .id;
        let presence = simulation
            .state_mut()
            .planetary_presence_mut(target)
            .expect("every planet has a mutable presence");
        presence.occupant = Owner::Unowned;
        presence.population = 0;
        presence.forces.clear();
        presence.revision = presence
            .revision
            .checked_add(1)
            .expect("test presence revision remains representable");

        let repository = simulation.universe_repository().clone();
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::PLANETARY_ANALYSIS,
            TechnologyId::COLONIZATION,
        ]);
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        analyze_planet(simulation.state_mut(), &repository, actor, target)
            .expect("the target follows the real analysis path");
        simulation.state_mut().colonies[0]
            .resources
            .credit(ResourceStock::new(100, 300, 200))
            .expect("test resources fit the ledger");
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::COLONY_SHIP, 1);
        (simulation, target)
    }

    fn simulation_with_two_colonies() -> (Simulation, FleetId) {
        let (mut simulation, target) = simulation_with_colonization_target();
        let origin_colony_id = simulation.state().colonies[0].id;
        simulation.apply_player_action(GameAction::LaunchColonization {
            colony_id: origin_colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        simulation.advance(Duration::from_secs(120));
        assert_eq!(simulation.state().colonies.len(), 2);
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_CARGO, 1);
        let actor = simulation.state().player_faction;
        let composition =
            FleetComposition::from_stacks([crate::ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("one light cargo is a valid composition");
        let created = form_fleet(simulation.state_mut(), actor, origin_colony_id, composition)
            .expect("the origin colony has one light cargo docked");
        (simulation, created.fleet_id)
    }

    fn simulation_with_harvest_site() -> (Simulation, ExtractionSiteId, FleetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_system = neighboring_target(&simulation);
        let target = simulation
            .universe()
            .system(neighboring_system)
            .expect("the neighboring system exists")
            .planets[0]
            .id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::REMOTE_EXTRACTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        analyze_planet(simulation.state_mut(), &repository, actor, target)
            .expect("the harvest target follows the real analysis path");
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_CARGO, 2);
        assert_eq!(simulation.state().colonies[0].system_id, origin);
        let origin_colony_id = simulation.state().colonies[0].id;
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("one light cargo is a valid composition");
        let created = form_fleet(simulation.state_mut(), actor, origin_colony_id, composition)
            .expect("the origin colony has light cargo docked");
        (
            simulation,
            ExtractionSiteId::for_planet(target),
            created.fleet_id,
        )
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
    fn generic_non_colonization_missions_use_the_shared_planner() {
        for kind in [MissionKind::Probe, MissionKind::Transport] {
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
    fn analysis_mission_requires_probe_technology_and_satellite() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = first_detected_planet(&simulation);
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::CARTOGRAPHER_SATELLITE, 1);
        let satellite_fleet = form_fleet(
            simulation.state_mut(),
            actor,
            colony_id,
            FleetComposition::from_stacks([ShipStack::new(CraftableId::CARTOGRAPHER_SATELLITE, 1)])
                .expect("satellite composition is valid"),
        )
        .expect("satellite fleet forms")
        .fleet_id;
        let order = MissionOrder {
            fleet_id: satellite_fleet,
            origin,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
            kind: MissionKind::Analyze,
            departure_at: simulation.state().clock.current_tick(),
        };

        assert!(matches!(
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                order
            ),
            Err(MissionError::AnalyzeTargetNotProbed { .. })
        ));

        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        assert!(matches!(
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                order
            ),
            Err(MissionError::MissingAnalyzeTechnology(
                TechnologyUnlock::AnalyzePlanets
            ))
        ));

        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_PROBE, 1);
        let probe_fleet = form_fleet(
            simulation.state_mut(),
            actor,
            colony_id,
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("probe composition is valid"),
        )
        .expect("probe fleet forms")
        .fleet_id;
        let wrong_fleet_order = MissionOrder {
            fleet_id: probe_fleet,
            ..order
        };
        assert!(matches!(
            plan_mission(
                simulation.state(),
                simulation.universe_repository(),
                actor,
                wrong_fleet_order
            ),
            Err(MissionError::AnalyzeSatelliteRequired(_))
        ));
    }

    #[test]
    fn analysis_report_is_revealed_only_after_return() {
        let (mut simulation, target, fleet_id) = simulation_with_analysis_fleet();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].system_id;
        let order = MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
            kind: MissionKind::Analyze,
            departure_at: simulation.state().clock.current_tick(),
        };
        let repository = simulation.universe_repository().clone();
        launch_mission(simulation.state_mut(), &repository, actor, order)
            .expect("analysis mission launches");
        let plan = simulation.state().missions[0].plan.clone();
        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert!(simulation.state().planet_analysis_report(target).is_none());

        advance_ticks(&mut simulation, plan.outbound_arrival_at.value());
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::OnSite);
        assert_eq!(simulation.state().missions[0].result, None);
        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert!(simulation.state().planet_analysis_report(target).is_none());

        let to_return_departure = plan
            .return_departure_at
            .value()
            .saturating_sub(simulation.state().clock.current_tick().value());
        advance_ticks(&mut simulation, to_return_departure);
        assert_eq!(
            simulation.state().missions[0].phase,
            MissionPhase::Returning
        );
        assert_eq!(simulation.state().missions[0].result, None);
        assert!(simulation.state().planet_analysis_report(target).is_none());

        let to_return_arrival = plan
            .return_arrival_at
            .value()
            .saturating_sub(simulation.state().clock.current_tick().value());
        advance_ticks(&mut simulation, to_return_arrival);
        assert_eq!(
            simulation.state().missions[0].phase,
            MissionPhase::Completed
        );
        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Analyzed
        );
        let report = *simulation
            .state()
            .planet_analysis_report(target)
            .expect("completed analysis creates a report");
        let Some(MissionResult::Analyze(result)) = simulation.state().missions[0].result else {
            panic!("analysis mission keeps its report result");
        };
        assert_eq!(result.planet_id, target);
        assert_eq!(result.previous, KnowledgeLevel::Probed);
        assert_eq!(result.current, KnowledgeLevel::Analyzed);
        assert_eq!(result.report, report);
        assert_eq!(
            simulation.state().mission_reports[0].result,
            Some(MissionResult::Analyze(result))
        );
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
                    kind: MissionKind::Transport,
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
        // COMBAT-001-C: arrival now opens a pending combat instead of
        // resolving synchronously — auto-resolve it (mirrors the temporary
        // client-side auto-pilot bridge) before the mission can return.
        simulation.apply_player_action(GameAction::AutoResolveCombat { mission_id });
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
    fn mixed_transport_and_military_fleet_is_eligible_for_attack() {
        let (mut simulation, target) = simulation_with_attack_target();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::LIGHT_CARGO, 1);
        let composition = FleetComposition::from_stacks([
            ShipStack::new(CraftableId::LIGHT_CARGO, 1),
            ShipStack::new(CraftableId::FRIGATE_BULWARK, 1),
        ])
        .expect("mixed fleet composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("mixed fleet can be formed");
        let target = MissionTarget::Planet {
            system_id: target.system_id(),
            planet_id: target,
        };

        let fleet = simulation.state().fleet(created.fleet_id).unwrap();
        assert!(fleet_supports_mission_kind(fleet, MissionKind::Attack));
        assert_eq!(
            eligible_fleets_for_mission(
                simulation.state(),
                &repository,
                actor,
                colony_id,
                target,
                MissionKind::Attack,
            ),
            vec![created.fleet_id],
        );
        assert!(
            plan_mission(
                simulation.state(),
                &repository,
                actor,
                MissionOrder {
                    fleet_id: created.fleet_id,
                    origin,
                    target,
                    kind: MissionKind::Attack,
                    departure_at: simulation.state().clock.current_tick(),
                },
            )
            .is_ok()
        );
    }

    #[test]
    fn returned_attack_fleet_with_survivors_can_be_disbanded() {
        let (mut simulation, target) = simulation_with_attack_target();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        let fleet_id = simulation.state().missions[0].order.fleet_id;
        let mission_id = simulation.state().missions[0].id;
        simulation.advance(Duration::from_secs(120));
        // COMBAT-001-C: arrival now opens a pending combat instead of
        // resolving synchronously — auto-resolve it before the mission can
        // return.
        simulation.apply_player_action(GameAction::AutoResolveCombat { mission_id });
        simulation.advance(Duration::from_secs(120));
        let returned = simulation
            .state()
            .fleet(fleet_id)
            .expect("the attacker fleet survived and returned");
        assert_eq!(returned.location, FleetLocation::Docked(colony_id));
        assert_eq!(returned.assignment, FleetAssignment::Idle);
        assert!(
            returned.composition.total_ships() < 3,
            "the test scenario should exercise a reduced surviving fleet",
        );

        disband_fleet(simulation.state_mut(), actor, fleet_id)
            .expect("a returned survivor fleet can be disbanded");

        assert!(simulation.state().fleet(fleet_id).is_none());
    }

    #[test]
    fn arrival_creates_exactly_one_pending_combat_and_locks_the_mission_at_on_site() {
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

        simulation.advance(Duration::from_secs(120));
        assert_eq!(simulation.state().pending_combats.len(), 1);
        assert_eq!(
            simulation
                .state()
                .pending_combat(mission_id)
                .unwrap()
                .planet_id,
            target
        );
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::OnSite);
        assert!(simulation.state().missions[0].result.is_none());
        assert!(simulation.state().combat_report(mission_id).is_none());

        // The mission stays parked here across arbitrarily many further
        // ticks — no round pacing is tied to strategic time (doc §11).
        simulation.advance(Duration::from_secs(600));
        assert_eq!(simulation.state().pending_combats.len(), 1);
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::OnSite);

        // The pending combat is created exactly once — a further advance
        // does not duplicate it.
        simulation.advance(Duration::from_secs(600));
        assert_eq!(simulation.state().pending_combats.len(), 1);
    }

    #[test]
    fn retreating_keeps_survivors_and_resumes_the_mission() {
        let (mut simulation, target) = simulation_with_attack_target();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        let mission_id = simulation.state().missions[0].id;
        let fleet_id = simulation.state().missions[0].order.fleet_id;
        simulation.advance(Duration::from_secs(120));
        assert_eq!(simulation.state().pending_combats.len(), 1);

        let result = retreat_from_combat(simulation.state_mut(), actor, mission_id)
            .expect("the attacker can retreat while a decision is pending");
        assert!(matches!(
            result.outcome,
            AttackMissionOutcome::Resolved(CombatOutcome::Retreat {
                retreating_side: RetreatingSide::Attacker,
            })
        ));
        assert!(!result.secured);
        assert_eq!(simulation.state().pending_combats.len(), 0);
        assert_eq!(
            simulation
                .state()
                .planetary_presence(target)
                .unwrap()
                .occupant,
            simulation
                .state()
                .combat_report(mission_id)
                .unwrap()
                .defender
                .occupant,
            "a retreat never secures the target"
        );

        simulation.advance(Duration::from_secs(120));
        assert_eq!(
            simulation.state().missions[0].phase,
            MissionPhase::Completed
        );
        let fleet = simulation
            .state()
            .fleet(fleet_id)
            .expect("a retreat keeps the surviving attacker fleet");
        assert_eq!(fleet.location, FleetLocation::Docked(colony_id));
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
    fn colonization_launch_reserves_payload_and_is_atomic_without_a_ship() {
        let (mut simulation, target) = simulation_with_colonization_target();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        let mission_target = MissionTarget::Planet {
            system_id: target.system_id(),
            planet_id: target,
        };
        simulation.state_mut().colonies[0]
            .inventory
            .take(CraftableId::COLONY_SHIP, 1);
        let before = simulation.state().clone();

        assert_eq!(
            launch_colonization_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                mission_target,
            ),
            Err(MissionError::ColonizationShipUnavailable(colony_id)),
        );
        assert_eq!(simulation.state(), &before);

        simulation.state_mut().colonies[0]
            .inventory
            .add(CraftableId::COLONY_SHIP, 1);
        let stock_before = simulation.state().colonies[0].resources.stock();
        let (_, launched) = launch_colonization_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            mission_target,
        )
        .expect("an eligible target accepts one colony ship");

        let reserved = simulation.state().colonies[0].resources.reserved_total();
        assert_eq!(
            reserved,
            launched
                .fuel_cost
                .as_stock()
                .checked_add(planetary_analysis_rules().foundation_cost().as_stock())
                .expect("the mission reservation remains representable"),
        );
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            stock_before,
        );
        assert_eq!(
            simulation
                .state()
                .fleet(launched.fleet_id)
                .expect("the colony ship has formed a fleet")
                .composition
                .quantity(CraftableId::COLONY_SHIP),
            1,
        );

        cancel_mission(simulation.state_mut(), actor, launched.mission_id)
            .expect("a colony ship can be recalled before departure");
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            stock_before,
        );
        assert!(
            simulation.state().colonies[0]
                .resources
                .reservations()
                .is_empty()
        );
        assert!(
            simulation
                .state()
                .fleet(launched.fleet_id)
                .expect("a cancelled colony ship remains available")
                .is_idle()
        );
    }

    #[test]
    fn successful_colonization_consumes_ship_and_payload_once() {
        let (mut simulation, target) = simulation_with_colonization_target();
        let colony_id = simulation.state().colonies[0].id;
        let stock_before = simulation.state().colonies[0].resources.stock();
        let events = simulation.apply_player_action(GameAction::LaunchColonization {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        let launched = events
            .iter()
            .find_map(|event| match event.kind {
                crate::GameEventKind::MissionLaunched(launched) => Some(launched),
                _ => None,
            })
            .expect("the colonization mission launches");
        let repository = simulation.universe_repository().clone();
        advance_missions(simulation.state_mut(), &repository, launched.departure_at);
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            stock_before
                .checked_sub(launched.fuel_cost)
                .expect("departure consumes only mission fuel"),
        );
        let arrival_at = simulation.state().missions[0].plan.outbound_arrival_at;

        let events = advance_missions(simulation.state_mut(), &repository, arrival_at);

        assert!(events.iter().any(|event| matches!(
            event,
            MissionEngineEvent::Foundation {
                foundation: ColonyFoundation {
                    mission_id,
                    planet_id,
                    ..
                },
                ..
            } if *mission_id == launched.mission_id && *planet_id == target
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            MissionEngineEvent::Colony {
                colony: ColonyEstablished {
                    mission_id,
                    planet_id,
                    ..
                },
                ..
            } if *mission_id == launched.mission_id && *planet_id == target
        )));
        assert!(simulation.state().fleet(launched.fleet_id).is_none());
        assert_eq!(simulation.state().colony_foundations.len(), 1);
        assert_eq!(
            simulation.state().colony_foundations[0].payload,
            planetary_analysis_rules().foundation_cost().as_stock(),
        );
        assert_eq!(simulation.state().colonies.len(), 2);
        let established = simulation
            .state()
            .colony_on_planet(target)
            .expect("the prepared foundation becomes a playable colony");
        assert_eq!(established.id, ColonyId::new(1));
        assert_eq!(established.founding_mission_id, Some(launched.mission_id));
        assert_eq!(
            established.resources.stock(),
            planetary_analysis_rules().foundation_cost().as_stock(),
        );
        assert_eq!(
            established.buildings,
            planetary_analysis_rules().colony_initialization().buildings,
        );
        assert_eq!(
            established.energy,
            default_building_catalog().energy_grid_for_levels(established.buildings),
        );
        assert_eq!(
            established.resource_profile,
            simulation
                .state()
                .planet_analysis_report(target)
                .expect("the target analysis remains available")
                .resource_profile,
        );
        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Colonized,
        );
        let presence = simulation
            .state()
            .planetary_presence(target)
            .expect("the colony controls its planet");
        assert_eq!(
            presence.occupant,
            Owner::Faction(simulation.state().player_faction)
        );
        assert_eq!(
            presence.population,
            planetary_analysis_rules()
                .colony_initialization()
                .population,
        );
        assert!(presence.forces.is_empty());
        let home_stock_before_upgrade = simulation.state().colonies[0].resources.stock();
        let new_colony_id = established.id;
        let actor = simulation.state().player_faction;
        enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            new_colony_id,
            crate::BuildingKind::METAL_MINE,
        )
        .expect("the new colony can queue its first independent construction");
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            home_stock_before_upgrade,
        );
        assert_eq!(
            simulation
                .state()
                .colony(new_colony_id)
                .expect("the new colony remains available")
                .construction_queue
                .len(),
            1,
        );
        assert!(matches!(
            simulation.state().missions[0].result,
            Some(MissionResult::Colonize(ColonizationMissionResult {
                outcome: ColonizationMissionOutcome::FoundationPrepared,
                colony_ship_consumed: true,
                ..
            }))
        ));
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            stock_before
                .checked_sub(launched.fuel_cost)
                .and_then(|stock| {
                    stock.checked_sub(planetary_analysis_rules().foundation_cost())
                })
                .expect("the prepared mission consumes fuel and payload"),
        );
        let blockers = assess_planet_colonizability(
            simulation.state(),
            simulation.universe_repository(),
            simulation.state().player_faction,
            target,
        )
        .blockers;
        assert!(blockers.contains(&ColonizationBlocker::AlreadyColonized));
        assert!(!blockers.contains(&ColonizationBlocker::FoundationAlreadyPrepared));
        let events = simulation.advance(Duration::from_secs(120));
        assert_eq!(simulation.state().colony_foundations.len(), 1);
        assert!(!events.iter().any(|event| matches!(
            event.kind,
            crate::GameEventKind::ColonyFoundationPrepared(_)
        )));
    }

    #[test]
    fn hostile_planet_refuses_colonization_without_mutating_state() {
        let (mut simulation, target) = simulation_with_colonization_target();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        let foreign = FactionId::new(2);
        let presence = simulation
            .state_mut()
            .planetary_presence_mut(target)
            .expect("the target presence exists");
        presence.occupant = Owner::Faction(foreign);
        presence.population = 2_000;
        presence.revision = presence
            .revision
            .checked_add(1)
            .expect("test revision remains representable");
        let before = simulation.state().clone();

        assert!(matches!(
            launch_colonization_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                MissionTarget::Planet {
                    system_id: target.system_id(),
                    planet_id: target,
                },
            ),
            Err(MissionError::ColonizationBlocked(
                ColonizationBlocker::OccupiedPlanet { occupant, .. }
            )) if occupant == foreign
        ));
        assert_eq!(simulation.state(), &before);
    }

    #[test]
    fn invalidated_colonization_returns_ship_and_releases_payload() {
        let (mut simulation, target) = simulation_with_colonization_target();
        let colony_id = simulation.state().colonies[0].id;
        let stock_before = simulation.state().colonies[0].resources.stock();
        let events = simulation.apply_player_action(GameAction::LaunchColonization {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        let launched = events
            .iter()
            .find_map(|event| match event.kind {
                crate::GameEventKind::MissionLaunched(launched) => Some(launched),
                _ => None,
            })
            .expect("the colonization mission launches");
        let repository = simulation.universe_repository().clone();
        let foreign = FactionId::new(2);
        let presence = simulation
            .state_mut()
            .planetary_presence_mut(target)
            .expect("the target presence exists");
        presence.occupant = Owner::Faction(foreign);
        presence.population = 2_000;
        presence.revision = presence
            .revision
            .checked_add(1)
            .expect("test revision remains representable");

        advance_missions(simulation.state_mut(), &repository, launched.departure_at);
        let return_at = simulation.state().missions[0].plan.return_arrival_at;
        advance_missions(simulation.state_mut(), &repository, return_at);

        assert!(simulation.state().colony_foundations.is_empty());
        let fleet = simulation
            .state()
            .fleet(launched.fleet_id)
            .expect("a rejected foundation returns its colony ship");
        assert!(fleet.is_idle());
        assert_eq!(fleet.location, FleetLocation::Docked(colony_id));
        assert_eq!(fleet.composition.quantity(CraftableId::COLONY_SHIP), 1);
        assert!(matches!(
            simulation.state().missions[0].result,
            Some(MissionResult::Colonize(ColonizationMissionResult {
                outcome: ColonizationMissionOutcome::TargetInvalid(
                    ColonizationBlocker::OccupiedPlanet { occupant, .. }
                ),
                colony_ship_consumed: false,
                ..
            })) if occupant == foreign
        ));
        assert_eq!(
            simulation.state().colonies[0].resources.stock(),
            stock_before
                .checked_sub(launched.fuel_cost)
                .expect("only mission fuel is consumed"),
        );
        assert!(
            simulation.state().colonies[0]
                .resources
                .reservations()
                .is_empty()
        );
    }

    #[test]
    fn transport_requires_stock_and_capacity_without_partial_mutation() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let repository = simulation.universe_repository().clone();
        let before = simulation.state().clone();
        let oversized = ResourceStock::new(801, 0, 0);

        assert_eq!(
            launch_transport_mission(
                simulation.state_mut(),
                &repository,
                actor,
                origin,
                destination,
                fleet_id,
                oversized,
            ),
            Err(MissionError::TransportCargoExceedsCapacity {
                cargo: oversized,
                capacity: 500,
            }),
        );
        assert_eq!(simulation.state(), &before);

        simulation.state_mut().colony_mut(origin).unwrap().resources =
            ResourceLedger::new(ResourceStock::new(20, 20, 100));
        let before_low_stock = simulation.state().clone();
        let cargo = ResourceStock::new(21, 0, 0);
        assert!(matches!(
            launch_transport_mission(
                simulation.state_mut(),
                &repository,
                actor,
                origin,
                destination,
                fleet_id,
                cargo,
            ),
            Err(MissionError::Resources(
                ResourceLedgerError::InsufficientResources { .. }
            )),
        ));
        assert_eq!(simulation.state(), &before_low_stock);
    }

    #[test]
    fn transport_cancellation_releases_every_reservation_without_loss() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let origin_before = simulation.state().colony(origin).unwrap().resources.stock();
        let cargo = ResourceStock::new(275, 125, 20);
        let repository = simulation.universe_repository().clone();
        let launched = launch_transport_mission(
            simulation.state_mut(),
            &repository,
            actor,
            origin,
            destination,
            fleet_id,
            cargo,
        )
        .expect("one light cargo can reserve the transport");
        assert!(
            !simulation
                .state()
                .colony(origin)
                .unwrap()
                .resources
                .reservations()
                .is_empty()
        );

        cancel_mission(simulation.state_mut(), actor, launched.mission_id)
            .expect("a transport can be cancelled before departure");

        assert_eq!(
            simulation.state().colony(origin).unwrap().resources.stock(),
            origin_before,
        );
        assert!(
            simulation
                .state()
                .colony(origin)
                .unwrap()
                .resources
                .reservations()
                .is_empty()
        );
        let fleet = simulation.state().fleet(launched.fleet_id).unwrap();
        assert!(fleet.cargo.is_zero());
        assert!(fleet.is_idle());
        assert_eq!(fleet.location, FleetLocation::Docked(origin));
    }

    #[test]
    fn transport_delivers_once_and_accounts_for_every_resource() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let origin_before = simulation.state().colony(origin).unwrap().resources.stock();
        let destination_before = simulation
            .state()
            .colony(destination)
            .unwrap()
            .resources
            .stock();
        let cargo = ResourceStock::new(120, 80, 20);
        let repository = simulation.universe_repository().clone();

        let launched = launch_transport_mission(
            simulation.state_mut(),
            &repository,
            actor,
            origin,
            destination,
            fleet_id,
            cargo,
        )
        .expect("one light cargo can carry the requested resources");
        let mission = simulation.state().mission(launched.mission_id).unwrap();
        assert_eq!(
            simulation
                .state()
                .colony(origin)
                .unwrap()
                .resources
                .reserved_total(),
            launched
                .fuel_cost
                .as_stock()
                .checked_add(cargo)
                .expect("test reservation total fits"),
        );
        let departure = mission.order.departure_at;
        let return_arrival = mission.plan.return_arrival_at;
        advance_missions(simulation.state_mut(), &repository, departure);
        assert_eq!(
            simulation.state().fleet(launched.fleet_id).unwrap().cargo,
            cargo
        );
        advance_missions(simulation.state_mut(), &repository, return_arrival);

        let origin_after = simulation.state().colony(origin).unwrap().resources.stock();
        let destination_after = simulation
            .state()
            .colony(destination)
            .unwrap()
            .resources
            .stock();
        assert_eq!(
            origin_after,
            origin_before
                .checked_sub(launched.fuel_cost)
                .and_then(|stock| stock.checked_sub(cargo))
                .expect("the source covers fuel and cargo"),
        );
        assert_eq!(
            destination_after,
            destination_before
                .checked_add(cargo)
                .expect("the destination accepts the cargo"),
        );
        let fleet = simulation.state().fleet(launched.fleet_id).unwrap();
        assert_eq!(fleet.location, FleetLocation::Docked(origin));
        assert!(fleet.cargo.is_zero());
        assert!(fleet.is_idle());
        assert!(matches!(
            simulation.state().mission(launched.mission_id).unwrap().result,
            Some(MissionResult::Transport(TransportMissionResult {
                requested,
                delivered,
                returned,
                retained,
                status: TransportDeliveryStatus::Delivered,
                ..
            })) if requested == cargo
                && delivered == cargo
                && returned.is_zero()
                && retained.is_zero()
        ));
        assert_eq!(
            simulation.state().mission_reports.last().unwrap().outcome,
            MissionReportOutcome::Completed,
        );
    }

    #[test]
    fn full_destination_storage_returns_every_resource_to_the_origin() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let cargo = ResourceStock::new(300, 180, 10);
        let destination_capacity =
            crate::storage_capacity(simulation.state().colony(destination).unwrap().buildings);
        simulation
            .state_mut()
            .colony_mut(destination)
            .unwrap()
            .resources = ResourceLedger::new(destination_capacity);
        let origin_before = simulation.state().colony(origin).unwrap().resources.stock();
        let destination_before = simulation
            .state()
            .colony(destination)
            .unwrap()
            .resources
            .stock();
        let repository = simulation.universe_repository().clone();

        let launched = launch_transport_mission(
            simulation.state_mut(),
            &repository,
            actor,
            origin,
            destination,
            fleet_id,
            cargo,
        )
        .expect("one light cargo can carry the requested resources");
        let return_arrival = simulation
            .state()
            .mission(launched.mission_id)
            .unwrap()
            .plan
            .return_arrival_at;
        advance_missions(simulation.state_mut(), &repository, return_arrival);

        assert_eq!(
            simulation.state().colony(origin).unwrap().resources.stock(),
            origin_before
                .checked_sub(launched.fuel_cost)
                .expect("only fuel is consumed when the cargo returns"),
        );
        assert_eq!(
            simulation
                .state()
                .colony(destination)
                .unwrap()
                .resources
                .stock(),
            destination_before,
        );
        let fleet = simulation.state().fleet(launched.fleet_id).unwrap();
        assert!(fleet.cargo.is_zero());
        assert!(fleet.is_idle());
        assert!(matches!(
            simulation.state().mission(launched.mission_id).unwrap().result,
            Some(MissionResult::Transport(TransportMissionResult {
                delivered,
                returned,
                retained,
                status: TransportDeliveryStatus::PartiallyDelivered,
                ..
            })) if delivered.is_zero() && returned == cargo && retained.is_zero()
        ));
    }

    #[test]
    fn invalid_transport_destination_returns_the_cargo_and_reports_failure() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let cargo = ResourceStock::new(90, 60, 10);
        let origin_before = simulation.state().colony(origin).unwrap().resources.stock();
        let repository = simulation.universe_repository().clone();
        let launched = launch_transport_mission(
            simulation.state_mut(),
            &repository,
            actor,
            origin,
            destination,
            fleet_id,
            cargo,
        )
        .expect("the transport initially has a valid destination");
        simulation
            .state_mut()
            .colony_mut(destination)
            .unwrap()
            .owner = Owner::Faction(FactionId::new(2));
        let return_arrival = simulation
            .state()
            .mission(launched.mission_id)
            .unwrap()
            .plan
            .return_arrival_at;

        advance_missions(simulation.state_mut(), &repository, return_arrival);

        assert_eq!(
            simulation.state().colony(origin).unwrap().resources.stock(),
            origin_before
                .checked_sub(launched.fuel_cost)
                .expect("only fuel is consumed when the cargo returns"),
        );
        assert!(
            simulation
                .state()
                .fleet(launched.fleet_id)
                .unwrap()
                .cargo
                .is_zero()
        );
        assert!(matches!(
            simulation.state().mission(launched.mission_id).unwrap().result,
            Some(MissionResult::Transport(TransportMissionResult {
                delivered,
                returned,
                retained,
                status: TransportDeliveryStatus::DestinationInvalid,
                ..
            })) if delivered.is_zero() && returned == cargo && retained.is_zero()
        ));
        assert_eq!(
            simulation.state().mission_reports.last().unwrap().outcome,
            MissionReportOutcome::Failed,
        );
    }

    #[test]
    fn harvest_requires_analysis_technology_and_an_unreserved_site_atomically() {
        let (mut simulation, site_id, fleet_id) = simulation_with_harvest_site();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        let before = simulation.state().clone();

        assert_eq!(
            launch_harvest_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                fleet_id,
                site_id,
            ),
            Err(MissionError::MissingHarvestTechnology(
                TechnologyUnlock::RemoteExtraction,
            )),
        );
        assert_eq!(simulation.state(), &before);

        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::REMOTE_EXTRACTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        let launched = launch_harvest_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            fleet_id,
            site_id,
        )
        .expect("an analyzed remote site accepts a cargo mission");
        assert_eq!(
            simulation
                .state()
                .extraction_site(site_id)
                .expect("the site exists")
                .reserved_by,
            Some(launched.mission_id),
        );

        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("one light cargo is a valid composition");
        let second_fleet = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("the origin colony still has one spare light cargo docked");
        let before_busy = simulation.state().clone();
        assert!(matches!(
            launch_harvest_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                second_fleet.fleet_id,
                site_id,
            ),
            Err(MissionError::ExtractionSiteBusy {
                site_id: found,
                mission_id,
            }) if found == site_id && mission_id == launched.mission_id
        ));
        assert_eq!(simulation.state(), &before_busy);
    }

    #[test]
    fn harvest_cancellation_releases_site_without_extracting() {
        let (mut simulation, site_id, fleet_id) = simulation_with_harvest_site();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        let remaining_before = simulation
            .state()
            .extraction_site(site_id)
            .expect("the site exists")
            .remaining;
        let launched = launch_harvest_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            fleet_id,
            site_id,
        )
        .expect("the harvest launches");

        cancel_mission(simulation.state_mut(), actor, launched.mission_id)
            .expect("a preparing harvest can be cancelled");

        let site = simulation
            .state()
            .extraction_site(site_id)
            .expect("the site remains");
        assert_eq!(site.remaining, remaining_before);
        assert_eq!(site.reserved_by, None);
        let fleet = simulation
            .state()
            .fleet(launched.fleet_id)
            .expect("the cargo fleet remains");
        assert!(fleet.is_idle());
        assert!(fleet.cargo.is_zero());
    }

    #[test]
    fn harvest_debits_the_site_once_and_delivers_the_exact_cargo() {
        let (mut simulation, site_id, fleet_id) = simulation_with_harvest_site();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        let origin_before = simulation.state().colonies[0].resources.stock();
        let site_before = *simulation
            .state()
            .extraction_site(site_id)
            .expect("the site exists");
        let planet = repository
            .planet(site_before.planet_id)
            .expect("the site planet exists");
        let rule = extraction_rules().rule_for(planet.kind);
        let launched = launch_harvest_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            fleet_id,
            site_id,
        )
        .expect("the harvest launches");
        let mission = simulation
            .state()
            .mission(launched.mission_id)
            .expect("the harvest mission exists");
        let return_departure = mission.plan.return_departure_at;
        let return_arrival = mission.plan.return_arrival_at;
        let fleet_capacity = simulation
            .state()
            .fleet(launched.fleet_id)
            .expect("the cargo fleet exists")
            .capabilities()
            .expect("the cargo fleet is valid")
            .cargo_capacity;
        let expected_amount = site_before
            .remaining
            .min(rule.maximum_harvest())
            .min(fleet_capacity);
        let expected_cargo = stock_for(site_before.resource, expected_amount);

        advance_missions(simulation.state_mut(), &repository, return_departure);

        let site_after_collection = simulation
            .state()
            .extraction_site(site_id)
            .expect("the site remains");
        assert_eq!(
            site_after_collection.remaining,
            site_before.remaining - expected_amount,
        );
        assert_eq!(site_after_collection.reserved_by, None);
        assert_eq!(
            simulation
                .state()
                .fleet(launched.fleet_id)
                .expect("the returning fleet exists")
                .cargo,
            expected_cargo,
        );
        assert!(
            simulation
                .state()
                .mission(launched.mission_id)
                .expect("the harvest mission exists")
                .result
                .is_none()
        );

        advance_missions(simulation.state_mut(), &repository, return_arrival);

        let origin_after = simulation.state().colonies[0].resources.stock();
        assert_eq!(
            origin_after,
            origin_before
                .checked_sub(launched.fuel_cost)
                .and_then(|stock| stock.checked_add(expected_cargo))
                .expect("the test harvest accounting fits"),
        );
        assert!(matches!(
            simulation
                .state()
                .mission(launched.mission_id)
                .expect("the harvest mission exists")
                .result,
            Some(MissionResult::Harvest(HarvestMissionResult {
                collected,
                delivered,
                retained,
                site_remaining,
                status: HarvestCollectionStatus::Collected,
                ..
            })) if collected == expected_cargo
                && delivered == expected_cargo
                && retained.is_zero()
                && site_remaining == site_before.remaining - expected_amount
        ));
        let state_after_completion = simulation.state().clone();
        advance_missions(
            simulation.state_mut(),
            &repository,
            StrategicTick::new(return_arrival.value() + 10_000),
        );
        assert_eq!(simulation.state(), &state_after_completion);
    }

    #[test]
    fn depleted_site_cannot_create_more_resources() {
        let (mut simulation, site_id, fleet_id) = simulation_with_harvest_site();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();
        simulation
            .state_mut()
            .extraction_site_mut(site_id)
            .expect("the site exists")
            .remaining = 75;
        let launched = launch_harvest_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            fleet_id,
            site_id,
        )
        .expect("the last harvest launches");
        let return_arrival = simulation
            .state()
            .mission(launched.mission_id)
            .expect("the harvest mission exists")
            .plan
            .return_arrival_at;
        advance_missions(simulation.state_mut(), &repository, return_arrival);
        assert_eq!(
            simulation
                .state()
                .extraction_site(site_id)
                .expect("the depleted site remains")
                .remaining,
            0,
        );
        assert!(matches!(
            simulation
                .state()
                .mission(launched.mission_id)
                .expect("the completed harvest exists")
                .result,
            Some(MissionResult::Harvest(HarvestMissionResult {
                collected,
                status: HarvestCollectionStatus::SiteDepleted,
                ..
            })) if collected == stock_for(
                simulation
                    .state()
                    .extraction_site(site_id)
                    .expect("the site remains")
                    .resource,
                75,
            )
        ));
        let before = simulation.state().clone();
        assert_eq!(
            launch_harvest_mission(
                simulation.state_mut(),
                &repository,
                actor,
                colony_id,
                fleet_id,
                site_id,
            ),
            Err(MissionError::ExtractionSiteDepleted(site_id)),
        );
        assert_eq!(simulation.state(), &before);
    }

    #[test]
    fn eligible_fleets_for_mission_lists_only_matching_idle_docked_fleets() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let repository = simulation.universe_repository().clone();
        let destination_colony = simulation.state().colony(destination).unwrap();
        let target = MissionTarget::Planet {
            system_id: destination_colony.system_id,
            planet_id: destination_colony.planet_id,
        };

        assert_eq!(
            eligible_fleets_for_mission(
                simulation.state(),
                &repository,
                actor,
                origin,
                target,
                MissionKind::Transport,
            ),
            vec![fleet_id],
        );
        assert!(
            eligible_fleets_for_mission(
                simulation.state(),
                &repository,
                actor,
                origin,
                target,
                MissionKind::Attack,
            )
            .is_empty(),
            "a light cargo fleet is not a combat fleet",
        );

        let cargo = ResourceStock::new(50, 0, 0);
        launch_transport_mission(
            simulation.state_mut(),
            &repository,
            actor,
            origin,
            destination,
            fleet_id,
            cargo,
        )
        .expect("the light cargo can carry a small transport");

        assert!(
            eligible_fleets_for_mission(
                simulation.state(),
                &repository,
                actor,
                origin,
                target,
                MissionKind::Transport,
            )
            .is_empty(),
            "a fleet away on a mission is never listed as eligible",
        );
    }

    #[test]
    fn harvest_recoverable_quantity_matches_the_amount_collected_at_resolution() {
        let (mut simulation, site_id, fleet_id) = simulation_with_harvest_site();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let repository = simulation.universe_repository().clone();

        let expected =
            harvest_recoverable_quantity(simulation.state(), &repository, site_id, fleet_id)
                .expect("the site and fleet are valid before launch");
        assert!(expected > 0);

        let launched = launch_harvest_mission(
            simulation.state_mut(),
            &repository,
            actor,
            colony_id,
            fleet_id,
            site_id,
        )
        .expect("the harvest launches");
        let return_arrival = simulation
            .state()
            .mission(launched.mission_id)
            .expect("the harvest mission exists")
            .plan
            .return_arrival_at;
        advance_missions(simulation.state_mut(), &repository, return_arrival);

        assert!(matches!(
            simulation
                .state()
                .mission(launched.mission_id)
                .expect("the harvest mission exists")
                .result,
            Some(MissionResult::Harvest(HarvestMissionResult { collected, .. }))
                if collected
                    == stock_for(
                        simulation
                            .state()
                            .extraction_site(site_id)
                            .expect("the site remains")
                            .resource,
                        expected,
                    )
        ));
    }

    #[test]
    fn transport_and_harvest_never_auto_form_and_reject_an_unknown_fleet() {
        let (mut simulation, _fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let repository = simulation.universe_repository().clone();
        let fleet_count_before = simulation.state().fleets.len();
        let unknown_fleet = FleetId::new(999_999);

        assert_eq!(
            launch_transport_mission(
                simulation.state_mut(),
                &repository,
                actor,
                origin,
                destination,
                unknown_fleet,
                ResourceStock::new(10, 0, 0),
            ),
            Err(MissionError::UnknownFleet(unknown_fleet)),
        );
        assert_eq!(simulation.state().fleets.len(), fleet_count_before);

        let (mut harvest_simulation, site_id, _harvest_fleet_id) = simulation_with_harvest_site();
        let harvest_colony_id = harvest_simulation.state().colonies[0].id;
        let harvest_repository = harvest_simulation.universe_repository().clone();
        let harvest_fleet_count_before = harvest_simulation.state().fleets.len();

        assert_eq!(
            launch_harvest_mission(
                harvest_simulation.state_mut(),
                &harvest_repository,
                actor,
                harvest_colony_id,
                unknown_fleet,
                site_id,
            ),
            Err(MissionError::UnknownFleet(unknown_fleet)),
        );
        assert_eq!(
            harvest_simulation.state().fleets.len(),
            harvest_fleet_count_before,
        );
    }

    #[test]
    fn transport_and_harvest_reject_a_fleet_already_carrying_cargo() {
        let (mut simulation, fleet_id) = simulation_with_two_colonies();
        let actor = simulation.state().player_faction;
        let origin = simulation.state().colonies[0].id;
        let destination = simulation.state().colonies[1].id;
        let repository = simulation.universe_repository().clone();
        simulation
            .state_mut()
            .fleet_mut(fleet_id)
            .expect("the fleet exists")
            .cargo = ResourceStock::new(1, 0, 0);

        assert_eq!(
            launch_transport_mission(
                simulation.state_mut(),
                &repository,
                actor,
                origin,
                destination,
                fleet_id,
                ResourceStock::new(10, 0, 0),
            ),
            Err(MissionError::TransportFleetHasCargo(fleet_id)),
        );

        let (mut harvest_simulation, site_id, harvest_fleet_id) = simulation_with_harvest_site();
        let harvest_colony_id = harvest_simulation.state().colonies[0].id;
        let harvest_repository = harvest_simulation.universe_repository().clone();
        harvest_simulation
            .state_mut()
            .fleet_mut(harvest_fleet_id)
            .expect("the fleet exists")
            .cargo = ResourceStock::new(1, 0, 0);

        assert_eq!(
            launch_harvest_mission(
                harvest_simulation.state_mut(),
                &harvest_repository,
                actor,
                harvest_colony_id,
                harvest_fleet_id,
                site_id,
            ),
            Err(MissionError::HarvestFleetHasCargo(harvest_fleet_id)),
        );
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

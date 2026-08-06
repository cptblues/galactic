use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, PlanetId, ResourceLedgerError, ResourceStock, SystemId,
};

use crate::{
    AuthorizationError, BuildingCatalogError, ColonyFoundationStateError, ConstructionQueueError,
    CraftStateError, DiplomacyError, ExtractionSiteStateError, FleetStateError, MissionStateError,
    PlanetAnalysisStateError, PlanetaryPresenceStateError, ResearchStateError,
    StartingScenarioError, UniverseIndexError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationBuildError {
    InvalidUniverse(UniverseIndexError),
    UnsupportedStateVersion {
        expected: u32,
        found: u32,
    },
    InvalidStartingScenario(StartingScenarioError),
    DuplicateFaction(FactionId),
    UnknownPlayerFaction(FactionId),
    PlayerFactionIsNotPlayer(FactionId),
    PlayerFactionIsInactive(FactionId),
    InvalidDiplomacy(DiplomacyError),
    UnknownRelationFaction(FactionId),
    DuplicateColony(ColonyId),
    DuplicateColonyPlanet(PlanetId),
    InvalidNextColonyId {
        next_colony_id: u64,
        existing_colony_id: ColonyId,
    },
    MissingActivePlayerColony,
    UnknownActiveColony(ColonyId),
    InvalidActiveColonyAccess {
        colony_id: ColonyId,
        error: AuthorizationError,
    },
    EmptyColonyName(ColonyId),
    InvalidColonyBuildings {
        colony_id: ColonyId,
        error: BuildingCatalogError,
    },
    InvalidColonyEnergy(ColonyId),
    InvalidProductionWindow {
        colony_id: ColonyId,
        pending_ticks: u16,
    },
    InvalidConstructionQueue {
        colony_id: ColonyId,
        error: ConstructionQueueError,
    },
    InvalidCraftState {
        colony_id: ColonyId,
        error: CraftStateError,
    },
    InvalidResearchState(ResearchStateError),
    InvalidPlanetAnalysisState(PlanetAnalysisStateError),
    InvalidExtractionSiteState(ExtractionSiteStateError),
    InvalidPlanetaryPresenceState(PlanetaryPresenceStateError),
    InvalidColonyFoundationState(ColonyFoundationStateError),
    InvalidColonyResourceLedger {
        colony_id: ColonyId,
        error: ResourceLedgerError,
    },
    ColonyStockExceedsCapacity {
        colony_id: ColonyId,
        stock: ResourceStock,
        capacity: ResourceStock,
    },
    UnknownColonyFaction {
        colony_id: ColonyId,
        faction_id: FactionId,
    },
    UnownedColony(ColonyId),
    DuplicateFleet(FleetId),
    InvalidFleetState {
        fleet_id: FleetId,
        error: FleetStateError,
    },
    InvalidNextFleetId {
        next_fleet_id: u64,
        existing_fleet_id: FleetId,
    },
    UnknownFleetFaction {
        fleet_id: FleetId,
        faction_id: FactionId,
    },
    UnownedFleet(FleetId),
    UnknownFleetColony {
        fleet_id: FleetId,
        colony_id: ColonyId,
    },
    UnknownFleetSystem {
        fleet_id: FleetId,
        system_id: SystemId,
    },
    DockedFleetOwnerMismatch {
        fleet_id: FleetId,
        colony_id: ColonyId,
    },
    DuplicateMission(MissionId),
    InvalidMissionState {
        mission_id: MissionId,
        error: MissionStateError,
    },
    InvalidNextMissionId {
        next_mission_id: u64,
        existing_mission_id: MissionId,
    },
    UnknownMissionFaction {
        mission_id: MissionId,
        faction_id: FactionId,
    },
    UnownedMission(MissionId),
    UnknownMissionFleet {
        mission_id: MissionId,
        fleet_id: FleetId,
    },
    MissionFleetOwnerMismatch {
        mission_id: MissionId,
        fleet_id: FleetId,
    },
    MissionFleetAssignmentMismatch {
        mission_id: MissionId,
        fleet_id: FleetId,
    },
    MissionFleetLocationMismatch {
        mission_id: MissionId,
        fleet_id: FleetId,
    },
    MissionFleetCargoMismatch {
        mission_id: MissionId,
        fleet_id: FleetId,
        expected: ResourceStock,
        found: ResourceStock,
    },
    UnknownMissionOriginColony {
        mission_id: MissionId,
        colony_id: ColonyId,
    },
    MissionOriginMismatch {
        mission_id: MissionId,
        colony_id: ColonyId,
        system_id: SystemId,
    },
    MissingMissionFuelReservation {
        mission_id: MissionId,
    },
    MissionFuelReservationMismatch {
        mission_id: MissionId,
    },
    MissingMissionFoundationReservation {
        mission_id: MissionId,
    },
    MissionFoundationReservationMismatch {
        mission_id: MissionId,
    },
    MissingMissionCargoReservation {
        mission_id: MissionId,
    },
    MissionCargoReservationMismatch {
        mission_id: MissionId,
    },
    HarvestSiteReservationMismatch {
        mission_id: MissionId,
    },
    ExtractionSiteReservedByUnknownMission {
        site_id: galactic_domain::ExtractionSiteId,
        mission_id: MissionId,
    },
    ExtractionSiteReservationMissionMismatch {
        site_id: galactic_domain::ExtractionSiteId,
        mission_id: MissionId,
    },
    FleetAssignedToUnknownMission {
        fleet_id: FleetId,
        mission_id: MissionId,
    },
    FleetAssignedToDifferentMission {
        fleet_id: FleetId,
        mission_id: MissionId,
    },
    DuplicateMissionReport(MissionId),
    MissionReportWithoutMission(MissionId),
    DuplicateCombatReport(MissionId),
    CombatReportWithoutMission(MissionId),
    MissingCombatReport(MissionId),
    CombatReportMissionMismatch(MissionId),
    CombatReportInFuture(MissionId),
    DuplicatePendingCombat(MissionId),
    PendingCombatWithoutMission(MissionId),
    PendingCombatForTerminalMission(MissionId),
    PendingCombatAlreadyCompleted(MissionId),
    PendingCombatRoundExceedsMaximum(MissionId),
    PendingCombatStackIdCollision(MissionId),
    PendingCombatHullExceedsMaximum(MissionId),
    DuplicateSystemKnowledge(SystemId),
    DuplicatePlanetKnowledge(PlanetId),
    ExplicitUnknownSystemKnowledge(SystemId),
    ExplicitUnknownPlanetKnowledge(PlanetId),
    UnknownKnowledgeSystem(SystemId),
    UnknownKnowledgePlanet(PlanetId),
    ColonySystemNotColonized {
        colony_id: ColonyId,
        system_id: SystemId,
    },
    ColonyPlanetNotColonized {
        colony_id: ColonyId,
        planet_id: PlanetId,
    },
    UnknownColonySystem {
        colony_id: ColonyId,
        system_id: SystemId,
    },
    UnknownColonyPlanet {
        colony_id: ColonyId,
        planet_id: PlanetId,
    },
    ColonyPlanetSystemMismatch {
        colony_id: ColonyId,
        system_id: SystemId,
        planet_id: PlanetId,
    },
    InvalidSelectedSystem(SystemId),
    InvalidSelectedPlanet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

impl From<UniverseIndexError> for SimulationBuildError {
    fn from(error: UniverseIndexError) -> Self {
        Self::InvalidUniverse(error)
    }
}

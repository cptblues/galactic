// MVP-021: deterministic simulation with faction commands and generic missions.
use std::collections::HashSet;
use std::time::Duration;

use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, Owner, PlanetId, ResourceLedgerError, ResourceStock,
    SystemId, UniverseConfig, UniverseDefinition,
};

use crate::{
    AttackMissionOutcome, AuthorizationError, BuildingCatalogError, ColonizationMissionOutcome,
    ColonyFoundationStateError, ColonySelectionError, ColonySelectionRejected, CombatReportStatus,
    CommandRejection, ConstructionQueueError, CraftStateError, DiplomacyError, DiplomacyState,
    FactionKind, FleetAssignment, FleetStateError, GAME_STATE_VERSION, GameAction, GameCommand,
    GameEvent, GameEventKind, GameState, KnowledgeLevel, MissionEngineEvent, MissionKind,
    MissionPhase, MissionResult, MissionStateError, PlanetAnalysisStateError,
    PlanetaryIntelPrecision, PlanetaryPresenceStateError, ResearchStateError, SelectionTarget,
    StartingScenario, StartingScenarioError, StrategicDuration, TimeSpeed, UniverseIndexError,
    UniverseRepository, advance_colony_construction, advance_colony_craft, advance_missions,
    advance_research, analyze_planet, build_planet_analysis_report, cancel_mission,
    default_building_catalog, enqueue_building_upgrade, enqueue_craft, enqueue_research,
    form_fleet, launch_attack_mission, launch_colonization_mission, launch_mission,
    launch_probe_mission, launch_transport_mission, planetary_analysis_rules,
    queue_colony_production, refresh_planetary_intelligence, storage_capacity,
    validate_colony_foundations, validate_construction_queue, validate_craft_state,
    validate_fleet_state, validate_mission_state, validate_planet_analysis_state,
    validate_planetary_presence_state, validate_research_state,
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

#[derive(Debug, Clone)]
pub struct Simulation {
    universe: UniverseRepository,
    state: GameState,
}

impl Simulation {
    pub fn new(config: UniverseConfig) -> Self {
        Self::new_with_scenario(config, StartingScenario::mvp())
            .expect("the MVP starting scenario must produce a valid simulation")
    }

    pub fn new_with_scenario(
        config: UniverseConfig,
        scenario: StartingScenario,
    ) -> Result<Self, SimulationBuildError> {
        let universe = UniverseRepository::generate(config);
        let state = GameState::from_starting_scenario(&universe, scenario)
            .map_err(SimulationBuildError::InvalidStartingScenario)?;
        validate_state(&universe, &state)?;
        Ok(Self { universe, state })
    }

    pub fn from_parts(
        universe: UniverseDefinition,
        state: GameState,
    ) -> Result<Self, SimulationBuildError> {
        let universe = UniverseRepository::new(universe)?;
        validate_state(&universe, &state)?;
        Ok(Self { universe, state })
    }

    /// Immutable generated definition. No mutable universe accessor exists.
    pub fn universe(&self) -> &UniverseDefinition {
        self.universe.definition()
    }

    pub fn universe_repository(&self) -> &UniverseRepository {
        &self.universe
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    pub fn player_command(&self, action: GameAction) -> GameCommand {
        GameCommand::new(
            self.state.player_faction,
            self.state.clock.current_tick(),
            action,
        )
    }

    pub fn apply_player_action(&mut self, action: GameAction) -> Vec<GameEvent> {
        let command = self.player_command(action);
        self.apply_command(command)
    }

    pub fn apply_command(&mut self, command: GameCommand) -> Vec<GameEvent> {
        let current_tick = self.state.clock.current_tick();
        if let Err(error) = self.validate_command(&command) {
            return vec![GameEvent::new(
                command.issuer,
                current_tick,
                GameEventKind::CommandRejected(error),
            )];
        }

        let issuer = command.issuer;
        let kinds = match command.action {
            GameAction::TogglePause => {
                let next_speed = self.state.clock.toggle_pause();
                vec![GameEventKind::SpeedChanged(next_speed)]
            }
            GameAction::SetSpeed(speed) => self.set_speed(speed),
            GameAction::SelectSystem(system_id) => self.select_system(system_id),
            GameAction::SelectPlanet {
                system_id,
                planet_id,
            } => self.select_planet(system_id, planet_id),
            GameAction::SelectColony { colony_id } => {
                match self.select_active_colony(issuer, colony_id) {
                    Ok(events) => events,
                    Err(error) => vec![GameEventKind::ActiveColonySelectionRejected(
                        ColonySelectionRejected { colony_id, error },
                    )],
                }
            }
            GameAction::ClearSelection => self.set_selection(SelectionTarget::None),
            GameAction::QueueBuildingUpgrade { colony_id, kind } => {
                match enqueue_building_upgrade(&mut self.state, issuer, colony_id, kind) {
                    Ok(queued) => vec![GameEventKind::ConstructionQueued(queued)],
                    Err(error) => vec![GameEventKind::ConstructionRejected(
                        crate::ConstructionRejected {
                            colony_id,
                            kind,
                            error,
                        },
                    )],
                }
            }
            GameAction::QueueResearch { technology } => {
                match enqueue_research(&mut self.state, issuer, technology) {
                    Ok(queued) => vec![GameEventKind::ResearchQueued(queued)],
                    Err(error) => vec![GameEventKind::ResearchRejected(crate::ResearchRejected {
                        technology,
                        error,
                    })],
                }
            }
            GameAction::QueueCraft {
                colony_id,
                craftable,
            } => match enqueue_craft(&mut self.state, issuer, colony_id, craftable) {
                Ok(queued) => vec![GameEventKind::CraftQueued(queued)],
                Err(error) => vec![GameEventKind::CraftRejected(crate::CraftRejected {
                    colony_id,
                    craftable,
                    error,
                })],
            },
            GameAction::FormFleet {
                colony_id,
                composition,
            } => match form_fleet(&mut self.state, issuer, colony_id, composition) {
                Ok(created) => vec![GameEventKind::FleetCreated(created)],
                Err(error) => vec![GameEventKind::FleetCreationRejected(
                    crate::FleetCreationRejected { colony_id, error },
                )],
            },
            GameAction::LaunchProbe { colony_id, target } => {
                match launch_probe_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    colony_id,
                    target,
                ) {
                    Ok((created, launched)) => {
                        let mut events = Vec::with_capacity(2);
                        if let Some(created) = created {
                            events.push(GameEventKind::FleetCreated(created));
                        }
                        events.push(GameEventKind::MissionLaunched(launched));
                        events
                    }
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: None,
                            error,
                        },
                    )],
                }
            }
            GameAction::LaunchAttack { colony_id, target } => {
                match launch_attack_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    colony_id,
                    target,
                ) {
                    Ok((created, launched)) => {
                        let mut events = Vec::with_capacity(2);
                        if let Some(created) = created {
                            events.push(GameEventKind::FleetCreated(created));
                        }
                        events.push(GameEventKind::MissionLaunched(launched));
                        events
                    }
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: None,
                            error,
                        },
                    )],
                }
            }
            GameAction::LaunchTransport {
                origin_colony_id,
                destination_colony_id,
                cargo,
            } => {
                match launch_transport_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    origin_colony_id,
                    destination_colony_id,
                    cargo,
                ) {
                    Ok((created, launched)) => {
                        let mut events = Vec::with_capacity(2);
                        if let Some(created) = created {
                            events.push(GameEventKind::FleetCreated(created));
                        }
                        events.push(GameEventKind::MissionLaunched(launched));
                        events
                    }
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: None,
                            error,
                        },
                    )],
                }
            }
            GameAction::LaunchColonization { colony_id, target } => {
                match launch_colonization_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    colony_id,
                    target,
                ) {
                    Ok((created, launched)) => {
                        let mut events = Vec::with_capacity(2);
                        if let Some(created) = created {
                            events.push(GameEventKind::FleetCreated(created));
                        }
                        events.push(GameEventKind::MissionLaunched(launched));
                        events
                    }
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: None,
                            error,
                        },
                    )],
                }
            }
            GameAction::AnalyzePlanet { planet_id } => {
                match analyze_planet(&mut self.state, &self.universe, issuer, planet_id) {
                    Ok(outcome) => {
                        let mut events =
                            Vec::with_capacity(outcome.knowledge_changes.len().saturating_add(1));
                        events.extend(
                            outcome
                                .knowledge_changes
                                .into_iter()
                                .map(GameEventKind::KnowledgeChanged),
                        );
                        events.push(GameEventKind::PlanetAnalyzed(outcome.report));
                        events
                    }
                    Err(error) => vec![GameEventKind::PlanetAnalysisRejected(
                        crate::PlanetAnalysisRejected { planet_id, error },
                    )],
                }
            }
            GameAction::LaunchMission(order) => {
                match launch_mission(&mut self.state, &self.universe, issuer, order) {
                    Ok(launched) => vec![GameEventKind::MissionLaunched(launched)],
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: Some(order.fleet_id),
                            error,
                        },
                    )],
                }
            }
            GameAction::CancelMission { mission_id } => {
                match cancel_mission(&mut self.state, issuer, mission_id) {
                    Ok((transition, report)) => vec![
                        GameEventKind::MissionTransitioned(transition),
                        GameEventKind::MissionReported(report),
                    ],
                    Err(error) => vec![GameEventKind::MissionCancellationRejected(
                        crate::MissionCancellationRejected { mission_id, error },
                    )],
                }
            }
            GameAction::DebugAdvanceSelectedKnowledge => self.debug_advance_selected_knowledge(),
        };
        let occurred_at = self.state.clock.current_tick();
        kinds
            .into_iter()
            .map(|kind| GameEvent::new(issuer, occurred_at, kind))
            .collect()
    }

    /// Advances simulation time from a real frame duration.
    ///
    /// The real duration is converted into fixed strategic ticks by the clock.
    /// Rendering, UI and camera systems remain outside this method.
    pub fn advance(&mut self, real_delta: Duration) -> Vec<GameEvent> {
        let advance = self.state.clock.advance(real_delta);
        if advance.ticks.is_zero() {
            return Vec::new();
        }

        let mut events = vec![GameEvent::new(
            self.state.player_faction,
            advance.current_tick,
            GameEventKind::TicksAdvanced {
                ticks: advance.ticks,
                current_tick: advance.current_tick,
            },
        )];
        let one_tick = StrategicDuration::from_ticks(1);
        for _ in 0..advance.ticks.ticks() {
            for colony in &mut self.state.colonies {
                let recipient = colony
                    .owner
                    .faction()
                    .expect("validated colonies must have a faction owner");
                if let Some(report) = queue_colony_production(colony, one_tick) {
                    events.push(GameEvent::new(
                        recipient,
                        advance.current_tick,
                        GameEventKind::ProductionRefreshed(report),
                    ));
                }
                let completed = advance_colony_construction(colony, one_tick)
                    .expect("validated construction reservations must commit");
                events.extend(completed.into_iter().map(|completed| {
                    GameEvent::new(
                        recipient,
                        advance.current_tick,
                        GameEventKind::ConstructionCompleted(completed),
                    )
                }));
                let crafted = advance_colony_craft(colony, one_tick)
                    .expect("validated craft reservations must commit");
                events.extend(crafted.into_iter().map(|completed| {
                    GameEvent::new(
                        recipient,
                        advance.current_tick,
                        GameEventKind::CraftCompleted(completed),
                    )
                }));
            }
            let research_owner = self.state.player_faction;
            events.extend(
                advance_research(&mut self.state, research_owner, one_tick)
                    .into_iter()
                    .map(|completed| {
                        GameEvent::new(
                            research_owner,
                            advance.current_tick,
                            GameEventKind::ResearchCompleted(completed),
                        )
                    }),
            );
        }
        for mission_event in advance_missions(&mut self.state, &self.universe, advance.current_tick)
        {
            match mission_event {
                MissionEngineEvent::Transition {
                    recipient,
                    transition,
                } => events.push(GameEvent::new(
                    recipient,
                    transition.transitioned_at,
                    GameEventKind::MissionTransitioned(transition),
                )),
                MissionEngineEvent::Report { recipient, report } => events.push(GameEvent::new(
                    recipient,
                    report.occurred_at,
                    GameEventKind::MissionReported(report),
                )),
                MissionEngineEvent::Knowledge {
                    recipient,
                    change,
                    occurred_at,
                } => events.push(GameEvent::new(
                    recipient,
                    occurred_at,
                    GameEventKind::KnowledgeChanged(change),
                )),
                MissionEngineEvent::Resolution {
                    recipient,
                    resolution,
                } => events.push(GameEvent::new(
                    recipient,
                    resolution.occurred_at,
                    GameEventKind::MissionResolved(resolution),
                )),
                MissionEngineEvent::Foundation {
                    recipient,
                    foundation,
                } => events.push(GameEvent::new(
                    recipient,
                    foundation.prepared_at,
                    GameEventKind::ColonyFoundationPrepared(foundation),
                )),
                MissionEngineEvent::Colony { recipient, colony } => events.push(GameEvent::new(
                    recipient,
                    colony.established_at,
                    GameEventKind::ColonyEstablished(colony),
                )),
            }
        }
        events
    }

    fn validate_command(&self, command: &GameCommand) -> Result<(), CommandRejection> {
        let Some(issuer) = self.state.faction(command.issuer) else {
            return Err(CommandRejection::UnknownIssuer(command.issuer));
        };
        if !issuer.active {
            return Err(CommandRejection::InactiveIssuer(command.issuer));
        }
        let expected = self.state.clock.current_tick();
        if command.issued_at != expected {
            return Err(CommandRejection::TickMismatch {
                expected,
                found: command.issued_at,
            });
        }
        Ok(())
    }

    fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEventKind> {
        if !self.state.clock.set_speed(speed) {
            return Vec::new();
        }

        vec![GameEventKind::SpeedChanged(speed)]
    }

    fn select_system(&mut self, system_id: SystemId) -> Vec<GameEventKind> {
        if self.universe.system(system_id).is_none() {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::System(system_id))
    }

    fn select_planet(&mut self, system_id: SystemId, planet_id: PlanetId) -> Vec<GameEventKind> {
        let Some((planet_system_id, _)) = self.universe.planet_location(planet_id) else {
            return Vec::new();
        };
        if planet_system_id != system_id {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::Planet {
            system_id,
            planet_id,
        })
    }

    fn debug_advance_selected_knowledge(&mut self) -> Vec<GameEventKind> {
        let changes = match self.state.selected {
            SelectionTarget::None => Vec::new(),
            SelectionTarget::System(system_id) => {
                let current = self.state.system_knowledge_level(system_id);
                let Some(next) = current.next_exploration_level() else {
                    return Vec::new();
                };
                self.state
                    .advance_system_knowledge(&self.universe, system_id, next)
            }
            SelectionTarget::Planet { planet_id, .. } => {
                let current = self.state.planet_knowledge_level(planet_id);
                let Some(next) = current.next_exploration_level() else {
                    return Vec::new();
                };
                if next == KnowledgeLevel::Analyzed {
                    let Some((system_id, planet)) = self.universe.planet_location(planet_id) else {
                        return Vec::new();
                    };
                    let report = build_planet_analysis_report(
                        planet,
                        system_id,
                        self.state.clock.current_tick(),
                        planetary_analysis_rules(),
                    );
                    let changes =
                        self.state
                            .advance_planet_knowledge(&self.universe, planet_id, next);
                    self.state.planet_analysis_reports.push(report);
                    self.state
                        .planet_analysis_reports
                        .sort_by_key(|entry| entry.planet_id);
                    let observed_at = self.state.clock.current_tick();
                    refresh_planetary_intelligence(
                        &mut self.state,
                        planet_id,
                        PlanetaryIntelPrecision::Surveyed,
                        observed_at,
                    )
                    .expect("an analyzed planet has a deterministic presence");
                    changes
                } else {
                    let changes =
                        self.state
                            .advance_planet_knowledge(&self.universe, planet_id, next);
                    if next == KnowledgeLevel::Probed {
                        let observed_at = self.state.clock.current_tick();
                        refresh_planetary_intelligence(
                            &mut self.state,
                            planet_id,
                            PlanetaryIntelPrecision::Contact,
                            observed_at,
                        )
                        .expect("a probed planet has a deterministic presence");
                    }
                    changes
                }
            }
        };

        changes
            .into_iter()
            .map(GameEventKind::KnowledgeChanged)
            .collect()
    }

    fn set_selection(&mut self, selection: SelectionTarget) -> Vec<GameEventKind> {
        if self.state.selected == selection {
            return Vec::new();
        }

        self.state.selected = selection;
        vec![GameEventKind::SelectionChanged(selection)]
    }

    fn select_active_colony(
        &mut self,
        actor: FactionId,
        colony_id: ColonyId,
    ) -> Result<Vec<GameEventKind>, ColonySelectionError> {
        if actor != self.state.player_faction {
            return Err(ColonySelectionError::NotPlayerFaction(actor));
        }
        let colony = self
            .state
            .colony(colony_id)
            .ok_or(ColonySelectionError::UnknownColony(colony_id))?;
        self.state
            .authorize_management(actor, colony.owner)
            .map_err(ColonySelectionError::Access)?;
        let selection = SelectionTarget::Planet {
            system_id: colony.system_id,
            planet_id: colony.planet_id,
        };

        let mut events = Vec::with_capacity(2);
        if self.state.active_colony_id != Some(colony_id) {
            self.state.active_colony_id = Some(colony_id);
            events.push(GameEventKind::ActiveColonyChanged(colony_id));
        }
        events.extend(self.set_selection(selection));
        Ok(events)
    }
}

fn validate_state(
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
        if let Err(error) = validate_mission_state(mission, universe) {
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

#[cfg(test)]
mod tests {
    use galactic_domain::{
        ColonyId, PlanetId, ResourceLedger, ResourceStock, SystemId, UniverseConfig,
    };

    use crate::{BuildingKind, KnowledgeTarget, TechnologyId};

    use super::*;

    fn advance_in_equal_frames(
        simulation: &mut Simulation,
        frame_count: u32,
        frame_duration: Duration,
    ) {
        for _ in 0..frame_count {
            simulation.advance(frame_duration);
        }
    }

    #[test]
    fn simulation_advances_without_renderer() {
        let mut simulation = Simulation::new(UniverseConfig::default());

        let events = simulation.advance(Duration::from_millis(250));

        assert_eq!(simulation.state().clock.current_tick().value(), 2);
        assert_eq!(
            events,
            vec![GameEvent::new(
                simulation.state().player_faction,
                crate::StrategicTick::new(2),
                GameEventKind::TicksAdvanced {
                    ticks: crate::StrategicDuration::from_ticks(2),
                    current_tick: crate::StrategicTick::new(2),
                },
            )]
        );
        assert_eq!(simulation.state().clock.remainder_nanos(), 50_000_000);
    }

    #[test]
    fn different_frame_rates_produce_the_same_ticks() {
        let mut fast_frames = Simulation::new(UniverseConfig::mvp());
        let mut slow_frames = Simulation::new(UniverseConfig::mvp());

        advance_in_equal_frames(&mut fast_frames, 100, Duration::from_millis(10));
        advance_in_equal_frames(&mut slow_frames, 10, Duration::from_millis(100));

        assert_eq!(fast_frames.state(), slow_frames.state());
    }

    #[test]
    fn production_is_independent_from_frame_rate() {
        let mut fast_frames = Simulation::new(UniverseConfig::mvp());
        let mut slow_frames = Simulation::new(UniverseConfig::mvp());

        advance_in_equal_frames(&mut fast_frames, 1_000, Duration::from_millis(10));
        advance_in_equal_frames(&mut slow_frames, 100, Duration::from_millis(100));

        assert_eq!(fast_frames.state(), slow_frames.state());
        assert_eq!(
            fast_frames
                .state()
                .player_home_colony()
                .expect("home colony exists")
                .resources
                .stock(),
            ResourceStock::new(625, 312, 227)
        );
    }

    #[test]
    fn pause_and_speed_apply_expected_production_ticks() {
        let mut paused = Simulation::new(UniverseConfig::mvp());
        paused.apply_player_action(GameAction::TogglePause);
        let initial = paused.state().clone();
        paused.advance(Duration::from_secs(10));
        assert_eq!(paused.state(), &initial);

        let mut x1 = Simulation::new(UniverseConfig::mvp());
        let mut x4 = Simulation::new(UniverseConfig::mvp());
        x4.apply_player_action(GameAction::SetSpeed(TimeSpeed::X4));

        x1.advance(Duration::from_secs(1));
        x4.advance(Duration::from_millis(250));
        x4.apply_player_action(GameAction::SetSpeed(TimeSpeed::X1));

        assert_eq!(x1.state(), x4.state());
    }

    #[test]
    fn reconstruction_rejects_stock_above_capacity() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let universe = simulation.universe().clone();
        let mut state = simulation.state().clone();
        let colony = state.colonies.first_mut().expect("home colony exists");
        let capacity = crate::storage_capacity(colony.buildings);
        colony.resources = ResourceLedger::new(ResourceStock::new(
            capacity.metal + 1,
            capacity.crystal,
            capacity.fuel,
        ));

        assert!(matches!(
            Simulation::from_parts(universe, state),
            Err(SimulationBuildError::ColonyStockExceedsCapacity { .. })
        ));
    }

    #[test]
    fn production_events_are_emitted_only_every_five_seconds() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());

        let early = simulation.advance(Duration::from_secs(4));
        assert!(
            early
                .iter()
                .all(|event| !matches!(event.kind, GameEventKind::ProductionRefreshed(_)))
        );

        let refresh = simulation.advance(Duration::from_secs(1));
        assert!(refresh.iter().any(|event| {
            matches!(
                event.kind,
                GameEventKind::ProductionRefreshed(report)
                    if report.ticks.ticks()
                        == crate::production_refresh_ticks()
            )
        }));
    }

    #[test]
    fn dormant_factions_do_not_change_the_player_loop() {
        let mut configured = Simulation::new(UniverseConfig::mvp());
        let mut player_only = configured.clone();
        player_only
            .state_mut()
            .factions
            .retain(|faction| faction.active);

        configured.advance(Duration::from_secs(30));
        player_only.advance(Duration::from_secs(30));

        assert_eq!(configured.state().colonies, player_only.state().colonies);
        assert_eq!(configured.state().research, player_only.state().research);
        assert_eq!(configured.state().clock, player_only.state().clock);
    }

    #[test]
    fn five_second_windows_remain_frame_rate_independent() {
        let mut fast = Simulation::new(UniverseConfig::mvp());
        let mut slow = Simulation::new(UniverseConfig::mvp());

        advance_in_equal_frames(&mut fast, 1_000, Duration::from_millis(10));
        advance_in_equal_frames(&mut slow, 10, Duration::from_secs(1));

        assert_eq!(fast.state(), slow.state());
        assert_eq!(
            fast.state()
                .player_home_colony()
                .expect("home colony exists")
                .resources
                .stock(),
            ResourceStock::new(625, 312, 227)
        );
    }

    #[test]
    fn research_and_laboratory_upgrade_are_frame_independent() {
        fn configured_simulation() -> Simulation {
            let mut simulation = Simulation::new(UniverseConfig::mvp());
            let colony_id = simulation
                .state()
                .player_home_colony()
                .expect("home colony exists")
                .id;
            {
                let colony = simulation
                    .state_mut()
                    .colony_mut(colony_id)
                    .expect("home colony exists");
                colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
                colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
                colony
                    .resources
                    .credit(ResourceStock::new(1_000, 1_000, 500))
                    .expect("test funding fits capacity");
            }

            for technology in crate::technology_catalog().ids() {
                let events =
                    simulation.apply_player_action(GameAction::QueueResearch { technology });
                assert!(matches!(
                    events.as_slice(),
                    [GameEvent {
                        kind: GameEventKind::ResearchQueued(_),
                        ..
                    }]
                ));
            }
            let events = simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
                colony_id,
                kind: BuildingKind::RESEARCH_LAB,
            });
            assert!(matches!(
                events.as_slice(),
                [GameEvent {
                    kind: GameEventKind::ConstructionQueued(_),
                    ..
                }]
            ));
            simulation
        }

        let mut single_batch = configured_simulation();
        let mut many_batches = configured_simulation();

        single_batch.advance(Duration::from_secs(200));
        advance_in_equal_frames(&mut many_batches, 200, Duration::from_secs(1));

        assert_eq!(single_batch.state(), many_batches.state(),);
    }

    #[test]
    fn research_events_are_emitted_on_completion() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);

        simulation.apply_player_action(GameAction::QueueResearch {
            technology: TechnologyId::SPATIAL_DETECTION,
        });
        let events = simulation.advance(Duration::from_secs(60));

        assert!(events.iter().any(|event| {
            matches!(
                event.kind,
                GameEventKind::ResearchCompleted(completed)
                    if completed.technology
                        == TechnologyId::SPATIAL_DETECTION
            )
        }));
    }

    #[test]
    fn selection_events_use_domain_ids() {
        let mut simulation = Simulation::new(UniverseConfig::default());
        let system_id = SystemId::from_index(0);
        let planet_id = PlanetId::from_system_index(system_id, 0);

        simulation.apply_player_action(GameAction::ClearSelection);
        let events = simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });

        assert_eq!(
            events,
            vec![GameEvent::new(
                simulation.state().player_faction,
                simulation.state().clock.current_tick(),
                GameEventKind::SelectionChanged(SelectionTarget::Planet {
                    system_id,
                    planet_id,
                }),
            )]
        );
    }

    #[test]
    fn active_colony_selection_is_explicit_and_updates_strategic_selection() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let mut second = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .clone();
        second.id = ColonyId::new(1);
        second.name = "Relais Boréal".to_string();
        let target = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|knowledge| knowledge.planet_id != second.planet_id)
            .expect("the universe contains another planet")
            .planet_id;
        second.system_id = target.system_id();
        second.planet_id = target;
        simulation.state_mut().colonies.push(second);
        simulation.state_mut().next_colony_id = 2;

        let colonies_before = simulation.state().colonies.clone();
        let research_before = simulation.state().research.clone();
        let clock_before = simulation.state().clock;
        let events = simulation.apply_player_action(GameAction::SelectColony {
            colony_id: ColonyId::new(1),
        });

        assert_eq!(simulation.state().active_colony_id, Some(ColonyId::new(1)));
        assert_eq!(
            simulation.state().selected,
            SelectionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        );
        assert_eq!(simulation.state().colonies, colonies_before);
        assert_eq!(simulation.state().research, research_before);
        assert_eq!(simulation.state().clock, clock_before);
        assert_eq!(
            events,
            vec![
                GameEvent::new(
                    simulation.state().player_faction,
                    simulation.state().clock.current_tick(),
                    GameEventKind::ActiveColonyChanged(ColonyId::new(1)),
                ),
                GameEvent::new(
                    simulation.state().player_faction,
                    simulation.state().clock.current_tick(),
                    GameEventKind::SelectionChanged(SelectionTarget::Planet {
                        system_id: target.system_id(),
                        planet_id: target,
                    }),
                ),
            ],
        );
    }

    #[test]
    fn active_colony_selection_rejects_a_foreign_colony_atomically() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let player = simulation.state().player_faction;
        let initial_active = simulation.state().active_colony_id;
        let initial_selection = simulation.state().selected;
        let mut foreign = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .clone();
        foreign.id = ColonyId::new(99);
        foreign.owner = Owner::Faction(FactionId::new(2));
        simulation.state_mut().colonies.push(foreign);

        let events = simulation.apply_player_action(GameAction::SelectColony {
            colony_id: ColonyId::new(99),
        });

        assert_eq!(simulation.state().active_colony_id, initial_active);
        assert_eq!(simulation.state().selected, initial_selection);
        assert!(matches!(
            events.as_slice(),
            [GameEvent {
                kind: GameEventKind::ActiveColonySelectionRejected(
                    ColonySelectionRejected {
                        colony_id,
                        error: ColonySelectionError::Access(
                            AuthorizationError::NotOwner { actor, owner },
                        ),
                    },
                ),
                ..
            }] if *colony_id == ColonyId::new(99)
                && *actor == player
                && *owner == FactionId::new(2)
        ));
    }

    #[test]
    fn colony_scoped_construction_does_not_modify_another_colony() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let mut second = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .clone();
        second.id = ColonyId::new(1);
        second.name = "Atelier Austral".to_string();
        simulation.state_mut().colonies.push(second);
        let home_before = simulation
            .state()
            .colony(ColonyId::new(0))
            .expect("home colony exists")
            .clone();

        let events = simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
            colony_id: ColonyId::new(1),
            kind: BuildingKind::METAL_MINE,
        });

        assert!(matches!(
            events.as_slice(),
            [GameEvent {
                kind: GameEventKind::ConstructionQueued(queued),
                ..
            }] if queued.colony_id == ColonyId::new(1)
        ));
        assert_eq!(
            simulation
                .state()
                .colony(ColonyId::new(0))
                .expect("home colony exists"),
            &home_before,
        );
        assert_eq!(
            simulation
                .state()
                .colony(ColonyId::new(1))
                .expect("second colony exists")
                .construction_queue
                .len(),
            1,
        );
    }

    #[test]
    fn invalid_selection_is_ignored() {
        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));
        let initial_selection = simulation.state().selected;

        let events = simulation.apply_player_action(GameAction::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(simulation.state().selected, initial_selection);
    }

    #[test]
    fn commands_retain_issuer_and_reject_inactive_sources() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let dormant = FactionId::new(2);
        let tick = simulation.state().clock.current_tick();

        let events = simulation.apply_command(GameCommand::new(
            dormant,
            tick,
            GameAction::SetSpeed(TimeSpeed::X4),
        ));

        assert_eq!(events[0].recipient, dormant);
        assert_eq!(events[0].occurred_at, tick);
        assert_eq!(
            events[0].kind,
            GameEventKind::CommandRejected(CommandRejection::InactiveIssuer(dormant)),
        );
        assert_eq!(simulation.state().clock.speed(), TimeSpeed::X1);
    }

    #[test]
    fn stale_commands_are_rejected_deterministically() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let command = simulation.player_command(GameAction::SetSpeed(TimeSpeed::X4));
        simulation.advance(Duration::from_secs(1));

        let events = simulation.apply_command(command);

        assert!(matches!(
            events.as_slice(),
            [GameEvent {
                kind: GameEventKind::CommandRejected(CommandRejection::TickMismatch {
                    expected,
                    found,
                }),
                ..
            }] if expected.value() == 10 && *found == crate::StrategicTick::ZERO
        ));
        assert_eq!(simulation.state().clock.speed(), TimeSpeed::X1);
    }

    #[test]
    fn debug_probe_progresses_selected_system_and_frontier() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .universe_repository()
            .neighboring_systems(SystemId::from_index(0))
            .into_iter()
            .next()
            .expect("home has a neighbor");

        simulation.apply_player_action(GameAction::SelectSystem(target));
        let events = simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);

        assert_eq!(
            simulation.state().system_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert!(events.iter().any(|event| {
            matches!(
                event.kind,
                GameEventKind::KnowledgeChanged(change)
                    if change.target
                        == KnowledgeTarget::System(target)
            )
        }));
    }

    #[test]
    fn knowledge_command_stops_before_colonization() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .universe_repository()
            .neighboring_systems(SystemId::from_index(0))
            .into_iter()
            .next()
            .expect("home has a neighbor");

        simulation.apply_player_action(GameAction::SelectSystem(target));
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        let events = simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);

        assert!(events.is_empty());
        assert_eq!(
            simulation.state().system_knowledge_level(target),
            KnowledgeLevel::Analyzed
        );
    }

    #[test]
    fn mutable_actions_do_not_change_generated_universe() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let initial_universe = simulation.universe().clone();

        simulation.advance(Duration::from_secs(42));
        simulation.apply_player_action(GameAction::SetSpeed(TimeSpeed::X4));
        simulation
            .state_mut()
            .colony_mut(ColonyId::new(0))
            .expect("home colony exists")
            .resources = ResourceLedger::new(ResourceStock::new(999, 888, 777));

        assert_eq!(simulation.universe(), &initial_universe);
    }

    #[test]
    fn reconstruction_rejects_missing_colony_knowledge() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let universe = simulation.universe().clone();
        let mut state = simulation.state().clone();
        state.planet_knowledge.clear();

        assert!(matches!(
            Simulation::from_parts(universe, state),
            Err(SimulationBuildError::ColonyPlanetNotColonized { .. })
        ));
    }
}

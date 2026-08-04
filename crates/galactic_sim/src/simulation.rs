// MVP-021: deterministic simulation with faction commands and generic missions.
use std::time::Duration;

use galactic_domain::{UniverseConfig, UniverseDefinition};

use crate::{
    ColonySelectionRejected, CommandRejection, GameAction, GameCommand, GameEvent, GameEventKind,
    GameState, MissionEngineEvent, SelectionTarget, StartingScenario, StrategicDuration,
    UniverseRepository, advance_colony_construction, advance_colony_craft, advance_missions,
    advance_research, cancel_construction, cancel_craft, cancel_mission, cancel_research,
    disband_fleet, enqueue_building_upgrade, enqueue_craft, enqueue_research, form_fleet,
    launch_attack_mission, launch_colonization_mission, launch_harvest_mission, launch_mission,
    launch_probe_mission, launch_transport_mission, queue_colony_production, rename_fleet,
};

mod build_error;
mod reconstruction;
mod selection;

pub use build_error::SimulationBuildError;
use reconstruction::validate_state;

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
                quantity,
            } => match enqueue_craft(&mut self.state, issuer, colony_id, craftable, quantity) {
                Ok(queued) => vec![GameEventKind::CraftQueued(queued)],
                Err(error) => vec![GameEventKind::CraftRejected(crate::CraftRejected {
                    colony_id,
                    craftable,
                    error,
                })],
            },
            GameAction::CancelCraft { colony_id } => {
                match cancel_craft(&mut self.state, issuer, colony_id) {
                    Ok(cancelled) => vec![GameEventKind::CraftCancelled(cancelled)],
                    Err(error) => vec![GameEventKind::CraftCancellationRejected(
                        crate::CraftCancellationRejected { colony_id, error },
                    )],
                }
            }
            GameAction::CancelConstruction { colony_id } => {
                match cancel_construction(&mut self.state, issuer, colony_id) {
                    Ok(cancelled) => vec![GameEventKind::ConstructionCancelled(cancelled)],
                    Err(error) => vec![GameEventKind::ConstructionCancellationRejected(
                        crate::ConstructionCancellationRejected { colony_id, error },
                    )],
                }
            }
            GameAction::CancelResearch => match cancel_research(&mut self.state, issuer) {
                Ok(cancelled) => vec![GameEventKind::ResearchCancelled(cancelled)],
                Err(error) => vec![GameEventKind::ResearchCancellationRejected(
                    crate::ResearchCancellationRejected { error },
                )],
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
            GameAction::RenameFleet { fleet_id, name } => {
                match rename_fleet(&mut self.state, issuer, fleet_id, name) {
                    Ok(renamed) => vec![GameEventKind::FleetRenamed(renamed)],
                    Err(error) => vec![GameEventKind::FleetRenameRejected(
                        crate::FleetRenameRejected { fleet_id, error },
                    )],
                }
            }
            GameAction::DisbandFleet { fleet_id } => {
                match disband_fleet(&mut self.state, issuer, fleet_id) {
                    Ok(disbanded) => vec![GameEventKind::FleetDisbanded(disbanded)],
                    Err(error) => vec![GameEventKind::FleetDisbandRejected(
                        crate::FleetDisbandRejected { fleet_id, error },
                    )],
                }
            }
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
                fleet_id,
                cargo,
            } => {
                match launch_transport_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    origin_colony_id,
                    destination_colony_id,
                    fleet_id,
                    cargo,
                ) {
                    Ok(launched) => vec![GameEventKind::MissionLaunched(launched)],
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: Some(fleet_id),
                            error,
                        },
                    )],
                }
            }
            GameAction::LaunchHarvest {
                colony_id,
                fleet_id,
                site_id,
            } => {
                match launch_harvest_mission(
                    &mut self.state,
                    &self.universe,
                    issuer,
                    colony_id,
                    fleet_id,
                    site_id,
                ) {
                    Ok(launched) => vec![GameEventKind::MissionLaunched(launched)],
                    Err(error) => vec![GameEventKind::MissionLaunchRejected(
                        crate::MissionLaunchRejected {
                            fleet_id: Some(fleet_id),
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
}

#[cfg(test)]
mod tests {
    use galactic_domain::{
        ColonyId, FactionId, Owner, PlanetId, ResourceLedger, ResourceStock, SystemId,
        UniverseConfig,
    };

    use crate::{
        AuthorizationError, BuildingKind, ColonySelectionError, KnowledgeLevel, KnowledgeTarget,
        TechnologyId, TimeSpeed, default_building_catalog,
    };

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
                colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 4);
                colony
                    .buildings
                    .set_level(BuildingKind::CONSTRUCTION_CENTER, 3);
                colony.buildings.set_level(BuildingKind::POWER_PLANT, 4);
                colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
                colony
                    .resources
                    .credit(ResourceStock::new(3_000, 2_500, 1_000))
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

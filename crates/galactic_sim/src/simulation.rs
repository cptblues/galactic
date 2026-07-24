// MVP-009: simulation commands and validation for progressive knowledge
use std::collections::HashSet;
use std::time::Duration;

use galactic_domain::{
    ColonyId, FactionId, PlanetId, SystemId, UniverseConfig, UniverseDefinition,
};

use crate::{
    FactionKind, GAME_STATE_VERSION, GameCommand, GameEvent, GameState, KnowledgeLevel,
    SelectionTarget, StartingScenario, StartingScenarioError, TimeSpeed, UniverseIndexError,
    UniverseRepository,
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
    DuplicateColony(ColonyId),
    UnknownColonyFaction {
        colony_id: ColonyId,
        faction_id: FactionId,
    },
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

    pub fn apply_command(&mut self, command: GameCommand) -> Vec<GameEvent> {
        match command {
            GameCommand::TogglePause => {
                let next_speed = self.state.clock.toggle_pause();
                vec![GameEvent::SpeedChanged(next_speed)]
            }
            GameCommand::SetSpeed(speed) => self.set_speed(speed),
            GameCommand::SelectSystem(system_id) => self.select_system(system_id),
            GameCommand::SelectPlanet {
                system_id,
                planet_id,
            } => self.select_planet(system_id, planet_id),
            GameCommand::ClearSelection => self.set_selection(SelectionTarget::None),
            GameCommand::DebugAdvanceSelectedKnowledge => self.debug_advance_selected_knowledge(),
        }
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

        // Future production, construction, research and mission systems will be
        // processed once per strategic tick here.
        vec![GameEvent::TicksAdvanced {
            ticks: advance.ticks,
            current_tick: advance.current_tick,
        }]
    }

    fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEvent> {
        if !self.state.clock.set_speed(speed) {
            return Vec::new();
        }

        vec![GameEvent::SpeedChanged(speed)]
    }

    fn select_system(&mut self, system_id: SystemId) -> Vec<GameEvent> {
        if self.universe.system(system_id).is_none() {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::System(system_id))
    }

    fn select_planet(&mut self, system_id: SystemId, planet_id: PlanetId) -> Vec<GameEvent> {
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

    fn debug_advance_selected_knowledge(&mut self) -> Vec<GameEvent> {
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
                self.state
                    .advance_planet_knowledge(&self.universe, planet_id, next)
            }
        };

        changes
            .into_iter()
            .map(GameEvent::KnowledgeChanged)
            .collect()
    }

    fn set_selection(&mut self, selection: SelectionTarget) -> Vec<GameEvent> {
        if self.state.selected == selection {
            return Vec::new();
        }

        self.state.selected = selection;
        vec![GameEvent::SelectionChanged(selection)]
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

    let mut colony_ids = HashSet::with_capacity(state.colonies.len());
    for colony in &state.colonies {
        if !colony_ids.insert(colony.id) {
            return Err(SimulationBuildError::DuplicateColony(colony.id));
        }
        if state.faction(colony.faction).is_none() {
            return Err(SimulationBuildError::UnknownColonyFaction {
                colony_id: colony.id,
                faction_id: colony.faction,
            });
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

    use crate::KnowledgeTarget;

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
            vec![GameEvent::TicksAdvanced {
                ticks: crate::StrategicDuration::from_ticks(2),
                current_tick: crate::StrategicTick::new(2),
            }]
        );
        assert_eq!(simulation.state().clock.remainder_nanos(), 50_000_000);
    }

    #[test]
    fn different_frame_rates_produce_the_same_ticks() {
        let mut fast_frames = Simulation::new(UniverseConfig::mvp());
        let mut slow_frames = Simulation::new(UniverseConfig::mvp());

        advance_in_equal_frames(&mut fast_frames, 100, Duration::from_millis(10));
        advance_in_equal_frames(&mut slow_frames, 10, Duration::from_millis(100));

        assert_eq!(fast_frames.state().clock, slow_frames.state().clock);
    }

    #[test]
    fn selection_events_use_domain_ids() {
        let mut simulation = Simulation::new(UniverseConfig::default());
        let system_id = SystemId::from_index(0);
        let planet_id = PlanetId::from_system_index(system_id, 0);

        simulation.apply_command(GameCommand::ClearSelection);
        let events = simulation.apply_command(GameCommand::SelectPlanet {
            system_id,
            planet_id,
        });

        assert_eq!(
            events,
            vec![GameEvent::SelectionChanged(SelectionTarget::Planet {
                system_id,
                planet_id,
            })]
        );
    }

    #[test]
    fn invalid_selection_is_ignored() {
        let mut simulation = Simulation::new(UniverseConfig::new(42, 16));
        let initial_selection = simulation.state().selected;

        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(simulation.state().selected, initial_selection);
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

        simulation.apply_command(GameCommand::SelectSystem(target));
        let events = simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);

        assert_eq!(
            simulation.state().system_knowledge_level(target),
            KnowledgeLevel::Probed
        );
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::KnowledgeChanged(change)
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

        simulation.apply_command(GameCommand::SelectSystem(target));
        simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);
        simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);
        let events = simulation.apply_command(GameCommand::DebugAdvanceSelectedKnowledge);

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
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));
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

// MVP-005: fixed strategic clock independent from rendering FPS
use std::collections::HashSet;
use std::time::Duration;

use galactic_domain::{ColonyId, PlanetId, SystemId, UniverseConfig, UniverseDefinition};

use crate::{
    GAME_STATE_VERSION, GameCommand, GameEvent, GameState, SelectionTarget, TimeSpeed,
    UniverseIndexError, UniverseRepository,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationBuildError {
    InvalidUniverse(UniverseIndexError),
    UnsupportedStateVersion {
        expected: u32,
        found: u32,
    },
    DuplicateColony(ColonyId),
    UnknownKnownSystem(SystemId),
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
        let universe = UniverseRepository::generate(config);
        let state = GameState::new(&universe);
        Self { universe, state }
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

    for system_id in &state.known_systems {
        if universe.system(*system_id).is_none() {
            return Err(SimulationBuildError::UnknownKnownSystem(*system_id));
        }
    }

    let mut colony_ids = HashSet::with_capacity(state.colonies.len());
    for colony in &state.colonies {
        if !colony_ids.insert(colony.id) {
            return Err(SimulationBuildError::DuplicateColony(colony.id));
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
    use galactic_domain::{PlanetId, ResourceStock, SystemId, UniverseConfig};

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
        assert_eq!(fast_frames.state().clock.current_tick().value(), 10);
    }

    #[test]
    fn pause_and_resume_do_not_duplicate_or_skip_ticks() {
        let mut simulation = Simulation::new(UniverseConfig::default());

        simulation.advance(Duration::from_millis(50));
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::Paused));
        assert!(simulation.advance(Duration::from_secs(10)).is_empty());
        assert_eq!(simulation.state().clock.current_tick().value(), 0);
        assert_eq!(simulation.state().clock.remainder_nanos(), 50_000_000);

        simulation.apply_command(GameCommand::TogglePause);
        simulation.advance(Duration::from_millis(50));

        assert_eq!(simulation.state().clock.current_tick().value(), 1);
        assert_eq!(simulation.state().clock.remainder_nanos(), 0);
    }

    #[test]
    fn speed_changes_simulation_tick_rate() {
        let mut normal = Simulation::new(UniverseConfig::mvp());
        let mut accelerated = Simulation::new(UniverseConfig::mvp());
        accelerated.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));

        normal.advance(Duration::from_millis(500));
        accelerated.advance(Duration::from_millis(500));

        assert_eq!(normal.state().clock.current_tick().value(), 5);
        assert_eq!(accelerated.state().clock.current_tick().value(), 20);
    }

    #[test]
    fn selection_events_use_domain_ids() {
        let mut simulation = Simulation::new(UniverseConfig::default());
        let system_id = SystemId::from_index(0);
        let planet_id = PlanetId::from_system_index(system_id, 0);

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

        let events = simulation.apply_command(GameCommand::SelectSystem(SystemId::new(999)));

        assert!(events.is_empty());
        assert_eq!(
            simulation.state().selected,
            SelectionTarget::System(SystemId::from_index(0))
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
            .stock = ResourceStock::new(999, 888, 777, 666);

        assert_eq!(simulation.universe(), &initial_universe);
        assert_ne!(
            simulation
                .state()
                .colony(ColonyId::new(0))
                .expect("home colony exists")
                .stock,
            ResourceStock::new(120, 45, 80, 30)
        );
    }

    #[test]
    fn visual_world_inputs_are_available_as_definition_plus_state() {
        let simulation = Simulation::new(UniverseConfig::mvp());

        assert!(!simulation.universe().systems.is_empty());
        assert!(!simulation.state().known_systems.is_empty());
        assert!(simulation.state().colony(ColonyId::new(0)).is_some());
    }
}

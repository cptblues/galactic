use galactic_domain::{PlanetId, SystemId, UniverseConfig};

use crate::{GameCommand, GameEvent, GameState, SelectionTarget, TimeSpeed};

#[derive(Debug, Clone)]
pub struct Simulation {
    state: GameState,
}

impl Simulation {
    pub fn new(config: UniverseConfig) -> Self {
        Self {
            state: GameState::new(config),
        }
    }

    pub fn from_state(state: GameState) -> Self {
        Self { state }
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
                let next_speed = if self.state.speed == TimeSpeed::Paused {
                    TimeSpeed::X1
                } else {
                    TimeSpeed::Paused
                };
                self.set_speed(next_speed)
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

    pub fn tick(&mut self, delta_seconds: f32) -> Vec<GameEvent> {
        let scaled_delta = delta_seconds.max(0.0) * self.state.speed.multiplier();
        if scaled_delta == 0.0 {
            return Vec::new();
        }

        self.state.elapsed_seconds += scaled_delta;
        vec![GameEvent::TickAdvanced {
            delta_seconds: scaled_delta,
            elapsed_seconds: self.state.elapsed_seconds,
        }]
    }

    fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEvent> {
        if self.state.speed == speed {
            return Vec::new();
        }

        self.state.speed = speed;
        vec![GameEvent::SpeedChanged(speed)]
    }

    fn select_system(&mut self, system_id: SystemId) -> Vec<GameEvent> {
        if self.state.universe.system(system_id).is_none() {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::System(system_id))
    }

    fn select_planet(&mut self, system_id: SystemId, planet_id: PlanetId) -> Vec<GameEvent> {
        let Some(system) = self.state.universe.system(system_id) else {
            return Vec::new();
        };
        if system.planet(planet_id).is_none() {
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

#[cfg(test)]
mod tests {
    use galactic_domain::{PlanetId, SystemId, UniverseConfig};

    use super::*;

    #[test]
    fn simulation_advances_without_renderer() {
        let mut simulation = Simulation::new(UniverseConfig::default());

        let events = simulation.tick(2.5);

        assert_eq!(simulation.state().elapsed_seconds, 2.5);
        assert_eq!(
            events,
            vec![GameEvent::TickAdvanced {
                delta_seconds: 2.5,
                elapsed_seconds: 2.5,
            }]
        );
    }

    #[test]
    fn pause_blocks_time_progression() {
        let mut simulation = Simulation::new(UniverseConfig::default());

        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::Paused));
        let events = simulation.tick(10.0);

        assert!(events.is_empty());
        assert_eq!(simulation.state().elapsed_seconds, 0.0);
    }

    #[test]
    fn selection_events_use_domain_ids() {
        let mut simulation = Simulation::new(UniverseConfig::default());

        let events = simulation.apply_command(GameCommand::SelectPlanet {
            system_id: SystemId::new(0),
            planet_id: PlanetId::new(0),
        });

        assert_eq!(
            events,
            vec![GameEvent::SelectionChanged(SelectionTarget::Planet {
                system_id: SystemId::new(0),
                planet_id: PlanetId::new(0),
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
            SelectionTarget::System(SystemId::new(0))
        );
    }
}

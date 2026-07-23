use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig};
use galactic_sim::{ColonyState, GameState, SelectionTarget, TimeSpeed};

pub const SAVE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    pub seed: u64,
    pub system_count: usize,
    pub elapsed_seconds: f32,
    pub speed: TimeSpeed,
    pub selected: SelectionTarget,
    pub known_systems: Vec<SystemId>,
    pub colonies: Vec<ColonySave>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonySave {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    UnsupportedVersion(u32),
}

pub fn snapshot_from_state(state: &GameState) -> SaveGame {
    SaveGame {
        version: SAVE_VERSION,
        seed: state.universe.seed,
        system_count: state.universe.systems.len(),
        elapsed_seconds: state.elapsed_seconds,
        speed: state.speed,
        selected: state.selected,
        known_systems: state.known_systems.clone(),
        colonies: state
            .colonies
            .iter()
            .map(|colony| ColonySave {
                id: colony.id,
                name: colony.name.clone(),
                faction: colony.faction,
                system_id: colony.system_id,
                planet_id: colony.planet_id,
                stock: colony.stock,
            })
            .collect(),
    }
}

pub fn restore_from_snapshot(save: &SaveGame) -> Result<GameState, SaveError> {
    if save.version != SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion(save.version));
    }

    let mut state = GameState::new(UniverseConfig::new(save.seed, save.system_count));
    state.elapsed_seconds = save.elapsed_seconds;
    state.speed = save.speed;
    state.selected = save.selected;
    state.known_systems = save.known_systems.clone();
    state.colonies = save
        .colonies
        .iter()
        .map(|colony| ColonyState {
            id: colony.id,
            name: colony.name.clone(),
            faction: colony.faction,
            system_id: colony.system_id,
            planet_id: colony.planet_id,
            stock: colony.stock,
        })
        .collect();

    Ok(state)
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::{GameCommand, Simulation, TimeSpeed};

    use super::*;

    #[test]
    fn snapshot_round_trips_business_state() {
        let mut simulation = Simulation::new(UniverseConfig::new(99, 14));
        simulation.tick(12.0);
        simulation.apply_command(GameCommand::SetSpeed(TimeSpeed::X4));

        let save = snapshot_from_state(simulation.state());
        let restored = restore_from_snapshot(&save).expect("save version is supported");

        assert_eq!(restored.universe, simulation.state().universe);
        assert_eq!(restored.elapsed_seconds, 12.0);
        assert_eq!(restored.speed, TimeSpeed::X4);
        assert_eq!(restored.colonies, simulation.state().colonies);
    }

    #[test]
    fn unsupported_save_version_is_rejected() {
        let state = GameState::new(UniverseConfig::default());
        let mut save = snapshot_from_state(&state);
        save.version = 999;

        assert_eq!(
            restore_from_snapshot(&save),
            Err(SaveError::UnsupportedVersion(999))
        );
    }
}

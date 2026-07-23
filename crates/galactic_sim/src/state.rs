// MVP-005: mutable game state owns a fixed, serializable strategic clock
use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, SystemId};

use crate::{SelectionTarget, StrategicClock, UniverseRepository};

/// Version of the mutable in-memory state contract.
///
/// Version 2 replaces floating elapsed seconds with a deterministic tick clock.
pub const GAME_STATE_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub version: u32,
    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub known_systems: Vec<SystemId>,
    pub selected: SelectionTarget,
    pub clock: StrategicClock,
}

impl GameState {
    pub fn new(universe: &UniverseRepository) -> Self {
        let home_system_id = SystemId::from_index(0);
        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);
        let player_faction = FactionId::new(0);
        let mut known_systems = vec![home_system_id];
        known_systems.extend(universe.neighboring_systems(home_system_id));
        known_systems.sort();
        known_systems.dedup();

        debug_assert!(universe.system(home_system_id).is_some());
        debug_assert!(universe.planet(home_planet_id).is_some());

        Self {
            version: GAME_STATE_VERSION,
            player_faction,
            colonies: vec![ColonyState {
                id: ColonyId::new(0),
                name: "Aster Prime Colony".to_string(),
                faction: player_faction,
                system_id: home_system_id,
                planet_id: home_planet_id,
                stock: ResourceStock::new(120, 45, 80, 30),
            }],
            known_systems,
            selected: SelectionTarget::System(home_system_id),
            clock: StrategicClock::new(),
        }
    }

    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {
        self.colonies.iter().find(|colony| colony.id == id)
    }

    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {
        self.colonies.iter_mut().find(|colony| colony.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonyState {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, UniverseConfig};

    use super::*;

    #[test]
    fn colony_is_accessible_by_stable_id() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);

        let colony = state
            .colony(ColonyId::new(0))
            .expect("home colony is indexed by its stable ID");

        assert_eq!(colony.name, "Aster Prime Colony");
    }

    #[test]
    fn new_game_starts_at_tick_zero_and_speed_one() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);

        assert_eq!(state.clock.current_tick().value(), 0);
        assert_eq!(state.clock.speed(), crate::TimeSpeed::X1);
    }
}

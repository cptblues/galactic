// MVP-007: universe visibility is derived from the discovered neighborhood
use std::collections::BTreeSet;

use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId};

use crate::{SelectionTarget, StrategicClock, UniverseRepository};

/// Version of the mutable in-memory state contract.
///
/// Version 2 replaces floating elapsed seconds with a deterministic tick clock.
pub const GAME_STATE_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemVisibility {
    Known,
    Detected,
}

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

    pub fn is_system_known(&self, system_id: SystemId) -> bool {
        self.known_systems.contains(&system_id)
    }

    /// Systems directly adjacent to known systems form the current detection
    /// frontier. This is presentation-oriented until MVP-009 stores explicit
    /// knowledge levels.
    pub fn detected_systems(&self, universe: &UniverseRepository) -> Vec<SystemId> {
        let mut detected = BTreeSet::new();

        for known_system in &self.known_systems {
            for neighbor in universe.neighboring_systems(*known_system) {
                if !self.is_system_known(neighbor) {
                    detected.insert(neighbor);
                }
            }
        }

        detected.into_iter().collect()
    }

    pub fn system_visibility(
        &self,
        universe: &UniverseRepository,
        system_id: SystemId,
    ) -> Option<SystemVisibility> {
        if self.is_system_known(system_id) {
            return Some(SystemVisibility::Known);
        }

        self.detected_systems(universe)
            .binary_search(&system_id)
            .ok()
            .map(|_| SystemVisibility::Detected)
    }

    pub fn visible_systems(
        &self,
        universe: &UniverseRepository,
    ) -> Vec<(SystemId, SystemVisibility)> {
        let mut systems = self
            .known_systems
            .iter()
            .copied()
            .map(|system_id| (system_id, SystemVisibility::Known))
            .collect::<Vec<_>>();

        systems.extend(
            self.detected_systems(universe)
                .into_iter()
                .map(|system_id| (system_id, SystemVisibility::Detected)),
        );
        systems.sort_by_key(|(system_id, _)| *system_id);
        systems
    }

    pub fn is_system_visible(&self, universe: &UniverseRepository, system_id: SystemId) -> bool {
        self.system_visibility(universe, system_id).is_some()
    }

    /// Visible routes connect known systems to each other or to the immediate
    /// detected frontier. Routes between two merely detected systems remain
    /// hidden so the map never reveals information beyond that frontier.
    pub fn visible_routes<'a>(&self, universe: &'a UniverseRepository) -> Vec<&'a Route> {
        universe
            .definition()
            .routes
            .iter()
            .filter(|route| {
                let from = self.system_visibility(universe, route.from);
                let to = self.system_visibility(universe, route.to);

                (from == Some(SystemVisibility::Known) && to.is_some())
                    || (from == Some(SystemVisibility::Detected)
                        && to == Some(SystemVisibility::Known))
            })
            .collect()
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

    #[test]
    fn detection_frontier_contains_only_neighbors_of_known_systems() {
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&repository);
        let detected = state.detected_systems(&repository);

        assert!(detected.iter().all(|system_id| {
            !state.is_system_known(*system_id)
                && state
                    .known_systems
                    .iter()
                    .any(|known| repository.route_exists(*known, *system_id))
        }));
    }

    #[test]
    fn normal_visibility_never_reveals_beyond_the_detection_frontier() {
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&repository);
        let visible = state.visible_systems(&repository);
        let visible_ids = visible
            .iter()
            .map(|(system_id, _)| *system_id)
            .collect::<BTreeSet<_>>();

        assert!(visible.len() <= repository.definition().systems.len());
        for (system_id, visibility) in visible {
            match visibility {
                SystemVisibility::Known => {
                    assert!(state.is_system_known(system_id));
                }
                SystemVisibility::Detected => {
                    assert!(
                        state
                            .known_systems
                            .iter()
                            .any(|known| { repository.route_exists(*known, system_id) })
                    );
                }
            }
        }

        assert!(state.visible_routes(&repository).iter().all(|route| {
            visible_ids.contains(&route.from)
                && visible_ids.contains(&route.to)
                && (state.is_system_known(route.from) || state.is_system_known(route.to))
        }));
    }
}

// MVP-008: configurable player faction, home world and starting colony
use std::collections::BTreeSet;

use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId};

use crate::{
    BuildingLevels, PlanetResourceProfile, SelectionTarget, StartingScenario,
    StartingScenarioError, StrategicClock, UniverseRepository,
};

/// Version of the mutable in-memory state contract.
///
/// Version 3 adds factions and configurable colony foundation data.
pub const GAME_STATE_VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemVisibility {
    Known,
    Detected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactionKind {
    Player,
    Neutral,
    FutureAi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactionState {
    pub id: FactionId,
    pub name: String,
    pub kind: FactionKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub version: u32,
    pub factions: Vec<FactionState>,
    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub known_systems: Vec<SystemId>,
    pub selected: SelectionTarget,
    pub clock: StrategicClock,
}

impl GameState {
    pub fn new(universe: &UniverseRepository) -> Self {
        Self::from_starting_scenario(universe, StartingScenario::mvp())
            .expect("the MVP starting scenario must match the reference universe")
    }

    pub fn from_starting_scenario(
        universe: &UniverseRepository,
        scenario: StartingScenario,
    ) -> Result<Self, StartingScenarioError> {
        scenario.validate(universe)?;

        let mut known_systems = scenario.initially_known_systems.to_vec();
        known_systems.sort();
        known_systems.dedup();

        let player_faction = scenario.player_faction.id;
        let home = scenario.home_colony;

        Ok(Self {
            version: GAME_STATE_VERSION,
            factions: vec![FactionState {
                id: player_faction,
                name: scenario.player_faction.name.to_string(),
                kind: FactionKind::Player,
            }],
            player_faction,
            colonies: vec![ColonyState {
                id: home.id,
                name: home.name.to_string(),
                faction: player_faction,
                system_id: home.system_id,
                planet_id: home.planet_id,
                stock: home.initial_stock,
                buildings: home.buildings,
                resource_profile: home.resource_profile,
            }],
            known_systems,
            selected: SelectionTarget::Planet {
                system_id: home.system_id,
                planet_id: home.planet_id,
            },
            clock: StrategicClock::new(),
        })
    }

    pub fn faction(&self, id: FactionId) -> Option<&FactionState> {
        self.factions.iter().find(|faction| faction.id == id)
    }

    pub fn player_faction_state(&self) -> Option<&FactionState> {
        self.faction(self.player_faction)
    }

    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {
        self.colonies.iter().find(|colony| colony.id == id)
    }

    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {
        self.colonies.iter_mut().find(|colony| colony.id == id)
    }

    pub fn colony_on_planet(&self, planet_id: PlanetId) -> Option<&ColonyState> {
        self.colonies
            .iter()
            .find(|colony| colony.planet_id == planet_id)
    }

    pub fn player_home_colony(&self) -> Option<&ColonyState> {
        self.colonies
            .iter()
            .find(|colony| colony.faction == self.player_faction)
    }

    pub fn is_system_known(&self, system_id: SystemId) -> bool {
        self.known_systems.contains(&system_id)
    }

    /// Systems directly adjacent to known systems form the current detection
    /// frontier. MVP-009 will replace this with persisted knowledge levels.
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
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, UniverseConfig};

    use super::*;

    #[test]
    fn new_game_uses_stable_home_world_and_player_faction() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let scenario = StartingScenario::mvp();
        let colony = state.colony(ColonyId::new(0)).expect("home colony exists");

        assert_eq!(state.player_faction, scenario.player_faction.id);
        assert_eq!(state.factions.len(), 1);
        assert_eq!(
            state
                .player_faction_state()
                .expect("player faction exists")
                .kind,
            FactionKind::Player
        );
        assert_eq!(colony.system_id, scenario.home_colony.system_id);
        assert_eq!(colony.planet_id, scenario.home_colony.planet_id);
    }

    #[test]
    fn home_planet_supports_the_starting_loop() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let colony = state.player_home_colony().expect("home colony exists");
        let planet = universe
            .planet(colony.planet_id)
            .expect("home planet exists");

        assert!(planet.habitability >= StartingScenario::mvp().minimum_home_habitability);
        assert!(colony.stock.can_cover(ResourceStock::new(100, 50, 25, 0)));
        assert!(colony.resource_profile.is_viable());
        assert!(
            colony.buildings.total_levels() >= 6,
            "the colony starts with the six foundation buildings"
        );
    }

    #[test]
    fn only_home_system_is_known_at_start() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let home = StartingScenario::mvp().home_colony.system_id;

        assert_eq!(state.known_systems, vec![home]);
        assert!(!state.detected_systems(&universe).is_empty());
        assert!(
            state
                .detected_systems(&universe)
                .iter()
                .all(|system_id| { universe.route_exists(home, *system_id) })
        );
    }

    #[test]
    fn custom_starting_data_does_not_change_generated_universe() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let fingerprint = universe.definition().generation_fingerprint;
        let mut scenario = StartingScenario::mvp();
        scenario.home_colony.initial_stock = ResourceStock::new(900, 700, 500, 100);
        scenario.home_colony.buildings.research_lab = 1;

        let state = GameState::from_starting_scenario(&universe, scenario)
            .expect("custom starting scenario is valid");
        let colony = state.player_home_colony().expect("home colony exists");

        assert_eq!(colony.stock, ResourceStock::new(900, 700, 500, 100));
        assert_eq!(colony.buildings.research_lab, 1);
        assert_eq!(universe.definition().generation_fingerprint, fingerprint);
    }
}

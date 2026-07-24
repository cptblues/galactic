// MVP-011: persistent knowledge and colony economy
use galactic_domain::{ColonyId, EnergyGrid, FactionId, PlanetId, ResourceLedger, Route, SystemId};

use crate::{
    BuildingLevels, KnowledgeChange, KnowledgeCounts, KnowledgeLevel, KnowledgeTarget,
    PlanetKnowledge, PlanetResourceProfile, SelectionTarget, StartingScenario,
    StartingScenarioError, StrategicClock, SystemKnowledge, UniverseRepository,
};

/// Version of the mutable in-memory state contract.
///
/// Version 5 adds atomic resource ledgers and an energy grid per colony.
pub const GAME_STATE_VERSION: u32 = 5;

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
    pub system_knowledge: Vec<SystemKnowledge>,
    pub planet_knowledge: Vec<PlanetKnowledge>,
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

        let player_faction = scenario.player_faction.id;
        let home = scenario.home_colony;

        let mut state = Self {
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
                resources: ResourceLedger::new(home.initial_stock),
                energy: home.initial_energy,
                buildings: home.buildings,
                resource_profile: home.resource_profile,
            }],
            system_knowledge: Vec::new(),
            planet_knowledge: Vec::new(),
            selected: SelectionTarget::Planet {
                system_id: home.system_id,
                planet_id: home.planet_id,
            },
            clock: StrategicClock::new(),
        };

        for knowledge in scenario.initial_system_knowledge {
            state.advance_system_knowledge(universe, knowledge.system_id, knowledge.level);
        }
        for knowledge in scenario.initial_planet_knowledge {
            state.advance_planet_knowledge(universe, knowledge.planet_id, knowledge.level);
        }

        Ok(state)
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

    pub fn system_knowledge_level(&self, system_id: SystemId) -> KnowledgeLevel {
        self.system_knowledge
            .iter()
            .find(|entry| entry.system_id == system_id)
            .map(|entry| entry.level)
            .unwrap_or_default()
    }

    pub fn planet_knowledge_level(&self, planet_id: PlanetId) -> KnowledgeLevel {
        self.planet_knowledge
            .iter()
            .find(|entry| entry.planet_id == planet_id)
            .map(|entry| entry.level)
            .unwrap_or_default()
    }

    pub fn is_system_known(&self, system_id: SystemId) -> bool {
        self.system_knowledge_level(system_id).reveals_identity()
    }

    pub fn is_system_visible(&self, system_id: SystemId) -> bool {
        self.system_knowledge_level(system_id).is_visible()
    }

    pub fn known_system_count(&self) -> usize {
        self.system_knowledge
            .iter()
            .filter(|entry| entry.level.reveals_identity())
            .count()
    }

    pub fn system_knowledge_counts(&self) -> KnowledgeCounts {
        let mut counts = KnowledgeCounts::default();
        for entry in &self.system_knowledge {
            counts.include(entry.level);
        }
        counts
    }

    pub fn planet_knowledge_counts(&self) -> KnowledgeCounts {
        let mut counts = KnowledgeCounts::default();
        for entry in &self.planet_knowledge {
            counts.include(entry.level);
        }
        counts
    }

    pub fn system_visibility(&self, system_id: SystemId) -> Option<SystemVisibility> {
        match self.system_knowledge_level(system_id) {
            KnowledgeLevel::Unknown => None,
            KnowledgeLevel::Detected => Some(SystemVisibility::Detected),
            KnowledgeLevel::Probed | KnowledgeLevel::Analyzed | KnowledgeLevel::Colonized => {
                Some(SystemVisibility::Known)
            }
        }
    }

    pub fn visible_systems(&self) -> Vec<(SystemId, SystemVisibility)> {
        let mut systems = self
            .system_knowledge
            .iter()
            .filter_map(|entry| {
                self.system_visibility(entry.system_id)
                    .map(|visibility| (entry.system_id, visibility))
            })
            .collect::<Vec<_>>();
        systems.sort_by_key(|(system_id, _)| *system_id);
        systems
    }

    pub fn visible_routes<'a>(&self, universe: &'a UniverseRepository) -> Vec<&'a Route> {
        universe
            .definition()
            .routes
            .iter()
            .filter(|route| {
                let from = self.system_knowledge_level(route.from);
                let to = self.system_knowledge_level(route.to);

                from.is_visible()
                    && to.is_visible()
                    && (from.reveals_identity() || to.reveals_identity())
            })
            .collect()
    }

    /// Raises a system's knowledge and propagates the immediate frontier.
    ///
    /// Once a system is probed, all its planets are detected and adjacent
    /// systems become detected. No information ever regresses.
    pub fn advance_system_knowledge(
        &mut self,
        universe: &UniverseRepository,
        system_id: SystemId,
        requested: KnowledgeLevel,
    ) -> Vec<KnowledgeChange> {
        let Some(system) = universe.system(system_id) else {
            return Vec::new();
        };

        let mut changes = Vec::new();
        if let Some(change) = self.upsert_system_knowledge(system_id, requested) {
            changes.push(change);
        }

        let effective = self.system_knowledge_level(system_id);
        if effective.reveals_identity() {
            for neighbor in universe.neighboring_systems(system_id) {
                if let Some(change) =
                    self.upsert_system_knowledge(neighbor, KnowledgeLevel::Detected)
                {
                    changes.push(change);
                }
            }

            for planet in &system.planets {
                if let Some(change) =
                    self.upsert_planet_knowledge(planet.id, KnowledgeLevel::Detected)
                {
                    changes.push(change);
                }
            }
        }

        changes
    }

    pub fn advance_planet_knowledge(
        &mut self,
        universe: &UniverseRepository,
        planet_id: PlanetId,
        requested: KnowledgeLevel,
    ) -> Vec<KnowledgeChange> {
        let Some((system_id, _)) = universe.planet_location(planet_id) else {
            return Vec::new();
        };

        let required_system_level = match requested {
            KnowledgeLevel::Unknown => KnowledgeLevel::Unknown,
            KnowledgeLevel::Detected => KnowledgeLevel::Detected,
            KnowledgeLevel::Probed | KnowledgeLevel::Analyzed => KnowledgeLevel::Probed,
            KnowledgeLevel::Colonized => KnowledgeLevel::Colonized,
        };

        let mut changes = self.advance_system_knowledge(universe, system_id, required_system_level);
        if let Some(change) = self.upsert_planet_knowledge(planet_id, requested) {
            changes.push(change);
        }
        changes
    }

    fn upsert_system_knowledge(
        &mut self,
        system_id: SystemId,
        requested: KnowledgeLevel,
    ) -> Option<KnowledgeChange> {
        if requested == KnowledgeLevel::Unknown {
            return None;
        }

        if let Some(entry) = self
            .system_knowledge
            .iter_mut()
            .find(|entry| entry.system_id == system_id)
        {
            if requested <= entry.level {
                return None;
            }
            let previous = entry.level;
            entry.level = requested;
            return Some(KnowledgeChange {
                target: KnowledgeTarget::System(system_id),
                previous,
                current: requested,
            });
        }

        self.system_knowledge.push(SystemKnowledge {
            system_id,
            level: requested,
        });
        self.system_knowledge.sort_by_key(|entry| entry.system_id);
        Some(KnowledgeChange {
            target: KnowledgeTarget::System(system_id),
            previous: KnowledgeLevel::Unknown,
            current: requested,
        })
    }

    fn upsert_planet_knowledge(
        &mut self,
        planet_id: PlanetId,
        requested: KnowledgeLevel,
    ) -> Option<KnowledgeChange> {
        if requested == KnowledgeLevel::Unknown {
            return None;
        }

        if let Some(entry) = self
            .planet_knowledge
            .iter_mut()
            .find(|entry| entry.planet_id == planet_id)
        {
            if requested <= entry.level {
                return None;
            }
            let previous = entry.level;
            entry.level = requested;
            return Some(KnowledgeChange {
                target: KnowledgeTarget::Planet(planet_id),
                previous,
                current: requested,
            });
        }

        self.planet_knowledge.push(PlanetKnowledge {
            planet_id,
            level: requested,
        });
        self.planet_knowledge.sort_by_key(|entry| entry.planet_id);
        Some(KnowledgeChange {
            target: KnowledgeTarget::Planet(planet_id),
            previous: KnowledgeLevel::Unknown,
            current: requested,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonyState {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub resources: ResourceLedger,
    pub energy: EnergyGrid,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, PlanetId, SystemId, UniverseConfig};

    use super::*;

    #[test]
    fn new_game_has_colonized_home_and_detected_frontier() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let scenario = StartingScenario::mvp();

        assert_eq!(
            state.system_knowledge_level(scenario.home_colony.system_id),
            KnowledgeLevel::Colonized
        );
        assert_eq!(
            state.planet_knowledge_level(scenario.home_colony.planet_id),
            KnowledgeLevel::Colonized
        );
        assert!(
            universe
                .neighboring_systems(scenario.home_colony.system_id)
                .into_iter()
                .all(|neighbor| {
                    state.system_knowledge_level(neighbor) == KnowledgeLevel::Detected
                })
        );
    }

    #[test]
    fn probing_system_reveals_planets_and_next_frontier() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let mut state = GameState::new(&universe);
        let target = universe
            .neighboring_systems(SystemId::from_index(0))
            .into_iter()
            .next()
            .expect("home has a neighbor");

        let changes = state.advance_system_knowledge(&universe, target, KnowledgeLevel::Probed);

        assert!(!changes.is_empty());
        assert_eq!(state.system_knowledge_level(target), KnowledgeLevel::Probed);
        let system = universe.system(target).expect("target exists");
        assert!(
            system.planets.iter().all(|planet| {
                state.planet_knowledge_level(planet.id) == KnowledgeLevel::Detected
            })
        );
        assert!(
            universe
                .neighboring_systems(target)
                .into_iter()
                .all(|neighbor| { state.system_knowledge_level(neighbor).is_visible() })
        );
    }

    #[test]
    fn knowledge_never_regresses() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let mut state = GameState::new(&universe);
        let home = SystemId::from_index(0);

        let changes = state.advance_system_knowledge(&universe, home, KnowledgeLevel::Detected);

        assert!(changes.is_empty());
        assert_eq!(
            state.system_knowledge_level(home),
            KnowledgeLevel::Colonized
        );
    }

    #[test]
    fn colony_is_accessible_by_stable_id() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);

        let colony = state
            .colony(ColonyId::new(0))
            .expect("home colony is indexed");

        assert_eq!(colony.name, "Aster Prime Colony");
    }

    #[test]
    fn home_colony_has_atomic_resources_and_energy_capacity() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let colony = state.player_home_colony().expect("home colony exists");

        assert_eq!(
            colony.resources.stock(),
            galactic_domain::ResourceStock::new(600, 300, 220)
        );
        assert_eq!(colony.resources.available(), colony.resources.stock());
        assert_eq!(colony.energy.production(), 80);
        assert_eq!(colony.energy.consumption(), 30);
        assert_eq!(colony.energy.balance(), 50);
    }

    #[test]
    fn non_home_planets_start_as_detected_only() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let home_system = universe
            .system(SystemId::from_index(0))
            .expect("home system exists");

        for planet in &home_system.planets {
            let expected = if planet.id == PlanetId::from_system_index(SystemId::from_index(0), 0) {
                KnowledgeLevel::Colonized
            } else {
                KnowledgeLevel::Detected
            };
            assert_eq!(state.planet_knowledge_level(planet.id), expected);
        }
    }
}

// MVP-018: faction-owned state with centralized management authorization.
use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, Owned, Owner, PlanetId, ResourceLedger, Route, SystemId,
};

use crate::{
    BuildingLevels, ConstructionQueue, CraftInventory, CraftQueue, KnowledgeChange,
    KnowledgeCounts, KnowledgeLevel, KnowledgeTarget, PlanetKnowledge, PlanetResourceProfile,
    ProductionRemainder, ResearchState, SelectionTarget, StartingScenario, StartingScenarioError,
    StrategicClock, SystemKnowledge, UniverseRepository,
};

/// Version of the mutable in-memory state contract.
///
/// Version 12 adds generic owners, configurable faction data and authorization.
pub const GAME_STATE_VERSION: u32 = 12;

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
pub struct FactionData {
    pub id: FactionId,
    pub name: String,
    pub kind: FactionKind,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationError {
    UnknownActor(FactionId),
    InactiveActor(FactionId),
    UnownedTarget,
    UnknownOwner(FactionId),
    NotOwner { actor: FactionId, owner: FactionId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub version: u32,
    pub factions: Vec<FactionData>,
    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub research: ResearchState,
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

        let player_faction = scenario.player_faction_id;
        let home = scenario.home_colony;

        let mut state = Self {
            version: GAME_STATE_VERSION,
            factions: scenario
                .factions
                .iter()
                .map(|faction| FactionData {
                    id: faction.id,
                    name: faction.name.to_string(),
                    kind: faction.kind,
                    active: faction.active,
                })
                .collect(),
            player_faction,
            colonies: vec![ColonyState {
                id: home.id,
                name: home.name.to_string(),
                owner: home.owner,
                system_id: home.system_id,
                planet_id: home.planet_id,
                resources: ResourceLedger::new(home.initial_stock),
                energy: home.initial_energy,
                production_remainder: ProductionRemainder::ZERO,
                production_pending_ticks: 0,
                construction_queue: ConstructionQueue::default(),
                craft_queue: CraftQueue::default(),
                inventory: CraftInventory::default(),
                buildings: home.buildings,
                resource_profile: home.resource_profile,
            }],
            research: ResearchState::from_completed(scenario.initial_technologies.iter().copied()),
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

    pub fn faction(&self, id: FactionId) -> Option<&FactionData> {
        self.factions.iter().find(|faction| faction.id == id)
    }

    pub fn player_faction_state(&self) -> Option<&FactionData> {
        self.faction(self.player_faction)
    }

    pub fn authorize_management(
        &self,
        actor: FactionId,
        owner: Owner,
    ) -> Result<(), AuthorizationError> {
        let Some(actor_data) = self.faction(actor) else {
            return Err(AuthorizationError::UnknownActor(actor));
        };
        if !actor_data.active {
            return Err(AuthorizationError::InactiveActor(actor));
        }
        let Owner::Faction(owner_id) = owner else {
            return Err(AuthorizationError::UnownedTarget);
        };
        if self.faction(owner_id).is_none() {
            return Err(AuthorizationError::UnknownOwner(owner_id));
        }
        if actor != owner_id {
            return Err(AuthorizationError::NotOwner {
                actor,
                owner: owner_id,
            });
        }
        Ok(())
    }

    pub fn can_manage(&self, actor: FactionId, owner: Owner) -> bool {
        self.authorize_management(actor, owner).is_ok()
    }

    pub fn player_colonies(&self) -> impl Iterator<Item = &ColonyState> {
        self.colonies
            .iter()
            .filter(|colony| self.can_manage(self.player_faction, colony.owner))
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
        self.player_colonies().next()
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
    pub owner: Owner,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub resources: ResourceLedger,
    pub energy: EnergyGrid,
    pub production_remainder: ProductionRemainder,
    pub production_pending_ticks: u16,
    pub construction_queue: ConstructionQueue,
    pub craft_queue: CraftQueue,
    pub inventory: CraftInventory,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

impl Owned for ColonyState {
    fn owner(&self) -> Owner {
        self.owner
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FactionId, Owner, PlanetId, SystemId, UniverseConfig};

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
    fn new_game_contains_configured_dormant_factions() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);

        assert_eq!(state.factions.len(), 3);
        assert_eq!(
            state
                .factions
                .iter()
                .filter(|faction| faction.active)
                .count(),
            1,
        );
        assert!(
            state
                .factions
                .iter()
                .any(|faction| faction.kind == FactionKind::Neutral && !faction.active)
        );
        assert!(
            state
                .factions
                .iter()
                .any(|faction| faction.kind == FactionKind::FutureAi && !faction.active)
        );
    }

    #[test]
    fn management_authorization_is_owner_based() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let player = state.player_faction;
        let foreign = FactionId::new(2);

        assert_eq!(
            state.authorize_management(player, Owner::Faction(player)),
            Ok(()),
        );
        assert!(matches!(
            state.authorize_management(player, Owner::Faction(foreign)),
            Err(AuthorizationError::NotOwner { actor, owner })
                if actor == player && owner == foreign
        ));
        assert_eq!(
            state.authorize_management(foreign, Owner::Faction(foreign)),
            Err(AuthorizationError::InactiveActor(foreign)),
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
    fn home_stock_fits_derived_storage_capacity() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let colony = state.player_home_colony().expect("home colony exists");
        let capacity = crate::storage_capacity(colony.buildings);

        assert!(colony.resources.stock().is_within(capacity));
        assert_eq!(colony.production_remainder, ProductionRemainder::ZERO);
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

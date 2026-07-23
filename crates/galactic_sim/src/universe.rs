// MVP-004: immutable generated universe separated from mutable game state
use std::collections::HashMap;

use galactic_domain::{
    Planet, PlanetId, StarSystem, SystemId, UniverseConfig, UniverseDefinition, generate_universe,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseIndexError {
    DuplicateSystem(SystemId),
    DuplicatePlanet(PlanetId),
}

/// Read-only repository around a generated universe.
///
/// The definition is owned by the simulation but has no mutable accessor. All
/// runtime changes belong in `GameState` instead.
#[derive(Debug, Clone)]
pub struct UniverseRepository {
    definition: UniverseDefinition,
    system_indices: HashMap<SystemId, usize>,
    planet_indices: HashMap<PlanetId, (usize, usize)>,
}

impl UniverseRepository {
    pub fn generate(config: UniverseConfig) -> Self {
        Self::new(generate_universe(config))
            .expect("the deterministic universe generator must produce unique stable IDs")
    }

    pub fn new(definition: UniverseDefinition) -> Result<Self, UniverseIndexError> {
        let mut system_indices = HashMap::with_capacity(definition.systems.len());
        let mut planet_indices = HashMap::new();

        for (system_index, system) in definition.systems.iter().enumerate() {
            if system_indices.insert(system.id, system_index).is_some() {
                return Err(UniverseIndexError::DuplicateSystem(system.id));
            }

            for (planet_index, planet) in system.planets.iter().enumerate() {
                if planet_indices
                    .insert(planet.id, (system_index, planet_index))
                    .is_some()
                {
                    return Err(UniverseIndexError::DuplicatePlanet(planet.id));
                }
            }
        }

        Ok(Self {
            definition,
            system_indices,
            planet_indices,
        })
    }

    pub fn definition(&self) -> &UniverseDefinition {
        &self.definition
    }

    pub fn system(&self, id: SystemId) -> Option<&StarSystem> {
        let index = *self.system_indices.get(&id)?;
        self.definition.systems.get(index)
    }

    pub fn planet(&self, id: PlanetId) -> Option<&Planet> {
        let (system_index, planet_index) = *self.planet_indices.get(&id)?;
        self.definition
            .systems
            .get(system_index)?
            .planets
            .get(planet_index)
    }

    pub fn planet_location(&self, id: PlanetId) -> Option<(SystemId, &Planet)> {
        let (system_index, planet_index) = *self.planet_indices.get(&id)?;
        let system = self.definition.systems.get(system_index)?;
        let planet = system.planets.get(planet_index)?;
        Some((system.id, planet))
    }

    pub fn neighboring_systems(&self, id: SystemId) -> Vec<SystemId> {
        self.definition.neighboring_systems(id)
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{PlanetId, SystemId, UniverseConfig};

    use super::*;

    #[test]
    fn repository_accesses_systems_and_planets_by_stable_id() {
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let home_system_id = SystemId::from_index(0);
        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);

        let system = repository
            .system(home_system_id)
            .expect("home system is indexed");
        let planet = repository
            .planet(home_planet_id)
            .expect("home planet is indexed");
        let (located_system_id, located_planet) = repository
            .planet_location(home_planet_id)
            .expect("planet location is indexed");

        assert_eq!(system.id, home_system_id);
        assert_eq!(planet.id, home_planet_id);
        assert_eq!(located_system_id, home_system_id);
        assert_eq!(located_planet.id, home_planet_id);
    }

    #[test]
    fn regenerated_repository_matches_the_reference_universe() {
        let left = UniverseRepository::generate(UniverseConfig::mvp());
        let right = UniverseRepository::generate(UniverseConfig::mvp());

        assert_eq!(left.definition(), right.definition());
    }
}

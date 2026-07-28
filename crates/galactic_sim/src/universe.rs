// MVP-006: indexed connected graph and deterministic path finding
use std::collections::{HashMap, HashSet, VecDeque};

use galactic_domain::{
    Planet, PlanetId, Route, SectorDefinition, SectorId, StarSystem, SystemId, UniverseConfig,
    UniverseDefinition, generate_universe,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseIndexError {
    DuplicateSystem(SystemId),
    DuplicatePlanet(PlanetId),
    UnknownRouteEndpoint(SystemId),
    SelfRoute(SystemId),
    DuplicateRoute(Route),
    DuplicateSector(SectorId),
    UnknownSectorSystem {
        sector_id: SectorId,
        system_id: SystemId,
    },
    DuplicateSectorMembership(SystemId),
    MissingSectorMembership(SystemId),
    DuplicateSectorGateway {
        sector_id: SectorId,
        route: Route,
    },
    InvalidSectorGateway {
        sector_id: SectorId,
        route: Route,
    },
    MissingSectorGateway {
        sector_id: SectorId,
        route: Route,
    },
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
    sector_indices: HashMap<SectorId, usize>,
    system_sectors: HashMap<SystemId, SectorId>,
    adjacency: HashMap<SystemId, Vec<SystemId>>,
}

impl UniverseRepository {
    pub fn generate(config: UniverseConfig) -> Self {
        Self::new(generate_universe(config))
            .expect("the deterministic universe generator must produce a valid connected graph")
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

        let mut adjacency = system_indices
            .keys()
            .copied()
            .map(|system_id| (system_id, Vec::new()))
            .collect::<HashMap<_, _>>();
        let mut route_set = HashSet::with_capacity(definition.routes.len());

        for route in &definition.routes {
            if route.from == route.to {
                return Err(UniverseIndexError::SelfRoute(route.from));
            }
            if !system_indices.contains_key(&route.from) {
                return Err(UniverseIndexError::UnknownRouteEndpoint(route.from));
            }
            if !system_indices.contains_key(&route.to) {
                return Err(UniverseIndexError::UnknownRouteEndpoint(route.to));
            }

            let canonical = Route::new(route.from, route.to);
            if !route_set.insert(canonical) {
                return Err(UniverseIndexError::DuplicateRoute(canonical));
            }

            adjacency
                .get_mut(&canonical.from)
                .expect("route endpoint was validated")
                .push(canonical.to);
            adjacency
                .get_mut(&canonical.to)
                .expect("route endpoint was validated")
                .push(canonical.from);
        }

        for neighbors in adjacency.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }

        let mut sector_indices = HashMap::with_capacity(definition.sectors.len());
        let mut system_sectors = HashMap::with_capacity(definition.systems.len());
        for (sector_index, sector) in definition.sectors.iter().enumerate() {
            if sector_indices.insert(sector.id, sector_index).is_some() {
                return Err(UniverseIndexError::DuplicateSector(sector.id));
            }
            for system_id in &sector.systems {
                if !system_indices.contains_key(system_id) {
                    return Err(UniverseIndexError::UnknownSectorSystem {
                        sector_id: sector.id,
                        system_id: *system_id,
                    });
                }
                if system_sectors.insert(*system_id, sector.id).is_some() {
                    return Err(UniverseIndexError::DuplicateSectorMembership(*system_id));
                }
            }
        }

        for system_id in system_indices.keys() {
            if !system_sectors.contains_key(system_id) {
                return Err(UniverseIndexError::MissingSectorMembership(*system_id));
            }
        }

        for sector in &definition.sectors {
            let mut gateways = HashSet::with_capacity(sector.gateway_routes.len());
            for route in &sector.gateway_routes {
                let canonical = Route::new(route.from, route.to);
                if !gateways.insert(canonical) {
                    return Err(UniverseIndexError::DuplicateSectorGateway {
                        sector_id: sector.id,
                        route: canonical,
                    });
                }
                let from_sector = system_sectors.get(&canonical.from);
                let to_sector = system_sectors.get(&canonical.to);
                let is_valid = route_set.contains(&canonical)
                    && from_sector != to_sector
                    && (from_sector == Some(&sector.id) || to_sector == Some(&sector.id));
                if !is_valid {
                    return Err(UniverseIndexError::InvalidSectorGateway {
                        sector_id: sector.id,
                        route: canonical,
                    });
                }
            }

            for route in &definition.routes {
                let from_sector = system_sectors[&route.from];
                let to_sector = system_sectors[&route.to];
                let expected = from_sector != to_sector
                    && (from_sector == sector.id || to_sector == sector.id);
                if expected && !gateways.contains(route) {
                    return Err(UniverseIndexError::MissingSectorGateway {
                        sector_id: sector.id,
                        route: *route,
                    });
                }
            }
        }

        Ok(Self {
            definition,
            system_indices,
            planet_indices,
            sector_indices,
            system_sectors,
            adjacency,
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

    pub fn sector(&self, id: SectorId) -> Option<&SectorDefinition> {
        let index = *self.sector_indices.get(&id)?;
        self.definition.sectors.get(index)
    }

    pub fn sector_for_system(&self, id: SystemId) -> Option<&SectorDefinition> {
        self.system_sectors
            .get(&id)
            .and_then(|sector_id| self.sector(*sector_id))
    }

    pub fn neighboring_systems(&self, id: SystemId) -> Vec<SystemId> {
        self.adjacency.get(&id).cloned().unwrap_or_default()
    }

    pub fn route_exists(&self, from: SystemId, to: SystemId) -> bool {
        self.adjacency
            .get(&from)
            .is_some_and(|neighbors| neighbors.binary_search(&to).is_ok())
    }

    /// Returns the deterministic shortest path by number of jumps.
    ///
    /// Both endpoints are included in the returned vector.
    pub fn shortest_path(&self, from: SystemId, to: SystemId) -> Option<Vec<SystemId>> {
        if !self.system_indices.contains_key(&from) || !self.system_indices.contains_key(&to) {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut previous = HashMap::<SystemId, SystemId>::new();

        visited.insert(from);
        queue.push_back(from);

        while let Some(current) = queue.pop_front() {
            let neighbors = self.adjacency.get(&current)?;
            for neighbor in neighbors {
                if !visited.insert(*neighbor) {
                    continue;
                }

                previous.insert(*neighbor, current);
                if *neighbor == to {
                    return reconstruct_path(from, to, &previous);
                }
                queue.push_back(*neighbor);
            }
        }

        None
    }

    pub fn hop_distance(&self, from: SystemId, to: SystemId) -> Option<u32> {
        self.shortest_path(from, to)
            .map(|path| path.len().saturating_sub(1) as u32)
    }

    pub fn all_systems_reachable_from(&self, start: SystemId) -> bool {
        if !self.system_indices.contains_key(&start) {
            return false;
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        visited.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            if let Some(neighbors) = self.adjacency.get(&current) {
                for neighbor in neighbors {
                    if visited.insert(*neighbor) {
                        queue.push_back(*neighbor);
                    }
                }
            }
        }

        visited.len() == self.definition.systems.len()
    }
}

fn reconstruct_path(
    from: SystemId,
    to: SystemId,
    previous: &HashMap<SystemId, SystemId>,
) -> Option<Vec<SystemId>> {
    let mut path = vec![to];
    let mut cursor = to;

    while cursor != from {
        cursor = *previous.get(&cursor)?;
        path.push(cursor);
    }

    path.reverse();
    Some(path)
}

#[cfg(test)]
mod tests {
    use galactic_domain::{PlanetId, Route, SectorId, SystemId, UniverseConfig};

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
        let sector = repository
            .sector_for_system(home_system_id)
            .expect("home system sector is indexed");
        assert_eq!(repository.sector(sector.id), Some(sector));
    }

    #[test]
    fn regenerated_repository_matches_the_reference_universe() {
        let left = UniverseRepository::generate(UniverseConfig::mvp());
        let right = UniverseRepository::generate(UniverseConfig::mvp());

        assert_eq!(left.definition(), right.definition());
    }

    #[test]
    fn all_mvp_systems_are_reachable_from_home() {
        let repository = UniverseRepository::generate(UniverseConfig::mvp());

        assert!(repository.all_systems_reachable_from(SystemId::from_index(0)));
    }

    #[test]
    fn shortest_path_is_valid_and_deterministic() {
        let repository = UniverseRepository::generate(UniverseConfig::mvp());
        let from = SystemId::from_index(0);
        let to = SystemId::from_index(15);

        let first = repository
            .shortest_path(from, to)
            .expect("the connected MVP graph has a path");
        let second = repository
            .shortest_path(from, to)
            .expect("the same graph has the same path");

        assert_eq!(first, second);
        assert_eq!(first.first(), Some(&from));
        assert_eq!(first.last(), Some(&to));
        assert!(
            first
                .windows(2)
                .all(|edge| repository.route_exists(edge[0], edge[1]))
        );
        assert_eq!(
            repository.hop_distance(from, to),
            Some(first.len().saturating_sub(1) as u32)
        );
    }

    #[test]
    fn duplicate_routes_are_rejected() {
        let mut definition = generate_universe(UniverseConfig::mvp());
        let duplicate = *definition.routes.first().expect("MVP routes exist");
        definition
            .routes
            .push(Route::new(duplicate.to, duplicate.from));

        assert!(matches!(
            UniverseRepository::new(definition),
            Err(UniverseIndexError::DuplicateRoute(route)) if route == duplicate
        ));
    }

    #[test]
    fn duplicate_sector_membership_is_rejected() {
        let mut definition = generate_universe(UniverseConfig::mvp());
        let system_id = definition.sectors[0].systems[0];
        definition.sectors[1].systems.push(system_id);

        assert!(matches!(
            UniverseRepository::new(definition),
            Err(UniverseIndexError::DuplicateSectorMembership(found)) if found == system_id
        ));
    }

    #[test]
    fn missing_sector_gateway_is_rejected() {
        let mut definition = generate_universe(UniverseConfig::mvp());
        let (sector_index, gateway) = definition
            .sectors
            .iter()
            .enumerate()
            .find_map(|(index, sector)| {
                sector
                    .gateway_routes
                    .first()
                    .copied()
                    .map(|route| (index, route))
            })
            .expect("MVP sectors include gateway routes");
        let sector_id = definition.sectors[sector_index].id;
        definition.sectors[sector_index]
            .gateway_routes
            .retain(|route| *route != gateway);

        assert!(matches!(
            UniverseRepository::new(definition),
            Err(UniverseIndexError::MissingSectorGateway {
                sector_id: found_sector,
                route: found_route,
            }) if found_sector == sector_id && found_route == gateway
        ));
    }

    #[test]
    fn duplicate_sector_ids_are_rejected() {
        let mut definition = generate_universe(UniverseConfig::mvp());
        definition.sectors[1].id = SectorId::from_index(0);

        assert!(matches!(
            UniverseRepository::new(definition),
            Err(UniverseIndexError::DuplicateSector(found))
                if found == SectorId::from_index(0)
        ));
    }
}

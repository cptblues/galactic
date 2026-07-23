use bevy::prelude::*;

use crate::data::{GalaxyData, PlanetKind, SelectableId};
use crate::strategic::StrategicGalaxyData;

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub id: SelectableId,
    pub label: String,
    pub path: String,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub results: Vec<SearchResult>,
}

pub fn search_galaxy(
    galaxy: &GalaxyData,
    strategic: &StrategicGalaxyData,
    query: &str,
) -> Vec<SearchResult> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    for faction in &strategic.factions {
        if faction.name.to_lowercase().contains(&query) {
            results.push(SearchResult {
                id: SelectableId::System(faction.capital),
                label: faction.name.clone(),
                path: format!("Empire > {}", faction.disposition.label()),
            });
        }
    }
    for system in &galaxy.systems {
        let sector = strategic
            .system_sector(system.id)
            .map(|sector| sector.name.as_str())
            .unwrap_or("Secteur inconnu");
        let state = strategic.system_states.get(&system.id);
        let tag_match = (query == "habitable" && system.tags.has_habitable_world)
            || (query == "anomalie" && system.tags.anomaly_detected)
            || (query == "colonise"
                && state
                    .map(|state| state.control.is_colonized())
                    .unwrap_or(false));
        if system.name.to_lowercase().contains(&query) || tag_match {
            results.push(SearchResult {
                id: SelectableId::System(system.id),
                label: system.name.clone(),
                path: format!("Galaxie > {sector}"),
            });
        }
        for planet in &system.planets {
            let kind_match = planet.kind.label().to_lowercase().contains(&query)
                || (query == "oceanique" && planet.kind == PlanetKind::Ocean);
            if planet.name.to_lowercase().contains(&query) || kind_match {
                results.push(SearchResult {
                    id: SelectableId::Planet(system.id, planet.id),
                    label: planet.name.clone(),
                    path: format!("Galaxie > {sector} > {}", system.name),
                });
            }
            for moon in &planet.moons {
                if moon.name.to_lowercase().contains(&query) {
                    results.push(SearchResult {
                        id: SelectableId::Moon(system.id, planet.id, moon.id),
                        label: moon.name.clone(),
                        path: format!("Galaxie > {sector} > {} > {}", system.name, planet.name),
                    });
                }
            }
        }
    }
    results.truncate(10);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GalaxyConfig;
    use crate::generation::galaxy::generate_galaxy;
    use crate::strategic::generate_strategic_galaxy;

    #[test]
    fn search_finds_oceanic_planets() {
        let config = GalaxyConfig::default();
        let galaxy = generate_galaxy(&config);
        let strategic = generate_strategic_galaxy(&galaxy, &config);
        let results = search_galaxy(&galaxy, &strategic, "oceanique");
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .any(|result| matches!(result.id, SelectableId::Planet(_, _)))
        );
    }
}

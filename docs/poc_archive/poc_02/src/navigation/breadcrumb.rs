use crate::data::{GalaxyData, SelectableId};
use crate::strategic::StrategicGalaxyData;

pub fn breadcrumb(
    galaxy: &GalaxyData,
    strategic: &StrategicGalaxyData,
    selected: Option<SelectableId>,
) -> String {
    let Some(selected) = selected else {
        return "Galaxie".to_string();
    };
    let system_id = selected.system_id();
    let sector = strategic
        .system_sector(system_id)
        .map(|sector| sector.name.as_str())
        .unwrap_or("Secteur inconnu");
    let Some(system) = galaxy.find_system(system_id) else {
        return format!("Galaxie > {sector}");
    };
    match selected {
        SelectableId::System(_) | SelectableId::Star(_) => {
            format!("Galaxie > {sector} > {}", system.name)
        }
        SelectableId::Planet(_, planet_id) => {
            let planet = system
                .planets
                .iter()
                .find(|planet| planet.id == planet_id)
                .map(|planet| planet.name.as_str())
                .unwrap_or("Planete");
            format!("Galaxie > {sector} > {} > {planet}", system.name)
        }
        SelectableId::Moon(_, planet_id, moon_id) => {
            let Some(planet) = system.planets.iter().find(|planet| planet.id == planet_id) else {
                return format!("Galaxie > {sector} > {}", system.name);
            };
            let moon = planet
                .moons
                .iter()
                .find(|moon| moon.id == moon_id)
                .map(|moon| moon.name.as_str())
                .unwrap_or("Lune");
            format!(
                "Galaxie > {sector} > {} > {} > {moon}",
                system.name, planet.name
            )
        }
    }
}

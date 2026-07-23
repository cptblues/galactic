use crate::data::{GalaxyData, SelectableId};
use crate::strategic::{MissionState, StrategicGalaxyData};

pub fn inspector_text(
    galaxy: &GalaxyData,
    strategic: &StrategicGalaxyData,
    selected: Option<SelectableId>,
    mission: &MissionState,
) -> String {
    let Some(selected) = selected else {
        return format!(
            "Inspecteur\n\nAucun objet selectionne\n\n{}",
            mission_block(mission)
        );
    };

    match selected {
        SelectableId::System(system_id) | SelectableId::Star(system_id) => {
            let Some(system) = galaxy.find_system(system_id) else {
                return "Inspecteur\n\nSysteme introuvable".to_string();
            };
            let state = strategic.system_states.get(&system.id);
            let sector = strategic
                .system_sector(system.id)
                .map(|sector| sector.name.as_str())
                .unwrap_or("Inconnu");
            let faction = strategic
                .controlling_faction(system.id)
                .map(|faction| format!("{} ({})", faction.name, faction.disposition.label()))
                .unwrap_or_else(|| "Aucun".to_string());
            let route_distance = strategic
                .friendly_route_distances
                .get(&system.id)
                .map(|distance| distance.to_string())
                .unwrap_or_else(|| "-".to_string());
            let exploration = state
                .map(|state| state.exploration.label())
                .unwrap_or("Inconnu");
            let control = state
                .map(|state| state.control.label())
                .unwrap_or("Inconnu");
            let alerts = state.map(|state| state.alerts.len()).unwrap_or(0);
            let alert_labels = state
                .map(|state| {
                    state
                        .alerts
                        .iter()
                        .filter_map(|id| strategic.alerts.iter().find(|alert| alert.id == *id))
                        .map(|alert| format!("{} {}", alert.severity.label(), alert.kind.label()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .filter(|labels| !labels.is_empty())
                .unwrap_or_else(|| "Aucune".to_string());
            format!(
                "Inspecteur systeme\n\nNom: {}\nId: {}\nSecteur: {}\nEmpire: {}\nExploration: {}\nControle: {}\nDistance alliee: {}\nAlertes: {} ({})\nPosition: {:.1}, {:.1}, {:.1}\nEtoile: {}\nTemperature: {} K\nLuminosite: {:.2}\nPlanetes: {}\nLunes: {}\nCeinture: {}\nHabitable: {}\nMineraux: {}\nAnomalie: {}\n\nDouble-clic ou Entree pour ouvrir\nK pour afficher chemin\n\n{}",
                system.name,
                system.id.0,
                sector,
                faction,
                exploration,
                control,
                route_distance,
                alerts,
                alert_labels,
                system.position.x,
                system.position.y,
                system.position.z,
                system.star.class.label(),
                system.star.temperature_kelvin,
                system.star.luminosity,
                system.planets.len(),
                system.moon_count(),
                yes_no(system.asteroid_belt.is_some()),
                yes_no(system.tags.has_habitable_world),
                yes_no(system.tags.mineral_rich),
                yes_no(system.tags.anomaly_detected),
                mission_block(mission)
            )
        }
        SelectableId::Planet(system_id, planet_id) => {
            let Some(system) = galaxy.find_system(system_id) else {
                return "Inspecteur\n\nSysteme introuvable".to_string();
            };
            let Some(planet) = system.planets.iter().find(|planet| planet.id == planet_id) else {
                return "Inspecteur\n\nPlanete introuvable".to_string();
            };
            let route_distance = strategic
                .friendly_route_distances
                .get(&system_id)
                .map(|distance| distance.to_string())
                .unwrap_or_else(|| "-".to_string());
            format!(
                "Inspecteur planete\n\nNom: {}\nType: {}\nRayon visuel: {:.2}\nRayon orbital: {:.1}\nLunes: {}\nHabitabilite: {}%\nDistance alliee systeme: {}\n\nF pour focaliser\nY pour valider pendant la mission\n\n{}",
                planet.name,
                planet.kind.label(),
                planet.visual_radius,
                planet.orbit_radius,
                planet.moons.len(),
                planet.habitability,
                route_distance,
                mission_block(mission)
            )
        }
        SelectableId::Moon(system_id, planet_id, moon_id) => {
            let Some(system) = galaxy.find_system(system_id) else {
                return "Inspecteur\n\nSysteme introuvable".to_string();
            };
            let Some(planet) = system.planets.iter().find(|planet| planet.id == planet_id) else {
                return "Inspecteur\n\nPlanete parente introuvable".to_string();
            };
            let Some(moon) = planet.moons.iter().find(|moon| moon.id == moon_id) else {
                return "Inspecteur\n\nLune introuvable".to_string();
            };
            format!(
                "Inspecteur lune\n\nNom: {}\nPlanete: {}\nRayon: {:.2}\nRayon orbital: {:.1}\n\nF pour focaliser\n\n{}",
                moon.name,
                planet.name,
                moon.visual_radius,
                moon.orbit_radius,
                mission_block(mission)
            )
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "oui" } else { "non" }
}

fn mission_block(mission: &MissionState) -> String {
    if !mission.active && mission.result.is_none() {
        return "Mission: M pour lancer le test de navigation".to_string();
    }
    format!(
        "Mission: {}\n{}",
        if mission.completed {
            "terminee"
        } else if mission.active {
            "active"
        } else {
            "inactive"
        },
        mission.result.as_deref().unwrap_or("")
    )
}

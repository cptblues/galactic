use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

use crate::data::{
    AlertId, FactionId, FleetId, GalaxyConfig, GalaxyData, PlanetKind, SectorId, SystemId,
};
use crate::strategic::{
    AlertSeverity, ControlState, ExplorationState, FactionColor, FactionData, FactionDisposition,
    FactionInfluence, FleetData, FleetImportance, MapAlertData, MapAlertKind, MissionTarget,
    RouteKey, RouteKind, SectorData, SystemStrategicState, bfs_distances, build_adjacency,
    route_key,
};

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct StrategicGalaxyData {
    pub factions: Vec<FactionData>,
    pub sectors: Vec<SectorData>,
    pub system_states: HashMap<SystemId, SystemStrategicState>,
    pub alerts: Vec<MapAlertData>,
    pub fleets: Vec<FleetData>,
    pub validation_target: Option<MissionTarget>,
    pub route_kinds: HashMap<RouteKey, RouteKind>,
    pub friendly_route_distances: HashMap<SystemId, u32>,
    pub origin: Option<SystemId>,
}

impl StrategicGalaxyData {
    pub fn faction(&self, id: FactionId) -> Option<&FactionData> {
        self.factions.iter().find(|faction| faction.id == id)
    }

    pub fn system_sector(&self, id: SystemId) -> Option<&SectorData> {
        self.system_states
            .get(&id)
            .and_then(|state| self.sectors.iter().find(|sector| sector.id == state.sector))
    }

    pub fn controlling_faction(&self, system: SystemId) -> Option<&FactionData> {
        self.system_states
            .get(&system)
            .and_then(|state| state.control.controlling_faction())
            .and_then(|id| self.faction(id))
    }

    pub fn is_hostile_system(&self, system: SystemId) -> bool {
        self.controlling_faction(system)
            .map(|faction| faction.disposition.is_hostile())
            .unwrap_or(false)
    }

    pub fn friendly_systems(&self) -> Vec<SystemId> {
        self.system_states
            .iter()
            .filter_map(|(system, state)| {
                let faction = state
                    .control
                    .controlling_faction()
                    .and_then(|id| self.faction(id))?;
                faction.disposition.is_friendly().then_some(*system)
            })
            .collect()
    }
}

pub fn generate_strategic_galaxy(
    galaxy: &GalaxyData,
    config: &GalaxyConfig,
) -> StrategicGalaxyData {
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed ^ 0x570A_7E61_C5A7_E020);
    let factions = generate_factions(galaxy);
    let sectors = generate_sectors(galaxy, 12);
    let sector_by_system = sectors
        .iter()
        .flat_map(|sector| {
            sector
                .systems
                .iter()
                .map(move |system| (*system, sector.id))
        })
        .collect::<HashMap<_, _>>();
    let capital_factions = factions
        .iter()
        .map(|faction| (faction.capital, faction.id))
        .collect::<HashMap<_, _>>();
    let mut system_states = generate_system_states(
        galaxy,
        &mut rng,
        &factions,
        &sector_by_system,
        &capital_factions,
    );
    let alerts = generate_alerts(galaxy, &mut rng, &mut system_states);
    let route_kinds = classify_routes(galaxy, &factions, &sector_by_system);
    let adjacency = build_adjacency(galaxy);
    let friendly_sources = friendly_sources(&factions, &system_states);
    let friendly_route_distances = bfs_distances(&adjacency, &friendly_sources);
    let validation_target =
        find_validation_target(galaxy, &system_states, &friendly_route_distances);
    let fleets = generate_fleets(&mut rng, &factions, &adjacency);

    StrategicGalaxyData {
        factions,
        sectors,
        system_states,
        alerts,
        fleets,
        validation_target,
        route_kinds,
        friendly_route_distances,
        origin: Some(SystemId(0)),
    }
}

fn generate_factions(galaxy: &GalaxyData) -> Vec<FactionData> {
    let count = galaxy.systems.len().max(5);
    let capital_indices = [0, 1, count / 4, count / 2, count * 3 / 4];
    let names = [
        "Aurora Compact",
        "Helian League",
        "Vesper Combine",
        "Orion Pact",
        "Crimson Reach",
    ];
    let dispositions = [
        FactionDisposition::Player,
        FactionDisposition::Allied,
        FactionDisposition::Neutral,
        FactionDisposition::Rival,
        FactionDisposition::Hostile,
    ];
    let colors = [
        FactionColor::new(0.18, 0.74, 1.0),
        FactionColor::new(0.16, 0.88, 0.46),
        FactionColor::new(0.86, 0.78, 0.22),
        FactionColor::new(0.92, 0.42, 0.18),
        FactionColor::new(0.9, 0.12, 0.3),
    ];
    let symbols = ['A', 'H', 'V', 'O', 'C'];
    let mut used = HashSet::new();

    capital_indices
        .iter()
        .enumerate()
        .map(|(index, capital_index)| {
            let mut capital = galaxy
                .systems
                .get(*capital_index)
                .map(|system| system.id)
                .unwrap_or(SystemId(index as u32));
            while used.contains(&capital) {
                capital.0 = (capital.0 + 1) % count as u32;
            }
            used.insert(capital);
            FactionData {
                id: FactionId(index as u32),
                name: names[index].to_string(),
                capital,
                disposition: dispositions[index],
                ui_color: colors[index],
                symbol: symbols[index],
            }
        })
        .collect()
}

fn generate_sectors(galaxy: &GalaxyData, sector_count: usize) -> Vec<SectorData> {
    let names = [
        "Orion", "Lyra", "Vega", "Nadir", "Helix", "Praxis", "Korus", "Aster", "Cerulean",
        "Velorum", "Drax", "Eos",
    ];
    let mut systems_by_sector = vec![Vec::<SystemId>::new(); sector_count];
    let mut sums = vec![Vec3::ZERO; sector_count];

    for system in &galaxy.systems {
        let angle = system.position.z.atan2(system.position.x).rem_euclid(TAU);
        let sector_index = ((angle / TAU) * sector_count as f32).floor() as usize % sector_count;
        systems_by_sector[sector_index].push(system.id);
        sums[sector_index] += system.position;
    }

    systems_by_sector
        .into_iter()
        .enumerate()
        .map(|(index, systems)| {
            let center = if systems.is_empty() {
                Vec3::ZERO
            } else {
                sums[index] / systems.len() as f32
            };
            SectorData {
                id: SectorId(index as u32),
                name: names[index % names.len()].to_string(),
                center,
                systems,
            }
        })
        .collect()
}

fn generate_system_states(
    galaxy: &GalaxyData,
    rng: &mut ChaCha8Rng,
    factions: &[FactionData],
    sector_by_system: &HashMap<SystemId, SectorId>,
    capital_factions: &HashMap<SystemId, FactionId>,
) -> HashMap<SystemId, SystemStrategicState> {
    let mut states = HashMap::new();
    for system in &galaxy.systems {
        let sector = sector_by_system
            .get(&system.id)
            .copied()
            .unwrap_or_default();
        let nearest = nearest_faction(system.position, galaxy, factions);
        let influence_strength = nearest
            .map(|(_, distance)| (1.0 - distance / 95.0).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let mut influence = nearest
            .map(|(faction, _)| {
                vec![FactionInfluence {
                    faction: faction.id,
                    strength: influence_strength,
                }]
            })
            .unwrap_or_default();
        influence.sort_by(|a, b| b.strength.total_cmp(&a.strength));

        let control = if let Some(faction) = capital_factions.get(&system.id).copied() {
            ControlState::Capital(faction)
        } else if system.id == SystemId(2) {
            ControlState::Unclaimed
        } else if let Some((faction, distance)) = nearest {
            let roll = rng.random_range(0.0..1.0);
            if distance < 20.0 && roll < 0.72 {
                ControlState::Colonized(faction.id)
            } else if distance < 32.0 && roll < 0.48 {
                ControlState::Outpost(faction.id)
            } else if distance < 42.0 && roll < 0.05 {
                ControlState::Contested(
                    faction.id,
                    factions[(faction.id.0 as usize + 1) % factions.len()].id,
                )
            } else {
                ControlState::Unclaimed
            }
        } else {
            ControlState::Unclaimed
        };

        let exploration = if matches!(control, ControlState::Capital(_)) || system.id == SystemId(2)
        {
            ExplorationState::Surveyed
        } else {
            match rng.random_range(0..100) {
                0..=13 => ExplorationState::Unknown,
                14..=35 => ExplorationState::Detected,
                36..=66 => ExplorationState::Scanned,
                _ => ExplorationState::Surveyed,
            }
        };

        states.insert(
            system.id,
            SystemStrategicState {
                sector,
                exploration,
                control,
                influence,
                alerts: Vec::new(),
            },
        );
    }
    states
}

fn nearest_faction<'a>(
    position: Vec3,
    galaxy: &'a GalaxyData,
    factions: &'a [FactionData],
) -> Option<(&'a FactionData, f32)> {
    factions
        .iter()
        .filter_map(|faction| {
            let capital = galaxy.find_system(faction.capital)?;
            Some((faction, capital.position.distance(position)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn generate_alerts(
    galaxy: &GalaxyData,
    rng: &mut ChaCha8Rng,
    states: &mut HashMap<SystemId, SystemStrategicState>,
) -> Vec<MapAlertData> {
    let mut alerts = Vec::new();
    for system in &galaxy.systems {
        let from_tag = system.tags.anomaly_detected;
        let roll = rng.random_range(0.0..1.0);
        if !from_tag && roll > 0.055 {
            continue;
        }
        let kind = if from_tag {
            MapAlertKind::Anomaly
        } else {
            match rng.random_range(0..5) {
                0 => MapAlertKind::HostileFleet,
                1 => MapAlertKind::DistressSignal,
                2 => MapAlertKind::BorderTension,
                3 => MapAlertKind::Opportunity,
                _ => MapAlertKind::Anomaly,
            }
        };
        let severity = match kind {
            MapAlertKind::HostileFleet | MapAlertKind::BorderTension => AlertSeverity::Warning,
            MapAlertKind::Anomaly if roll < 0.018 => AlertSeverity::Critical,
            MapAlertKind::Anomaly => AlertSeverity::Info,
            _ => AlertSeverity::Info,
        };
        let id = AlertId(alerts.len() as u32);
        alerts.push(MapAlertData {
            id,
            system: system.id,
            kind,
            severity,
        });
        if let Some(state) = states.get_mut(&system.id) {
            state.alerts.push(id);
        }
    }
    alerts
}

fn classify_routes(
    galaxy: &GalaxyData,
    factions: &[FactionData],
    sector_by_system: &HashMap<SystemId, SectorId>,
) -> HashMap<RouteKey, RouteKind> {
    let capitals = factions
        .iter()
        .map(|faction| faction.capital)
        .collect::<HashSet<_>>();
    galaxy
        .routes
        .iter()
        .map(|route| {
            let crosses_sector = sector_by_system.get(&route.a) != sector_by_system.get(&route.b);
            let major = capitals.contains(&route.a)
                || capitals.contains(&route.b)
                || crosses_sector
                || (route.a.0 + route.b.0) % 17 == 0;
            (
                route_key(route.a, route.b),
                if major {
                    RouteKind::Major
                } else {
                    RouteKind::Minor
                },
            )
        })
        .collect()
}

fn friendly_sources(
    factions: &[FactionData],
    states: &HashMap<SystemId, SystemStrategicState>,
) -> Vec<SystemId> {
    states
        .iter()
        .filter_map(|(system, state)| {
            let faction_id = state.control.controlling_faction()?;
            let faction = factions.iter().find(|faction| faction.id == faction_id)?;
            faction.disposition.is_friendly().then_some(*system)
        })
        .collect()
}

fn find_validation_target(
    galaxy: &GalaxyData,
    states: &HashMap<SystemId, SystemStrategicState>,
    friendly_distances: &HashMap<SystemId, u32>,
) -> Option<MissionTarget> {
    galaxy.systems.iter().find_map(|system| {
        if system.id != SystemId(2)
            && friendly_distances
                .get(&system.id)
                .copied()
                .unwrap_or(u32::MAX)
                > 3
        {
            return None;
        }
        let state = states.get(&system.id)?;
        if state.control.is_colonized() {
            return None;
        }
        let planet = system
            .planets
            .iter()
            .find(|planet| planet.kind == PlanetKind::Ocean)?;
        Some(MissionTarget {
            system: system.id,
            planet: planet.id,
        })
    })
}

fn generate_fleets(
    rng: &mut ChaCha8Rng,
    factions: &[FactionData],
    adjacency: &HashMap<SystemId, Vec<SystemId>>,
) -> Vec<FleetData> {
    let mut fleets = Vec::new();
    for faction in factions {
        for local_index in 0..4 {
            let Some(neighbors) = adjacency.get(&faction.capital) else {
                continue;
            };
            if neighbors.is_empty() {
                continue;
            }
            let mut route = vec![faction.capital];
            route.push(neighbors[(local_index + faction.id.0 as usize) % neighbors.len()]);
            if let Some(next_neighbors) =
                adjacency.get(route.last().expect("route has destination"))
                && let Some(next) = next_neighbors
                    .iter()
                    .find(|candidate| **candidate != faction.capital)
            {
                route.push(*next);
            }
            fleets.push(FleetData {
                id: FleetId(fleets.len() as u32),
                name: format!("{} Patrol {}", faction.symbol, local_index + 1),
                faction: faction.id,
                route,
                segment_index: 0,
                progress: rng.random_range(0.0..1.0),
                importance: if local_index == 0 {
                    FleetImportance::Major
                } else {
                    FleetImportance::Minor
                },
            });
        }
    }
    fleets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::galaxy::generate_galaxy;
    use crate::strategic::validate_planet_for_mission;

    #[test]
    fn strategic_generation_is_deterministic() {
        let config = GalaxyConfig::default();
        let galaxy = generate_galaxy(&config);
        let first = generate_strategic_galaxy(&galaxy, &config);
        let second = generate_strategic_galaxy(&galaxy, &config);
        assert_eq!(first, second);
    }

    #[test]
    fn factions_have_unique_valid_capitals() {
        let config = GalaxyConfig::default();
        let galaxy = generate_galaxy(&config);
        let strategic = generate_strategic_galaxy(&galaxy, &config);
        let mut capitals = HashSet::new();
        assert_eq!(strategic.factions.len(), 5);
        for faction in &strategic.factions {
            assert!(galaxy.find_system(faction.capital).is_some());
            assert!(capitals.insert(faction.capital));
            let state = strategic.system_states.get(&faction.capital).unwrap();
            assert_eq!(state.control, ControlState::Capital(faction.id));
        }
    }

    #[test]
    fn every_system_has_a_sector() {
        let config = GalaxyConfig::default();
        let galaxy = generate_galaxy(&config);
        let strategic = generate_strategic_galaxy(&galaxy, &config);
        for system in &galaxy.systems {
            let state = strategic.system_states.get(&system.id).unwrap();
            let sector = strategic
                .sectors
                .iter()
                .find(|sector| sector.id == state.sector);
            assert!(sector.is_some());
        }
    }

    #[test]
    fn mission_target_is_valid_for_demo_seed() {
        let config = GalaxyConfig::default();
        let galaxy = generate_galaxy(&config);
        let strategic = generate_strategic_galaxy(&galaxy, &config);
        let target = strategic.validation_target.expect("mission target exists");
        assert_eq!(
            validate_planet_for_mission(&galaxy, &strategic, target),
            crate::strategic::MissionValidation::Success
        );
    }
}

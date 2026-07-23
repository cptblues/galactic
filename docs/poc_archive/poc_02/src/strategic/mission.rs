use crate::data::{PlanetId, PlanetKind, SystemId};
use crate::strategic::StrategicGalaxyData;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MissionTarget {
    pub system: SystemId,
    pub planet: PlanetId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MissionValidation {
    Success,
    Failure(Vec<&'static str>),
}

pub fn validate_planet_for_mission(
    galaxy: &crate::data::GalaxyData,
    strategic: &StrategicGalaxyData,
    target: MissionTarget,
) -> MissionValidation {
    let mut failures = Vec::new();
    let Some(system) = galaxy.find_system(target.system) else {
        return MissionValidation::Failure(vec!["systeme introuvable"]);
    };
    let Some(planet) = system
        .planets
        .iter()
        .find(|planet| planet.id == target.planet)
    else {
        return MissionValidation::Failure(vec!["planete introuvable"]);
    };
    if planet.kind != PlanetKind::Ocean {
        failures.push("la planete n'est pas oceanique");
    }
    let Some(state) = strategic.system_states.get(&target.system) else {
        failures.push("etat strategique introuvable");
        return MissionValidation::Failure(failures);
    };
    if state.control.is_colonized() {
        failures.push("le systeme est deja colonise ou controle");
    }
    if strategic.is_hostile_system(target.system) {
        failures.push("le systeme est en territoire hostile");
    }
    if strategic
        .friendly_route_distances
        .get(&target.system)
        .copied()
        .is_none_or(|distance| distance > 3)
    {
        failures.push("le systeme est a plus de trois routes d'un allie");
    }
    if failures.is_empty() {
        MissionValidation::Success
    } else {
        MissionValidation::Failure(failures)
    }
}

#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct MissionState {
    pub active: bool,
    pub completed: bool,
    pub result: Option<String>,
}

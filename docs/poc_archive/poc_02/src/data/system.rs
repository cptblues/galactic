use bevy::prelude::*;

use crate::data::body::{AsteroidBeltData, PlanetData, StarData};
use crate::data::ids::SystemId;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemTags {
    pub has_habitable_world: bool,
    pub mineral_rich: bool,
    pub anomaly_detected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StarSystemData {
    pub id: SystemId,
    pub name: String,
    pub position: Vec3,
    pub star: StarData,
    pub planets: Vec<PlanetData>,
    pub asteroid_belt: Option<AsteroidBeltData>,
    pub tags: SystemTags,
}

impl StarSystemData {
    pub fn moon_count(&self) -> usize {
        self.planets.iter().map(|planet| planet.moons.len()).sum()
    }
}

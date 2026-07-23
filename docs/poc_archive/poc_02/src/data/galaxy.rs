use bevy::prelude::*;

use crate::data::ids::SystemId;
use crate::data::system::StarSystemData;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GalaxyRoute {
    pub a: SystemId,
    pub b: SystemId,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct GalaxyData {
    pub seed: u64,
    pub systems: Vec<StarSystemData>,
    pub routes: Vec<GalaxyRoute>,
}

impl GalaxyData {
    pub fn find_system(&self, id: SystemId) -> Option<&StarSystemData> {
        self.systems.iter().find(|system| system.id == id)
    }
}

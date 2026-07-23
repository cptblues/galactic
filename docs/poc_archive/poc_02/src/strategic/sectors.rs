use bevy::prelude::*;

use crate::data::{SectorId, SystemId};

#[derive(Clone, Debug, PartialEq)]
pub struct SectorData {
    pub id: SectorId,
    pub name: String,
    pub center: Vec3,
    pub systems: Vec<SystemId>,
}

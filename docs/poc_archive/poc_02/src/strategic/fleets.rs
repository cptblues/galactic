use crate::data::{FactionId, FleetId, SystemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FleetImportance {
    Minor,
    Major,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FleetData {
    pub id: FleetId,
    pub name: String,
    pub faction: FactionId,
    pub route: Vec<SystemId>,
    pub segment_index: usize,
    pub progress: f32,
    pub importance: FleetImportance,
}

use crate::data::{AlertId, FactionId, SectorId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ExplorationState {
    Unknown,
    Detected,
    Scanned,
    Surveyed,
}

impl ExplorationState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Inconnu",
            Self::Detected => "Detecte",
            Self::Scanned => "Scanne",
            Self::Surveyed => "Cartographie",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ControlState {
    Unclaimed,
    Outpost(FactionId),
    Colonized(FactionId),
    Capital(FactionId),
    Contested(FactionId, FactionId),
}

impl ControlState {
    pub fn controlling_faction(self) -> Option<FactionId> {
        match self {
            Self::Unclaimed => None,
            Self::Outpost(id) | Self::Colonized(id) | Self::Capital(id) => Some(id),
            Self::Contested(a, _) => Some(a),
        }
    }

    pub fn is_colonized(self) -> bool {
        !matches!(self, Self::Unclaimed)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unclaimed => "Libre",
            Self::Outpost(_) => "Avant-poste",
            Self::Colonized(_) => "Colonise",
            Self::Capital(_) => "Capitale",
            Self::Contested(_, _) => "Conteste",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FactionInfluence {
    pub faction: FactionId,
    pub strength: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemStrategicState {
    pub sector: SectorId,
    pub exploration: ExplorationState,
    pub control: ControlState,
    pub influence: Vec<FactionInfluence>,
    pub alerts: Vec<AlertId>,
}

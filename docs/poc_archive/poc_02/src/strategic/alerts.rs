use crate::data::{AlertId, SystemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MapAlertKind {
    HostileFleet,
    DistressSignal,
    Anomaly,
    BorderTension,
    Opportunity,
}

impl MapAlertKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::HostileFleet => "Flotte hostile",
            Self::DistressSignal => "Signal de detresse",
            Self::Anomaly => "Anomalie",
            Self::BorderTension => "Tension frontaliere",
            Self::Opportunity => "Opportunite",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Alerte",
            Self::Critical => "Critique",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapAlertData {
    pub id: AlertId,
    pub system: SystemId,
    pub kind: MapAlertKind,
    pub severity: AlertSeverity,
}

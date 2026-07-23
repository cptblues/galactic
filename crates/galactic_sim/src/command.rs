use std::fmt;

use galactic_domain::{PlanetId, SystemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSpeed {
    Paused,
    X1,
    X2,
    X4,
}

impl TimeSpeed {
    pub const fn multiplier(self) -> f32 {
        match self {
            Self::Paused => 0.0,
            Self::X1 => 1.0,
            Self::X2 => 2.0,
            Self::X4 => 4.0,
        }
    }
}

impl fmt::Display for TimeSpeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paused => formatter.write_str("pause"),
            Self::X1 => formatter.write_str("x1"),
            Self::X2 => formatter.write_str("x2"),
            Self::X4 => formatter.write_str("x4"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameCommand {
    TogglePause,
    SetSpeed(TimeSpeed),
    SelectSystem(SystemId),
    SelectPlanet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
    ClearSelection,
}

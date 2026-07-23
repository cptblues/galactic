use galactic_domain::{PlanetId, SystemId};

use crate::TimeSpeed;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SelectionTarget {
    #[default]
    None,
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameEvent {
    SpeedChanged(TimeSpeed),
    SelectionChanged(SelectionTarget),
    TickAdvanced {
        delta_seconds: f32,
        elapsed_seconds: f32,
    },
}

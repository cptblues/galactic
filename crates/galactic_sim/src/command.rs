use galactic_domain::{PlanetId, SystemId};

use crate::TimeSpeed;

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

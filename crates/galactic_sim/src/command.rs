use galactic_domain::{ColonyId, PlanetId, SystemId};

use crate::{BuildingKind, CraftableId, TechnologyId, TimeSpeed};

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
    QueueBuildingUpgrade {
        colony_id: ColonyId,
        kind: BuildingKind,
    },
    QueueResearch {
        technology: TechnologyId,
    },
    QueueCraft {
        colony_id: ColonyId,
        craftable: CraftableId,
    },
    /// Temporary validation command until the probe mission loop is added.
    DebugAdvanceSelectedKnowledge,
}

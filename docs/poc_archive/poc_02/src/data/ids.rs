use bevy::prelude::*;

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SystemId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PlanetId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct MoonId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FactionId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SectorId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct FleetId(pub u32);

#[derive(Component, Clone, Copy, Default, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct AlertId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SelectableId {
    System(SystemId),
    Star(SystemId),
    Planet(SystemId, PlanetId),
    Moon(SystemId, PlanetId, MoonId),
}

impl SelectableId {
    pub fn system_id(self) -> SystemId {
        match self {
            Self::System(id) | Self::Star(id) => id,
            Self::Planet(system_id, _) | Self::Moon(system_id, _, _) => system_id,
        }
    }
}

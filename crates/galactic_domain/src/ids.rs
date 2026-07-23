use std::fmt;

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }
            pub const fn raw(self) -> u64 {
                self.0
            }
            pub const fn index(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

stable_id!(UniverseId);
stable_id!(SystemId);
stable_id!(StarId);
stable_id!(PlanetId);
stable_id!(MoonId);
stable_id!(FactionId);
stable_id!(ColonyId);
stable_id!(FleetId);
stable_id!(MissionId);

impl UniverseId {
    pub const MVP: Self = Self::new(0);
}

impl SystemId {
    pub const fn from_index(index: u32) -> Self {
        Self::new(index as u64)
    }
}

impl StarId {
    pub const fn for_system(system_id: SystemId) -> Self {
        Self::new(system_id.raw())
    }
}

impl PlanetId {
    pub const fn from_system_index(system_id: SystemId, planet_index: u32) -> Self {
        Self::new((system_id.raw() << 32) | planet_index as u64)
    }

    pub const fn system_id(self) -> SystemId {
        SystemId::new(self.raw() >> 32)
    }
    pub const fn local_index(self) -> u32 {
        self.raw() as u32
    }
}

impl MoonId {
    pub const fn from_planet_index(planet_id: PlanetId, moon_index: u16) -> Self {
        Self::new((planet_id.raw() << 16) | moon_index as u64)
    }

    pub const fn local_index(self) -> u16 {
        self.raw() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchical_ids_are_stable_and_globally_distinct() {
        let system_a = SystemId::from_index(2);
        let system_b = SystemId::from_index(3);
        let planet_a0 = PlanetId::from_system_index(system_a, 0);
        let planet_a1 = PlanetId::from_system_index(system_a, 1);
        let planet_b0 = PlanetId::from_system_index(system_b, 0);

        assert_ne!(planet_a0, planet_a1);
        assert_ne!(planet_a0, planet_b0);
        assert_eq!(planet_a1.system_id(), system_a);
        assert_eq!(planet_a1.local_index(), 1);

        let moon = MoonId::from_planet_index(planet_a1, 4);
        assert_eq!(moon.local_index(), 4);
    }

    #[test]
    fn star_identity_is_derived_from_its_system() {
        let system = SystemId::from_index(7);
        assert_eq!(StarId::for_system(system).raw(), system.raw());
    }
}

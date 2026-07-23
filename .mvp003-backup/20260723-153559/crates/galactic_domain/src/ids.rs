use std::fmt;

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            pub const fn index(self) -> u32 {
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

stable_id!(SystemId);
stable_id!(PlanetId);
stable_id!(MoonId);
stable_id!(FactionId);
stable_id!(ColonyId);
stable_id!(FleetId);
stable_id!(MissionId);

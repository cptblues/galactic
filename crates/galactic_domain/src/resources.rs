use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Metal,
    Crystal,
    Fuel,
    Energy,
}

impl ResourceKind {
    pub const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Energy];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceStock {
    pub metal: i32,
    pub crystal: i32,
    pub fuel: i32,
    pub energy: i32,
}

impl ResourceStock {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);

    pub const fn new(metal: i32, crystal: i32, fuel: i32, energy: i32) -> Self {
        Self {
            metal,
            crystal,
            fuel,
            energy,
        }
    }

    pub fn can_cover(self, cost: Self) -> bool {
        self.metal >= cost.metal
            && self.crystal >= cost.crystal
            && self.fuel >= cost.fuel
            && self.energy >= cost.energy
    }
}

impl Default for ResourceStock {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Add for ResourceStock {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            metal: self.metal + other.metal,
            crystal: self.crystal + other.crystal,
            fuel: self.fuel + other.fuel,
            energy: self.energy + other.energy,
        }
    }
}

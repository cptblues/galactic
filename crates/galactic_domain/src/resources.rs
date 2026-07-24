// MVP-011: atomic stored-resource ledger and energy capacity.
use std::collections::BTreeSet;
use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Metal,
    Crystal,
    Fuel,
    /// Energy is retained as a catalog kind for compatibility, but is never
    /// stored in `ResourceStock`.
    Energy,
}

impl ResourceKind {
    pub const ALL: [Self; 4] = [Self::Metal, Self::Crystal, Self::Fuel, Self::Energy];
    pub const STORED: [Self; 3] = [Self::Metal, Self::Crystal, Self::Fuel];

    pub const fn is_stored(self) -> bool {
        !matches!(self, Self::Energy)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceStock {
    pub metal: u64,
    pub crystal: u64,
    pub fuel: u64,
}

impl ResourceStock {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(metal: u64, crystal: u64, fuel: u64) -> Self {
        Self {
            metal,
            crystal,
            fuel,
        }
    }

    pub const fn is_zero(self) -> bool {
        self.metal == 0 && self.crystal == 0 && self.fuel == 0
    }

    pub fn can_cover<T>(self, cost: T) -> bool
    where
        T: Into<ResourceCost>,
    {
        let cost = cost.into();
        self.metal >= cost.metal && self.crystal >= cost.crystal && self.fuel >= cost.fuel
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            metal: self.metal.checked_add(other.metal)?,
            crystal: self.crystal.checked_add(other.crystal)?,
            fuel: self.fuel.checked_add(other.fuel)?,
        })
    }

    pub fn checked_sub<T>(self, cost: T) -> Option<Self>
    where
        T: Into<ResourceCost>,
    {
        let cost = cost.into();
        Some(Self {
            metal: self.metal.checked_sub(cost.metal)?,
            crystal: self.crystal.checked_sub(cost.crystal)?,
            fuel: self.fuel.checked_sub(cost.fuel)?,
        })
    }
}

impl Add for ResourceStock {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self.checked_add(other)
            .expect("resource stock addition must not overflow")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceCost {
    pub metal: u64,
    pub crystal: u64,
    pub fuel: u64,
}

impl ResourceCost {
    pub const ZERO: Self = Self::new(0, 0, 0);

    pub const fn new(metal: u64, crystal: u64, fuel: u64) -> Self {
        Self {
            metal,
            crystal,
            fuel,
        }
    }

    pub const fn is_zero(self) -> bool {
        self.metal == 0 && self.crystal == 0 && self.fuel == 0
    }

    pub const fn as_stock(self) -> ResourceStock {
        ResourceStock::new(self.metal, self.crystal, self.fuel)
    }
}

impl From<ResourceStock> for ResourceCost {
    fn from(stock: ResourceStock) -> Self {
        Self::new(stock.metal, stock.crystal, stock.fuel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReservationId(u64);

impl ReservationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceReservation {
    pub id: ReservationId,
    pub cost: ResourceCost,
}

impl ResourceReservation {
    pub const fn new(id: ReservationId, cost: ResourceCost) -> Self {
        Self { id, cost }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLedger {
    stock: ResourceStock,
    reservations: Vec<ResourceReservation>,
    next_reservation_id: u64,
}

impl ResourceLedger {
    pub fn new(stock: ResourceStock) -> Self {
        Self {
            stock,
            reservations: Vec::new(),
            next_reservation_id: 1,
        }
    }

    pub fn from_parts(
        stock: ResourceStock,
        reservations: Vec<ResourceReservation>,
        next_reservation_id: u64,
    ) -> Result<Self, ResourceLedgerError> {
        let ledger = Self {
            stock,
            reservations,
            next_reservation_id,
        };
        ledger.validate()?;
        Ok(ledger)
    }

    pub const fn stock(&self) -> ResourceStock {
        self.stock
    }

    pub fn reservations(&self) -> &[ResourceReservation] {
        &self.reservations
    }

    pub const fn next_reservation_id(&self) -> u64 {
        self.next_reservation_id
    }

    pub fn reserved_total(&self) -> ResourceStock {
        self.reservations
            .iter()
            .try_fold(ResourceStock::ZERO, |total, reservation| {
                total.checked_add(reservation.cost.as_stock())
            })
            .expect("validated reservation totals must not overflow")
    }

    pub fn available(&self) -> ResourceStock {
        self.stock
            .checked_sub(self.reserved_total())
            .expect("validated reservations must be covered by stock")
    }

    pub fn credit(&mut self, amount: ResourceStock) -> Result<(), ResourceLedgerError> {
        let updated = self
            .stock
            .checked_add(amount)
            .ok_or(ResourceLedgerError::AmountOverflow)?;
        self.stock = updated;
        Ok(())
    }

    pub fn debit(&mut self, cost: ResourceCost) -> Result<(), ResourceLedgerError> {
        let available = self.available();
        if !available.can_cover(cost) {
            return Err(ResourceLedgerError::InsufficientResources {
                available,
                requested: cost,
            });
        }

        let updated = self
            .stock
            .checked_sub(cost)
            .expect("available resources already cover the debit");
        self.stock = updated;
        Ok(())
    }

    pub fn reserve(&mut self, cost: ResourceCost) -> Result<ReservationId, ResourceLedgerError> {
        if cost.is_zero() {
            return Err(ResourceLedgerError::EmptyReservation);
        }

        let available = self.available();
        if !available.can_cover(cost) {
            return Err(ResourceLedgerError::InsufficientResources {
                available,
                requested: cost,
            });
        }

        let next_id = self
            .next_reservation_id
            .checked_add(1)
            .ok_or(ResourceLedgerError::ReservationIdOverflow)?;
        let id = ReservationId::new(self.next_reservation_id);

        self.reservations.push(ResourceReservation::new(id, cost));
        self.next_reservation_id = next_id;
        Ok(id)
    }

    pub fn commit(&mut self, id: ReservationId) -> Result<ResourceCost, ResourceLedgerError> {
        let index = self
            .reservations
            .iter()
            .position(|reservation| reservation.id == id)
            .ok_or(ResourceLedgerError::UnknownReservation(id))?;
        let cost = self.reservations[index].cost;
        let updated = self
            .stock
            .checked_sub(cost)
            .expect("validated reservations are covered by stock");

        self.stock = updated;
        self.reservations.remove(index);
        Ok(cost)
    }

    pub fn release(&mut self, id: ReservationId) -> Result<ResourceCost, ResourceLedgerError> {
        let index = self
            .reservations
            .iter()
            .position(|reservation| reservation.id == id)
            .ok_or(ResourceLedgerError::UnknownReservation(id))?;
        Ok(self.reservations.remove(index).cost)
    }

    pub fn validate(&self) -> Result<(), ResourceLedgerError> {
        let mut ids = BTreeSet::new();
        let mut reserved = ResourceStock::ZERO;
        let mut highest_id = 0;

        for reservation in &self.reservations {
            if reservation.cost.is_zero() {
                return Err(ResourceLedgerError::EmptyReservation);
            }
            if !ids.insert(reservation.id) {
                return Err(ResourceLedgerError::DuplicateReservation(reservation.id));
            }
            highest_id = highest_id.max(reservation.id.value());
            reserved = reserved
                .checked_add(reservation.cost.as_stock())
                .ok_or(ResourceLedgerError::AmountOverflow)?;
        }

        if !self.stock.can_cover(reserved) {
            return Err(ResourceLedgerError::OverReserved {
                stock: self.stock,
                reserved,
            });
        }
        if !self.reservations.is_empty() && self.next_reservation_id <= highest_id {
            return Err(ResourceLedgerError::InvalidNextReservationId {
                next: self.next_reservation_id,
                highest_existing: highest_id,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLedgerError {
    EmptyReservation,
    InsufficientResources {
        available: ResourceStock,
        requested: ResourceCost,
    },
    UnknownReservation(ReservationId),
    DuplicateReservation(ReservationId),
    OverReserved {
        stock: ResourceStock,
        reserved: ResourceStock,
    },
    InvalidNextReservationId {
        next: u64,
        highest_existing: u64,
    },
    AmountOverflow,
    ReservationIdOverflow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnergyGrid {
    production: u64,
    consumption: u64,
}

impl EnergyGrid {
    pub const fn new(production: u64, consumption: u64) -> Self {
        Self {
            production,
            consumption,
        }
    }

    pub const fn production(self) -> u64 {
        self.production
    }

    pub const fn consumption(self) -> u64 {
        self.consumption
    }

    pub const fn balance(self) -> i128 {
        self.production as i128 - self.consumption as i128
    }

    pub const fn available_capacity(self) -> u64 {
        self.production.saturating_sub(self.consumption)
    }

    pub const fn is_deficit(self) -> bool {
        self.consumption > self.production
    }

    pub fn allocate(&mut self, amount: u64) -> Result<(), EnergyError> {
        let available = self.available_capacity();
        if amount > available {
            return Err(EnergyError::InsufficientCapacity {
                available,
                requested: amount,
            });
        }
        self.consumption = self
            .consumption
            .checked_add(amount)
            .ok_or(EnergyError::AmountOverflow)?;
        Ok(())
    }

    pub fn release(&mut self, amount: u64) -> Result<(), EnergyError> {
        self.consumption =
            self.consumption
                .checked_sub(amount)
                .ok_or(EnergyError::ReleaseExceedsConsumption {
                    consumption: self.consumption,
                    requested: amount,
                })?;
        Ok(())
    }

    pub fn set_production(&mut self, production: u64) {
        self.production = production;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyError {
    InsufficientCapacity { available: u64, requested: u64 },
    ReleaseExceedsConsumption { consumption: u64, requested: u64 },
    AmountOverflow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EconomicCost {
    pub resources: ResourceCost,
    /// Capacity that must be available; energy is not spent or stored.
    pub energy: u64,
}

impl EconomicCost {
    pub const fn new(resources: ResourceCost, energy: u64) -> Self {
        Self { resources, energy }
    }

    pub fn can_start(self, ledger: &ResourceLedger, grid: EnergyGrid) -> bool {
        ledger.available().can_cover(self.resources) && grid.available_capacity() >= self.energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_debit_is_atomic() {
        let initial = ResourceStock::new(100, 40, 20);
        let mut ledger = ResourceLedger::new(initial);

        let result = ledger.debit(ResourceCost::new(120, 0, 0));

        assert!(matches!(
            result,
            Err(ResourceLedgerError::InsufficientResources { .. })
        ));
        assert_eq!(ledger.stock(), initial);
        assert_eq!(ledger.available(), initial);
    }

    #[test]
    fn reservation_prevents_double_spending() {
        let mut ledger = ResourceLedger::new(ResourceStock::new(100, 50, 25));

        let id = ledger
            .reserve(ResourceCost::new(80, 20, 10))
            .expect("first reservation is funded");
        let second = ledger.reserve(ResourceCost::new(30, 10, 5));

        assert!(matches!(
            second,
            Err(ResourceLedgerError::InsufficientResources { .. })
        ));
        assert_eq!(ledger.available(), ResourceStock::new(20, 30, 15));

        ledger.commit(id).expect("reservation can commit");
        assert_eq!(ledger.stock(), ResourceStock::new(20, 30, 15));
        assert!(ledger.reservations().is_empty());
    }

    #[test]
    fn released_reservation_restores_availability() {
        let mut ledger = ResourceLedger::new(ResourceStock::new(100, 50, 25));
        let id = ledger
            .reserve(ResourceCost::new(80, 20, 10))
            .expect("reservation is funded");

        ledger.release(id).expect("reservation can release");

        assert_eq!(ledger.available(), ResourceStock::new(100, 50, 25));
        assert_eq!(ledger.stock(), ResourceStock::new(100, 50, 25));
    }

    #[test]
    fn credit_overflow_does_not_mutate_stock() {
        let initial = ResourceStock::new(u64::MAX, 0, 0);
        let mut ledger = ResourceLedger::new(initial);

        assert_eq!(
            ledger.credit(ResourceStock::new(1, 0, 0)),
            Err(ResourceLedgerError::AmountOverflow)
        );
        assert_eq!(ledger.stock(), initial);
    }

    #[test]
    fn energy_is_capacity_not_a_stock() {
        let mut grid = EnergyGrid::new(80, 30);

        grid.allocate(40).expect("capacity is available");
        assert_eq!(grid.production(), 80);
        assert_eq!(grid.consumption(), 70);
        assert_eq!(grid.balance(), 10);

        assert!(matches!(
            grid.allocate(11),
            Err(EnergyError::InsufficientCapacity {
                available: 10,
                requested: 11,
            })
        ));
        assert_eq!(grid.consumption(), 70);
    }

    #[test]
    fn configurable_cost_combines_all_resources_and_energy() {
        let ledger = ResourceLedger::new(ResourceStock::new(100, 80, 60));
        let grid = EnergyGrid::new(50, 20);
        let cost = EconomicCost::new(ResourceCost::new(90, 70, 50), 25);

        assert!(cost.can_start(&ledger, grid));
        assert!(!EconomicCost::new(ResourceCost::new(90, 70, 50), 31,).can_start(&ledger, grid));
    }
}

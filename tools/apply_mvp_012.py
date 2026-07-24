#!/usr/bin/env python3
"""
Applique MVP-012 au dépôt Galactic.

Baseline analysée :
    14e64f85ecff05b51ea9cf8106ba785f8bb8b707
    feat add stock management

Le script :
- produit Métal, Cristal et Carburant par tick stratégique ;
- applique les niveaux de bâtiments et potentiels planétaires ;
- ralentit proportionnellement la production en déficit énergétique ;
- ajoute des capacités de stockage par ressource ;
- plafonne les crédits et perd l'excédent à saturation ;
- sauvegarde les reliquats de production ;
- affiche taux, capacités et temps avant saturation ;
- conserve le catalogue configurable pour MVP-013.

Usage :
    python tools/apply_mvp_012.py --dry-run
    python tools/apply_mvp_012.py
    python tools/apply_mvp_012.py --skip-checks
    python tools/apply_mvp_012.py --root /chemin/vers/galactic

Le script est idempotent.
"""

from __future__ import annotations

import argparse
import difflib
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = (
    "14e64f85ecff05b51ea9cf8106ba785f8bb8b707"
)

RESOURCES_RS = '// MVP-012: atomic resources with capacity-aware production credits.\nuse std::collections::BTreeSet;\nuse std::ops::Add;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum ResourceKind {\n    Metal,\n    Crystal,\n    Fuel,\n    /// Energy remains a catalog kind for compatibility, but is never stored.\n    Energy,\n}\n\nimpl ResourceKind {\n    pub const ALL: [Self; 4] = [\n        Self::Metal,\n        Self::Crystal,\n        Self::Fuel,\n        Self::Energy,\n    ];\n    pub const STORED: [Self; 3] =\n        [Self::Metal, Self::Crystal, Self::Fuel];\n\n    pub const fn is_stored(self) -> bool {\n        !matches!(self, Self::Energy)\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ResourceStock {\n    pub metal: u64,\n    pub crystal: u64,\n    pub fuel: u64,\n}\n\nimpl ResourceStock {\n    pub const ZERO: Self = Self::new(0, 0, 0);\n\n    pub const fn new(\n        metal: u64,\n        crystal: u64,\n        fuel: u64,\n    ) -> Self {\n        Self {\n            metal,\n            crystal,\n            fuel,\n        }\n    }\n\n    pub const fn is_zero(self) -> bool {\n        self.metal == 0 && self.crystal == 0 && self.fuel == 0\n    }\n\n    pub fn can_cover<T>(self, cost: T) -> bool\n    where\n        T: Into<ResourceCost>,\n    {\n        let cost = cost.into();\n        self.metal >= cost.metal\n            && self.crystal >= cost.crystal\n            && self.fuel >= cost.fuel\n    }\n\n    pub const fn is_within(self, capacity: Self) -> bool {\n        self.metal <= capacity.metal\n            && self.crystal <= capacity.crystal\n            && self.fuel <= capacity.fuel\n    }\n\n    pub const fn component_min(self, other: Self) -> Self {\n        Self {\n            metal: if self.metal < other.metal {\n                self.metal\n            } else {\n                other.metal\n            },\n            crystal: if self.crystal < other.crystal {\n                self.crystal\n            } else {\n                other.crystal\n            },\n            fuel: if self.fuel < other.fuel {\n                self.fuel\n            } else {\n                other.fuel\n            },\n        }\n    }\n\n    pub const fn saturating_sub(self, other: Self) -> Self {\n        Self {\n            metal: self.metal.saturating_sub(other.metal),\n            crystal: self.crystal.saturating_sub(other.crystal),\n            fuel: self.fuel.saturating_sub(other.fuel),\n        }\n    }\n\n    pub fn checked_add(self, other: Self) -> Option<Self> {\n        Some(Self {\n            metal: self.metal.checked_add(other.metal)?,\n            crystal: self.crystal.checked_add(other.crystal)?,\n            fuel: self.fuel.checked_add(other.fuel)?,\n        })\n    }\n\n    pub fn checked_sub<T>(self, cost: T) -> Option<Self>\n    where\n        T: Into<ResourceCost>,\n    {\n        let cost = cost.into();\n        Some(Self {\n            metal: self.metal.checked_sub(cost.metal)?,\n            crystal: self.crystal.checked_sub(cost.crystal)?,\n            fuel: self.fuel.checked_sub(cost.fuel)?,\n        })\n    }\n}\n\nimpl Add for ResourceStock {\n    type Output = Self;\n\n    fn add(self, other: Self) -> Self::Output {\n        self.checked_add(other)\n            .expect("resource stock addition must not overflow")\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ResourceCost {\n    pub metal: u64,\n    pub crystal: u64,\n    pub fuel: u64,\n}\n\nimpl ResourceCost {\n    pub const ZERO: Self = Self::new(0, 0, 0);\n\n    pub const fn new(\n        metal: u64,\n        crystal: u64,\n        fuel: u64,\n    ) -> Self {\n        Self {\n            metal,\n            crystal,\n            fuel,\n        }\n    }\n\n    pub const fn is_zero(self) -> bool {\n        self.metal == 0 && self.crystal == 0 && self.fuel == 0\n    }\n\n    pub const fn as_stock(self) -> ResourceStock {\n        ResourceStock::new(self.metal, self.crystal, self.fuel)\n    }\n}\n\nimpl From<ResourceStock> for ResourceCost {\n    fn from(stock: ResourceStock) -> Self {\n        Self::new(stock.metal, stock.crystal, stock.fuel)\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct ReservationId(u64);\n\nimpl ReservationId {\n    pub const fn new(value: u64) -> Self {\n        Self(value)\n    }\n\n    pub const fn value(self) -> u64 {\n        self.0\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResourceReservation {\n    pub id: ReservationId,\n    pub cost: ResourceCost,\n}\n\nimpl ResourceReservation {\n    pub const fn new(\n        id: ReservationId,\n        cost: ResourceCost,\n    ) -> Self {\n        Self { id, cost }\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ResourceLedger {\n    stock: ResourceStock,\n    reservations: Vec<ResourceReservation>,\n    next_reservation_id: u64,\n}\n\nimpl ResourceLedger {\n    pub fn new(stock: ResourceStock) -> Self {\n        Self {\n            stock,\n            reservations: Vec::new(),\n            next_reservation_id: 1,\n        }\n    }\n\n    pub fn from_parts(\n        stock: ResourceStock,\n        reservations: Vec<ResourceReservation>,\n        next_reservation_id: u64,\n    ) -> Result<Self, ResourceLedgerError> {\n        let ledger = Self {\n            stock,\n            reservations,\n            next_reservation_id,\n        };\n        ledger.validate()?;\n        Ok(ledger)\n    }\n\n    pub const fn stock(&self) -> ResourceStock {\n        self.stock\n    }\n\n    pub fn reservations(&self) -> &[ResourceReservation] {\n        &self.reservations\n    }\n\n    pub const fn next_reservation_id(&self) -> u64 {\n        self.next_reservation_id\n    }\n\n    pub fn reserved_total(&self) -> ResourceStock {\n        self.reservations\n            .iter()\n            .try_fold(\n                ResourceStock::ZERO,\n                |total, reservation| {\n                    total.checked_add(reservation.cost.as_stock())\n                },\n            )\n            .expect(\n                "validated reservation totals must not overflow",\n            )\n    }\n\n    pub fn available(&self) -> ResourceStock {\n        self.stock\n            .checked_sub(self.reserved_total())\n            .expect(\n                "validated reservations must be covered by stock",\n            )\n    }\n\n    pub fn credit(\n        &mut self,\n        amount: ResourceStock,\n    ) -> Result<(), ResourceLedgerError> {\n        let updated = self\n            .stock\n            .checked_add(amount)\n            .ok_or(ResourceLedgerError::AmountOverflow)?;\n        self.stock = updated;\n        Ok(())\n    }\n\n    /// Credits at most the free capacity and returns the amount accepted.\n    ///\n    /// Reservations are not changed: newly produced resources immediately\n    /// increase the unreserved availability.\n    pub fn credit_capped(\n        &mut self,\n        amount: ResourceStock,\n        capacity: ResourceStock,\n    ) -> ResourceStock {\n        let headroom = capacity.saturating_sub(self.stock);\n        let credited = amount.component_min(headroom);\n        self.stock = self\n            .stock\n            .checked_add(credited)\n            .expect(\n                "a capacity-capped credit cannot overflow",\n            );\n        credited\n    }\n\n    pub fn debit(\n        &mut self,\n        cost: ResourceCost,\n    ) -> Result<(), ResourceLedgerError> {\n        let available = self.available();\n        if !available.can_cover(cost) {\n            return Err(\n                ResourceLedgerError::InsufficientResources {\n                    available,\n                    requested: cost,\n                },\n            );\n        }\n\n        let updated = self\n            .stock\n            .checked_sub(cost)\n            .expect(\n                "available resources already cover the debit",\n            );\n        self.stock = updated;\n        Ok(())\n    }\n\n    pub fn reserve(\n        &mut self,\n        cost: ResourceCost,\n    ) -> Result<ReservationId, ResourceLedgerError> {\n        if cost.is_zero() {\n            return Err(ResourceLedgerError::EmptyReservation);\n        }\n\n        let available = self.available();\n        if !available.can_cover(cost) {\n            return Err(\n                ResourceLedgerError::InsufficientResources {\n                    available,\n                    requested: cost,\n                },\n            );\n        }\n\n        let next_id = self\n            .next_reservation_id\n            .checked_add(1)\n            .ok_or(\n                ResourceLedgerError::ReservationIdOverflow,\n            )?;\n        let id = ReservationId::new(self.next_reservation_id);\n\n        self.reservations\n            .push(ResourceReservation::new(id, cost));\n        self.next_reservation_id = next_id;\n        Ok(id)\n    }\n\n    pub fn commit(\n        &mut self,\n        id: ReservationId,\n    ) -> Result<ResourceCost, ResourceLedgerError> {\n        let index = self\n            .reservations\n            .iter()\n            .position(|reservation| reservation.id == id)\n            .ok_or(\n                ResourceLedgerError::UnknownReservation(id),\n            )?;\n        let cost = self.reservations[index].cost;\n        let updated = self\n            .stock\n            .checked_sub(cost)\n            .expect(\n                "validated reservations are covered by stock",\n            );\n\n        self.stock = updated;\n        self.reservations.remove(index);\n        Ok(cost)\n    }\n\n    pub fn release(\n        &mut self,\n        id: ReservationId,\n    ) -> Result<ResourceCost, ResourceLedgerError> {\n        let index = self\n            .reservations\n            .iter()\n            .position(|reservation| reservation.id == id)\n            .ok_or(\n                ResourceLedgerError::UnknownReservation(id),\n            )?;\n        Ok(self.reservations.remove(index).cost)\n    }\n\n    pub fn validate(&self) -> Result<(), ResourceLedgerError> {\n        let mut ids = BTreeSet::new();\n        let mut reserved = ResourceStock::ZERO;\n        let mut highest_id = 0;\n\n        for reservation in &self.reservations {\n            if reservation.cost.is_zero() {\n                return Err(\n                    ResourceLedgerError::EmptyReservation,\n                );\n            }\n            if !ids.insert(reservation.id) {\n                return Err(\n                    ResourceLedgerError::DuplicateReservation(\n                        reservation.id,\n                    ),\n                );\n            }\n            highest_id = highest_id.max(reservation.id.value());\n            reserved = reserved\n                .checked_add(reservation.cost.as_stock())\n                .ok_or(ResourceLedgerError::AmountOverflow)?;\n        }\n\n        if !self.stock.can_cover(reserved) {\n            return Err(ResourceLedgerError::OverReserved {\n                stock: self.stock,\n                reserved,\n            });\n        }\n        if !self.reservations.is_empty()\n            && self.next_reservation_id <= highest_id\n        {\n            return Err(\n                ResourceLedgerError::InvalidNextReservationId {\n                    next: self.next_reservation_id,\n                    highest_existing: highest_id,\n                },\n            );\n        }\n\n        Ok(())\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ResourceLedgerError {\n    EmptyReservation,\n    InsufficientResources {\n        available: ResourceStock,\n        requested: ResourceCost,\n    },\n    UnknownReservation(ReservationId),\n    DuplicateReservation(ReservationId),\n    OverReserved {\n        stock: ResourceStock,\n        reserved: ResourceStock,\n    },\n    InvalidNextReservationId {\n        next: u64,\n        highest_existing: u64,\n    },\n    AmountOverflow,\n    ReservationIdOverflow,\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct EnergyGrid {\n    production: u64,\n    consumption: u64,\n}\n\nimpl EnergyGrid {\n    pub const fn new(\n        production: u64,\n        consumption: u64,\n    ) -> Self {\n        Self {\n            production,\n            consumption,\n        }\n    }\n\n    pub const fn production(self) -> u64 {\n        self.production\n    }\n\n    pub const fn consumption(self) -> u64 {\n        self.consumption\n    }\n\n    pub const fn balance(self) -> i128 {\n        self.production as i128 - self.consumption as i128\n    }\n\n    pub const fn available_capacity(self) -> u64 {\n        self.production.saturating_sub(self.consumption)\n    }\n\n    pub const fn is_deficit(self) -> bool {\n        self.consumption > self.production\n    }\n\n    pub fn allocate(\n        &mut self,\n        amount: u64,\n    ) -> Result<(), EnergyError> {\n        let available = self.available_capacity();\n        if amount > available {\n            return Err(EnergyError::InsufficientCapacity {\n                available,\n                requested: amount,\n            });\n        }\n        self.consumption = self\n            .consumption\n            .checked_add(amount)\n            .ok_or(EnergyError::AmountOverflow)?;\n        Ok(())\n    }\n\n    pub fn release(\n        &mut self,\n        amount: u64,\n    ) -> Result<(), EnergyError> {\n        self.consumption = self\n            .consumption\n            .checked_sub(amount)\n            .ok_or(\n                EnergyError::ReleaseExceedsConsumption {\n                    consumption: self.consumption,\n                    requested: amount,\n                },\n            )?;\n        Ok(())\n    }\n\n    pub fn set_production(&mut self, production: u64) {\n        self.production = production;\n    }\n\n    pub fn set_consumption(&mut self, consumption: u64) {\n        self.consumption = consumption;\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum EnergyError {\n    InsufficientCapacity {\n        available: u64,\n        requested: u64,\n    },\n    ReleaseExceedsConsumption {\n        consumption: u64,\n        requested: u64,\n    },\n    AmountOverflow,\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct EconomicCost {\n    pub resources: ResourceCost,\n    /// Capacity that must be available; energy is not spent or stored.\n    pub energy: u64,\n}\n\nimpl EconomicCost {\n    pub const fn new(\n        resources: ResourceCost,\n        energy: u64,\n    ) -> Self {\n        Self { resources, energy }\n    }\n\n    pub fn can_start(\n        self,\n        ledger: &ResourceLedger,\n        grid: EnergyGrid,\n    ) -> bool {\n        ledger.available().can_cover(self.resources)\n            && grid.available_capacity() >= self.energy\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn insufficient_debit_is_atomic() {\n        let initial = ResourceStock::new(100, 40, 20);\n        let mut ledger = ResourceLedger::new(initial);\n\n        let result =\n            ledger.debit(ResourceCost::new(120, 0, 0));\n\n        assert!(matches!(\n            result,\n            Err(ResourceLedgerError::InsufficientResources {\n                ..\n            })\n        ));\n        assert_eq!(ledger.stock(), initial);\n        assert_eq!(ledger.available(), initial);\n    }\n\n    #[test]\n    fn reservation_prevents_double_spending() {\n        let mut ledger =\n            ResourceLedger::new(ResourceStock::new(100, 50, 25));\n\n        let id = ledger\n            .reserve(ResourceCost::new(80, 20, 10))\n            .expect("first reservation is funded");\n        let second =\n            ledger.reserve(ResourceCost::new(30, 10, 5));\n\n        assert!(matches!(\n            second,\n            Err(ResourceLedgerError::InsufficientResources {\n                ..\n            })\n        ));\n        assert_eq!(\n            ledger.available(),\n            ResourceStock::new(20, 30, 15)\n        );\n\n        ledger.commit(id).expect("reservation can commit");\n        assert_eq!(\n            ledger.stock(),\n            ResourceStock::new(20, 30, 15)\n        );\n        assert!(ledger.reservations().is_empty());\n    }\n\n    #[test]\n    fn released_reservation_restores_availability() {\n        let mut ledger =\n            ResourceLedger::new(ResourceStock::new(100, 50, 25));\n        let id = ledger\n            .reserve(ResourceCost::new(80, 20, 10))\n            .expect("reservation is funded");\n\n        ledger.release(id).expect("reservation can release");\n\n        assert_eq!(\n            ledger.available(),\n            ResourceStock::new(100, 50, 25)\n        );\n        assert_eq!(\n            ledger.stock(),\n            ResourceStock::new(100, 50, 25)\n        );\n    }\n\n    #[test]\n    fn capped_credit_never_exceeds_capacity() {\n        let mut ledger =\n            ResourceLedger::new(ResourceStock::new(95, 30, 8));\n        let capacity = ResourceStock::new(100, 40, 10);\n\n        let credited = ledger.credit_capped(\n            ResourceStock::new(20, 4, 9),\n            capacity,\n        );\n\n        assert_eq!(\n            credited,\n            ResourceStock::new(5, 4, 2)\n        );\n        assert_eq!(\n            ledger.stock(),\n            ResourceStock::new(100, 34, 10)\n        );\n        assert!(ledger.stock().is_within(capacity));\n    }\n\n    #[test]\n    fn credit_overflow_does_not_mutate_stock() {\n        let initial = ResourceStock::new(u64::MAX, 0, 0);\n        let mut ledger = ResourceLedger::new(initial);\n\n        assert_eq!(\n            ledger.credit(ResourceStock::new(1, 0, 0)),\n            Err(ResourceLedgerError::AmountOverflow)\n        );\n        assert_eq!(ledger.stock(), initial);\n    }\n\n    #[test]\n    fn energy_is_capacity_not_a_stock() {\n        let mut grid = EnergyGrid::new(80, 30);\n\n        grid.allocate(40).expect("capacity is available");\n        assert_eq!(grid.production(), 80);\n        assert_eq!(grid.consumption(), 70);\n        assert_eq!(grid.balance(), 10);\n\n        assert!(matches!(\n            grid.allocate(11),\n            Err(EnergyError::InsufficientCapacity {\n                available: 10,\n                requested: 11,\n            })\n        ));\n        assert_eq!(grid.consumption(), 70);\n    }\n\n    #[test]\n    fn configurable_cost_combines_resources_and_energy() {\n        let ledger =\n            ResourceLedger::new(ResourceStock::new(100, 80, 60));\n        let grid = EnergyGrid::new(50, 20);\n        let cost = EconomicCost::new(\n            ResourceCost::new(90, 70, 50),\n            25,\n        );\n\n        assert!(cost.can_start(&ledger, grid));\n        assert!(\n            !EconomicCost::new(\n                ResourceCost::new(90, 70, 50),\n                31,\n            )\n            .can_start(&ledger, grid)\n        );\n    }\n}\n'
PRODUCTION_RS = '// MVP-012: deterministic production, storage and energy throttling.\nuse galactic_domain::{ColonyId, ResourceStock};\n\nuse crate::{\n    BuildingLevels, ColonyState, PlanetResourceProfile,\n    StrategicDuration, STRATEGIC_TICKS_PER_SECOND,\n};\n\n/// Fixed-point scale used for sub-unit production.\npub const PRODUCTION_SCALE: u64 = 1_000;\n\n/// Temporary centralized rules. MVP-013 will replace these constants with\n/// data from the building catalog without changing the simulation loop.\npub const BASE_METAL_MILLI_PER_TICK: u64 = 250;\npub const BASE_CRYSTAL_MILLI_PER_TICK: u64 = 125;\npub const BASE_FUEL_MILLI_PER_TICK: u64 = 75;\n\npub const BASE_STORAGE_CAPACITY: ResourceStock =\n    ResourceStock::new(1_000, 800, 600);\npub const WAREHOUSE_CAPACITY_PER_LEVEL: ResourceStock =\n    ResourceStock::new(4_000, 3_200, 2_400);\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ProductionRemainder {\n    metal_milli: u16,\n    crystal_milli: u16,\n    fuel_milli: u16,\n}\n\nimpl ProductionRemainder {\n    pub const ZERO: Self = Self::new_unchecked(0, 0, 0);\n\n    const fn new_unchecked(\n        metal_milli: u16,\n        crystal_milli: u16,\n        fuel_milli: u16,\n    ) -> Self {\n        Self {\n            metal_milli,\n            crystal_milli,\n            fuel_milli,\n        }\n    }\n\n    pub fn from_parts(\n        metal_milli: u16,\n        crystal_milli: u16,\n        fuel_milli: u16,\n    ) -> Result<Self, ProductionRemainderError> {\n        let scale = PRODUCTION_SCALE as u16;\n        if metal_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Metal,\n                    value: metal_milli,\n                },\n            );\n        }\n        if crystal_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Crystal,\n                    value: crystal_milli,\n                },\n            );\n        }\n        if fuel_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Fuel,\n                    value: fuel_milli,\n                },\n            );\n        }\n\n        Ok(Self::new_unchecked(\n            metal_milli,\n            crystal_milli,\n            fuel_milli,\n        ))\n    }\n\n    pub const fn metal_milli(self) -> u16 {\n        self.metal_milli\n    }\n\n    pub const fn crystal_milli(self) -> u16 {\n        self.crystal_milli\n    }\n\n    pub const fn fuel_milli(self) -> u16 {\n        self.fuel_milli\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ProductionResource {\n    Metal,\n    Crystal,\n    Fuel,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ProductionRemainderError {\n    OutOfRange {\n        resource: ProductionResource,\n        value: u16,\n    },\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ProductionRate {\n    pub metal_milli_per_tick: u64,\n    pub crystal_milli_per_tick: u64,\n    pub fuel_milli_per_tick: u64,\n}\n\nimpl ProductionRate {\n    pub const ZERO: Self = Self {\n        metal_milli_per_tick: 0,\n        crystal_milli_per_tick: 0,\n        fuel_milli_per_tick: 0,\n    };\n\n    pub fn for_colony(\n        buildings: BuildingLevels,\n        profile: PlanetResourceProfile,\n    ) -> Self {\n        Self {\n            metal_milli_per_tick: modified_rate(\n                BASE_METAL_MILLI_PER_TICK,\n                buildings.metal_mine,\n                profile.metal,\n            ),\n            crystal_milli_per_tick: modified_rate(\n                BASE_CRYSTAL_MILLI_PER_TICK,\n                buildings.crystal_extractor,\n                profile.crystal,\n            ),\n            fuel_milli_per_tick: modified_rate(\n                BASE_FUEL_MILLI_PER_TICK,\n                buildings.fuel_refinery,\n                profile.fuel,\n            ),\n        }\n    }\n\n    pub fn scaled_by_permille(\n        self,\n        efficiency_per_mille: u16,\n    ) -> Self {\n        Self {\n            metal_milli_per_tick: scale_rate(\n                self.metal_milli_per_tick,\n                efficiency_per_mille,\n            ),\n            crystal_milli_per_tick: scale_rate(\n                self.crystal_milli_per_tick,\n                efficiency_per_mille,\n            ),\n            fuel_milli_per_tick: scale_rate(\n                self.fuel_milli_per_tick,\n                efficiency_per_mille,\n            ),\n        }\n    }\n\n    pub fn metal_per_second(self) -> f64 {\n        per_second(self.metal_milli_per_tick)\n    }\n\n    pub fn crystal_per_second(self) -> f64 {\n        per_second(self.crystal_milli_per_tick)\n    }\n\n    pub fn fuel_per_second(self) -> f64 {\n        per_second(self.fuel_milli_per_tick)\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaturationTime {\n    Full,\n    Never,\n    In(StrategicDuration),\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SaturationEstimate {\n    pub metal: SaturationTime,\n    pub crystal: SaturationTime,\n    pub fuel: SaturationTime,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ColonyProductionSnapshot {\n    pub capacity: ResourceStock,\n    pub nominal_rate: ProductionRate,\n    pub effective_rate: ProductionRate,\n    pub nominal_energy_production: u64,\n    pub effective_energy_production: u64,\n    pub energy_efficiency_per_mille: u16,\n    pub saturation: SaturationEstimate,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ColonyProductionReport {\n    pub colony_id: ColonyId,\n    pub ticks: StrategicDuration,\n    pub produced: ResourceStock,\n    pub blocked_by_storage: ResourceStock,\n    pub energy_efficiency_per_mille: u16,\n}\n\npub fn storage_capacity(\n    buildings: BuildingLevels,\n) -> ResourceStock {\n    let warehouse_level = u64::from(buildings.warehouse);\n    ResourceStock::new(\n        BASE_STORAGE_CAPACITY\n            .metal\n            .saturating_add(\n                WAREHOUSE_CAPACITY_PER_LEVEL\n                    .metal\n                    .saturating_mul(warehouse_level),\n            ),\n        BASE_STORAGE_CAPACITY\n            .crystal\n            .saturating_add(\n                WAREHOUSE_CAPACITY_PER_LEVEL\n                    .crystal\n                    .saturating_mul(warehouse_level),\n            ),\n        BASE_STORAGE_CAPACITY\n            .fuel\n            .saturating_add(\n                WAREHOUSE_CAPACITY_PER_LEVEL\n                    .fuel\n                    .saturating_mul(warehouse_level),\n            ),\n    )\n}\n\npub fn colony_production_snapshot(\n    colony: &ColonyState,\n) -> ColonyProductionSnapshot {\n    let capacity = storage_capacity(colony.buildings);\n    let nominal_rate = ProductionRate::for_colony(\n        colony.buildings,\n        colony.resource_profile,\n    );\n    let nominal_energy_production = colony.energy.production();\n    let effective_energy_production =\n        apply_modifier(\n            nominal_energy_production,\n            colony.resource_profile.energy,\n        );\n    let energy_efficiency_per_mille =\n        energy_efficiency_per_mille(\n            effective_energy_production,\n            colony.energy.consumption(),\n        );\n    let effective_rate =\n        nominal_rate.scaled_by_permille(\n            energy_efficiency_per_mille,\n        );\n    let stock = colony.resources.stock();\n    let remainder = colony.production_remainder;\n\n    ColonyProductionSnapshot {\n        capacity,\n        nominal_rate,\n        effective_rate,\n        nominal_energy_production,\n        effective_energy_production,\n        energy_efficiency_per_mille,\n        saturation: SaturationEstimate {\n            metal: saturation_time(\n                stock.metal,\n                capacity.metal,\n                effective_rate.metal_milli_per_tick,\n                remainder.metal_milli(),\n            ),\n            crystal: saturation_time(\n                stock.crystal,\n                capacity.crystal,\n                effective_rate.crystal_milli_per_tick,\n                remainder.crystal_milli(),\n            ),\n            fuel: saturation_time(\n                stock.fuel,\n                capacity.fuel,\n                effective_rate.fuel_milli_per_tick,\n                remainder.fuel_milli(),\n            ),\n        },\n    }\n}\n\npub fn apply_colony_production(\n    colony: &mut ColonyState,\n    ticks: StrategicDuration,\n) -> ColonyProductionReport {\n    let snapshot = colony_production_snapshot(colony);\n    let tick_count = ticks.ticks();\n\n    let (metal, next_metal) = generated_units(\n        snapshot.effective_rate.metal_milli_per_tick,\n        tick_count,\n        colony.production_remainder.metal_milli(),\n    );\n    let (crystal, next_crystal) = generated_units(\n        snapshot.effective_rate.crystal_milli_per_tick,\n        tick_count,\n        colony.production_remainder.crystal_milli(),\n    );\n    let (fuel, next_fuel) = generated_units(\n        snapshot.effective_rate.fuel_milli_per_tick,\n        tick_count,\n        colony.production_remainder.fuel_milli(),\n    );\n\n    let requested = ResourceStock::new(\n        metal,\n        crystal,\n        fuel,\n    );\n    let produced = colony.resources.credit_capped(\n        requested,\n        snapshot.capacity,\n    );\n    let blocked_by_storage =\n        requested.saturating_sub(produced);\n\n    colony.production_remainder =\n        ProductionRemainder::new_unchecked(\n            if produced.metal < requested.metal {\n                0\n            } else {\n                next_metal\n            },\n            if produced.crystal < requested.crystal {\n                0\n            } else {\n                next_crystal\n            },\n            if produced.fuel < requested.fuel {\n                0\n            } else {\n                next_fuel\n            },\n        );\n\n    ColonyProductionReport {\n        colony_id: colony.id,\n        ticks,\n        produced,\n        blocked_by_storage,\n        energy_efficiency_per_mille:\n            snapshot.energy_efficiency_per_mille,\n    }\n}\n\nfn modified_rate(\n    base_milli_per_tick: u64,\n    level: u8,\n    modifier_percent: u16,\n) -> u64 {\n    let value = u128::from(base_milli_per_tick)\n        .saturating_mul(u128::from(level))\n        .saturating_mul(u128::from(modifier_percent))\n        / 100;\n    value.min(u128::from(u64::MAX)) as u64\n}\n\nfn scale_rate(\n    rate: u64,\n    efficiency_per_mille: u16,\n) -> u64 {\n    let value = u128::from(rate)\n        .saturating_mul(u128::from(\n            efficiency_per_mille,\n        ))\n        / u128::from(PRODUCTION_SCALE);\n    value.min(u128::from(u64::MAX)) as u64\n}\n\nfn apply_modifier(\n    value: u64,\n    modifier_percent: u16,\n) -> u64 {\n    let modified = u128::from(value)\n        .saturating_mul(u128::from(modifier_percent))\n        / 100;\n    modified.min(u128::from(u64::MAX)) as u64\n}\n\nfn energy_efficiency_per_mille(\n    effective_production: u64,\n    consumption: u64,\n) -> u16 {\n    if consumption == 0\n        || effective_production >= consumption\n    {\n        return PRODUCTION_SCALE as u16;\n    }\n\n    let value = u128::from(effective_production)\n        .saturating_mul(u128::from(PRODUCTION_SCALE))\n        / u128::from(consumption);\n    value.min(u128::from(PRODUCTION_SCALE)) as u16\n}\n\nfn generated_units(\n    rate_milli_per_tick: u64,\n    ticks: u64,\n    previous_remainder: u16,\n) -> (u64, u16) {\n    let total = u128::from(rate_milli_per_tick)\n        .saturating_mul(u128::from(ticks))\n        .saturating_add(u128::from(\n            previous_remainder,\n        ));\n    let units = total / u128::from(PRODUCTION_SCALE);\n    let remainder =\n        (total % u128::from(PRODUCTION_SCALE)) as u16;\n\n    (\n        units.min(u128::from(u64::MAX)) as u64,\n        remainder,\n    )\n}\n\nfn saturation_time(\n    stock: u64,\n    capacity: u64,\n    rate_milli_per_tick: u64,\n    remainder_milli: u16,\n) -> SaturationTime {\n    if stock >= capacity {\n        return SaturationTime::Full;\n    }\n    if rate_milli_per_tick == 0 {\n        return SaturationTime::Never;\n    }\n\n    let missing_units = capacity - stock;\n    let required_milli =\n        u128::from(missing_units)\n            .saturating_mul(u128::from(\n                PRODUCTION_SCALE,\n            ))\n            .saturating_sub(u128::from(\n                remainder_milli,\n            ));\n    let rate = u128::from(rate_milli_per_tick);\n    let ticks = required_milli\n        .saturating_add(rate - 1)\n        / rate;\n\n    SaturationTime::In(\n        StrategicDuration::from_ticks(\n            ticks.min(u128::from(u64::MAX)) as u64,\n        ),\n    )\n}\n\nfn per_second(rate_milli_per_tick: u64) -> f64 {\n    rate_milli_per_tick as f64\n        * f64::from(STRATEGIC_TICKS_PER_SECOND)\n        / PRODUCTION_SCALE as f64\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{\n        EnergyGrid, ResourceLedger, UniverseConfig,\n    };\n\n    use crate::Simulation;\n\n    use super::*;\n\n    fn home_colony() -> ColonyState {\n        Simulation::new(UniverseConfig::mvp())\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .clone()\n    }\n\n    #[test]\n    fn starting_colony_has_expected_rates_and_capacity() {\n        let colony = home_colony();\n        let snapshot =\n            colony_production_snapshot(&colony);\n\n        assert_eq!(\n            snapshot.capacity,\n            ResourceStock::new(5_000, 4_000, 3_000)\n        );\n        assert_eq!(\n            snapshot.nominal_rate,\n            ProductionRate {\n                metal_milli_per_tick: 250,\n                crystal_milli_per_tick: 125,\n                fuel_milli_per_tick: 75,\n            }\n        );\n        assert_eq!(\n            snapshot.energy_efficiency_per_mille,\n            1_000\n        );\n    }\n\n    #[test]\n    fn tick_batches_produce_identical_state() {\n        let mut batched = home_colony();\n        let mut incremental = batched.clone();\n\n        apply_colony_production(\n            &mut batched,\n            StrategicDuration::from_ticks(100),\n        );\n        for _ in 0..10 {\n            apply_colony_production(\n                &mut incremental,\n                StrategicDuration::from_ticks(10),\n            );\n        }\n\n        assert_eq!(batched, incremental);\n        assert_eq!(\n            batched.resources.stock(),\n            ResourceStock::new(625, 312, 227)\n        );\n        assert_eq!(\n            batched.production_remainder,\n            ProductionRemainder::from_parts(\n                0, 500, 500,\n            )\n            .expect("valid remainder")\n        );\n    }\n\n    #[test]\n    fn energy_deficit_throttles_all_extractors() {\n        let mut colony = home_colony();\n        colony.energy = EnergyGrid::new(15, 30);\n\n        let report = apply_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(1_000),\n        );\n\n        assert_eq!(\n            report.energy_efficiency_per_mille,\n            500\n        );\n        assert_eq!(\n            report.produced,\n            ResourceStock::new(125, 62, 37)\n        );\n    }\n\n    #[test]\n    fn full_storage_discards_blocked_output() {\n        let mut colony = home_colony();\n        let capacity = storage_capacity(colony.buildings);\n        colony.resources = ResourceLedger::new(\n            ResourceStock::new(\n                capacity.metal - 1,\n                capacity.crystal - 1,\n                capacity.fuel - 1,\n            ),\n        );\n\n        let report = apply_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(1_000),\n        );\n\n        assert_eq!(colony.resources.stock(), capacity);\n        assert!(\n            !report.blocked_by_storage.is_zero()\n        );\n        assert_eq!(\n            colony.production_remainder,\n            ProductionRemainder::ZERO\n        );\n    }\n\n    #[test]\n    fn planet_profile_applies_simple_modifiers() {\n        let mut colony = home_colony();\n        colony.resource_profile =\n            PlanetResourceProfile::new(\n                150, 80, 50, 50,\n            );\n        let snapshot =\n            colony_production_snapshot(&colony);\n\n        assert_eq!(\n            snapshot.nominal_rate\n                .metal_milli_per_tick,\n            375\n        );\n        assert_eq!(\n            snapshot.nominal_rate\n                .crystal_milli_per_tick,\n            100\n        );\n        assert_eq!(\n            snapshot.nominal_rate.fuel_milli_per_tick,\n            37\n        );\n        assert_eq!(\n            snapshot.effective_energy_production,\n            40\n        );\n        assert_eq!(\n            snapshot.energy_efficiency_per_mille,\n            1_000\n        );\n    }\n\n    #[test]\n    fn zero_energy_blocks_production_without_losing_remainder() {\n        let mut colony = home_colony();\n        colony.production_remainder =\n            ProductionRemainder::from_parts(\n                900, 800, 700,\n            )\n            .expect("valid remainder");\n        colony.energy = EnergyGrid::new(0, 30);\n        let before = colony.production_remainder;\n\n        let report = apply_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(100),\n        );\n\n        assert!(report.produced.is_zero());\n        assert_eq!(\n            report.energy_efficiency_per_mille,\n            0\n        );\n        assert_eq!(colony.production_remainder, before);\n    }\n}\n'
PERSISTENCE_RS = '// MVP-012: persist production remainders, storage-safe stocks and energy.\nuse galactic_domain::{\n    ColonyId, EnergyGrid, FactionId, PlanetId,\n    ResourceLedger, ResourceLedgerError, ResourceReservation,\n    ResourceStock, SystemId, UniverseConfig, UniverseId,\n    generate_universe,\n};\nuse galactic_sim::{\n    BuildingLevels, ColonyState, FactionKind, FactionState,\n    GameState, PlanetKnowledge, PlanetResourceProfile,\n    ProductionRemainder, ProductionRemainderError,\n    SelectionTarget, Simulation, SimulationBuildError,\n    StrategicClock, StrategicClockError, StrategicTick,\n    SystemKnowledge, TimeSpeed,\n};\n\npub const SAVE_VERSION: u32 = 7;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub factions: Vec<FactionSave>,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub system_knowledge: Vec<SystemKnowledge>,\n    pub planet_knowledge: Vec<PlanetKnowledge>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionSave {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub reservations: Vec<ResourceReservation>,\n    pub next_reservation_id: u64,\n    pub energy_production: u64,\n    pub energy_consumption: u64,\n    pub production_remainder_metal: u16,\n    pub production_remainder_crystal: u16,\n    pub production_remainder_fuel: u16,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidResourceLedger {\n        colony_id: ColonyId,\n        error: ResourceLedgerError,\n    },\n    InvalidProductionRemainder {\n        colony_id: ColonyId,\n        error: ProductionRemainderError,\n    },\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(\n    simulation: &Simulation,\n) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint:\n                universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            factions: state\n                .factions\n                .iter()\n                .map(|faction| FactionSave {\n                    id: faction.id,\n                    name: faction.name.clone(),\n                    kind: faction.kind,\n                })\n                .collect(),\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos:\n                    state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            system_knowledge:\n                state.system_knowledge.clone(),\n            planet_knowledge:\n                state.planet_knowledge.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.resources.stock(),\n                    reservations: colony\n                        .resources\n                        .reservations()\n                        .to_vec(),\n                    next_reservation_id: colony\n                        .resources\n                        .next_reservation_id(),\n                    energy_production:\n                        colony.energy.production(),\n                    energy_consumption:\n                        colony.energy.consumption(),\n                    production_remainder_metal:\n                        colony\n                            .production_remainder\n                            .metal_milli(),\n                    production_remainder_crystal:\n                        colony\n                            .production_remainder\n                            .crystal_milli(),\n                    production_remainder_fuel:\n                        colony\n                            .production_remainder\n                            .fuel_milli(),\n                    buildings: colony.buildings,\n                    resource_profile:\n                        colony.resource_profile,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(\n    save: &SaveGame,\n) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(\n            SaveError::UnsupportedVersion(save.version),\n        );\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version\n        != save.universe.generation_version\n    {\n        return Err(\n            SaveError::GenerationVersionMismatch {\n                expected: universe.generation_version,\n                found:\n                    save.universe.generation_version,\n            },\n        );\n    }\n    if universe.generation_fingerprint\n        != save.universe.generation_fingerprint\n    {\n        return Err(\n            SaveError::GenerationFingerprintMismatch {\n                expected:\n                    universe.generation_fingerprint,\n                found:\n                    save.universe\n                        .generation_fingerprint,\n            },\n        );\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let colonies = save\n        .state\n        .colonies\n        .iter()\n        .map(|colony| {\n            let resources = ResourceLedger::from_parts(\n                colony.stock,\n                colony.reservations.clone(),\n                colony.next_reservation_id,\n            )\n            .map_err(|error| {\n                SaveError::InvalidResourceLedger {\n                    colony_id: colony.id,\n                    error,\n                }\n            })?;\n            let production_remainder =\n                ProductionRemainder::from_parts(\n                    colony.production_remainder_metal,\n                    colony.production_remainder_crystal,\n                    colony.production_remainder_fuel,\n                )\n                .map_err(|error| {\n                    SaveError::InvalidProductionRemainder {\n                        colony_id: colony.id,\n                        error,\n                    }\n                })?;\n\n            Ok(ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                resources,\n                energy: EnergyGrid::new(\n                    colony.energy_production,\n                    colony.energy_consumption,\n                ),\n                production_remainder,\n                buildings: colony.buildings,\n                resource_profile:\n                    colony.resource_profile,\n            })\n        })\n        .collect::<Result<Vec<_>, SaveError>>()?;\n\n    let state = GameState {\n        version: save.state.version,\n        factions: save\n            .state\n            .factions\n            .iter()\n            .map(|faction| FactionState {\n                id: faction.id,\n                name: faction.name.clone(),\n                kind: faction.kind,\n            })\n            .collect(),\n        player_faction: save.state.player_faction,\n        colonies,\n        system_knowledge:\n            save.state.system_knowledge.clone(),\n        planet_knowledge:\n            save.state.planet_knowledge.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state)\n        .map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::{\n        ReservationId, ResourceCost,\n        ResourceReservation, SystemId, UniverseConfig,\n    };\n    use galactic_sim::{\n        GAME_STATE_VERSION, GameCommand, KnowledgeLevel,\n        PRODUCTION_SCALE, STRATEGIC_TICK_NANOS,\n        StrategicTick, TimeSpeed,\n    };\n\n    use super::*;\n\n    #[test]\n    fn snapshot_round_trips_production_and_economy() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let target = simulation\n            .universe_repository()\n            .neighboring_systems(SystemId::from_index(0))\n            .into_iter()\n            .next()\n            .expect("home has a neighbor");\n        simulation\n            .apply_command(GameCommand::SelectSystem(target));\n        simulation.apply_command(\n            GameCommand::DebugAdvanceSelectedKnowledge,\n        );\n        simulation.advance(Duration::from_millis(125));\n        simulation\n            .apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony\n            .resources\n            .reserve(ResourceCost::new(50, 25, 10))\n            .expect("test reservation is funded");\n        colony\n            .energy\n            .allocate(10)\n            .expect("energy capacity is available");\n\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("save is compatible");\n\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(\n            restored.state().system_knowledge_level(target),\n            KnowledgeLevel::Probed\n        );\n        assert_eq!(\n            restored.state().clock.current_tick(),\n            StrategicTick::new(1)\n        );\n    }\n\n    #[test]\n    fn snapshot_contains_production_remainders() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        simulation.advance(Duration::from_millis(100));\n        let save = snapshot_from_simulation(&simulation);\n        let colony = save\n            .state\n            .colonies\n            .first()\n            .expect("home colony is saved");\n\n        assert_eq!(save.state.version, GAME_STATE_VERSION);\n        assert_eq!(\n            colony.stock,\n            ResourceStock::new(600, 300, 220)\n        );\n        assert_eq!(\n            colony.production_remainder_metal,\n            250\n        );\n        assert_eq!(\n            colony.production_remainder_crystal,\n            125\n        );\n        assert_eq!(\n            colony.production_remainder_fuel,\n            75\n        );\n        assert_eq!(colony.energy_production, 80);\n        assert_eq!(colony.energy_consumption, 30);\n    }\n\n    #[test]\n    fn invalid_production_remainder_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.state.colonies[0]\n            .production_remainder_metal =\n            PRODUCTION_SCALE as u16;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(\n                SaveError::InvalidProductionRemainder {\n                    ..\n                }\n            )\n        ));\n    }\n\n    #[test]\n    fn invalid_over_reserved_ledger_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        let colony = save\n            .state\n            .colonies\n            .first_mut()\n            .expect("home colony is saved");\n        colony.reservations.push(\n            ResourceReservation::new(\n                ReservationId::new(1),\n                ResourceCost::new(700, 0, 0),\n            ),\n        );\n        colony.next_reservation_id = 2;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidResourceLedger {\n                ..\n            })\n        ));\n    }\n\n    #[test]\n    fn modified_fingerprint_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.universe.generation_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(\n                SaveError::GenerationFingerprintMismatch {\n                    ..\n                }\n            )\n        ));\n    }\n\n    #[test]\n    fn invalid_clock_remainder_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.state.clock.remainder_nanos =\n            STRATEGIC_TICK_NANOS;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidClock(\n                StrategicClockError::RemainderOutOfRange(\n                    _\n                )\n            ))\n        ));\n    }\n\n    #[test]\n    fn unsupported_save_version_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::default());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.version = 999;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::UnsupportedVersion(999))\n        ));\n    }\n}\n'
COLONY_ECONOMY_TEXT = 'fn colony_economy_text(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    let stock = colony.resources.stock();\n    let available = colony.resources.available();\n    let reserved = colony.resources.reserved_total();\n    let production =\n        galactic_sim::colony_production_snapshot(colony);\n\n    format!(\n        "STOCKS EXACTS\\nTotal — Métal {}  Cristal {}  Carburant {}\\nDisponible — Métal {}  Cristal {}  Carburant {}\\nRéservé — Métal {}  Cristal {}  Carburant {}\\nCapacité — Métal {}  Cristal {}  Carburant {}\\n\\nPRODUCTION ACTUELLE\\nMétal +{:.2}/s  Cristal +{:.2}/s  Carburant +{:.2}/s\\nSaturation — Métal {}  Cristal {}  Carburant {}\\n\\nÉNERGIE — CAPACITÉ\\nNominale : {}\\nEffective planète : {}\\nConsommation : {}\\nEfficacité extracteurs : {}%\\nBilan effectif : {:+}",\n        stock.metal,\n        stock.crystal,\n        stock.fuel,\n        available.metal,\n        available.crystal,\n        available.fuel,\n        reserved.metal,\n        reserved.crystal,\n        reserved.fuel,\n        production.capacity.metal,\n        production.capacity.crystal,\n        production.capacity.fuel,\n        production.effective_rate.metal_per_second(),\n        production.effective_rate.crystal_per_second(),\n        production.effective_rate.fuel_per_second(),\n        format_saturation_time(production.saturation.metal),\n        format_saturation_time(production.saturation.crystal),\n        format_saturation_time(production.saturation.fuel),\n        production.nominal_energy_production,\n        production.effective_energy_production,\n        colony.energy.consumption(),\n        u32::from(production.energy_efficiency_per_mille) / 10,\n        i128::from(production.effective_energy_production)\n            - i128::from(colony.energy.consumption()),\n    )\n}\n\nfn format_saturation_time(\n    saturation: galactic_sim::SaturationTime,\n) -> String {\n    match saturation {\n        galactic_sim::SaturationTime::Full => {\n            "plein".to_string()\n        }\n        galactic_sim::SaturationTime::Never => {\n            "jamais".to_string()\n        }\n        galactic_sim::SaturationTime::In(duration) => {\n            format_strategic_duration(duration)\n        }\n    }\n}\n\nfn format_strategic_duration(\n    duration: galactic_sim::StrategicDuration,\n) -> String {\n    let seconds = duration.as_duration().as_secs();\n    let hours = seconds / 3_600;\n    let minutes = (seconds % 3_600) / 60;\n    let remaining_seconds = seconds % 60;\n\n    if hours > 0 {\n        format!("{hours}h {minutes:02}m")\n    } else if minutes > 0 {\n        format!("{minutes}m {remaining_seconds:02}s")\n    } else {\n        format!("{remaining_seconds}s")\n    }\n}\n'
SIM_TESTS = '\n    #[test]\n    fn production_is_independent_from_frame_rate() {\n        let mut fast_frames =\n            Simulation::new(UniverseConfig::mvp());\n        let mut slow_frames =\n            Simulation::new(UniverseConfig::mvp());\n\n        advance_in_equal_frames(\n            &mut fast_frames,\n            1_000,\n            Duration::from_millis(10),\n        );\n        advance_in_equal_frames(\n            &mut slow_frames,\n            100,\n            Duration::from_millis(100),\n        );\n\n        assert_eq!(fast_frames.state(), slow_frames.state());\n        assert_eq!(\n            fast_frames\n                .state()\n                .player_home_colony()\n                .expect("home colony exists")\n                .resources\n                .stock(),\n            ResourceStock::new(625, 312, 227)\n        );\n    }\n\n    #[test]\n    fn pause_and_speed_apply_expected_production_ticks() {\n        let mut paused =\n            Simulation::new(UniverseConfig::mvp());\n        paused.apply_command(GameCommand::TogglePause);\n        let initial = paused.state().clone();\n        paused.advance(Duration::from_secs(10));\n        assert_eq!(paused.state(), &initial);\n\n        let mut x1 =\n            Simulation::new(UniverseConfig::mvp());\n        let mut x4 =\n            Simulation::new(UniverseConfig::mvp());\n        x4.apply_command(\n            GameCommand::SetSpeed(TimeSpeed::X4),\n        );\n\n        x1.advance(Duration::from_secs(1));\n        x4.advance(Duration::from_millis(250));\n        x4.apply_command(\n            GameCommand::SetSpeed(TimeSpeed::X1),\n        );\n\n        assert_eq!(x1.state(), x4.state());\n    }\n\n    #[test]\n    fn reconstruction_rejects_stock_above_capacity() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let universe = simulation.universe().clone();\n        let mut state = simulation.state().clone();\n        let colony = state\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        let capacity =\n            crate::storage_capacity(colony.buildings);\n        colony.resources = ResourceLedger::new(\n            ResourceStock::new(\n                capacity.metal + 1,\n                capacity.crystal,\n                capacity.fuel,\n            ),\n        );\n\n        assert!(matches!(\n            Simulation::from_parts(universe, state),\n            Err(\n                SimulationBuildError::ColonyStockExceedsCapacity {\n                    ..\n                }\n            )\n        ));\n    }\n'
DOC_APPEND = "\n## MVP-012 — Production planétaire et capacités de stockage\n\nLa production est exécutée uniquement à partir des ticks stratégiques :\n\n```text\ndurée réelle\n    ↓ StrategicClock\nnombre entier de ticks\n    ↓\nproduction de chaque colonie\n    ↓\ncrédit plafonné par la capacité\n```\n\nUne production possède un reliquat fixe au millième d'unité. Le reliquat est\nsauvegardé afin que plusieurs découpages de frames produisent exactement le\nmême état.\n\nRègles temporaires centralisées, en attendant le catalogue MVP-013 :\n\n- Mine de métal niveau 1 : 2,50 unités/s à potentiel 100 ;\n- Extracteur de cristal niveau 1 : 1,25 unité/s à potentiel 100 ;\n- Raffinerie niveau 1 : 0,75 unité/s à potentiel 100 ;\n- chaque taux est multiplié par le niveau et le potentiel planétaire ;\n- capacité de base : 1 000 / 800 / 600 ;\n- chaque niveau d'entrepôt ajoute 4 000 / 3 200 / 2 400.\n\nL'énergie suit une règle proportionnelle documentée :\n\n```text\nproduction énergétique effective\n    = capacité nominale × potentiel énergétique / 100\n\nsi production effective >= consommation\n    efficacité des extracteurs = 100 %\nsinon\n    efficacité = production effective / consommation\n```\n\nTous les extracteurs sont ralentis par le même facteur. Une production\nénergétique effective nulle bloque la production mais conserve le reliquat\nfractionnaire déjà acquis.\n\nQuand un stockage est plein :\n\n- le stock ne dépasse jamais sa capacité ;\n- la production excédentaire est perdue ;\n- aucun reliquat caché n'est accumulé pour contourner la saturation.\n\nL'inspecteur de colonie affiche :\n\n- stock total, disponible et réservé ;\n- capacité de chaque ressource ;\n- production effective par seconde ;\n- temps estimé avant saturation ;\n- énergie nominale et effective ;\n- consommation, efficacité et bilan.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 6` ;\n- `SAVE_VERSION = 7`.\n\nMVP-013 remplacera les constantes de production et de stockage par les\ndéfinitions du catalogue de bâtiments.\n"


@dataclass(frozen=True)
class Update:
    path: Path
    before: str
    after: str


def run(
    command: list[str],
    *,
    cwd: Path,
    check: bool = True,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if capture and result.stdout:
        print(
            result.stdout,
            end="" if result.stdout.endswith("\n") else "\n",
        )
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : "
            f"{' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (
                candidate
                / "crates/galactic_domain/src/resources.rs"
            ).exists()
            and (
                candidate
                / "crates/galactic_sim/src/simulation.rs"
            ).exists()
        ):
            return candidate
    raise SystemExit(
        "Racine Galactic introuvable. Utilise --root."
    )


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def replace_once(
    source: str,
    old: str,
    new: str,
    description: str,
) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"Patch impossible pour {description}: "
            f"{count} occurrence(s), 1 attendue."
        )
    return source.replace(old, new, 1)


def verify_baseline(root: Path, force: bool) -> None:
    head = run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
    ).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return

    ancestor = run(
        [
            "git",
            "merge-base",
            "--is-ancestor",
            EXPECTED_BASELINE_COMMIT,
            "HEAD",
        ],
        cwd=root,
        check=False,
    )
    if ancestor.returncode == 0:
        print(
            "Baseline présente dans l'historique ; "
            f"HEAD actuel : {head}"
        )
        return
    if force:
        print(
            "WARNING: baseline différente, poursuite "
            "autorisée par --force."
        )
        return

    raise SystemExit(
        "Le dépôt local ne correspond pas à la baseline "
        "MVP-011 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    resources = (
        root
        / "crates/galactic_domain/src/resources.rs"
    ).read_text(encoding="utf-8")
    state = (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "pub struct ResourceLedger",
        "pub struct EnergyGrid",
        "pub fn credit_capped",
    ):
        if marker == "pub fn credit_capped":
            continue
        if marker not in resources:
            failures.append(
                f"marqueur ressources absent : {marker}"
            )
    for marker in (
        "pub resources: ResourceLedger",
        "pub energy: EnergyGrid",
    ):
        if marker not in state:
            failures.append(
                f"marqueur colonie absent : {marker}"
            )
    for marker in (
        "// MVP-010-B: screen-space picking",
        "fn colony_economy_text(",
    ):
        if marker not in client:
            failures.append(
                f"marqueur client absent : {marker}"
            )

    if failures:
        raise SystemExit(
            "Baseline MVP-011 incohérente :\n- "
            + "\n- ".join(failures)
        )


def cargo_edition(root: Path) -> str:
    cargo = (root / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    match = re.search(
        r'(?m)^edition\s*=\s*"([^"]+)"',
        cargo,
    )
    return match.group(1) if match else "2024"


def format_rust(root: Path, content: str) -> str:
    rustfmt = shutil.which("rustfmt")
    if rustfmt is None:
        raise SystemExit(
            "rustfmt est requis, y compris pour --dry-run."
        )

    with tempfile.NamedTemporaryFile(
        mode="w",
        suffix=".rs",
        encoding="utf-8",
        delete=False,
    ) as handle:
        temporary = Path(handle.name)
        handle.write(normalize(content))

    try:
        result = subprocess.run(
            [
                rustfmt,
                "--edition",
                cargo_edition(root),
                "--config",
                "skip_children=true",
                str(temporary),
            ],
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        if result.returncode != 0:
            raise SystemExit(
                "rustfmt n'a pas pu formater une source "
                f"générée :\n{result.stdout}"
            )
        return normalize(
            temporary.read_text(encoding="utf-8")
        )
    finally:
        temporary.unlink(missing_ok=True)


def patch_sim_lib(source: str) -> str:
    if "pub mod production;" not in source:
        source = replace_once(
            source,
            "pub mod knowledge;\n",
            "pub mod knowledge;\npub mod production;\n",
            "module production",
        )
    if "pub use production::*;" not in source:
        source = replace_once(
            source,
            "pub use knowledge::*;\n",
            "pub use knowledge::*;\npub use production::*;\n",
            "export production",
        )
    return normalize(source)


def patch_state(source: str) -> str:
    if "pub production_remainder: ProductionRemainder" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-011: persistent knowledge and colony economy",
        "// MVP-012: persistent knowledge, production and storage",
        1,
    )
    source = replace_once(
        source,
        "    PlanetKnowledge, PlanetResourceProfile, SelectionTarget, StartingScenario,\n"
        "    StartingScenarioError, StrategicClock, SystemKnowledge, UniverseRepository,\n",
        "    PlanetKnowledge, PlanetResourceProfile, "
        "ProductionRemainder, SelectionTarget,\n"
        "    StartingScenario, StartingScenarioError, "
        "StrategicClock, SystemKnowledge,\n"
        "    UniverseRepository,\n",
        "import ProductionRemainder",
    )
    source = source.replace(
        "/// Version 5 adds atomic resource ledgers and an energy grid per colony.\n"
        "pub const GAME_STATE_VERSION: u32 = 5;",
        "/// Version 6 adds persisted fixed-point production remainders.\n"
        "pub const GAME_STATE_VERSION: u32 = 6;",
        1,
    )
    source = replace_once(
        source,
        "                energy: home.initial_energy,\n"
        "                buildings: home.buildings,\n",
        "                energy: home.initial_energy,\n"
        "                production_remainder: "
        "ProductionRemainder::ZERO,\n"
        "                buildings: home.buildings,\n",
        "reliquat initial",
    )
    source = replace_once(
        source,
        "    pub energy: EnergyGrid,\n"
        "    pub buildings: BuildingLevels,\n",
        "    pub energy: EnergyGrid,\n"
        "    pub production_remainder: "
        "ProductionRemainder,\n"
        "    pub buildings: BuildingLevels,\n",
        "champ production de ColonyState",
    )

    marker = (
        "    #[test]\n"
        "    fn non_home_planets_start_as_detected_only()"
    )
    insertion = r"""    #[test]
    fn home_stock_fits_derived_storage_capacity() {
        let universe =
            UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let colony =
            state.player_home_colony().expect("home colony exists");
        let capacity = crate::storage_capacity(colony.buildings);

        assert!(colony.resources.stock().is_within(capacity));
        assert_eq!(
            colony.production_remainder,
            ProductionRemainder::ZERO
        );
    }

"""
    if marker not in source:
        raise SystemExit(
            "Point d'insertion du test de capacité introuvable."
        )
    source = source.replace(
        marker,
        insertion + marker,
        1,
    )
    return normalize(source)


def patch_simulation(source: str) -> str:
    if "apply_colony_production" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-009: simulation commands and validation for progressive knowledge",
        "// MVP-012: simulation commands, production and validation",
        1,
    )
    source = replace_once(
        source,
        "use galactic_domain::{\n"
        "    ColonyId, FactionId, PlanetId, SystemId, UniverseConfig, UniverseDefinition,\n"
        "};",
        "use galactic_domain::{\n"
        "    ColonyId, FactionId, PlanetId, "
        "ResourceLedgerError, ResourceStock,\n"
        "    SystemId, UniverseConfig, UniverseDefinition,\n"
        "};",
        "imports économie simulation",
    )
    source = replace_once(
        source,
        "    FactionKind, GAME_STATE_VERSION, GameCommand, GameEvent, GameState, KnowledgeLevel,\n"
        "    SelectionTarget, StartingScenario, StartingScenarioError, TimeSpeed, UniverseIndexError,\n"
        "    UniverseRepository,\n",
        "    FactionKind, GAME_STATE_VERSION, GameCommand, "
        "GameEvent, GameState,\n"
        "    KnowledgeLevel, SelectionTarget, StartingScenario, "
        "StartingScenarioError,\n"
        "    TimeSpeed, UniverseIndexError, UniverseRepository, "
        "apply_colony_production,\n"
        "    storage_capacity,\n",
        "imports production simulation",
    )
    source = replace_once(
        source,
        "    DuplicateColony(ColonyId),\n",
        "    DuplicateColony(ColonyId),\n"
        "    InvalidColonyResourceLedger {\n"
        "        colony_id: ColonyId,\n"
        "        error: ResourceLedgerError,\n"
        "    },\n"
        "    ColonyStockExceedsCapacity {\n"
        "        colony_id: ColonyId,\n"
        "        stock: ResourceStock,\n"
        "        capacity: ResourceStock,\n"
        "    },\n",
        "erreurs de production",
    )
    source = replace_once(
        source,
        "        // Future production, construction, research and mission systems will be\n"
        "        // processed once per strategic tick here.\n"
        "        vec![GameEvent::TicksAdvanced {\n",
        "        for colony in &mut self.state.colonies {\n"
        "            apply_colony_production(\n"
        "                colony,\n"
        "                advance.ticks,\n"
        "            );\n"
        "        }\n\n"
        "        vec![GameEvent::TicksAdvanced {\n",
        "boucle de production",
    )

    validation_marker = (
        "        if state.faction(colony.faction).is_none() {\n"
    )
    validation_insert = (
        "        if let Err(error) = colony.resources.validate() {\n"
        "            return Err(\n"
        "                SimulationBuildError::InvalidColonyResourceLedger {\n"
        "                    colony_id: colony.id,\n"
        "                    error,\n"
        "                },\n"
        "            );\n"
        "        }\n"
        "        let capacity = storage_capacity(colony.buildings);\n"
        "        let stock = colony.resources.stock();\n"
        "        if !stock.is_within(capacity) {\n"
        "            return Err(\n"
        "                SimulationBuildError::ColonyStockExceedsCapacity {\n"
        "                    colony_id: colony.id,\n"
        "                    stock,\n"
        "                    capacity,\n"
        "                },\n"
        "            );\n"
        "        }\n"
    )
    if validation_marker not in source:
        raise SystemExit(
            "Validation de colonie introuvable."
        )
    source = source.replace(
        validation_marker,
        validation_insert + validation_marker,
        1,
    )

    source = replace_once(
        source,
        "        assert_eq!(fast_frames.state().clock, slow_frames.state().clock);\n"
        "    }\n",
        "        assert_eq!(fast_frames.state(), slow_frames.state());\n"
        "    }\n",
        "test framerate historique",
    )

    test_marker = (
        "    #[test]\n"
        "    fn selection_events_use_domain_ids()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests MVP-012 introuvable."
        )
    source = source.replace(
        test_marker,
        SIM_TESTS.rstrip() + "\n\n" + test_marker,
        1,
    )
    return normalize(source)


def patch_client(source: str) -> str:
    if "format_saturation_time(" in source:
        return normalize(source)

    pattern = re.compile(
        r"fn colony_economy_text\(.*?\n\}\n\n"
        r"(?=fn system_inspector_content)",
        flags=re.DOTALL,
    )
    source, count = pattern.subn(
        COLONY_ECONOMY_TEXT.rstrip() + "\n\n",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "Fonction colony_economy_text introuvable."
        )
    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-012 — Production planétaire et capacités de stockage" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_domain/src/resources.rs":
            RESOURCES_RS,
        root / "crates/galactic_sim/src/production.rs":
            PRODUCTION_RS,
        root / "crates/galactic_persistence/src/lib.rs":
            PERSISTENCE_RS,
    }
    for path, content in replacements.items():
        before = (
            path.read_text(encoding="utf-8")
            if path.exists()
            else ""
        )
        after = format_rust(root, content)
        if before != after:
            updates.append(Update(path, before, after))

    for path, patcher in (
        (
            root / "crates/galactic_sim/src/lib.rs",
            patch_sim_lib,
        ),
        (
            root / "crates/galactic_sim/src/state.rs",
            patch_state,
        ),
        (
            root / "crates/galactic_sim/src/simulation.rs",
            patch_simulation,
        ),
        (
            root / "crates/galactic_client/src/lib.rs",
            patch_client,
        ),
    ):
        before = path.read_text(encoding="utf-8")
        after = format_rust(root, patcher(before))
        if before != after:
            updates.append(Update(path, before, after))

    docs_path = root / "docs/mvp_architecture.md"
    docs_before = docs_path.read_text(encoding="utf-8")
    docs_after = patch_docs(docs_before)
    if docs_before != docs_after:
        updates.append(
            Update(docs_path, docs_before, docs_after)
        )

    validate_prospective_sources(root, updates)
    return updates


def validate_prospective_sources(
    root: Path,
    updates: list[Update],
) -> None:
    replacements = {
        update.path: update.after for update in updates
    }
    required = {
        "crates/galactic_sim/src/production.rs": (
            "pub fn apply_colony_production",
            "pub fn colony_production_snapshot",
        ),
        "crates/galactic_sim/src/state.rs": (
            "pub production_remainder: ProductionRemainder",
            "GAME_STATE_VERSION: u32 = 6",
        ),
        "crates/galactic_persistence/src/lib.rs": (
            "SAVE_VERSION: u32 = 7",
            "production_remainder_metal",
        ),
    }

    failures = []
    for relative, markers in required.items():
        path = root / relative
        content = replacements.get(
            path,
            path.read_text(encoding="utf-8")
            if path.exists()
            else "",
        )
        for marker in markers:
            if marker not in content:
                failures.append(
                    f"{relative}: marqueur absent {marker}"
                )

    if failures:
        raise SystemExit(
            "Migration MVP-012 incomplète :\n- "
            + "\n- ".join(failures)
        )


def show_diff(update: Update, root: Path) -> None:
    relative = update.path.relative_to(root)
    print(
        "".join(
            difflib.unified_diff(
                update.before.splitlines(keepends=True),
                update.after.splitlines(keepends=True),
                fromfile=f"a/{relative}",
                tofile=f"b/{relative}",
            )
        ),
        end="",
    )


def apply_updates(
    updates: list[Update],
    root: Path,
    dry_run: bool,
) -> None:
    if not updates:
        print("MVP-012 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp012-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        if update.path.exists():
            backup = backup_root / relative
            backup.parent.mkdir(
                parents=True,
                exist_ok=True,
            )
            shutil.copy2(update.path, backup)
        update.path.parent.mkdir(
            parents=True,
            exist_ok=True,
        )
        update.path.write_text(
            update.after,
            encoding="utf-8",
        )
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def checks(root: Path) -> None:
    run(
        ["cargo", "fmt", "--all"],
        cwd=root,
        capture=False,
    )
    run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        cwd=root,
        capture=False,
    )
    run(
        ["cargo", "test", "--workspace"],
        cwd=root,
        capture=False,
    )
    run(
        ["cargo", "build", "--release"],
        cwd=root,
        capture=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument(
        "--force",
        action="store_true",
    )
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_current_state(root)

    status = run(
        ["git", "status", "--porcelain"],
        cwd=root,
    ).stdout
    if status.strip():
        print(
            "WARNING: working tree already contains changes."
        )
        print(
            status,
            end="" if status.endswith("\n") else "\n",
        )

    updates = collect_updates(root)
    apply_updates(updates, root, args.dry_run)

    if args.dry_run:
        print(
            f"\nDry-run complete: {len(updates)} "
            "file(s) would change."
        )
        return 0

    if args.skip_checks:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets "
            "--all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        checks(root)

    print(
        "\nMVP-012 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

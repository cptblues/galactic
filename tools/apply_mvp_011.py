#!/usr/bin/env python3
"""
Applique MVP-011 au dépôt Galactic.

Baseline analysée :
    c4b43c47340890dc8403fbe963fd641303d6f589
    feat add polish click

Le script :
- remplace le stock à quatre champs par Métal/Cristal/Carburant ;
- modélise l'énergie comme capacité produite et consommée ;
- ajoute crédit, débit et réservation atomiques ;
- empêche stocks négatifs et doubles dépenses ;
- ajoute des coûts économiques configurables ;
- migre la nouvelle partie, l'état, le HUD et la persistance ;
- conserve production et capacités de stockage pour MVP-012.

Usage :
    python tools/apply_mvp_011.py --dry-run
    python tools/apply_mvp_011.py
    python tools/apply_mvp_011.py --skip-checks
    python tools/apply_mvp_011.py --root /chemin/vers/galactic

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
    "c4b43c47340890dc8403fbe963fd641303d6f589"
)

RESOURCES_RS = '// MVP-011: atomic stored-resource ledger and energy capacity.\nuse std::collections::BTreeSet;\nuse std::ops::Add;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum ResourceKind {\n    Metal,\n    Crystal,\n    Fuel,\n    /// Energy is retained as a catalog kind for compatibility, but is never\n    /// stored in `ResourceStock`.\n    Energy,\n}\n\nimpl ResourceKind {\n    pub const ALL: [Self; 4] = [\n        Self::Metal,\n        Self::Crystal,\n        Self::Fuel,\n        Self::Energy,\n    ];\n    pub const STORED: [Self; 3] = [\n        Self::Metal,\n        Self::Crystal,\n        Self::Fuel,\n    ];\n\n    pub const fn is_stored(self) -> bool {\n        !matches!(self, Self::Energy)\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ResourceStock {\n    pub metal: u64,\n    pub crystal: u64,\n    pub fuel: u64,\n}\n\nimpl ResourceStock {\n    pub const ZERO: Self = Self::new(0, 0, 0);\n\n    pub const fn new(\n        metal: u64,\n        crystal: u64,\n        fuel: u64,\n    ) -> Self {\n        Self {\n            metal,\n            crystal,\n            fuel,\n        }\n    }\n\n    pub const fn is_zero(self) -> bool {\n        self.metal == 0 && self.crystal == 0 && self.fuel == 0\n    }\n\n    pub fn can_cover<T>(self, cost: T) -> bool\n    where\n        T: Into<ResourceCost>,\n    {\n        let cost = cost.into();\n        self.metal >= cost.metal\n            && self.crystal >= cost.crystal\n            && self.fuel >= cost.fuel\n    }\n\n    pub fn checked_add(self, other: Self) -> Option<Self> {\n        Some(Self {\n            metal: self.metal.checked_add(other.metal)?,\n            crystal: self.crystal.checked_add(other.crystal)?,\n            fuel: self.fuel.checked_add(other.fuel)?,\n        })\n    }\n\n    pub fn checked_sub<T>(self, cost: T) -> Option<Self>\n    where\n        T: Into<ResourceCost>,\n    {\n        let cost = cost.into();\n        Some(Self {\n            metal: self.metal.checked_sub(cost.metal)?,\n            crystal: self.crystal.checked_sub(cost.crystal)?,\n            fuel: self.fuel.checked_sub(cost.fuel)?,\n        })\n    }\n}\n\nimpl Add for ResourceStock {\n    type Output = Self;\n\n    fn add(self, other: Self) -> Self::Output {\n        self.checked_add(other)\n            .expect("resource stock addition must not overflow")\n    }\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ResourceCost {\n    pub metal: u64,\n    pub crystal: u64,\n    pub fuel: u64,\n}\n\nimpl ResourceCost {\n    pub const ZERO: Self = Self::new(0, 0, 0);\n\n    pub const fn new(\n        metal: u64,\n        crystal: u64,\n        fuel: u64,\n    ) -> Self {\n        Self {\n            metal,\n            crystal,\n            fuel,\n        }\n    }\n\n    pub const fn is_zero(self) -> bool {\n        self.metal == 0 && self.crystal == 0 && self.fuel == 0\n    }\n\n    pub const fn as_stock(self) -> ResourceStock {\n        ResourceStock::new(self.metal, self.crystal, self.fuel)\n    }\n}\n\nimpl From<ResourceStock> for ResourceCost {\n    fn from(stock: ResourceStock) -> Self {\n        Self::new(stock.metal, stock.crystal, stock.fuel)\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub struct ReservationId(u64);\n\nimpl ReservationId {\n    pub const fn new(value: u64) -> Self {\n        Self(value)\n    }\n\n    pub const fn value(self) -> u64 {\n        self.0\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ResourceReservation {\n    pub id: ReservationId,\n    pub cost: ResourceCost,\n}\n\nimpl ResourceReservation {\n    pub const fn new(\n        id: ReservationId,\n        cost: ResourceCost,\n    ) -> Self {\n        Self { id, cost }\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ResourceLedger {\n    stock: ResourceStock,\n    reservations: Vec<ResourceReservation>,\n    next_reservation_id: u64,\n}\n\nimpl ResourceLedger {\n    pub fn new(stock: ResourceStock) -> Self {\n        Self {\n            stock,\n            reservations: Vec::new(),\n            next_reservation_id: 1,\n        }\n    }\n\n    pub fn from_parts(\n        stock: ResourceStock,\n        reservations: Vec<ResourceReservation>,\n        next_reservation_id: u64,\n    ) -> Result<Self, ResourceLedgerError> {\n        let ledger = Self {\n            stock,\n            reservations,\n            next_reservation_id,\n        };\n        ledger.validate()?;\n        Ok(ledger)\n    }\n\n    pub const fn stock(&self) -> ResourceStock {\n        self.stock\n    }\n\n    pub fn reservations(&self) -> &[ResourceReservation] {\n        &self.reservations\n    }\n\n    pub const fn next_reservation_id(&self) -> u64 {\n        self.next_reservation_id\n    }\n\n    pub fn reserved_total(&self) -> ResourceStock {\n        self.reservations\n            .iter()\n            .try_fold(ResourceStock::ZERO, |total, reservation| {\n                total.checked_add(reservation.cost.as_stock())\n            })\n            .expect("validated reservation totals must not overflow")\n    }\n\n    pub fn available(&self) -> ResourceStock {\n        self.stock\n            .checked_sub(self.reserved_total())\n            .expect("validated reservations must be covered by stock")\n    }\n\n    pub fn credit(\n        &mut self,\n        amount: ResourceStock,\n    ) -> Result<(), ResourceLedgerError> {\n        let updated = self\n            .stock\n            .checked_add(amount)\n            .ok_or(ResourceLedgerError::AmountOverflow)?;\n        self.stock = updated;\n        Ok(())\n    }\n\n    pub fn debit(\n        &mut self,\n        cost: ResourceCost,\n    ) -> Result<(), ResourceLedgerError> {\n        let available = self.available();\n        if !available.can_cover(cost) {\n            return Err(ResourceLedgerError::InsufficientResources {\n                available,\n                requested: cost,\n            });\n        }\n\n        let updated = self\n            .stock\n            .checked_sub(cost)\n            .expect("available resources already cover the debit");\n        self.stock = updated;\n        Ok(())\n    }\n\n    pub fn reserve(\n        &mut self,\n        cost: ResourceCost,\n    ) -> Result<ReservationId, ResourceLedgerError> {\n        if cost.is_zero() {\n            return Err(ResourceLedgerError::EmptyReservation);\n        }\n\n        let available = self.available();\n        if !available.can_cover(cost) {\n            return Err(ResourceLedgerError::InsufficientResources {\n                available,\n                requested: cost,\n            });\n        }\n\n        let next_id = self\n            .next_reservation_id\n            .checked_add(1)\n            .ok_or(ResourceLedgerError::ReservationIdOverflow)?;\n        let id = ReservationId::new(self.next_reservation_id);\n\n        self.reservations\n            .push(ResourceReservation::new(id, cost));\n        self.next_reservation_id = next_id;\n        Ok(id)\n    }\n\n    pub fn commit(\n        &mut self,\n        id: ReservationId,\n    ) -> Result<ResourceCost, ResourceLedgerError> {\n        let index = self\n            .reservations\n            .iter()\n            .position(|reservation| reservation.id == id)\n            .ok_or(ResourceLedgerError::UnknownReservation(id))?;\n        let cost = self.reservations[index].cost;\n        let updated = self\n            .stock\n            .checked_sub(cost)\n            .expect("validated reservations are covered by stock");\n\n        self.stock = updated;\n        self.reservations.remove(index);\n        Ok(cost)\n    }\n\n    pub fn release(\n        &mut self,\n        id: ReservationId,\n    ) -> Result<ResourceCost, ResourceLedgerError> {\n        let index = self\n            .reservations\n            .iter()\n            .position(|reservation| reservation.id == id)\n            .ok_or(ResourceLedgerError::UnknownReservation(id))?;\n        Ok(self.reservations.remove(index).cost)\n    }\n\n    pub fn validate(&self) -> Result<(), ResourceLedgerError> {\n        let mut ids = BTreeSet::new();\n        let mut reserved = ResourceStock::ZERO;\n        let mut highest_id = 0;\n\n        for reservation in &self.reservations {\n            if reservation.cost.is_zero() {\n                return Err(ResourceLedgerError::EmptyReservation);\n            }\n            if !ids.insert(reservation.id) {\n                return Err(ResourceLedgerError::DuplicateReservation(\n                    reservation.id,\n                ));\n            }\n            highest_id = highest_id.max(reservation.id.value());\n            reserved = reserved\n                .checked_add(reservation.cost.as_stock())\n                .ok_or(ResourceLedgerError::AmountOverflow)?;\n        }\n\n        if !self.stock.can_cover(reserved) {\n            return Err(ResourceLedgerError::OverReserved {\n                stock: self.stock,\n                reserved,\n            });\n        }\n        if !self.reservations.is_empty()\n            && self.next_reservation_id <= highest_id\n        {\n            return Err(ResourceLedgerError::InvalidNextReservationId {\n                next: self.next_reservation_id,\n                highest_existing: highest_id,\n            });\n        }\n\n        Ok(())\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ResourceLedgerError {\n    EmptyReservation,\n    InsufficientResources {\n        available: ResourceStock,\n        requested: ResourceCost,\n    },\n    UnknownReservation(ReservationId),\n    DuplicateReservation(ReservationId),\n    OverReserved {\n        stock: ResourceStock,\n        reserved: ResourceStock,\n    },\n    InvalidNextReservationId {\n        next: u64,\n        highest_existing: u64,\n    },\n    AmountOverflow,\n    ReservationIdOverflow,\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct EnergyGrid {\n    production: u64,\n    consumption: u64,\n}\n\nimpl EnergyGrid {\n    pub const fn new(\n        production: u64,\n        consumption: u64,\n    ) -> Self {\n        Self {\n            production,\n            consumption,\n        }\n    }\n\n    pub const fn production(self) -> u64 {\n        self.production\n    }\n\n    pub const fn consumption(self) -> u64 {\n        self.consumption\n    }\n\n    pub const fn balance(self) -> i128 {\n        self.production as i128 - self.consumption as i128\n    }\n\n    pub const fn available_capacity(self) -> u64 {\n        self.production.saturating_sub(self.consumption)\n    }\n\n    pub const fn is_deficit(self) -> bool {\n        self.consumption > self.production\n    }\n\n    pub fn allocate(\n        &mut self,\n        amount: u64,\n    ) -> Result<(), EnergyError> {\n        let available = self.available_capacity();\n        if amount > available {\n            return Err(EnergyError::InsufficientCapacity {\n                available,\n                requested: amount,\n            });\n        }\n        self.consumption = self\n            .consumption\n            .checked_add(amount)\n            .ok_or(EnergyError::AmountOverflow)?;\n        Ok(())\n    }\n\n    pub fn release(\n        &mut self,\n        amount: u64,\n    ) -> Result<(), EnergyError> {\n        self.consumption = self\n            .consumption\n            .checked_sub(amount)\n            .ok_or(EnergyError::ReleaseExceedsConsumption {\n                consumption: self.consumption,\n                requested: amount,\n            })?;\n        Ok(())\n    }\n\n    pub fn set_production(&mut self, production: u64) {\n        self.production = production;\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum EnergyError {\n    InsufficientCapacity {\n        available: u64,\n        requested: u64,\n    },\n    ReleaseExceedsConsumption {\n        consumption: u64,\n        requested: u64,\n    },\n    AmountOverflow,\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct EconomicCost {\n    pub resources: ResourceCost,\n    /// Capacity that must be available; energy is not spent or stored.\n    pub energy: u64,\n}\n\nimpl EconomicCost {\n    pub const fn new(\n        resources: ResourceCost,\n        energy: u64,\n    ) -> Self {\n        Self { resources, energy }\n    }\n\n    pub fn can_start(\n        self,\n        ledger: &ResourceLedger,\n        grid: EnergyGrid,\n    ) -> bool {\n        ledger.available().can_cover(self.resources)\n            && grid.available_capacity() >= self.energy\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn insufficient_debit_is_atomic() {\n        let initial = ResourceStock::new(100, 40, 20);\n        let mut ledger = ResourceLedger::new(initial);\n\n        let result = ledger.debit(ResourceCost::new(120, 0, 0));\n\n        assert!(matches!(\n            result,\n            Err(ResourceLedgerError::InsufficientResources { .. })\n        ));\n        assert_eq!(ledger.stock(), initial);\n        assert_eq!(ledger.available(), initial);\n    }\n\n    #[test]\n    fn reservation_prevents_double_spending() {\n        let mut ledger =\n            ResourceLedger::new(ResourceStock::new(100, 50, 25));\n\n        let id = ledger\n            .reserve(ResourceCost::new(80, 20, 10))\n            .expect("first reservation is funded");\n        let second = ledger.reserve(ResourceCost::new(30, 10, 5));\n\n        assert!(matches!(\n            second,\n            Err(ResourceLedgerError::InsufficientResources { .. })\n        ));\n        assert_eq!(\n            ledger.available(),\n            ResourceStock::new(20, 30, 15)\n        );\n\n        ledger.commit(id).expect("reservation can commit");\n        assert_eq!(\n            ledger.stock(),\n            ResourceStock::new(20, 30, 15)\n        );\n        assert!(ledger.reservations().is_empty());\n    }\n\n    #[test]\n    fn released_reservation_restores_availability() {\n        let mut ledger =\n            ResourceLedger::new(ResourceStock::new(100, 50, 25));\n        let id = ledger\n            .reserve(ResourceCost::new(80, 20, 10))\n            .expect("reservation is funded");\n\n        ledger.release(id).expect("reservation can release");\n\n        assert_eq!(\n            ledger.available(),\n            ResourceStock::new(100, 50, 25)\n        );\n        assert_eq!(\n            ledger.stock(),\n            ResourceStock::new(100, 50, 25)\n        );\n    }\n\n    #[test]\n    fn credit_overflow_does_not_mutate_stock() {\n        let initial = ResourceStock::new(u64::MAX, 0, 0);\n        let mut ledger = ResourceLedger::new(initial);\n\n        assert_eq!(\n            ledger.credit(ResourceStock::new(1, 0, 0)),\n            Err(ResourceLedgerError::AmountOverflow)\n        );\n        assert_eq!(ledger.stock(), initial);\n    }\n\n    #[test]\n    fn energy_is_capacity_not_a_stock() {\n        let mut grid = EnergyGrid::new(80, 30);\n\n        grid.allocate(40).expect("capacity is available");\n        assert_eq!(grid.production(), 80);\n        assert_eq!(grid.consumption(), 70);\n        assert_eq!(grid.balance(), 10);\n\n        assert!(matches!(\n            grid.allocate(11),\n            Err(EnergyError::InsufficientCapacity {\n                available: 10,\n                requested: 11,\n            })\n        ));\n        assert_eq!(grid.consumption(), 70);\n    }\n\n    #[test]\n    fn configurable_cost_combines_all_resources_and_energy() {\n        let ledger =\n            ResourceLedger::new(ResourceStock::new(100, 80, 60));\n        let grid = EnergyGrid::new(50, 20);\n        let cost = EconomicCost::new(\n            ResourceCost::new(90, 70, 50),\n            25,\n        );\n\n        assert!(cost.can_start(&ledger, grid));\n        assert!(!EconomicCost::new(\n            ResourceCost::new(90, 70, 50),\n            31,\n        )\n        .can_start(&ledger, grid));\n    }\n}\n'
PERSISTENCE_RS = '// MVP-011: persist resource reservations and energy capacity.\nuse galactic_domain::{\n    ColonyId, EnergyGrid, FactionId, PlanetId, ReservationId,\n    ResourceCost, ResourceLedger, ResourceLedgerError,\n    ResourceReservation, ResourceStock, SystemId, UniverseConfig,\n    UniverseId, generate_universe,\n};\nuse galactic_sim::{\n    BuildingLevels, ColonyState, FactionKind, FactionState, GameState,\n    PlanetKnowledge, PlanetResourceProfile, SelectionTarget, Simulation,\n    SimulationBuildError, StrategicClock, StrategicClockError,\n    StrategicTick, SystemKnowledge, TimeSpeed,\n};\n\npub const SAVE_VERSION: u32 = 6;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub factions: Vec<FactionSave>,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub system_knowledge: Vec<SystemKnowledge>,\n    pub planet_knowledge: Vec<PlanetKnowledge>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionSave {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub reservations: Vec<ResourceReservation>,\n    pub next_reservation_id: u64,\n    pub energy_production: u64,\n    pub energy_consumption: u64,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidResourceLedger {\n        colony_id: ColonyId,\n        error: ResourceLedgerError,\n    },\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(\n    simulation: &Simulation,\n) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint:\n                universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            factions: state\n                .factions\n                .iter()\n                .map(|faction| FactionSave {\n                    id: faction.id,\n                    name: faction.name.clone(),\n                    kind: faction.kind,\n                })\n                .collect(),\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos: state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            system_knowledge: state.system_knowledge.clone(),\n            planet_knowledge: state.planet_knowledge.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.resources.stock(),\n                    reservations: colony\n                        .resources\n                        .reservations()\n                        .to_vec(),\n                    next_reservation_id: colony\n                        .resources\n                        .next_reservation_id(),\n                    energy_production: colony.energy.production(),\n                    energy_consumption: colony.energy.consumption(),\n                    buildings: colony.buildings,\n                    resource_profile: colony.resource_profile,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(\n    save: &SaveGame,\n) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(SaveError::UnsupportedVersion(save.version));\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version\n        != save.universe.generation_version\n    {\n        return Err(SaveError::GenerationVersionMismatch {\n            expected: universe.generation_version,\n            found: save.universe.generation_version,\n        });\n    }\n    if universe.generation_fingerprint\n        != save.universe.generation_fingerprint\n    {\n        return Err(\n            SaveError::GenerationFingerprintMismatch {\n                expected: universe.generation_fingerprint,\n                found: save.universe.generation_fingerprint,\n            },\n        );\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let colonies = save\n        .state\n        .colonies\n        .iter()\n        .map(|colony| {\n            let resources = ResourceLedger::from_parts(\n                colony.stock,\n                colony.reservations.clone(),\n                colony.next_reservation_id,\n            )\n            .map_err(|error| {\n                SaveError::InvalidResourceLedger {\n                    colony_id: colony.id,\n                    error,\n                }\n            })?;\n\n            Ok(ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                resources,\n                energy: EnergyGrid::new(\n                    colony.energy_production,\n                    colony.energy_consumption,\n                ),\n                buildings: colony.buildings,\n                resource_profile: colony.resource_profile,\n            })\n        })\n        .collect::<Result<Vec<_>, SaveError>>()?;\n\n    let state = GameState {\n        version: save.state.version,\n        factions: save\n            .state\n            .factions\n            .iter()\n            .map(|faction| FactionState {\n                id: faction.id,\n                name: faction.name.clone(),\n                kind: faction.kind,\n            })\n            .collect(),\n        player_faction: save.state.player_faction,\n        colonies,\n        system_knowledge: save.state.system_knowledge.clone(),\n        planet_knowledge: save.state.planet_knowledge.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state)\n        .map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::{\n        ResourceCost, ResourceReservation, SystemId, UniverseConfig,\n    };\n    use galactic_sim::{\n        GAME_STATE_VERSION, GameCommand, KnowledgeLevel,\n        STRATEGIC_TICK_NANOS, StrategicTick, TimeSpeed,\n    };\n\n    use super::*;\n\n    #[test]\n    fn snapshot_round_trips_economy_knowledge_and_clock() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let target = simulation\n            .universe_repository()\n            .neighboring_systems(SystemId::from_index(0))\n            .into_iter()\n            .next()\n            .expect("home has a neighbor");\n        simulation.apply_command(GameCommand::SelectSystem(target));\n        simulation.apply_command(\n            GameCommand::DebugAdvanceSelectedKnowledge,\n        );\n        simulation.advance(Duration::from_millis(125));\n        simulation\n            .apply_command(GameCommand::SetSpeed(TimeSpeed::X4));\n\n        let colony = simulation\n            .state_mut()\n            .colonies\n            .first_mut()\n            .expect("home colony exists");\n        colony\n            .resources\n            .reserve(ResourceCost::new(50, 25, 10))\n            .expect("test reservation is funded");\n        colony\n            .energy\n            .allocate(10)\n            .expect("energy capacity is available");\n\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("save is compatible");\n\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(\n            restored.state().system_knowledge_level(target),\n            KnowledgeLevel::Probed\n        );\n        assert_eq!(\n            restored.state().clock.current_tick(),\n            StrategicTick::new(1)\n        );\n    }\n\n    #[test]\n    fn snapshot_contains_ledger_and_energy_balance() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let save = snapshot_from_simulation(&simulation);\n        let colony = save\n            .state\n            .colonies\n            .first()\n            .expect("home colony is saved");\n\n        assert_eq!(save.state.version, GAME_STATE_VERSION);\n        assert_eq!(colony.stock, ResourceStock::new(600, 300, 220));\n        assert_eq!(colony.energy_production, 80);\n        assert_eq!(colony.energy_consumption, 30);\n    }\n\n    #[test]\n    fn invalid_over_reserved_ledger_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        let colony = save\n            .state\n            .colonies\n            .first_mut()\n            .expect("home colony is saved");\n        colony.reservations.push(ResourceReservation::new(\n            ReservationId::new(1),\n            ResourceCost::new(700, 0, 0),\n        ));\n        colony.next_reservation_id = 2;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidResourceLedger { .. })\n        ));\n    }\n\n    #[test]\n    fn modified_fingerprint_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.universe.generation_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::GenerationFingerprintMismatch { .. })\n        ));\n    }\n\n    #[test]\n    fn invalid_clock_remainder_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.state.clock.remainder_nanos =\n            STRATEGIC_TICK_NANOS;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidClock(\n                StrategicClockError::RemainderOutOfRange(_)\n            ))\n        ));\n    }\n\n    #[test]\n    fn unsupported_save_version_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::default());\n        let mut save = snapshot_from_simulation(&simulation);\n        save.version = 999;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::UnsupportedVersion(999))\n        ));\n    }\n}\n'
HOME_INSPECTOR = 'fn home_inspector_content(\n    simulation: &Simulation,\n) -> InspectorContent {\n    let state = simulation.state();\n    let Some(faction) = state.player_faction_state() else {\n        return inspector_error("Faction joueur invalide");\n    };\n    let Some(colony) = state.player_home_colony() else {\n        return inspector_error("Colonie mère introuvable");\n    };\n    let Some(system) =\n        simulation.universe().system(colony.system_id)\n    else {\n        return inspector_error("Système mère introuvable");\n    };\n    let Some(planet) =\n        simulation.universe_repository().planet(colony.planet_id)\n    else {\n        return inspector_error("Planète mère introuvable");\n    };\n\n    InspectorContent {\n        level: Some(KnowledgeLevel::Colonized),\n        badge: knowledge_badge_fr(KnowledgeLevel::Colonized)\n            .to_string(),\n        title: format!("{} — {}", system.name, planet.name),\n        body: format!(\n            "Faction : {}\\nHabitabilité : {}%\\n\\n{}\\n\\nPOTENTIEL EXACT\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nINFRASTRUCTURE\\nMines : {}/{}/{}\\nCentrale : {}\\nEntrepôt : {}\\nConstruction : {}\\nLaboratoire : {}\\nChantier : {}",\n            faction.name,\n            planet.habitability,\n            colony_economy_text(colony),\n            colony.resource_profile.metal,\n            colony.resource_profile.crystal,\n            colony.resource_profile.fuel,\n            colony.resource_profile.energy,\n            colony.buildings.metal_mine,\n            colony.buildings.crystal_extractor,\n            colony.buildings.fuel_refinery,\n            colony.buildings.power_plant,\n            colony.buildings.warehouse,\n            colony.buildings.construction_center,\n            colony.buildings.research_lab,\n            colony.buildings.shipyard,\n        ),\n        hint: "Colonie active : ressources et énergie sont exactes."\n            .to_string(),\n    }\n}\n\nfn colony_economy_text(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    let stock = colony.resources.stock();\n    let available = colony.resources.available();\n    let reserved = colony.resources.reserved_total();\n\n    format!(\n        "STOCKS EXACTS\\nTotal — Métal {}  Cristal {}  Carburant {}\\nDisponible — Métal {}  Cristal {}  Carburant {}\\nRéservé — Métal {}  Cristal {}  Carburant {}\\n\\nÉNERGIE — CAPACITÉ\\nProduction : {}\\nConsommation : {}\\nBilan : {:+}",\n        stock.metal,\n        stock.crystal,\n        stock.fuel,\n        available.metal,\n        available.crystal,\n        available.fuel,\n        reserved.metal,\n        reserved.crystal,\n        reserved.fuel,\n        colony.energy.production(),\n        colony.energy.consumption(),\n        colony.energy.balance(),\n    )\n}\n'
PLANET_INSPECTOR = 'fn planet_inspector_content(\n    simulation: &Simulation,\n    selected_system_id: SystemId,\n    planet_id: galactic_domain::PlanetId,\n) -> InspectorContent {\n    let state = simulation.state();\n    let Some((system_id, planet)) =\n        simulation.universe_repository().planet_location(planet_id)\n    else {\n        return inspector_error(&format!(\n            "Référence planète invalide : {}",\n            planet_id.index(),\n        ));\n    };\n    let Some(system) = simulation.universe().system(system_id) else {\n        return inspector_error("Système de la planète introuvable");\n    };\n\n    let level = state.planet_knowledge_level(planet_id);\n    let colony = state.colony_on_planet(planet_id);\n    let system_label =\n        if state.system_knowledge_level(system_id).reveals_identity() {\n            system.name.clone()\n        } else {\n            format!("Signal {}", system_id.index())\n        };\n    let selection_note = if selected_system_id == system_id {\n        "Sélection : cohérente"\n    } else {\n        "Sélection : recoupée avec le système réel"\n    };\n\n    let (title, mut body) = match level {\n        KnowledgeLevel::Unknown => (\n            "Corps inconnu".to_string(),\n            format!(\n                "Système : {}\\nNom : ???\\nType : ???\\nHabitabilité : ???\\nPotentiel : ???\\nLunes : ???\\n{}",\n                system_label, selection_note,\n            ),\n        ),\n        KnowledgeLevel::Detected => (\n            format!("Corps détecté {}", planet_id.index()),\n            format!(\n                "Système : {}\\nNom : ???\\nType : ???\\nHabitabilité : ???\\nPotentiel : analyse requise\\nLunes : non recensées\\n{}",\n                system_label, selection_note,\n            ),\n        ),\n        KnowledgeLevel::Probed => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité estimée : {}\\nPotentiel : analyse requise\\nLunes : non recensées\\n{}",\n                system_label,\n                planet.kind,\n                habitability_estimate(planet.habitability),\n                selection_note,\n            ),\n        ),\n        KnowledgeLevel::Analyzed => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité exacte : {}%\\nStatut : non colonisée\\nPotentiel : aucune valeur économique générée pour ce corps\\nLunes : aucune donnée disponible\\n{}",\n                system_label,\n                planet.kind,\n                planet.habitability,\n                selection_note,\n            ),\n        ),\n        KnowledgeLevel::Colonized => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité exacte : {}%\\nStatut : {}\\nLunes : aucune donnée disponible\\n{}",\n                system_label,\n                planet.kind,\n                planet.habitability,\n                colony\n                    .map(|value| value.name.as_str())\n                    .unwrap_or("colonie non référencée"),\n                selection_note,\n            ),\n        ),\n    };\n\n    if let Some(colony) = colony {\n        body.push_str(&format!(\n            "\\n\\n{}\\n\\nPOTENTIEL EXACT\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nINFRASTRUCTURE\\nMines : {}/{}/{}\\nCentrale : {}\\nEntrepôt : {}\\nConstruction : {}\\nLaboratoire : {}\\nChantier : {}",\n            colony_economy_text(colony),\n            colony.resource_profile.metal,\n            colony.resource_profile.crystal,\n            colony.resource_profile.fuel,\n            colony.resource_profile.energy,\n            colony.buildings.metal_mine,\n            colony.buildings.crystal_extractor,\n            colony.buildings.fuel_refinery,\n            colony.buildings.power_plant,\n            colony.buildings.warehouse,\n            colony.buildings.construction_center,\n            colony.buildings.research_lab,\n            colony.buildings.shipyard,\n        ));\n    }\n\n    InspectorContent {\n        level: Some(level),\n        badge: knowledge_badge_fr(level).to_string(),\n        title,\n        body,\n        hint: planet_knowledge_hint(level).to_string(),\n    }\n}\n'
DOC_APPEND = "\n## MVP-011 — Registre de ressources et énergie\n\nLe modèle économique distingue maintenant deux concepts :\n\n```text\nRessources stockées\n├── Métal\n├── Cristal\n└── Carburant\n\nÉnergie\n├── capacité produite\n├── capacité consommée\n└── bilan = production - consommation\n```\n\nL'énergie n'est plus un stock dépensable. Une allocation augmente la\nconsommation sans réduire la production.\n\n`ResourceLedger` possède :\n\n- un stock total ;\n- une liste de réservations identifiées ;\n- un stock disponible calculé ;\n- des opérations atomiques de crédit et débit ;\n- `reserve`, `commit` et `release` ;\n- une validation des doublons, sur-réservations et identifiants.\n\nUne dépense ou réservation insuffisamment financée ne modifie aucune donnée.\nLes réservations sont soustraites du disponible et empêchent les doubles\ndépenses avant leur engagement définitif.\n\n`EconomicCost` combine un coût en Métal/Cristal/Carburant et une capacité\nénergétique requise. Les catalogues de bâtiments et crafts pourront utiliser\nce format dans les étapes suivantes.\n\nLa colonie initiale commence avec :\n\n- 600 Métal ;\n- 300 Cristal ;\n- 220 Carburant ;\n- 80 unités de production énergétique ;\n- 30 unités de consommation énergétique.\n\nLe HUD de colonie affiche stock total, disponible, réservé, production,\nconsommation et bilan énergétique.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 5` ;\n- `SAVE_VERSION = 6`.\n\nMVP-012 ajoutera la production par tick et les capacités maximales de\nstockage. MVP-013 ajoutera les coûts réels du catalogue de bâtiments.\n"


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
                candidate / "crates/galactic_domain/src/resources.rs"
            ).exists()
            and (
                candidate / "crates/galactic_client/src/lib.rs"
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
        "MVP-010-B analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    resources = (
        root / "crates/galactic_domain/src/resources.rs"
    ).read_text(encoding="utf-8")
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    legacy_resources = "pub energy: i32" in resources
    mvp11_resources = (
        "// MVP-011: atomic stored-resource ledger" in resources
        and "pub struct ResourceLedger" in resources
    )
    if not legacy_resources and not mvp11_resources:
        failures.append(
            "contrat de ressources attendu absent"
        )
    for marker in (
        "// MVP-010-B: screen-space picking",
        "fn information_panel_content(",
        "fn home_inspector_content(",
        "fn planet_inspector_content(",
    ):
        if marker not in client:
            failures.append(
                f"marqueur client absent : {marker}"
            )

    if failures:
        raise SystemExit(
            "Baseline MVP-010-B incohérente :\n- "
            + "\n- ".join(failures)
        )


def cargo_edition(root: Path) -> str:
    cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(
        r"(?m)^edition\s*=\s*\"([^\"]+)\"",
        cargo,
    )
    return match.group(1) if match else "2024"


def format_rust(
    root: Path,
    content: str,
) -> str:
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
        return normalize(temporary.read_text(encoding="utf-8"))
    finally:
        temporary.unlink(missing_ok=True)


def patch_starting(source: str) -> str:
    if "initial_energy: EnergyGrid" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-009: configurable starting knowledge and home-world state",
        "// MVP-011: configurable starting economy and knowledge",
        1,
    )
    source = replace_once(
        source,
        "use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, SystemId};",
        "use galactic_domain::{\n"
        "    ColonyId, EnergyGrid, FactionId, PlanetId, "
        "ResourceStock, SystemId,\n"
        "};",
        "imports de starting.rs",
    )
    source = replace_once(
        source,
        "    pub initial_stock: ResourceStock,\n"
        "    pub buildings: BuildingLevels,\n",
        "    pub initial_stock: ResourceStock,\n"
        "    pub initial_energy: EnergyGrid,\n"
        "    pub buildings: BuildingLevels,\n",
        "énergie initiale",
    )
    source = replace_once(
        source,
        "                initial_stock: ResourceStock::new(600, 300, 220, 80),\n"
        "                buildings: BuildingLevels::MVP_START,\n",
        "                initial_stock: ResourceStock::new(600, 300, 220),\n"
        "                initial_energy: EnergyGrid::new(80, 30),\n"
        "                buildings: BuildingLevels::MVP_START,\n",
        "configuration économique MVP",
    )
    source = replace_once(
        source,
        "        if !self.home_colony.resource_profile.is_viable() {\n"
        "            return Err(StartingScenarioError::InvalidResourceProfile);\n"
        "        }\n",
        "        if !self.home_colony.resource_profile.is_viable() {\n"
        "            return Err(StartingScenarioError::InvalidResourceProfile);\n"
        "        }\n"
        "        if self.home_colony.initial_energy.is_deficit() {\n"
        "            return Err(StartingScenarioError::InitialEnergyDeficit);\n"
        "        }\n",
        "validation énergétique",
    )
    source = replace_once(
        source,
        "    InvalidResourceProfile,\n"
        "    ExplicitUnknownKnowledge,\n",
        "    InvalidResourceProfile,\n"
        "    InitialEnergyDeficit,\n"
        "    ExplicitUnknownKnowledge,\n",
        "erreur énergie initiale",
    )
    source = replace_once(
        source,
        "        scenario.home_colony.initial_stock = ResourceStock::new(999, 888, 777, 66);\n"
        "        scenario.home_colony.buildings.research_lab = 1;\n",
        "        scenario.home_colony.initial_stock = ResourceStock::new(999, 888, 777);\n"
        "        scenario.home_colony.initial_energy = EnergyGrid::new(120, 45);\n"
        "        scenario.home_colony.buildings.research_lab = 1;\n",
        "test starting configurable",
    )
    return normalize(source)


def patch_state(source: str) -> str:
    if "pub resources: ResourceLedger" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-009: persistent progressive knowledge for systems and planets",
        "// MVP-011: persistent knowledge and colony economy",
        1,
    )
    source = replace_once(
        source,
        "use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId};",
        "use galactic_domain::{\n"
        "    ColonyId, EnergyGrid, FactionId, PlanetId, "
        "ResourceLedger, Route, SystemId,\n"
        "};",
        "imports de state.rs",
    )
    source = source.replace(
        "/// Version 4 replaces the binary known-system list with monotone knowledge\n"
        "/// levels for systems and planets.\n"
        "pub const GAME_STATE_VERSION: u32 = 4;",
        "/// Version 5 adds atomic resource ledgers and an energy grid per colony.\n"
        "pub const GAME_STATE_VERSION: u32 = 5;",
        1,
    )
    source = replace_once(
        source,
        "                stock: home.initial_stock,\n"
        "                buildings: home.buildings,\n",
        "                resources: ResourceLedger::new(home.initial_stock),\n"
        "                energy: home.initial_energy,\n"
        "                buildings: home.buildings,\n",
        "économie de la colonie initiale",
    )
    source = replace_once(
        source,
        "    pub stock: ResourceStock,\n"
        "    pub buildings: BuildingLevels,\n",
        "    pub resources: ResourceLedger,\n"
        "    pub energy: EnergyGrid,\n"
        "    pub buildings: BuildingLevels,\n",
        "champs économiques de ColonyState",
    )
    insertion = r"""
    #[test]
    fn home_colony_has_atomic_resources_and_energy_capacity() {
        let universe =
            UniverseRepository::generate(UniverseConfig::mvp());
        let state = GameState::new(&universe);
        let colony =
            state.player_home_colony().expect("home colony exists");

        assert_eq!(
            colony.resources.stock(),
            galactic_domain::ResourceStock::new(600, 300, 220)
        );
        assert_eq!(colony.resources.available(), colony.resources.stock());
        assert_eq!(colony.energy.production(), 80);
        assert_eq!(colony.energy.consumption(), 30);
        assert_eq!(colony.energy.balance(), 50);
    }

"""
    marker = "    #[test]\n    fn non_home_planets_start_as_detected_only()"
    if marker not in source:
        raise SystemExit(
            "Point d'insertion du test économique introuvable."
        )
    source = source.replace(marker, insertion + marker, 1)
    return normalize(source)


def patch_simulation(source: str) -> str:
    if "ResourceLedger::new(ResourceStock::new(999, 888, 777))" in source:
        return normalize(source)

    source = replace_once(
        source,
        "    use galactic_domain::{ColonyId, PlanetId, ResourceStock, SystemId, UniverseConfig};",
        "    use galactic_domain::{\n"
        "        ColonyId, PlanetId, ResourceLedger, "
        "ResourceStock, SystemId, UniverseConfig,\n"
        "    };",
        "imports des tests simulation",
    )
    source = replace_once(
        source,
        "            .stock = ResourceStock::new(999, 888, 777, 666);",
        "            .resources = ResourceLedger::new(\n"
        "            ResourceStock::new(999, 888, 777),\n"
        "        );",
        "mutation économique du test",
    )
    return normalize(source)


def patch_client(source: str) -> str:
    if "fn colony_economy_text(" in source:
        return normalize(source)

    home_pattern = re.compile(
        r"fn home_inspector_content\(.*?\n\}\n\n"
        r"(?=fn system_inspector_content)",
        flags=re.DOTALL,
    )
    source, home_count = home_pattern.subn(
        HOME_INSPECTOR.rstrip() + "\n\n",
        source,
        count=1,
    )
    if home_count != 1:
        raise SystemExit(
            "Fonction home_inspector_content introuvable."
        )

    planet_pattern = re.compile(
        r"fn planet_inspector_content\(.*?\n\}\n\n"
        r"(?=fn inspector_error)",
        flags=re.DOTALL,
    )
    source, planet_count = planet_pattern.subn(
        PLANET_INSPECTOR.rstrip() + "\n\n",
        source,
        count=1,
    )
    if planet_count != 1:
        raise SystemExit(
            "Fonction planet_inspector_content introuvable."
        )

    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-011 — Registre de ressources et énergie" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_domain/src/resources.rs":
            normalize(RESOURCES_RS),
        root / "crates/galactic_persistence/src/lib.rs":
            normalize(PERSISTENCE_RS),
    }
    for path, after in replacements.items():
        before = path.read_text(encoding="utf-8")
        after = format_rust(root, after)
        if before != after:
            updates.append(Update(path, before, after))

    for path, patcher in (
        (
            root / "crates/galactic_sim/src/starting.rs",
            patch_starting,
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
    replacements = {update.path: update.after for update in updates}
    failures = []

    legacy_constructor = re.compile(
        r"ResourceStock::new\(\s*[^,()]+,\s*"
        r"[^,()]+,\s*[^,()]+,\s*[^,()]+\s*\)"
    )
    for path in (root / "crates").rglob("*.rs"):
        content = replacements.get(
            path,
            path.read_text(encoding="utf-8"),
        )
        if legacy_constructor.search(content):
            failures.append(
                f"constructeur ResourceStock à quatre valeurs : "
                f"{path.relative_to(root)}"
            )
        if ".stock.energy" in content:
            failures.append(
                f"énergie encore stockée : {path.relative_to(root)}"
            )

    if failures:
        raise SystemExit(
            "Migration économique incomplète :\n- "
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
        print("MVP-011 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp011-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        backup = backup_root / relative
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(update.path, backup)
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
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument("--force", action="store_true")
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
        "\nMVP-011 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""
Applique MVP-014 au dépôt Galactic.

Baseline analysée :
    8f08c529daf3caf622c80b04ee37d897e8c9fa8a
    feat show productions

Le script :
- simplifie le tableau de ressources selon les retours joueur ;
- ajoute une file de construction de cinq ordres par colonie ;
- réserve les ressources au lancement ;
- valide niveau, prérequis, énergie et ressources ;
- progresse avec les ticks stratégiques ;
- applique les niveaux et effets à l’achèvement ;
- ajoute un panneau de huit améliorations ;
- sauvegarde la file et sa progression.

Usage :
    python tools/apply_mvp_014.py --dry-run
    python tools/apply_mvp_014.py
    python tools/apply_mvp_014.py --skip-checks
    python tools/apply_mvp_014.py --root /chemin/vers/galactic

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
    "8f08c529daf3caf622c80b04ee37d897e8c9fa8a"
)

CONSTRUCTION_RS = '// MVP-014: deterministic construction queues and building upgrades.\nuse std::collections::{BTreeSet, VecDeque};\n\nuse galactic_domain::{\n    ColonyId, EnergyGrid, ReservationId, ResourceCost,\n    ResourceLedgerError, ResourceStock,\n};\n\nuse crate::{\n    BuildingCatalogError, BuildingEffect, BuildingKind,\n    BuildingLevels, ColonyState, GameState, PlanetResourceProfile,\n    StrategicDuration, default_building_catalog,\n};\n\npub const MAX_CONSTRUCTION_QUEUE: usize = 5;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ConstructionOrder {\n    pub kind: BuildingKind,\n    pub target_level: u8,\n    pub cost: ResourceCost,\n    pub reservation_id: ReservationId,\n    pub total_ticks: u64,\n    pub remaining_ticks: u64,\n}\n\n#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct ConstructionQueue {\n    orders: VecDeque<ConstructionOrder>,\n}\n\nimpl ConstructionQueue {\n    pub fn orders(\n        &self,\n    ) -> impl Iterator<Item = &ConstructionOrder> {\n        self.orders.iter()\n    }\n\n    pub fn active(&self) -> Option<&ConstructionOrder> {\n        self.orders.front()\n    }\n\n    pub fn len(&self) -> usize {\n        self.orders.len()\n    }\n\n    pub fn is_empty(&self) -> bool {\n        self.orders.is_empty()\n    }\n\n    fn push(&mut self, order: ConstructionOrder) {\n        self.orders.push_back(order);\n    }\n\n    fn pop_front(&mut self) -> ConstructionOrder {\n        self.orders\n            .pop_front()\n            .expect("non-empty queue has an active order")\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct BuildingUpgradeQuote {\n    pub colony_id: ColonyId,\n    pub kind: BuildingKind,\n    pub current_level: u8,\n    pub target_level: u8,\n    pub cost: ResourceCost,\n    pub duration_ticks: u64,\n    pub projected_energy_production: u64,\n    pub projected_energy_consumption: u64,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ConstructionQueued {\n    pub colony_id: ColonyId,\n    pub order: ConstructionOrder,\n    pub queue_length: usize,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ConstructionCompleted {\n    pub colony_id: ColonyId,\n    pub kind: BuildingKind,\n    pub new_level: u8,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ConstructionRejected {\n    pub colony_id: ColonyId,\n    pub kind: BuildingKind,\n    pub error: ConstructionError,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConstructionError {\n    UnknownColony(ColonyId),\n    NotPlayerOwned(ColonyId),\n    QueueFull {\n        maximum: usize,\n    },\n    MaximumLevel {\n        kind: BuildingKind,\n        level: u8,\n    },\n    Catalog(BuildingCatalogError),\n    InsufficientResources {\n        available: ResourceStock,\n        cost: ResourceCost,\n    },\n    EnergyDeficit {\n        production: u64,\n        consumption: u64,\n    },\n    Reservation(ResourceLedgerError),\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ConstructionQueueError {\n    TooManyOrders {\n        found: usize,\n        maximum: usize,\n    },\n    InvalidTargetLevel {\n        kind: BuildingKind,\n        expected: u8,\n        found: u8,\n    },\n    InvalidDuration {\n        kind: BuildingKind,\n        total_ticks: u64,\n        remaining_ticks: u64,\n    },\n    UnexpectedTotalDuration {\n        kind: BuildingKind,\n        expected: u64,\n        found: u64,\n    },\n    InvalidCost {\n        kind: BuildingKind,\n        expected: ResourceCost,\n        found: ResourceCost,\n    },\n    MissingReservation {\n        reservation_id: ReservationId,\n    },\n    ReservationCostMismatch {\n        reservation_id: ReservationId,\n        expected: ResourceCost,\n        found: ResourceCost,\n    },\n    DuplicateReservation {\n        reservation_id: ReservationId,\n    },\n    Catalog(BuildingCatalogError),\n    EnergyDeficit {\n        production: u64,\n        consumption: u64,\n    },\n}\n\npub fn building_upgrade_quote(\n    state: &GameState,\n    colony_id: ColonyId,\n    kind: BuildingKind,\n) -> Result<BuildingUpgradeQuote, ConstructionError> {\n    let colony = state\n        .colony(colony_id)\n        .ok_or(ConstructionError::UnknownColony(colony_id))?;\n    if colony.faction != state.player_faction {\n        return Err(ConstructionError::NotPlayerOwned(colony_id));\n    }\n    if colony.construction_queue.len() >= MAX_CONSTRUCTION_QUEUE {\n        return Err(ConstructionError::QueueFull {\n            maximum: MAX_CONSTRUCTION_QUEUE,\n        });\n    }\n\n    let catalog = default_building_catalog();\n    let definition = catalog.definition(kind);\n    let projected = projected_building_levels(colony);\n    let current_level = projected.level(kind);\n    if current_level >= definition.max_level {\n        return Err(ConstructionError::MaximumLevel {\n            kind,\n            level: current_level,\n        });\n    }\n\n    let target_level = current_level + 1;\n    let mut after = projected;\n    after.set_level(kind, target_level);\n    catalog\n        .validate_levels(after)\n        .map_err(ConstructionError::Catalog)?;\n\n    let cost = definition\n        .cost_for_level(target_level)\n        .map_err(ConstructionError::Catalog)?;\n    let available = colony.resources.available();\n    if !available.can_cover(cost) {\n        return Err(ConstructionError::InsufficientResources {\n            available,\n            cost,\n        });\n    }\n\n    let grid = catalog.energy_grid_for_levels(after);\n    let effective_energy =\n        effective_energy_production(grid, colony.resource_profile);\n    if effective_energy < grid.consumption() {\n        return Err(ConstructionError::EnergyDeficit {\n            production: effective_energy,\n            consumption: grid.consumption(),\n        });\n    }\n\n    let base_duration = definition\n        .duration_for_level(target_level)\n        .map_err(ConstructionError::Catalog)?;\n    let duration_ticks =\n        adjusted_construction_duration(base_duration, projected);\n\n    Ok(BuildingUpgradeQuote {\n        colony_id,\n        kind,\n        current_level,\n        target_level,\n        cost,\n        duration_ticks,\n        projected_energy_production: effective_energy,\n        projected_energy_consumption: grid.consumption(),\n    })\n}\n\npub fn enqueue_building_upgrade(\n    state: &mut GameState,\n    colony_id: ColonyId,\n    kind: BuildingKind,\n) -> Result<ConstructionQueued, ConstructionError> {\n    let quote = building_upgrade_quote(state, colony_id, kind)?;\n    let colony = state\n        .colony_mut(colony_id)\n        .ok_or(ConstructionError::UnknownColony(colony_id))?;\n    let reservation_id = colony\n        .resources\n        .reserve(quote.cost)\n        .map_err(ConstructionError::Reservation)?;\n\n    let order = ConstructionOrder {\n        kind,\n        target_level: quote.target_level,\n        cost: quote.cost,\n        reservation_id,\n        total_ticks: quote.duration_ticks,\n        remaining_ticks: quote.duration_ticks,\n    };\n    colony.construction_queue.push(order);\n\n    Ok(ConstructionQueued {\n        colony_id,\n        order,\n        queue_length: colony.construction_queue.len(),\n    })\n}\n\npub fn advance_colony_construction(\n    colony: &mut ColonyState,\n    ticks: StrategicDuration,\n) -> Result<Vec<ConstructionCompleted>, ResourceLedgerError> {\n    let mut remaining = ticks.ticks();\n    let mut completed = Vec::new();\n\n    while remaining > 0 && !colony.construction_queue.is_empty() {\n        let active = colony\n            .construction_queue\n            .orders\n            .front_mut()\n            .expect("non-empty queue has an active order");\n        let step = remaining.min(active.remaining_ticks);\n        active.remaining_ticks -= step;\n        remaining -= step;\n\n        if active.remaining_ticks > 0 {\n            continue;\n        }\n\n        let order = colony.construction_queue.pop_front();\n        colony.resources.commit(order.reservation_id)?;\n        colony\n            .buildings\n            .set_level(order.kind, order.target_level);\n        colony.energy = default_building_catalog()\n            .energy_grid_for_levels(colony.buildings);\n        completed.push(ConstructionCompleted {\n            colony_id: colony.id,\n            kind: order.kind,\n            new_level: order.target_level,\n        });\n    }\n\n    Ok(completed)\n}\n\npub fn validate_construction_queue(\n    colony: &ColonyState,\n) -> Result<(), ConstructionQueueError> {\n    if colony.construction_queue.len() > MAX_CONSTRUCTION_QUEUE {\n        return Err(ConstructionQueueError::TooManyOrders {\n            found: colony.construction_queue.len(),\n            maximum: MAX_CONSTRUCTION_QUEUE,\n        });\n    }\n\n    let catalog = default_building_catalog();\n    let mut projected = colony.buildings;\n    let mut reservations = BTreeSet::new();\n\n    for order in colony.construction_queue.orders() {\n        let expected_level = projected.level(order.kind).saturating_add(1);\n        if order.target_level != expected_level {\n            return Err(\n                ConstructionQueueError::InvalidTargetLevel {\n                    kind: order.kind,\n                    expected: expected_level,\n                    found: order.target_level,\n                },\n            );\n        }\n        if order.total_ticks == 0\n            || order.remaining_ticks == 0\n            || order.remaining_ticks > order.total_ticks\n        {\n            return Err(ConstructionQueueError::InvalidDuration {\n                kind: order.kind,\n                total_ticks: order.total_ticks,\n                remaining_ticks: order.remaining_ticks,\n            });\n        }\n\n        let definition = catalog.definition(order.kind);\n        let expected_duration = adjusted_construction_duration(\n            definition\n                .duration_for_level(order.target_level)\n                .map_err(ConstructionQueueError::Catalog)?,\n            projected,\n        );\n        if order.total_ticks != expected_duration {\n            return Err(\n                ConstructionQueueError::UnexpectedTotalDuration {\n                    kind: order.kind,\n                    expected: expected_duration,\n                    found: order.total_ticks,\n                },\n            );\n        }\n\n        let expected_cost = definition\n            .cost_for_level(order.target_level)\n            .map_err(ConstructionQueueError::Catalog)?;\n        if order.cost != expected_cost {\n            return Err(ConstructionQueueError::InvalidCost {\n                kind: order.kind,\n                expected: expected_cost,\n                found: order.cost,\n            });\n        }\n        if !reservations.insert(order.reservation_id) {\n            return Err(\n                ConstructionQueueError::DuplicateReservation {\n                    reservation_id: order.reservation_id,\n                },\n            );\n        }\n        let reservation = colony\n            .resources\n            .reservations()\n            .iter()\n            .find(|reservation| {\n                reservation.id == order.reservation_id\n            })\n            .ok_or(\n                ConstructionQueueError::MissingReservation {\n                    reservation_id: order.reservation_id,\n                },\n            )?;\n        if reservation.cost != order.cost {\n            return Err(\n                ConstructionQueueError::ReservationCostMismatch {\n                    reservation_id: order.reservation_id,\n                    expected: order.cost,\n                    found: reservation.cost,\n                },\n            );\n        }\n\n        projected.set_level(order.kind, order.target_level);\n        catalog\n            .validate_levels(projected)\n            .map_err(ConstructionQueueError::Catalog)?;\n        let grid = catalog.energy_grid_for_levels(projected);\n        let effective =\n            effective_energy_production(grid, colony.resource_profile);\n        if effective < grid.consumption() {\n            return Err(ConstructionQueueError::EnergyDeficit {\n                production: effective,\n                consumption: grid.consumption(),\n            });\n        }\n    }\n\n    Ok(())\n}\n\npub fn projected_building_levels(\n    colony: &ColonyState,\n) -> BuildingLevels {\n    let mut projected = colony.buildings;\n    for order in colony.construction_queue.orders() {\n        projected.set_level(order.kind, order.target_level);\n    }\n    projected\n}\n\nfn adjusted_construction_duration(\n    base_ticks: u64,\n    levels: BuildingLevels,\n) -> u64 {\n    let catalog = default_building_catalog();\n    let definition =\n        catalog.definition(BuildingKind::ConstructionCenter);\n    let bonus_per_level = match definition.effect {\n        BuildingEffect::ConstructionSpeed {\n            permille_per_level,\n        } => permille_per_level,\n        _ => 0,\n    };\n    let speed_per_mille = 1_000_u64.saturating_add(\n        bonus_per_level.saturating_mul(u64::from(\n            levels.level(BuildingKind::ConstructionCenter),\n        )),\n    );\n    base_ticks\n        .saturating_mul(1_000)\n        .div_ceil(speed_per_mille)\n        .max(1)\n}\n\nfn effective_energy_production(\n    grid: EnergyGrid,\n    profile: PlanetResourceProfile,\n) -> u64 {\n    let value = u128::from(grid.production())\n        .saturating_mul(u128::from(profile.energy))\n        / 100;\n    value.min(u128::from(u64::MAX)) as u64\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::UniverseConfig;\n\n    use crate::{Simulation, STRATEGIC_TICK_NANOS};\n\n    use super::*;\n\n    #[test]\n    fn queueing_reserves_resources_atomically() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n        let before = simulation\n            .state()\n            .colony(colony_id)\n            .expect("colony exists")\n            .resources\n            .available();\n\n        let queued = enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::MetalMine,\n        )\n        .expect("upgrade is affordable");\n\n        let colony = simulation\n            .state()\n            .colony(colony_id)\n            .expect("colony exists");\n        assert_eq!(colony.construction_queue.len(), 1);\n        assert_eq!(\n            colony.resources.available(),\n            before\n                .checked_sub(queued.order.cost)\n                .expect("the quote was affordable")\n        );\n        assert_eq!(\n            colony.resources.stock(),\n            ResourceStock::new(600, 300, 220)\n        );\n    }\n\n    #[test]\n    fn construction_completion_commits_and_levels_up() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n        let queued = enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::MetalMine,\n        )\n        .expect("upgrade is affordable");\n\n        let duration = Duration::from_nanos(\n            queued\n                .order\n                .total_ticks\n                .saturating_mul(STRATEGIC_TICK_NANOS),\n        );\n        simulation.advance(duration);\n\n        let colony = simulation\n            .state()\n            .colony(colony_id)\n            .expect("colony exists");\n        assert_eq!(\n            colony.buildings.level(BuildingKind::MetalMine),\n            2\n        );\n        assert!(colony.construction_queue.is_empty());\n        assert!(colony.resources.reservations().is_empty());\n    }\n\n    #[test]\n    fn construction_progress_is_chunk_independent() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n        let queued = enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::MetalMine,\n        )\n        .expect("upgrade is affordable");\n\n        let mut batched = simulation\n            .state()\n            .colony(colony_id)\n            .expect("colony exists")\n            .clone();\n        let mut incremental = batched.clone();\n\n        advance_colony_construction(\n            &mut batched,\n            StrategicDuration::from_ticks(\n                queued.order.total_ticks,\n            ),\n        )\n        .expect("reservation commits");\n\n        let mut remaining = queued.order.total_ticks;\n        while remaining > 0 {\n            let step = remaining.min(7);\n            advance_colony_construction(\n                &mut incremental,\n                StrategicDuration::from_ticks(step),\n            )\n            .expect("reservation commits");\n            remaining -= step;\n        }\n\n        assert_eq!(batched, incremental);\n    }\n\n    #[test]\n    fn prerequisites_explain_shipyard_rejection() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists");\n\n        assert!(matches!(\n            building_upgrade_quote(\n                simulation.state(),\n                colony.id,\n                BuildingKind::Shipyard,\n            ),\n            Err(ConstructionError::Catalog(\n                BuildingCatalogError::UnsatisfiedPrerequisite {\n                    ..\n                }\n            ))\n        ));\n    }\n\n    #[test]\n    fn queued_levels_allow_sequential_prerequisites() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n\n        simulation\n            .state_mut()\n            .colony_mut(colony_id)\n            .expect("home colony exists")\n            .resources\n            .credit(ResourceStock::new(1_000, 1_000, 500))\n            .expect("test funding fits u64");\n\n        enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::ConstructionCenter,\n        )\n        .expect("center level 2 can be queued");\n        enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::MetalMine,\n        )\n        .expect("metal level 2 can be queued");\n        enqueue_building_upgrade(\n            simulation.state_mut(),\n            colony_id,\n            BuildingKind::CrystalExtractor,\n        )\n        .expect("crystal level 2 can be queued");\n\n        assert!(building_upgrade_quote(\n            simulation.state(),\n            colony_id,\n            BuildingKind::Shipyard,\n        )\n        .is_ok());\n    }\n}\n'
COMMAND_RS = 'use galactic_domain::{ColonyId, PlanetId, SystemId};\n\nuse crate::{BuildingKind, TimeSpeed};\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameCommand {\n    TogglePause,\n    SetSpeed(TimeSpeed),\n    SelectSystem(SystemId),\n    SelectPlanet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n    ClearSelection,\n    QueueBuildingUpgrade {\n        colony_id: ColonyId,\n        kind: BuildingKind,\n    },\n    /// Temporary validation command until the probe mission loop is added.\n    DebugAdvanceSelectedKnowledge,\n}\n'
EVENT_RS = 'use galactic_domain::{PlanetId, SystemId};\n\nuse crate::{\n    ColonyProductionReport, ConstructionCompleted,\n    ConstructionQueued, ConstructionRejected, KnowledgeChange,\n    StrategicDuration, StrategicTick, TimeSpeed,\n};\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub enum SelectionTarget {\n    #[default]\n    None,\n    System(SystemId),\n    Planet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameEvent {\n    SpeedChanged(TimeSpeed),\n    SelectionChanged(SelectionTarget),\n    KnowledgeChanged(KnowledgeChange),\n    TicksAdvanced {\n        ticks: StrategicDuration,\n        current_tick: StrategicTick,\n    },\n    ProductionRefreshed(ColonyProductionReport),\n    ConstructionQueued(ConstructionQueued),\n    ConstructionCompleted(ConstructionCompleted),\n    ConstructionRejected(ConstructionRejected),\n}\n'
PERSISTENCE_RS = '// MVP-014: persist construction queues and reserved upgrade costs.\nuse galactic_domain::{\n    ColonyId, EnergyGrid, FactionId, PlanetId,\n    ResourceLedger, ResourceLedgerError, ResourceReservation,\n    ResourceStock, SystemId, UniverseConfig, UniverseId,\n    generate_universe,\n};\nuse galactic_sim::{\n    BuildingLevels, ColonyState, ConstructionQueue,\n    FactionKind, FactionState, GameState,\n    PRODUCTION_REFRESH_TICKS, PlanetKnowledge,\n    PlanetResourceProfile, ProductionRemainder,\n    ProductionRemainderError, SelectionTarget, Simulation,\n    SimulationBuildError, StrategicClock, StrategicClockError,\n    StrategicTick, SystemKnowledge, TimeSpeed,\n    default_building_catalog,\n};\n\npub const SAVE_VERSION: u32 = 9;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub catalog_version: u32,\n    pub catalog_fingerprint: u64,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub factions: Vec<FactionSave>,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub system_knowledge: Vec<SystemKnowledge>,\n    pub planet_knowledge: Vec<PlanetKnowledge>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionSave {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub reservations: Vec<ResourceReservation>,\n    pub next_reservation_id: u64,\n    pub energy_production: u64,\n    pub energy_consumption: u64,\n    pub production_remainder_metal: u16,\n    pub production_remainder_crystal: u16,\n    pub production_remainder_fuel: u16,\n    pub production_pending_ticks: u16,\n    pub construction_queue: ConstructionQueue,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    CatalogVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    CatalogFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidResourceLedger {\n        colony_id: ColonyId,\n        error: ResourceLedgerError,\n    },\n    InvalidProductionRemainder {\n        colony_id: ColonyId,\n        error: ProductionRemainderError,\n    },\n    InvalidPendingProductionTicks {\n        colony_id: ColonyId,\n        found: u16,\n    },\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(\n    simulation: &Simulation,\n) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n    let catalog = default_building_catalog();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        catalog_version: catalog.version(),\n        catalog_fingerprint: catalog.fingerprint(),\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint:\n                universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            factions: state\n                .factions\n                .iter()\n                .map(|faction| FactionSave {\n                    id: faction.id,\n                    name: faction.name.clone(),\n                    kind: faction.kind,\n                })\n                .collect(),\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos:\n                    state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            system_knowledge:\n                state.system_knowledge.clone(),\n            planet_knowledge:\n                state.planet_knowledge.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.resources.stock(),\n                    reservations: colony\n                        .resources\n                        .reservations()\n                        .to_vec(),\n                    next_reservation_id: colony\n                        .resources\n                        .next_reservation_id(),\n                    energy_production:\n                        colony.energy.production(),\n                    energy_consumption:\n                        colony.energy.consumption(),\n                    production_remainder_metal:\n                        colony\n                            .production_remainder\n                            .metal_milli(),\n                    production_remainder_crystal:\n                        colony\n                            .production_remainder\n                            .crystal_milli(),\n                    production_remainder_fuel:\n                        colony\n                            .production_remainder\n                            .fuel_milli(),\n                    production_pending_ticks:\n                        colony.production_pending_ticks,\n                    construction_queue:\n                        colony.construction_queue.clone(),\n                    buildings: colony.buildings,\n                    resource_profile:\n                        colony.resource_profile,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(\n    save: &SaveGame,\n) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(\n            SaveError::UnsupportedVersion(save.version),\n        );\n    }\n\n    let catalog = default_building_catalog();\n    if save.catalog_version != catalog.version() {\n        return Err(SaveError::CatalogVersionMismatch {\n            expected: catalog.version(),\n            found: save.catalog_version,\n        });\n    }\n    if save.catalog_fingerprint != catalog.fingerprint() {\n        return Err(\n            SaveError::CatalogFingerprintMismatch {\n                expected: catalog.fingerprint(),\n                found: save.catalog_fingerprint,\n            },\n        );\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version\n        != save.universe.generation_version\n    {\n        return Err(\n            SaveError::GenerationVersionMismatch {\n                expected: universe.generation_version,\n                found:\n                    save.universe.generation_version,\n            },\n        );\n    }\n    if universe.generation_fingerprint\n        != save.universe.generation_fingerprint\n    {\n        return Err(\n            SaveError::GenerationFingerprintMismatch {\n                expected:\n                    universe.generation_fingerprint,\n                found:\n                    save.universe\n                        .generation_fingerprint,\n            },\n        );\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let colonies = save\n        .state\n        .colonies\n        .iter()\n        .map(|colony| {\n            let resources = ResourceLedger::from_parts(\n                colony.stock,\n                colony.reservations.clone(),\n                colony.next_reservation_id,\n            )\n            .map_err(|error| {\n                SaveError::InvalidResourceLedger {\n                    colony_id: colony.id,\n                    error,\n                }\n            })?;\n            let production_remainder =\n                ProductionRemainder::from_parts(\n                    colony.production_remainder_metal,\n                    colony.production_remainder_crystal,\n                    colony.production_remainder_fuel,\n                )\n                .map_err(|error| {\n                    SaveError::InvalidProductionRemainder {\n                        colony_id: colony.id,\n                        error,\n                    }\n                })?;\n            if u64::from(colony.production_pending_ticks)\n                >= PRODUCTION_REFRESH_TICKS\n            {\n                return Err(\n                    SaveError::InvalidPendingProductionTicks {\n                        colony_id: colony.id,\n                        found:\n                            colony.production_pending_ticks,\n                    },\n                );\n            }\n\n            Ok(ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                resources,\n                energy: EnergyGrid::new(\n                    colony.energy_production,\n                    colony.energy_consumption,\n                ),\n                production_remainder,\n                production_pending_ticks:\n                    colony.production_pending_ticks,\n                construction_queue:\n                    colony.construction_queue.clone(),\n                buildings: colony.buildings,\n                resource_profile:\n                    colony.resource_profile,\n            })\n        })\n        .collect::<Result<Vec<_>, SaveError>>()?;\n\n    let state = GameState {\n        version: save.state.version,\n        factions: save\n            .state\n            .factions\n            .iter()\n            .map(|faction| FactionState {\n                id: faction.id,\n                name: faction.name.clone(),\n                kind: faction.kind,\n            })\n            .collect(),\n        player_faction: save.state.player_faction,\n        colonies,\n        system_knowledge:\n            save.state.system_knowledge.clone(),\n        planet_knowledge:\n            save.state.planet_knowledge.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state)\n        .map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::UniverseConfig;\n    use galactic_sim::{\n        BuildingKind, GAME_STATE_VERSION, GameCommand,\n    };\n\n    use super::*;\n\n    #[test]\n    fn construction_queue_survives_round_trip() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n        simulation.apply_command(\n            GameCommand::QueueBuildingUpgrade {\n                colony_id,\n                kind: BuildingKind::MetalMine,\n            },\n        );\n        simulation.advance(Duration::from_secs(2));\n\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("save is compatible");\n\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(\n            restored\n                .state()\n                .colony(colony_id)\n                .expect("colony exists")\n                .construction_queue\n                .len(),\n            1\n        );\n    }\n\n    #[test]\n    fn catalog_changes_are_detected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.catalog_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(\n                SaveError::CatalogFingerprintMismatch {\n                    ..\n                }\n            )\n        ));\n    }\n\n    #[test]\n    fn state_and_save_versions_match_mvp_014() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let save = snapshot_from_simulation(&simulation);\n\n        assert_eq!(save.version, SAVE_VERSION);\n        assert_eq!(\n            save.state.version,\n            GAME_STATE_VERSION\n        );\n    }\n}\n'
CLIENT_TYPES = '\n#[derive(Component)]\nstruct ConstructionPanelRoot;\n\n#[derive(Component)]\nstruct ConstructionQueueText;\n\n#[derive(Component)]\nstruct ConstructionButton {\n    kind: galactic_sim::BuildingKind,\n}\n\ntype ConstructionButtonInteractionQuery<\'w, \'s> = Query<\n    \'w,\n    \'s,\n    (&\'static Interaction, &\'static ConstructionButton),\n    (Changed<Interaction>, With<Button>),\n>;\n\n#[derive(Component)]\nstruct ConstructionButtonText {\n    kind: galactic_sim::BuildingKind,\n}\n'
CLIENT_SPAWN = '\nfn spawn_construction_panel(commands: &mut Commands) {\n    commands\n        .spawn((\n            Node {\n                position_type: PositionType::Absolute,\n                left: Val::Px(290.0),\n                right: Val::Px(372.0),\n                bottom: Val::Px(218.0),\n                padding: UiRect::all(Val::Px(9.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(7.0),\n                ),\n                flex_direction: FlexDirection::Column,\n                row_gap: Val::Px(6.0),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.012, 0.020, 0.028, 0.96,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.58, 0.72, 0.76, 0.40),\n            ),\n            Visibility::Hidden,\n            Interaction::None,\n            UiPointerBlocker,\n            ConstructionPanelRoot,\n        ))\n        .with_children(|root| {\n            root.spawn((\n                Text::new("CONSTRUCTION"),\n                ui_text_font(12.0),\n                TextColor(Color::srgb(\n                    0.78, 0.92, 0.96,\n                )),\n            ));\n            root.spawn((\n                Text::new("File vide"),\n                ui_text_font(10.0),\n                TextColor(Color::srgb(\n                    0.68, 0.76, 0.80,\n                )),\n                ConstructionQueueText,\n            ));\n\n            for pair in galactic_sim::BuildingKind::ALL\n                .chunks(2)\n            {\n                root.spawn((\n                    Node {\n                        width: Val::Percent(100.0),\n                        flex_direction: FlexDirection::Row,\n                        column_gap: Val::Px(6.0),\n                        ..default()\n                    },\n                ))\n                .with_children(|row| {\n                    for kind in pair {\n                        spawn_construction_button(\n                            row,\n                            *kind,\n                        );\n                    }\n                });\n            }\n        });\n}\n\nfn spawn_construction_button(\n    parent: &mut ChildSpawnerCommands,\n    kind: galactic_sim::BuildingKind,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                flex_grow: 1.0,\n                flex_basis: Val::Px(0.0),\n                min_height: Val::Px(44.0),\n                padding: UiRect::all(Val::Px(6.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(action_button_color(\n                true,\n                false,\n                &Interaction::None,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.58, 0.72, 0.76, 0.30),\n            ),\n            ConstructionButton { kind },\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new(""),\n                ui_text_font(10.0),\n                TextColor(Color::srgb(\n                    0.88, 0.94, 0.96,\n                )),\n                ConstructionButtonText { kind },\n            ));\n        });\n}\n'
CLIENT_SYSTEMS = '\nfn handle_construction_buttons(\n    mut simulation: ResMut<SimulationResource>,\n    interactions: ConstructionButtonInteractionQuery,\n) {\n    for (interaction, button) in &interactions {\n        if *interaction != Interaction::Pressed {\n            continue;\n        }\n        let colony_id =\n            selected_colony_for_resource_dashboard(\n                simulation.simulation(),\n            )\n            .map(|colony| colony.id);\n        let Some(colony_id) = colony_id else {\n            continue;\n        };\n        apply_simulation_command(\n            &mut simulation,\n            GameCommand::QueueBuildingUpgrade {\n                colony_id,\n                kind: button.kind,\n            },\n        );\n    }\n}\n\nfn update_construction_panel(\n    simulation: Res<SimulationResource>,\n    mut roots: Query<\n        &mut Visibility,\n        With<ConstructionPanelRoot>,\n    >,\n    mut queue_texts: Query<\n        &mut Text,\n        (\n            With<ConstructionQueueText>,\n            Without<ConstructionButtonText>,\n        ),\n    >,\n    mut buttons: Query<(\n        &ConstructionButton,\n        &Interaction,\n        &mut BackgroundColor,\n        &mut Outline,\n    )>,\n    mut labels: Query<\n        (\n            &ConstructionButtonText,\n            &mut Text,\n            &mut TextColor,\n        ),\n        Without<ConstructionQueueText>,\n    >,\n) {\n    let Some(colony) =\n        selected_colony_for_resource_dashboard(\n            simulation.simulation(),\n        )\n    else {\n        for mut visibility in &mut roots {\n            *visibility = Visibility::Hidden;\n        }\n        return;\n    };\n\n    for mut visibility in &mut roots {\n        *visibility = Visibility::Visible;\n    }\n\n    let catalog = galactic_sim::default_building_catalog();\n    for mut text in &mut queue_texts {\n        text.0 = construction_queue_label(colony);\n    }\n\n    for (button, interaction, mut background, mut outline)\n        in &mut buttons\n    {\n        let available = galactic_sim::building_upgrade_quote(\n            simulation.simulation().state(),\n            colony.id,\n            button.kind,\n        )\n        .is_ok();\n        background.0 = action_button_color(\n            available,\n            false,\n            interaction,\n        );\n        outline.color = action_button_outline(\n            available,\n            false,\n            interaction,\n        );\n    }\n\n    for (label, mut text, mut color) in &mut labels {\n        let definition = catalog.definition(label.kind);\n        match galactic_sim::building_upgrade_quote(\n            simulation.simulation().state(),\n            colony.id,\n            label.kind,\n        ) {\n            Ok(quote) => {\n                text.0 = format!(\n                    "{}  {}→{}\\nCoût: {}  •  {}",\n                    definition.name,\n                    quote.current_level,\n                    quote.target_level,\n                    construction_cost_label(quote.cost),\n                    format_strategic_duration(\n                        galactic_sim::StrategicDuration::from_ticks(\n                            quote.duration_ticks,\n                        ),\n                    ),\n                );\n                color.0 = Color::srgb(\n                    0.88, 0.94, 0.96,\n                );\n            }\n            Err(error) => {\n                let current = galactic_sim::projected_building_levels(\n                    colony,\n                )\n                .level(label.kind);\n                text.0 = format!(\n                    "{}  niveau {}\\n{}",\n                    definition.name,\n                    current,\n                    construction_error_text(error),\n                );\n                color.0 = Color::srgb(\n                    0.64, 0.66, 0.66,\n                );\n            }\n        }\n    }\n}\n\nfn construction_queue_label(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    let catalog = galactic_sim::default_building_catalog();\n    let Some(active) = colony.construction_queue.active()\n    else {\n        return "File vide — sélectionne une amélioration."\n            .to_string();\n    };\n    let active_name =\n        &catalog.definition(active.kind).name;\n    let waiting =\n        colony.construction_queue.len().saturating_sub(1);\n\n    format!(\n        "EN COURS — {} niveau {}  •  restant {}  •  en attente {}",\n        active_name,\n        active.target_level,\n        format_strategic_duration(\n            galactic_sim::StrategicDuration::from_ticks(\n                active.remaining_ticks,\n            ),\n        ),\n        waiting,\n    )\n}\n\nfn construction_error_text(\n    error: galactic_sim::ConstructionError,\n) -> String {\n    match error {\n        galactic_sim::ConstructionError::UnknownColony(_)\n        | galactic_sim::ConstructionError::NotPlayerOwned(_) => {\n            "Colonie indisponible".to_string()\n        }\n        galactic_sim::ConstructionError::QueueFull {\n            maximum,\n        } => {\n            format!("File pleine ({maximum})")\n        }\n        galactic_sim::ConstructionError::MaximumLevel {\n            ..\n        } => "Niveau maximal".to_string(),\n        galactic_sim::ConstructionError::InsufficientResources {\n            available,\n            cost,\n        } => {\n            format!(\n                "Manque: {}",\n                construction_missing_resources_label(\n                    available, cost,\n                ),\n            )\n        }\n        galactic_sim::ConstructionError::EnergyDeficit {\n            production,\n            consumption,\n        } => format!(\n            "Énergie insuffisante {production}/{consumption}",\n        ),\n        galactic_sim::ConstructionError::Catalog(\n            galactic_sim::BuildingCatalogError::\n                UnsatisfiedPrerequisite {\n                    prerequisite,\n                    required,\n                    ..\n                },\n        ) => {\n            let name = &galactic_sim::default_building_catalog()\n                .definition(prerequisite)\n                .name;\n            format!("Requiert {name} niv. {required}")\n        }\n        galactic_sim::ConstructionError::Catalog(_) => {\n            "Règle catalogue invalide".to_string()\n        }\n        galactic_sim::ConstructionError::Reservation(_) => {\n            "Réservation impossible".to_string()\n        }\n    }\n}\n\nfn construction_cost_label(\n    cost: galactic_domain::ResourceCost,\n) -> String {\n    construction_resource_amounts_label(\n        cost.as_stock(),\n        "gratuit",\n    )\n}\n\nfn construction_missing_resources_label(\n    available: ResourceStock,\n    cost: galactic_domain::ResourceCost,\n) -> String {\n    construction_resource_amounts_label(\n        cost.as_stock().saturating_sub(available),\n        "0",\n    )\n}\n\nfn construction_resource_amounts_label(\n    resources: ResourceStock,\n    empty_label: &str,\n) -> String {\n    let mut parts = Vec::new();\n    append_resource_amount(\n        &mut parts,\n        resources.metal,\n        "métal",\n    );\n    append_resource_amount(\n        &mut parts,\n        resources.crystal,\n        "cristal",\n    );\n    append_resource_amount(\n        &mut parts,\n        resources.fuel,\n        "carburant",\n    );\n\n    if parts.is_empty() {\n        empty_label.to_string()\n    } else {\n        parts.join(", ")\n    }\n}\n\nfn append_resource_amount(\n    parts: &mut Vec<String>,\n    amount: u64,\n    label: &str,\n) {\n    if amount > 0 {\n        parts.push(format!("{amount} {label}"));\n    }\n}\n'
DOC_APPEND = '\n## MVP-014 — File de construction et améliorations\n\nChaque colonie possède une file de construction séquentielle de cinq ordres\nmaximum.\n\nLancer une amélioration :\n\n1. calcule le prochain niveau en tenant compte des ordres déjà en file ;\n2. valide le niveau maximal et les prérequis ;\n3. vérifie l’énergie projetée ;\n4. vérifie les ressources disponibles ;\n5. réserve le coût dans `ResourceLedger` ;\n6. ajoute l’ordre à la file.\n\nLes ressources restent dans le stock total mais disparaissent du disponible.\nElles sont consommées définitivement lorsque la construction se termine.\n\nLa progression utilise exclusivement les ticks stratégiques. Pause, x1, x2 et\nx4 produisent donc le même résultat métier. Une file peut terminer plusieurs\nordres dans un même lot de ticks.\n\nÀ l’achèvement :\n\n- la réservation est engagée ;\n- le niveau du bâtiment augmente ;\n- le réseau énergétique de la colonie est recalculé ;\n- la production et le stockage utilisent automatiquement le nouveau niveau ;\n- un événement `ConstructionCompleted` est émis.\n\nL’interface affiche les huit bâtiments en quatre rangées de deux boutons. Un\nbouton disponible montre niveau actuel, niveau cible, coût et durée. Un bouton\nbloqué explique directement la cause : ressources manquantes, prérequis,\nénergie, niveau maximal ou file pleine.\n\nLa file, sa progression et les réservations survivent aux sauvegardes.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 8` ;\n- `SAVE_VERSION = 9`.\n\n### Simplification du tableau de ressources\n\nLes informations de debug ont été retirées :\n\n- dernier crédit ;\n- prochain crédit ;\n- cadence de rafraîchissement ;\n- libellés `STABLE` et `PRESQUE PLEIN`.\n\nLa jauge suffit à communiquer le remplissage. Les avertissements textuels sont\nconservés uniquement lorsqu’ils ont une conséquence métier :\n\n- `PLEIN — PRODUCTION BLOQUÉE` ;\n- `DÉFICIT ÉNERGÉTIQUE`.\n'


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
            end="" if result.stdout.endswith("\n")
            else "\n",
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
                / "crates/galactic_client/src/lib.rs"
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
        "MVP-013-B analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")
    state = (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "// MVP-013-B: compact resource dashboard",
        "Dernier crédit",
        "prochain crédit",
        "ResourceDashboardRoot",
    ):
        if marker not in client:
            failures.append(
                f"marqueur UI absent : {marker}"
            )
    for marker in (
        "GAME_STATE_VERSION: u32 = 7",
        "pub production_pending_ticks: u16",
    ):
        if marker not in state:
            failures.append(
                f"marqueur état absent : {marker}"
            )

    if failures:
        raise SystemExit(
            "Baseline MVP-013-B incohérente :\n- "
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
    if "pub mod construction;" not in source:
        source = replace_once(
            source,
            "pub mod command;\n",
            "pub mod command;\npub mod construction;\n",
            "module construction",
        )
    if "pub use construction::*;" not in source:
        source = replace_once(
            source,
            "pub use command::*;\n",
            "pub use command::*;\npub use construction::*;\n",
            "export construction",
        )
    return normalize(source)


def patch_starting(source: str) -> str:
    if "pub fn set_level(" in source:
        return normalize(source)
    marker = (
        "    pub fn total_levels(self) -> u32 {\n"
    )
    method = (
        "    pub fn set_level(\n"
        "        &mut self,\n"
        "        kind: BuildingKind,\n"
        "        level: u8,\n"
        "    ) {\n"
        "        match kind {\n"
        "            BuildingKind::MetalMine => "
        "self.metal_mine = level,\n"
        "            BuildingKind::CrystalExtractor => "
        "self.crystal_extractor = level,\n"
        "            BuildingKind::FuelRefinery => "
        "self.fuel_refinery = level,\n"
        "            BuildingKind::PowerPlant => "
        "self.power_plant = level,\n"
        "            BuildingKind::Warehouse => "
        "self.warehouse = level,\n"
        "            BuildingKind::ConstructionCenter => "
        "self.construction_center = level,\n"
        "            BuildingKind::ResearchLab => "
        "self.research_lab = level,\n"
        "            BuildingKind::Shipyard => "
        "self.shipyard = level,\n"
        "        }\n"
        "    }\n\n"
    )
    if marker not in source:
        raise SystemExit(
            "Point d'insertion BuildingLevels introuvable."
        )
    return normalize(
        source.replace(marker, method + marker, 1)
    )


def patch_state(source: str) -> str:
    if "pub construction_queue: ConstructionQueue" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-013: persistent catalog-driven production windows",
        "// MVP-014: persistent production and construction queues",
        1,
    )
    source = replace_once(
        source,
        "    BuildingLevels, KnowledgeChange,",
        "    BuildingLevels, ConstructionQueue, "
        "KnowledgeChange,",
        "import ConstructionQueue",
    )
    source = source.replace(
        "/// Version 7 adds five-second production windows.\n"
        "pub const GAME_STATE_VERSION: u32 = 7;",
        "/// Version 8 adds persistent construction queues.\n"
        "pub const GAME_STATE_VERSION: u32 = 8;",
        1,
    )
    source = replace_once(
        source,
        "                production_pending_ticks: 0,\n"
        "                buildings: home.buildings,\n",
        "                production_pending_ticks: 0,\n"
        "                construction_queue: "
        "ConstructionQueue::default(),\n"
        "                buildings: home.buildings,\n",
        "file initiale",
    )
    source = replace_once(
        source,
        "    pub production_pending_ticks: u16,\n"
        "    pub buildings: BuildingLevels,\n",
        "    pub production_pending_ticks: u16,\n"
        "    pub construction_queue: "
        "ConstructionQueue,\n"
        "    pub buildings: BuildingLevels,\n",
        "champ file de construction",
    )
    return normalize(source)


def patch_simulation(source: str) -> str:
    if "enqueue_building_upgrade" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-013: catalog-driven simulation and production windows",
        "// MVP-014: catalog-driven production and construction",
        1,
    )
    source = replace_once(
        source,
        "    BuildingCatalogError, FactionKind,",
        "    BuildingCatalogError, "
        "ConstructionQueueError, FactionKind,",
        "import erreur file",
    )
    source = replace_once(
        source,
        "    storage_capacity,\n",
        "    advance_colony_construction, "
        "enqueue_building_upgrade,\n"
        "    storage_capacity, "
        "validate_construction_queue,\n",
        "imports construction",
    )
    source = replace_once(
        source,
        "    InvalidProductionWindow {\n"
        "        colony_id: ColonyId,\n"
        "        pending_ticks: u16,\n"
        "    },\n",
        "    InvalidProductionWindow {\n"
        "        colony_id: ColonyId,\n"
        "        pending_ticks: u16,\n"
        "    },\n"
        "    InvalidConstructionQueue {\n"
        "        colony_id: ColonyId,\n"
        "        error: ConstructionQueueError,\n"
        "    },\n",
        "erreur validation construction",
    )
    source = replace_once(
        source,
        "            GameCommand::ClearSelection => "
        "self.set_selection(SelectionTarget::None),\n"
        "            GameCommand::DebugAdvanceSelectedKnowledge =>",
        "            GameCommand::ClearSelection => "
        "self.set_selection(SelectionTarget::None),\n"
        "            GameCommand::QueueBuildingUpgrade {\n"
        "                colony_id,\n"
        "                kind,\n"
        "            } => match enqueue_building_upgrade(\n"
        "                &mut self.state,\n"
        "                colony_id,\n"
        "                kind,\n"
        "            ) {\n"
        "                Ok(queued) => vec![\n"
        "                    GameEvent::ConstructionQueued(queued),\n"
        "                ],\n"
        "                Err(error) => vec![\n"
        "                    GameEvent::ConstructionRejected(\n"
        "                        crate::ConstructionRejected {\n"
        "                            colony_id,\n"
        "                            kind,\n"
        "                            error,\n"
        "                        },\n"
        "                    ),\n"
        "                ],\n"
        "            },\n"
        "            GameCommand::DebugAdvanceSelectedKnowledge =>",
        "commande d'amélioration",
    )
    old_loop = (
        "        for colony in &mut self.state.colonies {\n"
        "            if let Some(report) = "
        "queue_colony_production(colony, advance.ticks) {\n"
        "                events.push("
        "GameEvent::ProductionRefreshed(report));\n"
        "            }\n"
        "        }\n"
    )
    new_loop = (
        "        for colony in &mut self.state.colonies {\n"
        "            if let Some(report) = "
        "queue_colony_production(colony, advance.ticks) {\n"
        "                events.push("
        "GameEvent::ProductionRefreshed(report));\n"
        "            }\n"
        "            let completed = "
        "advance_colony_construction(\n"
        "                colony,\n"
        "                advance.ticks,\n"
        "            )\n"
        "            .expect(\n"
        "                \"validated construction reservations "
        "must commit\",\n"
        "            );\n"
        "            events.extend(\n"
        "                completed.into_iter().map(\n"
        "                    GameEvent::ConstructionCompleted,\n"
        "                ),\n"
        "            );\n"
        "        }\n"
    )
    source = replace_once(
        source,
        old_loop,
        new_loop,
        "progression construction",
    )
    marker = (
        "        if let Err(error) = "
        "colony.resources.validate() {\n"
    )
    insert = (
        "        if let Err(error) = "
        "validate_construction_queue(colony) {\n"
        "            return Err(\n"
        "                SimulationBuildError::"
        "InvalidConstructionQueue {\n"
        "                    colony_id: colony.id,\n"
        "                    error,\n"
        "                },\n"
        "            );\n"
        "        }\n"
    )
    if marker not in source:
        raise SystemExit(
            "Validation ResourceLedger introuvable."
        )
    source = source.replace(
        marker,
        insert + marker,
        1,
    )
    return normalize(source)


def patch_client(source: str) -> str:
    if "ConstructionPanelRoot" in source:
        return normalize(source)

    # Imports and removed debug-only delta state.
    source = re.sub(
        r"(?m)^[ \t]*\.init_resource::<"
        r"ResourceDeltaState>\(\)\n",
        "",
        source,
    )
    source = source.replace(
        "                capture_resource_deltas,\n",
        "",
        1,
    )
    source = replace_once(
        source,
        "                handle_action_buttons,\n"
        "                update_action_buttons,\n",
        "                handle_action_buttons,\n"
        "                handle_construction_buttons,\n"
        "                update_action_buttons,\n"
        "                update_construction_panel,\n",
        "systèmes construction UI",
    )

    delta_pattern = re.compile(
        r"// MVP-013-B: compact resource dashboard.*?"
        r"(?=#\[derive\(Component\)\]\n"
        r"struct ResourceDashboardRoot;)",
        flags=re.DOTALL,
    )
    match = delta_pattern.search(source)
    if match is None:
        raise SystemExit(
            "Bloc ResourceDeltaState introuvable."
        )
    replacement = r"""// MVP-014: compact resources and construction UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceHudKind {
    Metal,
    Crystal,
    Fuel,
    Energy,
}

impl ResourceHudKind {
    const ALL: [Self; 4] = [
        Self::Metal,
        Self::Crystal,
        Self::Fuel,
        Self::Energy,
    ];

    const fn title(self) -> &'static str {
        match self {
            Self::Metal => "MÉTAL",
            Self::Crystal => "CRISTAL",
            Self::Fuel => "CARBURANT",
            Self::Energy => "ÉNERGIE",
        }
    }

    const fn help(self) -> &'static str {
        match self {
            Self::Metal | Self::Crystal | Self::Fuel => {
                "Total = stock physique. Disponible = total - réservé. \
Capacité = limite de stockage. Une jauge pleine signifie que la production est bloquée."
            }
            Self::Energy => {
                "L’énergie est une capacité, pas un stock. Disponible = production effective - \
consommation. Un déficit ralentit tous les extracteurs."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceHudStatus {
    Normal,
    Reserved,
    NearlyFull,
    Full,
    Deficit,
}

"""
    source = (
        source[: match.start()]
        + replacement
        + source[match.end() :]
    )

    source = replace_once(
        source,
        "#[derive(Component)]\n"
        "struct ResourceHudGaugeFill {\n"
        "    kind: ResourceHudKind,\n"
        "}\n",
        "#[derive(Component)]\n"
        "struct ResourceHudGaugeFill {\n"
        "    kind: ResourceHudKind,\n"
        "}\n\n"
        + CLIENT_TYPES.rstrip()
        + "\n",
        "types construction UI",
    )

    source = replace_once(
        source,
        "    spawn_resource_dashboard(&mut commands);\n",
        "    spawn_resource_dashboard(&mut commands);\n"
        "    spawn_construction_panel(&mut commands);\n",
        "spawn panneau construction",
    )
    source = replace_once(
        source,
        "\nfn spawn_panel_heading",
        "\n" + CLIENT_SPAWN.rstrip()
        + "\n\nfn spawn_panel_heading",
        "fonctions spawn construction",
    )

    # Simplify resource dashboard.
    capture_pattern = re.compile(
        r"fn capture_resource_deltas\(.*?\n\}\n\n",
        flags=re.DOTALL,
    )
    source, count = capture_pattern.subn(
        "",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "Fonction capture_resource_deltas introuvable."
        )

    update_pattern = re.compile(
        r"fn update_resource_dashboard\(.*?\n\}\n\n"
        r"(?=fn update_resource_card_help)",
        flags=re.DOTALL,
    )
    update_match = update_pattern.search(source)
    if update_match is None:
        raise SystemExit(
            "update_resource_dashboard introuvable."
        )
    new_update = r"""fn update_resource_dashboard(
    simulation: Res<SimulationResource>,
    mut roots: Query<
        &mut Visibility,
        With<ResourceDashboardRoot>,
    >,
    mut headers: Query<
        &mut Text,
        (
            With<ResourceDashboardHeaderText>,
            Without<ResourceHudCardText>,
        ),
    >,
    mut card_texts: Query<
        (
            &ResourceHudCardText,
            &mut Text,
            &mut TextColor,
        ),
        Without<ResourceDashboardHeaderText>,
    >,
    mut gauges: Query<(
        &ResourceHudGaugeFill,
        &mut Node,
        &mut BackgroundColor,
    )>,
) {
    let Some(colony) =
        selected_colony_for_resource_dashboard(
            simulation.simulation(),
        )
    else {
        for mut visibility in &mut roots {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    for mut visibility in &mut roots {
        *visibility = Visibility::Visible;
    }
    for mut header in &mut headers {
        header.0 = format!("ÉCONOMIE — {}", colony.name);
    }

    let production =
        galactic_sim::colony_production_snapshot(colony);
    for (card, mut text, mut text_color) in
        &mut card_texts
    {
        let view =
            resource_hud_view(card.kind, colony, production);
        text.0 = view.text;
        text_color.0 =
            status_text_color(card.kind, view.status);
    }

    for (gauge, mut node, mut background) in
        &mut gauges
    {
        let view =
            resource_hud_view(gauge.kind, colony, production);
        node.width = Val::Percent(
            (view.fill_ratio * 100.0)
                .clamp(0.0, 100.0),
        );
        background.0 =
            status_gauge_color(gauge.kind, view.status);
    }
}

"""
    source = (
        source[: update_match.start()]
        + new_update
        + source[update_match.end() :]
    )
    source = source.replace(
        '"Total / capacité • disponible = total - réservé • \\\n'
        'les stocks sont crédités toutes les 5 secondes stratégiques.",',
        '"Total / capacité • disponible = total - réservé.",',
        1,
    )

    hud_pattern = re.compile(
        r"fn resource_hud_view\(.*?\n\}\n\n"
        r"(?=fn energy_hud_view)",
        flags=re.DOTALL,
    )
    hud_match = hud_pattern.search(source)
    if hud_match is None:
        raise SystemExit(
            "resource_hud_view introuvable."
        )
    new_hud = r"""fn resource_hud_view(
    kind: ResourceHudKind,
    colony: &galactic_sim::ColonyState,
    production: galactic_sim::ColonyProductionSnapshot,
) -> ResourceHudView {
    if kind == ResourceHudKind::Energy {
        return energy_hud_view(production);
    }

    let stock =
        resource_value(kind, colony.resources.stock());
    let available = resource_value(
        kind,
        colony.resources.available(),
    );
    let reserved = resource_value(
        kind,
        colony.resources.reserved_total(),
    );
    let capacity =
        resource_value(kind, production.capacity);
    let rate =
        resource_rate_per_second(kind, production);
    let saturation =
        resource_saturation(kind, production);
    let fill_ratio =
        resource_fill_ratio(stock, capacity);
    let status = resource_hud_status(
        stock,
        available,
        reserved,
        capacity,
    );
    let warning = resource_status_label(status);
    let warning_text = if warning.is_empty() {
        String::new()
    } else {
        format!("\n{warning}")
    };

    ResourceHudView {
        text: format!(
            "{}  {} / {}\nDisponible {}  •  Réservé {}\n+{:.2}/s  •  saturation {}{}",
            kind.title(),
            stock,
            capacity,
            available,
            reserved,
            rate,
            format_saturation_time(saturation),
            warning_text,
        ),
        fill_ratio,
        status,
    }
}

"""
    source = (
        source[: hud_match.start()]
        + new_hud
        + source[hud_match.end() :]
    )
    source = source.replace(
        '        ResourceHudStatus::Normal => "STABLE",\n'
        '        ResourceHudStatus::Reserved => '
        '"INDISPONIBLE — RÉSERVÉ",\n'
        '        ResourceHudStatus::NearlyFull => '
        '"PRESQUE PLEIN",\n'
        '        ResourceHudStatus::Full => '
        '"PLEIN — PRODUCTION PERDUE",',
        '        ResourceHudStatus::Normal\n'
        '        | ResourceHudStatus::Reserved\n'
        '        | ResourceHudStatus::NearlyFull => "",\n'
        '        ResourceHudStatus::Full => '
        '"PLEIN — PRODUCTION BLOQUÉE",',
        1,
    )
    energy_old = (
        '            "ÉNERGIE  {} / {}\\nDisponible {}  •  '
        'Bilan {:+}\\nRendement extracteurs {}%\\n{}",'
    )
    energy_new = (
        '            "ÉNERGIE  {} / {}\\nDisponible {}  •  '
        'Bilan {:+}\\nRendement extracteurs {}%{}",'
    )
    source = replace_once(
        source,
        energy_old,
        energy_new,
        "format énergie",
    )
    source = replace_once(
        source,
        "            u32::from("
        "production.energy_efficiency_per_mille,) / 10,\n"
        "            resource_status_label(status),\n",
        "            u32::from("
        "production.energy_efficiency_per_mille,) / 10,\n"
        "            if status == ResourceHudStatus::Deficit {\n"
        "                \"\\nDÉFICIT ÉNERGÉTIQUE\"\n"
        "            } else {\n"
        "                \"\"\n"
        "            },\n",
        "alerte énergie",
    )

    inspector_pattern = re.compile(
        r"fn colony_economy_text\(.*?\n\}\n\n"
        r"(?=fn format_saturation_time)",
        flags=re.DOTALL,
    )
    inspector_match = inspector_pattern.search(source)
    if inspector_match is None:
        raise SystemExit(
            "colony_economy_text introuvable."
        )
    inspector_replacement = r"""fn colony_economy_text(
    colony: &galactic_sim::ColonyState,
) -> String {
    let stock = colony.resources.stock();
    let available = colony.resources.available();
    let reserved = colony.resources.reserved_total();
    let production =
        galactic_sim::colony_production_snapshot(colony);

    format!(
        "STOCKS EXACTS\nTotal — Métal {}  Cristal {}  Carburant {}\nDisponible — Métal {}  Cristal {}  Carburant {}\nRéservé — Métal {}  Cristal {}  Carburant {}\nCapacité — Métal {}  Cristal {}  Carburant {}\n\nPRODUCTION ACTUELLE\nMétal +{:.2}/s  Cristal +{:.2}/s  Carburant +{:.2}/s\nSaturation — Métal {}  Cristal {}  Carburant {}\n\nÉNERGIE — CAPACITÉ\nNominale : {}\nEffective planète : {}\nConsommation catalogue : {}\nEfficacité extracteurs : {}%\nBilan effectif : {:+}",
        stock.metal,
        stock.crystal,
        stock.fuel,
        available.metal,
        available.crystal,
        available.fuel,
        reserved.metal,
        reserved.crystal,
        reserved.fuel,
        production.capacity.metal,
        production.capacity.crystal,
        production.capacity.fuel,
        production.effective_rate.metal_per_second(),
        production.effective_rate.crystal_per_second(),
        production.effective_rate.fuel_per_second(),
        format_saturation_time(production.saturation.metal),
        format_saturation_time(production.saturation.crystal),
        format_saturation_time(production.saturation.fuel),
        production.nominal_energy_production,
        production.effective_energy_production,
        production.energy_consumption,
        u32::from(
            production.energy_efficiency_per_mille,
        ) / 10,
        i128::from(production.effective_energy_production)
            - i128::from(production.energy_consumption),
    )
}

"""
    source = (
        source[: inspector_match.start()]
        + inspector_replacement
        + source[inspector_match.end() :]
    )

    source = replace_once(
        source,
        "\nfn update_action_buttons(",
        "\n" + CLIENT_SYSTEMS.rstrip()
        + "\n\nfn update_action_buttons(",
        "systèmes construction",
    )

    source = replace_once(
        source,
        "    for event in simulation.pending_events.drain(..) {\n"
        "        if matches!(event, GameEvent::KnowledgeChanged(_)) {\n",
        "    for event in simulation.pending_events.drain(..) {\n"
        "        if matches!(event, GameEvent::ProductionRefreshed(_)) {\n"
        "            continue;\n"
        "        }\n"
        "        if matches!(event, GameEvent::KnowledgeChanged(_)) {\n",
        "filtrage du rafraîchissement de production",
    )

    # Extend event labels.
    event_arm = (
        "        GameEvent::ProductionRefreshed(report) => "
        "format!(\n"
        "            \"ressources +{}/{}/{} sur {} ticks\",\n"
        "            report.produced.metal,\n"
        "            report.produced.crystal,\n"
        "            report.produced.fuel,\n"
        "            report.ticks.ticks(),\n"
        "        ),\n"
    )
    neutral_production_arm = (
        "        GameEvent::ProductionRefreshed(_) => "
        "\"production actualisée\".to_string(),\n"
    )
    event_extension = neutral_production_arm + (
        "        GameEvent::ConstructionQueued(queued) => "
        "format!(\n"
        "            \"construction {:?} niveau {} ajoutée "
        "({})\",\n"
        "            queued.order.kind,\n"
        "            queued.order.target_level,\n"
        "            queued.queue_length,\n"
        "        ),\n"
        "        GameEvent::ConstructionCompleted(done) => "
        "format!(\n"
        "            \"construction {:?} niveau {} terminée\",\n"
        "            done.kind,\n"
        "            done.new_level,\n"
        "        ),\n"
        "        GameEvent::ConstructionRejected(rejected) => "
        "format!(\n"
        "            \"construction {:?} refusée : {:?}\",\n"
        "            rejected.kind,\n"
        "            rejected.error,\n"
        "        ),\n"
    )
    source = replace_once(
        source,
        event_arm,
        event_extension,
        "labels événements construction",
    )

    # Remove obsolete delta test.
    delta_test = re.compile(
        r"    #\[test\]\n"
        r"    fn resource_delta_expires_without_mutating_simulation"
        r"\(\) \{.*?\n    \}\n\n",
        flags=re.DOTALL,
    )
    source, _ = delta_test.subn(
        "",
        source,
        count=1,
    )
    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-014 — File de construction et améliorations" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_sim/src/construction.rs":
            CONSTRUCTION_RS,
        root / "crates/galactic_sim/src/command.rs":
            COMMAND_RS,
        root / "crates/galactic_sim/src/event.rs":
            EVENT_RS,
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

    docs = root / "docs/mvp_architecture.md"
    before = docs.read_text(encoding="utf-8")
    after = patch_docs(before)
    if before != after:
        updates.append(Update(docs, before, after))

    validate_prospective(root, updates)
    return updates


def validate_prospective(
    root: Path,
    updates: list[Update],
) -> None:
    mapped = {
        update.path: update.after for update in updates
    }
    required = {
        "crates/galactic_sim/src/construction.rs": (
            "pub struct ConstructionQueue",
            "enqueue_building_upgrade",
            "advance_colony_construction",
        ),
        "crates/galactic_sim/src/state.rs": (
            "GAME_STATE_VERSION: u32 = 8",
            "pub construction_queue: ConstructionQueue",
        ),
        "crates/galactic_persistence/src/lib.rs": (
            "SAVE_VERSION: u32 = 9",
            "construction_queue",
        ),
        "crates/galactic_client/src/lib.rs": (
            "ConstructionPanelRoot",
            "PLEIN — PRODUCTION BLOQUÉE",
            "handle_construction_buttons",
        ),
    }

    failures = []
    for relative, markers in required.items():
        path = root / relative
        content = mapped.get(
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

    client = mapped.get(
        root / "crates/galactic_client/src/lib.rs",
        "",
    )
    for obsolete in (
        "Dernier crédit",
        "prochain crédit",
        "Crédit des stocks",
        "Prochaine actualisation",
        "RESOURCE_DELTA_DISPLAY_SECONDS",
        '"STABLE"',
        '"PRESQUE PLEIN"',
        "ressources +",
    ):
        if obsolete in client:
            failures.append(
                f"UI: information obsolète encore présente "
                f"{obsolete}"
            )

    if failures:
        raise SystemExit(
            "Migration MVP-014 incomplète :\n- "
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
        print("MVP-014 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp014-backup"
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
            end="" if status.endswith("\n")
            else "\n",
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
        "\nMVP-014 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

// MVP-014: deterministic construction queues and building upgrades.
use std::collections::{BTreeSet, VecDeque};

use galactic_domain::{
    ColonyId, EnergyGrid, FactionId, ReservationId, ResourceCost, ResourceLedgerError,
    ResourceStock,
};

use crate::{
    AuthorizationError, BuildingCatalogError, BuildingEffect, BuildingKind, BuildingLevels,
    ColonyState, GameState, PlanetResourceProfile, StrategicDuration, default_building_catalog,
    default_ruleset,
};

pub fn max_construction_queue() -> usize {
    default_ruleset().economy().construction_queue_limit
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConstructionOrder {
    pub kind: BuildingKind,
    pub target_level: u8,
    pub cost: ResourceCost,
    pub reservation_id: ReservationId,
    pub total_ticks: u64,
    pub remaining_ticks: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConstructionQueue {
    orders: VecDeque<ConstructionOrder>,
}

impl ConstructionQueue {
    pub fn orders(&self) -> impl Iterator<Item = &ConstructionOrder> {
        self.orders.iter()
    }

    pub fn active(&self) -> Option<&ConstructionOrder> {
        self.orders.front()
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    fn push(&mut self, order: ConstructionOrder) {
        self.orders.push_back(order);
    }

    fn pop_front(&mut self) -> ConstructionOrder {
        self.orders
            .pop_front()
            .expect("non-empty queue has an active order")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingUpgradeQuote {
    pub colony_id: ColonyId,
    pub kind: BuildingKind,
    pub current_level: u8,
    pub target_level: u8,
    pub cost: ResourceCost,
    pub duration_ticks: u64,
    pub projected_energy_production: u64,
    pub projected_energy_consumption: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionQueued {
    pub colony_id: ColonyId,
    pub order: ConstructionOrder,
    pub queue_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionCompleted {
    pub colony_id: ColonyId,
    pub kind: BuildingKind,
    pub new_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionRejected {
    pub colony_id: ColonyId,
    pub kind: BuildingKind,
    pub error: ConstructionError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionCancelled {
    pub colony_id: ColonyId,
    pub kind: BuildingKind,
    pub target_level: u8,
    pub refunded: ResourceCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionCancellationRejected {
    pub colony_id: ColonyId,
    pub error: ConstructionError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionError {
    UnknownColony(ColonyId),
    Access(AuthorizationError),
    QueueFull {
        maximum: usize,
    },
    MaximumLevel {
        kind: BuildingKind,
        level: u8,
    },
    Catalog(BuildingCatalogError),
    InsufficientResources {
        available: ResourceStock,
        cost: ResourceCost,
    },
    EnergyDeficit {
        production: u64,
        consumption: u64,
    },
    Reservation(ResourceLedgerError),
    NoActiveOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionQueueError {
    TooManyOrders {
        found: usize,
        maximum: usize,
    },
    InvalidTargetLevel {
        kind: BuildingKind,
        expected: u8,
        found: u8,
    },
    InvalidDuration {
        kind: BuildingKind,
        total_ticks: u64,
        remaining_ticks: u64,
    },
    UnexpectedTotalDuration {
        kind: BuildingKind,
        expected: u64,
        found: u64,
    },
    InvalidCost {
        kind: BuildingKind,
        expected: ResourceCost,
        found: ResourceCost,
    },
    MissingReservation {
        reservation_id: ReservationId,
    },
    ReservationCostMismatch {
        reservation_id: ReservationId,
        expected: ResourceCost,
        found: ResourceCost,
    },
    DuplicateReservation {
        reservation_id: ReservationId,
    },
    Catalog(BuildingCatalogError),
    EnergyDeficit {
        production: u64,
        consumption: u64,
    },
}

pub fn building_upgrade_quote(
    state: &GameState,
    actor: FactionId,
    colony_id: ColonyId,
    kind: BuildingKind,
) -> Result<BuildingUpgradeQuote, ConstructionError> {
    let colony = state
        .colony(colony_id)
        .ok_or(ConstructionError::UnknownColony(colony_id))?;
    state
        .authorize_management(actor, colony.owner)
        .map_err(ConstructionError::Access)?;
    let maximum = max_construction_queue();
    if colony.construction_queue.len() >= maximum {
        return Err(ConstructionError::QueueFull { maximum });
    }

    let catalog = default_building_catalog();
    let definition = catalog.definition(kind);
    let projected = projected_building_levels(colony);
    let current_level = projected.level(kind);
    if current_level >= definition.max_level {
        return Err(ConstructionError::MaximumLevel {
            kind,
            level: current_level,
        });
    }

    let target_level = current_level + 1;
    let mut after = projected;
    after.set_level(kind, target_level);
    catalog
        .validate_levels(after)
        .map_err(ConstructionError::Catalog)?;

    let cost = definition
        .cost_for_level(target_level)
        .map_err(ConstructionError::Catalog)?;
    let available = colony.resources.available();
    if !available.can_cover(cost) {
        return Err(ConstructionError::InsufficientResources { available, cost });
    }

    let grid = catalog.energy_grid_for_levels(after);
    let effective_energy = effective_energy_production(grid, colony.resource_profile);
    if effective_energy < grid.consumption() {
        return Err(ConstructionError::EnergyDeficit {
            production: effective_energy,
            consumption: grid.consumption(),
        });
    }

    let base_duration = definition
        .duration_for_level(target_level)
        .map_err(ConstructionError::Catalog)?;
    let duration_ticks = adjusted_construction_duration(base_duration, projected);

    Ok(BuildingUpgradeQuote {
        colony_id,
        kind,
        current_level,
        target_level,
        cost,
        duration_ticks,
        projected_energy_production: effective_energy,
        projected_energy_consumption: grid.consumption(),
    })
}

pub fn enqueue_building_upgrade(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
    kind: BuildingKind,
) -> Result<ConstructionQueued, ConstructionError> {
    let quote = building_upgrade_quote(state, actor, colony_id, kind)?;
    let colony = state
        .colony_mut(colony_id)
        .ok_or(ConstructionError::UnknownColony(colony_id))?;
    let reservation_id = colony
        .resources
        .reserve(quote.cost)
        .map_err(ConstructionError::Reservation)?;

    let order = ConstructionOrder {
        kind,
        target_level: quote.target_level,
        cost: quote.cost,
        reservation_id,
        total_ticks: quote.duration_ticks,
        remaining_ticks: quote.duration_ticks,
    };
    colony.construction_queue.push(order);

    Ok(ConstructionQueued {
        colony_id,
        order,
        queue_length: colony.construction_queue.len(),
    })
}

pub fn cancel_construction(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
) -> Result<ConstructionCancelled, ConstructionError> {
    let colony = state
        .colony(colony_id)
        .ok_or(ConstructionError::UnknownColony(colony_id))?;
    state
        .authorize_management(actor, colony.owner)
        .map_err(ConstructionError::Access)?;
    if colony.construction_queue.active().is_none() {
        return Err(ConstructionError::NoActiveOrder);
    }

    let colony = state
        .colony_mut(colony_id)
        .ok_or(ConstructionError::UnknownColony(colony_id))?;
    let order = colony.construction_queue.pop_front();
    let refunded = colony
        .resources
        .release(order.reservation_id)
        .map_err(ConstructionError::Reservation)?;

    Ok(ConstructionCancelled {
        colony_id,
        kind: order.kind,
        target_level: order.target_level,
        refunded,
    })
}

pub fn advance_colony_construction(
    colony: &mut ColonyState,
    ticks: StrategicDuration,
) -> Result<Vec<ConstructionCompleted>, ResourceLedgerError> {
    let mut remaining = ticks.ticks();
    let mut completed = Vec::new();

    while remaining > 0 && !colony.construction_queue.is_empty() {
        let active = colony
            .construction_queue
            .orders
            .front_mut()
            .expect("non-empty queue has an active order");
        let step = remaining.min(active.remaining_ticks);
        active.remaining_ticks -= step;
        remaining -= step;

        if active.remaining_ticks > 0 {
            continue;
        }

        let order = colony.construction_queue.pop_front();
        colony.resources.commit(order.reservation_id)?;
        colony.buildings.set_level(order.kind, order.target_level);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        completed.push(ConstructionCompleted {
            colony_id: colony.id,
            kind: order.kind,
            new_level: order.target_level,
        });
    }

    Ok(completed)
}

pub fn validate_construction_queue(colony: &ColonyState) -> Result<(), ConstructionQueueError> {
    let maximum = max_construction_queue();
    if colony.construction_queue.len() > maximum {
        return Err(ConstructionQueueError::TooManyOrders {
            found: colony.construction_queue.len(),
            maximum,
        });
    }

    let catalog = default_building_catalog();
    let mut projected = colony.buildings;
    let mut reservations = BTreeSet::new();

    for order in colony.construction_queue.orders() {
        let expected_level = projected.level(order.kind).saturating_add(1);
        if order.target_level != expected_level {
            return Err(ConstructionQueueError::InvalidTargetLevel {
                kind: order.kind,
                expected: expected_level,
                found: order.target_level,
            });
        }
        if order.total_ticks == 0
            || order.remaining_ticks == 0
            || order.remaining_ticks > order.total_ticks
        {
            return Err(ConstructionQueueError::InvalidDuration {
                kind: order.kind,
                total_ticks: order.total_ticks,
                remaining_ticks: order.remaining_ticks,
            });
        }

        let definition = catalog.definition(order.kind);
        let expected_duration = adjusted_construction_duration(
            definition
                .duration_for_level(order.target_level)
                .map_err(ConstructionQueueError::Catalog)?,
            projected,
        );
        if order.total_ticks != expected_duration {
            return Err(ConstructionQueueError::UnexpectedTotalDuration {
                kind: order.kind,
                expected: expected_duration,
                found: order.total_ticks,
            });
        }

        let expected_cost = definition
            .cost_for_level(order.target_level)
            .map_err(ConstructionQueueError::Catalog)?;
        if order.cost != expected_cost {
            return Err(ConstructionQueueError::InvalidCost {
                kind: order.kind,
                expected: expected_cost,
                found: order.cost,
            });
        }
        if !reservations.insert(order.reservation_id) {
            return Err(ConstructionQueueError::DuplicateReservation {
                reservation_id: order.reservation_id,
            });
        }
        let reservation = colony
            .resources
            .reservations()
            .iter()
            .find(|reservation| reservation.id == order.reservation_id)
            .ok_or(ConstructionQueueError::MissingReservation {
                reservation_id: order.reservation_id,
            })?;
        if reservation.cost != order.cost {
            return Err(ConstructionQueueError::ReservationCostMismatch {
                reservation_id: order.reservation_id,
                expected: order.cost,
                found: reservation.cost,
            });
        }

        projected.set_level(order.kind, order.target_level);
        catalog
            .validate_levels(projected)
            .map_err(ConstructionQueueError::Catalog)?;
        let grid = catalog.energy_grid_for_levels(projected);
        let effective = effective_energy_production(grid, colony.resource_profile);
        if effective < grid.consumption() {
            return Err(ConstructionQueueError::EnergyDeficit {
                production: effective,
                consumption: grid.consumption(),
            });
        }
    }

    Ok(())
}

pub fn projected_building_levels(colony: &ColonyState) -> BuildingLevels {
    let mut projected = colony.buildings;
    for order in colony.construction_queue.orders() {
        projected.set_level(order.kind, order.target_level);
    }
    projected
}

fn adjusted_construction_duration(base_ticks: u64, levels: BuildingLevels) -> u64 {
    let catalog = default_building_catalog();
    let bonus = catalog
        .definitions()
        .filter_map(|definition| match definition.effect {
            BuildingEffect::ConstructionSpeed { permille_per_level } => {
                Some(permille_per_level.saturating_mul(u64::from(levels.level(definition.kind))))
            }
            _ => None,
        })
        .fold(0_u64, u64::saturating_add);
    let speed_per_mille = 1_000_u64.saturating_add(bonus);
    base_ticks
        .saturating_mul(1_000)
        .div_ceil(speed_per_mille)
        .max(1)
}

fn effective_energy_production(grid: EnergyGrid, profile: PlanetResourceProfile) -> u64 {
    let value = u128::from(grid.production()).saturating_mul(u128::from(profile.energy)) / 100;
    value.min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use galactic_domain::{ColonyId, FactionId, Owner, UniverseConfig};

    use crate::{STRATEGIC_TICK_NANOS, Simulation};

    use super::*;

    #[test]
    fn queueing_reserves_resources_atomically() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;
        let before = simulation
            .state()
            .colony(colony_id)
            .expect("colony exists")
            .resources
            .available();

        let queued = enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::METAL_MINE,
        )
        .expect("upgrade is affordable");

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert_eq!(colony.construction_queue.len(), 1);
        assert_eq!(
            colony.resources.available(),
            before
                .checked_sub(queued.order.cost)
                .expect("the quote was affordable")
        );
        assert_eq!(colony.resources.stock(), ResourceStock::new(600, 300, 220));
    }

    #[test]
    fn construction_completion_commits_and_levels_up() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;
        let queued = enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::METAL_MINE,
        )
        .expect("upgrade is affordable");

        let duration = Duration::from_nanos(
            queued
                .order
                .total_ticks
                .saturating_mul(STRATEGIC_TICK_NANOS),
        );
        simulation.advance(duration);

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert_eq!(colony.buildings.level(BuildingKind::METAL_MINE), 2);
        assert!(colony.construction_queue.is_empty());
        assert!(colony.resources.reservations().is_empty());
    }

    #[test]
    fn construction_progress_is_chunk_independent() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;
        let queued = enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::METAL_MINE,
        )
        .expect("upgrade is affordable");

        let mut batched = simulation
            .state()
            .colony(colony_id)
            .expect("colony exists")
            .clone();
        let mut incremental = batched.clone();

        advance_colony_construction(
            &mut batched,
            StrategicDuration::from_ticks(queued.order.total_ticks),
        )
        .expect("reservation commits");

        let mut remaining = queued.order.total_ticks;
        while remaining > 0 {
            let step = remaining.min(7);
            advance_colony_construction(&mut incremental, StrategicDuration::from_ticks(step))
                .expect("reservation commits");
            remaining -= step;
        }

        assert_eq!(batched, incremental);
    }

    #[test]
    fn prerequisites_explain_shipyard_rejection() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists");

        assert!(matches!(
            building_upgrade_quote(
                simulation.state(),
                simulation.state().player_faction,
                colony.id,
                BuildingKind::SHIPYARD,
            ),
            Err(ConstructionError::Catalog(
                BuildingCatalogError::UnsatisfiedPrerequisite { .. }
            ))
        ));
    }

    #[test]
    fn queued_levels_allow_sequential_prerequisites() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;

        simulation
            .state_mut()
            .colony_mut(colony_id)
            .expect("home colony exists")
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 500))
            .expect("test funding fits u64");

        enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::CONSTRUCTION_CENTER,
        )
        .expect("center level 2 can be queued");
        enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::METAL_MINE,
        )
        .expect("metal level 2 can be queued");
        enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::CRYSTAL_EXTRACTOR,
        )
        .expect("crystal level 2 can be queued");

        assert!(
            building_upgrade_quote(simulation.state(), actor, colony_id, BuildingKind::SHIPYARD,)
                .is_ok()
        );
    }

    #[test]
    fn foreign_colony_rejects_player_management() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let player = simulation.state().player_faction;
        let mut foreign_colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .clone();
        foreign_colony.id = ColonyId::new(99);
        foreign_colony.owner = Owner::Faction(FactionId::new(2));
        simulation.state_mut().colonies.push(foreign_colony);

        assert!(matches!(
            building_upgrade_quote(
                simulation.state(),
                player,
                ColonyId::new(99),
                BuildingKind::METAL_MINE,
            ),
            Err(ConstructionError::Access(
                AuthorizationError::NotOwner { actor, owner },
            )) if actor == player && owner == FactionId::new(2)
        ));
    }

    #[test]
    fn cancelling_an_active_order_refunds_the_reservation_and_loses_progress() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;
        let before = simulation
            .state()
            .colony(colony_id)
            .expect("colony exists")
            .resources
            .available();
        let queued = enqueue_building_upgrade(
            simulation.state_mut(),
            actor,
            colony_id,
            BuildingKind::METAL_MINE,
        )
        .expect("upgrade is affordable");

        // Make some partial progress before cancelling, to confirm it is discarded rather than
        // partially refunded.
        simulation.advance(Duration::from_secs(1));

        let cancelled = cancel_construction(simulation.state_mut(), actor, colony_id)
            .expect("an active order can be cancelled");
        assert_eq!(cancelled.kind, BuildingKind::METAL_MINE);
        assert_eq!(cancelled.target_level, queued.order.target_level);
        assert_eq!(cancelled.refunded, queued.order.cost);

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert!(colony.construction_queue.is_empty());
        assert!(colony.resources.reservations().is_empty());
        assert_eq!(colony.resources.available(), before);
        assert_eq!(colony.buildings.level(BuildingKind::METAL_MINE), 1);
    }

    #[test]
    fn cancelling_without_an_active_order_is_an_explicit_error() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        let actor = simulation.state().player_faction;

        assert_eq!(
            cancel_construction(simulation.state_mut(), actor, colony_id),
            Err(ConstructionError::NoActiveOrder),
        );
    }
}

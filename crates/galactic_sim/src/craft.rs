// MVP-017: ruleset-driven, deterministic shipyard craft queues.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use galactic_domain::{
    ColonyId, FactionId, ReservationId, ResourceCost, ResourceLedgerError, ResourceStock,
};
use serde::Deserialize;

use crate::{
    AuthorizationError, BuildingCatalog, BuildingEffect, BuildingKind, ColonyState, GameState,
    STRATEGIC_TICKS_PER_SECOND, StrategicDuration, TechnologyCatalog, TechnologyId,
    default_ruleset,
};

pub const MAX_RULESET_CRAFTABLES: usize = 128;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CraftableId(&'static str);

impl CraftableId {
    pub const LIGHT_PROBE: Self = Self("light_probe");
    pub const LIGHT_CARGO: Self = Self("light_cargo");
    pub const FRIGATE_BULWARK: Self = Self("frigate_bulwark");
    pub const COLONY_SHIP: Self = Self("colony_ship");

    pub const fn from_static(key: &'static str) -> Self {
        Self(key)
    }

    pub const fn key(self) -> &'static str {
        self.0
    }

    fn from_config(key: String) -> Result<Self, CraftCatalogError> {
        validate_identifier(&key).map_err(|()| CraftCatalogError::InvalidIdentifier)?;
        Ok(Self(Box::leak(key.into_boxed_str())))
    }
}

impl fmt::Debug for CraftableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("CraftableId").field(&self.0).finish()
    }
}

impl fmt::Display for CraftableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum CraftableCategory {
    Probe,
    Transport,
    Colony,
    Defense,
    Military,
    Support,
}

impl CraftableCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Probe => "Sonde",
            Self::Transport => "Transport",
            Self::Colony => "Colonisation",
            Self::Defense => "Défense",
            Self::Military => "Militaire",
            Self::Support => "Soutien",
        }
    }

    const fn structural_key(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Transport => "transport",
            Self::Colony => "colony",
            Self::Defense => "defense",
            Self::Military => "military",
            Self::Support => "support",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CraftCapabilityId(&'static str);

impl CraftCapabilityId {
    pub const fn key(self) -> &'static str {
        self.0
    }

    fn from_config(key: String) -> Result<Self, CraftCatalogError> {
        validate_identifier(&key).map_err(|()| CraftCatalogError::InvalidIdentifier)?;
        Ok(Self(Box::leak(key.into_boxed_str())))
    }
}

impl fmt::Debug for CraftCapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CraftCapabilityId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for CraftCapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCapability {
    pub id: CraftCapabilityId,
    pub value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ShipClass {
    Probe,
    Cargo,
    Colony,
    Military,
    Support,
}

impl ShipClass {
    const fn structural_key(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Cargo => "cargo",
            Self::Colony => "colony",
            Self::Military => "military",
            Self::Support => "support",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipDefinition {
    pub class: ShipClass,
    pub cruise_speed: u64,
    pub range_hops: u16,
    pub cargo_capacity: u64,
    pub fuel_per_hop: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftBuildingPrerequisite {
    pub kind: BuildingKind,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftableDefinition {
    pub id: CraftableId,
    pub name: &'static str,
    pub description: &'static str,
    pub category: CraftableCategory,
    pub cost: ResourceCost,
    pub base_duration_ticks: u64,
    pub required_work_milli: u64,
    pub result_quantity: u64,
    pub building_prerequisites: Vec<CraftBuildingPrerequisite>,
    pub technology_prerequisites: Vec<TechnologyId>,
    pub capabilities: Vec<CraftCapability>,
    pub ship: Option<ShipDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftableCatalog {
    version: u32,
    order: Vec<CraftableId>,
    definitions: BTreeMap<CraftableId, CraftableDefinition>,
}

impl CraftableCatalog {
    pub(crate) fn from_config(
        config: CraftableCatalogConfig,
        buildings: &BuildingCatalog,
        technologies: &TechnologyCatalog,
    ) -> Result<Self, CraftCatalogError> {
        if config.version != 2 {
            return Err(CraftCatalogError::UnsupportedVersion(config.version));
        }
        if config.craftables.is_empty() || config.craftables.len() > MAX_RULESET_CRAFTABLES {
            return Err(CraftCatalogError::InvalidCraftableCount {
                found: config.craftables.len(),
                maximum: MAX_RULESET_CRAFTABLES,
            });
        }

        let mut ids = BTreeMap::new();
        let mut order = Vec::with_capacity(config.craftables.len());
        for craftable in &config.craftables {
            let id = CraftableId::from_config(craftable.id.clone())?;
            if ids.insert(craftable.id.clone(), id).is_some() {
                return Err(CraftCatalogError::DuplicateCraftable(id));
            }
            order.push(id);
        }

        let mut definitions = BTreeMap::new();
        for craftable in config.craftables {
            let id = ids[&craftable.id];
            let building_prerequisites = craftable
                .building_prerequisites
                .into_iter()
                .map(|prerequisite| {
                    let Some(kind) = buildings.kind_by_key(&prerequisite.id) else {
                        return Err(CraftCatalogError::MissingBuildingPrerequisite {
                            craftable: id,
                            building: BuildingKind::from_static(Box::leak(
                                prerequisite.id.into_boxed_str(),
                            )),
                        });
                    };
                    Ok(CraftBuildingPrerequisite {
                        kind,
                        level: prerequisite.level,
                    })
                })
                .collect::<Result<Vec<_>, CraftCatalogError>>()?;
            let technology_prerequisites = craftable
                .technology_prerequisites
                .into_iter()
                .map(|prerequisite| {
                    let Some(technology) = technologies.id_by_key(&prerequisite) else {
                        return Err(CraftCatalogError::MissingTechnologyPrerequisite {
                            craftable: id,
                            technology: TechnologyId::from_static(Box::leak(
                                prerequisite.into_boxed_str(),
                            )),
                        });
                    };
                    Ok(technology)
                })
                .collect::<Result<Vec<_>, CraftCatalogError>>()?;
            let capabilities = craftable
                .capabilities
                .into_iter()
                .map(|capability| {
                    Ok(CraftCapability {
                        id: CraftCapabilityId::from_config(capability.id)?,
                        value: capability.value,
                    })
                })
                .collect::<Result<Vec<_>, CraftCatalogError>>()?;
            let base_duration_ticks = craftable
                .base_duration_seconds
                .checked_mul(u64::from(STRATEGIC_TICKS_PER_SECOND))
                .ok_or(CraftCatalogError::InvalidDefinition(id))?;
            let reference_output = building_prerequisites
                .iter()
                .filter_map(
                    |prerequisite| match buildings.definition(prerequisite.kind).effect {
                        BuildingEffect::ShipyardPoints {
                            milli_per_tick_per_level,
                        } => Some(
                            milli_per_tick_per_level.saturating_mul(u64::from(prerequisite.level)),
                        ),
                        _ => None,
                    },
                )
                .fold(0_u64, u64::saturating_add);
            let required_work_milli = base_duration_ticks
                .checked_mul(reference_output)
                .ok_or(CraftCatalogError::InvalidDefinition(id))?;

            definitions.insert(
                id,
                CraftableDefinition {
                    id,
                    name: leak_non_empty(craftable.name, CraftCatalogError::EmptyName(id))?,
                    description: leak_non_empty(
                        craftable.description,
                        CraftCatalogError::EmptyDescription(id),
                    )?,
                    category: craftable.category,
                    cost: craftable.cost.into_cost(),
                    base_duration_ticks,
                    required_work_milli,
                    result_quantity: craftable.result_quantity,
                    building_prerequisites,
                    technology_prerequisites,
                    capabilities,
                    ship: craftable.ship.map(ShipDefinitionConfig::compile),
                },
            );
        }

        let catalog = Self {
            version: config.version,
            order,
            definitions,
        };
        catalog.validate(buildings)?;
        Ok(catalog)
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub fn ids(&self) -> impl Iterator<Item = CraftableId> + '_ {
        self.order.iter().copied()
    }

    pub fn definitions(&self) -> impl Iterator<Item = &CraftableDefinition> {
        self.order.iter().map(|id| self.definition(*id))
    }

    pub fn definition(&self, craftable: CraftableId) -> &CraftableDefinition {
        self.get(craftable)
            .expect("validated craftable identifier must exist in the active ruleset")
    }

    pub fn get(&self, craftable: CraftableId) -> Option<&CraftableDefinition> {
        self.definitions.get(&craftable)
    }

    pub fn id_by_key(&self, key: &str) -> Option<CraftableId> {
        self.definitions
            .keys()
            .copied()
            .find(|craftable| craftable.key() == key)
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("craft-catalog:");
        output.push_str(&self.version.to_string());
        output.push(';');
        for definition in self.definitions() {
            output.push_str(definition.id.key());
            output.push(':');
            output.push_str(definition.category.structural_key());
            output.push('[');
            for prerequisite in &definition.building_prerequisites {
                output.push_str(prerequisite.kind.key());
                output.push('=');
                output.push_str(&prerequisite.level.to_string());
                output.push(',');
            }
            output.push('|');
            for prerequisite in &definition.technology_prerequisites {
                output.push_str(prerequisite.key());
                output.push(',');
            }
            output.push('|');
            for capability in &definition.capabilities {
                output.push_str(capability.id.key());
                output.push(',');
            }
            output.push('|');
            if let Some(ship) = definition.ship {
                output.push_str("ship:");
                output.push_str(ship.class.structural_key());
                output.push_str(":cruise_speed:range_hops:cargo_capacity:fuel_per_hop");
            }
            output.push_str("];");
        }
    }

    fn validate(&self, buildings: &BuildingCatalog) -> Result<(), CraftCatalogError> {
        for definition in self.definitions() {
            if definition.cost.is_zero()
                || definition.base_duration_ticks == 0
                || definition.required_work_milli == 0
                || definition.result_quantity == 0
            {
                return Err(CraftCatalogError::InvalidDefinition(definition.id));
            }

            let mut building_prerequisites = BTreeSet::new();
            let mut has_shipyard = false;
            for prerequisite in &definition.building_prerequisites {
                if prerequisite.level == 0 {
                    return Err(CraftCatalogError::InvalidBuildingLevel {
                        craftable: definition.id,
                        building: prerequisite.kind,
                    });
                }
                if !building_prerequisites.insert(prerequisite.kind) {
                    return Err(CraftCatalogError::DuplicateBuildingPrerequisite {
                        craftable: definition.id,
                        building: prerequisite.kind,
                    });
                }
                let building = buildings.definition(prerequisite.kind);
                if prerequisite.level > building.max_level {
                    return Err(CraftCatalogError::InvalidBuildingLevel {
                        craftable: definition.id,
                        building: prerequisite.kind,
                    });
                }
                has_shipyard |= matches!(building.effect, BuildingEffect::ShipyardPoints { .. });
            }
            if !has_shipyard {
                return Err(CraftCatalogError::MissingShipyardPrerequisite(
                    definition.id,
                ));
            }

            let mut technology_prerequisites = BTreeSet::new();
            for technology in &definition.technology_prerequisites {
                if !technology_prerequisites.insert(*technology) {
                    return Err(CraftCatalogError::DuplicateTechnologyPrerequisite {
                        craftable: definition.id,
                        technology: *technology,
                    });
                }
            }

            let mut capabilities = BTreeSet::new();
            for capability in &definition.capabilities {
                if capability.value == 0 {
                    return Err(CraftCatalogError::InvalidCapability {
                        craftable: definition.id,
                        capability: capability.id,
                    });
                }
                if !capabilities.insert(capability.id) {
                    return Err(CraftCatalogError::DuplicateCapability {
                        craftable: definition.id,
                        capability: capability.id,
                    });
                }
            }

            match (definition.category, definition.ship) {
                (CraftableCategory::Defense, Some(_)) => {
                    return Err(CraftCatalogError::DefenseCannotBeShip(definition.id));
                }
                (CraftableCategory::Defense, None) => {}
                (_, None) => {
                    return Err(CraftCatalogError::MissingShipDefinition(definition.id));
                }
                (category, Some(ship)) => {
                    let expected = match category {
                        CraftableCategory::Probe => ShipClass::Probe,
                        CraftableCategory::Transport => ShipClass::Cargo,
                        CraftableCategory::Colony => ShipClass::Colony,
                        CraftableCategory::Military => ShipClass::Military,
                        CraftableCategory::Support => ShipClass::Support,
                        CraftableCategory::Defense => unreachable!("handled above"),
                    };
                    if ship.class != expected {
                        return Err(CraftCatalogError::ShipClassMismatch {
                            craftable: definition.id,
                            expected,
                            found: ship.class,
                        });
                    }
                    if ship.cruise_speed == 0 || ship.range_hops == 0 || ship.fuel_per_hop == 0 {
                        return Err(CraftCatalogError::InvalidShipDefinition(definition.id));
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn craftable_catalog() -> &'static CraftableCatalog {
    default_ruleset().craftables()
}

pub fn craftable_definition(craftable: CraftableId) -> &'static CraftableDefinition {
    craftable_catalog().definition(craftable)
}

pub fn max_craft_queue() -> usize {
    default_ruleset().economy().craft_queue_limit
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftOrder {
    pub craftable: CraftableId,
    pub cost: ResourceCost,
    pub reservation_id: ReservationId,
    pub required_work_milli: u64,
    pub accumulated_work_milli: u64,
    pub result_quantity: u64,
}

impl CraftOrder {
    pub const fn remaining_work_milli(self) -> u64 {
        self.required_work_milli
            .saturating_sub(self.accumulated_work_milli)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraftQueue {
    orders: VecDeque<CraftOrder>,
}

impl CraftQueue {
    pub fn orders(&self) -> impl Iterator<Item = &CraftOrder> {
        self.orders.iter()
    }

    pub fn active(&self) -> Option<&CraftOrder> {
        self.orders.front()
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    fn push(&mut self, order: CraftOrder) {
        self.orders.push_back(order);
    }

    fn pop_front(&mut self) -> CraftOrder {
        self.orders
            .pop_front()
            .expect("non-empty craft queue has an active order")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraftInventory {
    quantities: BTreeMap<CraftableId, u64>,
}

impl CraftInventory {
    pub fn quantity(&self, craftable: CraftableId) -> u64 {
        self.quantities.get(&craftable).copied().unwrap_or(0)
    }

    pub fn entries(&self) -> impl Iterator<Item = (CraftableId, u64)> + '_ {
        self.quantities
            .iter()
            .map(|(craftable, quantity)| (*craftable, *quantity))
    }

    pub(crate) fn add(&mut self, craftable: CraftableId, quantity: u64) {
        let total = self
            .quantity(craftable)
            .checked_add(quantity)
            .expect("validated craft inventory cannot overflow");
        self.quantities.insert(craftable, total);
    }

    pub(crate) fn take(&mut self, craftable: CraftableId, quantity: u64) -> bool {
        let available = self.quantity(craftable);
        let Some(remaining) = available.checked_sub(quantity) else {
            return false;
        };
        if remaining == 0 {
            self.quantities.remove(&craftable);
        } else {
            self.quantities.insert(craftable, remaining);
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftQuote {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub cost: ResourceCost,
    pub required_work_milli: u64,
    pub output_milli_per_tick: u64,
    pub estimated_ticks: u64,
    pub result_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftQueued {
    pub colony_id: ColonyId,
    pub order: CraftOrder,
    pub queue_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCompleted {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub quantity: u64,
    pub inventory_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftRejected {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub error: CraftError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftError {
    UnknownColony(ColonyId),
    Access(AuthorizationError),
    UnknownCraftable(CraftableId),
    QueueFull {
        maximum: usize,
    },
    MissingBuilding {
        building: BuildingKind,
        required: u8,
        found: u8,
    },
    MissingTechnology(TechnologyId),
    NoShipyardCapacity,
    InsufficientResources {
        available: ResourceStock,
        cost: ResourceCost,
    },
    InventoryOverflow(CraftableId),
    Reservation(ResourceLedgerError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftStateError {
    TooManyOrders {
        found: usize,
        maximum: usize,
    },
    UnknownQueuedCraftable(CraftableId),
    UnknownInventoryCraftable(CraftableId),
    ZeroInventory(CraftableId),
    InvalidCost {
        craftable: CraftableId,
        expected: ResourceCost,
        found: ResourceCost,
    },
    InvalidRequiredWork {
        craftable: CraftableId,
        expected: u64,
        found: u64,
    },
    InvalidProgress {
        craftable: CraftableId,
        accumulated: u64,
        required: u64,
    },
    InvalidResultQuantity {
        craftable: CraftableId,
        expected: u64,
        found: u64,
    },
    MissingBuilding {
        craftable: CraftableId,
        building: BuildingKind,
        required: u8,
        found: u8,
    },
    MissingTechnology {
        craftable: CraftableId,
        technology: TechnologyId,
    },
    DuplicateReservation(ReservationId),
    MissingReservation(ReservationId),
    ReservationCostMismatch {
        reservation_id: ReservationId,
        expected: ResourceCost,
        found: ResourceCost,
    },
    InventoryOverflow(CraftableId),
}

pub fn craft_quote(
    state: &GameState,
    actor: FactionId,
    colony_id: ColonyId,
    craftable: CraftableId,
) -> Result<CraftQuote, CraftError> {
    let colony = state
        .colony(colony_id)
        .ok_or(CraftError::UnknownColony(colony_id))?;
    state
        .authorize_management(actor, colony.owner)
        .map_err(CraftError::Access)?;
    let maximum = max_craft_queue();
    if colony.craft_queue.len() >= maximum {
        return Err(CraftError::QueueFull { maximum });
    }

    let Some(definition) = craftable_catalog().get(craftable) else {
        return Err(CraftError::UnknownCraftable(craftable));
    };
    for prerequisite in &definition.building_prerequisites {
        let found = colony.buildings.level(prerequisite.kind);
        if found < prerequisite.level {
            return Err(CraftError::MissingBuilding {
                building: prerequisite.kind,
                required: prerequisite.level,
                found,
            });
        }
    }
    for technology in &definition.technology_prerequisites {
        if !state.research.has_completed(*technology) {
            return Err(CraftError::MissingTechnology(*technology));
        }
    }

    let output = shipyard_output_milli_per_tick(colony);
    if output == 0 {
        return Err(CraftError::NoShipyardCapacity);
    }
    let available = colony.resources.available();
    if !available.can_cover(definition.cost) {
        return Err(CraftError::InsufficientResources {
            available,
            cost: definition.cost,
        });
    }
    let projected_quantity = colony
        .craft_queue
        .orders()
        .filter(|order| order.craftable == craftable)
        .try_fold(colony.inventory.quantity(craftable), |quantity, order| {
            quantity.checked_add(order.result_quantity)
        })
        .and_then(|quantity| quantity.checked_add(definition.result_quantity));
    if projected_quantity.is_none() {
        return Err(CraftError::InventoryOverflow(craftable));
    }

    Ok(CraftQuote {
        colony_id,
        craftable,
        cost: definition.cost,
        required_work_milli: definition.required_work_milli,
        output_milli_per_tick: output,
        estimated_ticks: definition.required_work_milli.div_ceil(output),
        result_quantity: definition.result_quantity,
    })
}

pub fn enqueue_craft(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
    craftable: CraftableId,
) -> Result<CraftQueued, CraftError> {
    let quote = craft_quote(state, actor, colony_id, craftable)?;
    let colony = state
        .colony_mut(colony_id)
        .ok_or(CraftError::UnknownColony(colony_id))?;
    let reservation_id = colony
        .resources
        .reserve(quote.cost)
        .map_err(CraftError::Reservation)?;
    let order = CraftOrder {
        craftable,
        cost: quote.cost,
        reservation_id,
        required_work_milli: quote.required_work_milli,
        accumulated_work_milli: 0,
        result_quantity: quote.result_quantity,
    };
    colony.craft_queue.push(order);

    Ok(CraftQueued {
        colony_id,
        order,
        queue_length: colony.craft_queue.len(),
    })
}

pub fn advance_colony_craft(
    colony: &mut ColonyState,
    ticks: StrategicDuration,
) -> Result<Vec<CraftCompleted>, ResourceLedgerError> {
    let output = shipyard_output_milli_per_tick(colony);
    let mut budget = output.saturating_mul(ticks.ticks());
    if budget == 0 {
        return Ok(Vec::new());
    }

    let mut completed = Vec::new();
    while budget > 0 {
        let finished = {
            let Some(active) = colony.craft_queue.orders.front_mut() else {
                break;
            };
            let spent = active.remaining_work_milli().min(budget);
            active.accumulated_work_milli = active.accumulated_work_milli.saturating_add(spent);
            budget -= spent;
            (active.accumulated_work_milli >= active.required_work_milli).then_some(*active)
        };

        let Some(order) = finished else {
            continue;
        };
        colony.craft_queue.pop_front();
        colony.resources.commit(order.reservation_id)?;
        colony.inventory.add(order.craftable, order.result_quantity);
        completed.push(CraftCompleted {
            colony_id: colony.id,
            craftable: order.craftable,
            quantity: order.result_quantity,
            inventory_quantity: colony.inventory.quantity(order.craftable),
        });
    }

    Ok(completed)
}

pub fn shipyard_output_milli_per_tick(colony: &ColonyState) -> u64 {
    default_ruleset()
        .buildings()
        .definitions()
        .filter_map(|definition| match definition.effect {
            BuildingEffect::ShipyardPoints {
                milli_per_tick_per_level,
            } => Some(
                milli_per_tick_per_level
                    .saturating_mul(u64::from(colony.buildings.level(definition.kind))),
            ),
            _ => None,
        })
        .fold(0_u64, u64::saturating_add)
}

pub fn shipyard_output_points_per_second(colony: &ColonyState) -> f64 {
    shipyard_output_milli_per_tick(colony) as f64 * f64::from(STRATEGIC_TICKS_PER_SECOND) / 1_000.0
}

pub fn craft_progress_ratio(order: CraftOrder) -> f32 {
    if order.required_work_milli == 0 {
        return 1.0;
    }
    (order.accumulated_work_milli as f64 / order.required_work_milli as f64).clamp(0.0, 1.0) as f32
}

pub fn validate_craft_state(
    state: &GameState,
    colony: &ColonyState,
) -> Result<(), CraftStateError> {
    let catalog = craftable_catalog();
    let maximum = max_craft_queue();
    if colony.craft_queue.len() > maximum {
        return Err(CraftStateError::TooManyOrders {
            found: colony.craft_queue.len(),
            maximum,
        });
    }

    for (craftable, quantity) in colony.inventory.entries() {
        if catalog.get(craftable).is_none() {
            return Err(CraftStateError::UnknownInventoryCraftable(craftable));
        }
        if quantity == 0 {
            return Err(CraftStateError::ZeroInventory(craftable));
        }
    }

    let mut reservations = BTreeSet::new();
    let mut projected_inventory = colony.inventory.clone();
    for order in colony.craft_queue.orders() {
        let Some(definition) = catalog.get(order.craftable) else {
            return Err(CraftStateError::UnknownQueuedCraftable(order.craftable));
        };
        if order.cost != definition.cost {
            return Err(CraftStateError::InvalidCost {
                craftable: order.craftable,
                expected: definition.cost,
                found: order.cost,
            });
        }
        if order.required_work_milli != definition.required_work_milli {
            return Err(CraftStateError::InvalidRequiredWork {
                craftable: order.craftable,
                expected: definition.required_work_milli,
                found: order.required_work_milli,
            });
        }
        if order.required_work_milli == 0
            || order.accumulated_work_milli >= order.required_work_milli
        {
            return Err(CraftStateError::InvalidProgress {
                craftable: order.craftable,
                accumulated: order.accumulated_work_milli,
                required: order.required_work_milli,
            });
        }
        if order.result_quantity != definition.result_quantity {
            return Err(CraftStateError::InvalidResultQuantity {
                craftable: order.craftable,
                expected: definition.result_quantity,
                found: order.result_quantity,
            });
        }
        for prerequisite in &definition.building_prerequisites {
            let found = colony.buildings.level(prerequisite.kind);
            if found < prerequisite.level {
                return Err(CraftStateError::MissingBuilding {
                    craftable: order.craftable,
                    building: prerequisite.kind,
                    required: prerequisite.level,
                    found,
                });
            }
        }
        for technology in &definition.technology_prerequisites {
            if !state.research.has_completed(*technology) {
                return Err(CraftStateError::MissingTechnology {
                    craftable: order.craftable,
                    technology: *technology,
                });
            }
        }
        if !reservations.insert(order.reservation_id) {
            return Err(CraftStateError::DuplicateReservation(order.reservation_id));
        }
        let reservation = colony
            .resources
            .reservations()
            .iter()
            .find(|reservation| reservation.id == order.reservation_id)
            .ok_or(CraftStateError::MissingReservation(order.reservation_id))?;
        if reservation.cost != order.cost {
            return Err(CraftStateError::ReservationCostMismatch {
                reservation_id: order.reservation_id,
                expected: order.cost,
                found: reservation.cost,
            });
        }
        let Some(total) = projected_inventory
            .quantity(order.craftable)
            .checked_add(order.result_quantity)
        else {
            return Err(CraftStateError::InventoryOverflow(order.craftable));
        };
        projected_inventory
            .quantities
            .insert(order.craftable, total);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftCatalogError {
    InvalidIdentifier,
    UnsupportedVersion(u32),
    InvalidCraftableCount {
        found: usize,
        maximum: usize,
    },
    DuplicateCraftable(CraftableId),
    EmptyName(CraftableId),
    EmptyDescription(CraftableId),
    InvalidDefinition(CraftableId),
    MissingShipyardPrerequisite(CraftableId),
    MissingBuildingPrerequisite {
        craftable: CraftableId,
        building: BuildingKind,
    },
    MissingTechnologyPrerequisite {
        craftable: CraftableId,
        technology: TechnologyId,
    },
    InvalidBuildingLevel {
        craftable: CraftableId,
        building: BuildingKind,
    },
    DuplicateBuildingPrerequisite {
        craftable: CraftableId,
        building: BuildingKind,
    },
    DuplicateTechnologyPrerequisite {
        craftable: CraftableId,
        technology: TechnologyId,
    },
    InvalidCapability {
        craftable: CraftableId,
        capability: CraftCapabilityId,
    },
    DuplicateCapability {
        craftable: CraftableId,
        capability: CraftCapabilityId,
    },
    MissingShipDefinition(CraftableId),
    InvalidShipDefinition(CraftableId),
    DefenseCannotBeShip(CraftableId),
    ShipClassMismatch {
        craftable: CraftableId,
        expected: ShipClass,
        found: ShipClass,
    },
}

#[derive(Debug, Deserialize)]
pub(crate) struct CraftableCatalogConfig {
    version: u32,
    craftables: Vec<CraftableDefinitionConfig>,
}

#[derive(Debug, Deserialize)]
struct CraftableDefinitionConfig {
    id: String,
    name: String,
    description: String,
    category: CraftableCategory,
    cost: CraftResourceValuesConfig,
    base_duration_seconds: u64,
    result_quantity: u64,
    building_prerequisites: Vec<CraftBuildingPrerequisiteConfig>,
    technology_prerequisites: Vec<String>,
    capabilities: Vec<CraftCapabilityConfig>,
    #[serde(default)]
    ship: Option<ShipDefinitionConfig>,
}

#[derive(Debug, Deserialize)]
struct CraftBuildingPrerequisiteConfig {
    id: String,
    level: u8,
}

#[derive(Debug, Deserialize)]
struct CraftCapabilityConfig {
    id: String,
    value: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ShipDefinitionConfig {
    class: ShipClass,
    cruise_speed: u64,
    range_hops: u16,
    cargo_capacity: u64,
    fuel_per_hop: u64,
}

impl ShipDefinitionConfig {
    const fn compile(self) -> ShipDefinition {
        ShipDefinition {
            class: self.class,
            cruise_speed: self.cruise_speed,
            range_hops: self.range_hops,
            cargo_capacity: self.cargo_capacity,
            fuel_per_hop: self.fuel_per_hop,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct CraftResourceValuesConfig {
    metal: u64,
    crystal: u64,
    fuel: u64,
}

impl CraftResourceValuesConfig {
    const fn into_cost(self) -> ResourceCost {
        ResourceCost::new(self.metal, self.crystal, self.fuel)
    }
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(());
    };
    if !first.is_ascii_lowercase() {
        return Err(());
    }
    if chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(())
    }
}

fn leak_non_empty<T>(value: String, error: T) -> Result<&'static str, T> {
    if value.trim().is_empty() {
        Err(error)
    } else {
        Ok(Box::leak(value.into_boxed_str()))
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ResourceStock, UniverseConfig};

    use crate::{BuildingKind, Simulation, TechnologyId, default_building_catalog};

    use super::*;

    fn ready_simulation() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony
            .buildings
            .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
        colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
        colony
            .buildings
            .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
        colony.buildings.set_level(BuildingKind::SHIPYARD, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
        colony
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("resource credit fits");
        simulation.state_mut().research =
            crate::ResearchState::from_completed([TechnologyId::SPATIAL_DETECTION]);
        simulation
    }

    #[test]
    fn default_catalog_is_external_and_generic() {
        assert_eq!(craftable_catalog().version(), 2);
        assert_eq!(craftable_catalog().ids().count(), 4);
        assert_eq!(
            craftable_definition(CraftableId::LIGHT_PROBE).category,
            CraftableCategory::Probe,
        );
        assert_eq!(
            craftable_definition(CraftableId::LIGHT_CARGO)
                .ship
                .expect("cargo is a ship")
                .cargo_capacity,
            800,
        );
        assert_eq!(
            craftable_definition(CraftableId::FRIGATE_BULWARK).category,
            CraftableCategory::Military,
        );
    }

    #[test]
    fn queueing_reserves_resources_atomically() {
        let mut simulation = ready_simulation();
        let colony_id = simulation.state().colonies[0].id;
        let actor = simulation.state().player_faction;
        let before = simulation.state().colonies[0].resources.available();

        enqueue_craft(
            simulation.state_mut(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
        )
        .expect("probe prerequisites are met");

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert_eq!(colony.craft_queue.len(), 1);
        assert_eq!(
            colony.resources.available(),
            before
                .checked_sub(craftable_definition(CraftableId::LIGHT_PROBE).cost)
                .expect("the credited stock covers the probe"),
        );
    }

    #[test]
    fn craft_progress_is_chunk_independent_and_adds_inventory() {
        let mut whole = ready_simulation();
        let colony_id = whole.state().colonies[0].id;
        let actor = whole.state().player_faction;
        enqueue_craft(
            whole.state_mut(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
        )
        .expect("probe can be queued");
        let mut split = whole.clone();

        advance_colony_craft(
            whole
                .state_mut()
                .colony_mut(colony_id)
                .expect("colony exists"),
            StrategicDuration::from_ticks(450),
        )
        .expect("reservation commits");
        for _ in 0..45 {
            advance_colony_craft(
                split
                    .state_mut()
                    .colony_mut(colony_id)
                    .expect("colony exists"),
                StrategicDuration::from_ticks(10),
            )
            .expect("reservation commits");
        }

        assert_eq!(whole.state(), split.state());
        assert_eq!(
            whole
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .inventory
                .quantity(CraftableId::LIGHT_PROBE),
            1,
        );
    }

    #[test]
    fn missing_technology_is_explicit() {
        let mut simulation = ready_simulation();
        simulation.state_mut().research = crate::ResearchState::default();
        let colony_id = simulation.state().colonies[0].id;

        assert_eq!(
            craft_quote(
                simulation.state(),
                simulation.state().player_faction,
                colony_id,
                CraftableId::LIGHT_PROBE,
            ),
            Err(CraftError::MissingTechnology(
                TechnologyId::SPATIAL_DETECTION,
            )),
        );
    }
}

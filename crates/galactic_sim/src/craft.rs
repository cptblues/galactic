// MVP-017: ruleset-driven, deterministic shipyard craft queues.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use galactic_domain::{
    ColonyId, FactionId, ReservationId, ResourceCost, ResourceLedgerError, ResourceStock,
};
use serde::{Deserialize, Serialize};

use crate::{
    AuthorizationError, BuildingCatalog, BuildingEffect, BuildingKind, ColonyState, GameState,
    STRATEGIC_TICKS_PER_SECOND, StrategicDuration, TechnologyCatalog, TechnologyId,
    default_ruleset,
};

pub const MAX_RULESET_CRAFTABLES: usize = 128;
const COMBAT_STAT_LIMIT: u32 = 1_000_000;
const COMBAT_BONUS_PER_MILLE_LIMIT: u32 = 10_000;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CraftableId(&'static str);

impl CraftableId {
    pub const LIGHT_PROBE: Self = Self("light_probe");
    pub const CARTOGRAPHER_SATELLITE: Self = Self("cartographer_satellite");
    pub const LIGHT_CARGO: Self = Self("light_cargo");
    pub const MERIDIAN_CARRIER: Self = Self("meridian_carrier");
    pub const ATLAS_CARGO: Self = Self("atlas_cargo");
    pub const NEEDLE_INTERCEPTOR: Self = Self("needle_interceptor");
    pub const FRIGATE_BULWARK: Self = Self("frigate_bulwark");
    pub const BASTION_CRUISER: Self = Self("bastion_cruiser");
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

impl serde::Serialize for CraftableId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

impl<'de> serde::Deserialize<'de> for CraftableId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        default_ruleset()
            .craftables()
            .id_by_key(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown craftable id: {key}")))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CombatTargetClass {
    Light,
    Medium,
    Heavy,
}

impl CombatTargetClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Léger",
            Self::Medium => "Moyen",
            Self::Heavy => "Lourd",
        }
    }

    pub(crate) const fn structural_key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
        }
    }
}

pub const NEUTRAL_COMBAT_BONUS_PER_MILLE: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CombatTargetBonuses {
    pub light_per_mille: u32,
    pub medium_per_mille: u32,
    pub heavy_per_mille: u32,
}

impl Default for CombatTargetBonuses {
    fn default() -> Self {
        Self {
            light_per_mille: NEUTRAL_COMBAT_BONUS_PER_MILLE,
            medium_per_mille: NEUTRAL_COMBAT_BONUS_PER_MILLE,
            heavy_per_mille: NEUTRAL_COMBAT_BONUS_PER_MILLE,
        }
    }
}

impl CombatTargetBonuses {
    pub const fn multiplier_for(self, class: CombatTargetClass) -> u32 {
        match class {
            CombatTargetClass::Light => self.light_per_mille,
            CombatTargetClass::Medium => self.medium_per_mille,
            CombatTargetClass::Heavy => self.heavy_per_mille,
        }
    }

    pub const fn entries(self) -> [(CombatTargetClass, u32); 3] {
        [
            (CombatTargetClass::Light, self.light_per_mille),
            (CombatTargetClass::Medium, self.medium_per_mille),
            (CombatTargetClass::Heavy, self.heavy_per_mille),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipCombatStats {
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
    pub target_class: CombatTargetClass,
    pub bonuses: CombatTargetBonuses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipDefinition {
    pub class: ShipClass,
    pub cruise_speed: u64,
    pub range_hops: u16,
    pub cargo_capacity: u64,
    pub fuel_per_hop: u64,
    pub combat: Option<ShipCombatStats>,
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
                    ship: craftable.ship.map(|ship| ship.compile(id)).transpose()?,
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
                if let Some(combat) = ship.combat {
                    output.push_str(":combat:");
                    output.push_str(combat.target_class.structural_key());
                    output.push_str(":offense:defense:durability:bonuses[");
                    for (target_class, multiplier) in combat.bonuses.entries() {
                        if multiplier != NEUTRAL_COMBAT_BONUS_PER_MILLE {
                            output.push_str(target_class.structural_key());
                            output.push(',');
                        }
                    }
                    output.push(']');
                }
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
                    match (ship.class, ship.combat) {
                        (ShipClass::Military, None) => {
                            return Err(CraftCatalogError::MissingCombatStats(definition.id));
                        }
                        (ShipClass::Military, Some(_)) => {}
                        (_, Some(_)) => {
                            return Err(CraftCatalogError::UnexpectedCombatStats(definition.id));
                        }
                        (_, None) => {}
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

/// Upper bound on how many units a single batch can request — protects the numeric arithmetic
/// (cost/duration totals) and the UI's "MAX" button from unbounded values, per MVP-030-A3.
pub const MAX_CRAFT_BATCH_QUANTITY: u64 = 999;

/// A queued batch of one or more identical craftables (MVP-030-A3). Units are built one at a
/// time — `accumulated_work_milli` only ever tracks the *current* unit's progress — and each
/// not-yet-completed unit holds its own reservation so completed units can be credited (and
/// their reservation committed) independently of the rest of the batch, and a cancellation can
/// refund exactly the untouched units without disturbing already-produced ones.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CraftOrder {
    pub craftable: CraftableId,
    pub unit_cost: ResourceCost,
    pub result_quantity_per_unit: u64,
    pub required_work_milli_per_unit: u64,
    pub accumulated_work_milli: u64,
    pub quantity_requested: u64,
    pub quantity_completed: u64,
    /// One reservation per not-yet-completed unit; the front is the unit currently in progress.
    pub reservations: VecDeque<ReservationId>,
}

impl CraftOrder {
    pub fn remaining_work_milli(&self) -> u64 {
        self.required_work_milli_per_unit
            .saturating_sub(self.accumulated_work_milli)
    }

    pub fn quantity_remaining(&self) -> u64 {
        self.quantity_requested
            .saturating_sub(self.quantity_completed)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub quantity: u64,
    pub unit_cost: ResourceCost,
    pub total_cost: ResourceCost,
    pub unit_required_work_milli: u64,
    pub total_required_work_milli: u64,
    pub output_milli_per_tick: u64,
    pub estimated_ticks: u64,
    pub result_quantity_per_unit: u64,
    /// How many units the colony could afford right now (used by the UI's "MAX" control and by
    /// the displayed "quantité finançable" — independent of the `quantity` actually requested).
    pub max_affordable_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftQueued {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub quantity_requested: u64,
    pub queue_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCompleted {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub quantity_completed: u64,
    pub quantity_remaining: u64,
    pub inventory_quantity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCancelled {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub quantity_completed: u64,
    pub quantity_refunded: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftRejected {
    pub colony_id: ColonyId,
    pub craftable: CraftableId,
    pub error: CraftError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCancellationRejected {
    pub colony_id: ColonyId,
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
    InvalidQuantity {
        requested: u64,
        maximum: u64,
    },
    NoActiveOrder,
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
    InvalidQuantity {
        craftable: CraftableId,
        quantity_completed: u64,
        quantity_requested: u64,
    },
    ReservationCountMismatch {
        craftable: CraftableId,
        expected: u64,
        found: usize,
    },
    InventoryOverflow(CraftableId),
}

/// The largest whole number of units `unit_cost` each that `available` can cover — used both to
/// clamp/validate a requested batch quantity and to drive the UI's "MAX" control.
pub fn max_affordable_quantity(available: ResourceStock, unit_cost: ResourceCost) -> u64 {
    if unit_cost.is_zero() {
        return MAX_CRAFT_BATCH_QUANTITY;
    }
    let mut affordable = MAX_CRAFT_BATCH_QUANTITY;
    if let Some(metal_quantity) = available.metal.checked_div(unit_cost.metal) {
        affordable = affordable.min(metal_quantity);
    }
    if let Some(crystal_quantity) = available.crystal.checked_div(unit_cost.crystal) {
        affordable = affordable.min(crystal_quantity);
    }
    if let Some(fuel_quantity) = available.fuel.checked_div(unit_cost.fuel) {
        affordable = affordable.min(fuel_quantity);
    }
    affordable
}

fn scaled_cost(unit_cost: ResourceCost, quantity: u64) -> Option<ResourceCost> {
    Some(ResourceCost::new(
        unit_cost.metal.checked_mul(quantity)?,
        unit_cost.crystal.checked_mul(quantity)?,
        unit_cost.fuel.checked_mul(quantity)?,
    ))
}

pub fn craft_quote(
    state: &GameState,
    actor: FactionId,
    colony_id: ColonyId,
    craftable: CraftableId,
    quantity: u64,
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
    if quantity == 0 || quantity > MAX_CRAFT_BATCH_QUANTITY {
        return Err(CraftError::InvalidQuantity {
            requested: quantity,
            maximum: MAX_CRAFT_BATCH_QUANTITY,
        });
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
    let max_affordable_quantity = max_affordable_quantity(available, definition.cost);
    let total_cost =
        scaled_cost(definition.cost, quantity).ok_or(CraftError::InsufficientResources {
            available,
            cost: definition.cost,
        })?;
    if !available.can_cover(total_cost) {
        return Err(CraftError::InsufficientResources {
            available,
            cost: total_cost,
        });
    }

    let pending_from_existing_orders = colony
        .craft_queue
        .orders()
        .filter(|order| order.craftable == craftable)
        .try_fold(0_u64, |total, order| {
            order
                .result_quantity_per_unit
                .checked_mul(order.quantity_remaining())
                .and_then(|pending| total.checked_add(pending))
        });
    let projected_quantity = pending_from_existing_orders
        .and_then(|pending| {
            definition
                .result_quantity
                .checked_mul(quantity)?
                .checked_add(pending)
        })
        .and_then(|total_pending| {
            colony
                .inventory
                .quantity(craftable)
                .checked_add(total_pending)
        });
    if projected_quantity.is_none() {
        return Err(CraftError::InventoryOverflow(craftable));
    }

    let total_required_work_milli = definition
        .required_work_milli
        .checked_mul(quantity)
        .ok_or(CraftError::InventoryOverflow(craftable))?;

    Ok(CraftQuote {
        colony_id,
        craftable,
        quantity,
        unit_cost: definition.cost,
        total_cost,
        unit_required_work_milli: definition.required_work_milli,
        total_required_work_milli,
        output_milli_per_tick: output,
        estimated_ticks: total_required_work_milli.div_ceil(output),
        result_quantity_per_unit: definition.result_quantity,
        max_affordable_quantity,
    })
}

pub fn enqueue_craft(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
    craftable: CraftableId,
    quantity: u64,
) -> Result<CraftQueued, CraftError> {
    let quote = craft_quote(state, actor, colony_id, craftable, quantity)?;
    let colony = state
        .colony_mut(colony_id)
        .ok_or(CraftError::UnknownColony(colony_id))?;

    let mut reservations = VecDeque::with_capacity(quantity as usize);
    for _ in 0..quantity {
        match colony.resources.reserve(quote.unit_cost) {
            Ok(reservation_id) => reservations.push_back(reservation_id),
            Err(error) => {
                for reservation_id in reservations {
                    colony
                        .resources
                        .release(reservation_id)
                        .expect("a reservation just created by this call still exists");
                }
                return Err(CraftError::Reservation(error));
            }
        }
    }

    let order = CraftOrder {
        craftable,
        unit_cost: quote.unit_cost,
        result_quantity_per_unit: quote.result_quantity_per_unit,
        required_work_milli_per_unit: quote.unit_required_work_milli,
        accumulated_work_milli: 0,
        quantity_requested: quantity,
        quantity_completed: 0,
        reservations,
    };
    colony.craft_queue.push(order);

    Ok(CraftQueued {
        colony_id,
        craftable,
        quantity_requested: quantity,
        queue_length: colony.craft_queue.len(),
    })
}

pub fn cancel_craft(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
) -> Result<CraftCancelled, CraftError> {
    let colony = state
        .colony(colony_id)
        .ok_or(CraftError::UnknownColony(colony_id))?;
    state
        .authorize_management(actor, colony.owner)
        .map_err(CraftError::Access)?;
    if colony.craft_queue.active().is_none() {
        return Err(CraftError::NoActiveOrder);
    }

    let colony = state
        .colony_mut(colony_id)
        .ok_or(CraftError::UnknownColony(colony_id))?;
    let order = colony.craft_queue.pop_front();
    let craftable = order.craftable;
    let quantity_completed = order.quantity_completed;
    let quantity_refunded = order.quantity_remaining();
    for reservation_id in order.reservations {
        colony
            .resources
            .release(reservation_id)
            .map_err(CraftError::Reservation)?;
    }

    Ok(CraftCancelled {
        colony_id,
        craftable,
        quantity_completed,
        quantity_refunded,
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
        let unit_finished = {
            let Some(active) = colony.craft_queue.orders.front_mut() else {
                break;
            };
            let spent = active.remaining_work_milli().min(budget);
            active.accumulated_work_milli = active.accumulated_work_milli.saturating_add(spent);
            budget -= spent;
            active.accumulated_work_milli >= active.required_work_milli_per_unit
        };

        if !unit_finished {
            continue;
        }

        let (
            craftable,
            result_quantity_per_unit,
            reservation_id,
            quantity_completed,
            quantity_remaining,
            batch_finished,
        ) = {
            let active = colony
                .craft_queue
                .orders
                .front_mut()
                .expect("the order just progressed above still exists");
            let reservation_id = active
                .reservations
                .pop_front()
                .expect("a completed unit always has a matching reservation");
            active.quantity_completed = active.quantity_completed.saturating_add(1);
            active.accumulated_work_milli = 0;
            (
                active.craftable,
                active.result_quantity_per_unit,
                reservation_id,
                active.quantity_completed,
                active.quantity_remaining(),
                active.reservations.is_empty(),
            )
        };

        colony.resources.commit(reservation_id)?;
        colony.inventory.add(craftable, result_quantity_per_unit);
        completed.push(CraftCompleted {
            colony_id: colony.id,
            craftable,
            quantity_completed,
            quantity_remaining,
            inventory_quantity: colony.inventory.quantity(craftable),
        });

        if batch_finished {
            colony.craft_queue.pop_front();
        }
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

pub fn craft_progress_ratio(order: &CraftOrder) -> f32 {
    if order.required_work_milli_per_unit == 0 {
        return 1.0;
    }
    (order.accumulated_work_milli as f64 / order.required_work_milli_per_unit as f64)
        .clamp(0.0, 1.0) as f32
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
        if order.unit_cost != definition.cost {
            return Err(CraftStateError::InvalidCost {
                craftable: order.craftable,
                expected: definition.cost,
                found: order.unit_cost,
            });
        }
        if order.required_work_milli_per_unit != definition.required_work_milli {
            return Err(CraftStateError::InvalidRequiredWork {
                craftable: order.craftable,
                expected: definition.required_work_milli,
                found: order.required_work_milli_per_unit,
            });
        }
        if order.required_work_milli_per_unit == 0
            || order.accumulated_work_milli >= order.required_work_milli_per_unit
        {
            return Err(CraftStateError::InvalidProgress {
                craftable: order.craftable,
                accumulated: order.accumulated_work_milli,
                required: order.required_work_milli_per_unit,
            });
        }
        if order.result_quantity_per_unit != definition.result_quantity {
            return Err(CraftStateError::InvalidResultQuantity {
                craftable: order.craftable,
                expected: definition.result_quantity,
                found: order.result_quantity_per_unit,
            });
        }
        if order.quantity_completed > order.quantity_requested {
            return Err(CraftStateError::InvalidQuantity {
                craftable: order.craftable,
                quantity_completed: order.quantity_completed,
                quantity_requested: order.quantity_requested,
            });
        }
        if order.reservations.len() as u64 != order.quantity_remaining() {
            return Err(CraftStateError::ReservationCountMismatch {
                craftable: order.craftable,
                expected: order.quantity_remaining(),
                found: order.reservations.len(),
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
        for reservation_id in &order.reservations {
            if !reservations.insert(*reservation_id) {
                return Err(CraftStateError::DuplicateReservation(*reservation_id));
            }
            let reservation = colony
                .resources
                .reservations()
                .iter()
                .find(|reservation| reservation.id == *reservation_id)
                .ok_or(CraftStateError::MissingReservation(*reservation_id))?;
            if reservation.cost != order.unit_cost {
                return Err(CraftStateError::ReservationCostMismatch {
                    reservation_id: *reservation_id,
                    expected: order.unit_cost,
                    found: reservation.cost,
                });
            }
        }
        let Some(total) = order
            .result_quantity_per_unit
            .checked_mul(order.quantity_remaining())
            .and_then(|pending| {
                projected_inventory
                    .quantity(order.craftable)
                    .checked_add(pending)
            })
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
    MissingCombatStats(CraftableId),
    UnexpectedCombatStats(CraftableId),
    InvalidCombatStats(CraftableId),
    InvalidCombatBonus(CraftableId),
    DuplicateCombatBonus {
        craftable: CraftableId,
        target_class: CombatTargetClass,
    },
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

#[derive(Debug, Clone, Deserialize)]
struct ShipDefinitionConfig {
    class: ShipClass,
    cruise_speed: u64,
    range_hops: u16,
    cargo_capacity: u64,
    fuel_per_hop: u64,
    #[serde(default)]
    combat: Option<ShipCombatStatsConfig>,
}

impl ShipDefinitionConfig {
    fn compile(self, craftable: CraftableId) -> Result<ShipDefinition, CraftCatalogError> {
        let combat = self
            .combat
            .map(|combat| combat.compile(craftable))
            .transpose()?;
        Ok(ShipDefinition {
            class: self.class,
            cruise_speed: self.cruise_speed,
            range_hops: self.range_hops,
            cargo_capacity: self.cargo_capacity,
            fuel_per_hop: self.fuel_per_hop,
            combat,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ShipCombatStatsConfig {
    offense: u32,
    defense: u32,
    durability: u32,
    target_class: CombatTargetClass,
    #[serde(default)]
    bonuses: Vec<CombatTargetBonusConfig>,
}

impl ShipCombatStatsConfig {
    fn compile(self, craftable: CraftableId) -> Result<ShipCombatStats, CraftCatalogError> {
        if self.offense == 0
            || self.defense == 0
            || self.durability == 0
            || self.offense > COMBAT_STAT_LIMIT
            || self.defense > COMBAT_STAT_LIMIT
            || self.durability > COMBAT_STAT_LIMIT
        {
            return Err(CraftCatalogError::InvalidCombatStats(craftable));
        }

        let mut seen_targets = BTreeSet::new();
        let mut bonuses = CombatTargetBonuses::default();
        for bonus in self.bonuses {
            if bonus.offense_multiplier_per_mille == 0
                || bonus.offense_multiplier_per_mille > COMBAT_BONUS_PER_MILLE_LIMIT
            {
                return Err(CraftCatalogError::InvalidCombatBonus(craftable));
            }
            if !seen_targets.insert(bonus.target_class) {
                return Err(CraftCatalogError::DuplicateCombatBonus {
                    craftable,
                    target_class: bonus.target_class,
                });
            }
            match bonus.target_class {
                CombatTargetClass::Light => {
                    bonuses.light_per_mille = bonus.offense_multiplier_per_mille;
                }
                CombatTargetClass::Medium => {
                    bonuses.medium_per_mille = bonus.offense_multiplier_per_mille;
                }
                CombatTargetClass::Heavy => {
                    bonuses.heavy_per_mille = bonus.offense_multiplier_per_mille;
                }
            }
        }

        Ok(ShipCombatStats {
            offense: self.offense,
            defense: self.defense,
            durability: self.durability,
            target_class: self.target_class,
            bonuses,
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct CombatTargetBonusConfig {
    target_class: CombatTargetClass,
    offense_multiplier_per_mille: u32,
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

    #[test]
    fn unknown_craftable_id_key_fails_deserialization_cleanly() {
        let result = ron::de::from_str::<CraftableId>("\"not_a_real_craftable\"");
        assert!(result.is_err());
    }

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
        assert_eq!(craftable_catalog().ids().count(), 9);
        assert_eq!(
            craftable_definition(CraftableId::LIGHT_PROBE).category,
            CraftableCategory::Probe,
        );
        assert_eq!(
            craftable_definition(CraftableId::CARTOGRAPHER_SATELLITE).category,
            CraftableCategory::Probe,
        );
        assert_eq!(
            craftable_definition(CraftableId::LIGHT_CARGO)
                .ship
                .expect("cargo is a ship")
                .cargo_capacity,
            500,
        );
        assert_eq!(
            craftable_definition(CraftableId::MERIDIAN_CARRIER)
                .ship
                .expect("cargo is a ship")
                .cargo_capacity,
            1_600,
        );
        assert_eq!(
            craftable_definition(CraftableId::ATLAS_CARGO)
                .ship
                .expect("cargo is a ship")
                .cargo_capacity,
            4_200,
        );
        assert_eq!(
            craftable_definition(CraftableId::FRIGATE_BULWARK).category,
            CraftableCategory::Military,
        );
        assert!(
            craftable_definition(CraftableId::NEEDLE_INTERCEPTOR)
                .ship
                .expect("interceptor is a ship")
                .combat
                .is_some()
        );
        assert!(
            craftable_definition(CraftableId::BASTION_CRUISER)
                .ship
                .expect("cruiser is a ship")
                .combat
                .is_some()
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
            1,
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
            1,
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
                1,
            ),
            Err(CraftError::MissingTechnology(
                TechnologyId::SPATIAL_DETECTION,
            )),
        );
    }

    #[test]
    fn a_batch_reserves_the_total_cost_and_a_second_batch_fails_atomically_when_unaffordable() {
        let mut simulation = ready_simulation();
        let colony_id = simulation.state().colonies[0].id;
        let actor = simulation.state().player_faction;
        let unit_cost = craftable_definition(CraftableId::LIGHT_PROBE).cost;
        let available_before = simulation
            .state()
            .colony(colony_id)
            .expect("colony exists")
            .resources
            .available();
        let max_quantity = max_affordable_quantity(available_before, unit_cost);
        assert!(
            max_quantity >= 1,
            "the test colony's credited stock must afford at least one probe",
        );

        enqueue_craft(
            simulation.state_mut(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
            max_quantity,
        )
        .expect("the maximum affordable quantity must always be queueable");

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert_eq!(
            colony.craft_queue.len(),
            1,
            "a batch occupies a single queue slot"
        );
        assert_eq!(
            colony
                .craft_queue
                .active()
                .expect("batch queued")
                .reservations
                .len(),
            max_quantity as usize,
            "one reservation per not-yet-completed unit",
        );

        // One unit beyond what's affordable must be rejected without reserving anything, and
        // the batch's own reservations must be untouched.
        let error = craft_quote(
            simulation.state(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
            max_quantity + 1,
        );
        assert!(matches!(
            error,
            Err(CraftError::InsufficientResources { .. })
        ));
        assert_eq!(
            simulation
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .resources
                .reservations()
                .len(),
            max_quantity as usize,
            "a failed quote must not create any reservation",
        );
    }

    #[test]
    fn batch_units_complete_one_at_a_time_and_credit_inventory_incrementally() {
        let mut simulation = ready_simulation();
        let colony_id = simulation.state().colonies[0].id;
        let actor = simulation.state().player_faction;
        enqueue_craft(
            simulation.state_mut(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
            3,
        )
        .expect("3 probes fit the credited stock");

        // Advance far enough for exactly one unit (the same per-unit duration the single-unit
        // test already relies on), then check the batch reflects one completed / two remaining
        // instead of jumping straight to fully done.
        let completed = advance_colony_craft(
            simulation
                .state_mut()
                .colony_mut(colony_id)
                .expect("colony exists"),
            StrategicDuration::from_ticks(450),
        )
        .expect("reservation commits");

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].quantity_completed, 1);
        assert_eq!(completed[0].quantity_remaining, 2);
        assert_eq!(completed[0].inventory_quantity, 1);
        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert_eq!(
            colony.craft_queue.len(),
            1,
            "the batch stays queued until fully built"
        );
        assert_eq!(
            colony
                .craft_queue
                .active()
                .expect("batch queued")
                .reservations
                .len(),
            2,
        );
        assert_eq!(colony.inventory.quantity(CraftableId::LIGHT_PROBE), 1);

        // Finish the remaining two units.
        let completed = advance_colony_craft(
            simulation
                .state_mut()
                .colony_mut(colony_id)
                .expect("colony exists"),
            StrategicDuration::from_ticks(900),
        )
        .expect("reservation commits");
        assert_eq!(completed.len(), 2);
        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert!(
            colony.craft_queue.is_empty(),
            "a fully built batch leaves the queue"
        );
        assert_eq!(colony.inventory.quantity(CraftableId::LIGHT_PROBE), 3);
    }

    #[test]
    fn cancelling_a_partially_completed_batch_keeps_produced_units_and_refunds_the_rest() {
        let mut simulation = ready_simulation();
        let colony_id = simulation.state().colonies[0].id;
        let actor = simulation.state().player_faction;
        enqueue_craft(
            simulation.state_mut(),
            actor,
            colony_id,
            CraftableId::LIGHT_PROBE,
            3,
        )
        .expect("3 probes fit the credited stock");
        let available_after_queueing = simulation
            .state()
            .colony(colony_id)
            .expect("colony exists")
            .resources
            .available();

        advance_colony_craft(
            simulation
                .state_mut()
                .colony_mut(colony_id)
                .expect("colony exists"),
            StrategicDuration::from_ticks(450),
        )
        .expect("one unit completes");

        let cancelled = cancel_craft(simulation.state_mut(), actor, colony_id)
            .expect("an active batch can be cancelled");
        assert_eq!(cancelled.craftable, CraftableId::LIGHT_PROBE);
        assert_eq!(cancelled.quantity_completed, 1);
        assert_eq!(cancelled.quantity_refunded, 2);

        let colony = simulation.state().colony(colony_id).expect("colony exists");
        assert!(colony.craft_queue.is_empty());
        assert_eq!(
            colony.inventory.quantity(CraftableId::LIGHT_PROBE),
            1,
            "the already-produced unit is kept",
        );
        // The unit that had already committed its reservation is gone for good, but the 2
        // remaining units' reservations must be fully returned to availability.
        let unit_cost = craftable_definition(CraftableId::LIGHT_PROBE).cost;
        let refund = ResourceStock::new(
            unit_cost.metal * 2,
            unit_cost.crystal * 2,
            unit_cost.fuel * 2,
        );
        assert_eq!(
            colony.resources.available(),
            available_after_queueing
                .checked_add(refund)
                .expect("refund must not overflow in this test"),
        );
    }

    #[test]
    fn cancelling_without_an_active_batch_is_an_explicit_error() {
        let mut simulation = ready_simulation();
        let colony_id = simulation.state().colonies[0].id;
        let actor = simulation.state().player_faction;

        assert_eq!(
            cancel_craft(simulation.state_mut(), actor, colony_id),
            Err(CraftError::NoActiveOrder),
        );
    }
}

// MVP-020: faction-owned fleets assembled atomically from docked ships.
use std::collections::BTreeMap;

use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, Owned, Owner, ResourceStock, SystemId,
};

use crate::{
    AuthorizationError, CraftInventory, CraftableCatalog, CraftableId, GameState, ShipClass,
    default_ruleset,
};

pub const MAX_FLEET_SHIP_STACKS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipStack {
    pub craftable: CraftableId,
    pub quantity: u64,
}

impl ShipStack {
    pub const fn new(craftable: CraftableId, quantity: u64) -> Self {
        Self {
            craftable,
            quantity,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FleetComposition {
    ships: BTreeMap<CraftableId, u64>,
}

impl FleetComposition {
    pub fn from_stacks(
        stacks: impl IntoIterator<Item = ShipStack>,
    ) -> Result<Self, FleetCompositionError> {
        let catalog = default_ruleset().craftables();
        let mut ships = BTreeMap::new();
        for stack in stacks {
            if stack.quantity == 0 {
                return Err(FleetCompositionError::ZeroQuantity(stack.craftable));
            }
            let Some(definition) = catalog.get(stack.craftable) else {
                return Err(FleetCompositionError::UnknownCraftable(stack.craftable));
            };
            if definition.ship.is_none() {
                return Err(FleetCompositionError::NotShip(stack.craftable));
            }
            if ships.insert(stack.craftable, stack.quantity).is_some() {
                return Err(FleetCompositionError::DuplicateShip(stack.craftable));
            }
            if ships.len() > MAX_FLEET_SHIP_STACKS {
                return Err(FleetCompositionError::TooManyShipStacks {
                    found: ships.len(),
                    maximum: MAX_FLEET_SHIP_STACKS,
                });
            }
        }
        if ships.is_empty() {
            return Err(FleetCompositionError::Empty);
        }
        let composition = Self { ships };
        fleet_capabilities_with_catalog(&composition, catalog)?;
        Ok(composition)
    }

    pub fn quantity(&self, craftable: CraftableId) -> u64 {
        self.ships.get(&craftable).copied().unwrap_or(0)
    }

    pub fn entries(&self) -> impl Iterator<Item = ShipStack> + '_ {
        self.ships
            .iter()
            .map(|(craftable, quantity)| ShipStack::new(*craftable, *quantity))
    }

    pub fn total_ships(&self) -> u64 {
        self.ships
            .values()
            .copied()
            .fold(0_u64, u64::saturating_add)
    }

    pub fn capabilities(&self) -> Result<FleetCapabilities, FleetCompositionError> {
        fleet_capabilities_with_catalog(self, default_ruleset().craftables())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetCapabilities {
    pub cruise_speed: u64,
    pub range_hops: u16,
    pub cargo_capacity: u64,
    pub fuel_per_hop: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetLocation {
    Docked(ColonyId),
    InSystem(SystemId),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FleetAssignment {
    #[default]
    Idle,
    Mission(MissionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetState {
    pub id: FleetId,
    pub owner: Owner,
    pub location: FleetLocation,
    pub composition: FleetComposition,
    pub cargo: ResourceStock,
    pub assignment: FleetAssignment,
}

impl FleetState {
    pub fn capabilities(&self) -> Result<FleetCapabilities, FleetCompositionError> {
        self.composition.capabilities()
    }

    pub const fn is_idle(&self) -> bool {
        matches!(self.assignment, FleetAssignment::Idle)
    }
}

impl Owned for FleetState {
    fn owner(&self) -> Owner {
        self.owner
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetCreated {
    pub fleet_id: FleetId,
    pub colony_id: ColonyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetCreationRejected {
    pub colony_id: ColonyId,
    pub error: FleetError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetCompositionError {
    Empty,
    TooManyShipStacks { found: usize, maximum: usize },
    ZeroQuantity(CraftableId),
    DuplicateShip(CraftableId),
    UnknownCraftable(CraftableId),
    NotShip(CraftableId),
    ShipCountOverflow,
    CargoCapacityOverflow,
    FuelConsumptionOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetError {
    UnknownColony(ColonyId),
    Access(AuthorizationError),
    InvalidComposition(FleetCompositionError),
    InsufficientDockedShips {
        craftable: CraftableId,
        requested: u64,
        available: u64,
    },
    FleetIdOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetStateError {
    InvalidComposition(FleetCompositionError),
    CargoExceedsCapacity { cargo: ResourceStock, capacity: u64 },
}

pub fn form_fleet(
    state: &mut GameState,
    actor: FactionId,
    colony_id: ColonyId,
    composition: FleetComposition,
) -> Result<FleetCreated, FleetError> {
    composition
        .capabilities()
        .map_err(FleetError::InvalidComposition)?;
    let colony = state
        .colony(colony_id)
        .ok_or(FleetError::UnknownColony(colony_id))?;
    state
        .authorize_management(actor, colony.owner)
        .map_err(FleetError::Access)?;
    for stack in composition.entries() {
        let available = colony.inventory.quantity(stack.craftable);
        if available < stack.quantity {
            return Err(FleetError::InsufficientDockedShips {
                craftable: stack.craftable,
                requested: stack.quantity,
                available,
            });
        }
    }

    let next_fleet_id = state
        .next_fleet_id
        .checked_add(1)
        .ok_or(FleetError::FleetIdOverflow)?;
    let fleet_id = FleetId::new(state.next_fleet_id);
    let owner = colony.owner;
    let colony = state
        .colony_mut(colony_id)
        .expect("validated colony must remain present");
    for stack in composition.entries() {
        let removed = colony.inventory.take(stack.craftable, stack.quantity);
        debug_assert!(removed, "validated inventory allocation must succeed");
    }
    state.next_fleet_id = next_fleet_id;
    state.fleets.push(FleetState {
        id: fleet_id,
        owner,
        location: FleetLocation::Docked(colony_id),
        composition,
        cargo: ResourceStock::ZERO,
        assignment: FleetAssignment::Idle,
    });
    state.fleets.sort_by_key(|fleet| fleet.id);

    Ok(FleetCreated {
        fleet_id,
        colony_id,
    })
}

pub fn validate_fleet_state(fleet: &FleetState) -> Result<(), FleetStateError> {
    let capabilities = fleet
        .composition
        .capabilities()
        .map_err(FleetStateError::InvalidComposition)?;
    let cargo_total = fleet
        .cargo
        .metal
        .checked_add(fleet.cargo.crystal)
        .and_then(|total| total.checked_add(fleet.cargo.fuel))
        .ok_or(FleetStateError::CargoExceedsCapacity {
            cargo: fleet.cargo,
            capacity: capabilities.cargo_capacity,
        })?;
    if cargo_total > capabilities.cargo_capacity {
        return Err(FleetStateError::CargoExceedsCapacity {
            cargo: fleet.cargo,
            capacity: capabilities.cargo_capacity,
        });
    }
    Ok(())
}

pub fn docked_ship_quantity(inventory: &CraftInventory, craftable: CraftableId) -> u64 {
    inventory.quantity(craftable)
}

fn fleet_capabilities_with_catalog(
    composition: &FleetComposition,
    catalog: &CraftableCatalog,
) -> Result<FleetCapabilities, FleetCompositionError> {
    if composition.ships.is_empty() {
        return Err(FleetCompositionError::Empty);
    }
    if composition.ships.len() > MAX_FLEET_SHIP_STACKS {
        return Err(FleetCompositionError::TooManyShipStacks {
            found: composition.ships.len(),
            maximum: MAX_FLEET_SHIP_STACKS,
        });
    }

    let mut cruise_speed = u64::MAX;
    let mut range_hops = u16::MAX;
    let mut cargo_capacity = 0_u64;
    let mut fuel_per_hop = 0_u64;
    let mut total_ships = 0_u64;
    for stack in composition.entries() {
        if stack.quantity == 0 {
            return Err(FleetCompositionError::ZeroQuantity(stack.craftable));
        }
        let Some(definition) = catalog.get(stack.craftable) else {
            return Err(FleetCompositionError::UnknownCraftable(stack.craftable));
        };
        let Some(ship) = definition.ship else {
            return Err(FleetCompositionError::NotShip(stack.craftable));
        };
        total_ships = total_ships
            .checked_add(stack.quantity)
            .ok_or(FleetCompositionError::ShipCountOverflow)?;
        cruise_speed = cruise_speed.min(ship.cruise_speed);
        range_hops = range_hops.min(ship.range_hops);
        cargo_capacity = cargo_capacity
            .checked_add(
                ship.cargo_capacity
                    .checked_mul(stack.quantity)
                    .ok_or(FleetCompositionError::CargoCapacityOverflow)?,
            )
            .ok_or(FleetCompositionError::CargoCapacityOverflow)?;
        fuel_per_hop = fuel_per_hop
            .checked_add(
                ship.fuel_per_hop
                    .checked_mul(stack.quantity)
                    .ok_or(FleetCompositionError::FuelConsumptionOverflow)?,
            )
            .ok_or(FleetCompositionError::FuelConsumptionOverflow)?;
    }
    debug_assert!(total_ships > 0);

    Ok(FleetCapabilities {
        cruise_speed,
        range_hops,
        cargo_capacity,
        fuel_per_hop,
    })
}

pub const fn ship_class_label(class: ShipClass) -> &'static str {
    match class {
        ShipClass::Probe => "Sonde",
        ShipClass::Cargo => "Cargo",
        ShipClass::Colony => "Colonie",
        ShipClass::Military => "Militaire",
        ShipClass::Support => "Soutien",
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{Owner, UniverseConfig};

    use crate::{CraftableId, Simulation};

    use super::*;

    fn simulation_with_docked_ships() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.inventory.add(CraftableId::LIGHT_PROBE, 2);
        colony.inventory.add(CraftableId::LIGHT_CARGO, 2);
        colony.inventory.add(CraftableId::COLONY_SHIP, 1);
        simulation
    }

    #[test]
    fn fleet_is_formed_atomically_from_docked_inventory() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let composition = FleetComposition::from_stacks([
            ShipStack::new(CraftableId::LIGHT_PROBE, 1),
            ShipStack::new(CraftableId::LIGHT_CARGO, 2),
        ])
        .expect("composition is valid");

        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("fleet can be formed");

        let fleet = simulation
            .state()
            .fleet(created.fleet_id)
            .expect("fleet exists");
        assert_eq!(fleet.owner, Owner::Faction(actor));
        assert_eq!(fleet.location, FleetLocation::Docked(colony_id));
        assert_eq!(fleet.composition.total_ships(), 3);
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_PROBE),
            1,
        );
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            0,
        );
    }

    #[test]
    fn mixed_fleet_uses_slowest_ship_and_sums_capacity() {
        let composition = FleetComposition::from_stacks([
            ShipStack::new(CraftableId::LIGHT_PROBE, 2),
            ShipStack::new(CraftableId::LIGHT_CARGO, 3),
        ])
        .expect("composition is valid");

        assert_eq!(
            composition.capabilities(),
            Ok(FleetCapabilities {
                cruise_speed: 100,
                range_hops: 3,
                cargo_capacity: 2_400,
                fuel_per_hop: 57,
            }),
        );
    }

    #[test]
    fn a_ship_cannot_be_allocated_to_two_fleets() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let all_cargo =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 2)])
                .expect("composition is valid");
        form_fleet(simulation.state_mut(), actor, colony_id, all_cargo)
            .expect("first allocation succeeds");
        let another = FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
            .expect("composition is valid");

        assert_eq!(
            form_fleet(simulation.state_mut(), actor, colony_id, another),
            Err(FleetError::InsufficientDockedShips {
                craftable: CraftableId::LIGHT_CARGO,
                requested: 1,
                available: 0,
            }),
        );
        assert_eq!(simulation.state().fleets.len(), 1);
    }
}

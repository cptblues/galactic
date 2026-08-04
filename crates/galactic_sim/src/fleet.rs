// MVP-020: faction-owned fleets assembled atomically from docked ships.
use std::collections::BTreeMap;

use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, Owned, Owner, ResourceStock, SystemId,
};

use crate::{
    AuthorizationError, CraftInventory, CraftableCatalog, CraftableId, GameState, ShipClass,
    default_ruleset, storage_capacity,
};

pub const MAX_FLEET_SHIP_STACKS: usize = 128;
pub const MAX_FLEET_NAME_CHARS: usize = 32;

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
    pub name: String,
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
pub struct FleetRenamed {
    pub fleet_id: FleetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetRenameRejected {
    pub fleet_id: FleetId,
    pub error: FleetError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetDisbanded {
    pub fleet_id: FleetId,
    pub colony_id: ColonyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetDisbandRejected {
    pub fleet_id: FleetId,
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
    UnknownFleet(FleetId),
    Access(AuthorizationError),
    InvalidComposition(FleetCompositionError),
    InsufficientDockedShips {
        craftable: CraftableId,
        requested: u64,
        available: u64,
    },
    FleetNameEmpty,
    FleetNameTooLong {
        found: usize,
        maximum: usize,
    },
    FleetNameContainsControlCharacter,
    FleetNotIdle(FleetId),
    FleetNotDocked(FleetId),
    FleetCargoNotEmpty(FleetId),
    FleetDockOwnerMismatch {
        fleet_id: FleetId,
        colony_id: ColonyId,
    },
    FleetIdOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetStateError {
    EmptyName,
    NameTooLong { found: usize, maximum: usize },
    NameContainsControlCharacter,
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
        name: default_fleet_name(fleet_id),
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

pub fn rename_fleet(
    state: &mut GameState,
    actor: FactionId,
    fleet_id: FleetId,
    name: impl AsRef<str>,
) -> Result<FleetRenamed, FleetError> {
    let validated = validate_fleet_name(name.as_ref())?;
    let fleet = state
        .fleet(fleet_id)
        .ok_or(FleetError::UnknownFleet(fleet_id))?;
    state
        .authorize_management(actor, fleet.owner)
        .map_err(FleetError::Access)?;
    let fleet = state
        .fleet_mut(fleet_id)
        .expect("validated fleet must remain present");
    fleet.name = validated;
    Ok(FleetRenamed { fleet_id })
}

pub fn disband_fleet(
    state: &mut GameState,
    actor: FactionId,
    fleet_id: FleetId,
) -> Result<FleetDisbanded, FleetError> {
    let index = state
        .fleets
        .iter()
        .position(|fleet| fleet.id == fleet_id)
        .ok_or(FleetError::UnknownFleet(fleet_id))?;
    let fleet = &state.fleets[index];
    state
        .authorize_management(actor, fleet.owner)
        .map_err(FleetError::Access)?;
    if !fleet.is_idle() {
        return Err(FleetError::FleetNotIdle(fleet_id));
    }
    let FleetLocation::Docked(colony_id) = fleet.location else {
        return Err(FleetError::FleetNotDocked(fleet_id));
    };
    let Some(colony) = state.colony(colony_id) else {
        return Err(FleetError::UnknownColony(colony_id));
    };
    if colony.owner != fleet.owner {
        return Err(FleetError::FleetDockOwnerMismatch {
            fleet_id,
            colony_id,
        });
    }
    if fleet.cargo != ResourceStock::ZERO {
        let capacity = storage_capacity(colony.buildings);
        let accepted = {
            let mut resources = colony.resources.clone();
            resources.credit_capped(fleet.cargo, capacity)
        };
        if accepted != fleet.cargo {
            return Err(FleetError::FleetCargoNotEmpty(fleet_id));
        }
    }
    let fleet = state.fleets.remove(index);
    let colony = state
        .colony_mut(colony_id)
        .expect("validated docked colony must remain present");
    if fleet.cargo != ResourceStock::ZERO {
        let capacity = storage_capacity(colony.buildings);
        let accepted = colony.resources.credit_capped(fleet.cargo, capacity);
        debug_assert_eq!(accepted, fleet.cargo);
    }
    for stack in fleet.composition.entries() {
        colony.inventory.add(stack.craftable, stack.quantity);
    }
    Ok(FleetDisbanded {
        fleet_id,
        colony_id,
    })
}

pub fn validate_fleet_state(fleet: &FleetState) -> Result<(), FleetStateError> {
    validate_fleet_name_for_state(&fleet.name)?;
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

pub fn default_fleet_name(fleet_id: FleetId) -> String {
    format!("Flotte {}", fleet_id.raw() + 1)
}

fn validate_fleet_name(name: &str) -> Result<String, FleetError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FleetError::FleetNameEmpty);
    }
    let length = trimmed.chars().count();
    if length > MAX_FLEET_NAME_CHARS {
        return Err(FleetError::FleetNameTooLong {
            found: length,
            maximum: MAX_FLEET_NAME_CHARS,
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(FleetError::FleetNameContainsControlCharacter);
    }
    Ok(trimmed.to_string())
}

fn validate_fleet_name_for_state(name: &str) -> Result<(), FleetStateError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(FleetStateError::EmptyName);
    }
    let length = trimmed.chars().count();
    if length > MAX_FLEET_NAME_CHARS {
        return Err(FleetStateError::NameTooLong {
            found: length,
            maximum: MAX_FLEET_NAME_CHARS,
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(FleetStateError::NameContainsControlCharacter);
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
    use galactic_domain::{MissionId, Owner, ResourceStock, SystemId, UniverseConfig};

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
        assert_eq!(fleet.name, "Flotte 1");
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
    fn fleet_can_be_renamed_with_validation() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("fleet can be formed");

        rename_fleet(
            simulation.state_mut(),
            actor,
            created.fleet_id,
            "  Cartographie Alpha  ",
        )
        .expect("fleet can be renamed");

        assert_eq!(
            simulation.state().fleet(created.fleet_id).unwrap().name,
            "Cartographie Alpha"
        );
        assert_eq!(
            rename_fleet(simulation.state_mut(), actor, created.fleet_id, "   "),
            Err(FleetError::FleetNameEmpty),
        );
        assert_eq!(
            simulation.state().fleet(created.fleet_id).unwrap().name,
            "Cartographie Alpha"
        );
    }

    #[test]
    fn idle_docked_fleet_can_be_disbanded_without_losing_ships() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("fleet can be formed");

        let disbanded = disband_fleet(simulation.state_mut(), actor, created.fleet_id)
            .expect("fleet can be disbanded");

        assert_eq!(disbanded.colony_id, colony_id);
        assert!(simulation.state().fleet(created.fleet_id).is_none());
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_PROBE),
            2
        );
    }

    #[test]
    fn disbanding_a_docked_fleet_deposits_its_cargo() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("fleet can be formed");
        let cargo = ResourceStock::new(1, 2, 3);
        let before = simulation.state().colonies[0].resources.available();
        simulation
            .state_mut()
            .fleet_mut(created.fleet_id)
            .unwrap()
            .cargo = cargo;

        disband_fleet(simulation.state_mut(), actor, created.fleet_id)
            .expect("loaded docked fleet can be disbanded when storage has room");

        assert!(simulation.state().fleet(created.fleet_id).is_none());
        assert_eq!(
            simulation.state().colonies[0].resources.available(),
            before
                .checked_add(cargo)
                .expect("test cargo does not overflow"),
        );
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            2
        );
    }

    #[test]
    fn disband_rejects_fleets_that_are_busy_or_remote() {
        let mut simulation = simulation_with_docked_ships();
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("composition is valid");
        let created = form_fleet(simulation.state_mut(), actor, colony_id, composition)
            .expect("fleet can be formed");

        let fleet = simulation.state_mut().fleet_mut(created.fleet_id).unwrap();
        fleet.assignment = FleetAssignment::Mission(MissionId::new(7));
        assert_eq!(
            disband_fleet(simulation.state_mut(), actor, created.fleet_id),
            Err(FleetError::FleetNotIdle(created.fleet_id)),
        );

        let fleet = simulation.state_mut().fleet_mut(created.fleet_id).unwrap();
        fleet.assignment = FleetAssignment::Idle;
        fleet.location = FleetLocation::InSystem(SystemId::new(0));
        assert_eq!(
            disband_fleet(simulation.state_mut(), actor, created.fleet_id),
            Err(FleetError::FleetNotDocked(created.fleet_id)),
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
                cruise_speed: 130,
                range_hops: 3,
                cargo_capacity: 1_500,
                fuel_per_hop: 48,
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

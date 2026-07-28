#!/usr/bin/env python3
"""Apply Galactic MVP-020 safely from the exact pushed baseline.

The migration introduces configurable ship definitions, faction-owned fleets,
atomic allocation of docked ships and fleet persistence. A dry-run performs no
Cargo build unless --checks is explicitly requested.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
from textwrap import dedent


def load_shared_helpers():
    candidates = (
        Path(__file__).resolve().with_name("apply_mvp_016_b.py"),
        Path.cwd() / "tools" / "apply_mvp_016_b.py",
        Path(__file__).resolve().parent / "galactic" / "tools" / "apply_mvp_016_b.py",
    )
    helper = next((candidate for candidate in candidates if candidate.is_file()), None)
    if helper is None:
        return None
    spec = importlib.util.spec_from_file_location("apply_mvp_016_b", helper)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


base = load_shared_helpers()
if base is None:
    print(
        "ERREUR : tools/apply_mvp_016_b.py est requis à côté de ce script.",
        file=sys.stderr,
    )
    raise SystemExit(1)


MIGRATION = "MVP-020"
BASELINE_SHA = "fbfbe6b028cb47a1d986174336f27d362dd7262d"

MODIFIED_BLOBS = {
    "README.md": "b904c66380b467d0013327bf25b4b5abac34946f",
    "assets/rulesets/default/craftables.ron": "0acfff1f7f0c8d935744b47a0b2b829420bb6f57",
    "assets/rulesets/default/manifest.ron": "b66d4cbc2c59246fda4ac0ecdff58268682cf0dd",
    "crates/galactic_client/src/lib.rs": "f471190da781da5e331a150b680b99470ae029b4",
    "crates/galactic_persistence/src/lib.rs": "f1347f18f00cc8c8b145e3ee8d5e95a49aa8d8ef",
    "crates/galactic_sim/src/command.rs": "5e83671a51db62ef4b663d2b20badc53c473d434",
    "crates/galactic_sim/src/craft.rs": "4eb99c74cd4717194c9661e86334002a7763e1c3",
    "crates/galactic_sim/src/event.rs": "ff1138070d94dee400a66b57f18e643695a59ebf",
    "crates/galactic_sim/src/lib.rs": "01d5171d9692b147375487a767889e068e091455",
    "crates/galactic_sim/src/ruleset.rs": "fbfa8fe264790f59a1766a7216264f63641fc46b",
    "crates/galactic_sim/src/simulation.rs": "694516bc6b38900c0426b675cb9b075109a3cdf3",
    "crates/galactic_sim/src/state.rs": "bedcd0e7eaa2bf480cc5df2125cc133b33259880",
    "docs/mvp_architecture.md": "e2f8cacb4c97ad4a4003479606568aaffdf853a4",
    "docs/ruleset.md": "32b0fa8771dc721d50a57616a4ae60bcaece1189",
}

DEPENDENCY_BLOBS = {
    "tools/apply_mvp_016_b.py": "1557ff3f419abbf6a1b58b897100aa72da80bd38",
}

CREATED_PATHS = ("crates/galactic_sim/src/fleet.rs",)
EXPECTED_PATHS = frozenset((*MODIFIED_BLOBS, *CREATED_PATHS))

TARGETED_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all"),
    (
        "cargo",
        "check",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "--all-targets",
        "--all-features",
    ),
    (
        "cargo",
        "check",
        "-p",
        "galactic_client",
        "--lib",
        "--all-features",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_sim",
        "-p",
        "galactic_persistence",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    (
        "cargo",
        "clippy",
        "-p",
        "galactic_client",
        "--lib",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    ("cargo", "test", "-p", "galactic_sim", "-p", "galactic_persistence"),
)

FULL_CHECK_COMMANDS = (
    ("cargo", "fmt", "--all"),
    (
        "cargo",
        "check",
        "--workspace",
        "--all-targets",
        "--all-features",
    ),
    (
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ),
    ("cargo", "test", "--workspace"),
    ("cargo", "build", "--release"),
)


CRAFTABLES_RON = """\
(
    version: 2,
    craftables: [
        (
            id: "light_probe",
            name: "Sonde légère",
            description: "Un éclaireur automatisé destiné aux premières opérations de reconnaissance.",
            category: Probe,
            cost: (metal: 200, crystal: 150, fuel: 80),
            base_duration_seconds: 45,
            result_quantity: 1,
            building_prerequisites: [(id: "shipyard", level: 1)],
            technology_prerequisites: ["spatial_detection"],
            capabilities: [(id: "probe_strength", value: 1)],
            ship: Some((
                class: Probe,
                cruise_speed: 160,
                range_hops: 4,
                cargo_capacity: 0,
                fuel_per_hop: 6,
            )),
        ),
        (
            id: "light_cargo",
            name: "Cargo léger",
            description: "Un transport orbital modeste, homologué pour déplacer du matériel utile.",
            category: Transport,
            cost: (metal: 400, crystal: 260, fuel: 180),
            base_duration_seconds: 75,
            result_quantity: 1,
            building_prerequisites: [(id: "shipyard", level: 1)],
            technology_prerequisites: ["propulsion"],
            capabilities: [],
            ship: Some((
                class: Cargo,
                cruise_speed: 100,
                range_hops: 3,
                cargo_capacity: 800,
                fuel_per_hop: 15,
            )),
        ),
        (
            id: "colony_ship",
            name: "Vaisseau-colonie",
            description: "Une administration complète emballée dans une coque pressurisée.",
            category: Colony,
            cost: (metal: 1200, crystal: 900, fuel: 700),
            base_duration_seconds: 180,
            result_quantity: 1,
            building_prerequisites: [(id: "shipyard", level: 2)],
            technology_prerequisites: ["colonization"],
            capabilities: [(id: "colonization_capacity", value: 1)],
            ship: Some((
                class: Colony,
                cruise_speed: 70,
                range_hops: 2,
                cargo_capacity: 300,
                fuel_per_hop: 30,
            )),
        ),
    ],
)
"""


FLEET_RS = r"""// MVP-020: faction-owned fleets assembled atomically from docked ships.
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
    TooManyShipStacks {
        found: usize,
        maximum: usize,
    },
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
    CargoExceedsCapacity {
        cargo: ResourceStock,
        capacity: u64,
    },
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
        let another =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
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
"""


def replace_once(root: Path, relative: str, before: str, after: str) -> None:
    path = root / relative
    text = path.read_text(encoding="utf-8")
    count = text.count(before)
    if count != 1:
        raise base.MigrationError(
            f"{relative}: motif attendu exactement une fois, trouvé {count}"
        )
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def append_once(root: Path, relative: str, marker: str, addition: str) -> None:
    path = root / relative
    text = path.read_text(encoding="utf-8")
    if marker in text:
        raise base.MigrationError(f"{relative}: contenu MVP-020 déjà présent")
    if not text.endswith("\n"):
        raise base.MigrationError(f"{relative}: fin de fichier inattendue")
    path.write_text(text + addition, encoding="utf-8")


def transform_tree(root: Path) -> None:
    (root / "assets/rulesets/default/craftables.ron").write_text(
        CRAFTABLES_RON, encoding="utf-8"
    )
    (root / "assets/rulesets/default/manifest.ron").write_text(
        '(\n    id: "default",\n    schema_version: 5,\n    content_version: 5,\n)\n',
        encoding="utf-8",
    )

    replace_once(
        root,
        "README.md",
        """Le contenu économique actif est chargé depuis `assets/rulesets/default/` au
démarrage. Les coûts, durées, textes, bâtiments, technologies, fabrications,
factions, relations initiales, limites de files et données de départ peuvent être modifiés sans
recompiler le jeu.
""",
        """Le contenu économique actif est chargé depuis `assets/rulesets/default/` au
démarrage. Les coûts, durées, textes, bâtiments, technologies, vaisseaux,
capacités, factions, relations initiales, limites de files et données de départ
peuvent être modifiés sans recompiler le jeu.
""",
    )

    transform_craft(root)
    transform_state(root)
    transform_command_and_events(root)
    transform_simulation(root)
    transform_persistence(root)
    transform_client(root)
    transform_docs(root)

    fleet_path = root / CREATED_PATHS[0]
    fleet_path.parent.mkdir(parents=True, exist_ok=True)
    if fleet_path.exists():
        raise base.MigrationError(f"{CREATED_PATHS[0]} existe déjà")
    fleet_path.write_text(FLEET_RS, encoding="utf-8")


def transform_craft(root: Path) -> None:
    path = "crates/galactic_sim/src/craft.rs"
    replace_once(
        root,
        path,
        """#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CraftCapability {
    pub id: CraftCapabilityId,
    pub value: u64,
}

""",
        """#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

""",
    )
    replace_once(
        root,
        path,
        "    pub capabilities: Vec<CraftCapability>,\n",
        "    pub capabilities: Vec<CraftCapability>,\n    pub ship: Option<ShipDefinition>,\n",
    )
    replace_once(root, path, "        if config.version != 1 {\n", "        if config.version != 2 {\n")
    replace_once(
        root,
        path,
        """                    technology_prerequisites,
                    capabilities,
                },
""",
        """                    technology_prerequisites,
                    capabilities,
                    ship: craftable.ship.map(ShipDefinitionConfig::compile),
                },
""",
    )
    replace_once(
        root,
        path,
        """            output.push_str("];");
""",
        """            output.push('|');
            if let Some(ship) = definition.ship {
                output.push_str("ship:");
                output.push_str(ship.class.structural_key());
                output.push_str(":cruise_speed:range_hops:cargo_capacity:fuel_per_hop");
            }
            output.push_str("];");
""",
    )
    replace_once(
        root,
        path,
        """                if !capabilities.insert(capability.id) {
                    return Err(CraftCatalogError::DuplicateCapability {
                        craftable: definition.id,
                        capability: capability.id,
                    });
                }
            }
        }
        Ok(())
""",
        """                if !capabilities.insert(capability.id) {
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
""",
    )
    replace_once(
        root,
        path,
        """    fn add(&mut self, craftable: CraftableId, quantity: u64) {
        let total = self
            .quantity(craftable)
            .checked_add(quantity)
            .expect("validated craft inventory cannot overflow");
        self.quantities.insert(craftable, total);
    }
""",
        """    pub(crate) fn add(&mut self, craftable: CraftableId, quantity: u64) {
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
""",
    )
    replace_once(
        root,
        path,
        """    DuplicateCapability {
        craftable: CraftableId,
        capability: CraftCapabilityId,
    },
}
""",
        """    DuplicateCapability {
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
""",
    )
    replace_once(
        root,
        path,
        """    technology_prerequisites: Vec<String>,
    capabilities: Vec<CraftCapabilityConfig>,
}
""",
        """    technology_prerequisites: Vec<String>,
    capabilities: Vec<CraftCapabilityConfig>,
    #[serde(default)]
    ship: Option<ShipDefinitionConfig>,
}
""",
    )
    replace_once(
        root,
        path,
        """struct CraftCapabilityConfig {
    id: String,
    value: u64,
}

""",
        """struct CraftCapabilityConfig {
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

""",
    )
    replace_once(root, path, "        assert_eq!(craftable_catalog().version(), 1);\n", "        assert_eq!(craftable_catalog().version(), 2);\n")
    replace_once(
        root,
        path,
        """            CraftableCategory::Probe,
        );
    }
""",
        """            CraftableCategory::Probe,
        );
        assert_eq!(
            craftable_definition(CraftableId::LIGHT_CARGO)
                .ship
                .expect("cargo is a ship")
                .cargo_capacity,
            800,
        );
    }
""",
    )


def transform_state(root: Path) -> None:
    path = "crates/galactic_sim/src/state.rs"
    replace_once(
        root,
        path,
        """    ColonyId, EnergyGrid, FactionId, Owned, Owner, PlanetId, ResourceLedger, Route, SystemId,
""",
        """    ColonyId, EnergyGrid, FactionId, FleetId, Owned, Owner, PlanetId, ResourceLedger, Route,
    SystemId,
""",
    )
    replace_once(
        root,
        path,
        """    DiplomaticRelation, KnowledgeChange, KnowledgeCounts, KnowledgeLevel, KnowledgeTarget,
    PlanetKnowledge, PlanetResourceProfile, ProductionRemainder, ResearchState, SelectionTarget,
    StartingScenario, StartingScenarioError, StrategicClock, SystemKnowledge, UniverseRepository,
""",
        """    DiplomaticRelation, FleetState, KnowledgeChange, KnowledgeCounts, KnowledgeLevel,
    KnowledgeTarget, PlanetKnowledge, PlanetResourceProfile, ProductionRemainder, ResearchState,
    SelectionTarget, StartingScenario, StartingScenarioError, StrategicClock, SystemKnowledge,
    UniverseRepository,
""",
    )
    replace_once(
        root,
        path,
        """/// Version 13 adds deterministic faction relations and generic command metadata.
pub const GAME_STATE_VERSION: u32 = 13;
""",
        """/// Version 14 adds faction-owned fleets and docked ship allocation.
pub const GAME_STATE_VERSION: u32 = 14;
""",
    )
    replace_once(
        root,
        path,
        """    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub research: ResearchState,
""",
        """    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub fleets: Vec<FleetState>,
    pub next_fleet_id: u64,
    pub research: ResearchState,
""",
    )
    replace_once(
        root,
        path,
        """                resource_profile: home.resource_profile,
            }],
            research: ResearchState::from_completed(scenario.initial_technologies.iter().copied()),
""",
        """                resource_profile: home.resource_profile,
            }],
            fleets: Vec::new(),
            next_fleet_id: 0,
            research: ResearchState::from_completed(scenario.initial_technologies.iter().copied()),
""",
    )
    replace_once(
        root,
        path,
        """    pub fn player_home_colony(&self) -> Option<&ColonyState> {
        self.player_colonies().next()
    }

""",
        """    pub fn player_home_colony(&self) -> Option<&ColonyState> {
        self.player_colonies().next()
    }

    pub fn fleet(&self, id: FleetId) -> Option<&FleetState> {
        self.fleets.iter().find(|fleet| fleet.id == id)
    }

    pub fn fleet_mut(&mut self, id: FleetId) -> Option<&mut FleetState> {
        self.fleets.iter_mut().find(|fleet| fleet.id == id)
    }

    pub fn player_fleets(&self) -> impl Iterator<Item = &FleetState> {
        self.fleets
            .iter()
            .filter(|fleet| self.can_manage(self.player_faction, fleet.owner))
    }

""",
    )
    replace_once(
        root,
        "crates/galactic_sim/src/lib.rs",
        "pub mod event;\n",
        "pub mod event;\npub mod fleet;\n",
    )
    replace_once(
        root,
        "crates/galactic_sim/src/lib.rs",
        "pub use event::*;\n",
        "pub use event::*;\npub use fleet::*;\n",
    )


def transform_command_and_events(root: Path) -> None:
    command = "crates/galactic_sim/src/command.rs"
    replace_once(
        root,
        command,
        "use crate::{BuildingKind, CraftableId, StrategicTick, TechnologyId, TimeSpeed};\n",
        """use crate::{
    BuildingKind, CraftableId, FleetComposition, StrategicTick, TechnologyId, TimeSpeed,
};
""",
    )
    replace_once(
        root,
        command,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameAction",
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub enum GameAction",
    )
    replace_once(
        root,
        command,
        """    QueueCraft {
        colony_id: ColonyId,
        craftable: CraftableId,
    },
""",
        """    QueueCraft {
        colony_id: ColonyId,
        craftable: CraftableId,
    },
    FormFleet {
        colony_id: ColonyId,
        composition: FleetComposition,
    },
""",
    )
    replace_once(
        root,
        command,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct GameCommand",
        "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct GameCommand",
    )

    event = "crates/galactic_sim/src/event.rs"
    replace_once(
        root,
        event,
        """    ConstructionRejected, CraftCompleted, CraftQueued, CraftRejected, KnowledgeChange,
    ResearchCompleted, ResearchQueued, ResearchRejected, StrategicDuration, StrategicTick,
    TimeSpeed,
""",
        """    ConstructionRejected, CraftCompleted, CraftQueued, CraftRejected, FleetCreated,
    FleetCreationRejected, KnowledgeChange, ResearchCompleted, ResearchQueued, ResearchRejected,
    StrategicDuration, StrategicTick, TimeSpeed,
""",
    )
    replace_once(
        root,
        event,
        """    CraftCompleted(CraftCompleted),
    CraftRejected(CraftRejected),
}
""",
        """    CraftCompleted(CraftCompleted),
    CraftRejected(CraftRejected),
    FleetCreated(FleetCreated),
    FleetCreationRejected(FleetCreationRejected),
}
""",
    )


def transform_simulation(root: Path) -> None:
    path = "crates/galactic_sim/src/simulation.rs"
    replace_once(
        root,
        path,
        """    ColonyId, FactionId, Owner, PlanetId, ResourceLedgerError, ResourceStock, SystemId,
    UniverseConfig, UniverseDefinition,
""",
        """    ColonyId, FactionId, FleetId, Owner, PlanetId, ResourceLedgerError, ResourceStock, SystemId,
    UniverseConfig, UniverseDefinition,
""",
    )
    replace_once(
        root,
        path,
        """    DiplomacyError, DiplomacyState, FactionKind, GAME_STATE_VERSION, GameAction, GameCommand,
    GameEvent, GameEventKind, GameState, KnowledgeLevel, ResearchStateError, SelectionTarget,
    StartingScenario, StartingScenarioError, StrategicDuration, TimeSpeed, UniverseIndexError,
    UniverseRepository, advance_colony_construction, advance_colony_craft, advance_research,
    default_building_catalog, enqueue_building_upgrade, enqueue_craft, enqueue_research,
    queue_colony_production, storage_capacity, validate_construction_queue, validate_craft_state,
    validate_research_state,
""",
        """    DiplomacyError, DiplomacyState, FactionKind, FleetStateError, GAME_STATE_VERSION, GameAction,
    GameCommand, GameEvent, GameEventKind, GameState, KnowledgeLevel, ResearchStateError,
    SelectionTarget, StartingScenario, StartingScenarioError, StrategicDuration, TimeSpeed,
    UniverseIndexError, UniverseRepository, advance_colony_construction, advance_colony_craft,
    advance_research, default_building_catalog, enqueue_building_upgrade, enqueue_craft,
    enqueue_research, form_fleet, queue_colony_production, storage_capacity,
    validate_construction_queue, validate_craft_state, validate_fleet_state,
    validate_research_state,
""",
    )
    replace_once(
        root,
        path,
        """    UnownedColony(ColonyId),
    DuplicateSystemKnowledge(SystemId),
""",
        """    UnownedColony(ColonyId),
    DuplicateFleet(FleetId),
    InvalidFleetState {
        fleet_id: FleetId,
        error: FleetStateError,
    },
    InvalidNextFleetId {
        next_fleet_id: u64,
        existing_fleet_id: FleetId,
    },
    UnknownFleetFaction {
        fleet_id: FleetId,
        faction_id: FactionId,
    },
    UnownedFleet(FleetId),
    UnknownFleetColony {
        fleet_id: FleetId,
        colony_id: ColonyId,
    },
    UnknownFleetSystem {
        fleet_id: FleetId,
        system_id: SystemId,
    },
    DockedFleetOwnerMismatch {
        fleet_id: FleetId,
        colony_id: ColonyId,
    },
    DuplicateSystemKnowledge(SystemId),
""",
    )
    replace_once(
        root,
        path,
        "        if let Err(error) = self.validate_command(command) {\n",
        "        if let Err(error) = self.validate_command(&command) {\n",
    )
    replace_once(
        root,
        path,
        """            },
            GameAction::DebugAdvanceSelectedKnowledge => self.debug_advance_selected_knowledge(),
""",
        """            },
            GameAction::FormFleet {
                colony_id,
                composition,
            } => match form_fleet(&mut self.state, issuer, colony_id, composition) {
                Ok(created) => vec![GameEventKind::FleetCreated(created)],
                Err(error) => vec![GameEventKind::FleetCreationRejected(
                    crate::FleetCreationRejected { colony_id, error },
                )],
            },
            GameAction::DebugAdvanceSelectedKnowledge => self.debug_advance_selected_knowledge(),
""",
    )
    replace_once(
        root,
        path,
        "    fn validate_command(&self, command: GameCommand) -> Result<(), CommandRejection> {\n",
        "    fn validate_command(&self, command: &GameCommand) -> Result<(), CommandRejection> {\n",
    )
    replace_once(
        root,
        path,
        """    match state.selected {
""",
        """    let mut fleet_ids = HashSet::with_capacity(state.fleets.len());
    for fleet in &state.fleets {
        if !fleet_ids.insert(fleet.id) {
            return Err(SimulationBuildError::DuplicateFleet(fleet.id));
        }
        if fleet.id.raw() >= state.next_fleet_id {
            return Err(SimulationBuildError::InvalidNextFleetId {
                next_fleet_id: state.next_fleet_id,
                existing_fleet_id: fleet.id,
            });
        }
        if let Err(error) = validate_fleet_state(fleet) {
            return Err(SimulationBuildError::InvalidFleetState {
                fleet_id: fleet.id,
                error,
            });
        }
        match fleet.owner {
            Owner::Unowned => {
                return Err(SimulationBuildError::UnownedFleet(fleet.id));
            }
            Owner::Faction(faction_id) if state.faction(faction_id).is_none() => {
                return Err(SimulationBuildError::UnknownFleetFaction {
                    fleet_id: fleet.id,
                    faction_id,
                });
            }
            Owner::Faction(_) => {}
        }
        match fleet.location {
            crate::FleetLocation::Docked(colony_id) => {
                let Some(colony) = state.colony(colony_id) else {
                    return Err(SimulationBuildError::UnknownFleetColony {
                        fleet_id: fleet.id,
                        colony_id,
                    });
                };
                if colony.owner != fleet.owner {
                    return Err(SimulationBuildError::DockedFleetOwnerMismatch {
                        fleet_id: fleet.id,
                        colony_id,
                    });
                }
            }
            crate::FleetLocation::InSystem(system_id) => {
                if universe.system(system_id).is_none() {
                    return Err(SimulationBuildError::UnknownFleetSystem {
                        fleet_id: fleet.id,
                        system_id,
                    });
                }
            }
        }
    }

    match state.selected {
""",
    )


def transform_persistence(root: Path) -> None:
    path = "crates/galactic_persistence/src/lib.rs"
    replace_once(
        root,
        path,
        "// MVP-019: persist faction data, diplomacy, owners and the active ruleset identity.\n",
        "// MVP-020: persist fleet ownership, composition, location, cargo and allocation.\n",
    )
    replace_once(
        root,
        path,
        """    ColonyId, EnergyGrid, FactionId, Owner, PlanetId, ResourceLedger, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId, generate_universe,
""",
        """    ColonyId, EnergyGrid, FactionId, FleetId, Owner, PlanetId, ResourceLedger,
    ResourceLedgerError, ResourceReservation, ResourceStock, SystemId, UniverseConfig, UniverseId,
    generate_universe,
""",
    )
    replace_once(
        root,
        path,
        """    BuildingLevels, ColonyState, ConstructionQueue, CraftInventory, CraftQueue, DiplomacyState,
    FactionData, FactionKind, GameState, PlanetKnowledge, PlanetResourceProfile,
    ProductionRemainder, ProductionRemainderError, ResearchState, SelectionTarget, Simulation,
    SimulationBuildError, StrategicClock, StrategicClockError, StrategicTick, SystemKnowledge,
    TimeSpeed, default_ruleset, production_refresh_ticks,
""",
        """    BuildingLevels, ColonyState, ConstructionQueue, CraftInventory, CraftQueue, DiplomacyState,
    FactionData, FactionKind, FleetAssignment, FleetComposition, FleetLocation, FleetState,
    GameState, PlanetKnowledge, PlanetResourceProfile, ProductionRemainder,
    ProductionRemainderError, ResearchState, SelectionTarget, Simulation, SimulationBuildError,
    StrategicClock, StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed, default_ruleset,
    production_refresh_ticks,
""",
    )
    replace_once(root, path, "pub const SAVE_VERSION: u32 = 14;\n", "pub const SAVE_VERSION: u32 = 15;\n")
    replace_once(
        root,
        path,
        """    pub planet_knowledge: Vec<PlanetKnowledge>,
    pub colonies: Vec<ColonySave>,
    pub research: ResearchState,
""",
        """    pub planet_knowledge: Vec<PlanetKnowledge>,
    pub colonies: Vec<ColonySave>,
    pub fleets: Vec<FleetSave>,
    pub next_fleet_id: u64,
    pub research: ResearchState,
""",
    )
    replace_once(
        root,
        path,
        """    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
""",
        """    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetSave {
    pub id: FleetId,
    pub owner: Owner,
    pub location: FleetLocation,
    pub composition: FleetComposition,
    pub cargo: ResourceStock,
    pub assignment: FleetAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
""",
    )
    replace_once(
        root,
        path,
        """                })
                .collect(),
            research: state.research.clone(),
""",
        """                })
                .collect(),
            fleets: state
                .fleets
                .iter()
                .map(|fleet| FleetSave {
                    id: fleet.id,
                    owner: fleet.owner,
                    location: fleet.location,
                    composition: fleet.composition.clone(),
                    cargo: fleet.cargo,
                    assignment: fleet.assignment,
                })
                .collect(),
            next_fleet_id: state.next_fleet_id,
            research: state.research.clone(),
""",
    )
    replace_once(
        root,
        path,
        """        player_faction: save.state.player_faction,
        colonies,
        research: save.state.research.clone(),
""",
        """        player_faction: save.state.player_faction,
        colonies,
        fleets: save
            .state
            .fleets
            .iter()
            .map(|fleet| FleetState {
                id: fleet.id,
                owner: fleet.owner,
                location: fleet.location,
                composition: fleet.composition.clone(),
                cargo: fleet.cargo,
                assignment: fleet.assignment,
            })
            .collect(),
        next_fleet_id: save.state.next_fleet_id,
        research: save.state.research.clone(),
""",
    )
    replace_once(
        root,
        path,
        """        BuildingKind, CraftableId, GAME_STATE_VERSION, GameAction, TechnologyId,
        default_building_catalog,
""",
        """        BuildingKind, CraftableId, FleetComposition, GAME_STATE_VERSION, GameAction, ShipStack,
        TechnologyId, default_building_catalog,
""",
    )
    replace_once(
        root,
        path,
        """    #[test]
    fn state_and_save_versions_match_mvp_019() {
""",
        """    #[test]
    fn fleets_and_docked_inventory_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let colony = &mut simulation.state_mut().colonies[0];
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
            .expect("test funding fits");
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
        ]);
        for _ in 0..2 {
            let events = simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::LIGHT_CARGO,
            });
            assert!(matches!(
                events.as_slice(),
                [galactic_sim::GameEvent {
                    kind: galactic_sim::GameEventKind::CraftQueued(_),
                    ..
                }]
            ));
        }
        simulation.advance(Duration::from_secs(160));
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("composition is valid");
        simulation.apply_player_action(GameAction::FormFleet {
            colony_id,
            composition,
        });

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("fleet save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().player_fleets().count(), 1);
        assert_eq!(restored.state().fleets[0].owner, Owner::Faction(actor));
        assert_eq!(
            restored.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            1,
        );
    }

    #[test]
    fn state_and_save_versions_match_mvp_020() {
""",
    )


def transform_client(root: Path) -> None:
    replace_once(
        root,
        "crates/galactic_client/src/lib.rs",
        """        GameEventKind::CraftRejected(rejected) => format!(
            "craft {:?} refusé : {:?}",
            rejected.craftable, rejected.error,
        ),
    }
}
""",
        """        GameEventKind::CraftRejected(rejected) => format!(
            "craft {:?} refusé : {:?}",
            rejected.craftable, rejected.error,
        ),
        GameEventKind::FleetCreated(created) => {
            format!("flotte {:?} formée", created.fleet_id)
        }
        GameEventKind::FleetCreationRejected(rejected) => {
            format!("formation de flotte refusée : {:?}", rejected.error)
        }
    }
}
""",
    )


def transform_docs(root: Path) -> None:
    replace_once(
        root,
        "crates/galactic_sim/src/ruleset.rs",
        "pub const RULESET_SCHEMA_VERSION: u32 = 4;\n",
        "pub const RULESET_SCHEMA_VERSION: u32 = 5;\n",
    )
    ruleset = "docs/ruleset.md"
    replace_once(
        root,
        ruleset,
        "- `craftables.ron` : objets fabricables, coûts, durées, prérequis et capacités ;\n",
        "- `craftables.ron` : objets fabricables, vaisseaux, coûts, prérequis et capacités ;\n",
    )
    replace_once(
        root,
        ruleset,
        """Les fabrications possèdent un `CraftableId` textuel, une catégorie, un coût,
une durée de base, une quantité produite, des prérequis de bâtiments et de
technologies ainsi qu'une liste de capacités numériques. Le catalogue par
défaut contient :

- `light_probe` avec `probe_strength` ;
- `light_cargo` avec `cargo_capacity` ;
- `colony_ship` avec `colonization_capacity`.

Les catégories `Defense`, `Military` et `Support` sont déjà reconnues, sans
imposer de contenu actif. Une fabrication doit dépendre d'au moins un bâtiment
ayant l'effet `ShipyardPoints`. Sa durée de base correspond à la cadence du
niveau minimal requis ; améliorer le chantier accélère ensuite la file.
""",
        """Les fabrications possèdent un `CraftableId` textuel, une catégorie, un coût,
une durée de base, une quantité produite, des prérequis de bâtiments et de
technologies ainsi qu'une liste de capacités numériques. Une fabrication qui
représente un vaisseau ajoute une classe, une vitesse de croisière, une portée
en sauts, une capacité cargo et une consommation de carburant par saut. Le
catalogue par défaut contient :

- `light_probe` avec `probe_strength` ;
- `light_cargo` avec 800 unités de capacité cargo ;
- `colony_ship` avec `colonization_capacity`.

Les classes de vaisseau reconnues sont `Probe`, `Cargo`, `Colony`, `Military`
et `Support`. Les catégories `Defense`, `Military` et `Support` sont déjà
reconnues, sans imposer de contenu militaire actif. Une défense n'est pas un
vaisseau. Une fabrication doit dépendre d'au moins un bâtiment ayant l'effet
`ShipyardPoints`. Sa durée de base correspond à la cadence du niveau minimal
requis ; améliorer le chantier accélère ensuite la file.
""",
    )
    append_once(
        root,
        "docs/mvp_architecture.md",
        "## MVP-020 — Flottes, vaisseaux et capacités",
        dedent(
            """

            ## MVP-020 — Flottes, vaisseaux et capacités

            Les fabrications de vaisseaux décrivent maintenant dans `craftables.ron` leur
            classe, leur vitesse de croisière, leur portée en sauts, leur capacité cargo et
            leur consommation de carburant par saut. Les valeurs restent configurables,
            tandis que les règles d'agrégation appartiennent à la simulation.

            Une flotte possède un `FleetId` stable, un `Owner`, une localisation, une
            composition générique, une cargaison et une affectation éventuelle à une
            mission. Une flotte mixte utilise la vitesse et la portée de son vaisseau le
            plus contraignant ; les capacités cargo et consommations sont additionnées.

            Former une flotte est une commande atomique :

            1. valider la faction et la colonie ;
            2. valider chaque classe et quantité demandée ;
            3. vérifier l'inventaire disponible au sol ;
            4. retirer les vaisseaux de cet inventaire ;
            5. créer une seule flotte possédée par la faction.

            Une seconde formation ne peut donc pas réutiliser les mêmes unités. Les flottes,
            leur composition, leur localisation, leur cargaison, leur affectation et le
            prochain identifiant sont sauvegardés. Le moteur de trajet et les missions
            restent hors périmètre jusqu'à MVP-021.

            Versions après migration :

            - `GAME_STATE_VERSION = 14` ;
            - `SAVE_VERSION = 15` ;
            - `RULESET_SCHEMA_VERSION = 5` ;
            - `CRAFTABLE_CATALOG_VERSION = 2`.
            """
        ),
    )


def configure_shared_guards() -> None:
    base.BASELINE_SHA = BASELINE_SHA
    base.BASELINE_BLOBS = {**MODIFIED_BLOBS, **DEPENDENCY_BLOBS}
    base.CREATED_PATHS = CREATED_PATHS
    base.DELETED_PATHS = ()
    base.EXPECTED_PATHS = EXPECTED_PATHS


def selected_checks(*, full_checks: bool):
    return FULL_CHECK_COMMANDS if full_checks else TARGETED_CHECK_COMMANDS


def validate_expected_diff(worktree: Path) -> None:
    result = base.run(
        ("git", "diff", "--name-only", "HEAD", "--"),
        cwd=worktree,
        capture=True,
    )
    found = frozenset(
        line.decode("utf-8") for line in result.stdout.splitlines() if line
    )
    if found != EXPECTED_PATHS:
        missing = sorted(EXPECTED_PATHS - found)
        unexpected = sorted(found - EXPECTED_PATHS)
        raise base.MigrationError(
            f"périmètre inattendu ; manquants={missing}, inattendus={unexpected}"
        )


def validated_patch(
    root: Path,
    *,
    run_checks: bool,
    full_checks: bool,
) -> bytes:
    with tempfile.TemporaryDirectory(
        prefix="galactic-mvp020-", dir=root.parent
    ) as temporary:
        worktree = Path(temporary) / "worktree"
        added = False
        try:
            base.run(
                ("git", "worktree", "add", "--detach", str(worktree), base.head_sha(root)),
                cwd=root,
            )
            added = True
            transform_tree(worktree)
            base.run(("git", "add", "-N", "--", *CREATED_PATHS), cwd=worktree)
            base.run(("git", "diff", "--check"), cwd=worktree)
            validate_expected_diff(worktree)

            if run_checks:
                validation_env = os.environ.copy()
                validation_env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
                mode = "complets" if full_checks else "ciblés"
                print(f"Contrôles Cargo {mode}, avec réutilisation du cache :")
                for command in selected_checks(full_checks=full_checks):
                    base.run(command, cwd=worktree, env=validation_env)
            else:
                print("Contrôles Cargo non demandés pour cette validation.")

            base.run(("git", "diff", "--check"), cwd=worktree)
            validate_expected_diff(worktree)
            candidate = base.run(
                ("git", "diff", "--binary", "HEAD", "--"),
                cwd=worktree,
                capture=True,
            ).stdout
            if not candidate:
                raise base.MigrationError("Le patch validé est vide.")
            return candidate
        finally:
            if added:
                base.run(
                    ("git", "worktree", "remove", "--force", str(worktree)),
                    cwd=root,
                    check=False,
                )


def make_backup(root: Path, patch: bytes) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    parent = root / "backups" / ".mvp020-backup"
    destination = parent / stamp
    counter = 1
    while destination.exists():
        destination = parent / f"{stamp}-{counter}"
        counter += 1
    destination.mkdir(parents=True)

    backed_up: list[str] = []
    for relative in sorted(MODIFIED_BLOBS):
        source = root / relative
        if not source.is_file():
            continue
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        backed_up.append(relative)

    manifest = {
        "migration": MIGRATION,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": BASELINE_SHA,
        "actual_head_sha": base.head_sha(root),
        "validated_patch_sha256": hashlib.sha256(patch).hexdigest(),
        "backed_up_paths": backed_up,
        "created_paths": list(CREATED_PATHS),
        "deleted_paths": [],
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return destination


def apply_to_main(root: Path, patch: bytes, *, force: bool) -> Path:
    base.verify_baseline(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le patch validé ne s'applique plus au dépôt principal. "
            "Aucun fichier source n'a été modifié."
        )
    backup = make_backup(root, patch)
    base.verify_baseline(root, force=force)
    if not base.patch_check(root, patch):
        raise base.MigrationError(
            "Le dépôt a changé pendant la sauvegarde. "
            "Aucun fichier source n'a été modifié."
        )
    base.run(("git", "apply", "--binary", "-"), cwd=root, input_bytes=patch)
    return backup


def already_applied(root: Path) -> bool:
    fleet = root / CREATED_PATHS[0]
    if not fleet.is_file():
        return False
    state = (root / "crates/galactic_sim/src/state.rs").read_text(encoding="utf-8")
    persistence = (root / "crates/galactic_persistence/src/lib.rs").read_text(
        encoding="utf-8"
    )
    ruleset = (root / "crates/galactic_sim/src/ruleset.rs").read_text(
        encoding="utf-8"
    )
    return (
        "pub const GAME_STATE_VERSION: u32 = 14;" in state
        and "pub const SAVE_VERSION: u32 = 15;" in persistence
        and "pub const RULESET_SCHEMA_VERSION: u32 = 5;" in ruleset
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prépare MVP-020 : vaisseaux configurables, flottes possédées, "
            "allocation atomique et persistance."
        )
    )
    parser.add_argument(
        "--root",
        default=".",
        help="racine du dépôt Galactic (défaut : répertoire courant)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="valide baseline, transformations et périmètre sans compiler ni modifier",
    )
    parser.add_argument(
        "--checks",
        action="store_true",
        help="lance aussi les contrôles Cargo ciblés pendant un dry-run",
    )
    parser.add_argument(
        "--full-checks",
        action="store_true",
        help="remplace les contrôles ciblés par ceux de tout le workspace",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
        help="ignore les contrôles Cargo pendant l'application (déconseillé)",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="ignore les gardes SHA/blobs (dangereux ; le patch doit s'appliquer)",
    )
    args = parser.parse_args()
    if args.skip_checks and (args.checks or args.full_checks):
        parser.error("--skip-checks est incompatible avec --checks/--full-checks")
    return args


def main() -> int:
    args = parse_args()
    try:
        configure_shared_guards()
        base.ensure_command("git")
        run_checks = (
            args.checks
            or args.full_checks
            or (not args.dry_run and not args.skip_checks)
        )
        if run_checks:
            base.ensure_command("cargo")

        root = base.resolve_root(args.root)
        if already_applied(root):
            print("MVP-020 est déjà appliqué ; aucune modification nécessaire.")
            return 0

        base.verify_baseline(root, force=args.force)
        if args.skip_checks and not args.dry_run:
            print(
                "AVERTISSEMENT : contrôles Cargo ignorés pendant l'application.",
                file=sys.stderr,
            )
        candidate = validated_patch(
            root,
            run_checks=run_checks,
            full_checks=args.full_checks,
        )

        if args.dry_run:
            checks_label = " avec contrôles Cargo" if run_checks else ""
            print(
                f"Dry-run réussi{checks_label} : baseline, transformations et "
                "périmètre valides. Le dépôt principal n'a pas été modifié."
            )
            return 0

        with tempfile.TemporaryDirectory(
            prefix="galactic-mvp020-verify-", dir=root.parent
        ) as temporary:
            reference = Path(temporary) / "reference"
            added = False
            try:
                base.run(
                    (
                        "git",
                        "worktree",
                        "add",
                        "--detach",
                        str(reference),
                        base.head_sha(root),
                    ),
                    cwd=root,
                )
                added = True
                base.run(
                    ("git", "apply", "--binary", "-"),
                    cwd=reference,
                    input_bytes=candidate,
                )
                backup = apply_to_main(root, candidate, force=args.force)
                base.verify_applied_files(root, reference)
            finally:
                if added:
                    base.run(
                        ("git", "worktree", "remove", "--force", str(reference)),
                        cwd=root,
                        check=False,
                    )

        print("MVP-020 appliqué avec succès.")
        print(f"Sauvegarde : {backup}")
        print(
            "Versions cibles : GAME_STATE_VERSION=14, SAVE_VERSION=15, "
            "RULESET_SCHEMA_VERSION=5"
        )
        return 0
    except (base.MigrationError, OSError) as exc:
        print(f"ERREUR : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

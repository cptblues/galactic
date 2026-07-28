// MVP-016-B: configurable building identifiers and validated catalog data.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use galactic_domain::{EnergyGrid, ResourceCost, ResourceStock};
use serde::Deserialize;

use crate::{STRATEGIC_TICKS_PER_SECOND, default_ruleset};

pub const MAX_RULESET_BUILDINGS: usize = 64;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildingKind(&'static str);

impl BuildingKind {
    pub const METAL_MINE: Self = Self("metal_mine");
    pub const CRYSTAL_EXTRACTOR: Self = Self("crystal_extractor");
    pub const FUEL_REFINERY: Self = Self("fuel_refinery");
    pub const POWER_PLANT: Self = Self("power_plant");
    pub const WAREHOUSE: Self = Self("warehouse");
    pub const CONSTRUCTION_CENTER: Self = Self("construction_center");
    pub const RESEARCH_LAB: Self = Self("research_lab");
    pub const SHIPYARD: Self = Self("shipyard");

    pub const fn from_static(key: &'static str) -> Self {
        Self(key)
    }

    pub const fn key(self) -> &'static str {
        self.0
    }

    fn from_config(key: String) -> Result<Self, BuildingCatalogError> {
        validate_identifier(&key).map_err(|()| BuildingCatalogError::InvalidIdentifier)?;
        Ok(Self(Box::leak(key.into_boxed_str())))
    }
}

impl fmt::Debug for BuildingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BuildingKind")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for BuildingKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BuildingLevel {
    kind: BuildingKind,
    level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingLevels {
    entries: [BuildingLevel; MAX_RULESET_BUILDINGS],
    len: u8,
}

impl BuildingLevels {
    pub const EMPTY: Self = Self {
        entries: [BuildingLevel {
            kind: BuildingKind::from_static(""),
            level: 0,
        }; MAX_RULESET_BUILDINGS],
        len: 0,
    };

    pub fn level(self, kind: BuildingKind) -> u8 {
        self.entries[..usize::from(self.len)]
            .iter()
            .find(|entry| entry.kind == kind)
            .map_or(0, |entry| entry.level)
    }

    pub fn set_level(&mut self, kind: BuildingKind, level: u8) {
        if let Some(entry) = self.entries[..usize::from(self.len)]
            .iter_mut()
            .find(|entry| entry.kind == kind)
        {
            entry.level = level;
            return;
        }
        if level == 0 {
            return;
        }

        let index = usize::from(self.len);
        assert!(
            index < MAX_RULESET_BUILDINGS,
            "building level capacity must match the validated ruleset",
        );
        self.entries[index] = BuildingLevel { kind, level };
        self.len += 1;
    }

    pub fn iter(self) -> impl Iterator<Item = (BuildingKind, u8)> {
        self.entries
            .into_iter()
            .take(usize::from(self.len))
            .map(|entry| (entry.kind, entry.level))
    }

    pub fn total_levels(self) -> u32 {
        self.iter().map(|(_, level)| u32::from(level)).sum()
    }
}

impl Default for BuildingLevels {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum BuildingEffect {
    MetalProduction {
        milli_per_tick_per_level: u64,
    },
    CrystalProduction {
        milli_per_tick_per_level: u64,
    },
    FuelProduction {
        milli_per_tick_per_level: u64,
    },
    EnergyProduction {
        capacity_per_level: u64,
    },
    Storage {
        metal_per_level: u64,
        crystal_per_level: u64,
        fuel_per_level: u64,
    },
    ConstructionSpeed {
        permille_per_level: u64,
    },
    ResearchPoints {
        milli_per_tick_per_level: u64,
    },
    ShipyardPoints {
        milli_per_tick_per_level: u64,
    },
}

impl BuildingEffect {
    pub const fn storage_per_level(self) -> Option<ResourceStock> {
        match self {
            Self::Storage {
                metal_per_level,
                crystal_per_level,
                fuel_per_level,
            } => Some(ResourceStock::new(
                metal_per_level,
                crystal_per_level,
                fuel_per_level,
            )),
            _ => None,
        }
    }

    pub const fn structural_key(self) -> &'static str {
        match self {
            Self::MetalProduction { .. } => "metal_production",
            Self::CrystalProduction { .. } => "crystal_production",
            Self::FuelProduction { .. } => "fuel_production",
            Self::EnergyProduction { .. } => "energy_production",
            Self::Storage { .. } => "storage",
            Self::ConstructionSpeed { .. } => "construction_speed",
            Self::ResearchPoints { .. } => "research_points",
            Self::ShipyardPoints { .. } => "shipyard_points",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingPrerequisite {
    pub kind: BuildingKind,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDefinition {
    pub kind: BuildingKind,
    pub name: &'static str,
    pub description: &'static str,
    pub max_level: u8,
    pub base_cost: ResourceCost,
    pub cost_growth_per_mille: u16,
    pub base_duration_ticks: u64,
    pub duration_growth_per_mille: u16,
    pub energy_consumption_per_level: u64,
    pub effect: BuildingEffect,
    pub prerequisites: Vec<BuildingPrerequisite>,
}

impl BuildingDefinition {
    pub fn cost_for_level(&self, target_level: u8) -> Result<ResourceCost, BuildingCatalogError> {
        self.validate_target_level(target_level)?;
        let steps = target_level.saturating_sub(1);
        Ok(ResourceCost::new(
            scale_progression(self.base_cost.metal, self.cost_growth_per_mille, steps),
            scale_progression(self.base_cost.crystal, self.cost_growth_per_mille, steps),
            scale_progression(self.base_cost.fuel, self.cost_growth_per_mille, steps),
        ))
    }

    pub fn duration_for_level(&self, target_level: u8) -> Result<u64, BuildingCatalogError> {
        self.validate_target_level(target_level)?;
        Ok(scale_progression(
            self.base_duration_ticks,
            self.duration_growth_per_mille,
            target_level.saturating_sub(1),
        ))
    }

    fn validate_target_level(&self, target_level: u8) -> Result<(), BuildingCatalogError> {
        if target_level == 0 || target_level > self.max_level {
            return Err(BuildingCatalogError::InvalidTargetLevel {
                kind: self.kind,
                target_level,
                max_level: self.max_level,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingCatalog {
    version: u32,
    base_storage: ResourceStock,
    definitions: BTreeMap<BuildingKind, BuildingDefinition>,
}

impl BuildingCatalog {
    pub(crate) fn from_config(
        config: BuildingCatalogConfig,
        base_storage: ResourceStock,
    ) -> Result<Self, BuildingCatalogError> {
        if config.version != 1 {
            return Err(BuildingCatalogError::UnsupportedVersion(config.version));
        }
        if config.buildings.is_empty() || config.buildings.len() > MAX_RULESET_BUILDINGS {
            return Err(BuildingCatalogError::InvalidBuildingCount {
                found: config.buildings.len(),
                maximum: MAX_RULESET_BUILDINGS,
            });
        }

        let mut ids = BTreeMap::new();
        for building in &config.buildings {
            let kind = BuildingKind::from_config(building.id.clone())?;
            if ids.insert(building.id.clone(), kind).is_some() {
                return Err(BuildingCatalogError::DuplicateBuilding(kind));
            }
        }

        let mut definitions = BTreeMap::new();
        for building in config.buildings {
            let kind = ids[&building.id];
            let prerequisites = building
                .prerequisites
                .into_iter()
                .map(|prerequisite| {
                    let Some(&prerequisite_kind) = ids.get(&prerequisite.id) else {
                        return Err(BuildingCatalogError::MissingPrerequisite {
                            kind,
                            prerequisite: leak_identifier(prerequisite.id)?,
                        });
                    };
                    Ok(BuildingPrerequisite {
                        kind: prerequisite_kind,
                        level: prerequisite.level,
                    })
                })
                .collect::<Result<Vec<_>, BuildingCatalogError>>()?;
            let definition = BuildingDefinition {
                kind,
                name: leak_non_empty(building.name, BuildingCatalogError::EmptyName(kind))?,
                description: leak_non_empty(
                    building.description,
                    BuildingCatalogError::EmptyDescription(kind),
                )?,
                max_level: building.max_level,
                base_cost: building.base_cost.into_cost(),
                cost_growth_per_mille: building.cost_growth_per_mille,
                base_duration_ticks: building
                    .base_duration_seconds
                    .checked_mul(u64::from(STRATEGIC_TICKS_PER_SECOND))
                    .ok_or(BuildingCatalogError::InvalidDefinition(kind))?,
                duration_growth_per_mille: building.duration_growth_per_mille,
                energy_consumption_per_level: building.energy_consumption_per_level,
                effect: building.effect,
                prerequisites,
            };
            definitions.insert(kind, definition);
        }

        let catalog = Self {
            version: config.version,
            base_storage,
            definitions,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn base_storage(&self) -> ResourceStock {
        self.base_storage
    }

    pub fn definitions(&self) -> impl Iterator<Item = &BuildingDefinition> {
        self.definitions.values()
    }

    pub fn definition(&self, kind: BuildingKind) -> &BuildingDefinition {
        self.definitions
            .get(&kind)
            .expect("validated building identifier must exist in the active ruleset")
    }

    pub fn kind_by_key(&self, key: &str) -> Option<BuildingKind> {
        self.definitions
            .keys()
            .copied()
            .find(|kind| kind.key() == key)
    }

    pub fn validate_levels(&self, levels: BuildingLevels) -> Result<(), BuildingCatalogError> {
        for (kind, current) in levels.iter() {
            let Some(definition) = self.definitions.get(&kind) else {
                return Err(BuildingCatalogError::UnknownBuilding(kind));
            };
            if current > definition.max_level {
                return Err(BuildingCatalogError::LevelExceedsMaximum {
                    kind,
                    level: current,
                    max_level: definition.max_level,
                });
            }
            if current == 0 {
                continue;
            }
            for prerequisite in &definition.prerequisites {
                let found = levels.level(prerequisite.kind);
                if found < prerequisite.level {
                    return Err(BuildingCatalogError::UnsatisfiedPrerequisite {
                        kind,
                        prerequisite: prerequisite.kind,
                        required: prerequisite.level,
                        found,
                    });
                }
            }
        }
        Ok(())
    }

    pub fn energy_grid_for_levels(&self, levels: BuildingLevels) -> EnergyGrid {
        let mut production = 0_u64;
        let mut consumption = 0_u64;

        for definition in self.definitions.values() {
            let level = u64::from(levels.level(definition.kind));
            if level == 0 {
                continue;
            }
            consumption = consumption.saturating_add(
                definition
                    .energy_consumption_per_level
                    .saturating_mul(level),
            );
            if let BuildingEffect::EnergyProduction { capacity_per_level } = definition.effect {
                production = production.saturating_add(capacity_per_level.saturating_mul(level));
            }
        }

        EnergyGrid::new(production, consumption)
    }

    pub fn storage_capacity_for_levels(&self, levels: BuildingLevels) -> ResourceStock {
        let mut capacity = self.base_storage;
        for definition in self.definitions.values() {
            let level = u64::from(levels.level(definition.kind));
            if level == 0 {
                continue;
            }
            if let Some(per_level) = definition.effect.storage_per_level() {
                capacity = ResourceStock::new(
                    capacity
                        .metal
                        .saturating_add(per_level.metal.saturating_mul(level)),
                    capacity
                        .crystal
                        .saturating_add(per_level.crystal.saturating_mul(level)),
                    capacity
                        .fuel
                        .saturating_add(per_level.fuel.saturating_mul(level)),
                );
            }
        }
        capacity
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        for definition in self.definitions.values() {
            output.push_str(definition.kind.key());
            output.push(':');
            output.push_str(definition.effect.structural_key());
            output.push('[');
            for prerequisite in &definition.prerequisites {
                output.push_str(prerequisite.kind.key());
                output.push(',');
            }
            output.push_str("];");
        }
    }

    fn validate(&self) -> Result<(), BuildingCatalogError> {
        for definition in self.definitions.values() {
            if definition.max_level == 0
                || definition.base_duration_ticks == 0
                || definition.cost_growth_per_mille < 1_000
                || definition.duration_growth_per_mille < 1_000
            {
                return Err(BuildingCatalogError::InvalidDefinition(definition.kind));
            }

            let mut prerequisites = BTreeSet::new();
            for prerequisite in &definition.prerequisites {
                if prerequisite.kind == definition.kind {
                    return Err(BuildingCatalogError::SelfPrerequisite(definition.kind));
                }
                if !prerequisites.insert(prerequisite.kind) {
                    return Err(BuildingCatalogError::DuplicatePrerequisite {
                        kind: definition.kind,
                        prerequisite: prerequisite.kind,
                    });
                }
                let required = self.definition(prerequisite.kind);
                if prerequisite.level == 0 || prerequisite.level > required.max_level {
                    return Err(BuildingCatalogError::InvalidPrerequisiteLevel {
                        kind: definition.kind,
                        prerequisite: prerequisite.kind,
                        level: prerequisite.level,
                    });
                }
            }
        }

        for kind in self.definitions.keys().copied() {
            self.visit(kind, &mut BTreeSet::new(), &mut BTreeSet::new())?;
        }
        Ok(())
    }

    fn visit(
        &self,
        kind: BuildingKind,
        visiting: &mut BTreeSet<BuildingKind>,
        visited: &mut BTreeSet<BuildingKind>,
    ) -> Result<(), BuildingCatalogError> {
        if visited.contains(&kind) {
            return Ok(());
        }
        if !visiting.insert(kind) {
            return Err(BuildingCatalogError::PrerequisiteCycle(kind));
        }
        for prerequisite in &self.definition(kind).prerequisites {
            self.visit(prerequisite.kind, visiting, visited)?;
        }
        visiting.remove(&kind);
        visited.insert(kind);
        Ok(())
    }
}

pub fn default_building_catalog() -> &'static BuildingCatalog {
    default_ruleset().buildings()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCatalogError {
    InvalidIdentifier,
    UnsupportedVersion(u32),
    InvalidBuildingCount {
        found: usize,
        maximum: usize,
    },
    DuplicateBuilding(BuildingKind),
    UnknownBuilding(BuildingKind),
    EmptyName(BuildingKind),
    EmptyDescription(BuildingKind),
    InvalidDefinition(BuildingKind),
    SelfPrerequisite(BuildingKind),
    DuplicatePrerequisite {
        kind: BuildingKind,
        prerequisite: BuildingKind,
    },
    MissingPrerequisite {
        kind: BuildingKind,
        prerequisite: BuildingKind,
    },
    InvalidPrerequisiteLevel {
        kind: BuildingKind,
        prerequisite: BuildingKind,
        level: u8,
    },
    PrerequisiteCycle(BuildingKind),
    InvalidTargetLevel {
        kind: BuildingKind,
        target_level: u8,
        max_level: u8,
    },
    LevelExceedsMaximum {
        kind: BuildingKind,
        level: u8,
        max_level: u8,
    },
    UnsatisfiedPrerequisite {
        kind: BuildingKind,
        prerequisite: BuildingKind,
        required: u8,
        found: u8,
    },
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuildingCatalogConfig {
    version: u32,
    buildings: Vec<BuildingDefinitionConfig>,
}

#[derive(Debug, Deserialize)]
struct BuildingDefinitionConfig {
    id: String,
    name: String,
    description: String,
    max_level: u8,
    base_cost: ResourceValuesConfig,
    cost_growth_per_mille: u16,
    base_duration_seconds: u64,
    duration_growth_per_mille: u16,
    energy_consumption_per_level: u64,
    effect: BuildingEffect,
    prerequisites: Vec<BuildingPrerequisiteConfig>,
}

#[derive(Debug, Deserialize)]
struct BuildingPrerequisiteConfig {
    id: String,
    level: u8,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) struct ResourceValuesConfig {
    pub(crate) metal: u64,
    pub(crate) crystal: u64,
    pub(crate) fuel: u64,
}

impl ResourceValuesConfig {
    pub(crate) const fn into_stock(self) -> ResourceStock {
        ResourceStock::new(self.metal, self.crystal, self.fuel)
    }

    const fn into_cost(self) -> ResourceCost {
        ResourceCost::new(self.metal, self.crystal, self.fuel)
    }
}

fn scale_progression(base: u64, growth_per_mille: u16, steps: u8) -> u64 {
    let mut value = u128::from(base);
    for _ in 0..steps {
        value = value
            .saturating_mul(u128::from(growth_per_mille))
            .div_ceil(1_000);
    }
    value.min(u128::from(u64::MAX)) as u64
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

fn leak_identifier(value: String) -> Result<BuildingKind, BuildingCatalogError> {
    BuildingKind::from_config(value)
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
    use super::*;

    #[test]
    fn default_catalog_is_loaded_from_the_ruleset() {
        let catalog = default_building_catalog();
        assert_eq!(catalog.version(), 1);
        assert_eq!(catalog.definitions().count(), 8);
        assert_eq!(
            catalog.definition(BuildingKind::METAL_MINE).name,
            "Fosse sidérurgique",
        );
    }

    #[test]
    fn default_starting_levels_are_valid() {
        let levels = crate::StartingScenario::mvp().home_colony.buildings;
        assert_eq!(catalog_result(levels), Ok(()));
    }

    #[test]
    fn a_new_building_with_a_known_effect_is_data_only() {
        let source = r#"(
            version: 1,
            buildings: [(
                id: "propaganda_office",
                name: "Bureau de propagande",
                description: "Produit une science parfaitement objective.",
                max_level: 3,
                base_cost: (metal: 10, crystal: 20, fuel: 0),
                cost_growth_per_mille: 1200,
                base_duration_seconds: 5,
                duration_growth_per_mille: 1100,
                energy_consumption_per_level: 2,
                effect: ResearchPoints(milli_per_tick_per_level: 10),
                prerequisites: [],
            )],
        )"#;
        let config = ron::de::from_str(source).expect("test building RON is valid");
        let catalog = BuildingCatalog::from_config(config, ResourceStock::new(10, 10, 10))
            .expect("known effects accept new identifiers");

        let kind = catalog
            .kind_by_key("propaganda_office")
            .expect("configured building exists");
        assert_eq!(catalog.definition(kind).max_level, 3);
    }

    fn catalog_result(levels: BuildingLevels) -> Result<(), BuildingCatalogError> {
        default_building_catalog().validate_levels(levels)
    }
}

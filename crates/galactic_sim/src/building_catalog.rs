// MVP-013: validated building definitions loaded from a simple data asset.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use galactic_domain::{EnergyGrid, ResourceCost, ResourceStock};

use crate::{BuildingKind, BuildingLevels};

const EMBEDDED_CATALOG: &str = include_str!("../../../assets/data/buildings.catalog");

static DEFAULT_CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();

pub fn default_building_catalog() -> &'static BuildingCatalog {
    DEFAULT_CATALOG.get_or_init(|| {
        BuildingCatalog::parse(EMBEDDED_CATALOG)
            .expect("the embedded MVP building catalog must be valid")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingEffect {
    MetalProduction { milli_per_tick_per_level: u64 },
    CrystalProduction { milli_per_tick_per_level: u64 },
    FuelProduction { milli_per_tick_per_level: u64 },
    EnergyProduction { capacity_per_level: u64 },
    Storage { per_level: ResourceStock },
    ConstructionSpeed { permille_per_level: u64 },
    ResearchPoints { milli_per_tick_per_level: u64 },
    ShipyardPoints { milli_per_tick_per_level: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildingPrerequisite {
    pub kind: BuildingKind,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDefinition {
    pub kind: BuildingKind,
    pub name: String,
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
    fingerprint: u64,
    base_storage: ResourceStock,
    definitions: BTreeMap<BuildingKind, BuildingDefinition>,
}

impl BuildingCatalog {
    pub fn parse(source: &str) -> Result<Self, BuildingCatalogError> {
        let mut version = None;
        let mut base_storage = None;
        let mut definitions = BTreeMap::new();

        for (index, raw_line) in source.lines().enumerate() {
            let line_number = index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(value) = line.strip_prefix("version=") {
                if version.is_some() {
                    return Err(BuildingCatalogError::DuplicateHeader { line: line_number });
                }
                version = Some(
                    value
                        .parse()
                        .map_err(|_| BuildingCatalogError::InvalidNumber { line: line_number })?,
                );
                continue;
            }

            if let Some(value) = line.strip_prefix("base_storage=") {
                if base_storage.is_some() {
                    return Err(BuildingCatalogError::DuplicateHeader { line: line_number });
                }
                base_storage = Some(parse_stock(value, line_number)?);
                continue;
            }

            let Some(value) = line.strip_prefix("building=") else {
                return Err(BuildingCatalogError::InvalidLine { line: line_number });
            };
            let definition = parse_definition(value, line_number)?;
            if definitions.insert(definition.kind, definition).is_some() {
                return Err(BuildingCatalogError::DuplicateBuilding { line: line_number });
            }
        }

        let version = version.ok_or(BuildingCatalogError::MissingVersion)?;
        if version != 1 {
            return Err(BuildingCatalogError::UnsupportedVersion(version));
        }

        let catalog = Self {
            version,
            fingerprint: fnv1a64(source.as_bytes()),
            base_storage: base_storage.ok_or(BuildingCatalogError::MissingBaseStorage)?,
            definitions,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
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
            .expect("validated catalog contains every building")
    }

    pub fn validate_levels(&self, levels: BuildingLevels) -> Result<(), BuildingCatalogError> {
        for kind in BuildingKind::ALL {
            let current = levels.level(kind);
            let definition = self.definition(kind);
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

        for kind in BuildingKind::ALL {
            let level = u64::from(levels.level(kind));
            if level == 0 {
                continue;
            }
            let definition = self.definition(kind);
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

    fn validate(&self) -> Result<(), BuildingCatalogError> {
        for kind in BuildingKind::ALL {
            if !self.definitions.contains_key(&kind) {
                return Err(BuildingCatalogError::MissingBuilding(kind));
            }
        }
        if self.definitions.len() != BuildingKind::ALL.len() {
            return Err(BuildingCatalogError::UnexpectedBuildingCount {
                found: self.definitions.len(),
            });
        }

        for definition in self.definitions.values() {
            if definition.name.trim().is_empty() {
                return Err(BuildingCatalogError::EmptyName(definition.kind));
            }
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
                let Some(required) = self.definitions.get(&prerequisite.kind) else {
                    return Err(BuildingCatalogError::MissingPrerequisite {
                        kind: definition.kind,
                        prerequisite: prerequisite.kind,
                    });
                };
                if prerequisite.level == 0 || prerequisite.level > required.max_level {
                    return Err(BuildingCatalogError::InvalidPrerequisiteLevel {
                        kind: definition.kind,
                        prerequisite: prerequisite.kind,
                        level: prerequisite.level,
                    });
                }
            }
        }

        for kind in BuildingKind::ALL {
            let mut visiting = BTreeSet::new();
            let mut visited = BTreeSet::new();
            self.visit(kind, &mut visiting, &mut visited)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingCatalogError {
    InvalidLine {
        line: usize,
    },
    InvalidNumber {
        line: usize,
    },
    UnknownBuilding {
        line: usize,
    },
    UnknownEffect {
        line: usize,
    },
    InvalidEffect {
        line: usize,
    },
    InvalidPrerequisite {
        line: usize,
    },
    DuplicateHeader {
        line: usize,
    },
    DuplicateBuilding {
        line: usize,
    },
    MissingVersion,
    UnsupportedVersion(u32),
    MissingBaseStorage,
    MissingBuilding(BuildingKind),
    UnexpectedBuildingCount {
        found: usize,
    },
    EmptyName(BuildingKind),
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

impl BuildingKind {
    pub const fn key(self) -> &'static str {
        match self {
            Self::MetalMine => "metal_mine",
            Self::CrystalExtractor => "crystal_extractor",
            Self::FuelRefinery => "fuel_refinery",
            Self::PowerPlant => "power_plant",
            Self::Warehouse => "warehouse",
            Self::ConstructionCenter => "construction_center",
            Self::ResearchLab => "research_lab",
            Self::Shipyard => "shipyard",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "metal_mine" => Some(Self::MetalMine),
            "crystal_extractor" => Some(Self::CrystalExtractor),
            "fuel_refinery" => Some(Self::FuelRefinery),
            "power_plant" => Some(Self::PowerPlant),
            "warehouse" => Some(Self::Warehouse),
            "construction_center" => Some(Self::ConstructionCenter),
            "research_lab" => Some(Self::ResearchLab),
            "shipyard" => Some(Self::Shipyard),
            _ => None,
        }
    }
}

fn parse_definition(value: &str, line: usize) -> Result<BuildingDefinition, BuildingCatalogError> {
    let fields = value.split('|').collect::<Vec<_>>();
    if fields.len() != 10 {
        return Err(BuildingCatalogError::InvalidLine { line });
    }

    let kind =
        BuildingKind::parse(fields[0]).ok_or(BuildingCatalogError::UnknownBuilding { line })?;
    let max_level = parse_number(fields[2], line)?;
    let base_cost = parse_cost(fields[3], line)?;
    let cost_growth_per_mille = parse_number(fields[4], line)?;
    let base_duration_ticks = parse_number(fields[5], line)?;
    let duration_growth_per_mille = parse_number(fields[6], line)?;
    let energy_consumption_per_level = parse_number(fields[7], line)?;
    let effect = parse_effect(fields[8], line)?;
    let prerequisites = parse_prerequisites(fields[9], line)?;

    Ok(BuildingDefinition {
        kind,
        name: fields[1].trim().to_string(),
        max_level,
        base_cost,
        cost_growth_per_mille,
        base_duration_ticks,
        duration_growth_per_mille,
        energy_consumption_per_level,
        effect,
        prerequisites,
    })
}

fn parse_number<T>(value: &str, line: usize) -> Result<T, BuildingCatalogError>
where
    T: std::str::FromStr,
{
    value
        .trim()
        .parse()
        .map_err(|_| BuildingCatalogError::InvalidNumber { line })
}

fn parse_cost(value: &str, line: usize) -> Result<ResourceCost, BuildingCatalogError> {
    let stock = parse_stock(value, line)?;
    Ok(ResourceCost::new(stock.metal, stock.crystal, stock.fuel))
}

fn parse_stock(value: &str, line: usize) -> Result<ResourceStock, BuildingCatalogError> {
    let values = value.split(',').collect::<Vec<_>>();
    if values.len() != 3 {
        return Err(BuildingCatalogError::InvalidNumber { line });
    }
    Ok(ResourceStock::new(
        parse_number(values[0], line)?,
        parse_number(values[1], line)?,
        parse_number(values[2], line)?,
    ))
}

fn parse_effect(value: &str, line: usize) -> Result<BuildingEffect, BuildingCatalogError> {
    let Some((kind, amount)) = value.split_once(':') else {
        return Err(BuildingCatalogError::InvalidEffect { line });
    };

    match kind {
        "metal_production" => Ok(BuildingEffect::MetalProduction {
            milli_per_tick_per_level: parse_number(amount, line)?,
        }),
        "crystal_production" => Ok(BuildingEffect::CrystalProduction {
            milli_per_tick_per_level: parse_number(amount, line)?,
        }),
        "fuel_production" => Ok(BuildingEffect::FuelProduction {
            milli_per_tick_per_level: parse_number(amount, line)?,
        }),
        "energy_production" => Ok(BuildingEffect::EnergyProduction {
            capacity_per_level: parse_number(amount, line)?,
        }),
        "storage" => Ok(BuildingEffect::Storage {
            per_level: parse_stock(amount, line)?,
        }),
        "construction_speed" => Ok(BuildingEffect::ConstructionSpeed {
            permille_per_level: parse_number(amount, line)?,
        }),
        "research_points" => Ok(BuildingEffect::ResearchPoints {
            milli_per_tick_per_level: parse_number(amount, line)?,
        }),
        "shipyard_points" => Ok(BuildingEffect::ShipyardPoints {
            milli_per_tick_per_level: parse_number(amount, line)?,
        }),
        _ => Err(BuildingCatalogError::UnknownEffect { line }),
    }
}

fn parse_prerequisites(
    value: &str,
    line: usize,
) -> Result<Vec<BuildingPrerequisite>, BuildingCatalogError> {
    if value == "-" {
        return Ok(Vec::new());
    }

    value
        .split(',')
        .map(|item| {
            let Some((kind, level)) = item.split_once(':') else {
                return Err(BuildingCatalogError::InvalidPrerequisite { line });
            };
            Ok(BuildingPrerequisite {
                kind: BuildingKind::parse(kind)
                    .ok_or(BuildingCatalogError::UnknownBuilding { line })?,
                level: parse_number(level, line)?,
            })
        })
        .collect()
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

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_has_exact_mvp_scope() {
        let catalog = default_building_catalog();

        assert_eq!(catalog.version(), 1);
        assert_eq!(catalog.definitions().count(), BuildingKind::ALL.len());
        for kind in BuildingKind::ALL {
            assert_eq!(catalog.definition(kind).kind, kind);
        }
    }

    #[test]
    fn costs_and_durations_scale_without_simulation_changes() {
        let definition = default_building_catalog().definition(BuildingKind::MetalMine);

        assert_eq!(
            definition.cost_for_level(1).expect("level 1 exists"),
            ResourceCost::new(120, 60, 20)
        );
        assert!(definition.cost_for_level(2).expect("level 2 exists").metal > 120);
        assert!(
            definition.duration_for_level(2).expect("level 2 exists")
                > definition.base_duration_ticks
        );
    }

    #[test]
    fn invalid_prerequisite_is_rejected_at_load() {
        let invalid = EMBEDDED_CATALOG.replace(
            "construction_center:2,metal_mine:2,crystal_extractor:2",
            "shipyard:1",
        );

        assert!(matches!(
            BuildingCatalog::parse(&invalid),
            Err(BuildingCatalogError::SelfPrerequisite(
                BuildingKind::Shipyard
            ))
        ));
    }

    #[test]
    fn starting_levels_match_catalog_prerequisites() {
        assert_eq!(
            default_building_catalog().validate_levels(BuildingLevels::MVP_START),
            Ok(())
        );
    }
}

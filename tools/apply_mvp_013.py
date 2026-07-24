#!/usr/bin/env python3
"""
Applique MVP-013 au dépôt Galactic.

Baseline analysée :
    c00a79ad19be55730357f091c52decedc9d1f432
    feat add production

Le script :
- crée un catalogue de huit bâtiments dans un fichier de données simple ;
- valide coûts, durées, effets, niveaux et prérequis ;
- remplace les constantes de production par le catalogue ;
- regroupe les crédits de ressources toutes les 5 secondes stratégiques ;
- sauvegarde les ticks en attente et l'identité du catalogue ;
- ajoute un événement ProductionRefreshed ;
- prépare MVP-013-B pour le polish d'affichage des ressources.

Usage :
    python tools/apply_mvp_013.py --dry-run
    python tools/apply_mvp_013.py
    python tools/apply_mvp_013.py --skip-checks
    python tools/apply_mvp_013.py --root /chemin/vers/galactic
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
    "c00a79ad19be55730357f091c52decedc9d1f432"
)

CATALOG_DATA = '# Galactic MVP building catalog v1\n# Format:\n# building=kind|name|max_level|base_cost M,C,F|cost_growth_permille|base_duration_ticks|duration_growth_permille|energy_consumption_per_level|effect|prerequisites\nversion=1\nbase_storage=1000,800,600\n\nbuilding=metal_mine|Mine de métal|10|120,60,20|1450|300|1250|6|metal_production:250|-\nbuilding=crystal_extractor|Extracteur de cristal|10|100,80,25|1450|320|1250|5|crystal_production:125|-\nbuilding=fuel_refinery|Raffinerie de carburant|10|140,90,40|1500|360|1280|7|fuel_production:75|-\nbuilding=power_plant|Centrale énergétique|10|160,120,40|1500|400|1300|0|energy_production:80|-\nbuilding=warehouse|Entrepôt|10|180,100,30|1450|350|1250|2|storage:4000,3200,2400|-\nbuilding=construction_center|Centre de construction|5|250,180,80|1600|600|1400|10|construction_speed:100|-\nbuilding=research_lab|Laboratoire|5|300,250,100|1650|700|1450|12|research_points:25|construction_center:1\nbuilding=shipyard|Chantier spatial|5|500,350,220|1700|900|1500|18|shipyard_points:20|construction_center:2,metal_mine:2,crystal_extractor:2\n'
BUILDING_CATALOG_RS = '// MVP-013: validated building definitions loaded from a simple data asset.\nuse std::collections::{BTreeMap, BTreeSet};\nuse std::sync::OnceLock;\n\nuse galactic_domain::{EnergyGrid, ResourceCost, ResourceStock};\n\nuse crate::{BuildingKind, BuildingLevels};\n\nconst EMBEDDED_CATALOG: &str =\n    include_str!("../../../assets/data/buildings.catalog");\n\nstatic DEFAULT_CATALOG: OnceLock<BuildingCatalog> = OnceLock::new();\n\npub fn default_building_catalog() -> &\'static BuildingCatalog {\n    DEFAULT_CATALOG.get_or_init(|| {\n        BuildingCatalog::parse(EMBEDDED_CATALOG)\n            .expect("the embedded MVP building catalog must be valid")\n    })\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum BuildingEffect {\n    MetalProduction {\n        milli_per_tick_per_level: u64,\n    },\n    CrystalProduction {\n        milli_per_tick_per_level: u64,\n    },\n    FuelProduction {\n        milli_per_tick_per_level: u64,\n    },\n    EnergyProduction {\n        capacity_per_level: u64,\n    },\n    Storage {\n        per_level: ResourceStock,\n    },\n    ConstructionSpeed {\n        permille_per_level: u64,\n    },\n    ResearchPoints {\n        milli_per_tick_per_level: u64,\n    },\n    ShipyardPoints {\n        milli_per_tick_per_level: u64,\n    },\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct BuildingPrerequisite {\n    pub kind: BuildingKind,\n    pub level: u8,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BuildingDefinition {\n    pub kind: BuildingKind,\n    pub name: String,\n    pub max_level: u8,\n    pub base_cost: ResourceCost,\n    pub cost_growth_per_mille: u16,\n    pub base_duration_ticks: u64,\n    pub duration_growth_per_mille: u16,\n    pub energy_consumption_per_level: u64,\n    pub effect: BuildingEffect,\n    pub prerequisites: Vec<BuildingPrerequisite>,\n}\n\nimpl BuildingDefinition {\n    pub fn cost_for_level(\n        &self,\n        target_level: u8,\n    ) -> Result<ResourceCost, BuildingCatalogError> {\n        self.validate_target_level(target_level)?;\n        let steps = target_level.saturating_sub(1);\n        Ok(ResourceCost::new(\n            scale_progression(\n                self.base_cost.metal,\n                self.cost_growth_per_mille,\n                steps,\n            ),\n            scale_progression(\n                self.base_cost.crystal,\n                self.cost_growth_per_mille,\n                steps,\n            ),\n            scale_progression(\n                self.base_cost.fuel,\n                self.cost_growth_per_mille,\n                steps,\n            ),\n        ))\n    }\n\n    pub fn duration_for_level(\n        &self,\n        target_level: u8,\n    ) -> Result<u64, BuildingCatalogError> {\n        self.validate_target_level(target_level)?;\n        Ok(scale_progression(\n            self.base_duration_ticks,\n            self.duration_growth_per_mille,\n            target_level.saturating_sub(1),\n        ))\n    }\n\n    fn validate_target_level(\n        &self,\n        target_level: u8,\n    ) -> Result<(), BuildingCatalogError> {\n        if target_level == 0 || target_level > self.max_level {\n            return Err(BuildingCatalogError::InvalidTargetLevel {\n                kind: self.kind,\n                target_level,\n                max_level: self.max_level,\n            });\n        }\n        Ok(())\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BuildingCatalog {\n    version: u32,\n    fingerprint: u64,\n    base_storage: ResourceStock,\n    definitions: BTreeMap<BuildingKind, BuildingDefinition>,\n}\n\nimpl BuildingCatalog {\n    pub fn parse(source: &str) -> Result<Self, BuildingCatalogError> {\n        let mut version = None;\n        let mut base_storage = None;\n        let mut definitions = BTreeMap::new();\n\n        for (index, raw_line) in source.lines().enumerate() {\n            let line_number = index + 1;\n            let line = raw_line.trim();\n            if line.is_empty() || line.starts_with(\'#\') {\n                continue;\n            }\n\n            if let Some(value) = line.strip_prefix("version=") {\n                if version.is_some() {\n                    return Err(BuildingCatalogError::DuplicateHeader {\n                        line: line_number,\n                    });\n                }\n                version = Some(\n                    value\n                        .parse()\n                        .map_err(|_| BuildingCatalogError::InvalidNumber {\n                            line: line_number,\n                        })?,\n                );\n                continue;\n            }\n\n            if let Some(value) = line.strip_prefix("base_storage=") {\n                if base_storage.is_some() {\n                    return Err(BuildingCatalogError::DuplicateHeader {\n                        line: line_number,\n                    });\n                }\n                base_storage = Some(parse_stock(value, line_number)?);\n                continue;\n            }\n\n            let Some(value) = line.strip_prefix("building=") else {\n                return Err(BuildingCatalogError::InvalidLine {\n                    line: line_number,\n                });\n            };\n            let definition = parse_definition(value, line_number)?;\n            if definitions\n                .insert(definition.kind, definition)\n                .is_some()\n            {\n                return Err(BuildingCatalogError::DuplicateBuilding {\n                    line: line_number,\n                });\n            }\n        }\n\n        let version = version.ok_or(\n            BuildingCatalogError::MissingVersion,\n        )?;\n        if version != 1 {\n            return Err(BuildingCatalogError::UnsupportedVersion(\n                version,\n            ));\n        }\n\n        let catalog = Self {\n            version,\n            fingerprint: fnv1a64(source.as_bytes()),\n            base_storage: base_storage.ok_or(\n                BuildingCatalogError::MissingBaseStorage,\n            )?,\n            definitions,\n        };\n        catalog.validate()?;\n        Ok(catalog)\n    }\n\n    pub const fn version(&self) -> u32 {\n        self.version\n    }\n\n    pub const fn fingerprint(&self) -> u64 {\n        self.fingerprint\n    }\n\n    pub const fn base_storage(&self) -> ResourceStock {\n        self.base_storage\n    }\n\n    pub fn definitions(\n        &self,\n    ) -> impl Iterator<Item = &BuildingDefinition> {\n        self.definitions.values()\n    }\n\n    pub fn definition(\n        &self,\n        kind: BuildingKind,\n    ) -> &BuildingDefinition {\n        self.definitions\n            .get(&kind)\n            .expect("validated catalog contains every building")\n    }\n\n    pub fn validate_levels(\n        &self,\n        levels: BuildingLevels,\n    ) -> Result<(), BuildingCatalogError> {\n        for kind in BuildingKind::ALL {\n            let current = levels.level(kind);\n            let definition = self.definition(kind);\n            if current > definition.max_level {\n                return Err(\n                    BuildingCatalogError::LevelExceedsMaximum {\n                        kind,\n                        level: current,\n                        max_level: definition.max_level,\n                    },\n                );\n            }\n            if current == 0 {\n                continue;\n            }\n            for prerequisite in &definition.prerequisites {\n                let found = levels.level(prerequisite.kind);\n                if found < prerequisite.level {\n                    return Err(\n                        BuildingCatalogError::UnsatisfiedPrerequisite {\n                            kind,\n                            prerequisite: prerequisite.kind,\n                            required: prerequisite.level,\n                            found,\n                        },\n                    );\n                }\n            }\n        }\n        Ok(())\n    }\n\n    pub fn energy_grid_for_levels(\n        &self,\n        levels: BuildingLevels,\n    ) -> EnergyGrid {\n        let mut production = 0_u64;\n        let mut consumption = 0_u64;\n\n        for kind in BuildingKind::ALL {\n            let level = u64::from(levels.level(kind));\n            if level == 0 {\n                continue;\n            }\n            let definition = self.definition(kind);\n            consumption = consumption.saturating_add(\n                definition\n                    .energy_consumption_per_level\n                    .saturating_mul(level),\n            );\n            if let BuildingEffect::EnergyProduction {\n                capacity_per_level,\n            } = definition.effect\n            {\n                production = production.saturating_add(\n                    capacity_per_level.saturating_mul(level),\n                );\n            }\n        }\n\n        EnergyGrid::new(production, consumption)\n    }\n\n    fn validate(&self) -> Result<(), BuildingCatalogError> {\n        for kind in BuildingKind::ALL {\n            if !self.definitions.contains_key(&kind) {\n                return Err(\n                    BuildingCatalogError::MissingBuilding(kind),\n                );\n            }\n        }\n        if self.definitions.len() != BuildingKind::ALL.len() {\n            return Err(\n                BuildingCatalogError::UnexpectedBuildingCount {\n                    found: self.definitions.len(),\n                },\n            );\n        }\n\n        for definition in self.definitions.values() {\n            if definition.name.trim().is_empty() {\n                return Err(BuildingCatalogError::EmptyName(\n                    definition.kind,\n                ));\n            }\n            if definition.max_level == 0\n                || definition.base_duration_ticks == 0\n                || definition.cost_growth_per_mille < 1_000\n                || definition.duration_growth_per_mille < 1_000\n            {\n                return Err(\n                    BuildingCatalogError::InvalidDefinition(\n                        definition.kind,\n                    ),\n                );\n            }\n\n            let mut prerequisites = BTreeSet::new();\n            for prerequisite in &definition.prerequisites {\n                if prerequisite.kind == definition.kind {\n                    return Err(\n                        BuildingCatalogError::SelfPrerequisite(\n                            definition.kind,\n                        ),\n                    );\n                }\n                if !prerequisites.insert(prerequisite.kind) {\n                    return Err(\n                        BuildingCatalogError::DuplicatePrerequisite {\n                            kind: definition.kind,\n                            prerequisite: prerequisite.kind,\n                        },\n                    );\n                }\n                let Some(required) =\n                    self.definitions.get(&prerequisite.kind)\n                else {\n                    return Err(\n                        BuildingCatalogError::MissingPrerequisite {\n                            kind: definition.kind,\n                            prerequisite: prerequisite.kind,\n                        },\n                    );\n                };\n                if prerequisite.level == 0\n                    || prerequisite.level > required.max_level\n                {\n                    return Err(\n                        BuildingCatalogError::InvalidPrerequisiteLevel {\n                            kind: definition.kind,\n                            prerequisite: prerequisite.kind,\n                            level: prerequisite.level,\n                        },\n                    );\n                }\n            }\n        }\n\n        for kind in BuildingKind::ALL {\n            let mut visiting = BTreeSet::new();\n            let mut visited = BTreeSet::new();\n            self.visit(kind, &mut visiting, &mut visited)?;\n        }\n\n        Ok(())\n    }\n\n    fn visit(\n        &self,\n        kind: BuildingKind,\n        visiting: &mut BTreeSet<BuildingKind>,\n        visited: &mut BTreeSet<BuildingKind>,\n    ) -> Result<(), BuildingCatalogError> {\n        if visited.contains(&kind) {\n            return Ok(());\n        }\n        if !visiting.insert(kind) {\n            return Err(\n                BuildingCatalogError::PrerequisiteCycle(kind),\n            );\n        }\n\n        for prerequisite in\n            &self.definition(kind).prerequisites\n        {\n            self.visit(\n                prerequisite.kind,\n                visiting,\n                visited,\n            )?;\n        }\n\n        visiting.remove(&kind);\n        visited.insert(kind);\n        Ok(())\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum BuildingCatalogError {\n    InvalidLine {\n        line: usize,\n    },\n    InvalidNumber {\n        line: usize,\n    },\n    UnknownBuilding {\n        line: usize,\n    },\n    UnknownEffect {\n        line: usize,\n    },\n    InvalidEffect {\n        line: usize,\n    },\n    InvalidPrerequisite {\n        line: usize,\n    },\n    DuplicateHeader {\n        line: usize,\n    },\n    DuplicateBuilding {\n        line: usize,\n    },\n    MissingVersion,\n    UnsupportedVersion(u32),\n    MissingBaseStorage,\n    MissingBuilding(BuildingKind),\n    UnexpectedBuildingCount {\n        found: usize,\n    },\n    EmptyName(BuildingKind),\n    InvalidDefinition(BuildingKind),\n    SelfPrerequisite(BuildingKind),\n    DuplicatePrerequisite {\n        kind: BuildingKind,\n        prerequisite: BuildingKind,\n    },\n    MissingPrerequisite {\n        kind: BuildingKind,\n        prerequisite: BuildingKind,\n    },\n    InvalidPrerequisiteLevel {\n        kind: BuildingKind,\n        prerequisite: BuildingKind,\n        level: u8,\n    },\n    PrerequisiteCycle(BuildingKind),\n    InvalidTargetLevel {\n        kind: BuildingKind,\n        target_level: u8,\n        max_level: u8,\n    },\n    LevelExceedsMaximum {\n        kind: BuildingKind,\n        level: u8,\n        max_level: u8,\n    },\n    UnsatisfiedPrerequisite {\n        kind: BuildingKind,\n        prerequisite: BuildingKind,\n        required: u8,\n        found: u8,\n    },\n}\n\nimpl BuildingKind {\n    pub const fn key(self) -> &\'static str {\n        match self {\n            Self::MetalMine => "metal_mine",\n            Self::CrystalExtractor => "crystal_extractor",\n            Self::FuelRefinery => "fuel_refinery",\n            Self::PowerPlant => "power_plant",\n            Self::Warehouse => "warehouse",\n            Self::ConstructionCenter => "construction_center",\n            Self::ResearchLab => "research_lab",\n            Self::Shipyard => "shipyard",\n        }\n    }\n\n    fn parse(value: &str) -> Option<Self> {\n        match value {\n            "metal_mine" => Some(Self::MetalMine),\n            "crystal_extractor" => {\n                Some(Self::CrystalExtractor)\n            }\n            "fuel_refinery" => Some(Self::FuelRefinery),\n            "power_plant" => Some(Self::PowerPlant),\n            "warehouse" => Some(Self::Warehouse),\n            "construction_center" => {\n                Some(Self::ConstructionCenter)\n            }\n            "research_lab" => Some(Self::ResearchLab),\n            "shipyard" => Some(Self::Shipyard),\n            _ => None,\n        }\n    }\n}\n\nfn parse_definition(\n    value: &str,\n    line: usize,\n) -> Result<BuildingDefinition, BuildingCatalogError> {\n    let fields = value.split(\'|\').collect::<Vec<_>>();\n    if fields.len() != 10 {\n        return Err(BuildingCatalogError::InvalidLine {\n            line,\n        });\n    }\n\n    let kind = BuildingKind::parse(fields[0]).ok_or(\n        BuildingCatalogError::UnknownBuilding { line },\n    )?;\n    let max_level =\n        parse_number(fields[2], line)?;\n    let base_cost = parse_cost(fields[3], line)?;\n    let cost_growth_per_mille =\n        parse_number(fields[4], line)?;\n    let base_duration_ticks =\n        parse_number(fields[5], line)?;\n    let duration_growth_per_mille =\n        parse_number(fields[6], line)?;\n    let energy_consumption_per_level =\n        parse_number(fields[7], line)?;\n    let effect = parse_effect(fields[8], line)?;\n    let prerequisites =\n        parse_prerequisites(fields[9], line)?;\n\n    Ok(BuildingDefinition {\n        kind,\n        name: fields[1].trim().to_string(),\n        max_level,\n        base_cost,\n        cost_growth_per_mille,\n        base_duration_ticks,\n        duration_growth_per_mille,\n        energy_consumption_per_level,\n        effect,\n        prerequisites,\n    })\n}\n\nfn parse_number<T>(\n    value: &str,\n    line: usize,\n) -> Result<T, BuildingCatalogError>\nwhere\n    T: std::str::FromStr,\n{\n    value.trim().parse().map_err(|_| {\n        BuildingCatalogError::InvalidNumber { line }\n    })\n}\n\nfn parse_cost(\n    value: &str,\n    line: usize,\n) -> Result<ResourceCost, BuildingCatalogError> {\n    let stock = parse_stock(value, line)?;\n    Ok(ResourceCost::new(\n        stock.metal,\n        stock.crystal,\n        stock.fuel,\n    ))\n}\n\nfn parse_stock(\n    value: &str,\n    line: usize,\n) -> Result<ResourceStock, BuildingCatalogError> {\n    let values = value.split(\',\').collect::<Vec<_>>();\n    if values.len() != 3 {\n        return Err(BuildingCatalogError::InvalidNumber {\n            line,\n        });\n    }\n    Ok(ResourceStock::new(\n        parse_number(values[0], line)?,\n        parse_number(values[1], line)?,\n        parse_number(values[2], line)?,\n    ))\n}\n\nfn parse_effect(\n    value: &str,\n    line: usize,\n) -> Result<BuildingEffect, BuildingCatalogError> {\n    let Some((kind, amount)) = value.split_once(\':\') else {\n        return Err(BuildingCatalogError::InvalidEffect {\n            line,\n        });\n    };\n\n    match kind {\n        "metal_production" => Ok(\n            BuildingEffect::MetalProduction {\n                milli_per_tick_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "crystal_production" => Ok(\n            BuildingEffect::CrystalProduction {\n                milli_per_tick_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "fuel_production" => Ok(\n            BuildingEffect::FuelProduction {\n                milli_per_tick_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "energy_production" => Ok(\n            BuildingEffect::EnergyProduction {\n                capacity_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "storage" => Ok(BuildingEffect::Storage {\n            per_level: parse_stock(amount, line)?,\n        }),\n        "construction_speed" => Ok(\n            BuildingEffect::ConstructionSpeed {\n                permille_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "research_points" => Ok(\n            BuildingEffect::ResearchPoints {\n                milli_per_tick_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        "shipyard_points" => Ok(\n            BuildingEffect::ShipyardPoints {\n                milli_per_tick_per_level:\n                    parse_number(amount, line)?,\n            },\n        ),\n        _ => Err(BuildingCatalogError::UnknownEffect {\n            line,\n        }),\n    }\n}\n\nfn parse_prerequisites(\n    value: &str,\n    line: usize,\n) -> Result<Vec<BuildingPrerequisite>, BuildingCatalogError> {\n    if value == "-" {\n        return Ok(Vec::new());\n    }\n\n    value\n        .split(\',\')\n        .map(|item| {\n            let Some((kind, level)) = item.split_once(\':\')\n            else {\n                return Err(\n                    BuildingCatalogError::InvalidPrerequisite {\n                        line,\n                    },\n                );\n            };\n            Ok(BuildingPrerequisite {\n                kind: BuildingKind::parse(kind).ok_or(\n                    BuildingCatalogError::UnknownBuilding {\n                        line,\n                    },\n                )?,\n                level: parse_number(level, line)?,\n            })\n        })\n        .collect()\n}\n\nfn scale_progression(\n    base: u64,\n    growth_per_mille: u16,\n    steps: u8,\n) -> u64 {\n    let mut value = u128::from(base);\n    for _ in 0..steps {\n        value = value\n            .saturating_mul(u128::from(growth_per_mille))\n            .div_ceil(1_000);\n    }\n    value.min(u128::from(u64::MAX)) as u64\n}\n\nfn fnv1a64(bytes: &[u8]) -> u64 {\n    let mut hash = 0xcbf29ce484222325_u64;\n    for byte in bytes {\n        hash ^= u64::from(*byte);\n        hash = hash.wrapping_mul(0x100000001b3);\n    }\n    hash\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn embedded_catalog_has_exact_mvp_scope() {\n        let catalog = default_building_catalog();\n\n        assert_eq!(catalog.version(), 1);\n        assert_eq!(\n            catalog.definitions().count(),\n            BuildingKind::ALL.len()\n        );\n        for kind in BuildingKind::ALL {\n            assert_eq!(\n                catalog.definition(kind).kind,\n                kind\n            );\n        }\n    }\n\n    #[test]\n    fn costs_and_durations_scale_without_simulation_changes() {\n        let definition = default_building_catalog()\n            .definition(BuildingKind::MetalMine);\n\n        assert_eq!(\n            definition\n                .cost_for_level(1)\n                .expect("level 1 exists"),\n            ResourceCost::new(120, 60, 20)\n        );\n        assert!(\n            definition\n                .cost_for_level(2)\n                .expect("level 2 exists")\n                .metal\n                > 120\n        );\n        assert!(\n            definition\n                .duration_for_level(2)\n                .expect("level 2 exists")\n                > definition.base_duration_ticks\n        );\n    }\n\n    #[test]\n    fn invalid_prerequisite_is_rejected_at_load() {\n        let invalid = EMBEDDED_CATALOG.replace(\n            "construction_center:2,metal_mine:2,crystal_extractor:2",\n            "shipyard:1",\n        );\n\n        assert!(matches!(\n            BuildingCatalog::parse(&invalid),\n            Err(\n                BuildingCatalogError::SelfPrerequisite(\n                    BuildingKind::Shipyard\n                )\n            )\n        ));\n    }\n\n    #[test]\n    fn starting_levels_match_catalog_prerequisites() {\n        assert_eq!(\n            default_building_catalog()\n                .validate_levels(BuildingLevels::MVP_START),\n            Ok(())\n        );\n    }\n}\n'
PRODUCTION_RS = '// MVP-013: catalog-driven production with a five-second refresh cadence.\nuse galactic_domain::{ColonyId, ResourceStock};\n\nuse crate::{\n    BuildingEffect, BuildingKind, BuildingLevels,\n    ColonyState, PlanetResourceProfile, StrategicDuration,\n    STRATEGIC_TICKS_PER_SECOND, default_building_catalog,\n};\n\n/// Fixed-point scale used for sub-unit production.\npub const PRODUCTION_SCALE: u64 = 1_000;\n\n/// Stocks are credited every five strategic seconds, not every tick.\npub const PRODUCTION_REFRESH_SECONDS: u64 = 5;\npub const PRODUCTION_REFRESH_TICKS: u64 =\n    PRODUCTION_REFRESH_SECONDS\n        * STRATEGIC_TICKS_PER_SECOND as u64;\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ProductionRemainder {\n    metal_milli: u16,\n    crystal_milli: u16,\n    fuel_milli: u16,\n}\n\nimpl ProductionRemainder {\n    pub const ZERO: Self =\n        Self::new_unchecked(0, 0, 0);\n\n    const fn new_unchecked(\n        metal_milli: u16,\n        crystal_milli: u16,\n        fuel_milli: u16,\n    ) -> Self {\n        Self {\n            metal_milli,\n            crystal_milli,\n            fuel_milli,\n        }\n    }\n\n    pub fn from_parts(\n        metal_milli: u16,\n        crystal_milli: u16,\n        fuel_milli: u16,\n    ) -> Result<Self, ProductionRemainderError> {\n        let scale = PRODUCTION_SCALE as u16;\n        if metal_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Metal,\n                    value: metal_milli,\n                },\n            );\n        }\n        if crystal_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Crystal,\n                    value: crystal_milli,\n                },\n            );\n        }\n        if fuel_milli >= scale {\n            return Err(\n                ProductionRemainderError::OutOfRange {\n                    resource: ProductionResource::Fuel,\n                    value: fuel_milli,\n                },\n            );\n        }\n\n        Ok(Self::new_unchecked(\n            metal_milli,\n            crystal_milli,\n            fuel_milli,\n        ))\n    }\n\n    pub const fn metal_milli(self) -> u16 {\n        self.metal_milli\n    }\n\n    pub const fn crystal_milli(self) -> u16 {\n        self.crystal_milli\n    }\n\n    pub const fn fuel_milli(self) -> u16 {\n        self.fuel_milli\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ProductionResource {\n    Metal,\n    Crystal,\n    Fuel,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ProductionRemainderError {\n    OutOfRange {\n        resource: ProductionResource,\n        value: u16,\n    },\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct ProductionRate {\n    pub metal_milli_per_tick: u64,\n    pub crystal_milli_per_tick: u64,\n    pub fuel_milli_per_tick: u64,\n}\n\nimpl ProductionRate {\n    pub const ZERO: Self = Self {\n        metal_milli_per_tick: 0,\n        crystal_milli_per_tick: 0,\n        fuel_milli_per_tick: 0,\n    };\n\n    pub fn for_colony(\n        buildings: BuildingLevels,\n        profile: PlanetResourceProfile,\n    ) -> Self {\n        let catalog = default_building_catalog();\n        let mut rate = Self::ZERO;\n\n        for kind in BuildingKind::ALL {\n            let level = buildings.level(kind);\n            if level == 0 {\n                continue;\n            }\n            let effect = catalog.definition(kind).effect;\n            match effect {\n                BuildingEffect::MetalProduction {\n                    milli_per_tick_per_level,\n                } => {\n                    rate.metal_milli_per_tick =\n                        modified_rate(\n                            milli_per_tick_per_level,\n                            level,\n                            profile.metal,\n                        );\n                }\n                BuildingEffect::CrystalProduction {\n                    milli_per_tick_per_level,\n                } => {\n                    rate.crystal_milli_per_tick =\n                        modified_rate(\n                            milli_per_tick_per_level,\n                            level,\n                            profile.crystal,\n                        );\n                }\n                BuildingEffect::FuelProduction {\n                    milli_per_tick_per_level,\n                } => {\n                    rate.fuel_milli_per_tick =\n                        modified_rate(\n                            milli_per_tick_per_level,\n                            level,\n                            profile.fuel,\n                        );\n                }\n                _ => {}\n            }\n        }\n\n        rate\n    }\n\n    pub fn scaled_by_permille(\n        self,\n        efficiency_per_mille: u16,\n    ) -> Self {\n        Self {\n            metal_milli_per_tick: scale_rate(\n                self.metal_milli_per_tick,\n                efficiency_per_mille,\n            ),\n            crystal_milli_per_tick: scale_rate(\n                self.crystal_milli_per_tick,\n                efficiency_per_mille,\n            ),\n            fuel_milli_per_tick: scale_rate(\n                self.fuel_milli_per_tick,\n                efficiency_per_mille,\n            ),\n        }\n    }\n\n    pub fn metal_per_second(self) -> f64 {\n        per_second(self.metal_milli_per_tick)\n    }\n\n    pub fn crystal_per_second(self) -> f64 {\n        per_second(self.crystal_milli_per_tick)\n    }\n\n    pub fn fuel_per_second(self) -> f64 {\n        per_second(self.fuel_milli_per_tick)\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaturationTime {\n    Full,\n    Never,\n    In(StrategicDuration),\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SaturationEstimate {\n    pub metal: SaturationTime,\n    pub crystal: SaturationTime,\n    pub fuel: SaturationTime,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ColonyProductionSnapshot {\n    pub capacity: ResourceStock,\n    pub nominal_rate: ProductionRate,\n    pub effective_rate: ProductionRate,\n    pub nominal_energy_production: u64,\n    pub effective_energy_production: u64,\n    pub energy_consumption: u64,\n    pub energy_efficiency_per_mille: u16,\n    pub pending_ticks: u16,\n    pub ticks_until_refresh: u16,\n    pub saturation: SaturationEstimate,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ColonyProductionReport {\n    pub colony_id: ColonyId,\n    pub ticks: StrategicDuration,\n    pub produced: ResourceStock,\n    pub blocked_by_storage: ResourceStock,\n    pub energy_efficiency_per_mille: u16,\n}\n\npub fn storage_capacity(\n    buildings: BuildingLevels,\n) -> ResourceStock {\n    let catalog = default_building_catalog();\n    let mut capacity = catalog.base_storage();\n\n    for kind in BuildingKind::ALL {\n        let level = u64::from(buildings.level(kind));\n        if level == 0 {\n            continue;\n        }\n        if let BuildingEffect::Storage { per_level } =\n            catalog.definition(kind).effect\n        {\n            capacity = ResourceStock::new(\n                capacity.metal.saturating_add(\n                    per_level.metal.saturating_mul(level),\n                ),\n                capacity.crystal.saturating_add(\n                    per_level.crystal.saturating_mul(level),\n                ),\n                capacity.fuel.saturating_add(\n                    per_level.fuel.saturating_mul(level),\n                ),\n            );\n        }\n    }\n\n    capacity\n}\n\npub fn colony_production_snapshot(\n    colony: &ColonyState,\n) -> ColonyProductionSnapshot {\n    let catalog = default_building_catalog();\n    let capacity = storage_capacity(colony.buildings);\n    let nominal_rate = ProductionRate::for_colony(\n        colony.buildings,\n        colony.resource_profile,\n    );\n    let nominal_grid =\n        catalog.energy_grid_for_levels(colony.buildings);\n    let nominal_energy_production =\n        nominal_grid.production();\n    let energy_consumption =\n        nominal_grid.consumption();\n    let effective_energy_production =\n        apply_modifier(\n            nominal_energy_production,\n            colony.resource_profile.energy,\n        );\n    let energy_efficiency_per_mille =\n        energy_efficiency_per_mille(\n            effective_energy_production,\n            energy_consumption,\n        );\n    let effective_rate =\n        nominal_rate.scaled_by_permille(\n            energy_efficiency_per_mille,\n        );\n    let stock = colony.resources.stock();\n    let remainder = colony.production_remainder;\n    let pending_ticks = colony.production_pending_ticks;\n    let ticks_until_refresh =\n        if pending_ticks == 0 {\n            PRODUCTION_REFRESH_TICKS as u16\n        } else {\n            PRODUCTION_REFRESH_TICKS as u16\n                - pending_ticks\n        };\n\n    ColonyProductionSnapshot {\n        capacity,\n        nominal_rate,\n        effective_rate,\n        nominal_energy_production,\n        effective_energy_production,\n        energy_consumption,\n        energy_efficiency_per_mille,\n        pending_ticks,\n        ticks_until_refresh,\n        saturation: SaturationEstimate {\n            metal: saturation_time(\n                stock.metal,\n                capacity.metal,\n                effective_rate.metal_milli_per_tick,\n                remainder.metal_milli(),\n            ),\n            crystal: saturation_time(\n                stock.crystal,\n                capacity.crystal,\n                effective_rate.crystal_milli_per_tick,\n                remainder.crystal_milli(),\n            ),\n            fuel: saturation_time(\n                stock.fuel,\n                capacity.fuel,\n                effective_rate.fuel_milli_per_tick,\n                remainder.fuel_milli(),\n            ),\n        },\n    }\n}\n\n/// Adds strategic ticks to the five-second production window.\n///\n/// Returns a report only when one or more complete windows are credited.\npub fn queue_colony_production(\n    colony: &mut ColonyState,\n    ticks: StrategicDuration,\n) -> Option<ColonyProductionReport> {\n    let total = u64::from(\n        colony.production_pending_ticks,\n    )\n    .saturating_add(ticks.ticks());\n    let processed_ticks =\n        total / PRODUCTION_REFRESH_TICKS\n            * PRODUCTION_REFRESH_TICKS;\n    colony.production_pending_ticks =\n        (total % PRODUCTION_REFRESH_TICKS) as u16;\n\n    if processed_ticks == 0 {\n        return None;\n    }\n\n    let catalog = default_building_catalog();\n    colony.energy =\n        catalog.energy_grid_for_levels(colony.buildings);\n\n    Some(apply_colony_production(\n        colony,\n        StrategicDuration::from_ticks(processed_ticks),\n    ))\n}\n\npub fn apply_colony_production(\n    colony: &mut ColonyState,\n    ticks: StrategicDuration,\n) -> ColonyProductionReport {\n    let snapshot = colony_production_snapshot(colony);\n    let tick_count = ticks.ticks();\n\n    let (metal, next_metal) = generated_units(\n        snapshot.effective_rate.metal_milli_per_tick,\n        tick_count,\n        colony.production_remainder.metal_milli(),\n    );\n    let (crystal, next_crystal) = generated_units(\n        snapshot.effective_rate.crystal_milli_per_tick,\n        tick_count,\n        colony.production_remainder.crystal_milli(),\n    );\n    let (fuel, next_fuel) = generated_units(\n        snapshot.effective_rate.fuel_milli_per_tick,\n        tick_count,\n        colony.production_remainder.fuel_milli(),\n    );\n\n    let requested = ResourceStock::new(\n        metal,\n        crystal,\n        fuel,\n    );\n    let produced = colony.resources.credit_capped(\n        requested,\n        snapshot.capacity,\n    );\n    let blocked_by_storage =\n        requested.saturating_sub(produced);\n\n    colony.production_remainder =\n        ProductionRemainder::new_unchecked(\n            if produced.metal < requested.metal {\n                0\n            } else {\n                next_metal\n            },\n            if produced.crystal < requested.crystal {\n                0\n            } else {\n                next_crystal\n            },\n            if produced.fuel < requested.fuel {\n                0\n            } else {\n                next_fuel\n            },\n        );\n\n    ColonyProductionReport {\n        colony_id: colony.id,\n        ticks,\n        produced,\n        blocked_by_storage,\n        energy_efficiency_per_mille:\n            snapshot.energy_efficiency_per_mille,\n    }\n}\n\nfn modified_rate(\n    base_milli_per_tick: u64,\n    level: u8,\n    modifier_percent: u16,\n) -> u64 {\n    let value = u128::from(base_milli_per_tick)\n        .saturating_mul(u128::from(level))\n        .saturating_mul(u128::from(modifier_percent))\n        / 100;\n    value.min(u128::from(u64::MAX)) as u64\n}\n\nfn scale_rate(\n    rate: u64,\n    efficiency_per_mille: u16,\n) -> u64 {\n    let value = u128::from(rate)\n        .saturating_mul(u128::from(\n            efficiency_per_mille,\n        ))\n        / u128::from(PRODUCTION_SCALE);\n    value.min(u128::from(u64::MAX)) as u64\n}\n\nfn apply_modifier(\n    value: u64,\n    modifier_percent: u16,\n) -> u64 {\n    let modified = u128::from(value)\n        .saturating_mul(u128::from(modifier_percent))\n        / 100;\n    modified.min(u128::from(u64::MAX)) as u64\n}\n\nfn energy_efficiency_per_mille(\n    effective_production: u64,\n    consumption: u64,\n) -> u16 {\n    if consumption == 0\n        || effective_production >= consumption\n    {\n        return PRODUCTION_SCALE as u16;\n    }\n\n    let value = u128::from(effective_production)\n        .saturating_mul(u128::from(PRODUCTION_SCALE))\n        / u128::from(consumption);\n    value.min(u128::from(PRODUCTION_SCALE)) as u16\n}\n\nfn generated_units(\n    rate_milli_per_tick: u64,\n    ticks: u64,\n    previous_remainder: u16,\n) -> (u64, u16) {\n    let total = u128::from(rate_milli_per_tick)\n        .saturating_mul(u128::from(ticks))\n        .saturating_add(u128::from(\n            previous_remainder,\n        ));\n    let units = total / u128::from(PRODUCTION_SCALE);\n    let remainder =\n        (total % u128::from(PRODUCTION_SCALE)) as u16;\n\n    (\n        units.min(u128::from(u64::MAX)) as u64,\n        remainder,\n    )\n}\n\nfn saturation_time(\n    stock: u64,\n    capacity: u64,\n    rate_milli_per_tick: u64,\n    remainder_milli: u16,\n) -> SaturationTime {\n    if stock >= capacity {\n        return SaturationTime::Full;\n    }\n    if rate_milli_per_tick == 0 {\n        return SaturationTime::Never;\n    }\n\n    let missing_units = capacity - stock;\n    let required_milli =\n        u128::from(missing_units)\n            .saturating_mul(u128::from(\n                PRODUCTION_SCALE,\n            ))\n            .saturating_sub(u128::from(\n                remainder_milli,\n            ));\n    let rate = u128::from(rate_milli_per_tick);\n    let ticks = required_milli\n        .saturating_add(rate - 1)\n        / rate;\n\n    SaturationTime::In(\n        StrategicDuration::from_ticks(\n            ticks.min(u128::from(u64::MAX)) as u64,\n        ),\n    )\n}\n\nfn per_second(rate_milli_per_tick: u64) -> f64 {\n    rate_milli_per_tick as f64\n        * f64::from(STRATEGIC_TICKS_PER_SECOND)\n        / PRODUCTION_SCALE as f64\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{\n        ResourceLedger, UniverseConfig,\n    };\n\n    use crate::Simulation;\n\n    use super::*;\n\n    fn home_colony() -> ColonyState {\n        Simulation::new(UniverseConfig::mvp())\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .clone()\n    }\n\n    #[test]\n    fn starting_colony_uses_catalog_rates_and_capacity() {\n        let colony = home_colony();\n        let snapshot =\n            colony_production_snapshot(&colony);\n\n        assert_eq!(\n            snapshot.capacity,\n            ResourceStock::new(5_000, 4_000, 3_000)\n        );\n        assert_eq!(\n            snapshot.nominal_rate,\n            ProductionRate {\n                metal_milli_per_tick: 250,\n                crystal_milli_per_tick: 125,\n                fuel_milli_per_tick: 75,\n            }\n        );\n        assert_eq!(\n            snapshot.nominal_energy_production,\n            80\n        );\n        assert_eq!(snapshot.energy_consumption, 30);\n    }\n\n    #[test]\n    fn resources_refresh_only_after_fifty_ticks() {\n        let mut colony = home_colony();\n        let initial = colony.resources.stock();\n\n        assert!(queue_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(49),\n        )\n        .is_none());\n        assert_eq!(colony.resources.stock(), initial);\n        assert_eq!(colony.production_pending_ticks, 49);\n\n        let report = queue_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(1),\n        )\n        .expect("the five-second window completes");\n\n        assert_eq!(report.ticks.ticks(), 50);\n        assert_eq!(\n            colony.resources.stock(),\n            ResourceStock::new(612, 306, 223)\n        );\n        assert_eq!(colony.production_pending_ticks, 0);\n    }\n\n    #[test]\n    fn tick_batches_produce_identical_state() {\n        let mut batched = home_colony();\n        let mut incremental = batched.clone();\n\n        queue_colony_production(\n            &mut batched,\n            StrategicDuration::from_ticks(100),\n        );\n        for _ in 0..10 {\n            queue_colony_production(\n                &mut incremental,\n                StrategicDuration::from_ticks(10),\n            );\n        }\n\n        assert_eq!(batched, incremental);\n        assert_eq!(\n            batched.resources.stock(),\n            ResourceStock::new(625, 312, 227)\n        );\n        assert_eq!(\n            batched.production_remainder,\n            ProductionRemainder::from_parts(\n                0, 500, 500,\n            )\n            .expect("valid remainder")\n        );\n    }\n\n    #[test]\n    fn full_storage_discards_blocked_output() {\n        let mut colony = home_colony();\n        let capacity = storage_capacity(colony.buildings);\n        colony.resources = ResourceLedger::new(\n            ResourceStock::new(\n                capacity.metal - 1,\n                capacity.crystal - 1,\n                capacity.fuel - 1,\n            ),\n        );\n\n        let report = apply_colony_production(\n            &mut colony,\n            StrategicDuration::from_ticks(1_000),\n        );\n\n        assert_eq!(colony.resources.stock(), capacity);\n        assert!(\n            !report.blocked_by_storage.is_zero()\n        );\n        assert_eq!(\n            colony.production_remainder,\n            ProductionRemainder::ZERO\n        );\n    }\n\n    #[test]\n    fn planet_profile_applies_catalog_modifiers() {\n        let mut colony = home_colony();\n        colony.resource_profile =\n            PlanetResourceProfile::new(\n                150, 80, 50, 50,\n            );\n        let snapshot =\n            colony_production_snapshot(&colony);\n\n        assert_eq!(\n            snapshot.nominal_rate\n                .metal_milli_per_tick,\n            375\n        );\n        assert_eq!(\n            snapshot.nominal_rate\n                .crystal_milli_per_tick,\n            100\n        );\n        assert_eq!(\n            snapshot.nominal_rate.fuel_milli_per_tick,\n            37\n        );\n        assert_eq!(\n            snapshot.effective_energy_production,\n            40\n        );\n    }\n}\n'
EVENT_RS = 'use galactic_domain::{PlanetId, SystemId};\n\nuse crate::{\n    ColonyProductionReport, KnowledgeChange,\n    StrategicDuration, StrategicTick, TimeSpeed,\n};\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub enum SelectionTarget {\n    #[default]\n    None,\n    System(SystemId),\n    Planet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameEvent {\n    SpeedChanged(TimeSpeed),\n    SelectionChanged(SelectionTarget),\n    KnowledgeChanged(KnowledgeChange),\n    TicksAdvanced {\n        ticks: StrategicDuration,\n        current_tick: StrategicTick,\n    },\n    ProductionRefreshed(ColonyProductionReport),\n}\n'
PERSISTENCE_RS = '// MVP-013: persist catalog identity and five-second production windows.\nuse galactic_domain::{\n    ColonyId, EnergyGrid, FactionId, PlanetId,\n    ResourceLedger, ResourceLedgerError, ResourceReservation,\n    ResourceStock, SystemId, UniverseConfig, UniverseId,\n    generate_universe,\n};\nuse galactic_sim::{\n    BuildingLevels, ColonyState, FactionKind, FactionState,\n    GameState, PlanetKnowledge,\n    PlanetResourceProfile, ProductionRemainder,\n    ProductionRemainderError, PRODUCTION_REFRESH_TICKS,\n    SelectionTarget, Simulation, SimulationBuildError,\n    StrategicClock, StrategicClockError, StrategicTick,\n    SystemKnowledge, TimeSpeed, default_building_catalog,\n};\n\npub const SAVE_VERSION: u32 = 8;\n\n#[derive(Debug, Clone, PartialEq)]\npub struct SaveGame {\n    pub version: u32,\n    pub catalog_version: u32,\n    pub catalog_fingerprint: u64,\n    pub universe: UniverseReference,\n    pub state: MutableGameSave,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct UniverseReference {\n    pub id: UniverseId,\n    pub seed: u64,\n    pub system_count: usize,\n    pub generation_version: u32,\n    pub generation_fingerprint: u64,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct MutableGameSave {\n    pub version: u32,\n    pub factions: Vec<FactionSave>,\n    pub player_faction: FactionId,\n    pub clock: StrategicClockSave,\n    pub selected: SelectionTarget,\n    pub system_knowledge: Vec<SystemKnowledge>,\n    pub planet_knowledge: Vec<PlanetKnowledge>,\n    pub colonies: Vec<ColonySave>,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactionSave {\n    pub id: FactionId,\n    pub name: String,\n    pub kind: FactionKind,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct StrategicClockSave {\n    pub current_tick: StrategicTick,\n    pub remainder_nanos: u64,\n    pub speed: TimeSpeed,\n    pub resume_speed: TimeSpeed,\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonySave {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n    pub reservations: Vec<ResourceReservation>,\n    pub next_reservation_id: u64,\n    pub energy_production: u64,\n    pub energy_consumption: u64,\n    pub production_remainder_metal: u16,\n    pub production_remainder_crystal: u16,\n    pub production_remainder_fuel: u16,\n    pub production_pending_ticks: u16,\n    pub buildings: BuildingLevels,\n    pub resource_profile: PlanetResourceProfile,\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SaveError {\n    UnsupportedVersion(u32),\n    CatalogVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    CatalogFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    UniverseIdMismatch {\n        expected: UniverseId,\n        found: UniverseId,\n    },\n    GenerationVersionMismatch {\n        expected: u32,\n        found: u32,\n    },\n    GenerationFingerprintMismatch {\n        expected: u64,\n        found: u64,\n    },\n    InvalidClock(StrategicClockError),\n    InvalidResourceLedger {\n        colony_id: ColonyId,\n        error: ResourceLedgerError,\n    },\n    InvalidProductionRemainder {\n        colony_id: ColonyId,\n        error: ProductionRemainderError,\n    },\n    InvalidPendingProductionTicks {\n        colony_id: ColonyId,\n        found: u16,\n    },\n    InvalidState(SimulationBuildError),\n}\n\npub fn snapshot_from_simulation(\n    simulation: &Simulation,\n) -> SaveGame {\n    let universe = simulation.universe();\n    let state = simulation.state();\n    let catalog = default_building_catalog();\n\n    SaveGame {\n        version: SAVE_VERSION,\n        catalog_version: catalog.version(),\n        catalog_fingerprint: catalog.fingerprint(),\n        universe: UniverseReference {\n            id: universe.id,\n            seed: universe.seed,\n            system_count: universe.systems.len(),\n            generation_version: universe.generation_version,\n            generation_fingerprint:\n                universe.generation_fingerprint,\n        },\n        state: MutableGameSave {\n            version: state.version,\n            factions: state\n                .factions\n                .iter()\n                .map(|faction| FactionSave {\n                    id: faction.id,\n                    name: faction.name.clone(),\n                    kind: faction.kind,\n                })\n                .collect(),\n            player_faction: state.player_faction,\n            clock: StrategicClockSave {\n                current_tick: state.clock.current_tick(),\n                remainder_nanos:\n                    state.clock.remainder_nanos(),\n                speed: state.clock.speed(),\n                resume_speed: state.clock.resume_speed(),\n            },\n            selected: state.selected,\n            system_knowledge:\n                state.system_knowledge.clone(),\n            planet_knowledge:\n                state.planet_knowledge.clone(),\n            colonies: state\n                .colonies\n                .iter()\n                .map(|colony| ColonySave {\n                    id: colony.id,\n                    name: colony.name.clone(),\n                    faction: colony.faction,\n                    system_id: colony.system_id,\n                    planet_id: colony.planet_id,\n                    stock: colony.resources.stock(),\n                    reservations: colony\n                        .resources\n                        .reservations()\n                        .to_vec(),\n                    next_reservation_id: colony\n                        .resources\n                        .next_reservation_id(),\n                    energy_production:\n                        colony.energy.production(),\n                    energy_consumption:\n                        colony.energy.consumption(),\n                    production_remainder_metal:\n                        colony\n                            .production_remainder\n                            .metal_milli(),\n                    production_remainder_crystal:\n                        colony\n                            .production_remainder\n                            .crystal_milli(),\n                    production_remainder_fuel:\n                        colony\n                            .production_remainder\n                            .fuel_milli(),\n                    production_pending_ticks:\n                        colony.production_pending_ticks,\n                    buildings: colony.buildings,\n                    resource_profile:\n                        colony.resource_profile,\n                })\n                .collect(),\n        },\n    }\n}\n\npub fn restore_from_snapshot(\n    save: &SaveGame,\n) -> Result<Simulation, SaveError> {\n    if save.version != SAVE_VERSION {\n        return Err(\n            SaveError::UnsupportedVersion(save.version),\n        );\n    }\n\n    let catalog = default_building_catalog();\n    if save.catalog_version != catalog.version() {\n        return Err(SaveError::CatalogVersionMismatch {\n            expected: catalog.version(),\n            found: save.catalog_version,\n        });\n    }\n    if save.catalog_fingerprint != catalog.fingerprint() {\n        return Err(\n            SaveError::CatalogFingerprintMismatch {\n                expected: catalog.fingerprint(),\n                found: save.catalog_fingerprint,\n            },\n        );\n    }\n\n    let universe = generate_universe(UniverseConfig::new(\n        save.universe.seed,\n        save.universe.system_count,\n    ));\n\n    if universe.id != save.universe.id {\n        return Err(SaveError::UniverseIdMismatch {\n            expected: universe.id,\n            found: save.universe.id,\n        });\n    }\n    if universe.generation_version\n        != save.universe.generation_version\n    {\n        return Err(\n            SaveError::GenerationVersionMismatch {\n                expected: universe.generation_version,\n                found:\n                    save.universe.generation_version,\n            },\n        );\n    }\n    if universe.generation_fingerprint\n        != save.universe.generation_fingerprint\n    {\n        return Err(\n            SaveError::GenerationFingerprintMismatch {\n                expected:\n                    universe.generation_fingerprint,\n                found:\n                    save.universe\n                        .generation_fingerprint,\n            },\n        );\n    }\n\n    let clock = StrategicClock::from_parts(\n        save.state.clock.current_tick,\n        save.state.clock.remainder_nanos,\n        save.state.clock.speed,\n        save.state.clock.resume_speed,\n    )\n    .map_err(SaveError::InvalidClock)?;\n\n    let colonies = save\n        .state\n        .colonies\n        .iter()\n        .map(|colony| {\n            let resources = ResourceLedger::from_parts(\n                colony.stock,\n                colony.reservations.clone(),\n                colony.next_reservation_id,\n            )\n            .map_err(|error| {\n                SaveError::InvalidResourceLedger {\n                    colony_id: colony.id,\n                    error,\n                }\n            })?;\n            let production_remainder =\n                ProductionRemainder::from_parts(\n                    colony.production_remainder_metal,\n                    colony.production_remainder_crystal,\n                    colony.production_remainder_fuel,\n                )\n                .map_err(|error| {\n                    SaveError::InvalidProductionRemainder {\n                        colony_id: colony.id,\n                        error,\n                    }\n                })?;\n            if u64::from(colony.production_pending_ticks)\n                >= PRODUCTION_REFRESH_TICKS\n            {\n                return Err(\n                    SaveError::InvalidPendingProductionTicks {\n                        colony_id: colony.id,\n                        found:\n                            colony.production_pending_ticks,\n                    },\n                );\n            }\n\n            Ok(ColonyState {\n                id: colony.id,\n                name: colony.name.clone(),\n                faction: colony.faction,\n                system_id: colony.system_id,\n                planet_id: colony.planet_id,\n                resources,\n                energy: EnergyGrid::new(\n                    colony.energy_production,\n                    colony.energy_consumption,\n                ),\n                production_remainder,\n                production_pending_ticks:\n                    colony.production_pending_ticks,\n                buildings: colony.buildings,\n                resource_profile:\n                    colony.resource_profile,\n            })\n        })\n        .collect::<Result<Vec<_>, SaveError>>()?;\n\n    let state = GameState {\n        version: save.state.version,\n        factions: save\n            .state\n            .factions\n            .iter()\n            .map(|faction| FactionState {\n                id: faction.id,\n                name: faction.name.clone(),\n                kind: faction.kind,\n            })\n            .collect(),\n        player_faction: save.state.player_faction,\n        colonies,\n        system_knowledge:\n            save.state.system_knowledge.clone(),\n        planet_knowledge:\n            save.state.planet_knowledge.clone(),\n        selected: save.state.selected,\n        clock,\n    };\n\n    Simulation::from_parts(universe, state)\n        .map_err(SaveError::InvalidState)\n}\n\n#[cfg(test)]\nmod tests {\n    use std::time::Duration;\n\n    use galactic_domain::{\n        ReservationId, ResourceCost,\n        ResourceReservation, UniverseConfig,\n    };\n    use galactic_sim::GAME_STATE_VERSION;\n\n    use super::*;\n\n    #[test]\n    fn snapshot_round_trips_pending_production_window() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        simulation.advance(Duration::from_secs(2));\n\n        let save = snapshot_from_simulation(&simulation);\n        let restored = restore_from_snapshot(&save)\n            .expect("save is compatible");\n\n        assert_eq!(restored.state(), simulation.state());\n        assert_eq!(\n            save.state.colonies[0].production_pending_ticks,\n            20\n        );\n        assert_eq!(\n            save.catalog_fingerprint,\n            default_building_catalog().fingerprint()\n        );\n    }\n\n    #[test]\n    fn catalog_changes_are_detected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.catalog_fingerprint ^= 1;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::CatalogFingerprintMismatch {\n                ..\n            })\n        ));\n    }\n\n    #[test]\n    fn invalid_pending_window_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        save.state.colonies[0].production_pending_ticks =\n            PRODUCTION_REFRESH_TICKS as u16;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(\n                SaveError::InvalidPendingProductionTicks {\n                    ..\n                }\n            )\n        ));\n    }\n\n    #[test]\n    fn invalid_over_reserved_ledger_is_rejected() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut save =\n            snapshot_from_simulation(&simulation);\n        let colony = save\n            .state\n            .colonies\n            .first_mut()\n            .expect("home colony is saved");\n        colony.reservations.push(\n            ResourceReservation::new(\n                ReservationId::new(1),\n                ResourceCost::new(700, 0, 0),\n            ),\n        );\n        colony.next_reservation_id = 2;\n\n        assert!(matches!(\n            restore_from_snapshot(&save),\n            Err(SaveError::InvalidResourceLedger {\n                ..\n            })\n        ));\n    }\n\n    #[test]\n    fn state_and_save_versions_match_mvp_013() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let save = snapshot_from_simulation(&simulation);\n\n        assert_eq!(save.version, SAVE_VERSION);\n        assert_eq!(\n            save.state.version,\n            GAME_STATE_VERSION\n        );\n    }\n}\n'
COLONY_ECONOMY_TEXT = 'fn colony_economy_text(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    let stock = colony.resources.stock();\n    let available = colony.resources.available();\n    let reserved = colony.resources.reserved_total();\n    let production =\n        galactic_sim::colony_production_snapshot(colony);\n    let refresh = galactic_sim::StrategicDuration::from_ticks(\n        u64::from(production.ticks_until_refresh),\n    );\n\n    format!(\n        "STOCKS EXACTS\\nTotal — Métal {}  Cristal {}  Carburant {}\\nDisponible — Métal {}  Cristal {}  Carburant {}\\nRéservé — Métal {}  Cristal {}  Carburant {}\\nCapacité — Métal {}  Cristal {}  Carburant {}\\n\\nPRODUCTION ACTUELLE\\nMétal +{:.2}/s  Cristal +{:.2}/s  Carburant +{:.2}/s\\nCrédit des stocks : toutes les {} s stratégiques\\nProchaine actualisation : {}\\nSaturation — Métal {}  Cristal {}  Carburant {}\\n\\nÉNERGIE — CAPACITÉ\\nNominale : {}\\nEffective planète : {}\\nConsommation catalogue : {}\\nEfficacité extracteurs : {}%\\nBilan effectif : {:+}",\n        stock.metal,\n        stock.crystal,\n        stock.fuel,\n        available.metal,\n        available.crystal,\n        available.fuel,\n        reserved.metal,\n        reserved.crystal,\n        reserved.fuel,\n        production.capacity.metal,\n        production.capacity.crystal,\n        production.capacity.fuel,\n        production.effective_rate.metal_per_second(),\n        production.effective_rate.crystal_per_second(),\n        production.effective_rate.fuel_per_second(),\n        galactic_sim::PRODUCTION_REFRESH_SECONDS,\n        format_strategic_duration(refresh),\n        format_saturation_time(production.saturation.metal),\n        format_saturation_time(production.saturation.crystal),\n        format_saturation_time(production.saturation.fuel),\n        production.nominal_energy_production,\n        production.effective_energy_production,\n        production.energy_consumption,\n        u32::from(production.energy_efficiency_per_mille)\n            / 10,\n        i128::from(production.effective_energy_production)\n            - i128::from(production.energy_consumption),\n    )\n}\n'
SIM_TESTS = '\n    #[test]\n    fn production_events_are_emitted_only_every_five_seconds() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n\n        let early =\n            simulation.advance(Duration::from_secs(4));\n        assert!(\n            early.iter().all(|event| {\n                !matches!(\n                    event,\n                    GameEvent::ProductionRefreshed(_)\n                )\n            })\n        );\n\n        let refresh =\n            simulation.advance(Duration::from_secs(1));\n        assert!(\n            refresh.iter().any(|event| {\n                matches!(\n                    event,\n                    GameEvent::ProductionRefreshed(report)\n                        if report.ticks.ticks()\n                            == crate::PRODUCTION_REFRESH_TICKS\n                )\n            })\n        );\n    }\n\n    #[test]\n    fn five_second_windows_remain_frame_rate_independent() {\n        let mut fast =\n            Simulation::new(UniverseConfig::mvp());\n        let mut slow =\n            Simulation::new(UniverseConfig::mvp());\n\n        advance_in_equal_frames(\n            &mut fast,\n            1_000,\n            Duration::from_millis(10),\n        );\n        advance_in_equal_frames(\n            &mut slow,\n            10,\n            Duration::from_secs(1),\n        );\n\n        assert_eq!(fast.state(), slow.state());\n        assert_eq!(\n            fast.state()\n                .player_home_colony()\n                .expect("home colony exists")\n                .resources\n                .stock(),\n            ResourceStock::new(625, 312, 227)\n        );\n    }\n'
DOC_APPEND = "\n## MVP-013 — Catalogue de bâtiments et cadence de production\n\nLes huit bâtiments du scope MVP sont définis dans :\n\n```text\nassets/data/buildings.catalog\n```\n\nCe fichier contient pour chaque bâtiment :\n\n- nom ;\n- niveau maximal ;\n- coût de base ;\n- croissance du coût ;\n- durée de base ;\n- croissance de la durée ;\n- consommation énergétique par niveau ;\n- effet par niveau ;\n- prérequis.\n\nLe chargeur valide au démarrage :\n\n- présence exacte des huit bâtiments ;\n- unicité des définitions ;\n- niveaux maximaux ;\n- coûts et durées ;\n- prérequis existants et sans doublon ;\n- niveaux de prérequis valides ;\n- absence de dépendance sur soi-même ;\n- absence de cycle.\n\nLa simulation ne contient plus les constantes propres aux mines, à la centrale\nou à l'entrepôt. Production, capacité de stockage et bilan énergétique lisent\nle catalogue central. Modifier une valeur du fichier ne nécessite donc aucune\nmodification des systèmes de simulation.\n\nLe fichier possède une version et un fingerprint. Les sauvegardes refusent un\ncatalogue incompatible afin d'éviter qu'une partie change silencieusement de\nrègles économiques.\n\n### Cadence des ressources\n\nL'horloge reste à 10 ticks stratégiques par seconde, mais les stocks ne sont\ncrédités que toutes les 5 secondes stratégiques :\n\n```text\n50 ticks accumulés\n    ↓\nun crédit de production agrégé\n    ↓\nun événement ProductionRefreshed\n```\n\nLes ticks incomplets de la fenêtre sont sauvegardés par colonie. Les résultats\nrestent identiques quel que soit le framerate ou la vitesse de jeu.\n\nCette cadence concerne les stocks de ressources. Les futures constructions,\nrecherches et missions pourront conserver leur propre fréquence métier.\n\nVersions après migration :\n\n- `GAME_STATE_VERSION = 7` ;\n- `SAVE_VERSION = 8`.\n\nLe checkpoint `MVP-013-B` est réservé à une amélioration visuelle complète des\nressources avant la file de construction de MVP-014.\n"


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
                / "crates/galactic_sim/src/production.rs"
            ).exists()
            and (
                candidate
                / "crates/galactic_sim/src/starting.rs"
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
        "MVP-012 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    production = (
        root
        / "crates/galactic_sim/src/production.rs"
    ).read_text(encoding="utf-8")
    state = (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "pub const PRODUCTION_SCALE",
        "pub fn apply_colony_production",
        "pub fn storage_capacity",
    ):
        if marker not in production:
            failures.append(
                f"marqueur production absent : {marker}"
            )
    for marker in (
        "pub production_remainder: ProductionRemainder",
        "GAME_STATE_VERSION: u32 = 6",
    ):
        if marker not in state:
            failures.append(
                f"marqueur état absent : {marker}"
            )
    if "fn colony_economy_text(" not in client:
        failures.append(
            "inspecteur économique absent"
        )

    if failures:
        raise SystemExit(
            "Baseline MVP-012 incohérente :\n- "
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
    if "pub mod building_catalog;" not in source:
        source = replace_once(
            source,
            "pub mod command;\n",
            "pub mod building_catalog;\npub mod command;\n",
            "module building_catalog",
        )
    if "pub use building_catalog::*;" not in source:
        source = replace_once(
            source,
            "pub use command::*;\n",
            "pub use building_catalog::*;\npub use command::*;\n",
            "export building_catalog",
        )
    return normalize(source)


def patch_starting(source: str) -> str:
    if "InitialEnergyCatalogMismatch" in source:
        return normalize(source)

    source = source.replace(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n"
        "pub enum BuildingKind",
        "#[derive(\n"
        "    Debug, Clone, Copy, PartialEq, Eq, "
        "PartialOrd, Ord, Hash,\n"
        ")]\n"
        "pub enum BuildingKind",
        1,
    )

    validation = (
        "        if self.home_colony.initial_energy.is_deficit() {\n"
        "            return Err(StartingScenarioError::InitialEnergyDeficit);\n"
        "        }\n"
    )
    replacement = validation + (
        "        let expected_energy = "
        "crate::default_building_catalog()\n"
        "            .energy_grid_for_levels("
        "self.home_colony.buildings);\n"
        "        if self.home_colony.initial_energy "
        "!= expected_energy {\n"
        "            return Err(\n"
        "                StartingScenarioError::"
        "InitialEnergyCatalogMismatch {\n"
        "                    expected: expected_energy,\n"
        "                    found: "
        "self.home_colony.initial_energy,\n"
        "                },\n"
        "            );\n"
        "        }\n"
        "        crate::default_building_catalog()\n"
        "            .validate_levels("
        "self.home_colony.buildings)\n"
        "            .map_err(|_| "
        "StartingScenarioError::InvalidBuildingLevels)?;\n"
    )
    source = replace_once(
        source,
        validation,
        replacement,
        "validation du catalogue initial",
    )
    source = replace_once(
        source,
        "    InitialEnergyDeficit,\n",
        "    InitialEnergyDeficit,\n"
        "    InitialEnergyCatalogMismatch {\n"
        "        expected: EnergyGrid,\n"
        "        found: EnergyGrid,\n"
        "    },\n"
        "    InvalidBuildingLevels,\n",
        "erreurs du scénario catalogue",
    )
    source = replace_once(
        source,
        "        scenario.home_colony.initial_energy = "
        "EnergyGrid::new(120, 45);\n"
        "        scenario.home_colony.buildings.research_lab = 1;\n",
        "        scenario.home_colony.buildings.research_lab = 1;\n"
        "        scenario.home_colony.initial_energy =\n"
        "            crate::default_building_catalog()\n"
        "                .energy_grid_for_levels("
        "scenario.home_colony.buildings);\n",
        "test de scénario configurable",
    )
    return normalize(source)


def patch_state(source: str) -> str:
    if "pub production_pending_ticks: u16" in source:
        return normalize(source)

    source = source.replace(
        "// MVP-012: persistent knowledge, production and storage",
        "// MVP-013: persistent catalog-driven production windows",
        1,
    )
    source = source.replace(
        "/// Version 6 adds persisted fixed-point production remainders.\n"
        "pub const GAME_STATE_VERSION: u32 = 6;",
        "/// Version 7 adds five-second production windows.\n"
        "pub const GAME_STATE_VERSION: u32 = 7;",
        1,
    )
    source = replace_once(
        source,
        "                production_remainder: ProductionRemainder::ZERO,\n"
        "                buildings: home.buildings,\n",
        "                production_remainder: ProductionRemainder::ZERO,\n"
        "                production_pending_ticks: 0,\n"
        "                buildings: home.buildings,\n",
        "fenêtre initiale",
    )
    source = replace_once(
        source,
        "    pub production_remainder: ProductionRemainder,\n"
        "    pub buildings: BuildingLevels,\n",
        "    pub production_remainder: ProductionRemainder,\n"
        "    pub production_pending_ticks: u16,\n"
        "    pub buildings: BuildingLevels,\n",
        "champ ticks en attente",
    )
    return normalize(source)


def patch_simulation(source: str) -> str:
    fully_patched_markers = (
        "queue_colony_production",
        "default_building_catalog",
        "BuildingCatalogError",
        "InvalidProductionWindow",
        "ProductionRefreshed",
    )
    if all(marker in source for marker in fully_patched_markers):
        return normalize(source)

    source = source.replace(
        "// MVP-012: simulation commands, production and validation",
        "// MVP-013: catalog-driven simulation and production windows",
        1,
    )
    import_candidates = [
        (
            "    FactionKind, GAME_STATE_VERSION, GameCommand, GameEvent, "
            "GameState, KnowledgeLevel,\n"
            "    SelectionTarget, StartingScenario, StartingScenarioError, TimeSpeed, "
            "UniverseIndexError,\n"
            "    UniverseRepository, apply_colony_production, storage_capacity,\n"
        ),
        (
            "    FactionKind, GAME_STATE_VERSION, GameCommand, GameEvent, "
            "GameState, KnowledgeLevel,\n"
            "    TimeSpeed, UniverseIndexError, UniverseRepository, "
            "apply_colony_production,\n"
            "    storage_capacity,\n"
        ),
    ]
    import_replacement = (
        "    BuildingCatalogError, FactionKind, "
        "GAME_STATE_VERSION, GameCommand,\n"
        "    GameEvent, GameState, KnowledgeLevel, "
        "SelectionTarget, StartingScenario,\n"
        "    StartingScenarioError, TimeSpeed, "
        "UniverseIndexError, UniverseRepository,\n"
        "    default_building_catalog, "
        "queue_colony_production, storage_capacity,\n"
    )
    for old_import in import_candidates:
        count = source.count(old_import)
        if count == 1:
            source = source.replace(old_import, import_replacement, 1)
            break
        if count > 1:
            raise SystemExit(
                "Patch impossible pour imports simulation: "
                f"{count} occurrence(s), 1 attendue."
            )
    else:
        if "apply_colony_production" in source:
            raise SystemExit(
                "Patch impossible pour imports simulation: "
                "0 occurrence(s), 1 attendue."
            )
    source = replace_once(
        source,
        "    DuplicateColony(ColonyId),\n",
        "    DuplicateColony(ColonyId),\n"
        "    InvalidColonyBuildings {\n"
        "        colony_id: ColonyId,\n"
        "        error: BuildingCatalogError,\n"
        "    },\n"
        "    InvalidProductionWindow {\n"
        "        colony_id: ColonyId,\n"
        "        pending_ticks: u16,\n"
        "    },\n",
        "erreurs catalogue simulation",
    )

    old_loop_candidates = [
        (
            "        for colony in &mut self.state.colonies {\n"
            "            apply_colony_production(\n"
            "                colony,\n"
            "                advance.ticks,\n"
            "            );\n"
            "        }\n\n"
            "        vec![GameEvent::TicksAdvanced {\n"
            "            ticks: advance.ticks,\n"
            "            current_tick: advance.current_tick,\n"
            "        }]\n"
        ),
        (
            "        for colony in &mut self.state.colonies {\n"
            "            apply_colony_production(colony, advance.ticks);\n"
            "        }\n\n"
            "        vec![GameEvent::TicksAdvanced {\n"
            "            ticks: advance.ticks,\n"
            "            current_tick: advance.current_tick,\n"
            "        }]\n"
        ),
    ]
    new_loop = (
        "        let mut events = vec![GameEvent::TicksAdvanced {\n"
        "            ticks: advance.ticks,\n"
        "            current_tick: advance.current_tick,\n"
        "        }];\n"
        "        for colony in &mut self.state.colonies {\n"
        "            if let Some(report) = "
        "queue_colony_production(\n"
        "                colony,\n"
        "                advance.ticks,\n"
        "            ) {\n"
        "                events.push(\n"
        "                    GameEvent::ProductionRefreshed(report),\n"
        "                );\n"
        "            }\n"
        "        }\n"
        "        events\n"
    )
    for old_loop in old_loop_candidates:
        count = source.count(old_loop)
        if count == 1:
            source = source.replace(old_loop, new_loop, 1)
            break
        if count > 1:
            raise SystemExit(
                "Patch impossible pour cadence de production: "
                f"{count} occurrence(s), 1 attendue."
            )
    else:
        raise SystemExit(
            "Patch impossible pour cadence de production: "
            "0 occurrence(s), 1 attendue."
        )

    marker = (
        "        if let Err(error) = colony.resources.validate() {\n"
    )
    insert = (
        "        if let Err(error) = "
        "default_building_catalog()\n"
        "            .validate_levels(colony.buildings)\n"
        "        {\n"
        "            return Err(\n"
        "                SimulationBuildError::"
        "InvalidColonyBuildings {\n"
        "                    colony_id: colony.id,\n"
        "                    error,\n"
        "                },\n"
        "            );\n"
        "        }\n"
        "        if u64::from(colony.production_pending_ticks)\n"
        "            >= crate::PRODUCTION_REFRESH_TICKS\n"
        "        {\n"
        "            return Err(\n"
        "                SimulationBuildError::"
        "InvalidProductionWindow {\n"
        "                    colony_id: colony.id,\n"
        "                    pending_ticks: "
        "colony.production_pending_ticks,\n"
        "                },\n"
        "            );\n"
        "        }\n"
    )
    if marker not in source:
        raise SystemExit(
            "Validation économique introuvable."
        )
    source = source.replace(
        marker,
        insert + marker,
        1,
    )

    test_marker = (
        "    #[test]\n"
        "    fn selection_events_use_domain_ids()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests MVP-013 introuvable."
        )
    source = source.replace(
        test_marker,
        SIM_TESTS.rstrip() + "\n\n" + test_marker,
        1,
    )
    return normalize(source)


def patch_client(source: str) -> str:
    if "Prochaine actualisation" not in source:
        pattern = re.compile(
            r"fn colony_economy_text\(.*?\n\}\n\n"
            r"(?=fn format_saturation_time)",
            flags=re.DOTALL,
        )
        source, count = pattern.subn(
            COLONY_ECONOMY_TEXT.rstrip() + "\n\n",
            source,
            count=1,
        )
        if count != 1:
            raise SystemExit(
                "Fonction colony_economy_text introuvable."
            )

    old = (
        "        GameEvent::TicksAdvanced {\n"
        "            ticks,\n"
        "            current_tick,\n"
        "        } => format!(\"+{} ticks -> {}\", "
        "ticks.ticks(), current_tick),\n"
    )
    new = old + (
        "        GameEvent::ProductionRefreshed(report) => "
        "format!(\n"
        "            \"ressources +{}/{}/{} sur {} ticks\",\n"
        "            report.produced.metal,\n"
        "            report.produced.crystal,\n"
        "            report.produced.fuel,\n"
        "            report.ticks.ticks(),\n"
        "        ),\n"
    )
    source = replace_once(
        source,
        old,
        new,
        "label événement production",
    )
    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-013 — Catalogue de bâtiments et cadence de production" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "assets/data/buildings.catalog":
            normalize(CATALOG_DATA),
        root / "crates/galactic_sim/src/building_catalog.rs":
            BUILDING_CATALOG_RS,
        root / "crates/galactic_sim/src/production.rs":
            PRODUCTION_RS,
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
        after = (
            format_rust(root, content)
            if path.suffix == ".rs"
            else normalize(content)
        )
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
        "assets/data/buildings.catalog": (
            "building=metal_mine",
            "building=shipyard",
        ),
        "crates/galactic_sim/src/building_catalog.rs": (
            "pub struct BuildingCatalog",
            "PrerequisiteCycle",
        ),
        "crates/galactic_sim/src/production.rs": (
            "PRODUCTION_REFRESH_TICKS",
            "queue_colony_production",
        ),
        "crates/galactic_sim/src/state.rs": (
            "GAME_STATE_VERSION: u32 = 7",
            "production_pending_ticks",
        ),
        "crates/galactic_persistence/src/lib.rs": (
            "SAVE_VERSION: u32 = 8",
            "catalog_fingerprint",
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

    if failures:
        raise SystemExit(
            "Migration MVP-013 incomplète :\n- "
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
        print("MVP-013 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp013-backup"
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
        "\nMVP-013 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

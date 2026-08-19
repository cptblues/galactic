use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use galactic_domain::{PlanetId, PlanetKind};
use galactic_sim::{
    BuildingKind, CombatStackView, CombatTargetClass, CraftableId, PlanetaryForceId,
};
use serde::Deserialize;

const VISUAL_MANIFEST_VERSION: u32 = 1;
const DEFAULT_VISUAL_MANIFEST: &str = "assets/visuals/manifest.ron";

#[derive(Debug, Clone, Deserialize)]
struct VisualManifestConfig {
    version: u32,
    fallback: VisualFallbackConfig,
    ships: Vec<VisualEntry<CraftableId>>,
    buildings: Vec<VisualEntry<BuildingKind>>,
    forces: Vec<VisualEntry<PlanetaryForceId>>,
    planets: Vec<PlanetVisualEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct VisualFallbackConfig {
    unknown_entity: String,
    unknown_planet: String,
    contact_unknown: String,
    contact_light: String,
    contact_medium: String,
    contact_heavy: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VisualEntry<T> {
    id: T,
    image: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PlanetVisualEntry {
    kind: PlanetKind,
    variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VisualManifestError {
    Read(String),
    Parse(String),
    UnsupportedVersion(u32),
    EmptyPath,
    DuplicateShip(CraftableId),
    DuplicateBuilding(BuildingKind),
    DuplicateForce(PlanetaryForceId),
    DuplicatePlanet(PlanetKind),
    EmptyPlanetVariants(PlanetKind),
}

impl fmt::Display for VisualManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read visual manifest: {error}"),
            Self::Parse(error) => write!(f, "cannot parse visual manifest: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported visual manifest version {version}")
            }
            Self::EmptyPath => write!(f, "visual manifest contains an empty path"),
            Self::DuplicateShip(id) => write!(f, "duplicate ship visual entry {id:?}"),
            Self::DuplicateBuilding(id) => write!(f, "duplicate building visual entry {id:?}"),
            Self::DuplicateForce(id) => write!(f, "duplicate force visual entry {id:?}"),
            Self::DuplicatePlanet(kind) => write!(f, "duplicate planet visual entry {kind:?}"),
            Self::EmptyPlanetVariants(kind) => {
                write!(f, "planet visual entry {kind:?} has no variants")
            }
        }
    }
}

#[derive(Debug, Clone)]
struct VisualManifest {
    fallback: VisualFallbackConfig,
    ships: HashMap<CraftableId, String>,
    buildings: HashMap<BuildingKind, String>,
    forces: HashMap<PlanetaryForceId, String>,
    planets: HashMap<PlanetKind, Vec<String>>,
}

impl VisualManifest {
    fn load_from_path(path: &Path) -> Result<Self, VisualManifestError> {
        let source = fs::read_to_string(path)
            .map_err(|error| VisualManifestError::Read(format!("{} ({error})", path.display())))?;
        let config: VisualManifestConfig = ron::de::from_str(&source)
            .map_err(|error| VisualManifestError::Parse(error.to_string()))?;
        Self::from_config(config)
    }

    fn from_config(config: VisualManifestConfig) -> Result<Self, VisualManifestError> {
        if config.version != VISUAL_MANIFEST_VERSION {
            return Err(VisualManifestError::UnsupportedVersion(config.version));
        }
        validate_path(&config.fallback.unknown_entity)?;
        validate_path(&config.fallback.unknown_planet)?;
        validate_path(&config.fallback.contact_unknown)?;
        validate_path(&config.fallback.contact_light)?;
        validate_path(&config.fallback.contact_medium)?;
        validate_path(&config.fallback.contact_heavy)?;

        let mut ships = HashMap::new();
        for entry in config.ships {
            validate_path(&entry.image)?;
            if ships.insert(entry.id, entry.image).is_some() {
                return Err(VisualManifestError::DuplicateShip(entry.id));
            }
        }

        let mut buildings = HashMap::new();
        for entry in config.buildings {
            validate_path(&entry.image)?;
            if buildings.insert(entry.id, entry.image).is_some() {
                return Err(VisualManifestError::DuplicateBuilding(entry.id));
            }
        }

        let mut forces = HashMap::new();
        for entry in config.forces {
            validate_path(&entry.image)?;
            if forces.insert(entry.id, entry.image).is_some() {
                return Err(VisualManifestError::DuplicateForce(entry.id));
            }
        }

        let mut planets = HashMap::new();
        for entry in config.planets {
            if entry.variants.is_empty() {
                return Err(VisualManifestError::EmptyPlanetVariants(entry.kind));
            }
            for path in &entry.variants {
                validate_path(path)?;
            }
            if planets.insert(entry.kind, entry.variants).is_some() {
                return Err(VisualManifestError::DuplicatePlanet(entry.kind));
            }
        }

        Ok(Self {
            fallback: config.fallback,
            ships,
            buildings,
            forces,
            planets,
        })
    }
}

fn validate_path(path: &str) -> Result<(), VisualManifestError> {
    if path.trim().is_empty() {
        Err(VisualManifestError::EmptyPath)
    } else {
        Ok(())
    }
}

fn default_visual_manifest_path() -> PathBuf {
    if let Ok(current) = env::current_dir() {
        for ancestor in current.ancestors() {
            let candidate = ancestor.join(DEFAULT_VISUAL_MANIFEST);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(DEFAULT_VISUAL_MANIFEST)
}

#[derive(Resource)]
#[allow(dead_code)]
pub(crate) struct EntityVisualCatalog {
    fallback_entity: Handle<Image>,
    fallback_planet: Handle<Image>,
    contact_unknown: Handle<Image>,
    contact_light: Handle<Image>,
    contact_medium: Handle<Image>,
    contact_heavy: Handle<Image>,
    ships: HashMap<CraftableId, Handle<Image>>,
    buildings: HashMap<BuildingKind, Handle<Image>>,
    forces: HashMap<PlanetaryForceId, Handle<Image>>,
    planets: HashMap<PlanetKind, Vec<Handle<Image>>>,
}

#[cfg(test)]
pub(crate) struct EntityVisualTestHandles {
    pub(crate) contact_medium: Handle<Image>,
    pub(crate) force: Handle<Image>,
}

impl EntityVisualCatalog {
    #[cfg(test)]
    pub(crate) fn for_tests(images: &mut Assets<Image>) -> Self {
        let image = images.add(Image::default());
        Self {
            fallback_entity: image.clone(),
            fallback_planet: image.clone(),
            contact_unknown: image.clone(),
            contact_light: image.clone(),
            contact_medium: image.clone(),
            contact_heavy: image.clone(),
            ships: HashMap::new(),
            buildings: HashMap::new(),
            forces: HashMap::new(),
            planets: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests_with_force(
        images: &mut Assets<Image>,
        force_id: PlanetaryForceId,
    ) -> (Self, EntityVisualTestHandles) {
        let fallback_entity = images.add(Image::default());
        let fallback_planet = images.add(Image::default());
        let contact_unknown = images.add(Image::default());
        let contact_light = images.add(Image::default());
        let contact_medium = images.add(Image::default());
        let contact_heavy = images.add(Image::default());
        let force = images.add(Image::default());
        let mut forces = HashMap::new();
        forces.insert(force_id, force.clone());
        (
            Self {
                fallback_entity,
                fallback_planet,
                contact_unknown,
                contact_light: contact_light.clone(),
                contact_medium: contact_medium.clone(),
                contact_heavy: contact_heavy.clone(),
                ships: HashMap::new(),
                buildings: HashMap::new(),
                forces,
                planets: HashMap::new(),
            },
            EntityVisualTestHandles {
                contact_medium,
                force,
            },
        )
    }

    pub(crate) fn ship(&self, id: CraftableId) -> Handle<Image> {
        self.ships
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.fallback_entity.clone())
    }

    #[allow(dead_code)]
    pub(crate) fn building(&self, id: BuildingKind) -> Handle<Image> {
        self.buildings
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.fallback_entity.clone())
    }

    pub(crate) fn force(&self, id: PlanetaryForceId) -> Handle<Image> {
        self.forces
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.fallback_entity.clone())
    }

    pub(crate) fn enemy_contact(&self, stack: &CombatStackView) -> Handle<Image> {
        if let Some(galactic_sim::CombatUnitRef::PlanetaryForce(id)) = stack.identity {
            return self.force(id);
        }
        match stack.target_class {
            None => self.contact_unknown.clone(),
            Some(CombatTargetClass::Light) => self.contact_light.clone(),
            Some(CombatTargetClass::Medium) => self.contact_medium.clone(),
            Some(CombatTargetClass::Heavy) => self.contact_heavy.clone(),
        }
    }

    pub(crate) fn planet(&self, planet_id: PlanetId, kind: PlanetKind) -> Handle<Image> {
        let Some(variants) = self.planets.get(&kind) else {
            return self.fallback_planet.clone();
        };
        let index = planet_visual_variant_index(planet_id, kind, variants.len());
        variants[index].clone()
    }
}

impl FromWorld for EntityVisualCatalog {
    fn from_world(world: &mut World) -> Self {
        let manifest_path = default_visual_manifest_path();
        let manifest = match VisualManifest::load_from_path(&manifest_path) {
            Ok(manifest) => manifest,
            Err(error) => {
                warn!("{error}; using generated visual fallbacks only");
                fallback_manifest()
            }
        };
        let asset_server = world.resource::<AssetServer>().clone();
        let load = |path: &str| asset_server.load(path.to_string());

        Self {
            fallback_entity: load(&manifest.fallback.unknown_entity),
            fallback_planet: load(&manifest.fallback.unknown_planet),
            contact_unknown: load(&manifest.fallback.contact_unknown),
            contact_light: load(&manifest.fallback.contact_light),
            contact_medium: load(&manifest.fallback.contact_medium),
            contact_heavy: load(&manifest.fallback.contact_heavy),
            ships: manifest
                .ships
                .iter()
                .map(|(&id, path)| (id, load(path)))
                .collect(),
            buildings: manifest
                .buildings
                .iter()
                .map(|(&id, path)| (id, load(path)))
                .collect(),
            forces: manifest
                .forces
                .iter()
                .map(|(&id, path)| (id, load(path)))
                .collect(),
            planets: manifest
                .planets
                .iter()
                .map(|(&kind, paths)| (kind, paths.iter().map(|path| load(path)).collect()))
                .collect(),
        }
    }
}

fn fallback_manifest() -> VisualManifest {
    let fallback = VisualFallbackConfig {
        unknown_entity: "visuals/fallback/unknown_entity.png".to_string(),
        unknown_planet: "visuals/fallback/unknown_planet.png".to_string(),
        contact_unknown: "visuals/fallback/contact_unknown.png".to_string(),
        contact_light: "visuals/fallback/contact_light.png".to_string(),
        contact_medium: "visuals/fallback/contact_medium.png".to_string(),
        contact_heavy: "visuals/fallback/contact_heavy.png".to_string(),
    };
    VisualManifest {
        fallback,
        ships: HashMap::new(),
        buildings: HashMap::new(),
        forces: HashMap::new(),
        planets: HashMap::new(),
    }
}

#[allow(dead_code)]
pub(crate) fn planet_visual_variant_index(
    planet_id: PlanetId,
    kind: PlanetKind,
    variant_count: usize,
) -> usize {
    if variant_count == 0 {
        return 0;
    }
    let kind_tag: u64 = match kind {
        PlanetKind::Rocky => 11,
        PlanetKind::Ocean => 17,
        PlanetKind::Desert => 23,
        PlanetKind::Ice => 31,
        PlanetKind::GasGiant => 43,
        PlanetKind::Volcanic => 53,
    };
    let mixed = planet_id.raw() ^ kind_tag.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (mixed as usize) % variant_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use galactic_sim::default_ruleset;

    fn manifest() -> VisualManifest {
        VisualManifest::load_from_path(&default_visual_manifest_path())
            .expect("default visual manifest loads")
    }

    fn asset_path_exists(path: &str) -> bool {
        default_visual_manifest_path()
            .parent()
            .and_then(Path::parent)
            .map(|asset_root| asset_root.join(path).is_file())
            .unwrap_or(false)
    }

    #[test]
    fn every_default_craftable_has_visual() {
        let manifest = manifest();
        for craftable in default_ruleset().craftables().definitions() {
            let Some(path) = manifest.ships.get(&craftable.id) else {
                panic!("missing visual for craftable {:?}", craftable.id);
            };
            assert!(asset_path_exists(path), "missing asset file {path}");
        }
    }

    #[test]
    fn every_default_building_has_visual() {
        let manifest = manifest();
        for building in default_ruleset().buildings().definitions() {
            let Some(path) = manifest.buildings.get(&building.kind) else {
                panic!("missing visual for building {:?}", building.kind);
            };
            assert!(asset_path_exists(path), "missing asset file {path}");
        }
    }

    #[test]
    fn every_default_planetary_force_has_visual() {
        let manifest = manifest();
        for force in default_ruleset().planetary_presence().definitions() {
            let Some(path) = manifest.forces.get(&force.id) else {
                panic!("missing visual for force {:?}", force.id);
            };
            assert!(asset_path_exists(path), "missing asset file {path}");
        }
    }

    #[test]
    fn every_planet_kind_has_preview_variants() {
        let manifest = manifest();
        for kind in PlanetKind::ALL {
            let Some(paths) = manifest.planets.get(&kind) else {
                panic!("missing planet variants for {kind:?}");
            };
            assert!(paths.len() >= 3);
            for path in paths {
                assert!(asset_path_exists(path), "missing asset file {path}");
            }
        }
    }

    #[test]
    fn planet_variant_is_deterministic() {
        let planet_id = PlanetId::from_system_index(galactic_domain::SystemId::new(12), 3);
        let first = planet_visual_variant_index(planet_id, PlanetKind::Ocean, 3);
        let second = planet_visual_variant_index(planet_id, PlanetKind::Ocean, 3);
        assert_eq!(first, second);
    }
}

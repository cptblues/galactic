// MVP-016-B: external, versioned and validated gameplay ruleset.
use std::collections::BTreeSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use galactic_domain::{ColonyId, FactionId, Owner, PlanetId, SystemId};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{
    BuildingCatalog, BuildingCatalogConfig, BuildingCatalogError, BuildingLevels, CombatRules,
    CombatRulesConfig, CombatRulesError, CraftCatalogError, CraftableCatalog,
    CraftableCatalogConfig, DiplomacyState, DiplomaticRelation, ExtractionRules,
    ExtractionRulesConfig, ExtractionRulesError, FactionKind, FactionRelation,
    InitialPlanetKnowledge, InitialSystemKnowledge, KnowledgeLevel, PlanetResourceProfile,
    PlanetaryAnalysisRules, PlanetaryAnalysisRulesConfig, PlanetaryAnalysisRulesError,
    PlanetaryPresenceRules, PlanetaryPresenceRulesConfig, PlanetaryPresenceRulesError,
    ResourceValuesConfig, StartingColonyConfig, StartingFactionConfig, StartingScenario,
    TechnologyCatalog, TechnologyCatalogConfig, TechnologyCatalogError,
};

pub const RULESET_SCHEMA_VERSION: u32 = 12;
pub const RULESET_DIRECTORY_ENV: &str = "GALACTIC_RULESET_DIR";
pub const DEFAULT_RULESET_DIRECTORY: &str = "assets/rulesets/default";

static DEFAULT_RULESET: OnceLock<Ruleset> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EconomyRules {
    pub construction_queue_limit: usize,
    pub research_queue_limit: usize,
    pub craft_queue_limit: usize,
    pub production_refresh_seconds: u64,
}

#[derive(Debug)]
pub struct Ruleset {
    id: String,
    schema_version: u32,
    content_version: u32,
    structure_fingerprint: u64,
    economy: EconomyRules,
    buildings: BuildingCatalog,
    technologies: TechnologyCatalog,
    craftables: CraftableCatalog,
    extraction: ExtractionRules,
    planetary_analysis: PlanetaryAnalysisRules,
    planetary_presence: PlanetaryPresenceRules,
    combat: CombatRules,
    starting_scenario: StartingScenario,
}

impl Ruleset {
    pub fn load_from_dir(directory: &Path) -> Result<Self, RulesetLoadError> {
        let manifest: ManifestConfig = read_ron(directory, "manifest.ron")?;
        if manifest.schema_version != RULESET_SCHEMA_VERSION {
            return Err(RulesetLoadError::UnsupportedSchemaVersion {
                expected: RULESET_SCHEMA_VERSION,
                found: manifest.schema_version,
            });
        }
        validate_identifier(&manifest.id)
            .map_err(|()| RulesetLoadError::InvalidRulesetId(manifest.id.clone()))?;
        if manifest.content_version == 0 {
            return Err(RulesetLoadError::InvalidContentVersion);
        }

        let economy_config: EconomyConfig = read_ron(directory, "economy.ron")?;
        let economy = economy_config.compile()?;
        let faction_config: FactionCatalogConfig = read_ron(directory, "factions.ron")?;
        let faction_catalog = faction_config.compile()?;
        let building_config: BuildingCatalogConfig = read_ron(directory, "buildings.ron")?;
        let buildings =
            BuildingCatalog::from_config(building_config, economy_config.base_storage.into_stock())
                .map_err(RulesetLoadError::Buildings)?;
        let technology_config: TechnologyCatalogConfig = read_ron(directory, "technologies.ron")?;
        let technologies = TechnologyCatalog::from_config(technology_config, &buildings)
            .map_err(RulesetLoadError::Technologies)?;
        let craftable_config: CraftableCatalogConfig = read_ron(directory, "craftables.ron")?;
        let craftables = CraftableCatalog::from_config(craftable_config, &buildings, &technologies)
            .map_err(RulesetLoadError::Craftables)?;
        let extraction_config: ExtractionRulesConfig = read_ron(directory, "extraction.ron")?;
        let extraction = ExtractionRules::from_config(extraction_config)
            .map_err(RulesetLoadError::Extraction)?;
        let planetary_analysis_config: PlanetaryAnalysisRulesConfig =
            read_ron(directory, "planetary_analysis.ron")?;
        let planetary_analysis =
            PlanetaryAnalysisRules::from_config(planetary_analysis_config, &craftables, &buildings)
                .map_err(RulesetLoadError::PlanetaryAnalysis)?;
        let planetary_presence_config: PlanetaryPresenceRulesConfig =
            read_ron(directory, "planetary_presence.ron")?;
        let planetary_presence = PlanetaryPresenceRules::from_config(
            planetary_presence_config,
            faction_catalog.factions,
        )
        .map_err(RulesetLoadError::PlanetaryPresence)?;
        let combat_config: CombatRulesConfig = read_ron(directory, "combat.ron")?;
        let combat = CombatRules::from_config(combat_config, &craftables, &planetary_presence)
            .map_err(RulesetLoadError::Combat)?;
        let starting_config: StartingScenarioConfig = read_ron(directory, "starting_scenario.ron")?;
        let starting_scenario = starting_config.compile(
            &buildings,
            &technologies,
            faction_catalog.factions,
            faction_catalog.default_relation,
            faction_catalog.relations,
        )?;

        let mut structure = format!(
            "ruleset:{};schema:{};",
            manifest.id, manifest.schema_version,
        );
        append_faction_structure(faction_catalog.factions, &mut structure);
        append_diplomacy_structure(
            faction_catalog.default_relation,
            faction_catalog.relations,
            &mut structure,
        );
        buildings.append_structure(&mut structure);
        technologies.append_structure(&mut structure);
        craftables.append_structure(&mut structure);
        extraction.append_structure(&mut structure);
        planetary_analysis.append_structure(&mut structure);
        planetary_presence.append_structure(&mut structure);
        combat.append_structure(&mut structure);

        Ok(Self {
            id: manifest.id,
            schema_version: manifest.schema_version,
            content_version: manifest.content_version,
            structure_fingerprint: fnv1a64(structure.as_bytes()),
            economy,
            buildings,
            technologies,
            craftables,
            extraction,
            planetary_analysis,
            planetary_presence,
            combat,
            starting_scenario,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn content_version(&self) -> u32 {
        self.content_version
    }

    pub const fn structure_fingerprint(&self) -> u64 {
        self.structure_fingerprint
    }

    pub const fn economy(&self) -> EconomyRules {
        self.economy
    }

    pub const fn buildings(&self) -> &BuildingCatalog {
        &self.buildings
    }

    pub const fn technologies(&self) -> &TechnologyCatalog {
        &self.technologies
    }

    pub const fn craftables(&self) -> &CraftableCatalog {
        &self.craftables
    }

    pub const fn extraction(&self) -> &ExtractionRules {
        &self.extraction
    }

    pub const fn planetary_analysis(&self) -> &PlanetaryAnalysisRules {
        &self.planetary_analysis
    }

    pub const fn planetary_presence(&self) -> &PlanetaryPresenceRules {
        &self.planetary_presence
    }

    pub const fn combat(&self) -> &CombatRules {
        &self.combat
    }

    pub const fn starting_scenario(&self) -> StartingScenario {
        self.starting_scenario
    }

    pub const fn factions(&self) -> &'static [StartingFactionConfig] {
        self.starting_scenario.factions
    }
}

pub fn default_ruleset() -> &'static Ruleset {
    DEFAULT_RULESET.get_or_init(|| {
        let directory = default_ruleset_directory();
        Ruleset::load_from_dir(&directory).unwrap_or_else(|error| {
            panic!(
                "failed to load Galactic ruleset from {}: {error}",
                directory.display(),
            )
        })
    })
}

pub fn default_ruleset_directory() -> PathBuf {
    if let Some(configured) = env::var_os(RULESET_DIRECTORY_ENV) {
        return PathBuf::from(configured);
    }

    if let Ok(current) = env::current_dir() {
        for ancestor in current.ancestors() {
            let candidate = ancestor.join(DEFAULT_RULESET_DIRECTORY);
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    if let Ok(executable) = env::current_exe() {
        for ancestor in executable.ancestors().skip(1) {
            let candidate = ancestor.join(DEFAULT_RULESET_DIRECTORY);
            if candidate.is_dir() {
                return candidate;
            }
        }
    }

    PathBuf::from(DEFAULT_RULESET_DIRECTORY)
}

#[derive(Debug)]
pub enum RulesetLoadError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    InvalidRulesetId(String),
    UnsupportedSchemaVersion {
        expected: u32,
        found: u32,
    },
    InvalidContentVersion,
    InvalidEconomy(&'static str),
    InvalidFactions(&'static str),
    Buildings(BuildingCatalogError),
    Technologies(TechnologyCatalogError),
    Craftables(CraftCatalogError),
    Extraction(ExtractionRulesError),
    PlanetaryAnalysis(PlanetaryAnalysisRulesError),
    PlanetaryPresence(PlanetaryPresenceRulesError),
    Combat(CombatRulesError),
    StartingScenario(&'static str),
}

impl fmt::Display for RulesetLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "cannot read {}: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(formatter, "invalid RON in {}: {message}", path.display())
            }
            Self::InvalidRulesetId(id) => write!(formatter, "invalid ruleset id `{id}`"),
            Self::UnsupportedSchemaVersion { expected, found } => write!(
                formatter,
                "ruleset schema version {found} is unsupported (expected {expected})",
            ),
            Self::InvalidContentVersion => {
                formatter.write_str("ruleset content_version must be greater than zero")
            }
            Self::InvalidEconomy(message) => write!(formatter, "invalid economy: {message}"),
            Self::InvalidFactions(message) => write!(formatter, "invalid factions: {message}"),
            Self::Buildings(error) => write!(formatter, "invalid buildings catalog: {error:?}"),
            Self::Technologies(error) => {
                write!(formatter, "invalid technologies catalog: {error:?}")
            }
            Self::Craftables(error) => write!(formatter, "invalid craftables catalog: {error:?}"),
            Self::Extraction(error) => {
                write!(formatter, "invalid extraction rules: {error:?}")
            }
            Self::PlanetaryAnalysis(error) => {
                write!(formatter, "invalid planetary analysis rules: {error:?}")
            }
            Self::PlanetaryPresence(error) => {
                write!(formatter, "invalid planetary presence rules: {error:?}")
            }
            Self::Combat(error) => write!(formatter, "invalid combat rules: {error:?}"),
            Self::StartingScenario(message) => {
                write!(formatter, "invalid starting scenario: {message}")
            }
        }
    }
}

impl std::error::Error for RulesetLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ManifestConfig {
    id: String,
    schema_version: u32,
    content_version: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct EconomyConfig {
    base_storage: ResourceValuesConfig,
    construction_queue_limit: usize,
    research_queue_limit: usize,
    craft_queue_limit: usize,
    production_refresh_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct FactionCatalogConfig {
    version: u32,
    default_relation: DiplomaticRelationConfig,
    factions: Vec<FactionConfig>,
    relations: Vec<FactionRelationConfig>,
}

impl FactionCatalogConfig {
    fn compile(self) -> Result<CompiledFactionCatalog, RulesetLoadError> {
        if self.version != 2 {
            return Err(RulesetLoadError::InvalidFactions(
                "factions.ron version must be 2",
            ));
        }
        if self.factions.is_empty() {
            return Err(RulesetLoadError::InvalidFactions(
                "at least one faction is required",
            ));
        }

        let mut ids = BTreeSet::new();
        let mut compiled = Vec::with_capacity(self.factions.len());
        for faction in self.factions {
            if !ids.insert(faction.id) {
                return Err(RulesetLoadError::InvalidFactions(
                    "a faction id is duplicated",
                ));
            }
            if faction.name.trim().is_empty() {
                return Err(RulesetLoadError::InvalidFactions(
                    "faction names must not be empty",
                ));
            }
            compiled.push(StartingFactionConfig {
                id: FactionId::new(faction.id),
                name: Box::leak(faction.name.into_boxed_str()),
                kind: faction.kind.into(),
                active: faction.active,
            });
        }
        compiled.sort_by_key(|faction| faction.id);
        let factions = Box::leak(compiled.into_boxed_slice());
        let default_relation = self.default_relation.into();
        let mut relations = Vec::with_capacity(self.relations.len());
        for configured in self.relations {
            let first = FactionId::new(configured.first);
            let second = FactionId::new(configured.second);
            if !ids.contains(&configured.first) || !ids.contains(&configured.second) {
                return Err(RulesetLoadError::InvalidFactions(
                    "a relation references an unknown faction",
                ));
            }
            let relation = FactionRelation::new(first, second, configured.relation.into())
                .map_err(|_| RulesetLoadError::InvalidFactions("self-relations are implicit"))?;
            relations.push(relation);
        }
        let diplomacy = DiplomacyState::new(default_relation, relations).map_err(|_| {
            RulesetLoadError::InvalidFactions("relations must be unique and useful")
        })?;
        let relations = Box::leak(diplomacy.relations().to_vec().into_boxed_slice());
        Ok(CompiledFactionCatalog {
            factions,
            default_relation,
            relations,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct CompiledFactionCatalog {
    factions: &'static [StartingFactionConfig],
    default_relation: DiplomaticRelation,
    relations: &'static [FactionRelation],
}

#[derive(Debug, Deserialize)]
struct FactionConfig {
    id: u64,
    name: String,
    kind: FactionKindConfig,
    active: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum FactionKindConfig {
    Player,
    Neutral,
    FutureAi,
}

#[derive(Debug, Deserialize)]
struct FactionRelationConfig {
    first: u64,
    second: u64,
    relation: DiplomaticRelationConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum DiplomaticRelationConfig {
    Unknown,
    Neutral,
    Hostile,
    Allied,
}

impl From<DiplomaticRelationConfig> for DiplomaticRelation {
    fn from(relation: DiplomaticRelationConfig) -> Self {
        match relation {
            DiplomaticRelationConfig::Unknown => Self::Unknown,
            DiplomaticRelationConfig::Neutral => Self::Neutral,
            DiplomaticRelationConfig::Hostile => Self::Hostile,
            DiplomaticRelationConfig::Allied => Self::Allied,
        }
    }
}

impl From<FactionKindConfig> for FactionKind {
    fn from(kind: FactionKindConfig) -> Self {
        match kind {
            FactionKindConfig::Player => Self::Player,
            FactionKindConfig::Neutral => Self::Neutral,
            FactionKindConfig::FutureAi => Self::FutureAi,
        }
    }
}

impl EconomyConfig {
    fn compile(self) -> Result<EconomyRules, RulesetLoadError> {
        if self.base_storage.metal == 0
            || self.base_storage.crystal == 0
            || self.base_storage.fuel == 0
        {
            return Err(RulesetLoadError::InvalidEconomy(
                "base storage values must be greater than zero",
            ));
        }
        if self.construction_queue_limit == 0 {
            return Err(RulesetLoadError::InvalidEconomy(
                "construction_queue_limit must be greater than zero",
            ));
        }
        if self.research_queue_limit == 0 {
            return Err(RulesetLoadError::InvalidEconomy(
                "research_queue_limit must be greater than zero",
            ));
        }
        if self.craft_queue_limit == 0 {
            return Err(RulesetLoadError::InvalidEconomy(
                "craft_queue_limit must be greater than zero",
            ));
        }
        if self.production_refresh_seconds == 0
            || self.production_refresh_seconds > u64::from(u16::MAX) / 10
        {
            return Err(RulesetLoadError::InvalidEconomy(
                "production_refresh_seconds is outside the supported range",
            ));
        }
        Ok(EconomyRules {
            construction_queue_limit: self.construction_queue_limit,
            research_queue_limit: self.research_queue_limit,
            craft_queue_limit: self.craft_queue_limit,
            production_refresh_seconds: self.production_refresh_seconds,
        })
    }
}

#[derive(Debug, Deserialize)]
struct StartingScenarioConfig {
    player_faction_id: u64,
    colony_id: u64,
    colony_name: String,
    home_system_index: u32,
    home_planet_index: u32,
    initial_stock: ResourceValuesConfig,
    buildings: Vec<StartingBuildingConfig>,
    initial_technologies: Vec<String>,
    resource_profile: ResourceProfileConfig,
    minimum_home_habitability: u8,
}

impl StartingScenarioConfig {
    fn compile(
        self,
        catalog: &BuildingCatalog,
        technologies: &TechnologyCatalog,
        factions: &'static [StartingFactionConfig],
        default_relation: DiplomaticRelation,
        initial_relations: &'static [FactionRelation],
    ) -> Result<StartingScenario, RulesetLoadError> {
        let colony_name = leak_non_empty(self.colony_name, "colony_name must not be empty")?;
        let player_faction_id = FactionId::new(self.player_faction_id);
        if factions
            .iter()
            .all(|faction| faction.id != player_faction_id)
        {
            return Err(RulesetLoadError::StartingScenario(
                "player_faction_id does not exist in factions.ron",
            ));
        }
        let system_id = SystemId::from_index(self.home_system_index);
        let planet_id = PlanetId::from_system_index(system_id, self.home_planet_index);
        let mut buildings = BuildingLevels::EMPTY;
        let mut configured_buildings = BTreeSet::new();
        for configured in self.buildings {
            if !configured_buildings.insert(configured.id.clone()) {
                return Err(RulesetLoadError::StartingScenario(
                    "a starting building is duplicated",
                ));
            }
            let Some(kind) = catalog.kind_by_key(&configured.id) else {
                return Err(RulesetLoadError::StartingScenario(
                    "a starting building does not exist in buildings.ron",
                ));
            };
            buildings.set_level(kind, configured.level);
        }
        catalog
            .validate_levels(buildings)
            .map_err(RulesetLoadError::Buildings)?;

        let resource_profile = PlanetResourceProfile::new(
            self.resource_profile.metal,
            self.resource_profile.crystal,
            self.resource_profile.fuel,
            self.resource_profile.energy,
        );
        if !resource_profile.is_viable() {
            return Err(RulesetLoadError::StartingScenario(
                "resource profile values must be greater than zero",
            ));
        }

        let initial_system_knowledge = Box::leak(
            vec![InitialSystemKnowledge {
                system_id,
                level: KnowledgeLevel::Colonized,
            }]
            .into_boxed_slice(),
        );
        let initial_planet_knowledge = Box::leak(
            vec![InitialPlanetKnowledge {
                planet_id,
                level: KnowledgeLevel::Colonized,
            }]
            .into_boxed_slice(),
        );
        let mut completed = Vec::new();
        for key in self.initial_technologies {
            let Some(technology) = technologies.id_by_key(&key) else {
                return Err(RulesetLoadError::StartingScenario(
                    "an initial technology does not exist in technologies.ron",
                ));
            };
            if completed.contains(&technology) {
                return Err(RulesetLoadError::StartingScenario(
                    "an initial technology is duplicated",
                ));
            }
            if technologies
                .definition(technology)
                .prerequisites
                .iter()
                .any(|prerequisite| !completed.contains(prerequisite))
            {
                return Err(RulesetLoadError::StartingScenario(
                    "initial technologies are not ordered after their prerequisites",
                ));
            }
            completed.push(technology);
        }
        let initial_technologies = Box::leak(completed.into_boxed_slice());

        let initial_stock = self.initial_stock.into_stock();
        if !initial_stock.is_within(catalog.storage_capacity_for_levels(buildings)) {
            return Err(RulesetLoadError::StartingScenario(
                "initial stock exceeds configured storage capacity",
            ));
        }

        Ok(StartingScenario {
            factions,
            default_relation,
            initial_relations,
            player_faction_id,
            home_colony: StartingColonyConfig {
                id: ColonyId::new(self.colony_id),
                name: colony_name,
                owner: Owner::Faction(player_faction_id),
                system_id,
                planet_id,
                initial_stock,
                initial_energy: catalog.energy_grid_for_levels(buildings),
                buildings,
                resource_profile,
            },
            initial_system_knowledge,
            initial_planet_knowledge,
            initial_technologies,
            minimum_home_habitability: self.minimum_home_habitability,
        })
    }
}

fn append_faction_structure(factions: &[StartingFactionConfig], output: &mut String) {
    for faction in factions {
        output.push_str("faction:");
        output.push_str(&faction.id.raw().to_string());
        output.push(':');
        output.push_str(match faction.kind {
            FactionKind::Player => "player",
            FactionKind::Neutral => "neutral",
            FactionKind::FutureAi => "future_ai",
        });
        output.push(':');
        output.push_str(if faction.active {
            "active;"
        } else {
            "inactive;"
        });
    }
}

fn append_diplomacy_structure(
    default_relation: DiplomaticRelation,
    relations: &[FactionRelation],
    output: &mut String,
) {
    output.push_str("default_relation:");
    output.push_str(relation_key(default_relation));
    output.push(';');
    for relation in relations {
        output.push_str("relation:");
        output.push_str(&relation.first.raw().to_string());
        output.push(':');
        output.push_str(&relation.second.raw().to_string());
        output.push(':');
        output.push_str(relation_key(relation.relation));
        output.push(';');
    }
}

const fn relation_key(relation: DiplomaticRelation) -> &'static str {
    match relation {
        DiplomaticRelation::Unknown => "unknown",
        DiplomaticRelation::Neutral => "neutral",
        DiplomaticRelation::Hostile => "hostile",
        DiplomaticRelation::Allied => "allied",
    }
}

#[derive(Debug, Deserialize)]
struct StartingBuildingConfig {
    id: String,
    level: u8,
}

#[derive(Debug, Deserialize)]
struct ResourceProfileConfig {
    metal: u16,
    crystal: u16,
    fuel: u16,
    energy: u16,
}

fn read_ron<T: DeserializeOwned>(
    directory: &Path,
    filename: &'static str,
) -> Result<T, RulesetLoadError> {
    let path = directory.join(filename);
    let source = fs::read_to_string(&path).map_err(|source| RulesetLoadError::Read {
        path: path.clone(),
        source,
    })?;
    ron::de::from_str(&source).map_err(|error| RulesetLoadError::Parse {
        path,
        message: error.to_string(),
    })
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

fn leak_non_empty(value: String, message: &'static str) -> Result<&'static str, RulesetLoadError> {
    if value.trim().is_empty() {
        Err(RulesetLoadError::StartingScenario(message))
    } else {
        Ok(Box::leak(value.into_boxed_str()))
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ruleset_is_external_and_valid() {
        let ruleset = default_ruleset();
        assert_eq!(ruleset.id(), "default");
        assert_eq!(ruleset.schema_version(), RULESET_SCHEMA_VERSION);
        assert_ne!(ruleset.structure_fingerprint(), 0);
        assert_eq!(ruleset.factions().len(), 3);
        assert_eq!(
            ruleset
                .factions()
                .iter()
                .filter(|faction| faction.active)
                .count(),
            1,
        );
        assert_eq!(ruleset.buildings().definitions().count(), 8);
        assert_eq!(ruleset.technologies().definitions().count(), 6);
        assert_eq!(ruleset.craftables().definitions().count(), 9);
        assert_eq!(ruleset.extraction().version(), 1);
        assert_eq!(ruleset.planetary_analysis().version(), 4,);
        assert_eq!(ruleset.planetary_presence().version(), 2);
        assert_eq!(ruleset.planetary_presence().definitions().count(), 13);
        assert_eq!(ruleset.combat().version(), 2);
        assert_eq!(ruleset.combat().ships().count(), 3);
    }

    #[test]
    fn text_is_not_part_of_the_structure_fingerprint() {
        let mut first = format!("ruleset:default;schema:{RULESET_SCHEMA_VERSION};");
        append_faction_structure(default_ruleset().factions(), &mut first);
        append_diplomacy_structure(
            default_ruleset().starting_scenario().default_relation,
            default_ruleset().starting_scenario().initial_relations,
            &mut first,
        );
        default_ruleset().buildings().append_structure(&mut first);
        default_ruleset()
            .technologies()
            .append_structure(&mut first);
        default_ruleset().craftables().append_structure(&mut first);
        default_ruleset().extraction().append_structure(&mut first);
        default_ruleset()
            .planetary_analysis()
            .append_structure(&mut first);
        default_ruleset()
            .planetary_presence()
            .append_structure(&mut first);
        default_ruleset().combat().append_structure(&mut first);
        assert_eq!(
            default_ruleset().structure_fingerprint(),
            fnv1a64(first.as_bytes()),
        );
    }
}

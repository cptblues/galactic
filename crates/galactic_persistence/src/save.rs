use galactic_domain::{
    ColonyId, FactionId, FleetId, MissionId, Owner, PlanetId, ResourceLedgerError,
    ResourceReservation, ResourceStock, SystemId, UniverseId,
};
use galactic_sim::{
    BuildingLevels, CombatReport, ConstructionQueue, CraftInventory, CraftQueue, DiplomacyState,
    ExtractionSiteState, FactionKind, FleetAssignment, FleetComposition, FleetLocation,
    MissionReport, MissionState, PendingCombat, PlanetAnalysisReport, PlanetKnowledge,
    PlanetResourceProfile, PlanetaryIntelligenceReport, PlanetaryPresence,
    ProductionRemainderError, ResearchState, SelectionTarget, SimulationBuildError,
    StrategicClockError, StrategicTick, SystemKnowledge, TimeSpeed,
};

/// Version 29 persists A6 combat roles and per-ship combat snapshots.
pub const SAVE_VERSION: u32 = 29;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SaveGame {
    pub version: u32,
    pub ruleset_id: String,
    pub ruleset_schema_version: u32,
    pub ruleset_content_version: u32,
    pub ruleset_structure_fingerprint: u64,
    pub universe: UniverseReference,
    pub state: MutableGameSave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UniverseReference {
    pub id: UniverseId,
    pub seed: u64,
    pub system_count: usize,
    pub generation_version: u32,
    pub generation_fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MutableGameSave {
    pub version: u32,
    pub factions: Vec<FactionSave>,
    pub diplomacy: DiplomacyState,
    pub player_faction: FactionId,
    pub clock: StrategicClockSave,
    pub selected: SelectionTarget,
    pub system_knowledge: Vec<SystemKnowledge>,
    pub planet_knowledge: Vec<PlanetKnowledge>,
    pub colonies: Vec<ColonySave>,
    pub next_colony_id: u64,
    pub active_colony_id: Option<ColonyId>,
    pub fleets: Vec<FleetSave>,
    pub next_fleet_id: u64,
    pub missions: Vec<MissionState>,
    pub next_mission_id: u64,
    pub mission_reports: Vec<MissionReport>,
    pub combat_reports: Vec<CombatReport>,
    #[serde(default)]
    pub pending_combats: Vec<PendingCombat>,
    pub colony_foundations: Vec<galactic_sim::ColonyFoundation>,
    pub planet_analysis_reports: Vec<PlanetAnalysisReport>,
    pub extraction_sites: Vec<ExtractionSiteState>,
    pub planetary_presences: Vec<PlanetaryPresence>,
    pub planetary_intelligence_reports: Vec<PlanetaryIntelligenceReport>,
    pub research: ResearchState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FactionSave {
    pub id: FactionId,
    pub name: String,
    pub kind: FactionKind,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StrategicClockSave {
    pub current_tick: StrategicTick,
    pub remainder_nanos: u64,
    pub speed: TimeSpeed,
    pub resume_speed: TimeSpeed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColonySave {
    pub id: ColonyId,
    pub name: String,
    pub owner: Owner,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub founding_mission_id: Option<MissionId>,
    pub stock: ResourceStock,
    pub reservations: Vec<ResourceReservation>,
    pub next_reservation_id: u64,
    pub energy_production: u64,
    pub energy_consumption: u64,
    pub production_remainder_metal: u16,
    pub production_remainder_crystal: u16,
    pub production_remainder_fuel: u16,
    pub production_pending_ticks: u16,
    pub construction_queue: ConstructionQueue,
    pub craft_queue: CraftQueue,
    pub inventory: CraftInventory,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FleetSave {
    pub id: FleetId,
    pub name: String,
    pub owner: Owner,
    pub location: FleetLocation,
    pub composition: FleetComposition,
    pub cargo: ResourceStock,
    pub assignment: FleetAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    UnsupportedVersion(u32),
    RulesetIdMismatch,
    RulesetSchemaVersionMismatch {
        expected: u32,
        found: u32,
    },
    RulesetStructureMismatch {
        expected: u64,
        found: u64,
    },
    UniverseIdMismatch {
        expected: UniverseId,
        found: UniverseId,
    },
    GenerationVersionMismatch {
        expected: u32,
        found: u32,
    },
    GenerationFingerprintMismatch {
        expected: u64,
        found: u64,
    },
    InvalidClock(StrategicClockError),
    InvalidResourceLedger {
        colony_id: ColonyId,
        error: ResourceLedgerError,
    },
    InvalidProductionRemainder {
        colony_id: ColonyId,
        error: ProductionRemainderError,
    },
    InvalidPendingProductionTicks {
        colony_id: ColonyId,
        found: u16,
    },
    InvalidState(SimulationBuildError),
}

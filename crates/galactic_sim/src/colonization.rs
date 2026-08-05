// MVP-027: persistent foundations initialize deterministic playable colonies.
use std::collections::BTreeSet;

use galactic_domain::{
    ColonyId, FactionId, MissionId, Owner, PlanetId, ResourceCost, ResourceLedger, ResourceStock,
    SystemId,
};

use crate::{
    ColonizationBlocker, ColonyState, ConstructionQueue, CraftInventory, CraftQueue, CraftableId,
    GameState, KnowledgeChange, KnowledgeLevel, MissionKind, MissionPhase, MissionResult,
    PlanetaryIntelPrecision, ProductionRemainder, StrategicTick, UniverseRepository,
    assess_planet_colonizability, default_building_catalog, planetary_analysis_rules,
    refresh_planetary_intelligence,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColonizationMissionCommitment {
    pub planet_id: PlanetId,
    pub colony_ship: CraftableId,
    pub foundation_cost: ResourceCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColonizationMissionOutcome {
    FoundationPrepared,
    TargetInvalid(ColonizationBlocker),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColonizationMissionResult {
    pub target: PlanetId,
    pub outcome: ColonizationMissionOutcome,
    pub colony_ship_consumed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColonyFoundation {
    pub mission_id: MissionId,
    pub owner: FactionId,
    pub source_colony_id: ColonyId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub payload: ResourceStock,
    pub prepared_at: StrategicTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColonyEstablished {
    pub colony_id: ColonyId,
    pub mission_id: MissionId,
    pub owner: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub established_at: StrategicTick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColonyFoundationStateError {
    DuplicateMission(MissionId),
    DuplicatePlanet(PlanetId),
    UnknownMission(MissionId),
    MissingFoundation(MissionId),
    MissionMismatch(MissionId),
    UnknownOwner(FactionId),
    UnknownSourceColony(ColonyId),
    SourceOwnerMismatch(ColonyId),
    UnknownPlanet(PlanetId),
    PlanetSystemMismatch {
        planet_id: PlanetId,
        expected: SystemId,
        found: SystemId,
    },
    MissingColony(MissionId),
    ColonyMismatch(MissionId),
    MissingFoundationProvenance {
        colony_id: ColonyId,
        mission_id: MissionId,
    },
    InvalidPayload(MissionId),
    PreparedInFuture {
        mission_id: MissionId,
        prepared_at: StrategicTick,
        current_tick: StrategicTick,
    },
}

pub(crate) fn initialize_colony_from_foundation(
    state: &mut GameState,
    universe: &UniverseRepository,
    foundation: ColonyFoundation,
) -> (ColonyEstablished, Vec<KnowledgeChange>) {
    let initialization = planetary_analysis_rules().colony_initialization();
    let resource_profile = state
        .planet_analysis_report(foundation.planet_id)
        .expect("a validated colonization target has an analysis report")
        .resource_profile;
    let planet_name = universe
        .planet(foundation.planet_id)
        .expect("a validated foundation targets an existing planet")
        .name
        .clone();
    let colony_id = ColonyId::new(state.next_colony_id);
    state.next_colony_id = state
        .next_colony_id
        .checked_add(1)
        .expect("the configured colony limit keeps identities representable");
    let buildings = initialization.buildings;
    let owner = Owner::Faction(foundation.owner);

    let presence = state
        .planetary_presence_mut(foundation.planet_id)
        .expect("every generated planet has a validated presence");
    presence.occupant = owner;
    presence.population = initialization.population;
    presence.forces.clear();
    presence.revision = presence
        .revision
        .checked_add(1)
        .expect("planetary presence revision remains representable");

    let knowledge_changes =
        state.advance_planet_knowledge(universe, foundation.planet_id, KnowledgeLevel::Colonized);
    refresh_planetary_intelligence(
        state,
        foundation.planet_id,
        PlanetaryIntelPrecision::Exact,
        foundation.prepared_at,
    )
    .expect("a newly established colony has a planetary presence");

    state.colonies.push(ColonyState {
        id: colony_id,
        name: planet_name,
        owner,
        system_id: foundation.system_id,
        planet_id: foundation.planet_id,
        founding_mission_id: Some(foundation.mission_id),
        resources: ResourceLedger::new(foundation.payload),
        energy: default_building_catalog().energy_grid_for_levels(buildings),
        production_remainder: ProductionRemainder::ZERO,
        production_pending_ticks: 0,
        construction_queue: ConstructionQueue::default(),
        craft_queue: CraftQueue::default(),
        inventory: CraftInventory::default(),
        buildings,
        resource_profile,
    });
    state.colonies.sort_by_key(|colony| colony.id);

    (
        ColonyEstablished {
            colony_id,
            mission_id: foundation.mission_id,
            owner: foundation.owner,
            system_id: foundation.system_id,
            planet_id: foundation.planet_id,
            established_at: foundation.prepared_at,
        },
        knowledge_changes,
    )
}

pub(crate) fn colonization_arrival_blocker(
    state: &GameState,
    universe: &UniverseRepository,
    actor: FactionId,
    planet_id: PlanetId,
) -> Option<ColonizationBlocker> {
    assess_planet_colonizability(state, universe, actor, planet_id)
        .blockers
        .into_iter()
        .find(|blocker| {
            !matches!(
                blocker,
                ColonizationBlocker::NoAccessibleRoute
                    | ColonizationBlocker::InsufficientFoundationResources { .. }
            )
        })
}

pub fn validate_colony_foundations(
    state: &GameState,
    universe: &UniverseRepository,
) -> Result<(), ColonyFoundationStateError> {
    let mut mission_ids = BTreeSet::new();
    let mut planet_ids = BTreeSet::new();
    let expected_payload = planetary_analysis_rules().foundation_cost().as_stock();

    for foundation in &state.colony_foundations {
        if !mission_ids.insert(foundation.mission_id) {
            return Err(ColonyFoundationStateError::DuplicateMission(
                foundation.mission_id,
            ));
        }
        if !planet_ids.insert(foundation.planet_id) {
            return Err(ColonyFoundationStateError::DuplicatePlanet(
                foundation.planet_id,
            ));
        }
        let Some(mission) = state.mission(foundation.mission_id) else {
            return Err(ColonyFoundationStateError::UnknownMission(
                foundation.mission_id,
            ));
        };
        let mission_matches = matches!(
            mission.result,
            Some(MissionResult::Colonize(ColonizationMissionResult {
                target,
                outcome: ColonizationMissionOutcome::FoundationPrepared,
                colony_ship_consumed: true,
            })) if target == foundation.planet_id
        ) && mission.order.kind == MissionKind::Colonize
            && mission.phase == MissionPhase::Completed
            && mission.order.target.planet_id() == Some(foundation.planet_id)
            && mission.origin_colony_id == foundation.source_colony_id
            && mission.owner == Owner::Faction(foundation.owner);
        if !mission_matches {
            return Err(ColonyFoundationStateError::MissionMismatch(
                foundation.mission_id,
            ));
        }
        if state.faction(foundation.owner).is_none() {
            return Err(ColonyFoundationStateError::UnknownOwner(foundation.owner));
        }
        let Some(source) = state.colony(foundation.source_colony_id) else {
            return Err(ColonyFoundationStateError::UnknownSourceColony(
                foundation.source_colony_id,
            ));
        };
        if source.owner != Owner::Faction(foundation.owner) {
            return Err(ColonyFoundationStateError::SourceOwnerMismatch(
                foundation.source_colony_id,
            ));
        }
        let Some((expected_system, _)) = universe.planet_location(foundation.planet_id) else {
            return Err(ColonyFoundationStateError::UnknownPlanet(
                foundation.planet_id,
            ));
        };
        if expected_system != foundation.system_id {
            return Err(ColonyFoundationStateError::PlanetSystemMismatch {
                planet_id: foundation.planet_id,
                expected: expected_system,
                found: foundation.system_id,
            });
        }
        let Some(colony) = state.colony_on_planet(foundation.planet_id) else {
            return Err(ColonyFoundationStateError::MissingColony(
                foundation.mission_id,
            ));
        };
        if colony.founding_mission_id != Some(foundation.mission_id)
            || colony.owner != Owner::Faction(foundation.owner)
            || colony.system_id != foundation.system_id
        {
            return Err(ColonyFoundationStateError::ColonyMismatch(
                foundation.mission_id,
            ));
        }
        if foundation.payload != expected_payload {
            return Err(ColonyFoundationStateError::InvalidPayload(
                foundation.mission_id,
            ));
        }
        if foundation.prepared_at > state.clock.current_tick() {
            return Err(ColonyFoundationStateError::PreparedInFuture {
                mission_id: foundation.mission_id,
                prepared_at: foundation.prepared_at,
                current_tick: state.clock.current_tick(),
            });
        }
    }
    for mission in &state.missions {
        if matches!(
            mission.result,
            Some(MissionResult::Colonize(ColonizationMissionResult {
                outcome: ColonizationMissionOutcome::FoundationPrepared,
                colony_ship_consumed: true,
                ..
            }))
        ) && !mission_ids.contains(&mission.id)
        {
            return Err(ColonyFoundationStateError::MissingFoundation(mission.id));
        }
    }
    for colony in &state.colonies {
        if let Some(mission_id) = colony.founding_mission_id
            && !mission_ids.contains(&mission_id)
        {
            return Err(ColonyFoundationStateError::MissingFoundationProvenance {
                colony_id: colony.id,
                mission_id,
            });
        }
    }
    Ok(())
}

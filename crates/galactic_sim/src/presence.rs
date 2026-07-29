// MVP-025: deterministic planetary occupants, forces and bounded intelligence.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use galactic_domain::{FactionId, Owner, PlanetId};
use serde::Deserialize;

use crate::{
    GameState, KnowledgeLevel, StartingFactionConfig, StrategicTick, UniverseRepository,
    default_ruleset,
};

pub const MAX_PLANETARY_FORCE_DEFINITIONS: usize = 64;
pub const MAX_PLANETARY_PRESENCE_PROFILES: usize = 32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanetaryForceId(&'static str);

impl PlanetaryForceId {
    pub const fn from_static(key: &'static str) -> Self {
        Self(key)
    }

    pub const fn key(self) -> &'static str {
        self.0
    }

    fn from_config(key: String) -> Result<Self, PlanetaryPresenceRulesError> {
        validate_identifier(&key).map_err(|()| PlanetaryPresenceRulesError::InvalidIdentifier)?;
        Ok(Self(Box::leak(key.into_boxed_str())))
    }
}

impl fmt::Debug for PlanetaryForceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PlanetaryForceId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for PlanetaryForceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum PlanetaryForceDomain {
    Ground,
    Orbital,
}

impl PlanetaryForceDomain {
    const fn structural_key(self) -> &'static str {
        match self {
            Self::Ground => "ground",
            Self::Orbital => "orbital",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetaryForceDefinition {
    pub id: PlanetaryForceId,
    pub name: &'static str,
    pub domain: PlanetaryForceDomain,
    pub offense: u32,
    pub defense: u32,
    pub durability: u32,
    pub estimate_step: u32,
}

impl PlanetaryForceDefinition {
    pub const fn strategic_strength(self) -> u64 {
        self.offense as u64 + self.defense as u64 + self.durability as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProfileForce {
    definition_id: PlanetaryForceId,
    minimum: u32,
    maximum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanetaryPresenceProfile {
    id: &'static str,
    weight: u32,
    occupant: Owner,
    minimum_population: u64,
    maximum_population: u64,
    forces: Vec<ProfileForce>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryPresenceRules {
    version: u32,
    population_estimate_step: u64,
    strength_estimate_step: u64,
    home_population: u64,
    home_forces: Vec<PlanetaryForceStack>,
    definitions: BTreeMap<PlanetaryForceId, PlanetaryForceDefinition>,
    profiles: Vec<PlanetaryPresenceProfile>,
    total_profile_weight: u64,
}

impl PlanetaryPresenceRules {
    pub(crate) fn from_config(
        config: PlanetaryPresenceRulesConfig,
        factions: &[StartingFactionConfig],
    ) -> Result<Self, PlanetaryPresenceRulesError> {
        if config.version != 1 {
            return Err(PlanetaryPresenceRulesError::UnsupportedVersion(
                config.version,
            ));
        }
        if config.population_estimate_step < 2 {
            return Err(PlanetaryPresenceRulesError::InvalidPopulationEstimateStep);
        }
        if config.strength_estimate_step < 2 {
            return Err(PlanetaryPresenceRulesError::InvalidStrengthEstimateStep);
        }
        if config.units.is_empty() || config.units.len() > MAX_PLANETARY_FORCE_DEFINITIONS {
            return Err(PlanetaryPresenceRulesError::InvalidDefinitionCount {
                found: config.units.len(),
                maximum: MAX_PLANETARY_FORCE_DEFINITIONS,
            });
        }

        let mut definition_ids = BTreeMap::new();
        for configured in &config.units {
            let id = PlanetaryForceId::from_config(configured.id.clone())?;
            if definition_ids.insert(configured.id.clone(), id).is_some() {
                return Err(PlanetaryPresenceRulesError::DuplicateDefinition(id));
            }
        }

        let mut definitions = BTreeMap::new();
        for configured in config.units {
            let id = definition_ids[&configured.id];
            if configured.name.trim().is_empty() {
                return Err(PlanetaryPresenceRulesError::EmptyDefinitionName(id));
            }
            if configured.offense == 0 || configured.defense == 0 || configured.durability == 0 {
                return Err(PlanetaryPresenceRulesError::InvalidDefinitionStats(id));
            }
            if configured.estimate_step < 2 {
                return Err(PlanetaryPresenceRulesError::InvalidDefinitionEstimateStep(
                    id,
                ));
            }
            definitions.insert(
                id,
                PlanetaryForceDefinition {
                    id,
                    name: Box::leak(configured.name.into_boxed_str()),
                    domain: configured.domain,
                    offense: configured.offense,
                    defense: configured.defense,
                    durability: configured.durability,
                    estimate_step: configured.estimate_step,
                },
            );
        }

        if config.home.population == 0 {
            return Err(PlanetaryPresenceRulesError::InvalidHomePopulation);
        }
        let home_forces = compile_fixed_forces(config.home.forces, &definition_ids, &definitions)?;

        if config.profiles.is_empty() || config.profiles.len() > MAX_PLANETARY_PRESENCE_PROFILES {
            return Err(PlanetaryPresenceRulesError::InvalidProfileCount {
                found: config.profiles.len(),
                maximum: MAX_PLANETARY_PRESENCE_PROFILES,
            });
        }

        let faction_ids = factions
            .iter()
            .map(|faction| faction.id)
            .collect::<BTreeSet<_>>();
        let mut profile_ids = BTreeSet::new();
        let mut profiles = Vec::with_capacity(config.profiles.len());
        let mut has_unoccupied = false;
        let mut has_occupied = false;
        let mut total_profile_weight = 0_u64;
        for configured in config.profiles {
            validate_identifier(&configured.id)
                .map_err(|()| PlanetaryPresenceRulesError::InvalidIdentifier)?;
            if !profile_ids.insert(configured.id.clone()) {
                return Err(PlanetaryPresenceRulesError::DuplicateProfile);
            }
            if configured.weight == 0 {
                return Err(PlanetaryPresenceRulesError::InvalidProfileWeight);
            }
            total_profile_weight = total_profile_weight
                .checked_add(u64::from(configured.weight))
                .ok_or(PlanetaryPresenceRulesError::InvalidProfileWeight)?;

            let occupant = configured
                .occupant_faction_id
                .map(FactionId::new)
                .map_or(Owner::Unowned, Owner::Faction);
            match occupant {
                Owner::Unowned => {
                    has_unoccupied = true;
                    if configured.minimum_population != 0
                        || configured.maximum_population != 0
                        || !configured.forces.is_empty()
                    {
                        return Err(PlanetaryPresenceRulesError::InvalidUnoccupiedProfile);
                    }
                }
                Owner::Faction(faction_id) => {
                    has_occupied = true;
                    if !faction_ids.contains(&faction_id) {
                        return Err(PlanetaryPresenceRulesError::UnknownProfileFaction(
                            faction_id,
                        ));
                    }
                    if configured.minimum_population == 0
                        || configured.maximum_population < configured.minimum_population
                    {
                        return Err(PlanetaryPresenceRulesError::InvalidOccupiedPopulation);
                    }
                }
            }

            let forces = compile_profile_forces(configured.forces, &definition_ids, &definitions)?;
            profiles.push(PlanetaryPresenceProfile {
                id: Box::leak(configured.id.into_boxed_str()),
                weight: configured.weight,
                occupant,
                minimum_population: configured.minimum_population,
                maximum_population: configured.maximum_population,
                forces,
            });
        }
        if !has_unoccupied || !has_occupied {
            return Err(PlanetaryPresenceRulesError::MissingProfileCategory);
        }

        Ok(Self {
            version: config.version,
            population_estimate_step: config.population_estimate_step,
            strength_estimate_step: config.strength_estimate_step,
            home_population: config.home.population,
            home_forces,
            definitions,
            profiles,
            total_profile_weight,
        })
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub fn definitions(&self) -> impl Iterator<Item = &PlanetaryForceDefinition> {
        self.definitions.values()
    }

    pub fn definition(&self, id: PlanetaryForceId) -> Option<&PlanetaryForceDefinition> {
        self.definitions.get(&id)
    }

    pub fn id_by_key(&self, key: &str) -> Option<PlanetaryForceId> {
        self.definitions.keys().copied().find(|id| id.key() == key)
    }

    pub fn initial_presences(
        &self,
        universe: &UniverseRepository,
        home_planet: PlanetId,
        home_owner: Owner,
    ) -> Vec<PlanetaryPresence> {
        let mut presences = Vec::new();
        for system in &universe.definition().systems {
            for planet in &system.planets {
                let presence = if planet.id == home_planet {
                    PlanetaryPresence {
                        planet_id: planet.id,
                        occupant: home_owner,
                        population: self.home_population,
                        forces: self.home_forces.clone(),
                        revision: 0,
                    }
                } else {
                    self.presence_from_profile(universe.definition().seed, planet.id)
                };
                presences.push(presence);
            }
        }
        presences.sort_by_key(|presence| presence.planet_id);
        presences
    }

    fn presence_from_profile(&self, universe_seed: u64, planet_id: PlanetId) -> PlanetaryPresence {
        let selector = splitmix64(universe_seed ^ planet_id.raw() ^ 0x5052_4553_454e_4345)
            % self.total_profile_weight;
        let mut cursor = selector;
        let profile = self
            .profiles
            .iter()
            .find(|profile| {
                if cursor < u64::from(profile.weight) {
                    true
                } else {
                    cursor -= u64::from(profile.weight);
                    false
                }
            })
            .expect("validated profile weights must select one profile");
        let population = varied_inclusive(
            profile.minimum_population,
            profile.maximum_population,
            planet_id.raw() ^ 0x504f_5055_4c41_5449,
        );
        let mut forces = profile
            .forces
            .iter()
            .enumerate()
            .map(|(index, force)| PlanetaryForceStack {
                definition_id: force.definition_id,
                quantity: varied_inclusive(
                    u64::from(force.minimum),
                    u64::from(force.maximum),
                    planet_id.raw() ^ 0x464f_5243_4553 ^ index as u64,
                ) as u32,
            })
            .collect::<Vec<_>>();
        forces.sort_by_key(|force| force.definition_id);
        PlanetaryPresence {
            planet_id,
            occupant: profile.occupant,
            population,
            forces,
            revision: 0,
        }
    }

    fn estimate_presence(
        &self,
        presence: &PlanetaryPresence,
        precision: PlanetaryIntelPrecision,
        observed_at: StrategicTick,
    ) -> PlanetaryIntelligenceReport {
        let occupancy = match (precision, presence.occupant) {
            (_, Owner::Unowned) => PlanetaryOccupancyIntel::Unoccupied,
            (PlanetaryIntelPrecision::Contact, Owner::Faction(_)) => {
                PlanetaryOccupancyIntel::OccupiedUnknown
            }
            (
                PlanetaryIntelPrecision::Surveyed | PlanetaryIntelPrecision::Exact,
                Owner::Faction(faction_id),
            ) => PlanetaryOccupancyIntel::Occupied(faction_id),
        };
        let population = match (precision, presence.occupant) {
            (_, Owner::Unowned) | (PlanetaryIntelPrecision::Contact, _) => None,
            (PlanetaryIntelPrecision::Surveyed, Owner::Faction(_)) => Some(
                EstimateRange::bucketed(presence.population, self.population_estimate_step),
            ),
            (PlanetaryIntelPrecision::Exact, Owner::Faction(_)) => {
                Some(EstimateRange::exact(presence.population))
            }
        };
        let range_for_strength = |value| match precision {
            PlanetaryIntelPrecision::Exact => EstimateRange::exact(value),
            PlanetaryIntelPrecision::Contact | PlanetaryIntelPrecision::Surveyed => {
                EstimateRange::bucketed(value, self.strength_estimate_step)
            }
        };
        let ground_strength =
            range_for_strength(self.strategic_strength(presence, PlanetaryForceDomain::Ground));
        let orbital_strength =
            range_for_strength(self.strategic_strength(presence, PlanetaryForceDomain::Orbital));
        let forces = match precision {
            PlanetaryIntelPrecision::Contact => Vec::new(),
            PlanetaryIntelPrecision::Surveyed => presence
                .forces
                .iter()
                .map(|force| {
                    let definition = self
                        .definition(force.definition_id)
                        .expect("validated presence references a known definition");
                    PlanetaryForceEstimate {
                        definition_id: force.definition_id,
                        quantity: EstimateRange::bucketed(
                            u64::from(force.quantity),
                            u64::from(definition.estimate_step),
                        ),
                    }
                })
                .collect(),
            PlanetaryIntelPrecision::Exact => presence
                .forces
                .iter()
                .map(|force| PlanetaryForceEstimate {
                    definition_id: force.definition_id,
                    quantity: EstimateRange::exact(u64::from(force.quantity)),
                })
                .collect(),
        };

        PlanetaryIntelligenceReport {
            planet_id: presence.planet_id,
            observed_at,
            precision,
            occupancy,
            population,
            ground_strength,
            orbital_strength,
            forces,
        }
    }

    fn strategic_strength(
        &self,
        presence: &PlanetaryPresence,
        domain: PlanetaryForceDomain,
    ) -> u64 {
        presence
            .forces
            .iter()
            .filter_map(|force| {
                let definition = self.definition(force.definition_id)?;
                (definition.domain == domain).then_some(
                    definition
                        .strategic_strength()
                        .saturating_mul(u64::from(force.quantity)),
                )
            })
            .fold(0_u64, u64::saturating_add)
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("planetary-presence:");
        output.push_str(&self.version.to_string());
        output.push(';');
        for definition in self.definitions.values() {
            output.push_str(definition.id.key());
            output.push(':');
            output.push_str(definition.domain.structural_key());
            output.push(';');
        }
        output.push_str("profiles:");
        for profile in &self.profiles {
            output.push_str(profile.id);
            output.push(':');
            match profile.occupant {
                Owner::Unowned => output.push_str("unowned"),
                Owner::Faction(faction_id) => {
                    output.push_str("faction-");
                    output.push_str(&faction_id.raw().to_string());
                }
            }
            output.push('[');
            for force in &profile.forces {
                output.push_str(force.definition_id.key());
                output.push(',');
            }
            output.push_str("];");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetaryForceStack {
    pub definition_id: PlanetaryForceId,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryPresence {
    pub planet_id: PlanetId,
    pub occupant: Owner,
    pub population: u64,
    pub forces: Vec<PlanetaryForceStack>,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlanetaryIntelPrecision {
    Contact,
    Surveyed,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryOccupancyIntel {
    Unoccupied,
    OccupiedUnknown,
    Occupied(FactionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimateRange {
    pub minimum: u64,
    pub maximum: u64,
}

impl EstimateRange {
    pub const fn exact(value: u64) -> Self {
        Self {
            minimum: value,
            maximum: value,
        }
    }

    pub fn bucketed(value: u64, step: u64) -> Self {
        if value == 0 {
            return Self::exact(0);
        }
        let minimum = value / step * step;
        Self {
            minimum,
            maximum: minimum.saturating_add(step.saturating_sub(1)),
        }
    }

    pub const fn is_valid(self) -> bool {
        self.minimum <= self.maximum
    }

    pub const fn is_exact(self) -> bool {
        self.minimum == self.maximum
    }

    pub const fn contains(self, value: u64) -> bool {
        value >= self.minimum && value <= self.maximum
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetaryForceEstimate {
    pub definition_id: PlanetaryForceId,
    pub quantity: EstimateRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryIntelligenceReport {
    pub planet_id: PlanetId,
    pub observed_at: StrategicTick,
    pub precision: PlanetaryIntelPrecision,
    pub occupancy: PlanetaryOccupancyIntel,
    pub population: Option<EstimateRange>,
    pub ground_strength: EstimateRange,
    pub orbital_strength: EstimateRange,
    pub forces: Vec<PlanetaryForceEstimate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetaryForceLoss {
    pub definition_id: PlanetaryForceId,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetaryPresenceUpdate {
    pub planet_id: PlanetId,
    pub previous_revision: u64,
    pub current_revision: u64,
    pub losses: Vec<PlanetaryForceLoss>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryPresenceUpdateError {
    UnknownPlanet(PlanetId),
    EmptyUpdate,
    UnknownDefinition(PlanetaryForceId),
    DuplicateLoss(PlanetaryForceId),
    EmptyLoss(PlanetaryForceId),
    MissingForce(PlanetaryForceId),
    ExcessiveLoss {
        definition_id: PlanetaryForceId,
        available: u32,
        requested: u32,
    },
    RevisionOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryIntelligenceError {
    UnknownPlanet(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryPresenceStateError {
    DuplicatePresence(PlanetId),
    MissingPresence(PlanetId),
    UnknownPresencePlanet(PlanetId),
    UnknownOccupant {
        planet_id: PlanetId,
        faction_id: FactionId,
    },
    InvalidUnoccupiedPresence(PlanetId),
    InvalidOccupiedPopulation(PlanetId),
    UnknownForceDefinition {
        planet_id: PlanetId,
        definition_id: PlanetaryForceId,
    },
    DuplicateForce {
        planet_id: PlanetId,
        definition_id: PlanetaryForceId,
    },
    EmptyForce {
        planet_id: PlanetId,
        definition_id: PlanetaryForceId,
    },
    ColonyOccupantMismatch(PlanetId),
    DuplicateIntelligence(PlanetId),
    UnknownIntelligencePlanet(PlanetId),
    IntelligenceKnowledgeTooLow {
        planet_id: PlanetId,
        current: KnowledgeLevel,
    },
    IntelligencePrecisionTooLow {
        planet_id: PlanetId,
        expected: PlanetaryIntelPrecision,
        found: PlanetaryIntelPrecision,
    },
    IntelligenceInFuture(PlanetId),
    InvalidIntelligence(PlanetId),
    UnknownIntelligenceFaction {
        planet_id: PlanetId,
        faction_id: FactionId,
    },
    UnknownIntelligenceDefinition {
        planet_id: PlanetId,
        definition_id: PlanetaryForceId,
    },
    DuplicateIntelligenceForce {
        planet_id: PlanetId,
        definition_id: PlanetaryForceId,
    },
    MissingIntelligence(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetaryPresenceRulesError {
    UnsupportedVersion(u32),
    InvalidIdentifier,
    InvalidPopulationEstimateStep,
    InvalidStrengthEstimateStep,
    InvalidDefinitionCount { found: usize, maximum: usize },
    DuplicateDefinition(PlanetaryForceId),
    EmptyDefinitionName(PlanetaryForceId),
    InvalidDefinitionStats(PlanetaryForceId),
    InvalidDefinitionEstimateStep(PlanetaryForceId),
    InvalidHomePopulation,
    InvalidFixedForce,
    InvalidProfileCount { found: usize, maximum: usize },
    DuplicateProfile,
    InvalidProfileWeight,
    UnknownProfileFaction(FactionId),
    InvalidUnoccupiedProfile,
    InvalidOccupiedPopulation,
    InvalidProfileForce,
    MissingProfileCategory,
}

pub fn planetary_presence_rules() -> &'static PlanetaryPresenceRules {
    default_ruleset().planetary_presence()
}

pub fn refresh_planetary_intelligence(
    state: &mut GameState,
    planet_id: PlanetId,
    requested_precision: PlanetaryIntelPrecision,
    observed_at: StrategicTick,
) -> Result<PlanetaryIntelligenceReport, PlanetaryIntelligenceError> {
    let precision = state
        .planetary_intelligence_report(planet_id)
        .map(|report| report.precision.max(requested_precision))
        .unwrap_or(requested_precision);
    let report = {
        let presence = state
            .planetary_presence(planet_id)
            .ok_or(PlanetaryIntelligenceError::UnknownPlanet(planet_id))?;
        planetary_presence_rules().estimate_presence(presence, precision, observed_at)
    };
    match state
        .planetary_intelligence_reports
        .binary_search_by_key(&planet_id, |report| report.planet_id)
    {
        Ok(index) => state.planetary_intelligence_reports[index] = report.clone(),
        Err(index) => state
            .planetary_intelligence_reports
            .insert(index, report.clone()),
    }
    Ok(report)
}

pub fn apply_planetary_force_losses(
    state: &mut GameState,
    planet_id: PlanetId,
    losses: &[PlanetaryForceLoss],
) -> Result<PlanetaryPresenceUpdate, PlanetaryPresenceUpdateError> {
    if losses.is_empty() {
        return Err(PlanetaryPresenceUpdateError::EmptyUpdate);
    }
    let rules = planetary_presence_rules();
    let presence = state
        .planetary_presence(planet_id)
        .ok_or(PlanetaryPresenceUpdateError::UnknownPlanet(planet_id))?;
    let mut definition_ids = BTreeSet::new();
    for loss in losses {
        if rules.definition(loss.definition_id).is_none() {
            return Err(PlanetaryPresenceUpdateError::UnknownDefinition(
                loss.definition_id,
            ));
        }
        if !definition_ids.insert(loss.definition_id) {
            return Err(PlanetaryPresenceUpdateError::DuplicateLoss(
                loss.definition_id,
            ));
        }
        if loss.quantity == 0 {
            return Err(PlanetaryPresenceUpdateError::EmptyLoss(loss.definition_id));
        }
        let Some(force) = presence
            .forces
            .iter()
            .find(|force| force.definition_id == loss.definition_id)
        else {
            return Err(PlanetaryPresenceUpdateError::MissingForce(
                loss.definition_id,
            ));
        };
        if loss.quantity > force.quantity {
            return Err(PlanetaryPresenceUpdateError::ExcessiveLoss {
                definition_id: loss.definition_id,
                available: force.quantity,
                requested: loss.quantity,
            });
        }
    }
    let next_revision = presence
        .revision
        .checked_add(1)
        .ok_or(PlanetaryPresenceUpdateError::RevisionOverflow)?;

    let presence = state
        .planetary_presence_mut(planet_id)
        .expect("presence was validated before mutation");
    for loss in losses {
        let force = presence
            .forces
            .iter_mut()
            .find(|force| force.definition_id == loss.definition_id)
            .expect("loss target was validated before mutation");
        force.quantity -= loss.quantity;
    }
    presence.forces.retain(|force| force.quantity > 0);
    let previous_revision = presence.revision;
    presence.revision = next_revision;

    Ok(PlanetaryPresenceUpdate {
        planet_id,
        previous_revision,
        current_revision: next_revision,
        losses: losses.to_vec(),
    })
}

pub fn validate_planetary_presence_state(
    state: &GameState,
    universe: &UniverseRepository,
) -> Result<(), PlanetaryPresenceStateError> {
    let rules = planetary_presence_rules();
    let mut presence_ids = BTreeSet::new();
    for presence in &state.planetary_presences {
        if !presence_ids.insert(presence.planet_id) {
            return Err(PlanetaryPresenceStateError::DuplicatePresence(
                presence.planet_id,
            ));
        }
        if universe.planet(presence.planet_id).is_none() {
            return Err(PlanetaryPresenceStateError::UnknownPresencePlanet(
                presence.planet_id,
            ));
        }
        match presence.occupant {
            Owner::Unowned => {
                if presence.population != 0 || !presence.forces.is_empty() {
                    return Err(PlanetaryPresenceStateError::InvalidUnoccupiedPresence(
                        presence.planet_id,
                    ));
                }
            }
            Owner::Faction(faction_id) => {
                if state.faction(faction_id).is_none() {
                    return Err(PlanetaryPresenceStateError::UnknownOccupant {
                        planet_id: presence.planet_id,
                        faction_id,
                    });
                }
                if presence.population == 0 {
                    return Err(PlanetaryPresenceStateError::InvalidOccupiedPopulation(
                        presence.planet_id,
                    ));
                }
            }
        }
        let mut force_ids = BTreeSet::new();
        for force in &presence.forces {
            if rules.definition(force.definition_id).is_none() {
                return Err(PlanetaryPresenceStateError::UnknownForceDefinition {
                    planet_id: presence.planet_id,
                    definition_id: force.definition_id,
                });
            }
            if !force_ids.insert(force.definition_id) {
                return Err(PlanetaryPresenceStateError::DuplicateForce {
                    planet_id: presence.planet_id,
                    definition_id: force.definition_id,
                });
            }
            if force.quantity == 0 {
                return Err(PlanetaryPresenceStateError::EmptyForce {
                    planet_id: presence.planet_id,
                    definition_id: force.definition_id,
                });
            }
        }
    }
    for system in &universe.definition().systems {
        for planet in &system.planets {
            if !presence_ids.contains(&planet.id) {
                return Err(PlanetaryPresenceStateError::MissingPresence(planet.id));
            }
        }
    }
    for colony in &state.colonies {
        let Some(presence) = state.planetary_presence(colony.planet_id) else {
            return Err(PlanetaryPresenceStateError::MissingPresence(
                colony.planet_id,
            ));
        };
        if presence.occupant != colony.owner {
            return Err(PlanetaryPresenceStateError::ColonyOccupantMismatch(
                colony.planet_id,
            ));
        }
    }

    let mut intelligence_ids = BTreeSet::new();
    for report in &state.planetary_intelligence_reports {
        if !intelligence_ids.insert(report.planet_id) {
            return Err(PlanetaryPresenceStateError::DuplicateIntelligence(
                report.planet_id,
            ));
        }
        if universe.planet(report.planet_id).is_none() {
            return Err(PlanetaryPresenceStateError::UnknownIntelligencePlanet(
                report.planet_id,
            ));
        }
        let current = state.planet_knowledge_level(report.planet_id);
        if current < KnowledgeLevel::Probed {
            return Err(PlanetaryPresenceStateError::IntelligenceKnowledgeTooLow {
                planet_id: report.planet_id,
                current,
            });
        }
        let expected_precision = intelligence_precision_for_knowledge(current)
            .expect("knowledge at or above probed has a precision");
        if report.precision < expected_precision {
            return Err(PlanetaryPresenceStateError::IntelligencePrecisionTooLow {
                planet_id: report.planet_id,
                expected: expected_precision,
                found: report.precision,
            });
        }
        if report.observed_at > state.clock.current_tick() {
            return Err(PlanetaryPresenceStateError::IntelligenceInFuture(
                report.planet_id,
            ));
        }
        if !report.ground_strength.is_valid() || !report.orbital_strength.is_valid() {
            return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                report.planet_id,
            ));
        }
        match (report.precision, report.occupancy) {
            (
                PlanetaryIntelPrecision::Contact,
                PlanetaryOccupancyIntel::Unoccupied | PlanetaryOccupancyIntel::OccupiedUnknown,
            ) if report.population.is_none() && report.forces.is_empty() => {}
            (
                PlanetaryIntelPrecision::Surveyed | PlanetaryIntelPrecision::Exact,
                PlanetaryOccupancyIntel::Unoccupied,
            ) if report.population.is_none() && report.forces.is_empty() => {}
            (
                PlanetaryIntelPrecision::Surveyed | PlanetaryIntelPrecision::Exact,
                PlanetaryOccupancyIntel::Occupied(faction_id),
            ) if report.population.is_some() => {
                if state.faction(faction_id).is_none() {
                    return Err(PlanetaryPresenceStateError::UnknownIntelligenceFaction {
                        planet_id: report.planet_id,
                        faction_id,
                    });
                }
            }
            _ => {
                return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                    report.planet_id,
                ));
            }
        }
        if report
            .population
            .is_some_and(|population| !population.is_valid())
        {
            return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                report.planet_id,
            ));
        }
        let mut force_ids = BTreeSet::new();
        for force in &report.forces {
            if rules.definition(force.definition_id).is_none() {
                return Err(PlanetaryPresenceStateError::UnknownIntelligenceDefinition {
                    planet_id: report.planet_id,
                    definition_id: force.definition_id,
                });
            }
            if !force_ids.insert(force.definition_id) {
                return Err(PlanetaryPresenceStateError::DuplicateIntelligenceForce {
                    planet_id: report.planet_id,
                    definition_id: force.definition_id,
                });
            }
            if !force.quantity.is_valid() {
                return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                    report.planet_id,
                ));
            }
            if report.precision == PlanetaryIntelPrecision::Exact && !force.quantity.is_exact() {
                return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                    report.planet_id,
                ));
            }
        }
        if report.precision == PlanetaryIntelPrecision::Exact
            && (!report.ground_strength.is_exact()
                || !report.orbital_strength.is_exact()
                || report
                    .population
                    .is_some_and(|population| !population.is_exact()))
        {
            return Err(PlanetaryPresenceStateError::InvalidIntelligence(
                report.planet_id,
            ));
        }
    }
    for knowledge in &state.planet_knowledge {
        if knowledge.level >= KnowledgeLevel::Probed
            && !intelligence_ids.contains(&knowledge.planet_id)
        {
            return Err(PlanetaryPresenceStateError::MissingIntelligence(
                knowledge.planet_id,
            ));
        }
    }
    Ok(())
}

pub const fn intelligence_precision_for_knowledge(
    knowledge: KnowledgeLevel,
) -> Option<PlanetaryIntelPrecision> {
    match knowledge {
        KnowledgeLevel::Unknown | KnowledgeLevel::Detected => None,
        KnowledgeLevel::Probed => Some(PlanetaryIntelPrecision::Contact),
        KnowledgeLevel::Analyzed => Some(PlanetaryIntelPrecision::Surveyed),
        KnowledgeLevel::Colonized => Some(PlanetaryIntelPrecision::Exact),
    }
}

fn compile_fixed_forces(
    configured: Vec<FixedForceConfig>,
    ids: &BTreeMap<String, PlanetaryForceId>,
    definitions: &BTreeMap<PlanetaryForceId, PlanetaryForceDefinition>,
) -> Result<Vec<PlanetaryForceStack>, PlanetaryPresenceRulesError> {
    let mut seen = BTreeSet::new();
    let mut forces = Vec::with_capacity(configured.len());
    for configured in configured {
        let Some(definition_id) = ids.get(&configured.id).copied() else {
            return Err(PlanetaryPresenceRulesError::InvalidFixedForce);
        };
        if configured.quantity == 0
            || !seen.insert(definition_id)
            || !definitions.contains_key(&definition_id)
        {
            return Err(PlanetaryPresenceRulesError::InvalidFixedForce);
        }
        forces.push(PlanetaryForceStack {
            definition_id,
            quantity: configured.quantity,
        });
    }
    forces.sort_by_key(|force| force.definition_id);
    Ok(forces)
}

fn compile_profile_forces(
    configured: Vec<ProfileForceConfig>,
    ids: &BTreeMap<String, PlanetaryForceId>,
    definitions: &BTreeMap<PlanetaryForceId, PlanetaryForceDefinition>,
) -> Result<Vec<ProfileForce>, PlanetaryPresenceRulesError> {
    let mut seen = BTreeSet::new();
    let mut forces = Vec::with_capacity(configured.len());
    for configured in configured {
        let Some(definition_id) = ids.get(&configured.id).copied() else {
            return Err(PlanetaryPresenceRulesError::InvalidProfileForce);
        };
        if configured.minimum == 0
            || configured.maximum < configured.minimum
            || !seen.insert(definition_id)
            || !definitions.contains_key(&definition_id)
        {
            return Err(PlanetaryPresenceRulesError::InvalidProfileForce);
        }
        forces.push(ProfileForce {
            definition_id,
            minimum: configured.minimum,
            maximum: configured.maximum,
        });
    }
    forces.sort_by_key(|force| force.definition_id);
    Ok(forces)
}

fn varied_inclusive(minimum: u64, maximum: u64, seed: u64) -> u64 {
    if minimum == maximum {
        return minimum;
    }
    let width = maximum.saturating_sub(minimum).saturating_add(1);
    minimum.saturating_add(splitmix64(seed) % width)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err(());
    };
    if !first.is_ascii_lowercase() {
        return Err(());
    }
    if characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlanetaryPresenceRulesConfig {
    version: u32,
    population_estimate_step: u64,
    strength_estimate_step: u64,
    home: HomePresenceConfig,
    units: Vec<PlanetaryForceDefinitionConfig>,
    profiles: Vec<PlanetaryPresenceProfileConfig>,
}

#[derive(Debug, Deserialize)]
struct HomePresenceConfig {
    population: u64,
    forces: Vec<FixedForceConfig>,
}

#[derive(Debug, Deserialize)]
struct FixedForceConfig {
    id: String,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
struct PlanetaryForceDefinitionConfig {
    id: String,
    name: String,
    domain: PlanetaryForceDomain,
    offense: u32,
    defense: u32,
    durability: u32,
    estimate_step: u32,
}

#[derive(Debug, Deserialize)]
struct PlanetaryPresenceProfileConfig {
    id: String,
    weight: u32,
    occupant_faction_id: Option<u64>,
    minimum_population: u64,
    maximum_population: u64,
    forces: Vec<ProfileForceConfig>,
}

#[derive(Debug, Deserialize)]
struct ProfileForceConfig {
    id: String,
    minimum: u32,
    maximum: u32,
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;

    use crate::{GameAction, Simulation};

    use super::*;

    #[test]
    fn default_presence_is_seeded_deterministically() {
        let left = Simulation::new(UniverseConfig::mvp());
        let right = Simulation::new(UniverseConfig::mvp());

        assert_eq!(
            left.state().planetary_presences,
            right.state().planetary_presences,
        );
        assert!(
            left.state()
                .planetary_presences
                .iter()
                .any(|presence| presence.occupant == Owner::Unowned),
        );
        assert!(
            left.state()
                .planetary_presences
                .iter()
                .any(|presence| presence.occupant == Owner::Faction(FactionId::new(1))),
        );
        assert!(
            left.state()
                .planetary_presences
                .iter()
                .any(|presence| presence.occupant == Owner::Faction(FactionId::new(2))),
        );
    }

    #[test]
    fn contact_and_survey_reports_never_expose_exact_force_counts() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                presence.occupant != Owner::Unowned
                    && !presence.forces.is_empty()
                    && simulation
                        .state()
                        .colony_on_planet(presence.planet_id)
                        .is_none()
            })
            .expect("the MVP seed contains a remote occupied planet")
            .planet_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        let contact = refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Contact,
            StrategicTick::ZERO,
        )
        .expect("contact report");
        assert_eq!(contact.occupancy, PlanetaryOccupancyIntel::OccupiedUnknown,);
        assert!(contact.population.is_none());
        assert!(contact.forces.is_empty());

        let surveyed = refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Surveyed,
            StrategicTick::ZERO,
        )
        .expect("survey report");
        assert!(matches!(
            surveyed.occupancy,
            PlanetaryOccupancyIntel::Occupied(_),
        ));
        assert!(
            surveyed
                .population
                .expect("occupied population estimate")
                .maximum
                > surveyed.population.expect("population estimate").minimum,
        );
        assert!(
            surveyed
                .forces
                .iter()
                .all(|force| !force.quantity.is_exact()),
        );
    }

    #[test]
    fn force_losses_are_atomic_and_leave_old_intelligence_unchanged() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let (target, definition_id, available) = simulation
            .state()
            .planetary_presences
            .iter()
            .find_map(|presence| {
                let force = presence.forces.first()?;
                (force.quantity > 1).then_some((
                    presence.planet_id,
                    force.definition_id,
                    force.quantity,
                ))
            })
            .expect("the MVP seed contains a force stack");
        let report = refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Surveyed,
            StrategicTick::ZERO,
        )
        .expect("survey report");

        let rejected = apply_planetary_force_losses(
            simulation.state_mut(),
            target,
            &[
                PlanetaryForceLoss {
                    definition_id,
                    quantity: 1,
                },
                PlanetaryForceLoss {
                    definition_id,
                    quantity: 1,
                },
            ],
        );
        assert_eq!(
            rejected,
            Err(PlanetaryPresenceUpdateError::DuplicateLoss(definition_id,)),
        );
        assert_eq!(
            simulation
                .state()
                .planetary_presence(target)
                .expect("presence remains")
                .forces[0]
                .quantity,
            available,
        );

        let update = apply_planetary_force_losses(
            simulation.state_mut(),
            target,
            &[PlanetaryForceLoss {
                definition_id,
                quantity: 1,
            }],
        )
        .expect("valid loss");
        assert_eq!(update.previous_revision, 0);
        assert_eq!(update.current_revision, 1);
        assert_eq!(
            simulation.state().planetary_intelligence_report(target),
            Some(&report),
        );
    }

    #[test]
    fn probe_action_path_creates_a_contact_report() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|knowledge| knowledge.level == KnowledgeLevel::Detected)
            .expect("home system has detected planets")
            .planet_id;
        let system_id = target.system_id();
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id: target,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);

        assert_eq!(
            simulation
                .state()
                .planetary_intelligence_report(target)
                .expect("probe creates intelligence")
                .precision,
            PlanetaryIntelPrecision::Contact,
        );
    }
}

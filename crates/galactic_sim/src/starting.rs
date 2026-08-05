// MVP-019: configurable factions, relations, ownership and starting state.
use std::collections::BTreeSet;

use galactic_domain::{ColonyId, EnergyGrid, FactionId, Owner, PlanetId, ResourceStock, SystemId};

use crate::{
    BuildingLevels, DiplomacyError, DiplomacyState, DiplomaticRelation, FactionKind,
    FactionRelation, KnowledgeLevel, TechnologyId, UniverseRepository, default_ruleset,
};

pub const MVP_HOME_SYSTEM_ID: SystemId = SystemId::from_index(0);
pub const MVP_HOME_PLANET_ID: PlanetId = PlanetId::from_system_index(MVP_HOME_SYSTEM_ID, 0);
pub const MVP_PLAYER_FACTION_ID: FactionId = FactionId::new(0);
pub const MVP_NEUTRAL_FACTION_ID: FactionId = FactionId::new(1);
pub const MVP_FUTURE_AI_FACTION_ID: FactionId = FactionId::new(2);
pub const MVP_HOME_COLONY_ID: ColonyId = ColonyId::new(0);
pub const MVP_MIN_HOME_HABITABILITY: u8 = 80;

pub const MVP_INITIAL_SYSTEM_KNOWLEDGE: [InitialSystemKnowledge; 1] = [InitialSystemKnowledge {
    system_id: MVP_HOME_SYSTEM_ID,
    level: KnowledgeLevel::Colonized,
}];

pub const MVP_INITIAL_PLANET_KNOWLEDGE: [InitialPlanetKnowledge; 1] = [InitialPlanetKnowledge {
    planet_id: MVP_HOME_PLANET_ID,
    level: KnowledgeLevel::Colonized,
}];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanetResourceProfile {
    /// Relative production potential, where 100 is the balanced baseline.
    pub metal: u16,
    pub crystal: u16,
    pub fuel: u16,
    pub energy: u16,
}

impl PlanetResourceProfile {
    pub const BALANCED: Self = Self::new(100, 100, 100, 100);

    pub const fn new(metal: u16, crystal: u16, fuel: u16, energy: u16) -> Self {
        Self {
            metal,
            crystal,
            fuel,
            energy,
        }
    }

    pub const fn is_viable(self) -> bool {
        self.metal > 0 && self.crystal > 0 && self.fuel > 0 && self.energy > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingFactionConfig {
    pub id: FactionId,
    pub name: &'static str,
    pub kind: FactionKind,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingColonyConfig {
    pub id: ColonyId,
    pub name: &'static str,
    pub owner: Owner,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub initial_stock: ResourceStock,
    pub initial_energy: EnergyGrid,
    pub buildings: BuildingLevels,
    pub resource_profile: PlanetResourceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialSystemKnowledge {
    pub system_id: SystemId,
    pub level: KnowledgeLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitialPlanetKnowledge {
    pub planet_id: PlanetId,
    pub level: KnowledgeLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingScenario {
    pub factions: &'static [StartingFactionConfig],
    pub default_relation: DiplomaticRelation,
    pub initial_relations: &'static [FactionRelation],
    pub player_faction_id: FactionId,
    pub home_colony: StartingColonyConfig,
    pub initial_system_knowledge: &'static [InitialSystemKnowledge],
    pub initial_planet_knowledge: &'static [InitialPlanetKnowledge],
    pub initial_technologies: &'static [TechnologyId],
    pub minimum_home_habitability: u8,
}

impl StartingScenario {
    pub fn mvp() -> Self {
        default_ruleset().starting_scenario()
    }

    pub fn validate(self, universe: &UniverseRepository) -> Result<(), StartingScenarioError> {
        if self.factions.is_empty() {
            return Err(StartingScenarioError::MissingFactions);
        }
        let mut faction_ids = BTreeSet::new();
        let mut player_count = 0;
        for faction in self.factions {
            if !faction_ids.insert(faction.id) {
                return Err(StartingScenarioError::DuplicateFaction(faction.id));
            }
            if faction.name.trim().is_empty() {
                return Err(StartingScenarioError::EmptyFactionName(faction.id));
            }
            if faction.kind == FactionKind::Player {
                player_count += 1;
            }
        }
        for relation in self.initial_relations {
            if !faction_ids.contains(&relation.first) {
                return Err(StartingScenarioError::UnknownRelationFaction(
                    relation.first,
                ));
            }
            if !faction_ids.contains(&relation.second) {
                return Err(StartingScenarioError::UnknownRelationFaction(
                    relation.second,
                ));
            }
        }
        DiplomacyState::new(
            self.default_relation,
            self.initial_relations.iter().copied(),
        )
        .map_err(StartingScenarioError::InvalidDiplomacy)?;
        if player_count != 1 {
            return Err(StartingScenarioError::InvalidPlayerFactionCount(
                player_count,
            ));
        }
        let Some(player_faction) = self
            .factions
            .iter()
            .find(|faction| faction.id == self.player_faction_id)
        else {
            return Err(StartingScenarioError::UnknownPlayerFaction(
                self.player_faction_id,
            ));
        };
        if player_faction.kind != FactionKind::Player {
            return Err(StartingScenarioError::PlayerFactionKindMismatch(
                self.player_faction_id,
            ));
        }
        if !player_faction.active {
            return Err(StartingScenarioError::InactivePlayerFaction(
                self.player_faction_id,
            ));
        }
        if self.home_colony.owner != Owner::Faction(self.player_faction_id) {
            return Err(StartingScenarioError::HomeColonyNotPlayerOwned);
        }
        if self.home_colony.name.trim().is_empty() {
            return Err(StartingScenarioError::EmptyColonyName);
        }
        if !self.home_colony.resource_profile.is_viable() {
            return Err(StartingScenarioError::InvalidResourceProfile);
        }
        if self.home_colony.initial_energy.is_deficit() {
            return Err(StartingScenarioError::InitialEnergyDeficit);
        }
        let expected_energy =
            crate::default_building_catalog().energy_grid_for_levels(self.home_colony.buildings);
        if self.home_colony.initial_energy != expected_energy {
            return Err(StartingScenarioError::InitialEnergyCatalogMismatch {
                expected: expected_energy,
                found: self.home_colony.initial_energy,
            });
        }
        crate::default_building_catalog()
            .validate_levels(self.home_colony.buildings)
            .map_err(|_| StartingScenarioError::InvalidBuildingLevels)?;

        let Some(system) = universe.system(self.home_colony.system_id) else {
            return Err(StartingScenarioError::UnknownHomeSystem(
                self.home_colony.system_id,
            ));
        };
        let Some(planet) = universe.planet(self.home_colony.planet_id) else {
            return Err(StartingScenarioError::UnknownHomePlanet(
                self.home_colony.planet_id,
            ));
        };
        if planet.id.system_id() != system.id {
            return Err(StartingScenarioError::HomePlanetSystemMismatch {
                system_id: system.id,
                planet_id: planet.id,
            });
        }
        if planet.habitability < self.minimum_home_habitability {
            return Err(StartingScenarioError::InsufficientHabitability {
                required: self.minimum_home_habitability,
                found: planet.habitability,
            });
        }

        for knowledge in self.initial_system_knowledge {
            if knowledge.level == KnowledgeLevel::Unknown {
                return Err(StartingScenarioError::ExplicitUnknownKnowledge);
            }
            if universe.system(knowledge.system_id).is_none() {
                return Err(StartingScenarioError::UnknownInitialSystem(
                    knowledge.system_id,
                ));
            }
        }

        for knowledge in self.initial_planet_knowledge {
            if knowledge.level == KnowledgeLevel::Unknown {
                return Err(StartingScenarioError::ExplicitUnknownKnowledge);
            }
            if universe.planet(knowledge.planet_id).is_none() {
                return Err(StartingScenarioError::UnknownInitialPlanet(
                    knowledge.planet_id,
                ));
            }
        }

        let home_system_level = self
            .initial_system_knowledge
            .iter()
            .find(|entry| entry.system_id == self.home_colony.system_id)
            .map(|entry| entry.level)
            .unwrap_or_default();
        if home_system_level != KnowledgeLevel::Colonized {
            return Err(StartingScenarioError::HomeSystemNotColonized);
        }

        let home_planet_level = self
            .initial_planet_knowledge
            .iter()
            .find(|entry| entry.planet_id == self.home_colony.planet_id)
            .map(|entry| entry.level)
            .unwrap_or_default();
        if home_planet_level != KnowledgeLevel::Colonized {
            return Err(StartingScenarioError::HomePlanetNotColonized);
        }

        Ok(())
    }
}

impl Default for StartingScenario {
    fn default() -> Self {
        Self::mvp()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartingScenarioError {
    MissingFactions,
    DuplicateFaction(FactionId),
    EmptyFactionName(FactionId),
    UnknownRelationFaction(FactionId),
    InvalidDiplomacy(DiplomacyError),
    InvalidPlayerFactionCount(usize),
    UnknownPlayerFaction(FactionId),
    PlayerFactionKindMismatch(FactionId),
    InactivePlayerFaction(FactionId),
    HomeColonyNotPlayerOwned,
    EmptyColonyName,
    InvalidResourceProfile,
    InitialEnergyDeficit,
    InitialEnergyCatalogMismatch {
        expected: EnergyGrid,
        found: EnergyGrid,
    },
    InvalidBuildingLevels,
    ExplicitUnknownKnowledge,
    UnknownHomeSystem(SystemId),
    UnknownHomePlanet(PlanetId),
    HomePlanetSystemMismatch {
        system_id: SystemId,
        planet_id: PlanetId,
    },
    InsufficientHabitability {
        required: u8,
        found: u8,
    },
    UnknownInitialSystem(SystemId),
    UnknownInitialPlanet(PlanetId),
    HomeSystemNotColonized,
    HomePlanetNotColonized,
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;

    use super::*;

    #[test]
    fn mvp_starting_scenario_matches_reference_universe() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());

        assert_eq!(StartingScenario::mvp().validate(&universe), Ok(()));
    }

    #[test]
    fn starting_data_is_configurable_without_mutating_universe() {
        let universe = UniverseRepository::generate(UniverseConfig::mvp());
        let fingerprint = universe.definition().generation_fingerprint;
        let mut scenario = StartingScenario::mvp();
        scenario.home_colony.initial_stock = ResourceStock::new(999, 888, 777);
        scenario
            .home_colony
            .buildings
            .set_level(crate::BuildingKind::RESEARCH_LAB, 1);
        scenario.home_colony.initial_energy = crate::default_building_catalog()
            .energy_grid_for_levels(scenario.home_colony.buildings);

        assert_eq!(scenario.validate(&universe), Ok(()));
        assert_eq!(universe.definition().generation_fingerprint, fingerprint);
    }
}

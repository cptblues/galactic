// MVP-009: configurable starting knowledge and home-world state
use galactic_domain::{ColonyId, FactionId, PlanetId, ResourceStock, SystemId};

use crate::{KnowledgeLevel, UniverseRepository};

pub const MVP_HOME_SYSTEM_ID: SystemId = SystemId::from_index(0);
pub const MVP_HOME_PLANET_ID: PlanetId = PlanetId::from_system_index(MVP_HOME_SYSTEM_ID, 0);
pub const MVP_PLAYER_FACTION_ID: FactionId = FactionId::new(0);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildingKind {
    MetalMine,
    CrystalExtractor,
    FuelRefinery,
    PowerPlant,
    Warehouse,
    ConstructionCenter,
    ResearchLab,
    Shipyard,
}

impl BuildingKind {
    pub const ALL: [Self; 8] = [
        Self::MetalMine,
        Self::CrystalExtractor,
        Self::FuelRefinery,
        Self::PowerPlant,
        Self::Warehouse,
        Self::ConstructionCenter,
        Self::ResearchLab,
        Self::Shipyard,
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildingLevels {
    pub metal_mine: u8,
    pub crystal_extractor: u8,
    pub fuel_refinery: u8,
    pub power_plant: u8,
    pub warehouse: u8,
    pub construction_center: u8,
    pub research_lab: u8,
    pub shipyard: u8,
}

impl BuildingLevels {
    pub const EMPTY: Self = Self {
        metal_mine: 0,
        crystal_extractor: 0,
        fuel_refinery: 0,
        power_plant: 0,
        warehouse: 0,
        construction_center: 0,
        research_lab: 0,
        shipyard: 0,
    };

    pub const MVP_START: Self = Self {
        metal_mine: 1,
        crystal_extractor: 1,
        fuel_refinery: 1,
        power_plant: 1,
        warehouse: 1,
        construction_center: 1,
        research_lab: 0,
        shipyard: 0,
    };

    pub const fn level(self, kind: BuildingKind) -> u8 {
        match kind {
            BuildingKind::MetalMine => self.metal_mine,
            BuildingKind::CrystalExtractor => self.crystal_extractor,
            BuildingKind::FuelRefinery => self.fuel_refinery,
            BuildingKind::PowerPlant => self.power_plant,
            BuildingKind::Warehouse => self.warehouse,
            BuildingKind::ConstructionCenter => self.construction_center,
            BuildingKind::ResearchLab => self.research_lab,
            BuildingKind::Shipyard => self.shipyard,
        }
    }

    pub fn total_levels(self) -> u32 {
        BuildingKind::ALL
            .into_iter()
            .map(|kind| u32::from(self.level(kind)))
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartingColonyConfig {
    pub id: ColonyId,
    pub name: &'static str,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub initial_stock: ResourceStock,
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
    pub player_faction: StartingFactionConfig,
    pub home_colony: StartingColonyConfig,
    pub initial_system_knowledge: &'static [InitialSystemKnowledge],
    pub initial_planet_knowledge: &'static [InitialPlanetKnowledge],
    pub minimum_home_habitability: u8,
}

impl StartingScenario {
    pub const fn mvp() -> Self {
        Self {
            player_faction: StartingFactionConfig {
                id: MVP_PLAYER_FACTION_ID,
                name: "Aster Expedition",
            },
            home_colony: StartingColonyConfig {
                id: MVP_HOME_COLONY_ID,
                name: "Aster Prime Colony",
                system_id: MVP_HOME_SYSTEM_ID,
                planet_id: MVP_HOME_PLANET_ID,
                initial_stock: ResourceStock::new(600, 300, 220, 80),
                buildings: BuildingLevels::MVP_START,
                resource_profile: PlanetResourceProfile::BALANCED,
            },
            initial_system_knowledge: &MVP_INITIAL_SYSTEM_KNOWLEDGE,
            initial_planet_knowledge: &MVP_INITIAL_PLANET_KNOWLEDGE,
            minimum_home_habitability: MVP_MIN_HOME_HABITABILITY,
        }
    }

    pub fn validate(self, universe: &UniverseRepository) -> Result<(), StartingScenarioError> {
        if self.player_faction.name.trim().is_empty() {
            return Err(StartingScenarioError::EmptyFactionName);
        }
        if self.home_colony.name.trim().is_empty() {
            return Err(StartingScenarioError::EmptyColonyName);
        }
        if !self.home_colony.resource_profile.is_viable() {
            return Err(StartingScenarioError::InvalidResourceProfile);
        }

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
    EmptyFactionName,
    EmptyColonyName,
    InvalidResourceProfile,
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
        scenario.home_colony.initial_stock = ResourceStock::new(999, 888, 777, 66);
        scenario.home_colony.buildings.research_lab = 1;

        assert_eq!(scenario.validate(&universe), Ok(()));
        assert_eq!(universe.definition().generation_fingerprint, fingerprint);
    }
}

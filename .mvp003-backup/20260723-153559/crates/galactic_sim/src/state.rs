use galactic_domain::{
    ColonyId, FactionId, PlanetId, ResourceStock, SystemId, UniverseConfig, UniverseDefinition,
    generate_universe,
};

use crate::{SelectionTarget, TimeSpeed};

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub universe: UniverseDefinition,
    pub player_faction: FactionId,
    pub colonies: Vec<ColonyState>,
    pub known_systems: Vec<SystemId>,
    pub selected: SelectionTarget,
    pub elapsed_seconds: f32,
    pub speed: TimeSpeed,
}

impl GameState {
    pub fn new(config: UniverseConfig) -> Self {
        let universe = generate_universe(config);
        let home_system_id = SystemId::new(0);
        let home_planet_id = PlanetId::new(0);
        let player_faction = FactionId::new(0);
        let mut known_systems = vec![home_system_id];
        known_systems.extend(universe.neighboring_systems(home_system_id));
        known_systems.sort();
        known_systems.dedup();

        Self {
            universe,
            player_faction,
            colonies: vec![ColonyState {
                id: ColonyId::new(0),
                name: "Aster Prime Colony".to_string(),
                faction: player_faction,
                system_id: home_system_id,
                planet_id: home_planet_id,
                stock: ResourceStock::new(120, 45, 80, 30),
            }],
            known_systems,
            selected: SelectionTarget::System(home_system_id),
            elapsed_seconds: 0.0,
            speed: TimeSpeed::X1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColonyState {
    pub id: ColonyId,
    pub name: String,
    pub faction: FactionId,
    pub system_id: SystemId,
    pub planet_id: PlanetId,
    pub stock: ResourceStock,
}

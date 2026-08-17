// Dev-only shortcut for playtesting without the full build-up grind.
use crate::{
    FleetComposition, FleetCreated, FleetError, GameState, ResearchState, ShipStack,
    default_building_catalog, default_ruleset, form_fleet, technology_catalog,
};

/// Maxes out the player's home colony (every building at its ruleset-defined
/// maximum level), unlocks every technology, and forms a fleet of 5 units of
/// every ship-type craftable — all bypassing the normal construction/craft/
/// research queues.
///
/// Intended only for `--dev-combat`-style debug launches; never called from
/// normal gameplay.
pub fn seed_debug_scenario(state: &mut GameState) -> Result<FleetCreated, FleetError> {
    const SHIPS_PER_TYPE: u64 = 5;

    let colony_id = state.colonies[0].id;
    let actor = state.player_faction;

    state.research = ResearchState::from_completed(technology_catalog().ids());

    let colony = state
        .colony_mut(colony_id)
        .expect("the starting scenario always has a home colony");
    for building in default_building_catalog().definitions() {
        colony
            .buildings
            .set_level(building.kind, building.max_level);
    }

    let mut stacks = Vec::new();
    for craftable in default_ruleset().craftables().definitions() {
        if craftable.ship.is_some() {
            colony.inventory.add(craftable.id, SHIPS_PER_TYPE);
            stacks.push(ShipStack::new(craftable.id, SHIPS_PER_TYPE));
        }
    }

    let composition = FleetComposition::from_stacks(stacks)
        .expect("every ship craftable in the ruleset forms a valid fleet composition");

    form_fleet(state, actor, colony_id, composition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildingKind, GameState, UniverseRepository};
    use galactic_domain::{MVP_UNIVERSE_SEED, UniverseConfig, UniverseScalePreset};

    fn test_state() -> GameState {
        let universe = UniverseRepository::generate(UniverseConfig::for_preset(
            MVP_UNIVERSE_SEED,
            UniverseScalePreset::Test,
        ));
        GameState::new(&universe)
    }

    #[test]
    fn seeding_the_debug_scenario_maxes_buildings_research_and_grants_one_fleet_per_ship_type() {
        let mut state = test_state();
        let ship_type_count = default_ruleset()
            .craftables()
            .definitions()
            .filter(|craftable| craftable.ship.is_some())
            .count();
        let technology_count = technology_catalog().ids().count();

        let created = seed_debug_scenario(&mut state).expect("debug scenario must succeed");

        let colony = state.colony(state.colonies[0].id).expect("home colony");
        for building in default_building_catalog().definitions() {
            assert_eq!(colony.buildings.level(building.kind), building.max_level);
        }
        assert_eq!(colony.buildings.level(BuildingKind::SHIPYARD), 5);

        let fleet = state.fleet(created.fleet_id).expect("debug fleet exists");
        assert_eq!(fleet.composition.total_ships(), ship_type_count as u64 * 5);

        assert_eq!(state.research.completed().count(), technology_count);
    }
}

// Dev-only shortcut for playtesting without the full build-up grind.
use std::time::Duration;

use galactic_domain::{Owner, ResourceStock};

use crate::{
    CraftableCategory, FleetComposition, FleetCreated, FleetError, GameAction, GameState,
    KnowledgeLevel, MissionTarget, ResearchState, ShipStack, Simulation, default_building_catalog,
    default_ruleset, form_fleet, intelligence_precision_for_knowledge,
    refresh_planetary_intelligence, storage_capacity, technology_catalog,
};

/// Maxes out the player's home colony (every building at its ruleset-defined
/// maximum level), unlocks every technology, and forms a fleet of 5 units of
/// every ship-type craftable — all bypassing the normal construction/craft/
/// research queues.
///
/// Ships are split into two fleets (combat vs. everything else) rather than
/// one mixed fleet: `FleetCapabilities::cruise_speed` is the *slowest* ship
/// in the fleet, so mixing warships with probes/cargo/colony ships would
/// cripple the combat fleet's speed — see `seed_debug_pending_combat`, which
/// needs the combat fleet to actually reach a target within a bounded time.
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
    let capacity = storage_capacity(colony.buildings);
    colony
        .resources
        .credit_capped(ResourceStock::new(u64::MAX, u64::MAX, u64::MAX), capacity);

    let mut combat_stacks = Vec::new();
    let mut support_stacks = Vec::new();
    for craftable in default_ruleset().craftables().definitions() {
        if craftable.ship.is_none() {
            continue;
        }
        colony.inventory.add(craftable.id, SHIPS_PER_TYPE);
        let stack = ShipStack::new(craftable.id, SHIPS_PER_TYPE);
        if craftable.category == CraftableCategory::Military {
            combat_stacks.push(stack);
        } else {
            support_stacks.push(stack);
        }
    }

    let combat_composition = FleetComposition::from_stacks(combat_stacks)
        .expect("the ruleset's military ships form a valid fleet composition");
    let combat_fleet = form_fleet(state, actor, colony_id, combat_composition)?;

    if !support_stacks.is_empty() {
        let support_composition = FleetComposition::from_stacks(support_stacks)
            .expect("the ruleset's non-military ships form a valid fleet composition");
        form_fleet(state, actor, colony_id, support_composition)?;
    }

    Ok(combat_fleet)
}

/// Launches an attack against the nearest hostile outpost using the fleet
/// `seed_debug_scenario` already formed, and advances time until the attack
/// mission actually arrives and opens a pending combat — so a debug launch
/// can drop straight into the combat screen instead of requiring the player
/// to manually scout and attack first.
///
/// Intended only for debug launches (e.g. a `--dev-combat-encounter` CLI
/// flag); never called from normal gameplay. Panics if the fixed MVP
/// universe has no hostile presence in the home system's neighborhood, which
/// would indicate the universe generation itself is broken.
pub fn seed_debug_pending_combat(simulation: &mut Simulation) {
    let actor = simulation.state().player_faction;
    let colony_id = simulation.state().colonies[0].id;
    let origin = simulation.state().colonies[0].system_id;
    let neighboring_systems = simulation.universe_repository().neighboring_systems(origin);
    let target = simulation
        .state()
        .planetary_presences
        .iter()
        .find(|presence| {
            neighboring_systems.contains(&presence.planet_id.system_id())
                && presence.occupant != Owner::Faction(actor)
                && presence.occupant != Owner::Unowned
                && !presence.forces.is_empty()
        })
        .expect("the home neighborhood always has a hostile outpost in the fixed MVP universe")
        .planet_id;

    let repository = simulation.universe_repository().clone();
    simulation.state_mut().advance_system_knowledge(
        &repository,
        target.system_id(),
        KnowledgeLevel::Probed,
    );
    simulation
        .state_mut()
        .advance_planet_knowledge(&repository, target, KnowledgeLevel::Analyzed);
    let precision = intelligence_precision_for_knowledge(KnowledgeLevel::Analyzed)
        .expect("Analyzed knowledge always maps to an intelligence precision");
    let current_tick = simulation.state().clock.current_tick();
    refresh_planetary_intelligence(simulation.state_mut(), target, precision, current_tick)
        .expect("the target planet has a validated planetary presence");

    simulation.apply_player_action(GameAction::LaunchAttack {
        colony_id,
        target: MissionTarget::Planet {
            system_id: target.system_id(),
            planet_id: target,
        },
    });
    simulation.advance(Duration::from_secs(120));
}

/// Auto-resolves whatever pending combat `seed_debug_pending_combat` just
/// opened, straight to `FinalReport` — for debug launches that want to
/// preview the result screen without manually playing out every round.
/// No-op if there's no pending combat. Debug-only, see
/// `seed_debug_pending_combat`.
pub fn auto_resolve_debug_combat(simulation: &mut Simulation) {
    let Some(mission_id) = simulation
        .state()
        .pending_combats
        .first()
        .map(|pending| pending.mission_id)
    else {
        return;
    };
    simulation.apply_player_action(GameAction::AutoResolveCombat { mission_id });
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
    fn seeding_the_debug_scenario_maxes_buildings_research_and_grants_a_fast_combat_fleet() {
        let mut state = test_state();
        let ship_type_count = default_ruleset()
            .craftables()
            .definitions()
            .filter(|craftable| craftable.ship.is_some())
            .count();
        let combat_ship_type_count = default_ruleset()
            .craftables()
            .definitions()
            .filter(|craftable| craftable.category == CraftableCategory::Military)
            .count();
        let technology_count = technology_catalog().ids().count();

        let created = seed_debug_scenario(&mut state).expect("debug scenario must succeed");

        let colony = state.colony(state.colonies[0].id).expect("home colony");
        for building in default_building_catalog().definitions() {
            assert_eq!(colony.buildings.level(building.kind), building.max_level);
        }
        assert_eq!(colony.buildings.level(BuildingKind::SHIPYARD), 5);

        let combat_fleet = state.fleet(created.fleet_id).expect("combat fleet exists");
        assert_eq!(
            combat_fleet.composition.total_ships(),
            combat_ship_type_count as u64 * 5
        );
        assert_eq!(
            state.fleets.len(),
            2,
            "combat and support ships are split into two fleets"
        );
        let total_ships: u64 = state
            .fleets
            .iter()
            .map(|fleet| fleet.composition.total_ships())
            .sum();
        assert_eq!(total_ships, ship_type_count as u64 * 5);

        assert_eq!(state.research.completed().count(), technology_count);
    }

    #[test]
    fn seeding_a_debug_pending_combat_opens_a_real_combat_decision() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        seed_debug_scenario(simulation.state_mut()).expect("debug scenario must succeed");

        seed_debug_pending_combat(&mut simulation);

        assert_eq!(simulation.state().pending_combats.len(), 1);
    }
}

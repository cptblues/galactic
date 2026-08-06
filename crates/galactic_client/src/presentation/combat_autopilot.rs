// COMBAT-001-C: temporary auto-pilot bridge. No UI exists yet (COMBAT-001-D)
// for the player to choose a tactical doctrine round by round, so this
// system immediately auto-resolves every pending combat the instant it
// appears — keeping real gameplay exactly as it behaved before this ticket,
// on top of the genuinely interactive engine now underneath. D deletes this
// whole file (and its one line of wiring in `lib.rs`) once a real choice
// screen replaces it.

use bevy::prelude::*;
use galactic_domain::MissionId;
use galactic_sim::GameAction;

use crate::SimulationResource;
use crate::presentation::shortcuts::apply_simulation_command;

pub(crate) fn auto_resolve_pending_combats(mut simulation: ResMut<SimulationResource>) {
    let mission_ids: Vec<MissionId> = simulation
        .simulation()
        .state()
        .pending_combats
        .iter()
        .map(|pending| pending.mission_id)
        .collect();
    for mission_id in mission_ids {
        apply_simulation_command(
            &mut simulation,
            GameAction::AutoResolveCombat { mission_id },
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use galactic_domain::{Owner, UniverseConfig};
    use galactic_sim::{
        AttackMissionOutcome, BuildingKind, CraftableId, KnowledgeLevel, MissionResult,
        MissionTarget, ResearchState, Simulation, TechnologyId, default_building_catalog,
    };

    use super::*;

    fn simulation_with_pending_combat() -> Simulation {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_systems = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .to_vec();
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
            .expect("the home neighborhood guarantees a hostile outpost")
            .planet_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Analyzed,
        );
        let precision =
            galactic_sim::intelligence_precision_for_knowledge(KnowledgeLevel::Analyzed)
                .expect("Analyzed knowledge always maps to an intelligence precision");
        let current_tick = simulation.state().clock.current_tick();
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            precision,
            current_tick,
        )
        .expect("the target planet has a validated planetary presence");
        {
            let colony = &mut simulation.state_mut().colonies[0];
            colony
                .buildings
                .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
            colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
            colony
                .buildings
                .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
            colony.buildings.set_level(BuildingKind::SHIPYARD, 1);
            colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
            colony
                .resources
                .credit(galactic_domain::ResourceStock::new(1_000, 1_000, 1_000))
                .expect("the test resources fit the starting storage");
        }
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        for _ in 0..3 {
            simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::FRIGATE_BULWARK,
                quantity: 1,
            });
        }
        simulation.advance(std::time::Duration::from_secs(200));
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        simulation.advance(std::time::Duration::from_secs(120));
        assert_eq!(simulation.state().pending_combats.len(), 1);
        simulation
    }

    #[test]
    fn a_pending_combat_resolves_without_any_manual_command() {
        let simulation = simulation_with_pending_combat();

        let mut app = App::new();
        app.insert_resource(SimulationResource {
            simulation,
            pending_events: Vec::new(),
        })
        .add_systems(bevy::app::Update, auto_resolve_pending_combats);

        app.update();

        let resource = app.world().resource::<SimulationResource>();
        assert!(resource.simulation().state().pending_combats.is_empty());
        assert!(matches!(
            resource.simulation().state().missions[0].result,
            Some(MissionResult::Attack(result))
                if matches!(result.outcome, AttackMissionOutcome::Resolved(_))
        ));
    }
}

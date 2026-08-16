// MVP-023: persist reconnaissance results and progressive discovery frontiers.
#[cfg(test)]
use galactic_sim::Simulation;

mod file;
mod restore;
mod save;
mod settings;
mod snapshot;

pub use file::{
    SaveFileError, SaveFileHeader, SaveSlotMetadata, default_save_directory, delete_save,
    list_save_slots, load_from_path, save_to_path,
};
pub use restore::restore_from_snapshot;
pub use save::{
    ColonySave, FactionSave, FleetSave, MutableGameSave, SAVE_VERSION, SaveError, SaveGame,
    StrategicClockSave, UniverseReference,
};
pub use settings::{GraphicsPreset, default_settings_path, load_settings, save_settings};
pub use snapshot::snapshot_from_simulation;

#[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;

    use galactic_domain::{
        ColonyId, ExtractionSiteId, MissionId, Owner, PlanetId, ResourceStock, UniverseConfig,
    };
    use galactic_sim::{
        BuildingKind, CombatDoctrineId, CraftableId, FleetComposition, GAME_STATE_VERSION,
        GameAction, KnowledgeLevel, MissionKind, MissionOrder, MissionPhase, MissionResult,
        MissionTarget, PlanetaryIntelPrecision, ResearchState, ShipStack, SimulationBuildError,
        StrategicTick, TechnologyId, default_building_catalog,
    };

    use super::*;

    fn simulation_with_launched_attack() -> (Simulation, PlanetId) {
        simulation_with_launched_attack_using(3)
    }

    fn simulation_with_launched_attack_using(ship_count: u64) -> (Simulation, PlanetId) {
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
            KnowledgeLevel::Probed,
        );
        let observed_at = simulation.state().clock.current_tick();
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Contact,
            observed_at,
        )
        .expect("test setup creates contact intelligence");
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
                .credit(ResourceStock::new(1_000, 1_000, 1_000))
                .expect("the test resources fit the starting storage");
        }
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        galactic_sim::analyze_planet(simulation.state_mut(), &repository, actor, target)
            .expect("test setup creates an analyzed report");
        assert_eq!(
            simulation.state().planet_knowledge_level(target),
            KnowledgeLevel::Analyzed
        );
        for _ in 0..ship_count {
            simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::FRIGATE_BULWARK,
                quantity: 1,
            });
        }
        simulation.advance(Duration::from_secs(200));
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::FRIGATE_BULWARK),
            ship_count
        );
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        assert_eq!(simulation.state().missions.len(), 1);
        (simulation, target)
    }

    fn simulation_with_launched_colonization() -> (Simulation, PlanetId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_systems = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .to_vec();
        let rules = galactic_sim::planetary_analysis_rules();
        let target = simulation
            .universe()
            .systems
            .iter()
            .filter(|system| neighboring_systems.contains(&system.id))
            .flat_map(|system| system.planets.iter())
            .find(|planet| {
                rules.rule_for(planet.kind).colonizable
                    && planet.habitability >= rules.minimum_habitability()
            })
            .expect("a neighboring planet supports colonization")
            .id;
        let presence = simulation
            .state_mut()
            .planetary_presence_mut(target)
            .expect("every planet has a presence");
        presence.occupant = Owner::Unowned;
        presence.population = 0;
        presence.forces.clear();
        presence.revision = presence
            .revision
            .checked_add(1)
            .expect("test revision remains representable");

        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        let observed_at = simulation.state().clock.current_tick();
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Contact,
            observed_at,
        )
        .expect("test setup creates contact intelligence");
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::PLANETARY_ANALYSIS,
            TechnologyId::COLONIZATION,
        ]);
        galactic_sim::analyze_planet(simulation.state_mut(), &repository, actor, target)
            .expect("test setup creates an analyzed report");
        {
            let colony = &mut simulation.state_mut().colonies[0];
            colony
                .buildings
                .set_level(BuildingKind::CONSTRUCTION_CENTER, 4);
            colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
            colony
                .buildings
                .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
            colony.buildings.set_level(BuildingKind::WAREHOUSE, 3);
            colony.buildings.set_level(BuildingKind::POWER_PLANT, 3);
            colony.buildings.set_level(BuildingKind::SHIPYARD, 3);
            colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
            colony
                .resources
                .credit(ResourceStock::new(3_000, 2_200, 1_500))
                .expect("test funding fits the configured storage");
        }
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::COLONY_SHIP,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(800));
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::COLONY_SHIP),
            1,
        );
        simulation.apply_player_action(GameAction::LaunchColonization {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        assert_eq!(simulation.state().missions.len(), 1);
        assert_eq!(
            simulation.state().missions[0].phase,
            MissionPhase::Preparation
        );
        assert_eq!(simulation.state().missions[0].owner, Owner::Faction(actor),);
        (simulation, target)
    }

    fn simulation_with_launched_transport() -> (Simulation, MissionId, ResourceStock) {
        let (mut simulation, _) = simulation_with_launched_colonization();
        simulation.advance(Duration::from_secs(123));
        assert_eq!(simulation.state().colonies.len(), 2);
        let origin_colony_id = simulation.state().colonies[0].id;
        let destination_colony_id = simulation.state().colonies[1].id;
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id: origin_colony_id,
            craftable: CraftableId::LIGHT_CARGO,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(80));
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            1,
        );
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("one light cargo is a valid composition");
        let form_events = simulation.apply_player_action(GameAction::FormFleet {
            colony_id: origin_colony_id,
            composition,
        });
        let fleet_id = form_events
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::FleetCreated(created) => Some(created.fleet_id),
                _ => None,
            })
            .expect("the light cargo fleet forms");
        let cargo = ResourceStock::new(200, 150, 100);
        let events = simulation.apply_player_action(GameAction::LaunchTransport {
            origin_colony_id,
            destination_colony_id,
            fleet_id,
            cargo,
        });
        let launched = events
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::MissionLaunched(launched)
                    if launched.kind == MissionKind::Transport =>
                {
                    Some(launched)
                }
                _ => None,
            })
            .expect("the cargo fleet launches");
        (simulation, launched.mission_id, cargo)
    }

    pub(crate) fn simulation_with_launched_harvest() -> (Simulation, MissionId, ExtractionSiteId) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target_system = simulation.universe_repository().neighboring_systems(origin)[0];
        let target = simulation
            .universe()
            .system(target_system)
            .expect("the neighboring system exists")
            .planets[0]
            .id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::CARGO_CAPACITY,
            TechnologyId::REMOTE_EXTRACTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target_system,
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        {
            let colony = &mut simulation.state_mut().colonies[0];
            colony
                .buildings
                .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
            colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
            colony
                .buildings
                .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
            colony.buildings.set_level(BuildingKind::WAREHOUSE, 1);
            colony.buildings.set_level(BuildingKind::POWER_PLANT, 2);
            colony.buildings.set_level(BuildingKind::SHIPYARD, 1);
            colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
            colony
                .resources
                .credit(ResourceStock::new(1_000, 1_000, 1_000))
                .expect("test funding fits the configured storage");
        }
        galactic_sim::analyze_planet(simulation.state_mut(), &repository, actor, target)
            .expect("test setup creates an analyzed report");
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::LIGHT_CARGO,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(80));
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            1,
        );
        let site_id = ExtractionSiteId::for_planet(target);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("one light cargo is a valid composition");
        let form_events = simulation.apply_player_action(GameAction::FormFleet {
            colony_id,
            composition,
        });
        let fleet_id = form_events
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::FleetCreated(created) => Some(created.fleet_id),
                _ => None,
            })
            .expect("the light cargo fleet forms");
        let events = simulation.apply_player_action(GameAction::LaunchHarvest {
            colony_id,
            fleet_id,
            site_id,
        });
        let launched = events
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::MissionLaunched(launched)
                    if launched.kind == MissionKind::Harvest =>
                {
                    Some(launched)
                }
                _ => None,
            })
            .expect("the harvest cargo launches");
        assert_eq!(
            simulation
                .state()
                .extraction_site(site_id)
                .expect("the site exists")
                .reserved_by,
            Some(launched.mission_id),
        );
        assert_eq!(simulation.state().player_faction, actor);
        (simulation, launched.mission_id, site_id)
    }

    #[test]
    fn construction_queue_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
            colony_id,
            kind: BuildingKind::METAL_MINE,
        });
        simulation.advance(Duration::from_secs(2));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .construction_queue
                .len(),
            1
        );
    }

    #[test]
    fn research_queue_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
        colony.buildings.set_level(BuildingKind::RESEARCH_LAB, 1);
        colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);

        simulation.apply_player_action(GameAction::QueueResearch {
            technology: TechnologyId::SPATIAL_DETECTION,
        });
        simulation.advance(Duration::from_secs(12));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("research save is compatible");

        assert_eq!(restored.state().research, simulation.state().research,);
        assert_eq!(restored.state(), simulation.state(),);
    }

    #[test]
    fn planetary_analysis_report_survives_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, _) = simulation
            .universe_repository()
            .planet_location(planet_id)
            .expect("detected planet exists");
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        let repository = simulation.universe_repository().clone();
        let actor = simulation.state().player_faction;
        galactic_sim::analyze_planet(simulation.state_mut(), &repository, actor, planet_id)
            .expect("test setup creates an analyzed report");

        let report = *simulation
            .state()
            .planet_analysis_report(planet_id)
            .expect("analysis creates a persistent report");
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("analysis save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored.state().planet_analysis_report(planet_id),
            Some(&report)
        );
        assert_eq!(
            restored.state().planet_knowledge_level(planet_id),
            KnowledgeLevel::Analyzed
        );
    }

    #[test]
    fn planetary_presence_and_last_intelligence_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let system_id = planet_id.system_id();
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        let presence = simulation
            .state()
            .planetary_presence(planet_id)
            .expect("every planet has a real presence")
            .clone();
        let intelligence = simulation
            .state()
            .planetary_intelligence_report(planet_id)
            .expect("probing creates bounded intelligence")
            .clone();

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("presence save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored.state().planetary_presence(planet_id),
            Some(&presence),
        );
        assert_eq!(
            restored.state().planetary_intelligence_report(planet_id),
            Some(&intelligence),
        );
    }

    #[test]
    fn attack_resumes_identically_and_keeps_its_combat_report() {
        let (mut uninterrupted, target) = simulation_with_launched_attack();
        uninterrupted.advance(Duration::from_secs(3));
        let in_flight = snapshot_from_simulation(&uninterrupted);
        let mut restored =
            restore_from_snapshot(&in_flight).expect("an in-flight attack is compatible");

        assert_eq!(restored.state(), uninterrupted.state());
        uninterrupted.advance(Duration::from_secs(120));
        restored.advance(Duration::from_secs(120));
        assert_eq!(restored.state(), uninterrupted.state());

        // COMBAT-001-C: arrival now opens a pending combat instead of
        // resolving synchronously — auto-resolve it identically on both
        // sessions (mirroring what the client's temporary auto-pilot bridge
        // does for every real player) before asserting they stay identical.
        let mission_id = uninterrupted.state().missions[0].id;
        uninterrupted.apply_player_action(GameAction::AutoResolveCombat { mission_id });
        restored.apply_player_action(GameAction::AutoResolveCombat { mission_id });
        uninterrupted.advance(Duration::from_secs(120));
        restored.advance(Duration::from_secs(120));

        assert_eq!(restored.state(), uninterrupted.state());
        assert_eq!(restored.state().combat_reports.len(), 1);
        assert_eq!(restored.state().combat_reports[0].planet_id, target);
        let resolved = snapshot_from_simulation(&restored);
        let reloaded =
            restore_from_snapshot(&resolved).expect("a resolved combat report is compatible");
        assert_eq!(reloaded.state(), restored.state());
        assert_eq!(
            reloaded.state().combat_reports,
            restored.state().combat_reports
        );
    }

    #[test]
    fn pending_combats_default_to_empty_when_absent_from_an_older_save() {
        // COMBAT-001-C: `pending_combats` is a purely additive field — an
        // older save file (pre-C) simply never had it. Strip it out of a
        // real, freshly-serialized save (mirrors file.rs's synthetic
        // `additive_field_migration_defaults_when_absent_from_an_older_file`
        // test, but against the real `MutableGameSave` shape rather than a
        // toy struct) and confirm it still deserializes, defaulting to
        // empty.
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        assert!(save.state.pending_combats.is_empty());
        let full_ron = ron::ser::to_string(&save).expect("serializes");

        let field = "pending_combats:";
        let start = full_ron
            .find(field)
            .expect("the field is present in a freshly-serialized save");
        let end = full_ron[start..]
            .find(',')
            .map(|offset| start + offset + 1)
            .expect("the field is followed by a comma");
        let mut older_ron = full_ron.clone();
        older_ron.replace_range(start..end, "");
        assert_ne!(older_ron, full_ron);

        let migrated: SaveGame =
            ron::de::from_str(&older_ron).expect("a save missing the field still deserializes");
        assert!(migrated.state.pending_combats.is_empty());
    }

    #[test]
    fn saving_and_reloading_mid_combat_produces_the_same_next_round() {
        // Doc §20 Scenario F: save mid-combat, close the game, reload, same
        // groups, same next result. Proven here through a real RON
        // round-trip (not just an in-memory clone), on both sides of the
        // reload, to a further round that must match bit for bit. A
        // 2-frigate attacker (the fixture's default 3-ship fleet otherwise
        // wins in a single round) keeps the fight close enough to genuinely
        // span multiple rounds — the whole point of this scenario.
        let (mut simulation, target) = simulation_with_launched_attack_using(2);
        simulation.advance(Duration::from_secs(120));
        let mission_id = simulation.state().missions[0].id;
        assert_eq!(simulation.state().pending_combats.len(), 1);

        simulation.apply_player_action(GameAction::ChooseCombatDoctrine {
            mission_id,
            round: 1,
            doctrine: None,
            intervention: None,
        });
        assert_eq!(
            simulation
                .state()
                .pending_combat(mission_id)
                .expect("this fixture's target survives at least one round")
                .planet_id,
            target
        );
        assert_eq!(
            simulation
                .state()
                .pending_combat(mission_id)
                .expect("this fixture's target survives at least one round")
                .command_points_remaining(),
            galactic_sim::combat_rules().command().starting_points()
        );

        let mid_combat = snapshot_from_simulation(&simulation);
        let mut reloaded =
            restore_from_snapshot(&mid_combat).expect("a mid-combat save is compatible");
        assert_eq!(reloaded.state(), simulation.state());

        let uninterrupted_events =
            simulation.apply_player_action(GameAction::ChooseCombatDoctrine {
                mission_id,
                round: 2,
                doctrine: Some(CombatDoctrineId::DefensiveScreen),
                intervention: None,
            });
        let reloaded_events = reloaded.apply_player_action(GameAction::ChooseCombatDoctrine {
            mission_id,
            round: 2,
            doctrine: Some(CombatDoctrineId::DefensiveScreen),
            intervention: None,
        });

        assert_eq!(reloaded.state(), simulation.state());
        assert_eq!(
            uninterrupted_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            reloaded_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn colonization_resumes_identically_and_keeps_its_playable_colony() {
        let (mut uninterrupted, target) = simulation_with_launched_colonization();
        uninterrupted.advance(Duration::from_secs(3));
        assert_eq!(
            uninterrupted.state().missions[0].phase,
            MissionPhase::Outbound
        );
        let in_flight = snapshot_from_simulation(&uninterrupted);
        let mut restored =
            restore_from_snapshot(&in_flight).expect("an in-flight colonization is compatible");

        assert_eq!(restored.state(), uninterrupted.state());
        uninterrupted.advance(Duration::from_secs(120));
        restored.advance(Duration::from_secs(120));

        assert_eq!(restored.state(), uninterrupted.state());
        assert_eq!(restored.state().colony_foundations.len(), 1);
        assert_eq!(restored.state().colony_foundations[0].planet_id, target);
        assert_eq!(restored.state().colonies.len(), 2);
        let established = restored
            .state()
            .colony_on_planet(target)
            .expect("the restored colonization creates a playable colony");
        assert_eq!(established.id, ColonyId::new(1));
        assert_eq!(
            established.founding_mission_id,
            Some(restored.state().missions[0].id),
        );
        assert_eq!(
            established.resources.stock(),
            galactic_sim::planetary_analysis_rules()
                .foundation_cost()
                .as_stock(),
        );
        assert_eq!(restored.state().next_colony_id, 2);
        assert!(matches!(
            restored.state().missions[0].result,
            Some(MissionResult::Colonize(
                galactic_sim::ColonizationMissionResult {
                    outcome: galactic_sim::ColonizationMissionOutcome::FoundationPrepared,
                    colony_ship_consumed: true,
                    ..
                }
            ))
        ));
        let resolved = snapshot_from_simulation(&restored);
        let reloaded =
            restore_from_snapshot(&resolved).expect("a prepared foundation is compatible");
        assert_eq!(reloaded.state(), restored.state());
        assert_eq!(
            reloaded.state().colony_foundations,
            restored.state().colony_foundations,
        );
        assert_eq!(reloaded.state().colonies, restored.state().colonies);
        assert_eq!(
            reloaded.state().next_colony_id,
            restored.state().next_colony_id,
        );
    }

    #[test]
    fn active_colony_selection_survives_round_trip() {
        let (mut simulation, _) = simulation_with_launched_colonization();
        simulation.advance(Duration::from_secs(123));
        assert_eq!(simulation.state().colonies.len(), 2);

        simulation.apply_player_action(GameAction::SelectColony {
            colony_id: ColonyId::new(1),
        });
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("active colony save is compatible");

        assert_eq!(save.state.active_colony_id, Some(ColonyId::new(1)));
        assert_eq!(restored.state().active_colony_id, Some(ColonyId::new(1)));
        assert_eq!(
            restored
                .state()
                .active_player_colony()
                .map(|colony| colony.id),
            Some(ColonyId::new(1)),
        );
        assert_eq!(restored.state(), simulation.state());
    }

    #[test]
    fn transport_cargo_and_phase_resume_without_duplication() {
        let (mut uninterrupted, mission_id, cargo) = simulation_with_launched_transport();
        uninterrupted.advance(Duration::from_secs(1));
        let mission = uninterrupted
            .state()
            .mission(mission_id)
            .expect("the transport mission exists");
        assert_eq!(mission.phase, MissionPhase::Outbound);
        assert_eq!(
            uninterrupted
                .state()
                .fleet(mission.order.fleet_id)
                .expect("the cargo fleet exists")
                .cargo,
            cargo,
        );

        let in_flight = snapshot_from_simulation(&uninterrupted);
        let mut restored =
            restore_from_snapshot(&in_flight).expect("an in-flight transport is compatible");
        assert_eq!(restored.state(), uninterrupted.state());

        uninterrupted.advance(Duration::from_secs(120));
        restored.advance(Duration::from_secs(120));
        assert_eq!(restored.state(), uninterrupted.state());
        let result = restored
            .state()
            .mission(mission_id)
            .and_then(|mission| mission.result);
        assert!(matches!(
            result,
            Some(MissionResult::Transport(
                galactic_sim::TransportMissionResult {
                    requested,
                    delivered,
                    returned,
                    retained,
                    status: galactic_sim::TransportDeliveryStatus::Delivered,
                    ..
                }
            )) if requested == cargo
                && delivered == cargo
                && returned.is_zero()
                && retained.is_zero()
        ));
        let resolved = snapshot_from_simulation(&restored);
        let reloaded =
            restore_from_snapshot(&resolved).expect("a completed transport is compatible");
        assert_eq!(reloaded.state(), restored.state());
    }

    #[test]
    fn harvest_site_cargo_and_phase_resume_without_duplication() {
        let (mut uninterrupted, mission_id, site_id) = simulation_with_launched_harvest();
        let return_departure = uninterrupted
            .state()
            .mission(mission_id)
            .expect("the harvest mission exists")
            .plan
            .return_departure_at;
        let ticks_until_return = return_departure
            .value()
            .saturating_sub(uninterrupted.state().clock.current_tick().value());
        uninterrupted
            .advance(galactic_sim::StrategicDuration::from_ticks(ticks_until_return).as_duration());
        let mission = uninterrupted
            .state()
            .mission(mission_id)
            .expect("the harvest mission exists");
        assert_eq!(mission.phase, MissionPhase::Returning);
        let cargo = uninterrupted
            .state()
            .fleet(mission.order.fleet_id)
            .expect("the returning cargo fleet exists")
            .cargo;
        assert!(!cargo.is_zero());
        let remaining = uninterrupted
            .state()
            .extraction_site(site_id)
            .expect("the harvested site exists")
            .remaining;

        let in_flight = snapshot_from_simulation(&uninterrupted);
        let mut restored =
            restore_from_snapshot(&in_flight).expect("an in-flight harvest is compatible");
        assert_eq!(restored.state(), uninterrupted.state());
        assert_eq!(
            restored
                .state()
                .extraction_site(site_id)
                .expect("the site survives restore")
                .remaining,
            remaining,
        );

        uninterrupted.advance(Duration::from_secs(120));
        restored.advance(Duration::from_secs(120));
        assert_eq!(restored.state(), uninterrupted.state());
        assert!(matches!(
            restored
                .state()
                .mission(mission_id)
                .expect("the harvest mission completes")
                .result,
            Some(MissionResult::Harvest(galactic_sim::HarvestMissionResult {
                collected,
                delivered,
                retained,
                ..
            })) if collected == cargo && delivered == cargo && retained.is_zero()
        ));
        let resolved = snapshot_from_simulation(&restored);
        let reloaded =
            restore_from_snapshot(&resolved).expect("a completed harvest remains compatible");
        assert_eq!(reloaded.state(), restored.state());
    }

    #[test]
    fn unknown_active_colony_is_rejected_during_restore() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.state.active_colony_id = Some(ColonyId::new(999));

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::InvalidState(
                SimulationBuildError::UnknownActiveColony(colony_id),
            )) if colony_id == ColonyId::new(999)
        ));
    }

    #[test]
    fn catalog_changes_are_detected() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.ruleset_structure_fingerprint ^= 1;

        assert!(matches!(
            restore_from_snapshot(&save),
            Err(SaveError::RulesetStructureMismatch { .. })
        ));
    }

    #[test]
    fn craft_queue_and_inventory_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state_mut()
            .colonies
            .first_mut()
            .expect("home colony exists");
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
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("resource credit fits");
        simulation.state_mut().research =
            ResearchState::from_completed([TechnologyId::SPATIAL_DETECTION]);
        let colony_id = simulation.state().colonies[0].id;
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::LIGHT_PROBE,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(12));

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("craft save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(
            restored
                .state()
                .colony(colony_id)
                .expect("colony exists")
                .craft_queue
                .len(),
            1,
        );
    }

    #[test]
    fn faction_data_and_owner_survive_round_trip() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("faction save is compatible");

        assert_eq!(restored.state().factions, simulation.state().factions);
        assert_eq!(restored.state().diplomacy, simulation.state().diplomacy);
        assert_eq!(
            restored.state().colonies[0].owner,
            simulation.state().colonies[0].owner,
        );
        assert_eq!(restored.state().factions.len(), 3);
    }

    #[test]
    fn fleets_and_docked_inventory_survive_round_trip() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
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
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test funding fits");
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
        ]);
        for _ in 0..2 {
            let events = simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::LIGHT_CARGO,
                quantity: 1,
            });
            assert!(matches!(
                events.as_slice(),
                [galactic_sim::GameEvent {
                    kind: galactic_sim::GameEventKind::CraftQueued(_),
                    ..
                }]
            ));
        }
        simulation.advance(Duration::from_secs(160));
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_CARGO, 1)])
                .expect("composition is valid");
        let form_events = simulation.apply_player_action(GameAction::FormFleet {
            colony_id,
            composition,
        });
        let fleet_id = form_events
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::FleetCreated(created) => Some(created.fleet_id),
                _ => None,
            })
            .expect("fleet forms");
        simulation.apply_player_action(GameAction::RenameFleet {
            fleet_id,
            name: "Groupe Sillage".to_string(),
        });

        let save = snapshot_from_simulation(&simulation);
        let restored = restore_from_snapshot(&save).expect("fleet save is compatible");

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().player_fleets().count(), 1);
        assert_eq!(restored.state().fleets[0].owner, Owner::Faction(actor));
        assert_eq!(restored.state().fleets[0].name, "Groupe Sillage");
        assert_eq!(
            restored.state().colonies[0]
                .inventory
                .quantity(CraftableId::LIGHT_CARGO),
            1,
        );
    }

    #[test]
    fn active_mission_resumes_and_completes_at_the_same_tick() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .into_iter()
            .find(|candidate| {
                simulation
                    .universe_repository()
                    .neighboring_systems(*candidate)
                    .into_iter()
                    .any(|neighbor| {
                        simulation.state().system_knowledge_level(neighbor)
                            == KnowledgeLevel::Unknown
                    })
            })
            .expect("one initial signal opens a new frontier");
        let expected_frontier = simulation
            .universe_repository()
            .neighboring_systems(target)
            .into_iter()
            .filter(|neighbor| {
                simulation.state().system_knowledge_level(*neighbor) == KnowledgeLevel::Unknown
            })
            .collect::<Vec<_>>();
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
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test funding fits");
        simulation.state_mut().research =
            ResearchState::from_completed([TechnologyId::SPATIAL_DETECTION]);
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::LIGHT_PROBE,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(50));
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::LIGHT_PROBE, 1)])
                .expect("probe composition is valid");
        let actor = simulation.state().player_faction;
        let created =
            galactic_sim::form_fleet(simulation.state_mut(), actor, colony_id, composition)
                .expect("probe fleet can be formed");
        let departure_at = simulation.state().clock.current_tick();
        simulation.apply_player_action(GameAction::LaunchMission(MissionOrder {
            fleet_id: created.fleet_id,
            origin,
            target: MissionTarget::System(target),
            kind: MissionKind::Probe,
            departure_at,
        }));
        simulation.advance(Duration::from_secs(5));
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::Outbound);

        let save = snapshot_from_simulation(&simulation);
        let mut restored = restore_from_snapshot(&save).expect("active mission save is compatible");
        assert_eq!(restored.state(), simulation.state());

        simulation.advance(Duration::from_secs(16));
        restored.advance(Duration::from_secs(16));

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().missions[0].phase, MissionPhase::Completed);
        assert_eq!(
            restored.state().mission_reports[0].occurred_at,
            StrategicTick::new(701),
        );
        assert_eq!(
            restored.state().system_knowledge_level(target),
            KnowledgeLevel::Probed,
        );
        let Some(MissionResult::Probe(result)) = restored.state().mission_reports[0].result else {
            panic!("completed reconnaissance keeps its frontier result");
        };
        assert_eq!(result.target, MissionTarget::System(target));
        assert_eq!(result.current, KnowledgeLevel::Probed);
        assert_eq!(
            usize::from(result.newly_detected_systems),
            expected_frontier.len(),
        );
        assert_eq!(usize::from(result.revealed_routes), expected_frontier.len(),);
        assert!(expected_frontier.iter().all(|system_id| {
            restored.state().system_knowledge_level(*system_id) == KnowledgeLevel::Detected
        }));
    }

    #[test]
    fn satellite_analysis_mission_survives_restore_during_analysis() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let target = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Probed,
        );
        let observed_at = simulation.state().clock.current_tick();
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            PlanetaryIntelPrecision::Contact,
            observed_at,
        )
        .expect("test setup creates contact intelligence");
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.state_mut().colonies[0]
            .resources
            .credit(ResourceStock::new(1_000, 1_000, 1_000))
            .expect("test fuel fits the ledger");
        simulation.state_mut().colonies[0]
            .buildings
            .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
        simulation.state_mut().colonies[0]
            .buildings
            .set_level(BuildingKind::METAL_MINE, 2);
        simulation.state_mut().colonies[0]
            .buildings
            .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
        simulation.state_mut().colonies[0]
            .buildings
            .set_level(BuildingKind::SHIPYARD, 2);
        let buildings = simulation.state().colonies[0].buildings;
        simulation.state_mut().colonies[0].energy =
            default_building_catalog().energy_grid_for_levels(buildings);
        simulation.apply_player_action(GameAction::QueueCraft {
            colony_id,
            craftable: CraftableId::CARTOGRAPHER_SATELLITE,
            quantity: 1,
        });
        simulation.advance(Duration::from_secs(200));
        assert_eq!(
            simulation.state().colonies[0]
                .inventory
                .quantity(CraftableId::CARTOGRAPHER_SATELLITE),
            1
        );
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::CARTOGRAPHER_SATELLITE, 1)])
                .expect("satellite composition is valid");
        let formed = simulation.apply_player_action(GameAction::FormFleet {
            colony_id,
            composition,
        });
        let fleet_id = formed
            .iter()
            .find_map(|event| match event.kind {
                galactic_sim::GameEventKind::FleetCreated(created) => Some(created.fleet_id),
                _ => None,
            })
            .expect("satellite fleet forms");
        simulation.apply_player_action(GameAction::LaunchMission(MissionOrder {
            fleet_id,
            origin,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
            kind: MissionKind::Analyze,
            departure_at: simulation.state().clock.current_tick(),
        }));
        let plan = simulation.state().missions[0].plan.clone();
        let to_outbound_arrival = plan
            .outbound_arrival_at
            .value()
            .saturating_sub(simulation.state().clock.current_tick().value());
        simulation.advance(Duration::from_millis(
            to_outbound_arrival.saturating_mul(100),
        ));
        assert_eq!(simulation.state().missions[0].phase, MissionPhase::OnSite);
        assert!(simulation.state().planet_analysis_report(target).is_none());

        let save = snapshot_from_simulation(&simulation);
        let mut restored =
            restore_from_snapshot(&save).expect("analysis mission save is compatible");
        assert_eq!(restored.state(), simulation.state());

        let remaining = plan
            .return_arrival_at
            .value()
            .saturating_sub(restored.state().clock.current_tick().value());
        simulation.advance(Duration::from_millis(remaining.saturating_mul(100)));
        restored.advance(Duration::from_millis(remaining.saturating_mul(100)));

        assert_eq!(restored.state(), simulation.state());
        assert_eq!(restored.state().missions[0].phase, MissionPhase::Completed);
        assert_eq!(
            restored.state().planet_knowledge_level(target),
            KnowledgeLevel::Analyzed
        );
        assert!(matches!(
            restored.state().mission_reports[0].result,
            Some(MissionResult::Analyze(_))
        ));
    }

    #[test]
    fn state_and_save_versions_match_mvp_030_a6() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        assert_eq!(save.version, SAVE_VERSION);
        assert_eq!(save.state.version, GAME_STATE_VERSION);
    }
}

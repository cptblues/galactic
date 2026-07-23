#!/usr/bin/env python3
"""
Applique MVP-007 au dépôt Galactic.

Baseline analysée :
    8714f1baf0a2b4ecaf3d208306ca1a7335c2c4a4
    feat mvp 6 add discovered routes

Le script :
- limite l'instanciation aux systèmes connus et à leur frontière détectée ;
- ajoute un niveau de détail sémantique selon le zoom ;
- ajoute navigation, sélection cyclique et recentrage ;
- ajoute un mode debug affichant tout le graphe ;
- ajoute une transition légère Univers -> Système -> Univers ;
- conserve le contexte caméra et la sélection ;
- garde le preset graphique Low.

Usage :
    python tools/apply_mvp_007.py --dry-run
    python tools/apply_mvp_007.py
    python tools/apply_mvp_007.py --skip-checks
    python tools/apply_mvp_007.py --root /chemin/vers/galactic

Le script est idempotent.
"""

from __future__ import annotations

import argparse
import difflib
import shutil
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = "8714f1baf0a2b4ecaf3d208306ca1a7335c2c4a4"

STATE_RS = '// MVP-007: universe visibility is derived from the discovered neighborhood\nuse std::collections::BTreeSet;\n\nuse galactic_domain::{\n    ColonyId, FactionId, PlanetId, ResourceStock, Route, SystemId,\n};\n\nuse crate::{SelectionTarget, StrategicClock, UniverseRepository};\n\n/// Version of the mutable in-memory state contract.\n///\n/// Version 2 replaces floating elapsed seconds with a deterministic tick clock.\npub const GAME_STATE_VERSION: u32 = 2;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum SystemVisibility {\n    Known,\n    Detected,\n}\n\n#[derive(Debug, Clone, PartialEq)]\npub struct GameState {\n    pub version: u32,\n    pub player_faction: FactionId,\n    pub colonies: Vec<ColonyState>,\n    pub known_systems: Vec<SystemId>,\n    pub selected: SelectionTarget,\n    pub clock: StrategicClock,\n}\n\nimpl GameState {\n    pub fn new(universe: &UniverseRepository) -> Self {\n        let home_system_id = SystemId::from_index(0);\n        let home_planet_id = PlanetId::from_system_index(home_system_id, 0);\n        let player_faction = FactionId::new(0);\n        let mut known_systems = vec![home_system_id];\n        known_systems.extend(universe.neighboring_systems(home_system_id));\n        known_systems.sort();\n        known_systems.dedup();\n\n        debug_assert!(universe.system(home_system_id).is_some());\n        debug_assert!(universe.planet(home_planet_id).is_some());\n\n        Self {\n            version: GAME_STATE_VERSION,\n            player_faction,\n            colonies: vec![ColonyState {\n                id: ColonyId::new(0),\n                name: "Aster Prime Colony".to_string(),\n                faction: player_faction,\n                system_id: home_system_id,\n                planet_id: home_planet_id,\n                stock: ResourceStock::new(120, 45, 80, 30),\n            }],\n            known_systems,\n            selected: SelectionTarget::System(home_system_id),\n            clock: StrategicClock::new(),\n        }\n    }\n\n    pub fn colony(&self, id: ColonyId) -> Option<&ColonyState> {\n        self.colonies.iter().find(|colony| colony.id == id)\n    }\n\n    pub fn colony_mut(&mut self, id: ColonyId) -> Option<&mut ColonyState> {\n        self.colonies.iter_mut().find(|colony| colony.id == id)\n    }\n\n    pub fn is_system_known(&self, system_id: SystemId) -> bool {\n        self.known_systems.contains(&system_id)\n    }\n\n    /// Systems directly adjacent to known systems form the current detection\n    /// frontier. This is presentation-oriented until MVP-009 stores explicit\n    /// knowledge levels.\n    pub fn detected_systems(&self, universe: &UniverseRepository) -> Vec<SystemId> {\n        let mut detected = BTreeSet::new();\n\n        for known_system in &self.known_systems {\n            for neighbor in universe.neighboring_systems(*known_system) {\n                if !self.is_system_known(neighbor) {\n                    detected.insert(neighbor);\n                }\n            }\n        }\n\n        detected.into_iter().collect()\n    }\n\n    pub fn system_visibility(\n        &self,\n        universe: &UniverseRepository,\n        system_id: SystemId,\n    ) -> Option<SystemVisibility> {\n        if self.is_system_known(system_id) {\n            return Some(SystemVisibility::Known);\n        }\n\n        self.detected_systems(universe)\n            .binary_search(&system_id)\n            .ok()\n            .map(|_| SystemVisibility::Detected)\n    }\n\n    pub fn visible_systems(\n        &self,\n        universe: &UniverseRepository,\n    ) -> Vec<(SystemId, SystemVisibility)> {\n        let mut systems = self\n            .known_systems\n            .iter()\n            .copied()\n            .map(|system_id| (system_id, SystemVisibility::Known))\n            .collect::<Vec<_>>();\n\n        systems.extend(\n            self.detected_systems(universe)\n                .into_iter()\n                .map(|system_id| (system_id, SystemVisibility::Detected)),\n        );\n        systems.sort_by_key(|(system_id, _)| *system_id);\n        systems\n    }\n\n    pub fn is_system_visible(\n        &self,\n        universe: &UniverseRepository,\n        system_id: SystemId,\n    ) -> bool {\n        self.system_visibility(universe, system_id).is_some()\n    }\n\n    /// Visible routes connect known systems to each other or to the immediate\n    /// detected frontier. Routes between two merely detected systems remain\n    /// hidden so the map never reveals information beyond that frontier.\n    pub fn visible_routes<\'a>(\n        &self,\n        universe: &\'a UniverseRepository,\n    ) -> Vec<&\'a Route> {\n        universe\n            .definition()\n            .routes\n            .iter()\n            .filter(|route| {\n                let from =\n                    self.system_visibility(universe, route.from);\n                let to = self.system_visibility(universe, route.to);\n\n                (from == Some(SystemVisibility::Known) && to.is_some())\n                    || (\n                        from == Some(SystemVisibility::Detected)\n                            && to == Some(SystemVisibility::Known)\n                    )\n            })\n            .collect()\n    }\n}\n\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ColonyState {\n    pub id: ColonyId,\n    pub name: String,\n    pub faction: FactionId,\n    pub system_id: SystemId,\n    pub planet_id: PlanetId,\n    pub stock: ResourceStock,\n}\n\n#[cfg(test)]\nmod tests {\n    use galactic_domain::{ColonyId, SystemId, UniverseConfig};\n\n    use super::*;\n\n    #[test]\n    fn colony_is_accessible_by_stable_id() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        let colony = state\n            .colony(ColonyId::new(0))\n            .expect("home colony is indexed by its stable ID");\n\n        assert_eq!(colony.name, "Aster Prime Colony");\n    }\n\n    #[test]\n    fn new_game_starts_at_tick_zero_and_speed_one() {\n        let universe = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&universe);\n\n        assert_eq!(state.clock.current_tick().value(), 0);\n        assert_eq!(state.clock.speed(), crate::TimeSpeed::X1);\n    }\n\n    #[test]\n    fn detection_frontier_contains_only_neighbors_of_known_systems() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&repository);\n        let detected = state.detected_systems(&repository);\n\n        assert!(detected.iter().all(|system_id| {\n            !state.is_system_known(*system_id)\n                && state.known_systems.iter().any(|known| {\n                    repository.route_exists(*known, *system_id)\n                })\n        }));\n    }\n\n    #[test]\n    fn normal_visibility_never_reveals_beyond_the_detection_frontier() {\n        let repository = UniverseRepository::generate(UniverseConfig::mvp());\n        let state = GameState::new(&repository);\n        let visible = state.visible_systems(&repository);\n        let visible_ids = visible\n            .iter()\n            .map(|(system_id, _)| *system_id)\n            .collect::<BTreeSet<_>>();\n\n        assert!(visible.len() <= repository.definition().systems.len());\n        for (system_id, visibility) in visible {\n            match visibility {\n                SystemVisibility::Known => {\n                    assert!(state.is_system_known(system_id));\n                }\n                SystemVisibility::Detected => {\n                    assert!(state.known_systems.iter().any(|known| {\n                        repository.route_exists(*known, system_id)\n                    }));\n                }\n            }\n        }\n\n        assert!(state.visible_routes(&repository).iter().all(|route| {\n            visible_ids.contains(&route.from)\n                && visible_ids.contains(&route.to)\n                && (\n                    state.is_system_known(route.from)\n                        || state.is_system_known(route.to)\n                )\n        }));\n    }\n}\n'
CLIENT_RS = 'use std::collections::HashMap;\n\nuse bevy::prelude::*;\nuse bevy::window::PresentMode;\nuse galactic_domain::{\n    PlanetKind, StarClass, SystemId, UniverseConfig, WorldPosition,\n};\nuse galactic_sim::{\n    GameCommand, GameEvent, SelectionTarget, Simulation, SystemVisibility,\n    TimeSpeed,\n};\n\npub fn run() {\n    App::new().add_plugins(ClientPlugin).run();\n}\n\npub struct ClientPlugin;\n\nimpl Plugin for ClientPlugin {\n    fn build(&self, app: &mut App) {\n        app.add_plugins(DefaultPlugins.set(WindowPlugin {\n            primary_window: Some(Window {\n                title: "Galactic MVP".to_string(),\n                resolution: (1280, 720).into(),\n                present_mode: PresentMode::AutoVsync,\n                resizable: true,\n                ..default()\n            }),\n            ..default()\n        }))\n        .insert_resource(ClearColor(Color::srgb(0.006, 0.008, 0.014)))\n        .insert_resource(SimulationResource {\n            simulation: Simulation::new(UniverseConfig::default()),\n            pending_events: Vec::new(),\n        })\n        .init_resource::<PresentationLog>()\n        .init_resource::<VisualAssets>()\n        .init_resource::<StrategicNavigation>()\n        .init_resource::<ViewRebuildRequest>()\n        .add_plugins(SimulationBridgePlugin)\n        .add_plugins(PresentationPlugin)\n        .add_systems(Startup, log_startup);\n    }\n}\n\npub struct SimulationBridgePlugin;\n\nimpl Plugin for SimulationBridgePlugin {\n    fn build(&self, app: &mut App) {\n        app.add_systems(\n            Update,\n            (\n                handle_simulation_input,\n                handle_view_input,\n                tick_simulation,\n            )\n                .chain(),\n        );\n    }\n}\n\npub struct PresentationPlugin;\n\nimpl Plugin for PresentationPlugin {\n    fn build(&self, app: &mut App) {\n        app.add_systems(\n            Startup,\n            (spawn_scene, spawn_strategic_view, spawn_ui).chain(),\n        )\n        .add_systems(\n            Update,\n            (\n                rebuild_strategic_view_if_requested,\n                update_strategic_camera,\n                collect_presentation_events,\n                update_system_visuals,\n                update_system_labels,\n                draw_strategic_overlays,\n                update_ui,\n            ),\n        );\n    }\n}\n\n#[derive(Resource)]\npub struct SimulationResource {\n    simulation: Simulation,\n    pending_events: Vec<GameEvent>,\n}\n\nimpl SimulationResource {\n    pub fn simulation(&self) -> &Simulation {\n        &self.simulation\n    }\n}\n\n#[derive(Resource, Default)]\nstruct PresentationLog {\n    last_event: Option<GameEvent>,\n}\n\n#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\nenum GraphicsPreset {\n    #[default]\n    Low,\n}\n\n#[derive(Resource)]\nstruct VisualAssets {\n    system_mesh: Handle<Mesh>,\n    known_star_materials: HashMap<StarClass, Handle<StandardMaterial>>,\n    detected_material: Handle<StandardMaterial>,\n    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,\n}\n\nimpl FromWorld for VisualAssets {\n    fn from_world(world: &mut World) -> Self {\n        // Low preset: a very small shared mesh is sufficient at universe scale.\n        let system_mesh = {\n            let mut meshes = world.resource_mut::<Assets<Mesh>>();\n            meshes.add(Sphere::default().mesh().ico(1).unwrap())\n        };\n\n        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();\n        let known_star_materials = StarClass::ALL\n            .into_iter()\n            .map(|class| (class, materials.add(star_material(class))))\n            .collect();\n        let detected_material = materials.add(StandardMaterial {\n            base_color: Color::srgba(0.34, 0.48, 0.62, 0.75),\n            emissive: LinearRgba::rgb(0.28, 0.42, 0.62),\n            unlit: true,\n            alpha_mode: AlphaMode::Blend,\n            ..default()\n        });\n        let planet_materials = PlanetKind::ALL\n            .into_iter()\n            .map(|kind| (kind, materials.add(planet_material(kind))))\n            .collect();\n\n        Self {\n            system_mesh,\n            known_star_materials,\n            detected_material,\n            planet_materials,\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum StrategicViewMode {\n    Universe,\n    System(SystemId),\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum UniverseLod {\n    Overview,\n    Regional,\n    Local,\n}\n\nimpl UniverseLod {\n    fn from_distance(distance: f32) -> Self {\n        if distance >= 88.0 {\n            Self::Overview\n        } else if distance >= 48.0 {\n            Self::Regional\n        } else {\n            Self::Local\n        }\n    }\n}\n\n#[derive(Resource)]\nstruct StrategicNavigation {\n    mode: StrategicViewMode,\n    universe_focus: Vec3,\n    universe_distance: f32,\n    system_distance: f32,\n    lod: UniverseLod,\n    debug_full_graph: bool,\n    preset: GraphicsPreset,\n}\n\nimpl Default for StrategicNavigation {\n    fn default() -> Self {\n        let universe_distance = 108.0;\n        Self {\n            mode: StrategicViewMode::Universe,\n            universe_focus: Vec3::ZERO,\n            universe_distance,\n            system_distance: 34.0,\n            lod: UniverseLod::from_distance(universe_distance),\n            debug_full_graph: false,\n            preset: GraphicsPreset::Low,\n        }\n    }\n}\n\nimpl StrategicNavigation {\n    fn enter_system(&mut self, system_id: SystemId) {\n        self.mode = StrategicViewMode::System(system_id);\n    }\n\n    fn exit_system(&mut self) {\n        self.mode = StrategicViewMode::Universe;\n    }\n}\n\n#[derive(Resource, Default)]\nstruct ViewRebuildRequest(bool);\n\n#[derive(Component)]\nstruct StrategicViewEntity;\n\n#[derive(Component)]\nstruct StrategicCamera;\n\n#[derive(Component)]\nstruct SystemVisual {\n    id: SystemId,\n    visibility: SystemVisibility,\n    base_scale: Vec3,\n}\n\n#[derive(Component)]\nstruct SystemLabel {\n    id: SystemId,\n    visibility: SystemVisibility,\n}\n\n#[derive(Component)]\nstruct TopBarText;\n\n#[derive(Component)]\nstruct HelpText;\n\nfn log_startup() {\n    info!("Galactic MVP client starting on Bevy 0.19");\n}\n\nfn spawn_scene(mut commands: Commands) {\n    commands.spawn((\n        Camera3d::default(),\n        Camera {\n            clear_color: ClearColorConfig::Custom(Color::srgb(\n                0.006, 0.008, 0.014,\n            )),\n            ..default()\n        },\n        Transform::from_xyz(0.0, 62.0, 88.0)\n            .looking_at(Vec3::ZERO, Vec3::Y),\n        StrategicCamera,\n    ));\n\n    commands.spawn((\n        PointLight {\n            intensity: 9000.0,\n            range: 240.0,\n            shadow_maps_enabled: false,\n            ..default()\n        },\n        Transform::from_xyz(0.0, 40.0, 0.0),\n    ));\n}\n\nfn spawn_strategic_view(\n    mut commands: Commands,\n    simulation: Res<SimulationResource>,\n    assets: Res<VisualAssets>,\n    navigation: Res<StrategicNavigation>,\n    existing: Query<Entity, With<StrategicViewEntity>>,\n) {\n    rebuild_strategic_view(\n        &mut commands,\n        &simulation,\n        &assets,\n        &navigation,\n        &existing,\n    );\n}\n\nfn rebuild_strategic_view_if_requested(\n    mut commands: Commands,\n    simulation: Res<SimulationResource>,\n    assets: Res<VisualAssets>,\n    navigation: Res<StrategicNavigation>,\n    mut request: ResMut<ViewRebuildRequest>,\n    existing: Query<Entity, With<StrategicViewEntity>>,\n) {\n    if !request.0 {\n        return;\n    }\n\n    rebuild_strategic_view(\n        &mut commands,\n        &simulation,\n        &assets,\n        &navigation,\n        &existing,\n    );\n    request.0 = false;\n}\n\nfn rebuild_strategic_view(\n    commands: &mut Commands,\n    simulation: &SimulationResource,\n    assets: &VisualAssets,\n    navigation: &StrategicNavigation,\n    existing: &Query<Entity, With<StrategicViewEntity>>,\n) {\n    for entity in existing.iter() {\n        commands.entity(entity).despawn();\n    }\n\n    match navigation.mode {\n        StrategicViewMode::Universe => {\n            spawn_universe_view(commands, simulation, assets, navigation);\n        }\n        StrategicViewMode::System(system_id) => {\n            spawn_system_view(commands, simulation, assets, system_id);\n        }\n    }\n}\n\nfn spawn_universe_view(\n    commands: &mut Commands,\n    simulation: &SimulationResource,\n    assets: &VisualAssets,\n    navigation: &StrategicNavigation,\n) {\n    let simulation = simulation.simulation();\n    let universe = simulation.universe();\n    let repository = simulation.universe_repository();\n    let state = simulation.state();\n\n    let visible_systems = systems_for_universe_view(\n        simulation,\n        navigation.debug_full_graph,\n    );\n\n    for (system_id, visibility) in visible_systems {\n        let Some(system) = universe.system(system_id) else {\n            continue;\n        };\n\n        let material = match visibility {\n            SystemVisibility::Known => assets\n                .known_star_materials\n                .get(&system.star.class)\n                .expect("star material exists")\n                .clone(),\n            SystemVisibility::Detected => assets.detected_material.clone(),\n        };\n        let visibility_scale = match visibility {\n            SystemVisibility::Known => 1.0,\n            SystemVisibility::Detected => 0.72,\n        };\n        let scale = Vec3::splat(\n            (0.72 + system.star.luminosity.min(2.4) * 0.16)\n                * visibility_scale,\n        );\n        let position = to_vec3(system.position);\n\n        commands.spawn((\n            Mesh3d(assets.system_mesh.clone()),\n            MeshMaterial3d(material),\n            Transform::from_translation(position).with_scale(scale),\n            SystemVisual {\n                id: system.id,\n                visibility,\n                base_scale: scale,\n            },\n            StrategicViewEntity,\n        ));\n\n        let label = match visibility {\n            SystemVisibility::Known => system.name.clone(),\n            SystemVisibility::Detected => format!("Signal {}", system.id.index()),\n        };\n\n        commands.spawn((\n            Text2d::new(label),\n            TextFont {\n                font_size: FontSize::Px(12.0),\n                ..default()\n            },\n            TextColor(match visibility {\n                SystemVisibility::Known => {\n                    Color::srgba(0.76, 0.88, 1.0, 0.90)\n                }\n                SystemVisibility::Detected => {\n                    Color::srgba(0.48, 0.66, 0.82, 0.72)\n                }\n            }),\n            Transform::from_translation(\n                position + Vec3::new(0.0, 1.8, 0.0),\n            )\n            .with_scale(Vec3::splat(0.28)),\n            SystemLabel {\n                id: system.id,\n                visibility,\n            },\n            StrategicViewEntity,\n        ));\n    }\n\n    debug_assert!(\n        navigation.debug_full_graph\n            || state\n                .visible_systems(repository)\n                .iter()\n                .all(|(system_id, _)| {\n                    state.is_system_visible(repository, *system_id)\n                })\n    );\n}\n\nfn systems_for_universe_view(\n    simulation: &Simulation,\n    debug_full_graph: bool,\n) -> Vec<(SystemId, SystemVisibility)> {\n    if debug_full_graph {\n        return simulation\n            .universe()\n            .systems\n            .iter()\n            .map(|system| {\n                (\n                    system.id,\n                    simulation\n                        .state()\n                        .system_visibility(\n                            simulation.universe_repository(),\n                            system.id,\n                        )\n                        .unwrap_or(SystemVisibility::Detected),\n                )\n            })\n            .collect();\n    }\n\n    simulation\n        .state()\n        .visible_systems(simulation.universe_repository())\n}\n\nfn spawn_system_view(\n    commands: &mut Commands,\n    simulation: &SimulationResource,\n    assets: &VisualAssets,\n    system_id: SystemId,\n) {\n    let simulation = simulation.simulation();\n    let Some(system) = simulation.universe().system(system_id) else {\n        return;\n    };\n\n    let star_material = assets\n        .known_star_materials\n        .get(&system.star.class)\n        .expect("star material exists")\n        .clone();\n\n    commands.spawn((\n        Mesh3d(assets.system_mesh.clone()),\n        MeshMaterial3d(star_material),\n        Transform::from_scale(Vec3::splat(2.8)),\n        StrategicViewEntity,\n    ));\n\n    commands.spawn((\n        Text2d::new(system.name.clone()),\n        TextFont {\n            font_size: FontSize::Px(18.0),\n            ..default()\n        },\n        TextColor(Color::srgb(0.94, 0.97, 1.0)),\n        Transform::from_xyz(0.0, 3.6, 0.0)\n            .with_scale(Vec3::splat(0.34)),\n        StrategicViewEntity,\n    ));\n\n    for (index, planet) in system.planets.iter().enumerate() {\n        let radius = 6.0 + index as f32 * 4.8;\n        let angle = index as f32 * 1.37;\n        let position = Vec3::new(\n            angle.cos() * radius,\n            0.0,\n            angle.sin() * radius,\n        );\n        let material = assets\n            .planet_materials\n            .get(&planet.kind)\n            .expect("planet material exists")\n            .clone();\n        let scale = if planet.kind == PlanetKind::GasGiant {\n            1.25\n        } else {\n            0.72\n        };\n\n        commands.spawn((\n            Mesh3d(assets.system_mesh.clone()),\n            MeshMaterial3d(material),\n            Transform::from_translation(position)\n                .with_scale(Vec3::splat(scale)),\n            StrategicViewEntity,\n        ));\n\n        commands.spawn((\n            Text2d::new(planet.name.clone()),\n            TextFont {\n                font_size: FontSize::Px(11.0),\n                ..default()\n            },\n            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),\n            Transform::from_translation(\n                position + Vec3::new(0.0, 1.35, 0.0),\n            )\n            .with_scale(Vec3::splat(0.25)),\n            StrategicViewEntity,\n        ));\n    }\n}\n\nfn spawn_ui(mut commands: Commands) {\n    commands.spawn((\n        Text::new(""),\n        TextFont {\n            font_size: FontSize::Px(16.0),\n            ..default()\n        },\n        TextColor(Color::srgb(0.9, 0.96, 1.0)),\n        Node {\n            position_type: PositionType::Absolute,\n            left: Val::Px(12.0),\n            right: Val::Px(12.0),\n            top: Val::Px(10.0),\n            padding: UiRect::all(Val::Px(10.0)),\n            ..default()\n        },\n        BackgroundColor(Color::srgba(0.014, 0.022, 0.034, 0.78)),\n        TopBarText,\n    ));\n\n    commands.spawn((\n        Text::new(\n            "Space pause | 1/2/3 speed | WASD pan | Q/E zoom | Tab select | F focus | Enter system | Esc universe | F3 debug graph | R rebuild",\n        ),\n        TextFont {\n            font_size: FontSize::Px(13.0),\n            ..default()\n        },\n        TextColor(Color::srgb(0.72, 0.82, 0.92)),\n        Node {\n            position_type: PositionType::Absolute,\n            left: Val::Px(14.0),\n            bottom: Val::Px(14.0),\n            padding: UiRect::all(Val::Px(10.0)),\n            ..default()\n        },\n        BackgroundColor(Color::srgba(0.014, 0.022, 0.034, 0.66)),\n        HelpText,\n    ));\n}\n\nfn handle_simulation_input(\n    keyboard: Res<ButtonInput<KeyCode>>,\n    mut simulation: ResMut<SimulationResource>,\n) {\n    let command = if keyboard.just_pressed(KeyCode::Space) {\n        Some(GameCommand::TogglePause)\n    } else if keyboard.just_pressed(KeyCode::Digit1) {\n        Some(GameCommand::SetSpeed(TimeSpeed::X1))\n    } else if keyboard.just_pressed(KeyCode::Digit2) {\n        Some(GameCommand::SetSpeed(TimeSpeed::X2))\n    } else if keyboard.just_pressed(KeyCode::Digit3) {\n        Some(GameCommand::SetSpeed(TimeSpeed::X4))\n    } else {\n        None\n    };\n\n    let Some(command) = command else {\n        return;\n    };\n    let events = simulation.simulation.apply_command(command);\n    simulation.pending_events.extend(events);\n}\n\nfn handle_view_input(\n    keyboard: Res<ButtonInput<KeyCode>>,\n    mut simulation: ResMut<SimulationResource>,\n    mut navigation: ResMut<StrategicNavigation>,\n    mut rebuild: ResMut<ViewRebuildRequest>,\n) {\n    if keyboard.just_pressed(KeyCode::KeyR) {\n        rebuild.0 = true;\n    }\n\n    if keyboard.just_pressed(KeyCode::F3) {\n        navigation.debug_full_graph = !navigation.debug_full_graph;\n        rebuild.0 = true;\n    }\n\n    if keyboard.just_pressed(KeyCode::Tab)\n        && matches!(navigation.mode, StrategicViewMode::Universe)\n    {\n        cycle_visible_selection(&mut simulation, navigation.debug_full_graph);\n    }\n\n    if keyboard.just_pressed(KeyCode::KeyF)\n        && matches!(navigation.mode, StrategicViewMode::Universe)\n    {\n        focus_selected_system(&simulation, &mut navigation);\n    }\n\n    if keyboard.just_pressed(KeyCode::Enter)\n        && matches!(navigation.mode, StrategicViewMode::Universe)\n    {\n        if let Some(system_id) = enterable_selected_system(\n            &simulation,\n            navigation.debug_full_graph,\n        ) {\n            navigation.enter_system(system_id);\n            rebuild.0 = true;\n        }\n    }\n\n    if keyboard.just_pressed(KeyCode::Escape)\n        && matches!(navigation.mode, StrategicViewMode::System(_))\n    {\n        navigation.exit_system();\n        rebuild.0 = true;\n    }\n}\n\nfn focus_selected_system(\n    simulation: &SimulationResource,\n    navigation: &mut StrategicNavigation,\n) {\n    let Some(system_id) =\n        selected_system(simulation.simulation.state().selected)\n    else {\n        return;\n    };\n    let Some(system) = simulation.simulation.universe().system(system_id)\n    else {\n        return;\n    };\n\n    navigation.universe_focus = to_vec3(system.position);\n}\n\nfn enterable_selected_system(\n    simulation: &SimulationResource,\n    debug_full_graph: bool,\n) -> Option<SystemId> {\n    let system_id =\n        selected_system(simulation.simulation.state().selected)?;\n\n    if debug_full_graph\n        || simulation.simulation.state().is_system_known(system_id)\n    {\n        Some(system_id)\n    } else {\n        None\n    }\n}\n\nfn cycle_visible_selection(\n    simulation: &mut SimulationResource,\n    debug_full_graph: bool,\n) {\n    let systems = systems_for_universe_view(\n        simulation.simulation(),\n        debug_full_graph,\n    );\n    if systems.is_empty() {\n        return;\n    }\n\n    let current = selected_system(simulation.simulation.state().selected);\n    let current_index = current.and_then(|current_id| {\n        systems\n            .iter()\n            .position(|(system_id, _)| *system_id == current_id)\n    });\n    let next_index = current_index\n        .map(|index| (index + 1) % systems.len())\n        .unwrap_or(0);\n    let next_system = systems[next_index].0;\n\n    let events = simulation\n        .simulation\n        .apply_command(GameCommand::SelectSystem(next_system));\n    simulation.pending_events.extend(events);\n}\n\nfn selected_system(selection: SelectionTarget) -> Option<SystemId> {\n    match selection {\n        SelectionTarget::None => None,\n        SelectionTarget::System(system_id) => Some(system_id),\n        SelectionTarget::Planet { system_id, .. } => Some(system_id),\n    }\n}\n\nfn tick_simulation(\n    time: Res<Time>,\n    mut simulation: ResMut<SimulationResource>,\n) {\n    let events = simulation.simulation.advance(time.delta());\n    simulation.pending_events.extend(events);\n}\n\nfn update_strategic_camera(\n    time: Res<Time>,\n    keyboard: Res<ButtonInput<KeyCode>>,\n    mut navigation: ResMut<StrategicNavigation>,\n    mut query: Query<&mut Transform, With<StrategicCamera>>,\n) {\n    let Ok(mut transform) = query.single_mut() else {\n        return;\n    };\n\n    let delta_seconds = time.delta_secs();\n    match navigation.mode {\n        StrategicViewMode::Universe => {\n            let pan_speed =\n                (navigation.universe_distance * 0.55).max(18.0);\n            let mut pan = Vec3::ZERO;\n\n            if keyboard.pressed(KeyCode::KeyA) {\n                pan.x -= 1.0;\n            }\n            if keyboard.pressed(KeyCode::KeyD) {\n                pan.x += 1.0;\n            }\n            if keyboard.pressed(KeyCode::KeyW) {\n                pan.z -= 1.0;\n            }\n            if keyboard.pressed(KeyCode::KeyS) {\n                pan.z += 1.0;\n            }\n            if pan.length_squared() > 0.0 {\n                navigation.universe_focus +=\n                    pan.normalize() * pan_speed * delta_seconds;\n            }\n\n            let zoom_speed =\n                (navigation.universe_distance * 0.85).max(22.0);\n            if keyboard.pressed(KeyCode::KeyQ) {\n                navigation.universe_distance -= zoom_speed * delta_seconds;\n            }\n            if keyboard.pressed(KeyCode::KeyE) {\n                navigation.universe_distance += zoom_speed * delta_seconds;\n            }\n            navigation.universe_distance =\n                navigation.universe_distance.clamp(20.0, 150.0);\n            navigation.lod =\n                UniverseLod::from_distance(navigation.universe_distance);\n\n            let eye = navigation.universe_focus\n                + Vec3::new(\n                    0.0,\n                    navigation.universe_distance * 0.58,\n                    navigation.universe_distance * 0.82,\n                );\n            *transform = Transform::from_translation(eye)\n                .looking_at(navigation.universe_focus, Vec3::Y);\n        }\n        StrategicViewMode::System(_) => {\n            let zoom_speed =\n                (navigation.system_distance * 0.9).max(12.0);\n            if keyboard.pressed(KeyCode::KeyQ) {\n                navigation.system_distance -= zoom_speed * delta_seconds;\n            }\n            if keyboard.pressed(KeyCode::KeyE) {\n                navigation.system_distance += zoom_speed * delta_seconds;\n            }\n            navigation.system_distance =\n                navigation.system_distance.clamp(14.0, 68.0);\n\n            let eye = Vec3::new(\n                0.0,\n                navigation.system_distance * 0.58,\n                navigation.system_distance * 0.82,\n            );\n            *transform = Transform::from_translation(eye)\n                .looking_at(Vec3::ZERO, Vec3::Y);\n        }\n    }\n}\n\nfn collect_presentation_events(\n    mut simulation: ResMut<SimulationResource>,\n    mut log: ResMut<PresentationLog>,\n) {\n    for event in simulation.pending_events.drain(..) {\n        log.last_event = Some(event);\n    }\n}\n\nfn update_system_visuals(\n    simulation: Res<SimulationResource>,\n    navigation: Res<StrategicNavigation>,\n    mut query: Query<(&SystemVisual, &mut Transform)>,\n) {\n    if !matches!(navigation.mode, StrategicViewMode::Universe) {\n        return;\n    }\n\n    let selected_system =\n        selected_system(simulation.simulation().state().selected);\n\n    for (visual, mut transform) in &mut query {\n        let selected_multiplier =\n            if Some(visual.id) == selected_system { 1.55 } else { 1.0 };\n        let lod_multiplier = match navigation.lod {\n            UniverseLod::Overview => 0.78,\n            UniverseLod::Regional => 0.92,\n            UniverseLod::Local => 1.08,\n        };\n        let visibility_multiplier = match visual.visibility {\n            SystemVisibility::Known => 1.0,\n            SystemVisibility::Detected => 0.84,\n        };\n\n        transform.scale = visual.base_scale\n            * selected_multiplier\n            * lod_multiplier\n            * visibility_multiplier;\n    }\n}\n\nfn update_system_labels(\n    simulation: Res<SimulationResource>,\n    navigation: Res<StrategicNavigation>,\n    mut query: Query<(&SystemLabel, &mut Visibility)>,\n) {\n    if !matches!(navigation.mode, StrategicViewMode::Universe) {\n        return;\n    }\n\n    let state = simulation.simulation().state();\n    let selected = selected_system(state.selected);\n\n    for (label, mut visibility) in &mut query {\n        let is_selected = Some(label.id) == selected;\n        let is_colony = state\n            .colonies\n            .iter()\n            .any(|colony| colony.system_id == label.id);\n\n        let should_show = is_selected\n            || is_colony\n            || match navigation.lod {\n                UniverseLod::Overview => false,\n                UniverseLod::Regional => {\n                    label.visibility == SystemVisibility::Known\n                }\n                UniverseLod::Local => true,\n            };\n\n        *visibility = if should_show {\n            Visibility::Visible\n        } else {\n            Visibility::Hidden\n        };\n    }\n}\n\nfn draw_strategic_overlays(\n    mut gizmos: Gizmos,\n    simulation: Res<SimulationResource>,\n    navigation: Res<StrategicNavigation>,\n) {\n    match navigation.mode {\n        StrategicViewMode::Universe => {\n            draw_universe_routes(\n                &mut gizmos,\n                simulation.simulation(),\n                &navigation,\n            );\n        }\n        StrategicViewMode::System(system_id) => {\n            draw_system_orbits(\n                &mut gizmos,\n                simulation.simulation(),\n                system_id,\n            );\n        }\n    }\n}\n\nfn draw_universe_routes(\n    gizmos: &mut Gizmos,\n    simulation: &Simulation,\n    navigation: &StrategicNavigation,\n) {\n    let universe = simulation.universe();\n    let state = simulation.state();\n\n    if navigation.debug_full_graph {\n        for route in &universe.routes {\n            draw_route(\n                gizmos,\n                universe,\n                route.from,\n                route.to,\n                Color::srgba(0.42, 0.24, 0.62, 0.28),\n            );\n        }\n        return;\n    }\n\n    for route in state.visible_routes(simulation.universe_repository()) {\n        let both_known =\n            state.is_system_known(route.from)\n                && state.is_system_known(route.to);\n        let color = if both_known {\n            Color::srgba(0.28, 0.62, 0.94, 0.58)\n        } else {\n            Color::srgba(0.30, 0.48, 0.66, 0.38)\n        };\n        draw_route(gizmos, universe, route.from, route.to, color);\n    }\n}\n\nfn draw_route(\n    gizmos: &mut Gizmos,\n    universe: &galactic_domain::UniverseDefinition,\n    from_id: SystemId,\n    to_id: SystemId,\n    color: Color,\n) {\n    let Some(from) = universe.system(from_id) else {\n        return;\n    };\n    let Some(to) = universe.system(to_id) else {\n        return;\n    };\n    gizmos.line(to_vec3(from.position), to_vec3(to.position), color);\n}\n\nfn draw_system_orbits(\n    gizmos: &mut Gizmos,\n    simulation: &Simulation,\n    system_id: SystemId,\n) {\n    let Some(system) = simulation.universe().system(system_id) else {\n        return;\n    };\n\n    for index in 0..system.planets.len() {\n        let radius = 6.0 + index as f32 * 4.8;\n        draw_circle_xz(\n            gizmos,\n            radius,\n            48,\n            Color::srgba(0.32, 0.46, 0.62, 0.26),\n        );\n    }\n}\n\nfn draw_circle_xz(\n    gizmos: &mut Gizmos,\n    radius: f32,\n    segments: usize,\n    color: Color,\n) {\n    for segment in 0..segments {\n        let start_angle =\n            segment as f32 / segments as f32 * std::f32::consts::TAU;\n        let end_angle = (segment + 1) as f32\n            / segments as f32\n            * std::f32::consts::TAU;\n        let start = Vec3::new(\n            start_angle.cos() * radius,\n            0.0,\n            start_angle.sin() * radius,\n        );\n        let end = Vec3::new(\n            end_angle.cos() * radius,\n            0.0,\n            end_angle.sin() * radius,\n        );\n        gizmos.line(start, end, color);\n    }\n}\n\nfn update_ui(\n    simulation: Res<SimulationResource>,\n    navigation: Res<StrategicNavigation>,\n    log: Res<PresentationLog>,\n    mut query: Query<&mut Text, With<TopBarText>>,\n) {\n    let Ok(mut text) = query.single_mut() else {\n        return;\n    };\n    let simulation = simulation.simulation();\n    let universe = simulation.universe();\n    let repository = simulation.universe_repository();\n    let state = simulation.state();\n    let selected = selection_label(state.selected);\n    let last_event = log\n        .last_event\n        .map(event_label)\n        .unwrap_or_else(|| "ready".to_string());\n    let visible_route_count = if navigation.debug_full_graph {\n        universe.routes.len()\n    } else {\n        state.visible_routes(repository).len()\n    };\n    let visible_system_count = if navigation.debug_full_graph {\n        universe.systems.len()\n    } else {\n        state.visible_systems(repository).len()\n    };\n    let view_label = match navigation.mode {\n        StrategicViewMode::Universe => format!(\n            "universe/{:?}",\n            navigation.lod\n        ),\n        StrategicViewMode::System(system_id) => {\n            format!("system {}", system_id.index())\n        }\n    };\n\n    text.0 = format!(\n        "Galactic MVP | preset {:?} | view {} | seed {} | systems {}/{} | routes {}/{} | known {} | tick {} | t {:.1}s | speed {} | selected {} | debug {} | event {}",\n        navigation.preset,\n        view_label,\n        universe.seed,\n        visible_system_count,\n        universe.systems.len(),\n        visible_route_count,\n        universe.routes.len(),\n        state.known_systems.len(),\n        state.clock.current_tick(),\n        state.clock.elapsed_seconds(),\n        state.clock.speed(),\n        selected,\n        navigation.debug_full_graph,\n        last_event\n    );\n}\n\nfn to_vec3(position: WorldPosition) -> Vec3 {\n    Vec3::new(position.x, position.y, position.z)\n}\n\nfn star_material(class: StarClass) -> StandardMaterial {\n    StandardMaterial {\n        base_color: star_color(class),\n        emissive: star_emissive(class),\n        unlit: true,\n        ..default()\n    }\n}\n\nfn planet_material(kind: PlanetKind) -> StandardMaterial {\n    StandardMaterial {\n        base_color: match kind {\n            PlanetKind::Rocky => Color::srgb(0.48, 0.42, 0.36),\n            PlanetKind::Ocean => Color::srgb(0.18, 0.46, 0.72),\n            PlanetKind::Desert => Color::srgb(0.72, 0.52, 0.28),\n            PlanetKind::Ice => Color::srgb(0.62, 0.78, 0.90),\n            PlanetKind::GasGiant => Color::srgb(0.62, 0.50, 0.68),\n            PlanetKind::Volcanic => Color::srgb(0.72, 0.24, 0.12),\n        },\n        unlit: true,\n        ..default()\n    }\n}\n\nfn star_color(class: StarClass) -> Color {\n    match class {\n        StarClass::Blue => Color::srgb(0.42, 0.66, 1.0),\n        StarClass::White => Color::srgb(0.92, 0.96, 1.0),\n        StarClass::Yellow => Color::srgb(1.0, 0.86, 0.44),\n        StarClass::Orange => Color::srgb(1.0, 0.58, 0.28),\n        StarClass::Red => Color::srgb(0.95, 0.28, 0.24),\n    }\n}\n\nfn star_emissive(class: StarClass) -> LinearRgba {\n    match class {\n        StarClass::Blue => LinearRgba::rgb(1.2, 2.4, 5.0),\n        StarClass::White => LinearRgba::rgb(2.6, 2.8, 3.0),\n        StarClass::Yellow => LinearRgba::rgb(2.8, 2.1, 0.8),\n        StarClass::Orange => LinearRgba::rgb(2.6, 1.2, 0.45),\n        StarClass::Red => LinearRgba::rgb(2.2, 0.45, 0.35),\n    }\n}\n\nfn selection_label(selection: SelectionTarget) -> String {\n    match selection {\n        SelectionTarget::None => "none".to_string(),\n        SelectionTarget::System(system_id) => {\n            format!("system {}", system_id.index())\n        }\n        SelectionTarget::Planet {\n            system_id,\n            planet_id,\n        } => format!(\n            "planet {}:{}",\n            system_id.index(),\n            planet_id.index()\n        ),\n    }\n}\n\nfn event_label(event: GameEvent) -> String {\n    match event {\n        GameEvent::SpeedChanged(speed) => format!("speed {}", speed),\n        GameEvent::SelectionChanged(selection) => {\n            format!("selection {}", selection_label(selection))\n        }\n        GameEvent::TicksAdvanced {\n            ticks,\n            current_tick,\n        } => format!("+{} ticks -> {}", ticks.ticks(), current_tick),\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn semantic_lod_uses_stable_distance_bands() {\n        assert_eq!(\n            UniverseLod::from_distance(120.0),\n            UniverseLod::Overview\n        );\n        assert_eq!(\n            UniverseLod::from_distance(64.0),\n            UniverseLod::Regional\n        );\n        assert_eq!(\n            UniverseLod::from_distance(32.0),\n            UniverseLod::Local\n        );\n    }\n\n    #[test]\n    fn normal_view_instantiates_fewer_systems_than_debug_view() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n\n        let normal = systems_for_universe_view(&simulation, false);\n        let debug = systems_for_universe_view(&simulation, true);\n\n        assert!(normal.len() <= debug.len());\n        assert_eq!(debug.len(), simulation.universe().systems.len());\n    }\n\n    #[test]\n    fn universe_camera_context_survives_system_transition() {\n        let mut navigation = StrategicNavigation {\n            universe_focus: Vec3::new(12.0, 0.0, -7.0),\n            universe_distance: 73.0,\n            ..default()\n        };\n        let focus = navigation.universe_focus;\n        let distance = navigation.universe_distance;\n\n        navigation.enter_system(SystemId::from_index(3));\n        navigation.exit_system();\n\n        assert_eq!(navigation.mode, StrategicViewMode::Universe);\n        assert_eq!(navigation.universe_focus, focus);\n        assert_eq!(navigation.universe_distance, distance);\n    }\n\n    #[test]\n    fn presentation_labels_use_domain_selection_ids() {\n        let label = selection_label(SelectionTarget::Planet {\n            system_id: SystemId::new(2),\n            planet_id: galactic_domain::PlanetId::new(1),\n        });\n\n        assert_eq!(label, "planet 2:1");\n    }\n}\n'
DOC_APPEND = "\n## MVP-007 — Vue Univers limitée au voisinage découvert\n\nLa scène Bevy ne représente plus systématiquement tous les systèmes générés.\n\n```text\nSystèmes connus\n        │\n        ├── affichage complet\n        └── voisins directs\n                │\n                ▼\n        systèmes détectés\n                │ silhouette / signal\n                ▼\nFrontière visible de la carte\n```\n\nRègles :\n\n- les systèmes connus utilisent leur classe et leur nom ;\n- les voisins directs inconnus sont représentés comme signaux détectés ;\n- les systèmes situés au-delà de cette frontière ne sont pas instanciés ;\n- seules les routes connu↔connu et connu↔détecté sont affichées ;\n- le mode debug `F3` permet d'afficher temporairement tout le graphe ;\n- le preset actif reste `Low` avec un mesh partagé très simple ;\n- le zoom utilise trois niveaux sémantiques :\n  - `Overview` : sélection et colonies seulement ;\n  - `Regional` : labels des systèmes connus ;\n  - `Local` : tous les labels de la frontière visible ;\n- `WASD` déplace la caméra et `Q/E` contrôle le zoom ;\n- `Tab` sélectionne le prochain système visible et `F` le recentre ;\n- `Entrée` ouvre une vue Système légère et `Échap` revient à l'Univers ;\n- le retour à l'Univers conserve le focus, le zoom et la sélection.\n\nLes niveaux persistants `Inconnu`, `Détecté`, `Sondé`, `Analysé` et\n`Colonisé` seront introduits par `MVP-009`. Pour MVP-007, la détection est une\nfrontière dérivée du graphe et ne modifie pas encore le format de sauvegarde.\n"


STATE_RS = STATE_RS.replace(
    "    use galactic_domain::{ColonyId, SystemId, UniverseConfig};\n",
    "    use galactic_domain::{ColonyId, UniverseConfig};\n",
)
CLIENT_RS = CLIENT_RS.replace(
    "    if keyboard.just_pressed(KeyCode::Enter)\n"
    "        && matches!(navigation.mode, StrategicViewMode::Universe)\n"
    "    {\n"
    "        if let Some(system_id) = enterable_selected_system(\n"
    "            &simulation,\n"
    "            navigation.debug_full_graph,\n"
    "        ) {\n"
    "            navigation.enter_system(system_id);\n"
    "            rebuild.0 = true;\n"
    "        }\n"
    "    }\n",
    "    if keyboard.just_pressed(KeyCode::Enter)\n"
    "        && matches!(navigation.mode, StrategicViewMode::Universe)\n"
    "        && let Some(system_id) = enterable_selected_system(\n"
    "            &simulation,\n"
    "            navigation.debug_full_graph,\n"
    "        )\n"
    "    {\n"
    "        navigation.enter_system(system_id);\n"
    "        rebuild.0 = true;\n"
    "    }\n",
)


@dataclass(frozen=True)
class Update:
    path: Path
    before: str
    after: str


def run(
    command: list[str],
    *,
    cwd: Path,
    check: bool = True,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(command))
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    if capture and result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : {' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (candidate / "crates/galactic_sim/src/state.rs").exists()
            and (candidate / "crates/galactic_client/src/lib.rs").exists()
        ):
            return candidate
    raise SystemExit("Racine Galactic introuvable. Utilise --root.")


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def verify_baseline(root: Path, force: bool) -> None:
    head = run(["git", "rev-parse", "HEAD"], cwd=root).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return

    ancestor = run(
        ["git", "merge-base", "--is-ancestor", EXPECTED_BASELINE_COMMIT, "HEAD"],
        cwd=root,
        check=False,
    )
    if ancestor.returncode == 0:
        print(f"Baseline présente dans l'historique ; HEAD actuel : {head}")
        return
    if force:
        print("WARNING: baseline différente, poursuite autorisée par --force.")
        return

    raise SystemExit(
        "Le dépôt local ne correspond pas à la baseline MVP-006 analysée.\n"
        f"HEAD={head}\nAttendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après vérification."
    )


def verify_mvp6(root: Path) -> None:
    state = (root / "crates/galactic_sim/src/state.rs").read_text(
        encoding="utf-8"
    )
    universe = (root / "crates/galactic_sim/src/universe.rs").read_text(
        encoding="utf-8"
    )
    client = (root / "crates/galactic_client/src/lib.rs").read_text(
        encoding="utf-8"
    )

    failures = []
    if "visible_routes" not in state:
        failures.append("routes découvertes MVP-006 absentes")
    if "shortest_path" not in universe or "hop_distance" not in universe:
        failures.append("graphe indexé MVP-006 absent")
    if "state.visible_routes" not in client:
        failures.append("filtrage des routes absent du client")

    if failures:
        raise SystemExit(
            "Baseline MVP-006 incohérente :\n- " + "\n- ".join(failures)
        )


def patch_docs(source: str) -> str:
    if "## MVP-007 — Vue Univers limitée au voisinage découvert" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def format_rust_source(root: Path, source: str) -> str:
    result = subprocess.run(
        ["rustfmt", "--edition", "2024", "--emit", "stdout"],
        cwd=root,
        input=source,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        details = result.stderr or result.stdout
        raise SystemExit(
            "Impossible de formatter la source Rust générée par MVP-007.\n"
            + details
        )
    return normalize(result.stdout)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    replacements = {
        root / "crates/galactic_sim/src/state.rs": format_rust_source(
            root, STATE_RS
        ),
        root / "crates/galactic_client/src/lib.rs": format_rust_source(
            root, CLIENT_RS
        ),
    }
    for path, after in replacements.items():
        before = path.read_text(encoding="utf-8")
        if before != after:
            updates.append(Update(path, before, after))

    docs_path = root / "docs/mvp_architecture.md"
    docs_before = docs_path.read_text(encoding="utf-8")
    docs_after = patch_docs(docs_before)
    if docs_before != docs_after:
        updates.append(Update(docs_path, docs_before, docs_after))

    return updates


def show_diff(update: Update, root: Path) -> None:
    relative = update.path.relative_to(root)
    print(
        "".join(
            difflib.unified_diff(
                update.before.splitlines(keepends=True),
                update.after.splitlines(keepends=True),
                fromfile=f"a/{relative}",
                tofile=f"b/{relative}",
            )
        ),
        end="",
    )


def apply_updates(
    updates: list[Update],
    root: Path,
    dry_run: bool,
) -> None:
    if not updates:
        print("MVP-007 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp007-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        backup = backup_root / relative
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(update.path, backup)
        update.path.write_text(update.after, encoding="utf-8")
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def checks(root: Path) -> None:
    run(["cargo", "fmt", "--all"], cwd=root, capture=False)
    run(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        cwd=root,
        capture=False,
    )
    run(["cargo", "test", "--workspace"], cwd=root, capture=False)
    run(["cargo", "build", "--release"], cwd=root, capture=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-checks", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_mvp6(root)

    status = run(["git", "status", "--porcelain"], cwd=root).stdout
    if status.strip():
        print("WARNING: working tree already contains changes.")
        print(status, end="" if status.endswith("\n") else "\n")

    updates = collect_updates(root)
    apply_updates(updates, root, args.dry_run)

    if args.dry_run:
        print(f"\nDry-run complete: {len(updates)} file(s) would change.")
        return 0

    if args.skip_checks:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets --all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        checks(root)

    print(
        "\nMVP-007 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

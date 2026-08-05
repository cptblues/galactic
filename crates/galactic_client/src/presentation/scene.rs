use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use galactic_domain::{PlanetId, PlanetKind, StarClass, SystemId, WorldPosition};
use galactic_sim::{
    KnowledgeLevel, MVP_HOME_SYSTEM_ID, Simulation, SystemVisibility, TimeSpeed,
    VictoryConditionProgress, VictoryProgress, evaluate_victory_progress, technology_definition,
    victory_rules,
};

use crate::presentation::components::*;
use crate::presentation::icons::{IconAssets, IconKind, spawn_icon};
use crate::presentation::procedural_materials::star_color;
use crate::presentation::resource_hud::*;
use crate::presentation::strategic_navigation::*;
use crate::*;

const INSPECTOR_PANEL_TOP_PX: f32 = navigation_ui::NAVIGATION_BAR_TOP_PX;

pub(crate) fn spawn_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.006, 0.008, 0.014)),
            ..default()
        },
        Hdr,
        // Bloom fixe pour l'instant ; MVP-034 (futur) pourra le brancher sur GraphicsPreset.
        Bloom::default(),
        Tonemapping::TonyMcMapface,
        Transform::from_xyz(0.0, 62.0, 88.0).looking_at(Vec3::ZERO, Vec3::Y),
        StrategicCamera,
    ));

    commands.spawn((
        PointLight {
            intensity: 9000.0,
            range: 240.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 40.0, 0.0),
    ));
}

pub(crate) fn spawn_strategic_view(
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    navigation: Res<StrategicNavigation>,
    existing: Query<Entity, With<StrategicViewEntity>>,
) {
    rebuild_strategic_view(&mut commands, &simulation, &assets, &navigation, &existing);
}

pub(crate) fn rebuild_strategic_view_if_requested(
    mut commands: Commands,
    simulation: Res<SimulationResource>,
    assets: Res<VisualAssets>,
    navigation: Res<StrategicNavigation>,
    mut request: ResMut<ViewRebuildRequest>,
    existing: Query<Entity, With<StrategicViewEntity>>,
) {
    if !request.0 {
        return;
    }

    rebuild_strategic_view(&mut commands, &simulation, &assets, &navigation, &existing);
    request.0 = false;
}

pub(crate) fn rebuild_strategic_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    navigation: &StrategicNavigation,
    existing: &Query<Entity, With<StrategicViewEntity>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    match navigation.mode {
        StrategicViewMode::Universe => {
            spawn_universe_view(commands, simulation, assets, navigation);
        }
        StrategicViewMode::System(system_id) => {
            spawn_system_view(commands, simulation, assets, system_id);
        }
    }
}

pub(crate) fn spawn_universe_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    navigation: &StrategicNavigation,
) {
    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let state = simulation.state();

    let visible_systems = systems_for_universe_view(simulation, navigation.debug_full_graph);

    for entry in visible_systems {
        let Some(system) = universe.system(entry.id) else {
            continue;
        };

        let material = match entry.tier {
            UniverseSystemTier::Known => assets
                .known_star_materials
                .get(&system.star.class)
                .expect("star material exists")
                .clone(),
            UniverseSystemTier::Detected => assets.detected_material.clone(),
            UniverseSystemTier::Observed => assets.observed_material.clone(),
        };
        let visibility_scale = match entry.tier {
            UniverseSystemTier::Known => 1.0,
            UniverseSystemTier::Detected => 0.72,
            UniverseSystemTier::Observed => 0.44,
        };
        let scale = Vec3::splat((0.72 + system.star.luminosity.min(2.4) * 0.16) * visibility_scale);
        let position = projected_universe_position(system.position, navigation.projection_mix);

        let mut entity = commands.spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(scale),
            SystemVisual {
                id: system.id,
                tier: entry.tier,
                base_scale: scale,
            },
            StrategicViewEntity,
        ));
        let selectable_visibility = entry.tier.visibility().or_else(|| {
            navigation
                .debug_full_graph
                .then_some(SystemVisibility::Detected)
        });
        if let Some(visibility) = selectable_visibility {
            entity.insert(SelectableVisual {
                target: PickTarget::System(system.id),
                pick_radius_px: 18.0,
                priority: system_pick_priority(simulation, system.id, visibility),
            });
            spawn_pointer_halo(
                commands,
                assets,
                PickTarget::System(system.id),
                position,
                scale.x * 1.65,
            );
        }

        if entry.tier == UniverseSystemTier::Known {
            spawn_star_halo(
                commands,
                assets,
                system.id,
                system.star.class,
                position,
                scale.x * 1.8,
            );
            if let Some(tint) =
                crate::presentation::territory::system_territory_tint(state, universe, system.id)
            {
                spawn_territory_ring(commands, assets, tint, position, scale.x * 2.4);
            }
        }

        if let Some(visibility) = selectable_visibility {
            let label = match entry.tier {
                UniverseSystemTier::Known => system.name.clone(),
                UniverseSystemTier::Detected => format!("Signal {}", system.id.index()),
                UniverseSystemTier::Observed => format!("Observation {}", system.id.index()),
            };

            commands.spawn((
                Text2d::new(label),
                ui_text_font(12.0),
                TextColor(match entry.tier {
                    UniverseSystemTier::Known => Color::srgba(0.76, 0.88, 1.0, 0.90),
                    UniverseSystemTier::Detected => Color::srgba(0.48, 0.66, 0.82, 0.72),
                    UniverseSystemTier::Observed => Color::srgba(0.38, 0.44, 0.56, 0.52),
                }),
                Transform::from_translation(position + Vec3::new(0.0, 1.8, 0.0))
                    .with_scale(Vec3::splat(0.28)),
                SystemLabel {
                    id: system.id,
                    visibility,
                },
                StrategicViewEntity,
            ));
        }
    }

    for sector_label in known_sector_labels(simulation) {
        let position =
            projected_universe_position(sector_label.position, navigation.projection_mix);
        commands.spawn((
            Text2d::new(sector_label.text.clone()),
            ui_text_font(18.0),
            TextColor(Color::srgba(0.96, 0.74, 0.36, 0.88)),
            Transform::from_translation(position + Vec3::new(0.0, 4.2, 0.0))
                .with_scale(Vec3::splat(0.36)),
            SectorLabel {
                id: sector_label.id,
                base_text: sector_label.text,
                position: sector_label.position,
            },
            StrategicViewEntity,
        ));
    }

    debug_assert!(
        navigation.debug_full_graph
            || state
                .visible_systems()
                .iter()
                .all(|(system_id, _)| { state.is_system_visible(*system_id) })
    );
}

pub(crate) fn known_sector_labels(simulation: &Simulation) -> Vec<KnownSectorLabel> {
    let universe = simulation.universe();
    let state = simulation.state();

    universe
        .sectors
        .iter()
        .filter_map(|sector| {
            let known_positions = sector
                .systems
                .iter()
                .filter(|system_id| state.is_system_known(**system_id))
                .filter_map(|system_id| universe.system(*system_id).map(|system| system.position))
                .collect::<Vec<_>>();
            if known_positions.is_empty() {
                return None;
            }

            let divisor = known_positions.len() as f32;
            let position =
                known_positions
                    .iter()
                    .fold(WorldPosition::ZERO, |accumulator, position| {
                        WorldPosition::new(
                            accumulator.x + position.x,
                            accumulator.y + position.y,
                            accumulator.z + position.z,
                        )
                    });

            Some(KnownSectorLabel {
                id: sector.id,
                text: sector.name.clone(),
                position: WorldPosition::new(
                    position.x / divisor,
                    position.y / divisor,
                    position.z / divisor,
                ),
            })
        })
        .collect()
}

pub(crate) fn systems_for_universe_view(
    simulation: &Simulation,
    debug_full_graph: bool,
) -> Vec<UniverseSystemEntry> {
    if debug_full_graph {
        return simulation
            .universe()
            .systems
            .iter()
            .map(|system| {
                let tier = simulation
                    .state()
                    .system_visibility(system.id)
                    .map(UniverseSystemTier::from_visibility)
                    .unwrap_or(UniverseSystemTier::Observed);
                UniverseSystemEntry {
                    id: system.id,
                    tier,
                }
            })
            .collect();
    }

    let mut systems = simulation
        .state()
        .visible_systems()
        .into_iter()
        .map(|(id, visibility)| UniverseSystemEntry {
            id,
            tier: UniverseSystemTier::from_visibility(visibility),
        })
        .collect::<Vec<_>>();
    let observation_ids = initial_observation_systems(
        simulation.universe(),
        MVP_HOME_SYSTEM_ID,
        INITIAL_OBSERVATION_SYSTEM_LIMIT,
    );
    for id in observation_ids {
        if simulation.state().system_visibility(id).is_none() {
            systems.push(UniverseSystemEntry {
                id,
                tier: UniverseSystemTier::Observed,
            });
        }
    }
    systems.sort_by_key(|entry| entry.id);
    systems
}

pub(crate) fn initial_observation_systems(
    universe: &galactic_domain::UniverseDefinition,
    origin_id: SystemId,
    limit: usize,
) -> Vec<SystemId> {
    let Some(origin) = universe.system(origin_id) else {
        return Vec::new();
    };
    let origin = to_vec3(origin.position);
    let mut candidates = universe
        .systems
        .iter()
        .map(|system| (system.id, to_vec3(system.position).distance_squared(origin)))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left_id.cmp(right_id))
    });
    candidates
        .into_iter()
        .take(limit.min(universe.systems.len()))
        .map(|(id, _)| id)
        .collect()
}

pub(crate) fn spawn_star_halo(
    commands: &mut Commands,
    assets: &VisualAssets,
    system_id: SystemId,
    class: StarClass,
    position: Vec3,
    scale: f32,
) {
    let material = assets
        .star_halo_materials
        .get(&class)
        .expect("star halo material exists")
        .clone();
    let base_scale = Vec3::splat(scale);
    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_scale(base_scale),
        SystemVisual {
            id: system_id,
            tier: UniverseSystemTier::Known,
            base_scale,
        },
        StrategicViewEntity,
    ));
}

pub(crate) fn spawn_territory_ring(
    commands: &mut Commands,
    assets: &VisualAssets,
    tint: crate::presentation::territory::TerritoryTint,
    position: Vec3,
    scale: f32,
) {
    let material = assets
        .territory_materials
        .get(&tint)
        .expect("territory material exists")
        .clone();
    commands.spawn((
        Mesh3d(assets.colony_ring_mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(position)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(scale)),
        StrategicViewEntity,
    ));
}

pub(crate) fn spawn_system_view(
    commands: &mut Commands,
    simulation: &SimulationResource,
    assets: &VisualAssets,
    system_id: SystemId,
) {
    let simulation = simulation.simulation();
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };
    let state = simulation.state();

    let star_material = assets
        .known_star_materials
        .get(&system.star.class)
        .expect("star material exists")
        .clone();

    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(star_material),
        Transform::from_scale(Vec3::splat(2.8)),
        SelectableVisual {
            target: PickTarget::System(system_id),
            pick_radius_px: 20.0,
            priority: system_pick_priority(simulation, system_id, SystemVisibility::Known),
        },
        StrategicViewEntity,
    ));
    let star_halo_material = assets
        .star_halo_materials
        .get(&system.star.class)
        .expect("star halo material exists")
        .clone();
    commands.spawn((
        Mesh3d(assets.system_mesh.clone()),
        MeshMaterial3d(star_halo_material),
        Transform::from_scale(Vec3::splat(4.4)),
        StrategicViewEntity,
    ));
    commands.spawn((
        PointLight {
            color: star_color(system.star.class),
            intensity: 11_000.0,
            range: 90.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.5, 0.0),
        StrategicViewEntity,
    ));
    spawn_pointer_halo(
        commands,
        assets,
        PickTarget::System(system_id),
        Vec3::ZERO,
        3.5,
    );

    commands.spawn((
        Text2d::new(system.name.clone()),
        ui_text_font(18.0),
        TextColor(Color::srgb(0.94, 0.97, 1.0)),
        Transform::from_xyz(0.0, 3.6, 0.0).with_scale(Vec3::splat(0.34)),
        StrategicViewEntity,
    ));

    for (index, planet) in system.planets.iter().enumerate() {
        let level = state.planet_knowledge_level(planet.id);
        if level == KnowledgeLevel::Unknown {
            continue;
        }

        let radius = 6.0 + index as f32 * 4.8;
        let orbit = planet_orbit(index, radius, 0.0);
        let position = orbit.translation_at(0.0);
        let colony = state.colony_on_planet(planet.id);
        let material = if level.reveals_identity() {
            assets
                .planet_materials
                .get(&planet.kind)
                .expect("planet material exists")
                .clone()
        } else {
            assets.detected_material.clone()
        };
        let scale = if level.reveals_identity() {
            planet_visual_scale(planet.kind)
        } else {
            0.72
        };
        let label = match level {
            KnowledgeLevel::Unknown => {
                continue;
            }
            KnowledgeLevel::Detected => provisional_planet_label(&system.name, index),
            KnowledgeLevel::Probed => {
                format!("{} — {:?}", planet.name, planet.kind)
            }
            KnowledgeLevel::Analyzed => format!(
                "{} — {:?} — hab {}%",
                planet.name, planet.kind, planet.habitability,
            ),
            KnowledgeLevel::Colonized => {
                let colony_name = colony.map(|value| value.name.as_str()).unwrap_or("Colonie");
                format!(
                    "{} — {} — hab {}%",
                    planet.name, colony_name, planet.habitability,
                )
            }
        };

        commands.spawn((
            Mesh3d(if level.reveals_identity() {
                assets.planet_mesh.clone()
            } else {
                assets.system_mesh.clone()
            }),
            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            orbit,
            AxialSpin {
                radians_per_second: planet_spin_speed(planet.id, planet.kind),
            },
            SelectableVisual {
                target: PickTarget::Planet {
                    system_id,
                    planet_id: planet.id,
                },
                pick_radius_px: if level.reveals_identity() && planet.kind == PlanetKind::GasGiant {
                    18.0
                } else {
                    16.0
                },
                priority: planet_pick_priority(simulation, planet.id, level),
            },
            StrategicViewEntity,
        ));
        let halo = spawn_pointer_halo(
            commands,
            assets,
            PickTarget::Planet {
                system_id,
                planet_id: planet.id,
            },
            position,
            scale * 1.65,
        );
        commands.entity(halo).insert(orbit);

        if level.reveals_identity() {
            if let Some(atmosphere) = assets.atmosphere_materials.get(&planet.kind) {
                commands.spawn((
                    Mesh3d(assets.planet_mesh.clone()),
                    MeshMaterial3d(atmosphere.clone()),
                    Transform::from_translation(position).with_scale(Vec3::splat(scale * 1.07)),
                    orbit,
                    StrategicViewEntity,
                ));
            }
            if planet.kind == PlanetKind::GasGiant {
                let ring_orbit = OrbitingVisual {
                    vertical_offset: -0.02,
                    ..orbit
                };
                commands.spawn((
                    Mesh3d(assets.ring_mesh.clone()),
                    MeshMaterial3d(assets.ring_material.clone()),
                    Transform::from_translation(ring_orbit.translation_at(0.0)).with_rotation(
                        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
                            * Quat::from_rotation_z(0.22),
                    ),
                    ring_orbit,
                    StrategicViewEntity,
                ));
            }
        }

        // MVP-030-A1: a colonized planet must be identifiable without being selected.
        // The marker keeps the self-owned tint but uses the same thin mesh as system
        // territory rings so it does not visually swallow the planet.
        if colony.is_some() {
            let ring_orbit = OrbitingVisual {
                vertical_offset: -0.03,
                ..orbit
            };
            let material = assets
                .territory_materials
                .get(&crate::presentation::territory::TerritoryTint::SelfOwned)
                .expect("territory material exists")
                .clone();
            commands.spawn((
                Mesh3d(assets.colony_ring_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(ring_orbit.translation_at(0.0))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(scale * 1.5)),
                ring_orbit,
                StrategicViewEntity,
            ));
        }

        commands.spawn((
            Text2d::new(label),
            ui_text_font(11.0),
            TextColor(Color::srgba(0.72, 0.82, 0.92, 0.86)),
            Transform::from_translation(position + Vec3::new(0.0, 1.35, 0.0))
                .with_scale(Vec3::splat(0.25)),
            OrbitingVisual {
                vertical_offset: 1.35,
                ..orbit
            },
            StrategicViewEntity,
        ));
    }
}

pub(crate) fn spawn_pointer_halo(
    commands: &mut Commands,
    assets: &VisualAssets,
    target: PickTarget,
    position: Vec3,
    scale: f32,
) -> Entity {
    commands
        .spawn((
            Mesh3d(assets.system_mesh.clone()),
            MeshMaterial3d(assets.hover_material.clone()),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            Visibility::Hidden,
            PointerHalo { target },
            StrategicViewEntity,
        ))
        .id()
}

pub(crate) fn planet_orbit(index: usize, radius: f32, vertical_offset: f32) -> OrbitingVisual {
    OrbitingVisual {
        radius,
        phase: index as f32 * 1.37,
        angular_speed: 0.014 / (index as f32 + 1.0).sqrt(),
        vertical_offset,
    }
}

pub(crate) fn planet_visual_scale(kind: PlanetKind) -> f32 {
    match kind {
        PlanetKind::Rocky => 0.68,
        PlanetKind::Ocean => 0.82,
        PlanetKind::Desert => 0.78,
        PlanetKind::Ice => 0.74,
        PlanetKind::GasGiant => 1.32,
        PlanetKind::Volcanic => 0.70,
    }
}

pub(crate) fn planet_spin_speed(planet_id: PlanetId, kind: PlanetKind) -> f32 {
    let base = match kind {
        PlanetKind::GasGiant => 0.025,
        PlanetKind::Ocean | PlanetKind::Ice => 0.040,
        PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => 0.034,
    };
    base + (planet_id.raw() % 5) as f32 * 0.003
}

pub(crate) fn system_pick_priority(
    simulation: &Simulation,
    system_id: SystemId,
    visibility: SystemVisibility,
) -> u8 {
    if simulation
        .state()
        .colonies
        .iter()
        .any(|colony| colony.system_id == system_id)
    {
        120
    } else if visibility == SystemVisibility::Known {
        90
    } else {
        70
    }
}

pub(crate) fn planet_pick_priority(
    simulation: &Simulation,
    planet_id: PlanetId,
    level: KnowledgeLevel,
) -> u8 {
    if simulation.state().colony_on_planet(planet_id).is_some() {
        120
    } else {
        match level {
            KnowledgeLevel::Unknown => 0,
            KnowledgeLevel::Detected => 70,
            KnowledgeLevel::Probed => 85,
            KnowledgeLevel::Analyzed => 95,
            KnowledgeLevel::Colonized => 120,
        }
    }
}

pub(crate) fn spawn_ui(mut commands: Commands, icon_assets: Res<IconAssets>) {
    spawn_resource_bar(&mut commands, &icon_assets);

    commands.spawn((
        Text::new(""),
        ui_text_font(14.0),
        TextColor(Color::srgb(0.9, 0.96, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            right: Val::Px(12.0),
            top: Val::Px(64.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        Interaction::None,
        UiPointerBlocker,
        TopBarText,
        DebugOverlayRoot,
        Visibility::Hidden,
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(130.0),
                width: Val::Px(268.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
            DebugOverlayRoot,
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            spawn_panel_heading(parent, "DEBUG ( ` )");
            spawn_action_button(parent, UiAction::TogglePause, "Pause", "Space");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X1), "Vitesse x1", "1");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X2), "Vitesse x2", "2");
            spawn_action_button(parent, UiAction::SetSpeed(TimeSpeed::X4), "Vitesse x4", "3");
            spawn_action_button(parent, UiAction::ToggleDebugGraph, "Debug graphe", "G");
            spawn_action_button(parent, UiAction::RebuildView, "Reconstruire", "R");
        });

    spawn_tab_bar(&mut commands);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(14.0),
                top: Val::Px(INSPECTOR_PANEL_TOP_PX),
                width: Val::Px(348.0),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                ui_text_font(14.0),
                TextColor(Color::srgb(0.82, 0.90, 0.98)),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
                InspectorTextRole::Title,
            ));

            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(4.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    InspectorTabBarRoot,
                ))
                .with_children(|row| {
                    for index in 0..INSPECTOR_TAB_COUNT {
                        spawn_inspector_tab_button(row, index);
                    }
                });

            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px(300.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                })
                .with_children(|scroll| {
                    scroll.spawn((
                        Text::new(""),
                        ui_text_font(12.0),
                        TextColor(Color::srgb(0.82, 0.90, 0.98)),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        InspectorTextRole::Body,
                    ));
                });

            parent.spawn((
                Text::new(""),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.70, 0.78, 0.84)),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
                InspectorTextRole::Footer,
            ));
        });

    commands.spawn((
        Text::new(help_panel_text()),
        ui_text_font(12.0),
        TextColor(Color::srgb(0.76, 0.84, 0.90)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            bottom: Val::Px(68.0),
            width: Val::Px(560.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.022, 0.026, 0.030, 0.72)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.60, 0.50, 0.34, 0.35),
        ),
        Visibility::Inherited,
        HelpText,
    ));

    commands.spawn((
        Text::new(""),
        ui_text_font(12.0),
        TextColor(Color::srgb(0.88, 0.96, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(258.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.030, 0.94)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.28, 0.92, 0.82, 0.58),
        ),
        Visibility::Hidden,
        Interaction::None,
        UiPointerBlocker,
        PointerTooltipText,
    ));

    commands.spawn((
        Text::new(""),
        ui_text_font(13.0),
        TextColor(Color::srgb(0.88, 0.94, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            bottom: Val::Px(58.0),
            width: Val::Px(440.0),
            margin: UiRect::left(Val::Px(-220.0)),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.018, 0.025, 0.034, 0.96)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.74, 0.68, 0.34, 0.70),
        ),
        Visibility::Hidden,
        Interaction::None,
        UiPointerBlocker,
        AmbiguityPanelText,
    ));

    spawn_colony_management_screen(&mut commands);
    spawn_intro_pitch_modal(&mut commands);
    spawn_victory_modal(&mut commands);
}

fn spawn_intro_pitch_modal(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(14.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(220),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Inherited,
            IntroPitchRoot,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(690.0),
                        max_width: Val::Percent(96.0),
                        max_height: Val::Percent(94.0),
                        min_height: Val::Px(0.0),
                        padding: UiRect::all(Val::Px(22.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.020, 0.026, 0.034, 0.98)),
                    Outline::new(
                        Val::Px(1.0),
                        Val::ZERO,
                        Color::srgba(0.70, 0.76, 0.86, 0.56),
                    ),
                    Interaction::None,
                    UiPointerBlocker,
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("INTRODUCTION"),
                        ui_text_font(22.0),
                        TextColor(Color::srgb(0.90, 0.96, 1.0)),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ));

                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            position_type: PositionType::Relative,
                            ..default()
                        })
                        .with_children(|frame| {
                            frame
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        min_height: Val::Px(0.0),
                                        padding: UiRect::right(Val::Px(12.0)),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ScrollPosition::default(),
                                    RelativeCursorPosition::default(),
                                    ScrollIndicatorArea {
                                        id: ScrollIndicatorId::IntroPitch,
                                    },
                                ))
                                .with_children(|scroll| {
                                    scroll.spawn((
                                        Text::new(intro_pitch_text()),
                                        ui_text_font(14.0),
                                        TextColor(Color::srgb(0.78, 0.86, 0.92)),
                                        Node {
                                            width: Val::Percent(100.0),
                                            ..default()
                                        },
                                    ));
                                });
                            spawn_scroll_indicator(frame, ScrollIndicatorId::IntroPitch);
                        });

                    panel
                        .spawn((
                            Button,
                            Node {
                                align_self: AlignSelf::End,
                                min_width: Val::Px(132.0),
                                min_height: Val::Px(38.0),
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.18, 0.22, 0.98)),
                            Outline::new(
                                Val::Px(1.0),
                                Val::ZERO,
                                Color::srgba(0.34, 0.82, 0.92, 0.68),
                            ),
                            IntroPitchCloseButton,
                            UiPointerBlocker,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Commencer"),
                                ui_text_font(13.0),
                                TextColor(Color::srgb(0.88, 0.98, 1.0)),
                            ));
                        });
                });
        });
}

fn spawn_victory_modal(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(14.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.66)),
            GlobalZIndex(230),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Hidden,
            VictoryModalRoot,
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(730.0),
                        max_width: Val::Percent(96.0),
                        max_height: Val::Percent(94.0),
                        min_height: Val::Px(0.0),
                        padding: UiRect::all(Val::Px(22.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.018, 0.030, 0.026, 0.985)),
                    Outline::new(
                        Val::Px(1.0),
                        Val::ZERO,
                        Color::srgba(0.64, 0.92, 0.70, 0.66),
                    ),
                    Interaction::None,
                    UiPointerBlocker,
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("DIRECTIVE RÉGIONALE VALIDÉE"),
                        ui_text_font(22.0),
                        TextColor(Color::srgb(0.90, 1.0, 0.92)),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ));

                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            position_type: PositionType::Relative,
                            ..default()
                        })
                        .with_children(|frame| {
                            frame
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        min_height: Val::Px(0.0),
                                        padding: UiRect::right(Val::Px(12.0)),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ScrollPosition::default(),
                                    RelativeCursorPosition::default(),
                                    ScrollIndicatorArea {
                                        id: ScrollIndicatorId::VictoryDirective,
                                    },
                                ))
                                .with_children(|scroll| {
                                    scroll.spawn((
                                        Text::new(""),
                                        ui_text_font(13.0),
                                        TextColor(Color::srgb(0.78, 0.90, 0.82)),
                                        Node {
                                            width: Val::Percent(100.0),
                                            ..default()
                                        },
                                        VictoryDirectiveText,
                                    ));
                                });
                            spawn_scroll_indicator(frame, ScrollIndicatorId::VictoryDirective);
                        });

                    panel
                        .spawn((
                            Button,
                            Node {
                                align_self: AlignSelf::End,
                                min_width: Val::Px(248.0),
                                min_height: Val::Px(38.0),
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.07, 0.17, 0.12, 0.98)),
                            Outline::new(
                                Val::Px(1.0),
                                Val::ZERO,
                                Color::srgba(0.42, 0.88, 0.58, 0.72),
                            ),
                            VictoryContinueButton,
                            UiPointerBlocker,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("Continuer l’administration du secteur"),
                                ui_text_font(12.0),
                                TextColor(Color::srgb(0.88, 1.0, 0.90)),
                            ));
                        });
                });
        });
}

pub(crate) fn spawn_scroll_indicator(parent: &mut ChildSpawnerCommands, id: ScrollIndicatorId) {
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(3.0),
                top: Val::Px(4.0),
                bottom: Val::Px(4.0),
                width: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.58, 0.70, 0.76, 0.20)),
            Visibility::Hidden,
            FocusPolicy::Pass,
            ScrollIndicatorTrack { id },
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    min_height: Val::Px(18.0),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.82, 0.96, 1.0, 0.70)),
                FocusPolicy::Pass,
                ScrollIndicatorThumb { id },
            ));
        });
}

fn spawn_tab_bar(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                bottom: Val::Px(14.0),
                height: Val::Px(46.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_cyan()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|row| {
            spawn_tab_bar_slot(row, spawn_galaxy_tab_button);
            spawn_tab_bar_slot(row, spawn_colony_management_toggle);
            spawn_tab_bar_slot(row, research_ui::spawn_research_toggle);
            spawn_tab_bar_slot(row, craft_ui::spawn_craft_toggle);
            spawn_tab_bar_slot(row, fleet_ui::spawn_fleet_toggle);
            spawn_tab_bar_slot(row, objectives_ui::spawn_objectives_toggle);
            spawn_tab_bar_slot(row, navigation_ui::spawn_search_toggle);
            row.spawn(Node {
                width: Val::Px(42.0),
                ..default()
            })
            .with_children(spawn_help_toggle_button);
        });
}

/// Wraps a toggle button (each written to fill 100% of its own parent, the
/// stacked "COMMANDES" layout it originally shipped with) in a `flex_grow`
/// slot so the bottom tab bar can lay the same, untouched buttons out
/// horizontally with even spacing.
fn spawn_tab_bar_slot(
    row: &mut ChildSpawnerCommands,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    row.spawn(Node {
        flex_grow: 1.0,
        ..default()
    })
    .with_children(build);
}

fn spawn_galaxy_tab_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.05, 0.06, 0.96)),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_cyan()),
            TabBarGalaxyButton,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Galaxie"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.85, 0.98, 1.0)),
            ));
        });
}

#[derive(Component)]
pub(crate) struct HelpToggleButton;

fn spawn_help_toggle_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.09, 0.11, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.76, 0.84, 0.90, 0.50),
            ),
            HelpToggleButton,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("?"),
                ui_text_font(14.0),
                TextColor(Color::srgb(0.86, 0.92, 0.98)),
                HelpToggleText,
            ));
        });
}

pub(crate) fn handle_tab_bar_galaxy_button(
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut history: ResMut<NavigationHistory>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut open_panel: ResMut<OpenPanel>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<TabBarGalaxyButton>)>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            *open_panel = OpenPanel::None;
            navigate_to_galaxy(&mut simulation, &mut navigation, &mut history, &mut rebuild);
        }
    }
}

pub(crate) fn handle_help_toggle_button(
    mut help: ResMut<HelpUiState>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<HelpToggleButton>)>,
) {
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            help.visible = !help.visible;
        }
    }
}

pub(crate) fn handle_intro_pitch_buttons(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut intro: ResMut<IntroPitchUiState>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<IntroPitchCloseButton>)>,
) {
    if !intro.visible {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) || keyboard.just_pressed(KeyCode::Enter) {
        intro.visible = false;
        return;
    }
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            intro.visible = false;
            return;
        }
    }
}

pub(crate) fn handle_victory_modal_buttons(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut victory: ResMut<VictoryUiState>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<VictoryContinueButton>)>,
) {
    if !victory.visible {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        victory.visible = false;
        return;
    }
    for interaction in &interactions {
        if *interaction == Interaction::Pressed {
            victory.visible = false;
            return;
        }
    }
}

pub(crate) fn update_victory_state(
    simulation: Res<SimulationResource>,
    mut victory: ResMut<VictoryUiState>,
) {
    if victory.achieved_once {
        return;
    }
    let simulation = simulation.simulation();
    let progress = evaluate_victory_progress(
        simulation.state(),
        simulation.universe_repository(),
        victory_rules(),
    );
    if progress.is_complete() {
        victory.achieved_once = true;
        victory.visible = true;
    }
}

pub(crate) fn update_victory_modal(
    victory: Res<VictoryUiState>,
    simulation: Res<SimulationResource>,
    mut roots: Query<&mut Visibility, With<VictoryModalRoot>>,
    mut texts: Query<&mut Text, With<VictoryDirectiveText>>,
) {
    for mut visibility in &mut roots {
        let next = if victory.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    if !victory.visible {
        return;
    }
    let simulation = simulation.simulation();
    let progress = evaluate_victory_progress(
        simulation.state(),
        simulation.universe_repository(),
        victory_rules(),
    );
    let body = victory_directive_text(progress);
    for mut text in &mut texts {
        if text.0 != body {
            text.0 = body.clone();
        }
    }
}

pub(crate) fn handle_scroll_areas(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut scrolls: Query<(
        &ScrollIndicatorArea,
        &mut ScrollPosition,
        &ComputedNode,
        Option<&InheritedVisibility>,
        Option<&RelativeCursorPosition>,
    )>,
) {
    for wheel in mouse_wheel.read() {
        let mut applied = false;
        for (_area, mut position, computed, visibility, cursor) in &mut scrolls {
            if !scroll_area_can_scroll(computed, visibility) {
                continue;
            }
            if !cursor.is_some_and(RelativeCursorPosition::cursor_over) {
                continue;
            }
            position.y =
                scroll_position_y(position.y, wheel.y, wheel.unit, scroll_max_offset(computed));
            applied = true;
            break;
        }
        if applied {
            continue;
        }

        let mut fallback = None;
        let mut candidate_count = 0;
        for (area, _position, computed, visibility, _cursor) in &mut scrolls {
            if scroll_area_can_scroll(computed, visibility) {
                candidate_count += 1;
                fallback = Some(area.id);
            }
        }
        if candidate_count != 1 {
            continue;
        }
        let Some(target) = fallback else {
            continue;
        };
        for (area, mut position, computed, visibility, _cursor) in &mut scrolls {
            if area.id == target && scroll_area_can_scroll(computed, visibility) {
                position.y =
                    scroll_position_y(position.y, wheel.y, wheel.unit, scroll_max_offset(computed));
                break;
            }
        }
    }
}

pub(crate) fn update_scroll_indicators(
    areas: Query<(
        &ScrollIndicatorArea,
        &ScrollPosition,
        &ComputedNode,
        Option<&InheritedVisibility>,
    )>,
    mut tracks: Query<(&ScrollIndicatorTrack, &mut Visibility)>,
    mut thumbs: Query<(&ScrollIndicatorThumb, &mut Node)>,
) {
    for (track, mut visibility) in &mut tracks {
        let visible = areas
            .iter()
            .any(|(area, _position, computed, area_visibility)| {
                area.id == track.id && scroll_area_can_scroll(computed, area_visibility)
            });
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }

    for (thumb, mut node) in &mut thumbs {
        let metrics = areas
            .iter()
            .find(|(area, _position, computed, visibility)| {
                area.id == thumb.id && scroll_area_can_scroll(computed, *visibility)
            })
            .and_then(|(_area, position, computed, _visibility)| {
                scroll_indicator_metrics(position.y, computed)
            });
        let Some((top_percent, height_percent)) = metrics else {
            continue;
        };
        node.top = Val::Percent(top_percent);
        node.height = Val::Percent(height_percent);
    }
}

fn scroll_area_can_scroll(
    computed: &ComputedNode,
    visibility: Option<&InheritedVisibility>,
) -> bool {
    let visible = visibility
        .map(|visibility| visibility.get())
        .unwrap_or(true);
    visible && scroll_max_offset(computed) > 1.0
}

fn scroll_max_offset(computed: &ComputedNode) -> f32 {
    (computed.content_size().y - computed.size().y).max(0.0) * computed.inverse_scale_factor()
}

fn scroll_indicator_metrics(position_y: f32, computed: &ComputedNode) -> Option<(f32, f32)> {
    let viewport_height = computed.size().y;
    let content_height = computed.content_size().y;
    if viewport_height <= 0.0 || content_height <= viewport_height + 1.0 {
        return None;
    }
    let height_percent = ((viewport_height / content_height) * 100.0).clamp(12.0, 100.0);
    let max_offset = scroll_max_offset(computed);
    let progress = if max_offset <= 0.0 {
        0.0
    } else {
        (position_y / max_offset).clamp(0.0, 1.0)
    };
    let top_percent = progress * (100.0 - height_percent);
    Some((top_percent, height_percent))
}

fn scroll_position_y(current: f32, wheel_y: f32, unit: MouseScrollUnit, max_offset: f32) -> f32 {
    let dy = match unit {
        MouseScrollUnit::Line => wheel_y * 24.0,
        MouseScrollUnit::Pixel => wheel_y,
    };
    (current - dy).clamp(0.0, max_offset)
}

fn victory_directive_text(progress: VictoryProgress) -> String {
    let technology = technology_definition(victory_rules().required_technology);
    [
        "Le secteur dispose désormais d'une autonomie opérationnelle provisoire.".to_string(),
        "L'approvisionnement régional en Ambre de phase est considéré comme sécurisé selon les tolérances actuellement publiables.".to_string(),
        "La présence Sylve a été ramenée à un niveau compatible avec les opérations civiles.".to_string(),
        String::new(),
        "RÉSUMÉ DE CONFORMITÉ".to_string(),
        victory_progress_line("Colonies opérationnelles", progress.colonies),
        victory_progress_line("Systèmes sondés", progress.probed_systems),
        format!(
            "{} : {}",
            technology.name,
            if progress.required_technology.complete() {
                "validée"
            } else {
                "en attente"
            }
        ),
        victory_progress_line("Récoltes distantes livrées", progress.completed_harvests),
        victory_progress_line("Présences Sylves analysées", progress.sylve_analysis_reports),
        victory_progress_line("Croissances Sylves sécurisées", progress.sylve_attack_victories),
        String::new(),
        "La directive est validée. La poursuite de l'administration du secteur reste autorisée, recommandée et naturellement attendue.".to_string(),
    ]
    .join("\n")
}

fn victory_progress_line(label: &str, progress: VictoryConditionProgress) -> String {
    let status = if progress.complete() {
        "validé"
    } else {
        "en attente"
    };
    format!(
        "{label} : {}/{} ({status})",
        progress.current, progress.required
    )
}

pub(crate) fn update_help_visibility(
    help: Res<HelpUiState>,
    mut texts: Query<&mut Visibility, With<HelpText>>,
) {
    let visibility = if help.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut text in &mut texts {
        if *text != visibility {
            *text = visibility;
        }
    }
}

pub(crate) fn update_intro_pitch_visibility(
    intro: Res<IntroPitchUiState>,
    mut roots: Query<&mut Visibility, With<IntroPitchRoot>>,
) {
    let visibility = if intro.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut root in &mut roots {
        if *root != visibility {
            *root = visibility;
        }
    }
}

pub(crate) fn intro_pitch_text() -> &'static str {
    "Le monde d’avant était à bout de souffle.\n\
\n\
Guerres, famines, pollution, surpopulation. Les nations se disputaient les dernières ressources tandis que notre planète s’épuisait.\n\
\n\
Face à l’effondrement, les gouvernements survivants formèrent le Consortium, autour d’une idée simple :\n\
\n\
une seule direction, un seul objectif, une seule humanité.\n\
\n\
Et, pour la première fois, un avenir commun.\n\
\n\
L’espace devint notre seule chance de survie.\n\
\n\
Nous avons exploré.\n\
\n\
Colonisé.\n\
\n\
Exploité les ressources nécessaires à notre avenir.\n\
\n\
Chaque nouveau monde offrait quelques années de répit à notre civilisation.\n\
\n\
Puis quelques années devinrent des décennies.\n\
\n\
Les colonies prospérèrent.\n\
Les flottes grandirent.\n\
Nos besoins aussi.\n\
\n\
Nous avons découvert des mondes fertiles, des richesses inconnues et des formes de vie que personne n’avait imaginées.\n\
\n\
Certaines étaient inoffensives.\n\
\n\
D’autres occupaient des territoires dont notre survie finirait par dépendre.\n\
\n\
Nous avons appris à nous adapter.\n\
\n\
À négocier lorsque cela était possible.\n\
\n\
À sécuriser ce qui devait l’être.\n\
\n\
Aujourd’hui, l’humanité s’étend à travers la galaxie, apportant stabilité et protection aux mondes placés sous la responsabilité du Consortium.\n\
\n\
Vous venez d’être promu Amiral.\n\
\n\
Votre mission est simple :\n\
\n\
protéger notre peuple, garantir ses approvisionnements et étendre notre présence aussi loin que sa survie l’exigera.\n\
\n\
La galaxie est vaste.\n\
\n\
Nos besoins aussi."
}

pub(crate) fn help_panel_text() -> &'static str {
    "BRIEFING DU CONSORTIUM\n\
Mission : assurer la survie de notre peuple, sécuriser les ressources nécessaires \
et étendre notre présence aussi loin que nécessaire.\n\
Boucle conseillée : ouvrir Objectifs [O], produire -> rechercher -> sonder -> analyser -> exploiter -> coloniser.\n\
\n\
COMMANDES\n\
Souris : clic gauche sélectionner  ·  double-clic ouvrir/recentrer  ·  \
clic droit orbite  ·  clic molette déplacer  ·  molette zoom\n\
Vue : [Tab] cible suivante  ·  [P] projection  ·  [?] afficher/masquer l’aide\n\
Missions : [K] sonder  ·  [L] analyser  ·  [M] attaquer  ·  [H] récolter  ·  [N] coloniser"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_starts_hidden_with_consortium_briefing_available() {
        assert!(!HelpUiState::default().visible);

        let text = help_panel_text();
        assert!(text.contains("BRIEFING DU CONSORTIUM"));
        assert!(text.contains("Boucle conseillée"));
        assert!(text.contains("[Tab] cible suivante"));
        assert!(!text.contains("Helldivers"));
    }

    #[test]
    fn intro_starts_visible_with_pitch_excerpt() {
        assert!(IntroPitchUiState::default().visible);

        let text = intro_pitch_text();
        assert!(text.contains("Le monde d’avant était à bout de souffle"));
        assert!(text.contains("formèrent le Consortium"));
        assert!(text.contains("Vous venez d’être promu Amiral"));
    }

    #[test]
    fn victory_directive_keeps_phase_amber_narrative_only() {
        let done = VictoryConditionProgress::new(1, 1);
        let text = victory_directive_text(VictoryProgress {
            colonies: VictoryConditionProgress::new(3, 3),
            probed_systems: VictoryConditionProgress::new(8, 8),
            required_technology: done,
            completed_harvests: done,
            sylve_analysis_reports: done,
            sylve_attack_victories: done,
        });

        assert!(!text.contains("DIRECTIVE"));
        assert!(text.contains("Ambre de phase"));
        assert!(text.contains("approvisionnement régional"));
        assert!(!text.contains("stock"));
    }

    #[test]
    fn inspector_panel_aligns_with_navigation_bar() {
        assert_eq!(INSPECTOR_PANEL_TOP_PX, navigation_ui::NAVIGATION_BAR_TOP_PX);
    }

    #[test]
    fn scroll_position_y_moves_down_and_clamps() {
        assert_eq!(
            scroll_position_y(0.0, -2.0, MouseScrollUnit::Line, 100.0),
            48.0,
        );
        assert_eq!(
            scroll_position_y(96.0, -2.0, MouseScrollUnit::Line, 100.0),
            100.0,
        );
        assert_eq!(
            scroll_position_y(12.0, 120.0, MouseScrollUnit::Pixel, 100.0),
            0.0,
        );
    }
}

pub(crate) fn spawn_colony_management_toggle(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.18, 0.19, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.28, 0.78, 0.72, 0.55),
            ),
            ManagementButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Gestion colonie"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.94, 0.90)),
                ManagementTextRole::ToggleLabel,
            ));
        });
}

pub(crate) fn spawn_colony_management_screen(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(112.0),
                bottom: Val::Px(74.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(9.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.008, 0.014, 0.020, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.30, 0.72, 0.74, 0.68),
            ),
            Visibility::Hidden,
            GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            ColonyManagementRoot,
        ))
        .with_children(|root| {
            spawn_management_header(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.70, 0.84, 0.88)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ManagementTextRole::ColonyList,
            ));
            spawn_management_resource_row(root);
            spawn_management_main_row(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.90, 0.72, 0.42)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                ManagementTextRole::Feedback,
            ));
        });
}

pub(crate) fn spawn_management_header(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(42.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    },))
        .with_children(|header| {
            header.spawn((
                Text::new("GESTION PLANÉTAIRE"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.82, 0.96, 0.96)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                ManagementTextRole::Title,
            ));

            spawn_management_small_button(
                header,
                "< Précédente",
                ManagementButtonAction::PreviousColony,
                92.0,
            );
            header.spawn((
                Text::new("Colonie"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.76, 0.86, 0.90)),
                Node {
                    width: Val::Px(250.0),
                    ..default()
                },
                ManagementTextRole::Colony,
            ));
            spawn_management_small_button(
                header,
                "Suivante >",
                ManagementButtonAction::NextColony,
                92.0,
            );
            spawn_management_small_button(
                header,
                "Fermer  [C / Échap]",
                ManagementButtonAction::Close,
                154.0,
            );
        });
}

pub(crate) fn spawn_management_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ManagementButtonAction,
    width: f32,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(width),
                min_height: Val::Px(32.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.10, 0.13, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.44, 0.62, 0.68, 0.50),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.90, 0.94)),
            ));
        });
}

/// Always-visible persistent resource bar (top of screen), condensed to icon + amount +
/// rate, as opposed to the detailed gauge cards shown inside the full-screen Colony
/// Management panel (`spawn_management_resource_card`). Reflects the active player colony
/// only — there is no empire-wide aggregate in `galactic_sim`, and inventing one here would
/// be simulation-adjacent logic outside the scope of a presentation-only overhaul.
pub(crate) fn spawn_resource_bar(commands: &mut Commands, icon_assets: &IconAssets) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                right: Val::Px(12.0),
                top: Val::Px(10.0),
                height: Val::Px(46.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, accent_cyan()),
            Interaction::None,
            UiPointerBlocker,
            ResourceBarRoot,
        ))
        .with_children(|row| {
            for kind in ResourceHudKind::ALL {
                spawn_resource_bar_card(row, icon_assets, kind);
            }
        });
}

fn spawn_resource_bar_card(
    parent: &mut ChildSpawnerCommands,
    icon_assets: &IconAssets,
    kind: ResourceHudKind,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|card| {
            spawn_icon(
                card,
                icon_assets,
                resource_bar_icon_kind(kind),
                20.0,
                resource_kind_color(kind),
            );
            card.spawn((
                Text::new("—"),
                ui_text_font(12.0),
                TextColor(resource_kind_color(kind)),
                ResourceBarCardText { kind },
            ));
        });
}

fn resource_bar_icon_kind(kind: ResourceHudKind) -> IconKind {
    match kind {
        ResourceHudKind::Metal => IconKind::Metal,
        ResourceHudKind::Crystal => IconKind::Crystal,
        ResourceHudKind::Fuel => IconKind::Fuel,
        ResourceHudKind::Energy => IconKind::Energy,
    }
}

/// Mirrors `update_colony_management_resources` (`colony_management_ui.rs`) closely, but
/// targets the always-visible bar's `ResourceBarCardText` and uses the compact
/// `resource_bar_text` formatting instead of the detailed multi-line view.
pub(crate) fn update_resource_bar(
    simulation: Res<SimulationResource>,
    mut texts: Query<(&ResourceBarCardText, &mut Text, &mut TextColor)>,
) {
    let Some(colony) = simulation.simulation().state().active_player_colony() else {
        for (_, mut text, _) in &mut texts {
            text.0 = "—".to_string();
        }
        return;
    };
    let production = galactic_sim::colony_production_snapshot(colony);

    for (card, mut text, mut color) in &mut texts {
        let view = resource_hud_view(card.kind, colony, production);
        text.0 = resource_bar_text(card.kind, colony, production);
        color.0 = status_text_color(card.kind, view.status);
    }
}

pub(crate) fn spawn_management_resource_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(94.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(8.0),
        ..default()
    },))
        .with_children(|row| {
            for kind in ResourceHudKind::ALL {
                spawn_management_resource_card(row, kind);
            }
        });
}

pub(crate) fn spawn_management_resource_card(
    parent: &mut ChildSpawnerCommands,
    kind: ResourceHudKind,
) {
    parent
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(resource_card_background(kind)),
            Outline::new(Val::Px(1.0), Val::ZERO, resource_outline_color(kind)),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(kind.title()),
                ui_text_font(11.0),
                TextColor(resource_kind_color(kind)),
                ManagementResourceCardText { kind },
            ));
            card.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(7.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.11, 0.14, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(resource_kind_color(kind)),
                    ManagementResourceGaugeFill { kind },
                ));
            });
        });
}

pub(crate) fn spawn_management_main_row(root: &mut ChildSpawnerCommands) {
    root.spawn((Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(390.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(9.0),
        ..default()
    },))
        .with_children(|row| {
            spawn_management_building_list(row);
            spawn_management_building_detail(row);
            spawn_management_queue(row);
        });
}

pub(crate) fn spawn_management_building_list(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(292.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|list| {
        list.spawn((
            Text::new("BÂTIMENTS"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.72, 0.88, 0.92)),
        ));
        for definition in galactic_sim::default_building_catalog().definitions() {
            spawn_management_building_button(list, definition.kind);
        }
    });
}

pub(crate) fn spawn_management_building_button(
    parent: &mut ChildSpawnerCommands,
    kind: galactic_sim::BuildingKind,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(37.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.055, 0.070, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.30, 0.44, 0.50, 0.42),
            ),
            ManagementButtonAction::SelectBuilding(kind),
            ManagementBuildingButton { kind },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.82, 0.88, 0.90)),
                ManagementBuildingButtonText { kind },
            ));
        });
}

pub(crate) fn spawn_management_building_detail(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|detail| {
        detail.spawn((
            Text::new("Sélectionne un bâtiment."),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.84, 0.90, 0.94)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ManagementTextRole::BuildingDetail,
        ));
        detail
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(42.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.28, 0.24, 0.98)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.30, 0.88, 0.72, 0.70),
                ),
                ManagementButtonAction::UpgradeSelected,
                ManagementUpgradeButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("AMÉLIORER"),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.86, 0.98, 0.92)),
                    ManagementTextRole::UpgradeLabel,
                ));
            });
    });
}

pub(crate) fn spawn_management_queue(row: &mut ChildSpawnerCommands) {
    row.spawn((
        Node {
            width: Val::Px(306.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|queue| {
        queue.spawn((
            Text::new("FILE DE CONSTRUCTION"),
            ui_text_font(12.0),
            TextColor(Color::srgb(0.72, 0.88, 0.92)),
        ));
        queue
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.11, 0.14, 0.96)),
            ))
            .with_children(|gauge| {
                gauge.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.36, 0.84, 0.72)),
                    ManagementQueueProgressFill,
                ));
            });
        queue.spawn((
            Text::new("File vide."),
            ui_text_font(11.0),
            TextColor(Color::srgb(0.78, 0.84, 0.88)),
            ManagementTextRole::Queue,
        ));
        queue
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(30.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.30, 0.09, 0.09, 0.94)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    Color::srgba(0.86, 0.40, 0.36, 0.60),
                ),
                ManagementButtonAction::CancelConstruction,
                CancelConstructionButton,
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Annuler la construction en cours"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(1.0, 0.80, 0.78)),
                ));
            });
    });
}

/// Fixed pool of inspector tab buttons. Matches the largest section count any
/// `InspectorContent` ever produces (a colonized planet: Aperçu, Renseignement, Économie,
/// Potentiel, Infrastructure); extra slots are hidden via `Display::None`.
pub(crate) const INSPECTOR_TAB_COUNT: usize = 5;

fn spawn_inspector_tab_button(parent: &mut ChildSpawnerCommands<'_>, index: usize) {
    parent
        .spawn((
            Button,
            Node {
                min_height: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                action_button_outline(true, false, &Interaction::None),
            ),
            InspectorTabButton { index },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.88, 0.94, 0.98)),
                InspectorTabButtonLabel,
            ));
        });
}

pub(crate) fn spawn_panel_heading(parent: &mut ChildSpawnerCommands<'_>, label: &str) {
    parent.spawn((
        Text::new(label),
        ui_text_font(11.0),
        TextColor(Color::srgb(0.62, 0.86, 0.78)),
        Node {
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        },
    ));
}

pub(crate) fn spawn_action_button(
    parent: &mut ChildSpawnerCommands<'_>,
    action: UiAction,
    label: &str,
    shortcut: &str,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.58, 0.72, 0.76, 0.30),
            ),
            ActionButton { action },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(13.0),
                TextColor(Color::srgb(0.90, 0.95, 0.96)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            button.spawn((
                Text::new(shortcut),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.70, 0.76, 0.72)),
            ));
        });
}

pub(crate) fn ui_text_font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub(crate) fn panel_background() -> Color {
    Color::srgba(0.016, 0.020, 0.024, 0.84)
}

pub(crate) fn panel_outline() -> Color {
    Color::srgba(0.28, 0.56, 0.62, 0.42)
}

pub(crate) fn action_button_color(
    available: bool,
    active: bool,
    interaction: &Interaction,
) -> Color {
    if !available {
        return Color::srgba(0.050, 0.052, 0.052, 0.56);
    }
    if active {
        return match interaction {
            Interaction::Pressed => Color::srgba(0.22, 0.62, 0.52, 0.95),
            Interaction::Hovered => Color::srgba(0.18, 0.52, 0.46, 0.92),
            Interaction::None => Color::srgba(0.14, 0.42, 0.38, 0.88),
        };
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.26, 0.36, 0.42, 0.94),
        Interaction::Hovered => Color::srgba(0.18, 0.30, 0.35, 0.90),
        Interaction::None => Color::srgba(0.075, 0.095, 0.105, 0.86),
    }
}

pub(crate) fn action_button_outline(
    available: bool,
    active: bool,
    interaction: &Interaction,
) -> Color {
    if !available {
        return Color::srgba(0.30, 0.32, 0.32, 0.24);
    }
    if active {
        return Color::srgba(0.34, 0.92, 0.72, 0.70);
    }
    match interaction {
        Interaction::Pressed | Interaction::Hovered => Color::srgba(0.72, 0.74, 0.52, 0.64),
        Interaction::None => Color::srgba(0.58, 0.72, 0.76, 0.30),
    }
}

pub(crate) fn accent_cyan() -> Color {
    Color::srgba(0.38, 0.92, 0.98, 0.78)
}

// Palette d'accent par domaine (docs/mvp_architecture.md, refonte visuelle Étape 8).
pub(crate) fn accent_fleet_blue() -> Color {
    Color::srgba(0.42, 0.62, 0.94, 0.60)
}

pub(crate) fn accent_craft_amber() -> Color {
    Color::srgba(0.94, 0.60, 0.24, 0.60)
}

pub(crate) fn accent_research_violet() -> Color {
    Color::srgba(0.54, 0.58, 0.96, 0.58)
}

// Pas encore câblée : le panneau de gestion de colonie (presentation/colony_management_ui.rs)
// est hors périmètre strict de l'Étape 8 (limitée aux 4 panneaux fleet/craft/research/navigation).
#[allow(dead_code)]
pub(crate) fn accent_colony_teal() -> Color {
    Color::srgba(0.30, 0.72, 0.74, 0.68)
}

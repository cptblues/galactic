use bevy::prelude::*;
use galactic_domain::SystemId;
use galactic_sim::{MissionPhase, MissionTarget, Simulation, SystemVisibility};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::presentation::components::*;
use crate::presentation::graphics_settings::{GraphicsPreset, GraphicsSettings};
use crate::presentation::scene::{galaxy_star_visual_profile, planet_orbit};
use crate::presentation::shortcuts::selected_system;
use crate::presentation::strategic_navigation::*;
use crate::presentation::universe_labels::*;
use crate::*;

pub(crate) fn update_projection_transition(
    time: Res<Time>,
    mut navigation: ResMut<StrategicNavigation>,
) {
    const TRANSITION_SECONDS: f32 = 0.65;
    let current = navigation.projection_mix;
    let target = navigation.projection.target_mix();
    if current == target {
        return;
    }
    let step = time.delta_secs() / TRANSITION_SECONDS;
    let next = advance_projection_mix(current, target, step);
    if next != current {
        navigation.projection_mix = next;
    }
}

pub(crate) fn advance_projection_mix(current: f32, target: f32, maximum_step: f32) -> f32 {
    let maximum_step = maximum_step.max(0.0);
    if current < target {
        (current + maximum_step).min(target)
    } else {
        (current - maximum_step).max(target)
    }
    .clamp(0.0, 1.0)
}

pub(crate) fn update_system_visuals(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SystemVisual, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let selected_system = selected_system(simulation.simulation().state().selected);

    for (visual, mut transform) in &mut query {
        if let Some(system) = simulation.simulation().universe().system(visual.id) {
            let next = projected_universe_position(system.position, navigation.projection_mix);
            if transform.translation != next {
                transform.translation = next;
            }
        }
        let selected_multiplier = if Some(visual.id) == selected_system {
            1.55
        } else {
            1.0
        };
        let lod_multiplier = match navigation.lod {
            UniverseLod::Overview => 0.78,
            UniverseLod::Regional => 0.92,
            UniverseLod::Local => 1.08,
        };
        let next_scale = visual.base_scale * selected_multiplier * lod_multiplier;
        if transform.scale != next_scale {
            transform.scale = next_scale;
        }
    }
}

pub(crate) fn update_orbiting_visuals(
    time: Res<Time>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&OrbitingVisual, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::System(_)) {
        return;
    }

    let elapsed = time.elapsed_secs();
    for (orbit, mut transform) in &mut query {
        let next = orbit.translation_at(elapsed);
        if transform.translation != next {
            transform.translation = next;
        }
    }
}

pub(crate) fn update_planet_spins(
    time: Res<Time>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&AxialSpin, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::System(_)) {
        return;
    }

    let delta = time.delta_secs().min(0.1);
    for (spin, mut transform) in &mut query {
        transform.rotate_y(spin.radians_per_second * delta);
    }
}

pub(crate) fn compute_label_budget(
    time: Res<Time>,
    navigation: Res<StrategicNavigation>,
    simulation: Res<SimulationResource>,
    graphics: Res<crate::presentation::graphics_settings::GraphicsSettings>,
    mut budget: ResMut<LabelBudgetState>,
    system_labels: Query<&SystemLabel>,
) {
    let now = time.elapsed_secs();
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        budget.systems.clear();
        return;
    }

    let simulation = simulation.simulation();
    let state = simulation.state();
    let universe = simulation.universe();
    let selected = selected_system(state.selected);
    let focus = Vec2::new(navigation.universe_focus.x, navigation.universe_focus.z);

    let mut always_visible = Vec::new();
    let mut candidates: Vec<(SystemId, f32)> = Vec::new();

    for label in &system_labels {
        let Some(system) = universe.system(label.id) else {
            continue;
        };
        let is_selected = Some(label.id) == selected;
        let is_colony = state
            .colonies
            .iter()
            .any(|colony| colony.system_id == label.id);
        let naive_visible = is_selected
            || is_colony
            || match navigation.lod {
                UniverseLod::Overview => false,
                UniverseLod::Regional => label.visibility == SystemVisibility::Known,
                UniverseLod::Local => true,
            };
        if !naive_visible {
            continue;
        }
        if is_selected || is_colony {
            always_visible.push(label.id);
            continue;
        }
        let position = projected_universe_position(system.position, navigation.projection_mix);
        let distance = Vec2::new(position.x, position.z).distance(focus);
        let base = if label.visibility == SystemVisibility::Known {
            20.0
        } else {
            10.0
        };
        candidates.push((label.id, base - distance * 0.02));
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let budget_limit = label_budget_for_lod(navigation.lod, graphics.preset);
    let min_separation = navigation.universe_distance * LABEL_MIN_SEPARATION_FACTOR;

    let mut accepted_positions = Vec::new();
    let mut winning: HashSet<SystemId> = always_visible.iter().copied().collect();
    for id in &always_visible {
        if let Some(system) = universe.system(*id) {
            let position = projected_universe_position(system.position, navigation.projection_mix);
            accepted_positions.push(Vec2::new(position.x, position.z));
        }
    }

    let mut extra_accepted = 0usize;
    for (id, _priority) in &candidates {
        if let Some(limit) = budget_limit
            && extra_accepted >= limit
        {
            break;
        }
        let Some(system) = universe.system(*id) else {
            continue;
        };
        let position = projected_universe_position(system.position, navigation.projection_mix);
        let point = Vec2::new(position.x, position.z);
        if accepted_positions
            .iter()
            .any(|other| labels_overlap(point, *other, min_separation))
        {
            continue;
        }
        accepted_positions.push(point);
        winning.insert(*id);
        extra_accepted += 1;
    }

    let mut next = HashMap::new();
    for label in &system_labels {
        let currently_winning = winning.contains(&label.id);
        let previous = budget.systems.get(&label.id).copied();
        next.insert(
            label.id,
            advance_label_memory(previous, currently_winning, now, LABEL_HIDE_DELAY_SECONDS),
        );
    }
    budget.systems = next;
}

pub(crate) fn update_system_labels(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    budget: Res<LabelBudgetState>,
    mut query: Query<(&SystemLabel, &mut Transform, &mut Visibility)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let state = simulation.simulation().state();
    let selected = selected_system(state.selected);

    for (label, mut transform, mut visibility) in &mut query {
        if let Some(system) = simulation.simulation().universe().system(label.id) {
            let tier = state
                .system_visibility(label.id)
                .map(UniverseSystemTier::from_visibility)
                .unwrap_or(UniverseSystemTier::Observed);
            let profile = galaxy_star_visual_profile(
                system.id,
                system.star.class,
                system.star.luminosity,
                tier,
                system.position,
            );
            let next = projected_universe_position(system.position, navigation.projection_mix)
                + Vec3::new(0.0, profile.label_offset_y, 0.0);
            if transform.translation != next {
                transform.translation = next;
            }
        }
        let is_selected = Some(label.id) == selected;
        let is_colony = state
            .colonies
            .iter()
            .any(|colony| colony.system_id == label.id);

        let budget_allows = budget
            .systems
            .get(&label.id)
            .is_none_or(|memory| memory.visible);
        let should_show = is_selected
            || is_colony
            || (match navigation.lod {
                UniverseLod::Overview => false,
                UniverseLod::Regional => label.visibility == SystemVisibility::Known,
                UniverseLod::Local => true,
            } && budget_allows);

        let next_visibility = if should_show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
    }
}

pub(crate) fn update_sector_labels(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut query: Query<(&SectorLabel, &mut Transform, &mut Visibility, &mut Text2d)>,
) {
    let should_show = matches!(navigation.mode, StrategicViewMode::Universe)
        && navigation.lod == UniverseLod::Overview;

    let mission_counts = if should_show {
        let simulation = simulation.simulation();
        group_missions_by_sector(
            simulation
                .state()
                .player_missions()
                .filter(|mission| !mission.phase.is_terminal()),
            simulation.universe_repository(),
        )
    } else {
        HashMap::new()
    };

    for (label, mut transform, mut visibility, mut text) in &mut query {
        let next = projected_universe_position(label.position, navigation.projection_mix)
            + Vec3::new(0.0, 4.2, 0.0);
        if transform.translation != next {
            transform.translation = next;
        }
        let next_visibility = if should_show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
        let next_text = match mission_counts.get(&label.id) {
            Some(count) if *count > 0 => format!("{} • {} mission(s)", label.base_text, count),
            _ => label.base_text.clone(),
        };
        if text.0 != next_text {
            text.0 = next_text;
        }
    }
}

pub(crate) fn update_pointer_halo_positions(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut halos: Query<(&PointerHalo, &mut Transform)>,
) {
    if !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    for (halo, mut transform) in &mut halos {
        let PickTarget::System(system_id) = halo.target else {
            continue;
        };
        if let Some(system) = simulation.simulation().universe().system(system_id) {
            let next = projected_universe_position(system.position, navigation.projection_mix);
            if transform.translation != next {
                transform.translation = next;
            }
        }
    }
}

pub(crate) fn draw_strategic_overlays(
    mut gizmos: Gizmos,
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    selected_mission: Res<SelectedMission>,
    time: Res<Time>,
) {
    match navigation.mode {
        StrategicViewMode::Universe => {
            draw_universe_routes(&mut gizmos, simulation.simulation(), &navigation);
            draw_universe_missions(&mut gizmos, simulation.simulation(), &navigation);
            draw_selected_mission_highlight(
                &mut gizmos,
                simulation.simulation(),
                &navigation,
                selected_mission.0,
            );
        }
        StrategicViewMode::System(system_id) => {
            draw_system_orbits(&mut gizmos, simulation.simulation(), system_id);
            draw_system_missions(
                &mut gizmos,
                simulation.simulation(),
                system_id,
                time.elapsed_secs(),
                selected_mission.0,
            );
        }
    }
}

pub(crate) fn draw_selected_mission_highlight(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    navigation: &StrategicNavigation,
    selected_mission: Option<galactic_domain::MissionId>,
) {
    let Some(mission_id) = selected_mission else {
        return;
    };
    let Some(mission) = simulation.state().mission(mission_id) else {
        return;
    };
    if mission.phase.is_terminal() || mission.plan.route.len() < 2 {
        return;
    }

    let universe = simulation.universe();
    for pair in mission.plan.route.windows(2) {
        draw_route(
            gizmos,
            universe,
            pair[0],
            pair[1],
            navigation.projection_mix,
            RouteVisualStyle {
                color: Color::srgba(1.0, 0.92, 0.42, 0.96),
                dash_length: 2.6,
                gap_length: 0.35,
            },
        );
    }

    if let Some(origin) = mission
        .plan
        .route
        .first()
        .and_then(|id| universe.system(*id))
    {
        draw_circle_xz(
            gizmos,
            projected_universe_position(origin.position, navigation.projection_mix),
            2.4,
            32,
            Color::srgba(0.42, 0.96, 0.52, 0.96),
        );
    }
    if let Some(target) = mission
        .plan
        .route
        .last()
        .and_then(|id| universe.system(*id))
    {
        draw_circle_xz(
            gizmos,
            projected_universe_position(target.position, navigation.projection_mix),
            2.4,
            32,
            Color::srgba(0.98, 0.66, 0.28, 0.96),
        );
    }
}

pub(crate) fn draw_universe_routes(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    navigation: &StrategicNavigation,
) {
    let universe = simulation.universe();
    let state = simulation.state();

    if navigation.debug_full_graph {
        for route in &universe.routes {
            draw_route(
                gizmos,
                universe,
                route.from,
                route.to,
                navigation.projection_mix,
                RouteVisualStyle {
                    color: Color::srgba(0.42, 0.24, 0.62, 0.28),
                    dash_length: 1.25,
                    gap_length: 1.10,
                },
            );
        }
        return;
    }

    for route in state.visible_routes(simulation.universe_repository()) {
        let both_known = state.is_system_known(route.from) && state.is_system_known(route.to);
        let crosses_sector = simulation
            .universe_repository()
            .sector_for_system(route.from)
            .zip(simulation.universe_repository().sector_for_system(route.to))
            .is_some_and(|(from, to)| from.id != to.id);
        let color = if crosses_sector && both_known {
            Color::srgba(0.96, 0.66, 0.26, 0.72)
        } else if crosses_sector {
            Color::srgba(0.72, 0.48, 0.24, 0.46)
        } else if both_known {
            Color::srgba(0.28, 0.62, 0.94, 0.58)
        } else {
            Color::srgba(0.30, 0.48, 0.66, 0.38)
        };
        let (dash_length, gap_length) = if crosses_sector {
            (2.10, 0.90)
        } else if both_known {
            (1.60, 0.72)
        } else {
            (1.15, 1.00)
        };
        draw_route(
            gizmos,
            universe,
            route.from,
            route.to,
            navigation.projection_mix,
            RouteVisualStyle {
                color,
                dash_length,
                gap_length,
            },
        );
    }
}

pub(crate) fn draw_route(
    gizmos: &mut Gizmos,
    universe: &galactic_domain::UniverseDefinition,
    from_id: SystemId,
    to_id: SystemId,
    projection_mix: f32,
    style: RouteVisualStyle,
) {
    let Some(from) = universe.system(from_id) else {
        return;
    };
    let Some(to) = universe.system(to_id) else {
        return;
    };
    draw_dashed_line(
        gizmos,
        projected_universe_position(from.position, projection_mix),
        projected_universe_position(to.position, projection_mix),
        style.dash_length,
        style.gap_length,
        style.color,
    );
}

pub(crate) fn draw_dashed_line(
    gizmos: &mut Gizmos,
    start: Vec3,
    end: Vec3,
    dash_length: f32,
    gap_length: f32,
    color: Color,
) {
    let delta = end - start;
    let length = delta.length();
    let dash_length = dash_length.max(0.05);
    let gap_length = gap_length.max(0.0);
    if length <= f32::EPSILON {
        return;
    }

    let direction = delta / length;
    let step = dash_length + gap_length;
    let segment_count = dashed_segment_count(length, dash_length, gap_length);
    for segment in 0..segment_count {
        let offset = segment as f32 * step;
        let dash_end = (offset + dash_length).min(length);
        gizmos.line(
            start + direction * offset,
            start + direction * dash_end,
            color,
        );
    }
}

pub(crate) fn dashed_segment_count(length: f32, dash_length: f32, gap_length: f32) -> usize {
    if length <= 0.0 || dash_length <= 0.0 {
        return 0;
    }
    (length / (dash_length + gap_length.max(0.0))).ceil() as usize
}

pub(crate) fn draw_system_orbits(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    system_id: SystemId,
) {
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };

    for index in 0..system.planets.len() {
        let radius = 6.0 + index as f32 * 4.8;
        draw_circle_xz(
            gizmos,
            Vec3::ZERO,
            radius,
            48,
            Color::srgba(0.32, 0.46, 0.62, 0.26),
        );
    }
}

pub(crate) fn draw_circle_xz(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    segments: usize,
    color: Color,
) {
    for segment in 0..segments {
        let start_angle = segment as f32 / segments as f32 * std::f32::consts::TAU;
        let end_angle = (segment + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let start = center + Vec3::new(start_angle.cos() * radius, 0.0, start_angle.sin() * radius);
        let end = center + Vec3::new(end_angle.cos() * radius, 0.0, end_angle.sin() * radius);
        gizmos.line(start, end, color);
    }
}

pub(crate) fn draw_universe_missions(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    navigation: &StrategicNavigation,
) {
    let current_tick = simulation.state().clock.current_tick();
    for mission in simulation
        .state()
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal() && mission.plan.route.len() > 1)
    {
        let Some(progress) = mission_route_progress(mission, current_tick) else {
            continue;
        };
        let Some(position) = mission_route_position(
            simulation,
            &mission.plan.route,
            progress,
            navigation.projection_mix,
        ) else {
            continue;
        };
        draw_probe_marker(gizmos, position, 1.15);
    }
}

pub(crate) fn draw_system_missions(
    gizmos: &mut Gizmos,
    simulation: &Simulation,
    system_id: SystemId,
    elapsed_seconds: f32,
    selected_mission: Option<galactic_domain::MissionId>,
) {
    let current_tick = simulation.state().clock.current_tick();
    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };
    for mission in simulation.state().player_missions().filter(|mission| {
        !mission.phase.is_terminal()
            && mission.order.origin == system_id
            && matches!(
                mission.order.target,
                MissionTarget::Planet {
                    system_id: target_system,
                    ..
                } if target_system == system_id
            )
    }) {
        let MissionTarget::Planet { planet_id, .. } = mission.order.target else {
            continue;
        };
        let Some(origin_colony) = simulation.state().colony(mission.origin_colony_id) else {
            continue;
        };
        let Some(origin_index) = system
            .planets
            .iter()
            .position(|planet| planet.id == origin_colony.planet_id)
        else {
            continue;
        };
        let Some(target_index) = system
            .planets
            .iter()
            .position(|planet| planet.id == planet_id)
        else {
            continue;
        };
        let Some(progress) = mission_route_progress(mission, current_tick) else {
            continue;
        };
        let origin_radius = 6.0 + origin_index as f32 * 4.8;
        let target_radius = 6.0 + target_index as f32 * 4.8;
        let origin =
            planet_orbit(origin_index, origin_radius, 0.32).translation_at(elapsed_seconds);
        let target =
            planet_orbit(target_index, target_radius, 0.32).translation_at(elapsed_seconds);
        let is_selected = selected_mission == Some(mission.id);
        let line_color = if is_selected {
            Color::srgba(1.0, 0.92, 0.42, 0.96)
        } else {
            Color::srgba(0.32, 0.88, 0.96, 0.24)
        };
        gizmos.line(origin, target, line_color);
        if is_selected {
            draw_circle_xz(
                gizmos,
                origin,
                1.1,
                24,
                Color::srgba(0.42, 0.96, 0.52, 0.96),
            );
            draw_circle_xz(
                gizmos,
                target,
                1.1,
                24,
                Color::srgba(0.98, 0.66, 0.28, 0.96),
            );
        }
        draw_probe_marker(gizmos, origin.lerp(target, progress), 0.62);
    }
}

pub(crate) fn mission_route_progress(
    mission: &galactic_sim::MissionState,
    current_tick: galactic_sim::StrategicTick,
) -> Option<f32> {
    let ratio = |start: galactic_sim::StrategicTick, end: galactic_sim::StrategicTick| {
        let duration = end.value().saturating_sub(start.value());
        if duration == 0 {
            return 1.0;
        }
        current_tick.value().saturating_sub(start.value()) as f32 / duration as f32
    };
    match mission.phase {
        MissionPhase::Preparation => Some(0.0),
        MissionPhase::Outbound => Some(
            ratio(mission.order.departure_at, mission.plan.outbound_arrival_at).clamp(0.0, 1.0),
        ),
        MissionPhase::OnSite => Some(1.0),
        MissionPhase::Returning => Some(
            1.0 - ratio(
                mission.plan.return_departure_at,
                mission.plan.return_arrival_at,
            )
            .clamp(0.0, 1.0),
        ),
        MissionPhase::Completed | MissionPhase::Cancelled | MissionPhase::Failed => None,
    }
}

pub(crate) fn mission_route_position(
    simulation: &Simulation,
    route: &[SystemId],
    progress: f32,
    projection_mix: f32,
) -> Option<Vec3> {
    let segments = route.len().checked_sub(1)?;
    if segments == 0 {
        return None;
    }
    let scaled = progress.clamp(0.0, 1.0) * segments as f32;
    let segment = (scaled.floor() as usize).min(segments - 1);
    let local = (scaled - segment as f32).clamp(0.0, 1.0);
    let from = simulation.universe().system(route[segment])?;
    let to = simulation.universe().system(route[segment + 1])?;
    Some(
        projected_universe_position(from.position, projection_mix).lerp(
            projected_universe_position(to.position, projection_mix),
            local,
        ),
    )
}

/// MVP-034: small, bounded engine-trail effect for fleets in transit between
/// systems — chosen over an ambient/decorative particle effect because it
/// reads as gameplay information (which fleets are moving) rather than pure
/// decoration, and because "in transit" already has a well-defined spawn
/// trigger and a natural lifetime.
#[derive(Component)]
pub(crate) struct FleetTrailParticle {
    velocity: Vec3,
    initial_lifetime: f32,
    lifetime_remaining: f32,
}

#[derive(Resource)]
pub(crate) struct FleetTrailSpawnTimer(Timer);

impl Default for FleetTrailSpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.25, TimerMode::Repeating))
    }
}

struct FleetTrailSettings {
    enabled: bool,
    spawn_interval_secs: f32,
    max_live: usize,
}

const fn fleet_trail_settings_for_preset(preset: GraphicsPreset) -> FleetTrailSettings {
    match preset {
        GraphicsPreset::Low => FleetTrailSettings {
            enabled: false,
            spawn_interval_secs: 0.25,
            max_live: 0,
        },
        GraphicsPreset::Medium => FleetTrailSettings {
            enabled: true,
            spawn_interval_secs: 0.25,
            max_live: 40,
        },
        GraphicsPreset::High => FleetTrailSettings {
            enabled: true,
            spawn_interval_secs: 0.1,
            max_live: 120,
        },
    }
}

/// Only spawns while in the Universe view — the same view
/// `draw_universe_missions` draws its in-transit marker in — since that is
/// where a fleet's continuous position along its route is meaningful.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_fleet_trail_particles(
    time: Res<Time>,
    graphics: Res<GraphicsSettings>,
    mut spawn_timer: ResMut<FleetTrailSpawnTimer>,
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    visual_assets: Res<VisualAssets>,
    particles: Query<(), With<FleetTrailParticle>>,
    mut commands: Commands,
) {
    let settings = fleet_trail_settings_for_preset(graphics.preset);
    if !settings.enabled || !matches!(navigation.mode, StrategicViewMode::Universe) {
        return;
    }

    let interval = Duration::from_secs_f32(settings.spawn_interval_secs);
    if spawn_timer.0.duration() != interval {
        spawn_timer.0.set_duration(interval);
    }
    spawn_timer.0.tick(time.delta());
    if !spawn_timer.0.just_finished() {
        return;
    }
    if particles.iter().count() >= settings.max_live {
        return;
    }

    let simulation = simulation.simulation();
    let current_tick = simulation.state().clock.current_tick();
    for mission in simulation
        .state()
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal() && mission.plan.route.len() > 1)
    {
        let Some(progress) = mission_route_progress(mission, current_tick) else {
            continue;
        };
        let Some(position) = mission_route_position(
            simulation,
            &mission.plan.route,
            progress,
            navigation.projection_mix,
        ) else {
            continue;
        };

        commands.spawn((
            Mesh3d(visual_assets.particle_mesh.clone()),
            MeshMaterial3d(visual_assets.particle_material.clone()),
            Transform::from_translation(position).with_scale(Vec3::splat(0.35)),
            FleetTrailParticle {
                velocity: Vec3::new(0.0, 0.35, 0.0),
                initial_lifetime: 1.2,
                lifetime_remaining: 1.2,
            },
        ));
    }
}

/// Fades via shrinking scale rather than alpha, so every particle can share
/// one `StandardMaterial` handle instead of needing a unique material per
/// entity just to animate its own opacity.
pub(crate) fn advance_fleet_trail_particles(
    time: Res<Time>,
    mut commands: Commands,
    mut particles: Query<(Entity, &mut Transform, &mut FleetTrailParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut particle) in &mut particles {
        particle.lifetime_remaining -= dt;
        if particle.lifetime_remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation += particle.velocity * dt;
        let life_fraction =
            (particle.lifetime_remaining / particle.initial_lifetime).clamp(0.0, 1.0);
        transform.scale = Vec3::splat(0.35 * life_fraction);
    }
}

pub(crate) fn draw_probe_marker(gizmos: &mut Gizmos, position: Vec3, radius: f32) {
    let color = Color::srgba(0.42, 0.96, 1.0, 0.96);
    gizmos.line(
        position - Vec3::X * radius,
        position + Vec3::X * radius,
        color,
    );
    gizmos.line(
        position - Vec3::Y * radius,
        position + Vec3::Y * radius,
        color,
    );
    gizmos.line(
        position - Vec3::Z * radius,
        position + Vec3::Z * radius,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_trails_are_disabled_on_low_and_denser_on_high_than_medium() {
        let low = fleet_trail_settings_for_preset(GraphicsPreset::Low);
        assert!(!low.enabled);
        assert_eq!(low.max_live, 0);

        let medium = fleet_trail_settings_for_preset(GraphicsPreset::Medium);
        assert!(medium.enabled);

        let high = fleet_trail_settings_for_preset(GraphicsPreset::High);
        assert!(high.enabled);
        assert!(high.max_live > medium.max_live);
        assert!(high.spawn_interval_secs < medium.spawn_interval_secs);
    }
}

pub mod help;
pub mod hud;
pub mod inspector;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::data::{GalaxyData, Notifications, Selection, ViewOptions};
use crate::map::{
    AmbiguousSelection, GraphicsSettings, LabelDiagnostics, MapFilters, MapProjectionMode,
    MapProjectionState, SemanticZoomState,
};
use crate::navigation::{NavigationHistory, SearchState, breadcrumb};
use crate::state::ViewState;
use crate::strategic::{MissionState, StrategicGalaxyData};
use crate::usability::UsabilityMetrics;

pub struct GalacticUiPlugin;

impl Plugin for GalacticUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui).add_systems(
            Update,
            (
                update_top_bar,
                update_inspector,
                update_help,
                update_notification,
                update_search_panel,
                update_ambiguous_panel,
                update_debug_panel,
            ),
        );
    }
}

#[derive(Component)]
struct TopBarText;

#[derive(Component)]
struct InspectorText;

#[derive(Component)]
struct HelpText;

#[derive(Component)]
struct NotificationText;

#[derive(Component)]
struct SearchText;

#[derive(Component)]
struct AmbiguousText;

#[derive(Component)]
struct DebugText;

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(17.0),
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            right: Val::Px(12.0),
            top: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.72)),
        TopBarText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.86, 0.91, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(72.0),
            right: Val::Px(18.0),
            width: Val::Px(365.0),
            padding: UiRect::all(Val::Px(14.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.017, 0.028, 0.78)),
        InspectorText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.75, 0.84, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            bottom: Val::Px(16.0),
            width: Val::Px(470.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.016, 0.026, 0.68)),
        HelpText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.92, 0.62)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(60.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.032, 0.012, 0.74)),
        Visibility::Hidden,
        NotificationText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(106.0),
            width: Val::Px(520.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.012, 0.02, 0.036, 0.82)),
        Visibility::Hidden,
        SearchText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.78)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(552.0),
            top: Val::Px(106.0),
            width: Val::Px(300.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.028, 0.012, 0.84)),
        Visibility::Hidden,
        AmbiguousText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 1.0, 0.82)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            bottom: Val::Px(18.0),
            width: Val::Px(300.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.01, 0.025, 0.014, 0.75)),
        Visibility::Hidden,
        DebugText,
    ));
}

#[derive(SystemParam)]
struct TopBarParams<'w> {
    state: Res<'w, State<ViewState>>,
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    active_system: Res<'w, crate::data::ActiveSystem>,
    selection: Res<'w, Selection>,
    filters: Res<'w, MapFilters>,
    zoom: Res<'w, SemanticZoomState>,
    projection: Res<'w, MapProjectionState>,
    history: Res<'w, NavigationHistory>,
    diagnostics: Res<'w, DiagnosticsStore>,
}

fn update_top_bar(params: TopBarParams, mut query: Query<&mut Text, With<TopBarText>>) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let fps = params
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or(0.0);
    let view = match *params.state.get() {
        ViewState::Galaxy => "GALAXIE".to_string(),
        ViewState::System => params
            .active_system
            .id
            .and_then(|id| params.galaxy.find_system(id))
            .map(|system| format!("SYSTEME : {}", system.name))
            .unwrap_or_else(|| "SYSTEME".to_string()),
    };
    let projection = match params.projection.mode {
        MapProjectionMode::ThreeDimensional => "3D",
        MapProjectionMode::Flattened => "Aplati",
    };
    text.0 = format!(
        "{view} | {} | seed {} | systemes {} | FPS {:.0} | zoom {} | {} | routes {}/{} | labels {} | hist {}/{}",
        breadcrumb(&params.galaxy, &params.strategic, params.selection.selected),
        params.galaxy.seed,
        params.galaxy.systems.len(),
        fps,
        params.zoom.level.label(),
        projection,
        on_off(params.filters.major_routes),
        on_off(params.filters.minor_routes),
        on_off(params.filters.labels),
        params
            .history
            .cursor
            .saturating_add(usize::from(!params.history.entries.is_empty())),
        params.history.entries.len()
    );
}

fn update_inspector(
    galaxy: Res<GalaxyData>,
    strategic: Res<StrategicGalaxyData>,
    selection: Res<Selection>,
    mission: Res<MissionState>,
    mut query: Query<&mut Text, With<InspectorText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    text.0 = inspector::inspector_text(&galaxy, &strategic, selection.selected, &mission);
}

fn update_help(
    options: Res<ViewOptions>,
    mut query: Query<(&mut Text, &mut Visibility), With<HelpText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };
    *visibility = if options.show_help {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    text.0 = help::HELP_TEXT.to_string();
}

fn update_notification(
    notifications: Res<Notifications>,
    mut query: Query<(&mut Text, &mut Visibility), With<NotificationText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };
    if let Some(message) = &notifications.message {
        text.0 = message.clone();
        *visibility = Visibility::Visible;
    } else {
        text.0.clear();
        *visibility = Visibility::Hidden;
    }
}

fn update_search_panel(
    search: Res<SearchState>,
    mut query: Query<(&mut Text, &mut Visibility), With<SearchText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };
    if !search.active {
        *visibility = Visibility::Hidden;
        text.0.clear();
        return;
    }
    *visibility = Visibility::Visible;
    let results = search
        .results
        .iter()
        .enumerate()
        .map(|(index, result)| format!("{}. {} - {}", index + 1, result.label, result.path))
        .collect::<Vec<_>>()
        .join("\n");
    text.0 = format!(
        "Recherche: {}\n{}",
        search.query,
        if results.is_empty() {
            "Aucun resultat".to_string()
        } else {
            results
        }
    );
}

fn update_ambiguous_panel(
    ambiguous: Res<AmbiguousSelection>,
    mut query: Query<(&mut Text, &mut Visibility), With<AmbiguousText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };
    if !ambiguous.active {
        *visibility = Visibility::Hidden;
        text.0.clear();
        return;
    }
    *visibility = Visibility::Visible;
    let rows = ambiguous
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            format!(
                "{}{} {} ({:.0}px)",
                if index == ambiguous.index { "> " } else { "  " },
                index + 1,
                candidate.label,
                candidate.screen_distance
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.0 = format!("Selection ambigue\nTab pour parcourir\nEchap pour fermer\n{rows}");
}

#[derive(SystemParam)]
struct DebugParams<'w, 's> {
    state: Res<'w, State<ViewState>>,
    galaxy: Res<'w, GalaxyData>,
    strategic: Res<'w, StrategicGalaxyData>,
    selection: Res<'w, Selection>,
    options: Res<'w, ViewOptions>,
    zoom: Res<'w, SemanticZoomState>,
    projection: Res<'w, MapProjectionState>,
    graphics: Res<'w, GraphicsSettings>,
    labels: Res<'w, LabelDiagnostics>,
    history: Res<'w, NavigationHistory>,
    metrics: Res<'w, UsabilityMetrics>,
    diagnostics: Res<'w, DiagnosticsStore>,
    transform_entities: Query<'w, 's, Entity, With<Transform>>,
}

fn update_debug_panel(
    params: DebugParams,
    mut query: Query<(&mut Text, &mut Visibility), With<DebugText>>,
) {
    let Ok((mut text, mut visibility)) = query.single_mut() else {
        return;
    };
    *visibility = if params.options.show_debug {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let fps = params
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or(0.0);
    let frame_time = params
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.smoothed())
        .unwrap_or(0.0);
    let projection = match params.projection.mode {
        MapProjectionMode::ThreeDimensional => "3D",
        MapProjectionMode::Flattened => "Aplati",
    };
    text.0 = format!(
        "Debug\nVue: {:?}\nFPS: {:.1}\nFrame: {:.2} ms\nEntites transform: {}\nSystemes: {}\nFactions: {}\nAllies: {}\nSecteurs: {}\nFlottes: {}\nZoom: {}\nMode: {} {:.2}\nGraphique: {:?}\nLabels: {}/{}\nSelection: {:?}\nHistorique: {}\nMetrics sel={} amb={} filtres={} search={} transitions={}",
        params.state.get(),
        fps,
        frame_time,
        params.transform_entities.iter().count(),
        params.galaxy.systems.len(),
        params.strategic.factions.len(),
        params.strategic.friendly_systems().len(),
        params.strategic.sectors.len(),
        params.strategic.fleets.len(),
        params.zoom.level.label(),
        projection,
        params.projection.blend,
        params.graphics.preset,
        params.labels.visible,
        params.labels.candidates,
        params.selection.selected,
        params.history.entries.len(),
        params.metrics.selection_count,
        params.metrics.ambiguous_selection_count,
        params.metrics.filter_change_count,
        params.metrics.search_count,
        params.metrics.view_transition_count
    );
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

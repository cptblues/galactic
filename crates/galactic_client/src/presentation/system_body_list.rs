// MVP visual refonte: contextual list of a system's planets with an inline
// "Coloniser" action, shown only while the strategic view is focused on that
// system. Reuses the existing colonization workflow end to end (knowledge
// gate, `assess_planet_colonizability`, `GameAction::LaunchColonization`) —
// no new simulation logic, purely a new presentation entry point alongside
// the Fleet panel's global candidate list.

use bevy::prelude::*;
use galactic_domain::{PlanetId, SystemId};
use galactic_sim::{
    GameAction, GameState, MissionTarget, UniverseRepository, assess_planet_colonizability,
};

use crate::SimulationResource;
use crate::presentation::components::UiPointerBlocker;
use crate::presentation::input::provisional_planet_label;
use crate::presentation::inspector_panel::colonization_blocker_label;
use crate::presentation::scene::{
    action_button_color, action_button_outline, panel_background, panel_outline,
    spawn_panel_heading, ui_text_font,
};
use crate::presentation::shortcuts::apply_simulation_command;
use crate::presentation::strategic_navigation::{StrategicNavigation, StrategicViewMode};

const MAX_BODY_ROWS: usize = 12;

pub(crate) struct PlanetRowView {
    pub(crate) planet_id: PlanetId,
    pub(crate) label: String,
    pub(crate) colonizable: bool,
    pub(crate) reason: String,
}

/// Builds the planet rows for a system, gated by the exact same
/// `KnowledgeLevel` rules already used by `system_inspector_content`/
/// `planet_inspector_content`: a planet must be at least `Detected` (i.e.
/// `is_visible()`) to appear at all, and its name is only resolved once
/// `reveals_identity()` (`Probed+`); otherwise a provisional orbit label is
/// shown, matching `probe_candidates`'s own convention. Colonizability
/// itself is delegated entirely to `assess_planet_colonizability`, which
/// already enforces the `Analyzed` knowledge gate internally — no gate is
/// reimplemented here.
pub(crate) fn system_body_rows(
    state: &GameState,
    universe_repository: &UniverseRepository,
    system_id: SystemId,
) -> Vec<PlanetRowView> {
    let Some(system) = universe_repository.definition().system(system_id) else {
        return Vec::new();
    };
    let actor = state.player_faction;

    system
        .planets
        .iter()
        .enumerate()
        .filter_map(|(orbit_index, planet)| {
            let level = state.planet_knowledge_level(planet.id);
            if !level.is_visible() {
                return None;
            }
            let label = if level.reveals_identity() {
                planet.name.clone()
            } else {
                provisional_planet_label(&system.name, orbit_index)
            };
            let assessment =
                assess_planet_colonizability(state, universe_repository, actor, planet.id);
            let colonizable = assessment.is_colonizable();
            let reason = if colonizable {
                "Éligible à la colonisation".to_string()
            } else {
                assessment
                    .blockers
                    .first()
                    .map(|blocker| colonization_blocker_label(*blocker, state))
                    .unwrap_or_default()
            };
            Some(PlanetRowView {
                planet_id: planet.id,
                label,
                colonizable,
                reason,
            })
        })
        .collect()
}

#[derive(Component)]
pub(crate) struct SystemBodyListRoot;

#[derive(Component)]
pub(crate) struct SystemBodyRow {
    slot: usize,
    binding: Option<PlanetId>,
}

#[derive(Component)]
pub(crate) struct SystemBodyRowLabel;

#[derive(Component)]
pub(crate) struct SystemBodyRowReason;

#[derive(Component)]
pub(crate) struct SystemBodyColonizeButton {
    slot: usize,
}

pub(crate) fn spawn_system_body_list(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                bottom: Val::Px(74.0),
                width: Val::Px(268.0),
                max_height: Val::Px(280.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Visibility::Hidden,
            Interaction::None,
            UiPointerBlocker,
            SystemBodyListRoot,
        ))
        .with_children(|parent| {
            spawn_panel_heading(parent, "CORPS DU SYSTÈME");
            for slot in 0..MAX_BODY_ROWS {
                spawn_system_body_row(parent, slot);
            }
        });
}

fn spawn_system_body_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.9)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.34, 0.46, 0.62, 0.40),
            ),
            Visibility::Hidden,
            SystemBodyRow {
                slot,
                binding: None,
            },
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.88, 0.92, 0.98)),
                SystemBodyRowLabel,
            ));
            row.spawn((
                Text::new(""),
                ui_text_font(9.5),
                TextColor(Color::srgb(0.62, 0.70, 0.78)),
                SystemBodyRowReason,
            ));
            row.spawn((
                Button,
                Node {
                    margin: UiRect::top(Val::Px(3.0)),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    align_self: AlignSelf::Start,
                    ..default()
                },
                BackgroundColor(action_button_color(false, false, &Interaction::None)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    action_button_outline(false, false, &Interaction::None),
                ),
                SystemBodyColonizeButton { slot },
                UiPointerBlocker,
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Coloniser"),
                    ui_text_font(10.0),
                    TextColor(Color::srgb(0.90, 0.94, 0.98)),
                ));
            });
        });
}

pub(crate) fn update_system_body_list_visibility(
    navigation: Res<StrategicNavigation>,
    mut root: Query<&mut Visibility, With<SystemBodyListRoot>>,
) {
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    let next = if matches!(navigation.mode, StrategicViewMode::System(_)) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != next {
        *visibility = next;
    }
}

pub(crate) fn update_system_body_rows(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    mut rows: Query<(&mut SystemBodyRow, &mut Visibility, &Children)>,
    mut labels: Query<&mut Text, (With<SystemBodyRowLabel>, Without<SystemBodyRowReason>)>,
    mut reasons: Query<&mut Text, (With<SystemBodyRowReason>, Without<SystemBodyRowLabel>)>,
    mut colonize_buttons: Query<(
        &SystemBodyColonizeButton,
        &mut BackgroundColor,
        &mut Outline,
    )>,
) {
    let StrategicViewMode::System(system_id) = navigation.mode else {
        return;
    };
    let planets = system_body_rows(
        simulation.simulation().state(),
        simulation.simulation().universe_repository(),
        system_id,
    );

    for (mut row, mut visibility, children) in &mut rows {
        let Some(view) = planets.get(row.slot) else {
            row.binding = None;
            *visibility = Visibility::Hidden;
            continue;
        };
        row.binding = Some(view.planet_id);
        *visibility = Visibility::Inherited;
        for child in children {
            if let Ok(mut text) = labels.get_mut(*child)
                && text.0 != view.label
            {
                text.0 = view.label.clone();
            }
            if let Ok(mut text) = reasons.get_mut(*child)
                && text.0 != view.reason
            {
                text.0 = view.reason.clone();
            }
        }
    }

    for (button, mut background, mut outline) in &mut colonize_buttons {
        let available = planets
            .get(button.slot)
            .is_some_and(|view| view.colonizable);
        *background = BackgroundColor(action_button_color(available, false, &Interaction::None));
        outline.color = action_button_outline(available, false, &Interaction::None);
    }
}

pub(crate) fn handle_system_body_colonize_buttons(
    mut simulation: ResMut<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    interactions: Query<(&Interaction, &SystemBodyColonizeButton), Changed<Interaction>>,
    rows: Query<&SystemBodyRow>,
) {
    let StrategicViewMode::System(system_id) = navigation.mode else {
        return;
    };
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(planet_id) = rows
            .iter()
            .find(|row| row.slot == button.slot)
            .and_then(|row| row.binding)
        else {
            continue;
        };
        let Some(colony_id) = simulation.simulation().state().active_colony_id else {
            continue;
        };
        apply_simulation_command(
            &mut simulation,
            GameAction::LaunchColonization {
                colony_id,
                target: MissionTarget::Planet {
                    system_id,
                    planet_id,
                },
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galactic_domain::UniverseConfig;
    use galactic_sim::{KnowledgeLevel, Simulation};

    fn other_system_id(simulation: &Simulation) -> SystemId {
        let home_system = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists")
            .system_id;
        simulation
            .universe_repository()
            .definition()
            .systems
            .iter()
            .map(|system| system.id)
            .find(|id| *id != home_system)
            .expect("universe has more than one system")
    }

    #[test]
    fn a_planet_known_only_at_detected_level_never_reveals_its_real_name() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let system_id = other_system_id(&simulation);
        let universe = simulation.universe_repository().clone();
        let planet_id = simulation
            .universe()
            .system(system_id)
            .expect("system exists")
            .planets
            .first()
            .expect("system has a planet")
            .id;
        let real_name = simulation
            .universe()
            .system(system_id)
            .unwrap()
            .planets
            .first()
            .unwrap()
            .name
            .clone();

        let mut state = simulation.state().clone();
        state.advance_planet_knowledge(&universe, planet_id, KnowledgeLevel::Detected);
        assert_eq!(
            state.planet_knowledge_level(planet_id),
            KnowledgeLevel::Detected
        );

        let rows = system_body_rows(&state, &universe, system_id);
        let row = rows
            .iter()
            .find(|row| row.planet_id == planet_id)
            .expect("the detected planet is still visible");
        assert_ne!(row.label, real_name);
        assert!(!row.colonizable);
    }

    #[test]
    fn a_planet_below_detected_level_does_not_appear_in_the_list() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let system_id = other_system_id(&simulation);
        let universe = simulation.universe_repository().clone();
        let planet_id = simulation
            .universe()
            .system(system_id)
            .expect("system exists")
            .planets
            .first()
            .expect("system has a planet")
            .id;
        let state = simulation.state().clone();
        assert_eq!(
            state.planet_knowledge_level(planet_id),
            KnowledgeLevel::Unknown
        );

        let rows = system_body_rows(&state, &universe, system_id);
        assert!(rows.iter().all(|row| row.planet_id != planet_id));
    }
}

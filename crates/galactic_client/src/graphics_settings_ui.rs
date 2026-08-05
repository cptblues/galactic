// MVP-034: graphics preset selector (Low/Medium/High), persisted via
// `galactic_persistence::settings`. Buttons-only panel, no text input, so it
// doesn't need to join the shared `_is_editing` keyboard-guard chain other
// panels check (navigation search, fleet rename, save naming).
use bevy::prelude::*;
use galactic_persistence::{default_settings_path, save_settings};

use crate::presentation::graphics_settings::{GraphicsPreset, GraphicsSettings};

use super::{
    OpenPanel, PresentationUpdateSet, UiPointerBlocker, panel_background, panel_outline,
    ui_text_font,
};

const GRAPHICS_SETTINGS_Z_INDEX: i32 = 120;

pub(crate) struct GraphicsSettingsUiPlugin;

impl Plugin for GraphicsSettingsUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GraphicsSettingsUiState>()
            .add_systems(Startup, spawn_graphics_settings_screen)
            .add_systems(
                Update,
                (
                    handle_graphics_settings_shortcuts,
                    handle_graphics_settings_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (update_graphics_settings_visibility, update_preset_buttons)
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Resource, Default)]
pub(crate) struct GraphicsSettingsUiState {
    feedback: String,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum GraphicsSettingsButtonAction {
    Toggle,
    Close,
    SelectPreset(GraphicsPreset),
}

type GraphicsSettingsButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static GraphicsSettingsButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct GraphicsSettingsRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum GraphicsSettingsTextRole {
    Toggle,
    Feedback,
}

#[derive(Component)]
struct PresetButton {
    preset: GraphicsPreset,
}

pub(crate) fn spawn_graphics_settings_toggle(parent: &mut ChildSpawnerCommands) {
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
            BackgroundColor(Color::srgba(0.09, 0.11, 0.10, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.56, 0.78, 0.62, 0.55),
            ),
            GraphicsSettingsButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Réglages  [K]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.84, 0.96, 0.88)),
                GraphicsSettingsTextRole::Toggle,
            ));
        });
}

fn spawn_graphics_settings_screen(mut commands: Commands) {
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
            BackgroundColor(Color::srgba(0.011, 0.016, 0.013, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.56, 0.78, 0.62, 0.74),
            ),
            Visibility::Hidden,
            GlobalZIndex(GRAPHICS_SETTINGS_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            GraphicsSettingsRoot,
        ))
        .with_children(|root| {
            spawn_graphics_settings_header(root);
            spawn_preset_row(root);
            root.spawn((
                Text::new(
                    "Bloom, HDR, ombres, particules, labels et qualité des textures suivent le \
preset choisi. Faible et Élevé redimensionnent aussi la fenêtre \
(960×540 / 1280×720 / 1600×900).",
                ),
                ui_text_font(10.5),
                TextColor(Color::srgba(0.70, 0.80, 0.74, 0.90)),
            ));
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.78, 0.90, 0.82)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                GraphicsSettingsTextRole::Feedback,
            ));
        });
}

fn spawn_graphics_settings_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("RÉGLAGES GRAPHIQUES"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.86, 0.98, 0.90)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            spawn_graphics_settings_small_button(
                header,
                "Fermer  [K / Échap]",
                GraphicsSettingsButtonAction::Close,
                160.0,
            );
        });
}

fn spawn_graphics_settings_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: GraphicsSettingsButtonAction,
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
            BackgroundColor(Color::srgba(0.10, 0.13, 0.11, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.56, 0.78, 0.62, 0.60),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.90, 0.96, 0.92)),
            ));
        });
}

fn spawn_preset_row(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(9.0),
            padding: UiRect::all(Val::Px(9.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|row| {
        spawn_preset_button(row, GraphicsPreset::Low, "Faible");
        spawn_preset_button(row, GraphicsPreset::Medium, "Moyen");
        spawn_preset_button(row, GraphicsPreset::High, "Élevé");
    });
}

fn spawn_preset_button(parent: &mut ChildSpawnerCommands, preset: GraphicsPreset, label: &str) {
    parent
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                min_height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.09, 0.07, 0.94)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.46, 0.62, 0.50, 0.40),
            ),
            GraphicsSettingsButtonAction::SelectPreset(preset),
            PresetButton { preset },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(12.5),
                TextColor(Color::srgb(0.88, 0.94, 0.90)),
            ));
        });
}

fn handle_graphics_settings_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    save_load_ui: Res<crate::save_load_ui::SaveLoadUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_name_is_editing(&fleet_ui)
        || crate::save_load_ui::save_name_is_editing(&save_load_ui)
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyK) {
        let opening = *open_panel != OpenPanel::Settings;
        *open_panel = if opening {
            OpenPanel::Settings
        } else {
            OpenPanel::None
        };
        if opening {
            navigation_ui.search_open = false;
            navigation_ui.filters_open = false;
        }
        return;
    }

    if *open_panel == OpenPanel::Settings && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
    }
}

fn handle_graphics_settings_buttons(
    mut graphics: ResMut<GraphicsSettings>,
    mut ui: ResMut<GraphicsSettingsUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: GraphicsSettingsButtonInteractionQuery,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            GraphicsSettingsButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::Settings;
                *open_panel = if opening {
                    OpenPanel::Settings
                } else {
                    OpenPanel::None
                };
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                }
            }
            GraphicsSettingsButtonAction::Close => *open_panel = OpenPanel::None,
            GraphicsSettingsButtonAction::SelectPreset(preset) => {
                graphics.preset = preset;
                match save_settings(&default_settings_path(), preset) {
                    Ok(()) => ui.feedback = "Réglage enregistré.".to_string(),
                    Err(error) => {
                        ui.feedback = format!("Le réglage n’a pas pu être enregistré : {error}");
                    }
                }
            }
        }
    }
}

fn update_graphics_settings_visibility(
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<GraphicsSettingsRoot>>,
    mut texts: Query<(&GraphicsSettingsTextRole, &mut Text)>,
    ui: Res<GraphicsSettingsUiState>,
) {
    let is_open = *open_panel == OpenPanel::Settings;
    for mut visibility in &mut roots {
        let next = if is_open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (role, mut text) in &mut texts {
        match role {
            GraphicsSettingsTextRole::Toggle => {
                let next = if is_open {
                    "Fermer réglages"
                } else {
                    "Réglages  [K]"
                };
                if text.0 != next {
                    text.0 = next.to_string();
                }
            }
            GraphicsSettingsTextRole::Feedback => {
                if text.0 != ui.feedback {
                    text.0 = ui.feedback.clone();
                }
            }
        }
    }
}

fn update_preset_buttons(
    graphics: Res<GraphicsSettings>,
    mut buttons: Query<(&PresetButton, &mut BackgroundColor, &mut Outline)>,
) {
    for (button, mut background, mut outline) in &mut buttons {
        let selected = button.preset == graphics.preset;
        let next_background = if selected {
            Color::srgba(0.16, 0.30, 0.20, 0.96)
        } else {
            Color::srgba(0.06, 0.09, 0.07, 0.94)
        };
        if background.0 != next_background {
            background.0 = next_background;
        }
        let next_outline_color = if selected {
            Color::srgba(0.56, 0.90, 0.66, 0.90)
        } else {
            Color::srgba(0.46, 0.62, 0.50, 0.40)
        };
        if outline.color != next_outline_color {
            outline.color = next_outline_color;
        }
    }
}

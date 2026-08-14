// MVP-031: manual save/load screen backed by `galactic_persistence`.
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use galactic_domain::SystemId;
use galactic_persistence::{
    SaveFileError, SaveFileHeader, SaveSlotMetadata, default_save_directory, delete_save,
    list_save_slots, load_from_path, restore_from_snapshot, save_to_path, snapshot_from_simulation,
};

use crate::presentation::{
    components::{ScrollIndicatorArea, ScrollIndicatorId},
    scene::spawn_scroll_indicator,
    strategic_navigation::StrategicViewMode,
};

use super::{
    OpenPanel, PresentationUpdateSet, SimulationResource, UiPointerBlocker, panel_background,
    panel_outline, replace_simulation, ui_text_font,
};

const SAVE_LOAD_Z_INDEX: i32 = 120;
const MAX_SAVE_SLOT_ROWS: usize = 20;
const MAX_SAVE_NAME_CHARS: usize = 48;

pub(crate) struct SaveLoadUiPlugin;

impl Plugin for SaveLoadUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveLoadUiState>()
            .init_resource::<AutosaveState>()
            .add_systems(Startup, spawn_save_load_screen)
            .add_systems(
                Update,
                tick_autosave_timer.in_set(PresentationUpdateSet::Management),
            )
            .add_systems(
                Update,
                (
                    handle_save_load_shortcuts,
                    handle_quicksave_shortcuts,
                    handle_save_name_input,
                    handle_save_load_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_save_load_visibility,
                    update_save_name_field,
                    update_save_slot_rows,
                    update_delete_button_label,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Resource, Default)]
pub(crate) struct SaveLoadUiState {
    slots: Vec<SaveSlotMetadata>,
    selected_slot: Option<usize>,
    feedback: String,
    name_buffer: String,
    name_editing: bool,
    /// Two-press delete confirmation (mirrors `combat_ui.rs`'s retreat
    /// pattern) — `Some(deadline)` while armed; expires back to `None`.
    delete_armed_until: Option<Duration>,
}

pub(crate) fn save_name_is_editing(ui: &SaveLoadUiState) -> bool {
    ui.name_editing
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum SaveLoadButtonAction {
    Toggle,
    Close,
    Save,
    Load,
    Overwrite,
    Delete,
    SelectSlot(usize),
    ToggleNameEditing,
}

type SaveLoadButtonInteractionQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static SaveLoadButtonAction),
    (Changed<Interaction>, With<Button>),
>;

#[derive(Component)]
struct SaveLoadRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum SaveLoadTextRole {
    Toggle,
    Feedback,
    NameField,
}

#[derive(Component)]
struct SaveSlotRow {
    slot: usize,
}

#[derive(Component)]
struct DeleteButtonLabel;

pub(crate) fn spawn_save_load_toggle(parent: &mut ChildSpawnerCommands) {
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
            BackgroundColor(Color::srgba(0.10, 0.09, 0.14, 0.96)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.70, 0.62, 0.90, 0.55),
            ),
            SaveLoadButtonAction::Toggle,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Sauvegardes  [J]"),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.88, 0.84, 0.98)),
                SaveLoadTextRole::Toggle,
            ));
        });
}

fn spawn_save_load_screen(mut commands: Commands) {
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
            BackgroundColor(Color::srgba(0.014, 0.012, 0.020, 0.995)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.70, 0.62, 0.90, 0.74),
            ),
            Visibility::Hidden,
            GlobalZIndex(SAVE_LOAD_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            SaveLoadRoot,
        ))
        .with_children(|root| {
            spawn_save_load_header(root);
            spawn_save_name_row(root);
            spawn_save_slot_list(root);
            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.90, 0.70, 0.70)),
                Node {
                    min_height: Val::Px(18.0),
                    ..default()
                },
                SaveLoadTextRole::Feedback,
            ));
        });
}

fn spawn_save_load_header(root: &mut ChildSpawnerCommands) {
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
                Text::new("SAUVEGARDES"),
                ui_text_font(18.0),
                TextColor(Color::srgb(0.90, 0.86, 1.0)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            spawn_save_load_small_button(header, "Enregistrer", SaveLoadButtonAction::Save, 120.0);
            spawn_save_load_small_button(header, "Charger", SaveLoadButtonAction::Load, 100.0);
            spawn_save_load_small_button(header, "Écraser", SaveLoadButtonAction::Overwrite, 100.0);
            spawn_delete_button(header);
            spawn_save_load_small_button(
                header,
                "Fermer  [J / Échap]",
                SaveLoadButtonAction::Close,
                160.0,
            );
        });
}

fn spawn_save_load_small_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: SaveLoadButtonAction,
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
            BackgroundColor(Color::srgba(0.12, 0.10, 0.17, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.70, 0.62, 0.90, 0.60),
            ),
            action,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.92, 0.88, 1.0)),
            ));
        });
}

/// Destructive — two-press confirmation (`delete_armed_until`), label swaps
/// between "Supprimer" and "Confirmer ?" via `DeleteButtonLabel`, mirroring
/// `combat_ui.rs`'s retreat-confirmation pattern.
fn spawn_delete_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(140.0),
                min_height: Val::Px(32.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.10, 0.17, 0.98)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.70, 0.62, 0.90, 0.60),
            ),
            SaveLoadButtonAction::Delete,
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("Supprimer"),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.92, 0.88, 1.0)),
                DeleteButtonLabel,
            ));
        });
}

fn spawn_save_name_row(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Button,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(30.0),
            padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.05, 0.09, 0.94)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            Color::srgba(0.70, 0.62, 0.90, 0.40),
        ),
        SaveLoadButtonAction::ToggleNameEditing,
        UiPointerBlocker,
    ))
    .with_children(|field| {
        field.spawn((
            Text::new("Nom (clique pour saisir) : Sauvegarde <horodatage>"),
            ui_text_font(11.5),
            TextColor(Color::srgb(0.88, 0.86, 0.96)),
            SaveLoadTextRole::NameField,
        ));
    });
}

fn spawn_save_slot_list(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            position_type: PositionType::Relative,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
    ))
    .with_children(|frame| {
        frame
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    min_height: Val::Px(0.0),
                    padding: UiRect::all(Val::Px(9.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                ScrollIndicatorArea {
                    id: ScrollIndicatorId::SaveSlotList,
                },
            ))
            .with_children(|list| {
                for slot in 0..MAX_SAVE_SLOT_ROWS {
                    spawn_save_slot_row(list, slot);
                }
            });
        spawn_scroll_indicator(frame, ScrollIndicatorId::SaveSlotList);
    });
}

fn spawn_save_slot_row(parent: &mut ChildSpawnerCommands, slot: usize) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(28.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.05, 0.09, 0.94)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                Color::srgba(0.46, 0.40, 0.62, 0.40),
            ),
            Visibility::Hidden,
            SaveLoadButtonAction::SelectSlot(slot),
            SaveSlotRow { slot },
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.88, 0.86, 0.96)),
            ));
        });
}

/// Howard Hinnant's `civil_from_days`: days-since-epoch to a proleptic
/// Gregorian (year, month, day), with no date/time crate dependency for a
/// single cosmetic column.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

fn format_save_timestamp(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds_of_day = unix_seconds % 86_400;
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Client-only, Bevy-free mirror of `StrategicViewMode` — `galactic_persistence`
/// must not depend on Bevy, so the "which view was the player in" bit of
/// "navigation pertinente" is persisted as a sidecar file next to the save
/// rather than through the shared `SaveGame` envelope. Camera position/zoom
/// is cosmetic and intentionally not covered here.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum ClientViewMode {
    Universe,
    System(SystemId),
}

impl From<StrategicViewMode> for ClientViewMode {
    fn from(mode: StrategicViewMode) -> Self {
        match mode {
            StrategicViewMode::Universe => Self::Universe,
            StrategicViewMode::System(system_id) => Self::System(system_id),
        }
    }
}

/// Deliberately does NOT end in `.ron` — `list_save_slots` scans the save
/// directory for `.ron` files, and the sidecar must not be mistaken for a
/// second (invalid) save slot.
fn nav_sidecar_path(save_path: &std::path::Path) -> PathBuf {
    PathBuf::from(format!("{}.nav", save_path.display()))
}

fn write_nav_sidecar(save_path: &std::path::Path, mode: ClientViewMode) {
    if let Ok(text) = ron::ser::to_string(&mode) {
        let _ = std::fs::write(nav_sidecar_path(save_path), text);
    }
}

/// Never surfaces an error: a missing or unreadable sidecar just means the
/// caller falls back to the galaxy view, per the "navigation pertinente"
/// requirement's own "never an error" spirit.
fn read_nav_sidecar(save_path: &std::path::Path) -> Option<ClientViewMode> {
    let text = std::fs::read_to_string(nav_sidecar_path(save_path)).ok()?;
    ron::de::from_str(&text).ok()
}

fn slugify(name: &str) -> String {
    let slug: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect();
    let trimmed = slug.trim_matches('_');
    if trimmed.is_empty() {
        "sauvegarde".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn refresh_save_slots(ui: &mut SaveLoadUiState) {
    ui.slots = list_save_slots(&default_save_directory());
    ui.selected_slot = None;
}

fn save_current_game(
    simulation: &SimulationResource,
    navigation: &super::StrategicNavigation,
    ui: &mut SaveLoadUiState,
) {
    let timestamp = unix_seconds_now();
    let trimmed = ui.name_buffer.trim();
    let display_name = if trimmed.is_empty() {
        format!("Sauvegarde {timestamp}")
    } else {
        trimmed.to_string()
    };
    let header = SaveFileHeader {
        display_name: display_name.clone(),
        saved_at_unix_seconds: timestamp,
    };
    let save = snapshot_from_simulation(simulation.simulation());
    let path = default_save_directory().join(format!("{}-{timestamp}.ron", slugify(&display_name)));

    match save_to_path(&path, header, &save) {
        Ok(()) => {
            write_nav_sidecar(&path, ClientViewMode::from(navigation.mode));
            ui.feedback = format!("Partie enregistrée ({display_name}).");
            refresh_save_slots(ui);
        }
        Err(error) => {
            ui.feedback = save_file_error_message(&error);
        }
    }
}

fn load_selected_slot(
    simulation: &mut SimulationResource,
    rebuild: &mut super::ViewRebuildRequest,
    navigation: &mut super::StrategicNavigation,
    ui: &mut SaveLoadUiState,
) {
    let Some(index) = ui.selected_slot else {
        ui.feedback = "Sélectionne une sauvegarde à charger.".to_string();
        return;
    };
    let Some(slot) = ui.slots.get(index) else {
        ui.feedback = "Sélectionne une sauvegarde à charger.".to_string();
        return;
    };
    if slot.corrupted {
        ui.feedback = "Ce fichier de sauvegarde est corrompu.".to_string();
        return;
    }

    let result = load_from_path(&slot.path)
        .and_then(|(_, save)| restore_from_snapshot(&save).map_err(SaveFileError::from));

    match result {
        Ok(loaded) => {
            replace_simulation(simulation, loaded, rebuild);
            match read_nav_sidecar(&slot.path) {
                Some(ClientViewMode::System(system_id)) => navigation.enter_system(system_id),
                Some(ClientViewMode::Universe) | None => navigation.exit_system(),
            }
            ui.feedback = "Partie chargée.".to_string();
        }
        Err(error) => {
            ui.feedback = save_file_error_message(&error);
        }
    }
}

/// Rewrites the selected slot's own file in place — same path, fresh
/// timestamp, same display name (an in-place update, not a rename) —
/// instead of `save_current_game`'s always-new-file behavior.
fn overwrite_selected_slot(
    simulation: &SimulationResource,
    navigation: &super::StrategicNavigation,
    ui: &mut SaveLoadUiState,
) {
    let Some(index) = ui.selected_slot else {
        ui.feedback = "Sélectionne une sauvegarde à écraser.".to_string();
        return;
    };
    let Some(slot) = ui.slots.get(index) else {
        ui.feedback = "Sélectionne une sauvegarde à écraser.".to_string();
        return;
    };
    let path = slot.path.clone();
    let display_name = slot.display_name.clone();
    let header = SaveFileHeader {
        display_name: display_name.clone(),
        saved_at_unix_seconds: unix_seconds_now(),
    };
    let save = snapshot_from_simulation(simulation.simulation());

    match save_to_path(&path, header, &save) {
        Ok(()) => {
            write_nav_sidecar(&path, ClientViewMode::from(navigation.mode));
            ui.feedback = format!("Sauvegarde écrasée ({display_name}).");
            refresh_save_slots(ui);
        }
        Err(error) => {
            ui.feedback = save_file_error_message(&error);
        }
    }
}

/// Two-press confirmation, mirroring `combat_ui.rs`'s
/// `CombatUiAction::Retreat` handler: first call (or a call after the 3s
/// window expired) arms it, a second call inside the window deletes.
fn delete_selected_slot(ui: &mut SaveLoadUiState, now: Duration) {
    let Some(index) = ui.selected_slot else {
        ui.feedback = "Sélectionne une sauvegarde à supprimer.".to_string();
        return;
    };
    let Some(slot) = ui.slots.get(index).cloned() else {
        ui.feedback = "Sélectionne une sauvegarde à supprimer.".to_string();
        return;
    };

    match ui.delete_armed_until {
        Some(deadline) if now <= deadline => {
            ui.delete_armed_until = None;
            match delete_save(&slot.path) {
                Ok(()) => {
                    std::fs::remove_file(nav_sidecar_path(&slot.path)).ok();
                    ui.feedback = format!("Sauvegarde supprimée ({}).", slot.display_name);
                    refresh_save_slots(ui);
                }
                Err(error) => {
                    ui.feedback = save_file_error_message(&error);
                }
            }
        }
        _ => {
            ui.delete_armed_until = Some(now + Duration::from_secs(3));
        }
    }
}

fn save_file_error_message(error: &SaveFileError) -> String {
    match error {
        SaveFileError::Incompatible(_) => {
            "Cette sauvegarde n’est plus compatible avec la partie actuelle.".to_string()
        }
        SaveFileError::Corrupted { .. } | SaveFileError::Io { .. } => {
            "Fichier de sauvegarde illisible ou corrompu.".to_string()
        }
    }
}

fn quicksave_path() -> PathBuf {
    default_save_directory().join("quicksave.ron")
}

fn quicksave(simulation: &SimulationResource, ui: &mut SaveLoadUiState) {
    let save = snapshot_from_simulation(simulation.simulation());
    let header = SaveFileHeader {
        display_name: "Sauvegarde rapide".to_string(),
        saved_at_unix_seconds: unix_seconds_now(),
    };
    match save_to_path(&quicksave_path(), header, &save) {
        Ok(()) => ui.feedback = "Sauvegarde rapide effectuée (F5).".to_string(),
        Err(error) => ui.feedback = save_file_error_message(&error),
    }
}

fn quickload(
    simulation: &mut SimulationResource,
    rebuild: &mut super::ViewRebuildRequest,
    ui: &mut SaveLoadUiState,
) {
    let path = quicksave_path();
    if !path.exists() {
        ui.feedback = "Aucune sauvegarde rapide disponible.".to_string();
        return;
    }

    let result = load_from_path(&path)
        .and_then(|(_, save)| restore_from_snapshot(&save).map_err(SaveFileError::from));

    match result {
        Ok(loaded) => {
            replace_simulation(simulation, loaded, rebuild);
            ui.feedback = "Sauvegarde rapide chargée (F9).".to_string();
        }
        Err(error) => {
            ui.feedback = save_file_error_message(&error);
        }
    }
}

fn handle_quicksave_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<SaveLoadUiState>,
    mut rebuild: ResMut<super::ViewRebuildRequest>,
    navigation_ui: Res<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    craft_ui: Res<crate::craft_ui::CraftUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_text_input_is_active(&fleet_ui)
        || crate::craft_ui::craft_quantity_is_editing(&craft_ui)
        || ui.name_editing
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::F5) {
        quicksave(&simulation, &mut ui);
    } else if keyboard.just_pressed(KeyCode::F9) {
        quickload(&mut simulation, &mut rebuild, &mut ui);
    }
}

const AUTOSAVE_SLOT_COUNT: usize = 3;
const AUTOSAVE_INTERVAL_SECS: u64 = 300;

#[derive(Resource)]
struct AutosaveState {
    timer: Timer,
}

impl Default for AutosaveState {
    fn default() -> Self {
        Self {
            timer: Timer::new(
                std::time::Duration::from_secs(AUTOSAVE_INTERVAL_SECS),
                TimerMode::Repeating,
            ),
        }
    }
}

fn autosave_path(slot: usize) -> PathBuf {
    default_save_directory().join(format!("autosave-{slot}.ron"))
}

/// The slot to overwrite next: the one with the oldest `saved_at_unix_seconds`
/// — a slot that was never written (or is unreadable) reads as `0`, the
/// smallest possible value, so empty slots are always filled before any
/// existing autosave is overwritten.
fn oldest_autosave_slot() -> usize {
    (0..AUTOSAVE_SLOT_COUNT)
        .min_by_key(|&slot| {
            load_from_path(&autosave_path(slot))
                .map(|(header, _)| header.saved_at_unix_seconds)
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

fn run_autosave(simulation: &SimulationResource, ui: &mut SaveLoadUiState) {
    let slot = oldest_autosave_slot();
    let save = snapshot_from_simulation(simulation.simulation());
    let header = SaveFileHeader {
        display_name: format!("Sauvegarde automatique {}", slot + 1),
        saved_at_unix_seconds: unix_seconds_now(),
    };
    if save_to_path(&autosave_path(slot), header, &save).is_ok() {
        ui.feedback = format!(
            "Sauvegarde automatique effectuée (emplacement {}).",
            slot + 1
        );
    }
}

fn tick_autosave_timer(
    time: Res<Time>,
    mut autosave: ResMut<AutosaveState>,
    simulation: Res<SimulationResource>,
    mut ui: ResMut<SaveLoadUiState>,
) {
    autosave.timer.tick(time.delta());
    if autosave.timer.just_finished() {
        run_autosave(&simulation, &mut ui);
    }
}

fn handle_save_load_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<SaveLoadUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    fleet_ui: Res<crate::fleet_ui::FleetUiState>,
    craft_ui: Res<crate::craft_ui::CraftUiState>,
) {
    if super::navigation_ui::navigation_text_or_filter_is_active(&navigation_ui)
        || crate::fleet_ui::fleet_text_input_is_active(&fleet_ui)
        || crate::craft_ui::craft_quantity_is_editing(&craft_ui)
        || ui.name_editing
    {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyJ) {
        let opening = *open_panel != OpenPanel::SaveLoad;
        *open_panel = if opening {
            OpenPanel::SaveLoad
        } else {
            OpenPanel::None
        };
        ui.feedback.clear();
        ui.delete_armed_until = None;
        if opening {
            navigation_ui.search_open = false;
            navigation_ui.filters_open = false;
            refresh_save_slots(&mut ui);
        }
        return;
    }

    if *open_panel == OpenPanel::SaveLoad && keyboard.just_pressed(KeyCode::Escape) {
        *open_panel = OpenPanel::None;
        ui.delete_armed_until = None;
    }
}

fn handle_save_name_input(
    mut events: MessageReader<KeyboardInput>,
    open_panel: Res<OpenPanel>,
    mut ui: ResMut<SaveLoadUiState>,
) {
    if *open_panel != OpenPanel::SaveLoad || !ui.name_editing {
        return;
    }

    for event in events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Backspace => {
                ui.name_buffer.pop();
            }
            KeyCode::Enter | KeyCode::Escape => {
                ui.name_editing = false;
            }
            _ => {
                if let Some(text) = &event.text {
                    for ch in text.chars() {
                        if !ch.is_control() && ui.name_buffer.chars().count() < MAX_SAVE_NAME_CHARS
                        {
                            ui.name_buffer.push(ch);
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_save_load_buttons(
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<SaveLoadUiState>,
    mut open_panel: ResMut<OpenPanel>,
    mut rebuild: ResMut<super::ViewRebuildRequest>,
    mut navigation: ResMut<super::StrategicNavigation>,
    mut navigation_ui: ResMut<super::navigation_ui::NavigationUiState>,
    interactions: SaveLoadButtonInteractionQuery,
    time: Res<Time>,
) {
    for (interaction, action) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            SaveLoadButtonAction::Toggle => {
                let opening = *open_panel != OpenPanel::SaveLoad;
                *open_panel = if opening {
                    OpenPanel::SaveLoad
                } else {
                    OpenPanel::None
                };
                ui.feedback.clear();
                ui.delete_armed_until = None;
                if opening {
                    navigation_ui.search_open = false;
                    navigation_ui.filters_open = false;
                    refresh_save_slots(&mut ui);
                }
            }
            SaveLoadButtonAction::Close => {
                *open_panel = OpenPanel::None;
                ui.delete_armed_until = None;
            }
            SaveLoadButtonAction::Save => save_current_game(&simulation, &navigation, &mut ui),
            SaveLoadButtonAction::Load => {
                load_selected_slot(&mut simulation, &mut rebuild, &mut navigation, &mut ui);
            }
            SaveLoadButtonAction::Overwrite => {
                overwrite_selected_slot(&simulation, &navigation, &mut ui);
            }
            SaveLoadButtonAction::Delete => {
                delete_selected_slot(&mut ui, time.elapsed());
            }
            SaveLoadButtonAction::SelectSlot(slot) => {
                ui.selected_slot = Some(slot);
                ui.feedback.clear();
                ui.delete_armed_until = None;
            }
            SaveLoadButtonAction::ToggleNameEditing => {
                ui.name_editing = !ui.name_editing;
            }
        }
    }
}

fn update_save_load_visibility(
    open_panel: Res<OpenPanel>,
    mut roots: Query<&mut Visibility, With<SaveLoadRoot>>,
    mut texts: Query<(&SaveLoadTextRole, &mut Text)>,
    ui: Res<SaveLoadUiState>,
) {
    let is_open = *open_panel == OpenPanel::SaveLoad;
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
            SaveLoadTextRole::Toggle => {
                let next = if is_open {
                    "Fermer sauvegardes"
                } else {
                    "Sauvegardes  [J]"
                };
                if text.0 != next {
                    text.0 = next.to_string();
                }
            }
            SaveLoadTextRole::Feedback => {
                if text.0 != ui.feedback {
                    text.0 = ui.feedback.clone();
                }
            }
            SaveLoadTextRole::NameField => {}
        }
    }
}

fn update_save_name_field(
    ui: Res<SaveLoadUiState>,
    mut texts: Query<(&SaveLoadTextRole, &mut Text)>,
) {
    let label = if ui.name_editing {
        format!("Nom : {}_", ui.name_buffer)
    } else if ui.name_buffer.trim().is_empty() {
        "Nom (clique pour saisir) : Sauvegarde <horodatage>".to_string()
    } else {
        format!("Nom : {}", ui.name_buffer)
    };
    for (role, mut text) in &mut texts {
        if *role == SaveLoadTextRole::NameField && text.0 != label {
            text.0 = label.clone();
        }
    }
}

fn update_delete_button_label(
    ui: Res<SaveLoadUiState>,
    time: Res<Time>,
    mut texts: Query<&mut Text, With<DeleteButtonLabel>>,
) {
    let armed = ui
        .delete_armed_until
        .is_some_and(|deadline| time.elapsed() <= deadline);
    let label = if armed { "Confirmer ?" } else { "Supprimer" };
    if let Ok(mut text) = texts.single_mut()
        && text.0 != label
    {
        text.0 = label.to_string();
    }
}

fn update_save_slot_rows(
    ui: Res<SaveLoadUiState>,
    mut rows: Query<(
        &SaveSlotRow,
        &mut Visibility,
        &mut BackgroundColor,
        &Children,
    )>,
    mut texts: Query<&mut Text>,
) {
    for (row, mut visibility, mut background, children) in &mut rows {
        let Some(metadata) = ui.slots.get(row.slot) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;

        let selected = ui.selected_slot == Some(row.slot);
        let next_background = if metadata.corrupted {
            Color::srgba(0.20, 0.06, 0.06, 0.70)
        } else if selected {
            Color::srgba(0.22, 0.18, 0.34, 0.94)
        } else {
            Color::srgba(0.06, 0.05, 0.09, 0.94)
        };
        if background.0 != next_background {
            background.0 = next_background;
        }

        let label = if metadata.corrupted {
            format!("⚠ {} — fichier corrompu", metadata.display_name)
        } else {
            let minutes = metadata.playtime_seconds / 60;
            format!(
                "{}  ·  {}  ·  v{}  ·  {} colonie(s)  ·  {} min de jeu",
                metadata.display_name,
                format_save_timestamp(metadata.saved_at_unix_seconds),
                metadata.save_version,
                metadata.colony_count,
                minutes
            )
        };
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != label
            {
                text.0 = label.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::Simulation;

    use super::*;
    use crate::presentation::strategic_navigation::{StrategicNavigation, ViewRebuildRequest};

    /// `GALACTIC_SAVE_DIR` is process-global state, and `cargo test` runs tests
    /// on multiple threads by default — this lock keeps the env-var-dependent
    /// tests in this module from stomping on each other's directory.
    static SAVE_DIR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct TempSaveDir {
        path: std::path::PathBuf,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl TempSaveDir {
        fn new(label: &str) -> Self {
            let guard = SAVE_DIR_ENV_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let path = std::env::temp_dir().join(format!(
                "galactic-save-ui-test-{label}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("temp save dir is creatable");
            // SAFETY: serialized by SAVE_DIR_ENV_LOCK, held for this guard's lifetime.
            unsafe {
                std::env::set_var("GALACTIC_SAVE_DIR", &path);
            }
            Self {
                path,
                _guard: guard,
            }
        }
    }

    impl Drop for TempSaveDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.path).ok();
            unsafe {
                std::env::remove_var("GALACTIC_SAVE_DIR");
            }
        }
    }

    #[test]
    fn save_then_load_round_trips_through_the_button_handlers_own_logic() {
        let _dir = TempSaveDir::new("roundtrip");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();

        save_current_game(&source, &StrategicNavigation::default(), &mut ui);
        assert!(
            ui.feedback.starts_with("Partie enregistrée"),
            "unexpected feedback: {}",
            ui.feedback
        );
        assert_eq!(ui.slots.len(), 1);
        assert!(!ui.slots[0].corrupted);

        ui.selected_slot = Some(0);
        let mut destination = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: vec![],
        };
        let mut rebuild = ViewRebuildRequest(false);

        load_selected_slot(
            &mut destination,
            &mut rebuild,
            &mut StrategicNavigation::default(),
            &mut ui,
        );

        assert_eq!(ui.feedback, "Partie chargée.");
        assert!(rebuild.0, "loading a save must request a full view rebuild");
        assert_eq!(
            destination.simulation().state().colonies.len(),
            source.simulation().state().colonies.len(),
        );
    }

    #[test]
    fn loading_restores_the_system_view_the_player_was_in_when_saving() {
        let _dir = TempSaveDir::new("nav-sidecar");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let system_id = source
            .simulation()
            .universe()
            .systems
            .first()
            .expect("reference universe has at least one system")
            .id;
        let mut save_navigation = StrategicNavigation::default();
        save_navigation.enter_system(system_id);
        let mut ui = SaveLoadUiState::default();

        save_current_game(&source, &save_navigation, &mut ui);
        ui.selected_slot = Some(0);

        let mut destination = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut rebuild = ViewRebuildRequest(false);
        let mut load_navigation = StrategicNavigation::default();

        load_selected_slot(
            &mut destination,
            &mut rebuild,
            &mut load_navigation,
            &mut ui,
        );

        assert_eq!(load_navigation.mode, StrategicViewMode::System(system_id));
    }

    #[test]
    fn loading_a_save_without_a_sidecar_falls_back_to_the_galaxy_view() {
        let _dir = TempSaveDir::new("nav-sidecar-missing");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        save_current_game(&source, &StrategicNavigation::default(), &mut ui);
        // Remove the sidecar to simulate a save written before this feature existed.
        std::fs::remove_file(nav_sidecar_path(&ui.slots[0].path)).expect("sidecar exists");
        ui.selected_slot = Some(0);

        let mut destination = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut rebuild = ViewRebuildRequest(false);
        let mut load_navigation = StrategicNavigation::default();
        load_navigation.enter_system(
            source
                .simulation()
                .universe()
                .systems
                .first()
                .expect("reference universe has at least one system")
                .id,
        );

        load_selected_slot(
            &mut destination,
            &mut rebuild,
            &mut load_navigation,
            &mut ui,
        );

        assert_eq!(load_navigation.mode, StrategicViewMode::Universe);
    }

    #[test]
    fn loading_without_a_selected_slot_reports_feedback_and_does_not_touch_the_simulation() {
        let _dir = TempSaveDir::new("no-selection");

        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        let mut rebuild = ViewRebuildRequest(false);

        load_selected_slot(
            &mut simulation,
            &mut rebuild,
            &mut StrategicNavigation::default(),
            &mut ui,
        );

        assert!(!rebuild.0);
        assert!(!ui.feedback.is_empty());
    }

    #[test]
    fn loading_a_corrupted_slot_is_reported_and_does_not_touch_the_simulation() {
        let dir = TempSaveDir::new("corrupted");
        std::fs::write(dir.path.join("broken.ron"), b"not ron at all").expect("write succeeds");

        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        refresh_save_slots(&mut ui);
        assert_eq!(ui.slots.len(), 1);
        assert!(ui.slots[0].corrupted);

        ui.selected_slot = Some(0);
        let mut rebuild = ViewRebuildRequest(false);
        load_selected_slot(
            &mut simulation,
            &mut rebuild,
            &mut StrategicNavigation::default(),
            &mut ui,
        );

        assert!(!rebuild.0);
        assert_eq!(ui.feedback, "Ce fichier de sauvegarde est corrompu.");
    }

    #[test]
    fn autosave_fills_empty_slots_before_overwriting_any_existing_one() {
        let _dir = TempSaveDir::new("autosave-fill");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();

        for expected_slot in 0..AUTOSAVE_SLOT_COUNT {
            assert_eq!(oldest_autosave_slot(), expected_slot);
            run_autosave(&source, &mut ui);
            assert!(autosave_path(expected_slot).exists());
        }
    }

    #[test]
    fn autosave_then_rotates_the_oldest_slot_once_all_three_are_filled() {
        let _dir = TempSaveDir::new("autosave-rotate");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();

        // `oldest_autosave_slot` compares `SaveFileHeader::saved_at_unix_seconds`,
        // not filesystem mtimes (see its doc comment) — fill all 3 slots with
        // explicit, strictly increasing fake timestamps directly, instead of
        // sleeping in real time between autosaves to let a real clock advance.
        let save = snapshot_from_simulation(source.simulation());
        for (slot, saved_at_unix_seconds) in (0..AUTOSAVE_SLOT_COUNT).zip([1_000, 2_000, 3_000]) {
            let header = SaveFileHeader {
                display_name: format!("Sauvegarde automatique {}", slot + 1),
                saved_at_unix_seconds,
            };
            save_to_path(&autosave_path(slot), header, &save).expect("save succeeds");
        }

        assert_eq!(
            oldest_autosave_slot(),
            0,
            "slot 0 has the smallest saved_at_unix_seconds, so it is oldest"
        );
        run_autosave(&source, &mut ui);
        assert_eq!(
            ui.feedback,
            "Sauvegarde automatique effectuée (emplacement 1)."
        );
    }

    #[test]
    fn quickload_without_a_prior_quicksave_reports_feedback_and_does_not_touch_the_simulation() {
        let _dir = TempSaveDir::new("quickload-empty");

        let mut simulation = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        let mut rebuild = ViewRebuildRequest(false);

        quickload(&mut simulation, &mut rebuild, &mut ui);

        assert!(!rebuild.0);
        assert_eq!(ui.feedback, "Aucune sauvegarde rapide disponible.");
    }

    #[test]
    fn quicksave_then_quickload_round_trips_to_the_reserved_slot() {
        let _dir = TempSaveDir::new("quicksave-roundtrip");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();

        quicksave(&source, &mut ui);
        assert_eq!(ui.feedback, "Sauvegarde rapide effectuée (F5).");
        assert!(quicksave_path().exists());

        let mut destination = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut rebuild = ViewRebuildRequest(false);

        quickload(&mut destination, &mut rebuild, &mut ui);

        assert_eq!(ui.feedback, "Sauvegarde rapide chargée (F9).");
        assert!(rebuild.0);
        assert_eq!(
            destination.simulation().state().colonies.len(),
            source.simulation().state().colonies.len(),
        );
    }

    #[test]
    fn format_save_timestamp_matches_known_unix_seconds() {
        assert_eq!(format_save_timestamp(0), "1970-01-01 00:00");
        // 2024-01-15T13:45:00Z
        assert_eq!(format_save_timestamp(1_705_326_300), "2024-01-15 13:45");
    }

    #[test]
    fn slugify_falls_back_to_a_stable_name_for_blank_input() {
        assert_eq!(slugify("  "), "sauvegarde");
        assert_eq!(slugify("Ma Sauvegarde !!"), "ma_sauvegarde");
    }

    #[test]
    fn a_typed_name_is_used_as_the_saved_display_name() {
        let _dir = TempSaveDir::new("named");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState {
            name_buffer: "  Avant l’assaut  ".to_string(),
            ..Default::default()
        };

        save_current_game(&source, &StrategicNavigation::default(), &mut ui);

        assert_eq!(ui.slots.len(), 1);
        assert_eq!(ui.slots[0].display_name, "Avant l’assaut");
    }

    #[test]
    fn save_name_editing_is_reported_through_the_shared_guard() {
        let mut ui = SaveLoadUiState::default();
        assert!(!save_name_is_editing(&ui));
        ui.name_editing = true;
        assert!(save_name_is_editing(&ui));
    }

    #[test]
    fn overwriting_a_slot_keeps_its_path_and_display_name() {
        let _dir = TempSaveDir::new("overwrite");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        save_current_game(&source, &StrategicNavigation::default(), &mut ui);
        let original_path = ui.slots[0].path.clone();
        let original_name = ui.slots[0].display_name.clone();
        ui.selected_slot = Some(0);

        overwrite_selected_slot(&source, &StrategicNavigation::default(), &mut ui);

        assert!(
            ui.feedback.starts_with("Sauvegarde écrasée"),
            "unexpected feedback: {}",
            ui.feedback
        );
        assert_eq!(ui.slots.len(), 1, "overwrite must not create a new file");
        assert_eq!(ui.slots[0].path, original_path);
        assert_eq!(ui.slots[0].display_name, original_name);
    }

    #[test]
    fn overwriting_without_a_selected_slot_reports_feedback_and_writes_nothing() {
        let _dir = TempSaveDir::new("overwrite-no-selection");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();

        overwrite_selected_slot(&source, &StrategicNavigation::default(), &mut ui);

        assert!(!ui.feedback.is_empty());
        assert!(ui.slots.is_empty());
    }

    #[test]
    fn deleting_a_slot_requires_a_second_press_within_the_confirmation_window() {
        let _dir = TempSaveDir::new("delete-two-press");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        save_current_game(&source, &StrategicNavigation::default(), &mut ui);
        let path = ui.slots[0].path.clone();
        ui.selected_slot = Some(0);

        // First press arms the confirmation — nothing is deleted yet.
        delete_selected_slot(&mut ui, Duration::from_secs(0));
        assert!(path.exists(), "the file must survive the first press");
        assert!(ui.delete_armed_until.is_some());

        // Second press, still inside the 3s window, actually deletes.
        delete_selected_slot(&mut ui, Duration::from_secs(1));
        assert!(!path.exists());
        assert!(ui.feedback.starts_with("Sauvegarde supprimée"));
        assert!(ui.delete_armed_until.is_none());
        assert!(ui.slots.is_empty());
    }

    #[test]
    fn deleting_expires_the_confirmation_window_and_requires_rearming() {
        let _dir = TempSaveDir::new("delete-expired");

        let source = SimulationResource {
            simulation: Simulation::new(UniverseConfig::mvp()),
            pending_events: Vec::new(),
        };
        let mut ui = SaveLoadUiState::default();
        save_current_game(&source, &StrategicNavigation::default(), &mut ui);
        let path = ui.slots[0].path.clone();
        ui.selected_slot = Some(0);

        delete_selected_slot(&mut ui, Duration::from_secs(0));
        // A press arriving after the 3s window re-arms instead of deleting.
        delete_selected_slot(&mut ui, Duration::from_secs(10));

        assert!(path.exists(), "an expired confirmation must not delete");
        assert!(ui.delete_armed_until.is_some());
    }

    #[test]
    fn deleting_without_a_selected_slot_reports_feedback_and_deletes_nothing() {
        let _dir = TempSaveDir::new("delete-no-selection");

        let mut ui = SaveLoadUiState::default();

        delete_selected_slot(&mut ui, Duration::from_secs(0));

        assert!(!ui.feedback.is_empty());
        assert!(ui.delete_armed_until.is_none());
    }
}

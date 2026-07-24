#!/usr/bin/env python3
"""
Applique MVP-015 au dépôt Galactic.

Baseline analysée :
    3ec466b3f4ff890a3ae2a20ec201cacda25c79be
    feat add construction update

Le script :
- remplace les overlays économie/construction par un écran dédié ;
- ajoute ouverture/fermeture avec C et un bouton ;
- permet de parcourir les colonies du joueur ;
- affiche ressources, niveaux, effets et file sans répétition ;
- présente une fiche détaillée du bâtiment sélectionné ;
- conserve les erreurs de construction visibles et non bloquantes ;
- compacte l’inspecteur stratégique ;
- ne modifie ni simulation ni sauvegarde.

Usage :
    python tools/apply_mvp_015.py --dry-run
    python tools/apply_mvp_015.py
    python tools/apply_mvp_015.py --skip-checks
    python tools/apply_mvp_015.py --root /chemin/vers/galactic
"""

from __future__ import annotations

import argparse
import difflib
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = (
    "3ec466b3f4ff890a3ae2a20ec201cacda25c79be"
)

MANAGEMENT_TYPES = '\n// MVP-015: dedicated colony management screen.\n#[derive(Resource)]\nstruct ColonyManagementState {\n    open: bool,\n    active_colony_id: Option<galactic_domain::ColonyId>,\n    selected_building: galactic_sim::BuildingKind,\n    feedback: String,\n}\n\nimpl Default for ColonyManagementState {\n    fn default() -> Self {\n        Self {\n            open: false,\n            active_colony_id: None,\n            selected_building: galactic_sim::BuildingKind::MetalMine,\n            feedback: String::new(),\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum ResourceHudKind {\n    Metal,\n    Crystal,\n    Fuel,\n    Energy,\n}\n\nimpl ResourceHudKind {\n    const ALL: [Self; 4] = [\n        Self::Metal,\n        Self::Crystal,\n        Self::Fuel,\n        Self::Energy,\n    ];\n\n    const fn title(self) -> &\'static str {\n        match self {\n            Self::Metal => "MÉTAL",\n            Self::Crystal => "CRISTAL",\n            Self::Fuel => "CARBURANT",\n            Self::Energy => "ÉNERGIE",\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum ResourceHudStatus {\n    Normal,\n    NearlyFull,\n    Full,\n    Deficit,\n}\n\n#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]\nenum ManagementButtonAction {\n    Toggle,\n    Close,\n    PreviousColony,\n    NextColony,\n    SelectBuilding(galactic_sim::BuildingKind),\n    UpgradeSelected,\n}\n\ntype ManagementButtonInteractionQuery<\'w, \'s> = Query<\n    \'w,\n    \'s,\n    (&\'static Interaction, &\'static ManagementButtonAction),\n    (Changed<Interaction>, With<Button>),\n>;\n\n#[derive(Component)]\nstruct ColonyManagementRoot;\n\n#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]\nenum ManagementTextRole {\n    ToggleLabel,\n    Title,\n    Colony,\n    Feedback,\n    BuildingDetail,\n    UpgradeLabel,\n    Queue,\n}\n\n#[derive(Component)]\nstruct ManagementResourceCardText {\n    kind: ResourceHudKind,\n}\n\n#[derive(Component)]\nstruct ManagementResourceGaugeFill {\n    kind: ResourceHudKind,\n}\n\n#[derive(Component)]\nstruct ManagementBuildingButton {\n    kind: galactic_sim::BuildingKind,\n}\n\n#[derive(Component)]\nstruct ManagementBuildingButtonText {\n    kind: galactic_sim::BuildingKind,\n}\n\n#[derive(Component)]\nstruct ManagementUpgradeButton;\n\n#[derive(Component)]\nstruct ManagementQueueProgressFill;\n'
MANAGEMENT_SPAWN = '\nfn spawn_colony_management_toggle(\n    parent: &mut ChildSpawnerCommands,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Percent(100.0),\n                min_height: Val::Px(36.0),\n                padding: UiRect::axes(\n                    Val::Px(10.0),\n                    Val::Px(7.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.08, 0.18, 0.19, 0.96,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.28, 0.78, 0.72, 0.55),\n            ),\n            ManagementButtonAction::Toggle,\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new("Gestion colonie"),\n                ui_text_font(12.0),\n                TextColor(Color::srgb(\n                    0.78, 0.94, 0.90,\n                )),\n                ManagementTextRole::ToggleLabel,\n            ));\n        });\n}\n\nfn spawn_colony_management_screen(\n    commands: &mut Commands,\n) {\n    commands\n        .spawn((\n            Node {\n                position_type: PositionType::Absolute,\n                left: Val::Px(14.0),\n                right: Val::Px(14.0),\n                top: Val::Px(72.0),\n                bottom: Val::Px(14.0),\n                padding: UiRect::all(Val::Px(12.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(8.0),\n                ),\n                flex_direction: FlexDirection::Column,\n                row_gap: Val::Px(9.0),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.008, 0.014, 0.020, 0.995,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.30, 0.72, 0.74, 0.68),\n            ),\n            Visibility::Hidden,\n            Interaction::None,\n            UiPointerBlocker,\n            ColonyManagementRoot,\n        ))\n        .with_children(|root| {\n            spawn_management_header(root);\n            spawn_management_resource_row(root);\n            spawn_management_main_row(root);\n            root.spawn((\n                Text::new(""),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.90, 0.72, 0.42,\n                )),\n                Node {\n                    min_height: Val::Px(18.0),\n                    ..default()\n                },\n                ManagementTextRole::Feedback,\n            ));\n        });\n}\n\nfn spawn_management_header(\n    root: &mut ChildSpawnerCommands,\n) {\n    root.spawn((\n        Node {\n            width: Val::Percent(100.0),\n            min_height: Val::Px(42.0),\n            flex_direction: FlexDirection::Row,\n            align_items: AlignItems::Center,\n            column_gap: Val::Px(8.0),\n            ..default()\n        },\n    ))\n    .with_children(|header| {\n        header.spawn((\n            Text::new("GESTION PLANÉTAIRE"),\n            ui_text_font(18.0),\n            TextColor(Color::srgb(\n                0.82, 0.96, 0.96,\n            )),\n            Node {\n                flex_grow: 1.0,\n                ..default()\n            },\n            ManagementTextRole::Title,\n        ));\n\n        spawn_management_small_button(\n            header,\n            "◀",\n            ManagementButtonAction::PreviousColony,\n            36.0,\n        );\n        header.spawn((\n            Text::new("Colonie"),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.76, 0.86, 0.90,\n            )),\n            Node {\n                width: Val::Px(250.0),\n                ..default()\n            },\n            ManagementTextRole::Colony,\n        ));\n        spawn_management_small_button(\n            header,\n            "▶",\n            ManagementButtonAction::NextColony,\n            36.0,\n        );\n        spawn_management_small_button(\n            header,\n            "Fermer  [C / Échap]",\n            ManagementButtonAction::Close,\n            154.0,\n        );\n    });\n}\n\nfn spawn_management_small_button(\n    parent: &mut ChildSpawnerCommands,\n    label: &str,\n    action: ManagementButtonAction,\n    width: f32,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Px(width),\n                min_height: Val::Px(32.0),\n                padding: UiRect::axes(\n                    Val::Px(8.0),\n                    Val::Px(5.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                justify_content: JustifyContent::Center,\n                align_items: AlignItems::Center,\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.06, 0.10, 0.13, 0.98,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.44, 0.62, 0.68, 0.50),\n            ),\n            action,\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new(label),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.82, 0.90, 0.94,\n                )),\n            ));\n        });\n}\n\nfn spawn_management_resource_row(\n    root: &mut ChildSpawnerCommands,\n) {\n    root.spawn((\n        Node {\n            width: Val::Percent(100.0),\n            min_height: Val::Px(94.0),\n            flex_direction: FlexDirection::Row,\n            column_gap: Val::Px(8.0),\n            ..default()\n        },\n    ))\n    .with_children(|row| {\n        for kind in ResourceHudKind::ALL {\n            spawn_management_resource_card(row, kind);\n        }\n    });\n}\n\nfn spawn_management_resource_card(\n    parent: &mut ChildSpawnerCommands,\n    kind: ResourceHudKind,\n) {\n    parent\n        .spawn((\n            Node {\n                flex_grow: 1.0,\n                flex_basis: Val::Px(0.0),\n                padding: UiRect::all(Val::Px(9.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(6.0),\n                ),\n                flex_direction: FlexDirection::Column,\n                justify_content: JustifyContent::SpaceBetween,\n                ..default()\n            },\n            BackgroundColor(resource_card_background(kind)),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                resource_outline_color(kind),\n            ),\n        ))\n        .with_children(|card| {\n            card.spawn((\n                Text::new(kind.title()),\n                ui_text_font(11.0),\n                TextColor(resource_kind_color(kind)),\n                ManagementResourceCardText { kind },\n            ));\n            card.spawn((\n                Node {\n                    width: Val::Percent(100.0),\n                    height: Val::Px(7.0),\n                    border_radius: BorderRadius::all(\n                        Val::Px(4.0),\n                    ),\n                    ..default()\n                },\n                BackgroundColor(Color::srgba(\n                    0.08, 0.11, 0.14, 0.96,\n                )),\n            ))\n            .with_children(|gauge| {\n                gauge.spawn((\n                    Node {\n                        width: Val::Percent(0.0),\n                        height: Val::Percent(100.0),\n                        border_radius: BorderRadius::all(\n                            Val::Px(4.0),\n                        ),\n                        ..default()\n                    },\n                    BackgroundColor(resource_kind_color(kind)),\n                    ManagementResourceGaugeFill { kind },\n                ));\n            });\n        });\n}\n\nfn spawn_management_main_row(\n    root: &mut ChildSpawnerCommands,\n) {\n    root.spawn((\n        Node {\n            width: Val::Percent(100.0),\n            flex_grow: 1.0,\n            min_height: Val::Px(390.0),\n            flex_direction: FlexDirection::Row,\n            column_gap: Val::Px(9.0),\n            ..default()\n        },\n    ))\n    .with_children(|row| {\n        spawn_management_building_list(row);\n        spawn_management_building_detail(row);\n        spawn_management_queue(row);\n    });\n}\n\nfn spawn_management_building_list(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            width: Val::Px(292.0),\n            padding: UiRect::all(Val::Px(9.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(5.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|list| {\n        list.spawn((\n            Text::new("BÂTIMENTS"),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.72, 0.88, 0.92,\n            )),\n        ));\n        for kind in galactic_sim::BuildingKind::ALL {\n            spawn_management_building_button(\n                list,\n                kind,\n            );\n        }\n    });\n}\n\nfn spawn_management_building_button(\n    parent: &mut ChildSpawnerCommands,\n    kind: galactic_sim::BuildingKind,\n) {\n    parent\n        .spawn((\n            Button,\n            Node {\n                width: Val::Percent(100.0),\n                min_height: Val::Px(37.0),\n                padding: UiRect::axes(\n                    Val::Px(8.0),\n                    Val::Px(6.0),\n                ),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(\n                    Val::Px(5.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.035, 0.055, 0.070, 0.96,\n            )),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.30, 0.44, 0.50, 0.42),\n            ),\n            ManagementButtonAction::SelectBuilding(kind),\n            ManagementBuildingButton { kind },\n            UiPointerBlocker,\n        ))\n        .with_children(|button| {\n            button.spawn((\n                Text::new(""),\n                ui_text_font(11.0),\n                TextColor(Color::srgb(\n                    0.82, 0.88, 0.90,\n                )),\n                ManagementBuildingButtonText { kind },\n            ));\n        });\n}\n\nfn spawn_management_building_detail(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            flex_grow: 1.0,\n            flex_basis: Val::Px(0.0),\n            padding: UiRect::all(Val::Px(12.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(10.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|detail| {\n        detail.spawn((\n            Text::new("Sélectionne un bâtiment."),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.84, 0.90, 0.94,\n            )),\n            Node {\n                flex_grow: 1.0,\n                ..default()\n            },\n            ManagementTextRole::BuildingDetail,\n        ));\n        detail\n            .spawn((\n                Button,\n                Node {\n                    width: Val::Percent(100.0),\n                    min_height: Val::Px(42.0),\n                    padding: UiRect::axes(\n                        Val::Px(12.0),\n                        Val::Px(8.0),\n                    ),\n                    border: UiRect::all(Val::Px(1.0)),\n                    border_radius: BorderRadius::all(\n                        Val::Px(6.0),\n                    ),\n                    justify_content: JustifyContent::Center,\n                    align_items: AlignItems::Center,\n                    ..default()\n                },\n                BackgroundColor(Color::srgba(\n                    0.08, 0.28, 0.24, 0.98,\n                )),\n                Outline::new(\n                    Val::Px(1.0),\n                    Val::ZERO,\n                    Color::srgba(\n                        0.30, 0.88, 0.72, 0.70,\n                    ),\n                ),\n                ManagementButtonAction::UpgradeSelected,\n                ManagementUpgradeButton,\n                UiPointerBlocker,\n            ))\n            .with_children(|button| {\n                button.spawn((\n                    Text::new("AMÉLIORER"),\n                    ui_text_font(12.0),\n                    TextColor(Color::srgb(\n                        0.86, 0.98, 0.92,\n                    )),\n                    ManagementTextRole::UpgradeLabel,\n                ));\n            });\n    });\n}\n\nfn spawn_management_queue(\n    row: &mut ChildSpawnerCommands,\n) {\n    row.spawn((\n        Node {\n            width: Val::Px(306.0),\n            padding: UiRect::all(Val::Px(10.0)),\n            border: UiRect::all(Val::Px(1.0)),\n            border_radius: BorderRadius::all(\n                Val::Px(6.0),\n            ),\n            flex_direction: FlexDirection::Column,\n            row_gap: Val::Px(8.0),\n            ..default()\n        },\n        BackgroundColor(panel_background()),\n        Outline::new(\n            Val::Px(1.0),\n            Val::ZERO,\n            panel_outline(),\n        ),\n    ))\n    .with_children(|queue| {\n        queue.spawn((\n            Text::new("FILE DE CONSTRUCTION"),\n            ui_text_font(12.0),\n            TextColor(Color::srgb(\n                0.72, 0.88, 0.92,\n            )),\n        ));\n        queue.spawn((\n            Node {\n                width: Val::Percent(100.0),\n                height: Val::Px(8.0),\n                border_radius: BorderRadius::all(\n                    Val::Px(4.0),\n                ),\n                ..default()\n            },\n            BackgroundColor(Color::srgba(\n                0.08, 0.11, 0.14, 0.96,\n            )),\n        ))\n        .with_children(|gauge| {\n            gauge.spawn((\n                Node {\n                    width: Val::Percent(0.0),\n                    height: Val::Percent(100.0),\n                    border_radius: BorderRadius::all(\n                        Val::Px(4.0),\n                    ),\n                    ..default()\n                },\n                BackgroundColor(Color::srgb(\n                    0.36, 0.84, 0.72,\n                )),\n                ManagementQueueProgressFill,\n            ));\n        });\n        queue.spawn((\n            Text::new("File vide."),\n            ui_text_font(11.0),\n            TextColor(Color::srgb(\n                0.78, 0.84, 0.88,\n            )),\n            ManagementTextRole::Queue,\n        ));\n    });\n}\n'
MANAGEMENT_SYSTEMS = '\nfn handle_colony_management_buttons(\n    mut simulation: ResMut<SimulationResource>,\n    mut management: ResMut<ColonyManagementState>,\n    interactions: ManagementButtonInteractionQuery,\n) {\n    for (interaction, action) in &interactions {\n        if *interaction != Interaction::Pressed {\n            continue;\n        }\n\n        match *action {\n            ManagementButtonAction::Toggle => {\n                toggle_colony_management(\n                    &mut management,\n                    &mut simulation,\n                );\n            }\n            ManagementButtonAction::Close => {\n                management.open = false;\n            }\n            ManagementButtonAction::PreviousColony => {\n                cycle_management_colony(\n                    &mut management,\n                    &mut simulation,\n                    true,\n                );\n            }\n            ManagementButtonAction::NextColony => {\n                cycle_management_colony(\n                    &mut management,\n                    &mut simulation,\n                    false,\n                );\n            }\n            ManagementButtonAction::SelectBuilding(kind) => {\n                management.selected_building = kind;\n                management.feedback.clear();\n            }\n            ManagementButtonAction::UpgradeSelected => {\n                queue_selected_management_upgrade(\n                    &mut management,\n                    &mut simulation,\n                );\n            }\n        }\n    }\n}\n\nfn capture_colony_management_feedback(\n    simulation: Res<SimulationResource>,\n    mut management: ResMut<ColonyManagementState>,\n) {\n    for event in &simulation.pending_events {\n        match *event {\n            GameEvent::ConstructionQueued(queued) => {\n                let name = &galactic_sim::default_building_catalog()\n                    .definition(queued.order.kind)\n                    .name;\n                management.feedback = format!(\n                    "{} niveau {} ajouté à la file.",\n                    name,\n                    queued.order.target_level,\n                );\n            }\n            GameEvent::ConstructionCompleted(completed) => {\n                let name = &galactic_sim::default_building_catalog()\n                    .definition(completed.kind)\n                    .name;\n                management.feedback = format!(\n                    "{} niveau {} terminé.",\n                    name,\n                    completed.new_level,\n                );\n            }\n            GameEvent::ConstructionRejected(rejected) => {\n                management.feedback = format!(\n                    "Amélioration refusée : {}",\n                    construction_error_text(rejected.error),\n                );\n            }\n            _ => {}\n        }\n    }\n}\n\nfn update_colony_management_visibility(\n    simulation: Res<SimulationResource>,\n    mut management: ResMut<ColonyManagementState>,\n    mut roots: Query<\n        &mut Visibility,\n        With<ColonyManagementRoot>,\n    >,\n    mut texts: Query<(\n        &ManagementTextRole,\n        &mut Text,\n    )>,\n) {\n    if management.open {\n        sync_management_colony(\n            &mut management,\n            simulation.simulation(),\n        );\n    }\n\n    for mut visibility in &mut roots {\n        *visibility = if management.open {\n            Visibility::Visible\n        } else {\n            Visibility::Hidden\n        };\n    }\n\n    let colony = active_management_colony(\n        &management,\n        simulation.simulation(),\n    );\n    let colonies = player_colony_ids(\n        simulation.simulation(),\n    );\n    let current_index = colony.and_then(|active| {\n        colonies\n            .iter()\n            .position(|candidate| *candidate == active.id)\n    });\n\n    for (role, mut text) in &mut texts {\n        match role {\n            ManagementTextRole::ToggleLabel => {\n                text.0 = if management.open {\n                    "Fermer gestion colonie".to_string()\n                } else {\n                    "Gestion colonie  [C]".to_string()\n                };\n            }\n            ManagementTextRole::Title => {\n                text.0 = colony\n                    .map(|active| {\n                        format!(\n                            "GESTION PLANÉTAIRE — {}",\n                            active.name,\n                        )\n                    })\n                    .unwrap_or_else(|| {\n                        "GESTION PLANÉTAIRE".to_string()\n                    });\n            }\n            ManagementTextRole::Colony => {\n                text.0 = colony\n                    .map(|active| {\n                        let index = current_index.unwrap_or(0) + 1;\n                        let planet = simulation\n                            .simulation()\n                            .universe_repository()\n                            .planet(active.planet_id)\n                            .map(|value| value.name.as_str())\n                            .unwrap_or("Planète");\n                        format!(\n                            "{} / {}  •  {}",\n                            index,\n                            colonies.len().max(1),\n                            planet,\n                        )\n                    })\n                    .unwrap_or_else(|| {\n                        "Aucune colonie".to_string()\n                    });\n            }\n            ManagementTextRole::Feedback => {\n                text.0 = management.feedback.clone();\n            }\n            ManagementTextRole::BuildingDetail\n            | ManagementTextRole::UpgradeLabel\n            | ManagementTextRole::Queue => {}\n        }\n    }\n}\n\nfn update_colony_management_resources(\n    simulation: Res<SimulationResource>,\n    management: Res<ColonyManagementState>,\n    mut texts: Query<(\n        &ManagementResourceCardText,\n        &mut Text,\n        &mut TextColor,\n    )>,\n    mut gauges: Query<(\n        &ManagementResourceGaugeFill,\n        &mut Node,\n        &mut BackgroundColor,\n    )>,\n) {\n    if !management.open {\n        return;\n    }\n    let Some(colony) = active_management_colony(\n        &management,\n        simulation.simulation(),\n    ) else {\n        return;\n    };\n    let production =\n        galactic_sim::colony_production_snapshot(colony);\n\n    for (card, mut text, mut color) in &mut texts {\n        let view =\n            resource_hud_view(card.kind, colony, production);\n        text.0 = view.text;\n        color.0 =\n            status_text_color(card.kind, view.status);\n    }\n    for (gauge, mut node, mut color) in &mut gauges {\n        let view =\n            resource_hud_view(gauge.kind, colony, production);\n        node.width = Val::Percent(\n            (view.fill_ratio * 100.0)\n                .clamp(0.0, 100.0),\n        );\n        color.0 =\n            status_gauge_color(gauge.kind, view.status);\n    }\n}\n\nfn update_colony_management_buildings(\n    simulation: Res<SimulationResource>,\n    management: Res<ColonyManagementState>,\n    mut buttons: Query<(\n        &ManagementBuildingButton,\n        &Interaction,\n        &mut BackgroundColor,\n        &mut Outline,\n    )>,\n    mut labels: Query<(\n        &ManagementBuildingButtonText,\n        &mut Text,\n        &mut TextColor,\n    )>,\n) {\n    if !management.open {\n        return;\n    }\n    let Some(colony) = active_management_colony(\n        &management,\n        simulation.simulation(),\n    ) else {\n        return;\n    };\n    let catalog =\n        galactic_sim::default_building_catalog();\n    let projected =\n        galactic_sim::projected_building_levels(colony);\n\n    for (button, interaction, mut background, mut outline)\n        in &mut buttons\n    {\n        let selected =\n            button.kind == management.selected_building;\n        background.0 = management_building_button_color(\n            selected,\n            interaction,\n        );\n        outline.color =\n            management_building_button_outline(selected);\n    }\n\n    for (label, mut text, mut color) in &mut labels {\n        let definition = catalog.definition(label.kind);\n        let active_level = colony.buildings.level(label.kind);\n        let projected_level = projected.level(label.kind);\n        let queue_suffix = if projected_level > active_level {\n            format!("  → {} en file", projected_level)\n        } else {\n            String::new()\n        };\n        text.0 = format!(\n            "{}\\nNiveau {}{}",\n            definition.name,\n            active_level,\n            queue_suffix,\n        );\n        color.0 =\n            if label.kind == management.selected_building {\n                Color::srgb(0.86, 0.98, 0.94)\n            } else {\n                Color::srgb(0.78, 0.84, 0.88)\n            };\n    }\n}\n\nfn update_colony_management_detail(\n    simulation: Res<SimulationResource>,\n    management: Res<ColonyManagementState>,\n    mut texts: Query<(\n        &ManagementTextRole,\n        &mut Text,\n        &mut TextColor,\n    )>,\n    mut buttons: Query<\n        (\n            &Interaction,\n            &mut BackgroundColor,\n            &mut Outline,\n        ),\n        With<ManagementUpgradeButton>,\n    >,\n) {\n    if !management.open {\n        return;\n    }\n    let Some(colony) = active_management_colony(\n        &management,\n        simulation.simulation(),\n    ) else {\n        return;\n    };\n\n    let kind = management.selected_building;\n    let quote = galactic_sim::building_upgrade_quote(\n        simulation.simulation().state(),\n        colony.id,\n        kind,\n    );\n    let available = quote.is_ok();\n    let detail = building_management_detail_text(\n        colony,\n        kind,\n        quote,\n    );\n    let upgrade_label = match quote {\n        Ok(value) => format!(\n            "AMÉLIORER VERS LE NIVEAU {}",\n            value.target_level,\n        ),\n        Err(error) => construction_error_text(error),\n    };\n\n    for (role, mut text, mut color) in &mut texts {\n        match role {\n            ManagementTextRole::BuildingDetail => {\n                text.0 = detail.clone();\n                color.0 =\n                    Color::srgb(0.84, 0.90, 0.94);\n            }\n            ManagementTextRole::UpgradeLabel => {\n                text.0 = upgrade_label.clone();\n                color.0 = if available {\n                    Color::srgb(0.86, 0.98, 0.92)\n                } else {\n                    Color::srgb(0.64, 0.66, 0.66)\n                };\n            }\n            _ => {}\n        }\n    }\n\n    for (interaction, mut background, mut outline)\n        in &mut buttons\n    {\n        background.0 = action_button_color(\n            available,\n            false,\n            interaction,\n        );\n        outline.color = action_button_outline(\n            available,\n            false,\n            interaction,\n        );\n    }\n}\n\nfn update_colony_management_queue(\n    simulation: Res<SimulationResource>,\n    management: Res<ColonyManagementState>,\n    mut texts: Query<(\n        &ManagementTextRole,\n        &mut Text,\n    )>,\n    mut progress: Query<\n        &mut Node,\n        With<ManagementQueueProgressFill>,\n    >,\n) {\n    if !management.open {\n        return;\n    }\n    let Some(colony) = active_management_colony(\n        &management,\n        simulation.simulation(),\n    ) else {\n        return;\n    };\n\n    let label = construction_queue_detail_label(colony);\n    for (role, mut text) in &mut texts {\n        if *role == ManagementTextRole::Queue {\n            text.0 = label.clone();\n        }\n    }\n\n    let ratio = construction_progress_ratio(colony);\n    for mut node in &mut progress {\n        node.width = Val::Percent(\n            (ratio * 100.0).clamp(0.0, 100.0),\n        );\n    }\n}\n\nfn toggle_colony_management(\n    management: &mut ColonyManagementState,\n    simulation: &mut SimulationResource,\n) {\n    management.open = !management.open;\n    management.feedback.clear();\n    if management.open {\n        sync_management_colony(\n            management,\n            simulation.simulation(),\n        );\n        select_active_management_colony(\n            management,\n            simulation,\n        );\n    }\n}\n\nfn sync_management_colony(\n    management: &mut ColonyManagementState,\n    simulation: &Simulation,\n) {\n    let state = simulation.state();\n    let active_is_valid =\n        management.active_colony_id.is_some_and(|colony_id| {\n            state.colony(colony_id).is_some_and(|colony| {\n                colony.faction == state.player_faction\n            })\n        });\n    if active_is_valid {\n        return;\n    }\n\n    management.active_colony_id =\n        selected_player_colony_id(simulation)\n            .or_else(|| {\n                state\n                    .colonies\n                    .iter()\n                    .find(|colony| {\n                        colony.faction == state.player_faction\n                    })\n                    .map(|colony| colony.id)\n            });\n}\n\nfn selected_player_colony_id(\n    simulation: &Simulation,\n) -> Option<galactic_domain::ColonyId> {\n    let state = simulation.state();\n    let colony = match state.selected {\n        SelectionTarget::Planet { planet_id, .. } => {\n            state.colony_on_planet(planet_id)\n        }\n        SelectionTarget::System(system_id) => {\n            state.colonies.iter().find(|colony| {\n                colony.system_id == system_id\n                    && colony.faction\n                        == state.player_faction\n            })\n        }\n        SelectionTarget::None => {\n            state.player_home_colony()\n        }\n    }?;\n    (colony.faction == state.player_faction)\n        .then_some(colony.id)\n}\n\nfn player_colony_ids(\n    simulation: &Simulation,\n) -> Vec<galactic_domain::ColonyId> {\n    let state = simulation.state();\n    state\n        .colonies\n        .iter()\n        .filter(|colony| {\n            colony.faction == state.player_faction\n        })\n        .map(|colony| colony.id)\n        .collect()\n}\n\nfn active_management_colony<\'a>(\n    management: &ColonyManagementState,\n    simulation: &\'a Simulation,\n) -> Option<&\'a galactic_sim::ColonyState> {\n    let colony_id = management.active_colony_id?;\n    let colony = simulation.state().colony(colony_id)?;\n    (colony.faction\n        == simulation.state().player_faction)\n        .then_some(colony)\n}\n\nfn cycle_management_colony(\n    management: &mut ColonyManagementState,\n    simulation: &mut SimulationResource,\n    reverse: bool,\n) {\n    let colonies =\n        player_colony_ids(simulation.simulation());\n    if colonies.is_empty() {\n        management.active_colony_id = None;\n        return;\n    }\n\n    let current = management\n        .active_colony_id\n        .and_then(|active| {\n            colonies.iter().position(|id| *id == active)\n        })\n        .unwrap_or(0);\n    let next = if reverse {\n        current\n            .checked_sub(1)\n            .unwrap_or(colonies.len() - 1)\n    } else {\n        (current + 1) % colonies.len()\n    };\n    management.active_colony_id = Some(colonies[next]);\n    management.feedback.clear();\n    select_active_management_colony(\n        management,\n        simulation,\n    );\n}\n\nfn select_active_management_colony(\n    management: &ColonyManagementState,\n    simulation: &mut SimulationResource,\n) {\n    let Some(colony) = active_management_colony(\n        management,\n        simulation.simulation(),\n    ) else {\n        return;\n    };\n    let system_id = colony.system_id;\n    let planet_id = colony.planet_id;\n    apply_simulation_command(\n        simulation,\n        GameCommand::SelectPlanet {\n            system_id,\n            planet_id,\n        },\n    );\n}\n\nfn queue_selected_management_upgrade(\n    management: &mut ColonyManagementState,\n    simulation: &mut SimulationResource,\n) {\n    let Some(colony_id) = management.active_colony_id\n    else {\n        management.feedback =\n            "Aucune colonie active.".to_string();\n        return;\n    };\n    let kind = management.selected_building;\n    match galactic_sim::building_upgrade_quote(\n        simulation.simulation().state(),\n        colony_id,\n        kind,\n    ) {\n        Ok(_) => {\n            apply_simulation_command(\n                simulation,\n                GameCommand::QueueBuildingUpgrade {\n                    colony_id,\n                    kind,\n                },\n            );\n        }\n        Err(error) => {\n            management.feedback =\n                construction_error_text(error);\n        }\n    }\n}\n\nstruct ResourceHudView {\n    text: String,\n    fill_ratio: f32,\n    status: ResourceHudStatus,\n}\n\nfn resource_hud_view(\n    kind: ResourceHudKind,\n    colony: &galactic_sim::ColonyState,\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> ResourceHudView {\n    if kind == ResourceHudKind::Energy {\n        return energy_hud_view(production);\n    }\n\n    let stock =\n        resource_value(kind, colony.resources.stock());\n    let available = resource_value(\n        kind,\n        colony.resources.available(),\n    );\n    let reserved = resource_value(\n        kind,\n        colony.resources.reserved_total(),\n    );\n    let capacity =\n        resource_value(kind, production.capacity);\n    let rate =\n        resource_rate_per_second(kind, production);\n    let saturation =\n        resource_saturation(kind, production);\n    let fill_ratio =\n        resource_fill_ratio(stock, capacity);\n    let status =\n        resource_hud_status(stock, capacity);\n    let warning = if status == ResourceHudStatus::Full {\n        "\\nPLEIN — PRODUCTION BLOQUÉE"\n    } else {\n        ""\n    };\n\n    ResourceHudView {\n        text: format!(\n            "{}  {} / {}\\nDisponible {}  •  Réservé {}\\nProduction +{:.2}/s  •  plein {}{}",\n            kind.title(),\n            stock,\n            capacity,\n            available,\n            reserved,\n            rate,\n            format_saturation_time(saturation),\n            warning,\n        ),\n        fill_ratio,\n        status,\n    }\n}\n\nfn energy_hud_view(\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> ResourceHudView {\n    let produced =\n        production.effective_energy_production;\n    let consumed = production.energy_consumption;\n    let available = produced.saturating_sub(consumed);\n    let deficit = consumed > produced;\n    let status = if deficit {\n        ResourceHudStatus::Deficit\n    } else {\n        ResourceHudStatus::Normal\n    };\n    let warning = if deficit {\n        "\\nDÉFICIT — PRODUCTION RALENTIE"\n    } else {\n        ""\n    };\n\n    ResourceHudView {\n        text: format!(\n            "ÉNERGIE  {} / {}\\nDisponible {}  •  Bilan {:+}\\nRendement {}%{}",\n            consumed,\n            produced,\n            available,\n            i128::from(produced)\n                - i128::from(consumed),\n            u32::from(\n                production.energy_efficiency_per_mille,\n            ) / 10,\n            warning,\n        ),\n        fill_ratio:\n            energy_fill_ratio(consumed, produced),\n        status,\n    }\n}\n\nfn resource_value(\n    kind: ResourceHudKind,\n    stock: ResourceStock,\n) -> u64 {\n    match kind {\n        ResourceHudKind::Metal => stock.metal,\n        ResourceHudKind::Crystal => stock.crystal,\n        ResourceHudKind::Fuel => stock.fuel,\n        ResourceHudKind::Energy => 0,\n    }\n}\n\nfn resource_rate_per_second(\n    kind: ResourceHudKind,\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> f64 {\n    match kind {\n        ResourceHudKind::Metal => {\n            production.effective_rate.metal_per_second()\n        }\n        ResourceHudKind::Crystal => {\n            production.effective_rate.crystal_per_second()\n        }\n        ResourceHudKind::Fuel => {\n            production.effective_rate.fuel_per_second()\n        }\n        ResourceHudKind::Energy => 0.0,\n    }\n}\n\nfn resource_saturation(\n    kind: ResourceHudKind,\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> galactic_sim::SaturationTime {\n    match kind {\n        ResourceHudKind::Metal => {\n            production.saturation.metal\n        }\n        ResourceHudKind::Crystal => {\n            production.saturation.crystal\n        }\n        ResourceHudKind::Fuel => {\n            production.saturation.fuel\n        }\n        ResourceHudKind::Energy => {\n            galactic_sim::SaturationTime::Never\n        }\n    }\n}\n\nfn resource_fill_ratio(\n    stock: u64,\n    capacity: u64,\n) -> f32 {\n    if capacity == 0 {\n        return if stock == 0 { 0.0 } else { 1.0 };\n    }\n    (stock as f64 / capacity as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\nfn energy_fill_ratio(\n    consumption: u64,\n    production: u64,\n) -> f32 {\n    if production == 0 {\n        return if consumption == 0 {\n            0.0\n        } else {\n            1.0\n        };\n    }\n    (consumption as f64 / production as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\nfn resource_hud_status(\n    stock: u64,\n    capacity: u64,\n) -> ResourceHudStatus {\n    if capacity > 0 && stock >= capacity {\n        ResourceHudStatus::Full\n    } else if capacity > 0\n        && stock.saturating_mul(100)\n            >= capacity.saturating_mul(90)\n    {\n        ResourceHudStatus::NearlyFull\n    } else {\n        ResourceHudStatus::Normal\n    }\n}\n\nfn resource_kind_color(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgb(0.90, 0.66, 0.42)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgb(0.56, 0.82, 0.96)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgb(0.86, 0.82, 0.38)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgb(0.52, 0.92, 0.62)\n        }\n    }\n}\n\nfn resource_outline_color(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgba(0.90, 0.66, 0.42, 0.42)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgba(0.56, 0.82, 0.96, 0.42)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgba(0.86, 0.82, 0.38, 0.42)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgba(0.52, 0.92, 0.62, 0.42)\n        }\n    }\n}\n\nfn resource_card_background(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgba(0.16, 0.10, 0.06, 0.94)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgba(0.06, 0.11, 0.16, 0.94)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgba(0.15, 0.14, 0.05, 0.94)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgba(0.05, 0.14, 0.08, 0.94)\n        }\n    }\n}\n\nfn status_text_color(\n    kind: ResourceHudKind,\n    status: ResourceHudStatus,\n) -> Color {\n    match status {\n        ResourceHudStatus::Full\n        | ResourceHudStatus::Deficit => {\n            Color::srgb(1.0, 0.48, 0.42)\n        }\n        ResourceHudStatus::NearlyFull => {\n            Color::srgb(1.0, 0.78, 0.36)\n        }\n        ResourceHudStatus::Normal => {\n            resource_kind_color(kind)\n        }\n    }\n}\n\nfn status_gauge_color(\n    kind: ResourceHudKind,\n    status: ResourceHudStatus,\n) -> Color {\n    status_text_color(kind, status)\n}\n\nfn management_building_button_color(\n    selected: bool,\n    interaction: &Interaction,\n) -> Color {\n    if selected {\n        return Color::srgba(0.08, 0.30, 0.26, 0.98);\n    }\n    match interaction {\n        Interaction::Pressed => {\n            Color::srgba(0.09, 0.24, 0.24, 0.98)\n        }\n        Interaction::Hovered => {\n            Color::srgba(0.07, 0.16, 0.18, 0.98)\n        }\n        Interaction::None => {\n            Color::srgba(0.035, 0.055, 0.070, 0.96)\n        }\n    }\n}\n\nfn management_building_button_outline(\n    selected: bool,\n) -> Color {\n    if selected {\n        Color::srgba(0.32, 0.92, 0.76, 0.80)\n    } else {\n        Color::srgba(0.30, 0.44, 0.50, 0.42)\n    }\n}\n\nfn building_management_detail_text(\n    colony: &galactic_sim::ColonyState,\n    kind: galactic_sim::BuildingKind,\n    quote: Result<\n        galactic_sim::BuildingUpgradeQuote,\n        galactic_sim::ConstructionError,\n    >,\n) -> String {\n    let catalog =\n        galactic_sim::default_building_catalog();\n    let definition = catalog.definition(kind);\n    let actual_level = colony.buildings.level(kind);\n    let projected_levels =\n        galactic_sim::projected_building_levels(colony);\n    let projected_level = projected_levels.level(kind);\n    let current_effect =\n        building_effect_label(colony, kind, colony.buildings);\n    let projected_effect = building_effect_label(\n        colony,\n        kind,\n        projected_levels,\n    );\n\n    let mut lines = vec![\n        definition.name.to_uppercase(),\n        format!(\n            "Niveau actif : {}  •  après la file : {}  •  maximum : {}",\n            actual_level,\n            projected_level,\n            definition.max_level,\n        ),\n        String::new(),\n        "EFFET".to_string(),\n        format!("Actuel : {current_effect}"),\n    ];\n    if projected_level != actual_level {\n        lines.push(format!(\n            "Après la file : {projected_effect}",\n        ));\n    }\n\n    if projected_level >= definition.max_level {\n        lines.push(String::new());\n        lines.push(\n            "Niveau maximal atteint ou déjà planifié."\n                .to_string(),\n        );\n        return lines.join("\\n");\n    }\n\n    let target_level = projected_level + 1;\n    let mut next_levels = projected_levels;\n    next_levels.set_level(kind, target_level);\n    let next_effect =\n        building_effect_label(colony, kind, next_levels);\n    let cost = definition\n        .cost_for_level(target_level)\n        .expect("catalog target level is valid");\n    let duration = definition\n        .duration_for_level(target_level)\n        .expect("catalog target level is valid");\n\n    lines.extend([\n        format!("Prochain niveau : {next_effect}"),\n        String::new(),\n        format!(\n            "AMÉLIORATION VERS LE NIVEAU {}",\n            target_level,\n        ),\n        format!(\n            "Coût : {}",\n            construction_cost_label(cost),\n        ),\n        format!(\n            "Durée catalogue : {}",\n            format_strategic_duration(\n                galactic_sim::StrategicDuration::from_ticks(\n                    duration,\n                ),\n            ),\n        ),\n    ]);\n\n    match quote {\n        Ok(value) => {\n            lines.push(format!(\n                "Durée effective : {}",\n                format_strategic_duration(\n                    galactic_sim::StrategicDuration::from_ticks(\n                        value.duration_ticks,\n                    ),\n                ),\n            ));\n            lines.push(format!(\n                "Énergie projetée : {} produite / {} consommée",\n                value.projected_energy_production,\n                value.projected_energy_consumption,\n            ));\n            lines.push(String::new());\n            lines.push(\n                "Prêt à être ajouté à la file."\n                    .to_string(),\n            );\n        }\n        Err(error) => {\n            lines.push(String::new());\n            lines.push(format!(\n                "BLOCAGE : {}",\n                construction_error_text(error),\n            ));\n        }\n    }\n\n    lines.join("\\n")\n}\n\nfn building_effect_label(\n    colony: &galactic_sim::ColonyState,\n    kind: galactic_sim::BuildingKind,\n    levels: galactic_sim::BuildingLevels,\n) -> String {\n    let mut preview = colony.clone();\n    preview.buildings = levels;\n    preview.energy =\n        galactic_sim::default_building_catalog()\n            .energy_grid_for_levels(levels);\n    let production =\n        galactic_sim::colony_production_snapshot(&preview);\n\n    match kind {\n        galactic_sim::BuildingKind::MetalMine => {\n            format!(\n                "+{:.2} métal/s",\n                production\n                    .effective_rate\n                    .metal_per_second(),\n            )\n        }\n        galactic_sim::BuildingKind::CrystalExtractor => {\n            format!(\n                "+{:.2} cristal/s",\n                production\n                    .effective_rate\n                    .crystal_per_second(),\n            )\n        }\n        galactic_sim::BuildingKind::FuelRefinery => {\n            format!(\n                "+{:.2} carburant/s",\n                production\n                    .effective_rate\n                    .fuel_per_second(),\n            )\n        }\n        galactic_sim::BuildingKind::PowerPlant => {\n            format!(\n                "{} énergie effective",\n                production.effective_energy_production,\n            )\n        }\n        galactic_sim::BuildingKind::Warehouse => {\n            format!(\n                "capacité {}/{}/{}",\n                production.capacity.metal,\n                production.capacity.crystal,\n                production.capacity.fuel,\n            )\n        }\n        galactic_sim::BuildingKind::ConstructionCenter => {\n            let level = u64::from(\n                levels.level(kind),\n            );\n            let bonus = match galactic_sim::default_building_catalog()\n                .definition(kind)\n                .effect\n            {\n                galactic_sim::BuildingEffect::ConstructionSpeed {\n                    permille_per_level,\n                } => {\n                    permille_per_level\n                        .saturating_mul(level)\n                        / 10\n                }\n                _ => 0,\n            };\n            format!("vitesse de construction +{bonus}%")\n        }\n        galactic_sim::BuildingKind::ResearchLab => {\n            future_points_effect_label(\n                kind,\n                levels.level(kind),\n                "recherche",\n            )\n        }\n        galactic_sim::BuildingKind::Shipyard => {\n            future_points_effect_label(\n                kind,\n                levels.level(kind),\n                "chantier",\n            )\n        }\n    }\n}\n\nfn future_points_effect_label(\n    kind: galactic_sim::BuildingKind,\n    level: u8,\n    label: &str,\n) -> String {\n    let definition =\n        galactic_sim::default_building_catalog()\n            .definition(kind);\n    let milli_per_tick = match definition.effect {\n        galactic_sim::BuildingEffect::ResearchPoints {\n            milli_per_tick_per_level,\n        }\n        | galactic_sim::BuildingEffect::ShipyardPoints {\n            milli_per_tick_per_level,\n        } => milli_per_tick_per_level,\n        _ => 0,\n    };\n    let per_second = milli_per_tick as f64\n        * f64::from(level)\n        * f64::from(\n            galactic_sim::STRATEGIC_TICKS_PER_SECOND,\n        )\n        / 1_000.0;\n    format!("{per_second:.2} points de {label}/s")\n}\n\nfn construction_queue_detail_label(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    if colony.construction_queue.is_empty() {\n        return format!(\n            "File vide\\n\\n{} emplacement(s) disponible(s).",\n            galactic_sim::MAX_CONSTRUCTION_QUEUE,\n        );\n    }\n\n    let catalog =\n        galactic_sim::default_building_catalog();\n    let mut lines = Vec::new();\n    for (index, order) in\n        colony.construction_queue.orders().enumerate()\n    {\n        let definition = catalog.definition(order.kind);\n        if index == 0 {\n            lines.push(format!(\n                "EN COURS\\n{}. {} — niveau {}\\n{} restant • coût réservé {}",\n                index + 1,\n                definition.name,\n                order.target_level,\n                format_strategic_duration(\n                    galactic_sim::StrategicDuration::from_ticks(\n                        order.remaining_ticks,\n                    ),\n                ),\n                construction_cost_label(order.cost),\n            ));\n        } else {\n            lines.push(format!(\n                "\\nEN ATTENTE\\n{}. {} — niveau {}\\ncoût réservé {}",\n                index + 1,\n                definition.name,\n                order.target_level,\n                construction_cost_label(order.cost),\n            ));\n        }\n    }\n\n    lines.push(format!(\n        "\\n\\n{} / {} emplacement(s) utilisé(s)",\n        colony.construction_queue.len(),\n        galactic_sim::MAX_CONSTRUCTION_QUEUE,\n    ));\n    lines.join("\\n")\n}\n\nfn construction_progress_ratio(\n    colony: &galactic_sim::ColonyState,\n) -> f32 {\n    let Some(active) = colony.construction_queue.active()\n    else {\n        return 0.0;\n    };\n    if active.total_ticks == 0 {\n        return 1.0;\n    }\n    let completed =\n        active.total_ticks.saturating_sub(\n            active.remaining_ticks,\n        );\n    (completed as f64 / active.total_ticks as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\nfn construction_error_text(\n    error: galactic_sim::ConstructionError,\n) -> String {\n    match error {\n        galactic_sim::ConstructionError::UnknownColony(_)\n        | galactic_sim::ConstructionError::NotPlayerOwned(_) => {\n            "Colonie indisponible".to_string()\n        }\n        galactic_sim::ConstructionError::QueueFull {\n            maximum,\n        } => {\n            format!("File pleine ({maximum})")\n        }\n        galactic_sim::ConstructionError::MaximumLevel {\n            ..\n        } => "Niveau maximal".to_string(),\n        galactic_sim::ConstructionError::InsufficientResources {\n            available,\n            cost,\n        } => format!(\n            "Manque: {}",\n            construction_missing_resources_label(\n                available,\n                cost,\n            ),\n        ),\n        galactic_sim::ConstructionError::EnergyDeficit {\n            production,\n            consumption,\n        } => format!(\n            "Énergie insuffisante : {production}/{consumption}",\n        ),\n        galactic_sim::ConstructionError::Catalog(\n            galactic_sim::BuildingCatalogError::\n                UnsatisfiedPrerequisite {\n                    prerequisite,\n                    required,\n                    ..\n                },\n        ) => {\n            let name =\n                &galactic_sim::default_building_catalog()\n                    .definition(prerequisite)\n                    .name;\n            format!("Requiert {name} niveau {required}")\n        }\n        galactic_sim::ConstructionError::Catalog(_) => {\n            "Règle catalogue invalide".to_string()\n        }\n        galactic_sim::ConstructionError::Reservation(_) => {\n            "Réservation impossible".to_string()\n        }\n    }\n}\n\nfn construction_cost_label(\n    cost: galactic_domain::ResourceCost,\n) -> String {\n    construction_resource_amounts_label(\n        cost.as_stock(),\n        "gratuit",\n    )\n}\n\nfn construction_missing_resources_label(\n    available: ResourceStock,\n    cost: galactic_domain::ResourceCost,\n) -> String {\n    construction_resource_amounts_label(\n        cost.as_stock().saturating_sub(available),\n        "0",\n    )\n}\n\nfn construction_resource_amounts_label(\n    resources: ResourceStock,\n    empty_label: &str,\n) -> String {\n    let mut parts = Vec::new();\n    append_resource_amount(\n        &mut parts,\n        resources.metal,\n        "métal",\n    );\n    append_resource_amount(\n        &mut parts,\n        resources.crystal,\n        "cristal",\n    );\n    append_resource_amount(\n        &mut parts,\n        resources.fuel,\n        "carburant",\n    );\n\n    if parts.is_empty() {\n        empty_label.to_string()\n    } else {\n        parts.join(", ")\n    }\n}\n\nfn append_resource_amount(\n    parts: &mut Vec<String>,\n    amount: u64,\n    label: &str,\n) {\n    if amount > 0 {\n        parts.push(format!("{amount} {label}"));\n    }\n}\n'
COMPACT_ECONOMY = 'fn colony_economy_text(\n    colony: &galactic_sim::ColonyState,\n) -> String {\n    let available = colony.resources.available();\n    let production =\n        galactic_sim::colony_production_snapshot(colony);\n    let construction = colony\n        .construction_queue\n        .active()\n        .map(|order| {\n            let name =\n                &galactic_sim::default_building_catalog()\n                    .definition(order.kind)\n                    .name;\n            format!(\n                "{} niveau {} — {}",\n                name,\n                order.target_level,\n                format_strategic_duration(\n                    galactic_sim::StrategicDuration::from_ticks(\n                        order.remaining_ticks,\n                    ),\n                ),\n            )\n        })\n        .unwrap_or_else(|| "aucune".to_string());\n\n    format!(\n        "ÉCONOMIE — RÉSUMÉ\\nDisponible : {} métal, {} cristal, {} carburant\\nProduction : +{:.2} / +{:.2} / +{:.2} par seconde\\nÉnergie : {} produite, {} consommée\\nConstruction : {}\\n\\nGestion complète : touche C",\n        available.metal,\n        available.crystal,\n        available.fuel,\n        production.effective_rate.metal_per_second(),\n        production.effective_rate.crystal_per_second(),\n        production.effective_rate.fuel_per_second(),\n        production.effective_energy_production,\n        production.energy_consumption,\n        construction,\n    )\n}\n'
TESTS_CODE = '\n    #[test]\n    fn management_defaults_to_selected_player_colony() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let mut management =\n            ColonyManagementState::default();\n\n        sync_management_colony(\n            &mut management,\n            &simulation,\n        );\n\n        assert_eq!(\n            management.active_colony_id,\n            simulation\n                .state()\n                .player_home_colony()\n                .map(|colony| colony.id),\n        );\n    }\n\n    #[test]\n    fn management_colony_cycle_wraps() {\n        let mut simulation = SimulationResource {\n            simulation:\n                Simulation::new(UniverseConfig::mvp()),\n            pending_events: Vec::new(),\n        };\n        let mut management =\n            ColonyManagementState::default();\n        sync_management_colony(\n            &mut management,\n            simulation.simulation(),\n        );\n        let initial = management.active_colony_id;\n\n        cycle_management_colony(\n            &mut management,\n            &mut simulation,\n            false,\n        );\n\n        assert_eq!(\n            management.active_colony_id,\n            initial,\n        );\n    }\n\n    #[test]\n    fn building_detail_uses_catalog_and_simulation_values() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists");\n        let quote = galactic_sim::building_upgrade_quote(\n            simulation.state(),\n            colony.id,\n            galactic_sim::BuildingKind::MetalMine,\n        );\n\n        let text = building_management_detail_text(\n            colony,\n            galactic_sim::BuildingKind::MetalMine,\n            quote,\n        );\n\n        assert!(text.contains("MINE DE MÉTAL"));\n        assert!(text.contains("Niveau actif"));\n        assert!(text.contains("Coût"));\n        assert!(text.contains("Actuel"));\n    }\n\n    #[test]\n    fn queue_progress_is_clamped_and_empty_is_zero() {\n        let mut simulation =\n            Simulation::new(UniverseConfig::mvp());\n        let colony_id = simulation\n            .state()\n            .player_home_colony()\n            .expect("home colony exists")\n            .id;\n        assert_eq!(\n            construction_progress_ratio(\n                simulation\n                    .state()\n                    .colony(colony_id)\n                    .expect("colony exists"),\n            ),\n            0.0,\n        );\n\n        simulation.apply_command(\n            GameCommand::QueueBuildingUpgrade {\n                colony_id,\n                kind:\n                    galactic_sim::BuildingKind::MetalMine,\n            },\n        );\n        let ratio = construction_progress_ratio(\n            simulation\n                .state()\n                .colony(colony_id)\n                .expect("colony exists"),\n        );\n        assert!((0.0..=1.0).contains(&ratio));\n    }\n'
DOC_APPEND = '\n## MVP-015 — Écran de gestion planétaire\n\nLa gestion économique n’est plus affichée sous forme de panneaux superposés à\nla vue stratégique.\n\nUn écran dédié s’ouvre avec la touche `C` ou le bouton `Gestion colonie`. Il\noccupe l’espace central entre la barre supérieure et les bords de la fenêtre.\nLa vue 3D reste en arrière-plan mais la caméra et le picking sont neutralisés\npendant la gestion.\n\nL’écran est organisé en quatre niveaux de lecture :\n\n1. en-tête de la colonie active ;\n2. bandeau Métal, Cristal, Carburant et Énergie ;\n3. liste compacte des bâtiments ;\n4. détail du bâtiment sélectionné et file de construction.\n\n### Sélection de la colonie\n\nLes boutons précédent et suivant parcourent uniquement les colonies du joueur.\nChanger de colonie met aussi à jour la sélection stratégique vers sa planète.\nL’architecture fonctionne déjà avec une seule colonie et prépare MVP-028.\n\n### Bâtiments\n\nLa liste montre uniquement le nom, le niveau actif et le niveau déjà planifié.\nSélectionner un bâtiment ouvre sa fiche détaillée :\n\n- niveau actif ;\n- niveau après la file ;\n- niveau maximal ;\n- effet actuel ;\n- effet après la file ;\n- effet du prochain niveau ;\n- coût ;\n- durée catalogue et durée effective ;\n- énergie projetée ;\n- raison précise d’un blocage.\n\nLe bouton principal lance l’amélioration sans outil de debug.\n\n### File de construction\n\nLa colonne droite distingue clairement :\n\n- ordre en cours ;\n- progression ;\n- temps restant ;\n- coût réservé ;\n- ordres en attente ;\n- capacité utilisée de la file.\n\nLes refus, ajouts et achèvements apparaissent comme un message non bloquant au\nbas de l’écran.\n\n### HUD stratégique\n\nL’inspecteur de planète conserve seulement un résumé économique et indique la\ntouche `C`. Les informations détaillées ne sont plus répétées dans plusieurs\npanneaux.\n\nCette étape est client-only :\n\n- aucune modification du domaine ;\n- aucune modification de la simulation ;\n- aucune migration de sauvegarde ;\n- `GAME_STATE_VERSION` reste 8 ;\n- `SAVE_VERSION` reste 9.\n'


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
        print(
            result.stdout,
            end="" if result.stdout.endswith("\n")
            else "\n",
        )
    if check and result.returncode != 0:
        raise SystemExit(
            f"Commande en échec ({result.returncode}) : "
            f"{' '.join(command)}"
        )
    return result


def find_root(start: Path) -> Path:
    for candidate in [start, *start.parents]:
        if (
            (candidate / ".git").exists()
            and (candidate / "Cargo.toml").exists()
            and (
                candidate
                / "crates/galactic_client/src/lib.rs"
            ).exists()
            and (
                candidate
                / "crates/galactic_sim/src/construction.rs"
            ).exists()
        ):
            return candidate
    raise SystemExit(
        "Racine Galactic introuvable. Utilise --root."
    )


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


def replace_once(
    source: str,
    old: str,
    new: str,
    description: str,
) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"Patch impossible pour {description}: "
            f"{count} occurrence(s), 1 attendue."
        )
    return source.replace(old, new, 1)


def verify_baseline(root: Path, force: bool) -> None:
    head = run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
    ).stdout.strip()
    if head == EXPECTED_BASELINE_COMMIT:
        print(f"Baseline reconnue : {head}")
        return

    ancestor = run(
        [
            "git",
            "merge-base",
            "--is-ancestor",
            EXPECTED_BASELINE_COMMIT,
            "HEAD",
        ],
        cwd=root,
        check=False,
    )
    if ancestor.returncode == 0:
        print(
            "Baseline présente dans l'historique ; "
            f"HEAD actuel : {head}"
        )
        return
    if force:
        print(
            "WARNING: baseline différente, poursuite "
            "autorisée par --force."
        )
        return

    raise SystemExit(
        "Le dépôt local ne correspond pas à la baseline "
        "MVP-014 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")
    construction = (
        root / "crates/galactic_sim/src/construction.rs"
    ).read_text(encoding="utf-8")
    state = (
        root / "crates/galactic_sim/src/state.rs"
    ).read_text(encoding="utf-8")
    persistence = (
        root / "crates/galactic_persistence/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "// MVP-014: compact resources and construction UI.",
        "ConstructionPanelRoot",
        "ResourceDashboardRoot",
        "fn update_construction_panel(",
    ):
        if marker not in client:
            failures.append(
                f"marqueur UI absent : {marker}"
            )
    for marker in (
        "pub struct ConstructionQueue",
        "pub fn building_upgrade_quote",
    ):
        if marker not in construction:
            failures.append(
                f"marqueur construction absent : {marker}"
            )
    if "GAME_STATE_VERSION: u32 = 8" not in state:
        failures.append("GAME_STATE_VERSION != 8")
    if "SAVE_VERSION: u32 = 9" not in persistence:
        failures.append("SAVE_VERSION != 9")

    if failures:
        raise SystemExit(
            "Baseline MVP-014 incohérente :\n- "
            + "\n- ".join(failures)
        )


def cargo_edition(root: Path) -> str:
    cargo = (root / "Cargo.toml").read_text(
        encoding="utf-8"
    )
    match = re.search(
        r'(?m)^edition\s*=\s*"([^"]+)"',
        cargo,
    )
    return match.group(1) if match else "2024"


def format_rust(root: Path, content: str) -> str:
    rustfmt = shutil.which("rustfmt")
    if rustfmt is None:
        raise SystemExit(
            "rustfmt est requis, y compris pour --dry-run."
        )

    with tempfile.NamedTemporaryFile(
        mode="w",
        suffix=".rs",
        encoding="utf-8",
        delete=False,
    ) as handle:
        temporary = Path(handle.name)
        handle.write(normalize(content))

    try:
        result = subprocess.run(
            [
                rustfmt,
                "--edition",
                cargo_edition(root),
                "--config",
                "skip_children=true",
                str(temporary),
            ],
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        if result.returncode != 0:
            raise SystemExit(
                "rustfmt n'a pas pu formater une source "
                f"générée :\n{result.stdout}"
            )
        return normalize(
            temporary.read_text(encoding="utf-8")
        )
    finally:
        temporary.unlink(missing_ok=True)


def patch_client(source: str) -> str:
    if "// MVP-015: dedicated colony management screen." in source:
        return normalize(source)

    source = replace_once(
        source,
        "use std::{collections::HashMap, time::Duration};\n\n"
        "use bevy::input::mouse",
        "use std::{collections::HashMap, time::Duration};\n\n"
        "use bevy::ecs::system::SystemParam;\n"
        "use bevy::input::mouse",
        "import SystemParam",
    )

    source = replace_once(
        source,
        "        .init_resource::<PointerSelectionState>()\n",
        "        .init_resource::<PointerSelectionState>()\n"
        "        .init_resource::<ColonyManagementState>()\n",
        "ressource de gestion",
    )

    old_schedule = """                handle_pointer_selection,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                handle_construction_buttons,
                update_action_buttons,
                update_construction_panel,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_resource_dashboard,
                update_resource_card_help,
                update_ui,
                update_info_panel,
"""
    new_schedule = """                handle_pointer_selection,
                capture_colony_management_feedback,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                handle_colony_management_buttons,
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_colony_management_visibility,
                update_colony_management_resources,
                update_colony_management_buildings,
                update_colony_management_detail,
                update_colony_management_queue,
                update_ui,
                update_info_panel,
"""
    source = replace_once(
        source,
        old_schedule,
        new_schedule,
        "systèmes de gestion",
    )
    old_presentation_schedule = """        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_strategic_camera,
                update_pointer_candidates,
                handle_pointer_selection,
                capture_colony_management_feedback,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                handle_colony_management_buttons,
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_colony_management_visibility,
                update_colony_management_resources,
                update_colony_management_buildings,
                update_colony_management_detail,
                update_colony_management_queue,
                update_ui,
                update_info_panel,
            )
                .chain(),
        );
"""
    new_presentation_schedule = """        .configure_sets(
            Update,
            (
                PresentationUpdateSet::View,
                PresentationUpdateSet::Interaction,
                PresentationUpdateSet::Management,
                PresentationUpdateSet::Ui,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_strategic_camera,
                update_pointer_candidates,
                handle_pointer_selection,
                capture_colony_management_feedback,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
            )
                .chain()
                .in_set(PresentationUpdateSet::View),
        )
        .add_systems(
            Update,
            (
                handle_action_buttons,
                handle_colony_management_buttons,
            )
                .chain()
                .in_set(PresentationUpdateSet::Interaction),
        )
        .add_systems(
            Update,
            (
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_colony_management_visibility,
                update_colony_management_resources,
                update_colony_management_buildings,
                update_colony_management_detail,
                update_colony_management_queue,
            )
                .chain()
                .in_set(PresentationUpdateSet::Management),
        )
        .add_systems(
            Update,
            (
                update_ui,
                update_info_panel,
            )
                .chain()
                .in_set(PresentationUpdateSet::Ui),
        );
"""
    source = replace_once(
        source,
        old_presentation_schedule,
        new_presentation_schedule,
        "ordonnancement de présentation",
    )
    source = replace_once(
        source,
        "\n#[derive(Resource)]\npub struct SimulationResource",
        "\n#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]\n"
        "enum PresentationUpdateSet {\n"
        "    View,\n"
        "    Interaction,\n"
        "    Management,\n"
        "    Ui,\n"
        "}\n\n"
        "#[derive(Resource)]\n"
        "pub struct SimulationResource",
        "sets de présentation",
    )

    types_pattern = re.compile(
        r"// MVP-014: compact resources and construction UI\."
        r".*?"
        r"(?=// MVP-010: partial-information inspectors)",
        flags=re.DOTALL,
    )
    source, count = types_pattern.subn(
        MANAGEMENT_TYPES.rstrip() + "\n\n",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "Bloc de types MVP-014 introuvable."
        )
    source = replace_once(
        source,
        "struct ManagementQueueProgressFill;\n\n"
        "// MVP-010: partial-information inspectors",
        "struct ManagementQueueProgressFill;\n\n"
        "const COLONY_MANAGEMENT_Z_INDEX: i32 = 100;\n\n"
        "// MVP-010: partial-information inspectors",
        "constante de couche gestion",
    )

    source = replace_once(
        source,
        '            spawn_action_button(parent, UiAction::RebuildView, "Reconstruire", "R");\n',
        '            spawn_action_button(parent, UiAction::RebuildView, "Reconstruire", "R");\n'
        "            spawn_colony_management_toggle(parent);\n",
        "bouton de gestion",
    )
    source = replace_once(
        source,
        "    spawn_resource_dashboard(&mut commands);\n"
        "    spawn_construction_panel(&mut commands);\n",
        "    spawn_colony_management_screen(&mut commands);\n",
        "écran de gestion",
    )

    spawn_pattern = re.compile(
        r"fn spawn_resource_dashboard\(.*?\n"
        r"(?=fn spawn_panel_heading)",
        flags=re.DOTALL,
    )
    source, count = spawn_pattern.subn(
        MANAGEMENT_SPAWN.rstrip() + "\n\n",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "Bloc de spawn économie/construction introuvable."
        )
    source = replace_once(
        source,
        "            Visibility::Hidden,\n"
        "            Interaction::None,\n"
        "            UiPointerBlocker,\n"
        "            ColonyManagementRoot,\n",
        "            Visibility::Hidden,\n"
        "            GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX),\n"
        "            Interaction::None,\n"
        "            UiPointerBlocker,\n"
        "            ColonyManagementRoot,\n",
        "couche globale écran de gestion",
    )

    view_pattern = re.compile(
        r"fn handle_view_input\(.*?\n\}\n\n"
        r"(?=fn handle_action_buttons)",
        flags=re.DOTALL,
    )
    view_replacement = r"""fn handle_view_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut pointer_state: ResMut<PointerSelectionState>,
    mut management: ResMut<ColonyManagementState>,
) {
    if keyboard.just_pressed(KeyCode::KeyC) {
        toggle_colony_management(
            &mut management,
            &mut simulation,
        );
        pointer_state.ambiguity = None;
        return;
    }

    if management.open {
        if keyboard.just_pressed(KeyCode::Escape) {
            management.open = false;
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            cycle_management_colony(
                &mut management,
                &mut simulation,
                true,
            );
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            cycle_management_colony(
                &mut management,
                &mut simulation,
                false,
            );
        }
        return;
    }

    if pointer_state.ambiguity.is_some() {
        if keyboard.just_pressed(KeyCode::Tab) {
            let reverse = keyboard.any_pressed([
                KeyCode::ShiftLeft,
                KeyCode::ShiftRight,
            ]);
            if let Some(target) =
                pointer_state.cycle_ambiguity(reverse)
            {
                select_pick_target(
                    &mut simulation,
                    target,
                );
            }
            return;
        }
        if keyboard.just_pressed(KeyCode::Enter) {
            pointer_state.ambiguity = None;
            return;
        }
        if keyboard.just_pressed(KeyCode::Escape) {
            pointer_state.ambiguity = None;
            return;
        }
    }

    if let Some(action) = view_shortcut(&keyboard) {
        apply_ui_action(
            action,
            &mut simulation,
            &mut navigation,
            &mut rebuild,
        );
    }
}

"""
    source, count = view_pattern.subn(
        view_replacement,
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "handle_view_input introuvable."
        )

    source = replace_once(
        source,
        "fn update_strategic_camera(\n"
        "    time: Res<Time>,\n"
        "    keyboard: Res<ButtonInput<KeyCode>>,\n"
        "    mouse_buttons: Res<ButtonInput<MouseButton>>,\n"
        "    mouse_motion: Res<AccumulatedMouseMotion>,\n"
        "    mouse_scroll: Res<AccumulatedMouseScroll>,\n"
        "    mut navigation: ResMut<StrategicNavigation>,\n"
        "    mut query: Query<&mut Transform, With<StrategicCamera>>,\n"
        ") {\n",
        "#[derive(SystemParam)]\n"
        "struct StrategicCameraInput<'w> {\n"
        "    time: Res<'w, Time>,\n"
        "    keyboard: Res<'w, ButtonInput<KeyCode>>,\n"
        "    mouse_buttons: Res<'w, ButtonInput<MouseButton>>,\n"
        "    mouse_motion: Res<'w, AccumulatedMouseMotion>,\n"
        "    mouse_scroll: Res<'w, AccumulatedMouseScroll>,\n"
        "    management: Res<'w, ColonyManagementState>,\n"
        "}\n\n"
        "fn update_strategic_camera(\n"
        "    input: StrategicCameraInput,\n"
        "    mut navigation: ResMut<StrategicNavigation>,\n"
        "    mut query: Query<&mut Transform, With<StrategicCamera>>,\n"
        ") {\n",
        "paramètres caméra",
    )
    source = replace_once(
        source,
        "    let Ok(mut transform) = query.single_mut() else {\n"
        "        return;\n"
        "    };\n\n"
        "    let delta_seconds = time.delta_secs();\n",
        "    let Ok(mut transform) = query.single_mut() else {\n"
        "        return;\n"
        "    };\n"
        "    if input.management.open {\n"
        "        return;\n"
        "    }\n\n"
        "    let delta_seconds = input.time.delta_secs();\n",
        "arrêt caméra en gestion",
    )
    source = source.replace(
        "    let motion = mouse_motion.delta;\n",
        "    let motion = input.mouse_motion.delta;\n",
    )
    source = source.replace(
        "    let scroll_lines = match mouse_scroll.unit {\n"
        "        MouseScrollUnit::Line => mouse_scroll.delta.y,\n"
        "        MouseScrollUnit::Pixel => mouse_scroll.delta.y / 40.0,\n"
        "    };\n",
        "    let scroll_lines = match input.mouse_scroll.unit {\n"
        "        MouseScrollUnit::Line => input.mouse_scroll.delta.y,\n"
        "        MouseScrollUnit::Pixel => input.mouse_scroll.delta.y / 40.0,\n"
        "    };\n",
    )
    source = source.replace(
        "mouse_buttons.pressed(MouseButton::",
        "input.mouse_buttons.pressed(MouseButton::",
    )
    source = source.replace(
        "keyboard_pan_direction(&keyboard,",
        "keyboard_pan_direction(&input.keyboard,",
    )
    source = source.replace(
        "                &keyboard,\n"
        "                delta_seconds,\n",
        "                &input.keyboard,\n"
        "                delta_seconds,\n",
    )

    systems_pattern = re.compile(
        r"fn update_resource_dashboard\(.*?\n"
        r"(?=fn update_action_buttons)",
        flags=re.DOTALL,
    )
    source, count = systems_pattern.subn(
        MANAGEMENT_SYSTEMS.rstrip() + "\n\n",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "Bloc des anciens panneaux introuvable."
        )

    economy_pattern = re.compile(
        r"fn colony_economy_text\(.*?\n\}\n\n"
        r"(?=fn format_saturation_time)",
        flags=re.DOTALL,
    )
    source, count = economy_pattern.subn(
        COMPACT_ECONOMY.rstrip() + "\n\n",
        source,
        count=1,
    )
    if count != 1:
        raise SystemExit(
            "colony_economy_text introuvable."
        )

    status_test_pattern = re.compile(
        r"    #\[test\]\n"
        r"    fn resource_status_distinguishes_reservations_and_saturation"
        r"\(\) \{.*?\n    \}\n\n",
        flags=re.DOTALL,
    )
    source, status_count = status_test_pattern.subn(
        """    #[test]
    fn resource_status_distinguishes_normal_nearly_full_and_full() {
        assert_eq!(
            resource_hud_status(40, 100),
            ResourceHudStatus::Normal
        );
        assert_eq!(
            resource_hud_status(90, 100),
            ResourceHudStatus::NearlyFull
        );
        assert_eq!(
            resource_hud_status(100, 100),
            ResourceHudStatus::Full
        );
    }

""",
        source,
        count=1,
    )
    if status_count != 1:
        raise SystemExit(
            "Test de statut des ressources introuvable."
        )

    for test_name in (
        "resource_dashboard_system_queries_are_disjoint",
        "construction_panel_system_queries_are_disjoint",
        "dashboard_uses_the_selected_player_colony",
    ):
        pattern = re.compile(
            r"    #\[test\]\n"
            r"    fn "
            + re.escape(test_name)
            + r"\(\) \{.*?\n    \}\n\n",
            flags=re.DOTALL,
        )
        source, _ = pattern.subn(
            "",
            source,
            count=1,
        )

    source = replace_once(
        source,
        '        assert!(rendered.contains("STOCKS EXACTS"));\n',
        '        assert!(rendered.contains("ÉCONOMIE — RÉSUMÉ"));\n'
        '        assert!(rendered.contains("Gestion complète : touche C"));\n',
        "test inspecteur économie compact",
    )

    test_marker = (
        "    #[test]\n"
        "    fn ui_font_uses_a_system_sans_serif()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests MVP-015 introuvable."
        )
    source = source.replace(
        test_marker,
        TESTS_CODE.rstrip()
        + "\n\n"
        + test_marker,
        1,
    )
    z_index_test = """    #[test]
    fn colony_management_screen_uses_global_overlay_layer() {
        fn spawn_management_for_test(mut commands: Commands) {
            spawn_colony_management_screen(&mut commands);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Startup, spawn_management_for_test);

        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&GlobalZIndex, With<ColonyManagementRoot>>();
        let z_index = query
            .single(app.world())
            .expect("management root should have a global z-index");

        assert_eq!(
            *z_index,
            GlobalZIndex(COLONY_MANAGEMENT_Z_INDEX)
        );
        assert!(z_index.0 > 0);
    }

"""
    source = source.replace(test_marker, z_index_test + test_marker, 1)

    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-015 — Écran de gestion planétaire" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    client = root / "crates/galactic_client/src/lib.rs"
    before = client.read_text(encoding="utf-8")
    after = format_rust(root, patch_client(before))
    if before != after:
        updates.append(Update(client, before, after))

    docs = root / "docs/mvp_architecture.md"
    before = docs.read_text(encoding="utf-8")
    after = patch_docs(before)
    if before != after:
        updates.append(Update(docs, before, after))

    validate_prospective(root, updates)
    return updates


def validate_prospective(
    root: Path,
    updates: list[Update],
) -> None:
    mapped = {
        update.path: update.after for update in updates
    }
    client_path = (
        root / "crates/galactic_client/src/lib.rs"
    )
    client = mapped.get(
        client_path,
        client_path.read_text(encoding="utf-8"),
    )

    required = (
        "ColonyManagementState",
        "spawn_colony_management_screen",
        "handle_colony_management_buttons",
        "update_colony_management_detail",
        "construction_queue_detail_label",
        "Gestion complète : touche C",
    )
    failures = [
        f"marqueur absent : {marker}"
        for marker in required
        if marker not in client
    ]
    for obsolete in (
        "ResourceDashboardRoot",
        "ConstructionPanelRoot",
        "fn update_resource_dashboard(",
        "fn update_construction_panel(",
    ):
        if obsolete in client:
            failures.append(
                f"ancien overlay encore présent : {obsolete}"
            )

    if failures:
        raise SystemExit(
            "Migration MVP-015 incomplète :\n- "
            + "\n- ".join(failures)
        )


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
        print("MVP-015 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp015-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        backup = backup_root / relative
        backup.parent.mkdir(
            parents=True,
            exist_ok=True,
        )
        shutil.copy2(update.path, backup)
        update.path.write_text(
            update.after,
            encoding="utf-8",
        )
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def checks(root: Path) -> None:
    run(
        ["cargo", "fmt", "--all"],
        cwd=root,
        capture=False,
    )
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
    run(
        ["cargo", "test", "--workspace"],
        cwd=root,
        capture=False,
    )
    run(
        ["cargo", "build", "--release"],
        cwd=root,
        capture=False,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
    )
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument(
        "--force",
        action="store_true",
    )
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_current_state(root)

    status = run(
        ["git", "status", "--porcelain"],
        cwd=root,
    ).stdout
    if status.strip():
        print(
            "WARNING: working tree already contains changes."
        )
        print(
            status,
            end="" if status.endswith("\n")
            else "\n",
        )

    updates = collect_updates(root)
    apply_updates(updates, root, args.dry_run)

    if args.dry_run:
        print(
            f"\nDry-run complete: {len(updates)} "
            "file(s) would change."
        )
        return 0

    if args.skip_checks:
        print(
            "\nChecks ignorés. Lance ensuite :\n"
            "  cargo fmt --all\n"
            "  cargo clippy --workspace --all-targets "
            "--all-features -- -D warnings\n"
            "  cargo test --workspace\n"
            "  cargo build --release"
        )
    else:
        checks(root)

    print(
        "\nMVP-015 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

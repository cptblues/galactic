#!/usr/bin/env python3
"""
Applique MVP-013-B au dépôt Galactic.

Baseline analysée :
    af36afd256f8b55e9de4225ae75d29a3cb36b5e8
    feat prepare construction

Le script ajoute :
- quatre cartes économiques compactes ;
- jauges de stockage et de consommation énergétique ;
- distinction total, disponible et réservé ;
- temps avant saturation et avant prochain crédit ;
- alertes de saturation et déficit ;
- deltas temporaires après ProductionRefreshed ;
- aide contextuelle au survol ;
- masquage hors sélection d’une colonie du joueur.

Usage :
    python tools/apply_mvp_013_b.py --dry-run
    python tools/apply_mvp_013_b.py
    python tools/apply_mvp_013_b.py --skip-checks
    python tools/apply_mvp_013_b.py --root /chemin/vers/galactic

Le script est idempotent.
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
    "af36afd256f8b55e9de4225ae75d29a3cb36b5e8"
)

TYPES_CODE = '\n// MVP-013-B: compact resource dashboard for colonized worlds.\nconst RESOURCE_DELTA_DISPLAY_SECONDS: f32 = 2.2;\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum ResourceHudKind {\n    Metal,\n    Crystal,\n    Fuel,\n    Energy,\n}\n\nimpl ResourceHudKind {\n    const ALL: [Self; 4] = [\n        Self::Metal,\n        Self::Crystal,\n        Self::Fuel,\n        Self::Energy,\n    ];\n\n    const fn title(self) -> &\'static str {\n        match self {\n            Self::Metal => "MÉTAL",\n            Self::Crystal => "CRISTAL",\n            Self::Fuel => "CARBURANT",\n            Self::Energy => "ÉNERGIE",\n        }\n    }\n\n    const fn help(self) -> &\'static str {\n        match self {\n            Self::Metal | Self::Crystal | Self::Fuel => {\n                "Total = stock physique. Disponible = total - réservé. \\\nCapacité = limite de stockage. Une jauge orange ou rouge signale une saturation proche."\n            }\n            Self::Energy => {\n                "L’énergie est une capacité, pas un stock. Disponible = production effective - \\\nconsommation. Un déficit ralentit tous les extracteurs."\n            }\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\nenum ResourceHudStatus {\n    Normal,\n    Reserved,\n    NearlyFull,\n    Full,\n    Deficit,\n}\n\n#[derive(Resource, Default)]\nstruct ResourceDeltaState {\n    produced: ResourceStock,\n    remaining_seconds: f32,\n}\n\nimpl ResourceDeltaState {\n    fn show(&mut self, produced: ResourceStock) {\n        self.produced = produced;\n        self.remaining_seconds = RESOURCE_DELTA_DISPLAY_SECONDS;\n    }\n\n    fn tick(&mut self, delta_seconds: f32) {\n        self.remaining_seconds =\n            (self.remaining_seconds - delta_seconds).max(0.0);\n        if self.remaining_seconds == 0.0 {\n            self.produced = ResourceStock::ZERO;\n        }\n    }\n\n    fn active(&self) -> ResourceStock {\n        if self.remaining_seconds > 0.0 {\n            self.produced\n        } else {\n            ResourceStock::ZERO\n        }\n    }\n}\n\n#[derive(Component)]\nstruct ResourceDashboardRoot;\n\n#[derive(Component)]\nstruct ResourceDashboardHeaderText;\n\n#[derive(Component)]\nstruct ResourceDashboardHelpText;\n\n#[derive(Component)]\nstruct ResourceHudCard {\n    kind: ResourceHudKind,\n}\n\n#[derive(Component)]\nstruct ResourceHudCardText {\n    kind: ResourceHudKind,\n}\n\n#[derive(Component)]\nstruct ResourceHudGaugeFill {\n    kind: ResourceHudKind,\n}\n'
SPAWN_CODE = '\nfn spawn_resource_dashboard(commands: &mut Commands) {\n    commands\n        .spawn((\n            Node {\n                position_type: PositionType::Absolute,\n                left: Val::Px(238.0),\n                right: Val::Px(372.0),\n                bottom: Val::Px(54.0),\n                min_width: Val::Px(580.0),\n                padding: UiRect::all(Val::Px(9.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(Val::Px(7.0)),\n                flex_direction: FlexDirection::Column,\n                ..default()\n            },\n            BackgroundColor(Color::srgba(0.012, 0.020, 0.028, 0.96)),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                Color::srgba(0.30, 0.70, 0.76, 0.48),\n            ),\n            Visibility::Hidden,\n            Interaction::None,\n            UiPointerBlocker,\n            ResourceDashboardRoot,\n        ))\n        .with_children(|root| {\n            root.spawn((\n                Text::new("ÉCONOMIE"),\n                ui_text_font(13.0),\n                TextColor(Color::srgb(0.78, 0.92, 0.96)),\n                Node {\n                    margin: UiRect {\n                        bottom: Val::Px(7.0),\n                        ..default()\n                    },\n                    ..default()\n                },\n                ResourceDashboardHeaderText,\n            ));\n\n            root.spawn((\n                Node {\n                    width: Val::Percent(100.0),\n                    min_height: Val::Px(104.0),\n                    flex_direction: FlexDirection::Row,\n                    align_items: AlignItems::Stretch,\n                    ..default()\n                },\n            ))\n            .with_children(|row| {\n                for kind in ResourceHudKind::ALL {\n                    spawn_resource_hud_card(row, kind);\n                }\n            });\n\n            root.spawn((\n                Text::new(\n                    "Survole une carte pour afficher son aide.",\n                ),\n                ui_text_font(10.0),\n                TextColor(Color::srgb(0.62, 0.70, 0.76)),\n                Node {\n                    margin: UiRect {\n                        top: Val::Px(6.0),\n                        ..default()\n                    },\n                    ..default()\n                },\n                ResourceDashboardHelpText,\n            ));\n        });\n}\n\nfn spawn_resource_hud_card(\n    parent: &mut ChildSpawnerCommands,\n    kind: ResourceHudKind,\n) {\n    parent\n        .spawn((\n            Node {\n                flex_grow: 1.0,\n                flex_basis: Val::Px(0.0),\n                min_width: Val::Px(126.0),\n                margin: UiRect {\n                    left: Val::Px(3.0),\n                    right: Val::Px(3.0),\n                    ..default()\n                },\n                padding: UiRect::all(Val::Px(8.0)),\n                border: UiRect::all(Val::Px(1.0)),\n                border_radius: BorderRadius::all(Val::Px(5.0)),\n                flex_direction: FlexDirection::Column,\n                justify_content: JustifyContent::SpaceBetween,\n                ..default()\n            },\n            BackgroundColor(resource_card_background(kind)),\n            Outline::new(\n                Val::Px(1.0),\n                Val::ZERO,\n                resource_outline_color(kind),\n            ),\n            Interaction::None,\n            UiPointerBlocker,\n            ResourceHudCard { kind },\n        ))\n        .with_children(|card| {\n            card.spawn((\n                Text::new(kind.title()),\n                ui_text_font(11.0),\n                TextColor(resource_kind_color(kind)),\n                ResourceHudCardText { kind },\n            ));\n\n            card\n                .spawn((\n                    Node {\n                        width: Val::Percent(100.0),\n                        height: Val::Px(7.0),\n                        border_radius: BorderRadius::all(\n                            Val::Px(4.0),\n                        ),\n                        ..default()\n                    },\n                    BackgroundColor(Color::srgba(\n                        0.10, 0.13, 0.16, 0.92,\n                    )),\n                ))\n                .with_children(|gauge| {\n                    gauge.spawn((\n                        Node {\n                            width: Val::Percent(0.0),\n                            height: Val::Percent(100.0),\n                            border_radius: BorderRadius::all(\n                                Val::Px(4.0),\n                            ),\n                            ..default()\n                        },\n                        BackgroundColor(resource_kind_color(kind)),\n                        ResourceHudGaugeFill { kind },\n                    ));\n                });\n        });\n}\n'
SYSTEMS_CODE = '\nfn capture_resource_deltas(\n    simulation: Res<SimulationResource>,\n    mut delta_state: ResMut<ResourceDeltaState>,\n) {\n    for event in &simulation.pending_events {\n        if let GameEvent::ProductionRefreshed(report) = event\n            && !report.produced.is_zero()\n        {\n            delta_state.show(report.produced);\n        }\n    }\n}\n\nfn update_resource_dashboard(\n    time: Res<Time>,\n    simulation: Res<SimulationResource>,\n    mut delta_state: ResMut<ResourceDeltaState>,\n    mut roots: Query<\n        &mut Visibility,\n        With<ResourceDashboardRoot>,\n    >,\n    mut headers: Query<\n        &mut Text,\n        With<ResourceDashboardHeaderText>,\n    >,\n    mut card_texts: Query<(\n        &ResourceHudCardText,\n        &mut Text,\n        &mut TextColor,\n    )>,\n    mut gauges: Query<(\n        &ResourceHudGaugeFill,\n        &mut Node,\n        &mut BackgroundColor,\n    )>,\n) {\n    delta_state.tick(time.delta_secs());\n\n    let Some(colony) =\n        selected_colony_for_resource_dashboard(\n            simulation.simulation(),\n        )\n    else {\n        for mut visibility in &mut roots {\n            *visibility = Visibility::Hidden;\n        }\n        return;\n    };\n\n    for mut visibility in &mut roots {\n        *visibility = Visibility::Visible;\n    }\n\n    let production =\n        galactic_sim::colony_production_snapshot(colony);\n    let refresh =\n        galactic_sim::StrategicDuration::from_ticks(\n            u64::from(production.ticks_until_refresh),\n        );\n    for mut header in &mut headers {\n        header.0 = format!(\n            "ÉCONOMIE — {}  •  prochain crédit {}  •  cadence {} s",\n            colony.name,\n            format_strategic_duration(refresh),\n            galactic_sim::PRODUCTION_REFRESH_SECONDS,\n        );\n    }\n\n    let active_delta = delta_state.active();\n    for (card, mut text, mut text_color) in\n        &mut card_texts\n    {\n        let view = resource_hud_view(\n            card.kind,\n            colony,\n            production,\n            active_delta,\n        );\n        text.0 = view.text;\n        text_color.0 = status_text_color(\n            card.kind,\n            view.status,\n        );\n    }\n\n    for (gauge, mut node, mut background) in\n        &mut gauges\n    {\n        let view = resource_hud_view(\n            gauge.kind,\n            colony,\n            production,\n            active_delta,\n        );\n        node.width = Val::Percent(\n            (view.fill_ratio * 100.0).clamp(0.0, 100.0),\n        );\n        background.0 = status_gauge_color(\n            gauge.kind,\n            view.status,\n        );\n    }\n}\n\nfn update_resource_card_help(\n    cards: Query<(&Interaction, &ResourceHudCard)>,\n    mut help_texts: Query<\n        &mut Text,\n        With<ResourceDashboardHelpText>,\n    >,\n) {\n    let hovered = cards\n        .iter()\n        .find(|(interaction, _)| {\n            **interaction != Interaction::None\n        })\n        .map(|(_, card)| card.kind);\n\n    let text = hovered\n        .map(ResourceHudKind::help)\n        .unwrap_or(\n            "Total / capacité • disponible = total - réservé • \\\nles stocks sont crédités toutes les 5 secondes stratégiques.",\n        );\n    for mut help in &mut help_texts {\n        help.0 = text.to_string();\n    }\n}\n\nfn selected_colony_for_resource_dashboard(\n    simulation: &Simulation,\n) -> Option<&galactic_sim::ColonyState> {\n    let state = simulation.state();\n    match state.selected {\n        SelectionTarget::Planet { planet_id, .. } => {\n            state.colony_on_planet(planet_id)\n        }\n        SelectionTarget::System(system_id) => {\n            state.colonies.iter().find(|colony| {\n                colony.system_id == system_id\n                    && colony.faction == state.player_faction\n            })\n        }\n        SelectionTarget::None => state.player_home_colony(),\n    }\n}\n\nstruct ResourceHudView {\n    text: String,\n    fill_ratio: f32,\n    status: ResourceHudStatus,\n}\n\nfn resource_hud_view(\n    kind: ResourceHudKind,\n    colony: &galactic_sim::ColonyState,\n    production: galactic_sim::ColonyProductionSnapshot,\n    delta: ResourceStock,\n) -> ResourceHudView {\n    if kind == ResourceHudKind::Energy {\n        return energy_hud_view(production);\n    }\n\n    let stock = resource_value(kind, colony.resources.stock());\n    let available =\n        resource_value(kind, colony.resources.available());\n    let reserved =\n        resource_value(kind, colony.resources.reserved_total());\n    let capacity =\n        resource_value(kind, production.capacity);\n    let delta = resource_value(kind, delta);\n    let rate = resource_rate_per_second(kind, production);\n    let saturation =\n        resource_saturation(kind, production);\n    let fill_ratio = resource_fill_ratio(stock, capacity);\n    let status = resource_hud_status(\n        stock,\n        available,\n        reserved,\n        capacity,\n    );\n\n    let delta_text = if delta > 0 {\n        format!("\\nDernier crédit  +{delta}")\n    } else {\n        String::new()\n    };\n\n    ResourceHudView {\n        text: format!(\n            "{}  {} / {}\\nDisponible {}  •  Réservé {}\\n+{:.2}/s  •  saturation {}\\n{}{}",\n            kind.title(),\n            stock,\n            capacity,\n            available,\n            reserved,\n            rate,\n            format_saturation_time(saturation),\n            resource_status_label(status),\n            delta_text,\n        ),\n        fill_ratio,\n        status,\n    }\n}\n\nfn energy_hud_view(\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> ResourceHudView {\n    let produced = production.effective_energy_production;\n    let consumed = production.energy_consumption;\n    let available = produced.saturating_sub(consumed);\n    let deficit = consumed > produced;\n    let status = if deficit {\n        ResourceHudStatus::Deficit\n    } else {\n        ResourceHudStatus::Normal\n    };\n    let fill_ratio = energy_fill_ratio(\n        consumed,\n        produced,\n    );\n    let balance =\n        i128::from(produced) - i128::from(consumed);\n\n    ResourceHudView {\n        text: format!(\n            "ÉNERGIE  {} / {}\\nDisponible {}  •  Bilan {:+}\\nRendement extracteurs {}%\\n{}",\n            consumed,\n            produced,\n            available,\n            balance,\n            u32::from(\n                production.energy_efficiency_per_mille,\n            ) / 10,\n            resource_status_label(status),\n        ),\n        fill_ratio,\n        status,\n    }\n}\n\nfn resource_value(\n    kind: ResourceHudKind,\n    stock: ResourceStock,\n) -> u64 {\n    match kind {\n        ResourceHudKind::Metal => stock.metal,\n        ResourceHudKind::Crystal => stock.crystal,\n        ResourceHudKind::Fuel => stock.fuel,\n        ResourceHudKind::Energy => 0,\n    }\n}\n\nfn resource_rate_per_second(\n    kind: ResourceHudKind,\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> f64 {\n    match kind {\n        ResourceHudKind::Metal => {\n            production.effective_rate.metal_per_second()\n        }\n        ResourceHudKind::Crystal => {\n            production.effective_rate.crystal_per_second()\n        }\n        ResourceHudKind::Fuel => {\n            production.effective_rate.fuel_per_second()\n        }\n        ResourceHudKind::Energy => 0.0,\n    }\n}\n\nfn resource_saturation(\n    kind: ResourceHudKind,\n    production: galactic_sim::ColonyProductionSnapshot,\n) -> galactic_sim::SaturationTime {\n    match kind {\n        ResourceHudKind::Metal => {\n            production.saturation.metal\n        }\n        ResourceHudKind::Crystal => {\n            production.saturation.crystal\n        }\n        ResourceHudKind::Fuel => {\n            production.saturation.fuel\n        }\n        ResourceHudKind::Energy => {\n            galactic_sim::SaturationTime::Never\n        }\n    }\n}\n\nfn resource_fill_ratio(\n    stock: u64,\n    capacity: u64,\n) -> f32 {\n    if capacity == 0 {\n        return if stock == 0 { 0.0 } else { 1.0 };\n    }\n    (stock as f64 / capacity as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\nfn energy_fill_ratio(\n    consumption: u64,\n    production: u64,\n) -> f32 {\n    if production == 0 {\n        return if consumption == 0 { 0.0 } else { 1.0 };\n    }\n    (consumption as f64 / production as f64)\n        .clamp(0.0, 1.0) as f32\n}\n\nfn resource_hud_status(\n    stock: u64,\n    available: u64,\n    reserved: u64,\n    capacity: u64,\n) -> ResourceHudStatus {\n    if capacity > 0 && stock >= capacity {\n        ResourceHudStatus::Full\n    } else if capacity > 0\n        && stock.saturating_mul(100)\n            >= capacity.saturating_mul(90)\n    {\n        ResourceHudStatus::NearlyFull\n    } else if available == 0 && reserved > 0 {\n        ResourceHudStatus::Reserved\n    } else {\n        ResourceHudStatus::Normal\n    }\n}\n\nconst fn resource_status_label(\n    status: ResourceHudStatus,\n) -> &\'static str {\n    match status {\n        ResourceHudStatus::Normal => "STABLE",\n        ResourceHudStatus::Reserved => "INDISPONIBLE — RÉSERVÉ",\n        ResourceHudStatus::NearlyFull => "PRESQUE PLEIN",\n        ResourceHudStatus::Full => "PLEIN — PRODUCTION PERDUE",\n        ResourceHudStatus::Deficit => "DÉFICIT ÉNERGÉTIQUE",\n    }\n}\n\nfn resource_kind_color(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgb(0.90, 0.66, 0.42)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgb(0.56, 0.82, 0.96)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgb(0.86, 0.82, 0.38)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgb(0.52, 0.92, 0.62)\n        }\n    }\n}\n\nfn resource_outline_color(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgba(0.90, 0.66, 0.42, 0.42)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgba(0.56, 0.82, 0.96, 0.42)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgba(0.86, 0.82, 0.38, 0.42)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgba(0.52, 0.92, 0.62, 0.42)\n        }\n    }\n}\n\nfn resource_card_background(\n    kind: ResourceHudKind,\n) -> Color {\n    match kind {\n        ResourceHudKind::Metal => {\n            Color::srgba(0.16, 0.10, 0.06, 0.94)\n        }\n        ResourceHudKind::Crystal => {\n            Color::srgba(0.06, 0.11, 0.16, 0.94)\n        }\n        ResourceHudKind::Fuel => {\n            Color::srgba(0.15, 0.14, 0.05, 0.94)\n        }\n        ResourceHudKind::Energy => {\n            Color::srgba(0.05, 0.14, 0.08, 0.94)\n        }\n    }\n}\n\nfn status_text_color(\n    kind: ResourceHudKind,\n    status: ResourceHudStatus,\n) -> Color {\n    match status {\n        ResourceHudStatus::Full\n        | ResourceHudStatus::Deficit => {\n            Color::srgb(1.0, 0.48, 0.42)\n        }\n        ResourceHudStatus::NearlyFull\n        | ResourceHudStatus::Reserved => {\n            Color::srgb(1.0, 0.78, 0.36)\n        }\n        ResourceHudStatus::Normal => {\n            resource_kind_color(kind)\n        }\n    }\n}\n\nfn status_gauge_color(\n    kind: ResourceHudKind,\n    status: ResourceHudStatus,\n) -> Color {\n    status_text_color(kind, status)\n}\n'
TESTS_CODE = '\n    #[test]\n    fn resource_fill_ratio_is_clamped() {\n        assert_eq!(resource_fill_ratio(0, 100), 0.0);\n        assert_eq!(resource_fill_ratio(50, 100), 0.5);\n        assert_eq!(resource_fill_ratio(150, 100), 1.0);\n        assert_eq!(resource_fill_ratio(0, 0), 0.0);\n        assert_eq!(resource_fill_ratio(1, 0), 1.0);\n    }\n\n    #[test]\n    fn resource_status_distinguishes_reservations_and_saturation() {\n        assert_eq!(\n            resource_hud_status(40, 0, 40, 100),\n            ResourceHudStatus::Reserved\n        );\n        assert_eq!(\n            resource_hud_status(90, 90, 0, 100),\n            ResourceHudStatus::NearlyFull\n        );\n        assert_eq!(\n            resource_hud_status(100, 100, 0, 100),\n            ResourceHudStatus::Full\n        );\n    }\n\n    #[test]\n    fn resource_delta_expires_without_mutating_simulation() {\n        let mut state = ResourceDeltaState::default();\n        state.show(ResourceStock::new(12, 6, 3));\n\n        assert_eq!(\n            state.active(),\n            ResourceStock::new(12, 6, 3)\n        );\n\n        state.tick(RESOURCE_DELTA_DISPLAY_SECONDS + 0.1);\n        assert_eq!(state.active(), ResourceStock::ZERO);\n    }\n\n    #[test]\n    fn dashboard_uses_the_selected_player_colony() {\n        let simulation =\n            Simulation::new(UniverseConfig::mvp());\n\n        let colony =\n            selected_colony_for_resource_dashboard(&simulation)\n                .expect("the home planet is selected initially");\n\n        assert_eq!(\n            colony.planet_id,\n            PlanetId::from_system_index(\n                MVP_HOME_SYSTEM_ID,\n                0,\n            )\n        );\n    }\n\n    #[test]\n    fn energy_deficit_uses_a_full_warning_gauge() {\n        assert_eq!(energy_fill_ratio(60, 40), 1.0);\n        assert_eq!(energy_fill_ratio(0, 0), 0.0);\n    }\n'
DOC_APPEND = '\n## MVP-013-B — Affichage des ressources et de l’énergie\n\nUn tableau économique compact complète l’inspecteur détaillé lorsqu’une\ncolonie du joueur est sélectionnée.\n\nLe tableau contient quatre cartes :\n\n```text\nMétal | Cristal | Carburant | Énergie\n```\n\nChaque ressource stockée affiche :\n\n- stock total et capacité ;\n- quantité disponible ;\n- quantité réservée ;\n- production effective par seconde ;\n- temps avant saturation ;\n- jauge de remplissage ;\n- delta du dernier crédit de production.\n\nLes états visuels sont :\n\n```text\nSTABLE\nINDISPONIBLE — RÉSERVÉ\nPRESQUE PLEIN\nPLEIN — PRODUCTION PERDUE\nDÉFICIT ÉNERGÉTIQUE\n```\n\nLa carte Énergie distingue production effective, consommation, capacité libre,\nbilan et rendement des extracteurs. L’énergie reste une capacité et n’est\njamais présentée comme un stock.\n\nL’en-tête affiche la colonie active, la prochaine actualisation des stocks et\nla cadence de cinq secondes stratégiques. Les deltas ne sont affichés qu’après\nun événement `ProductionRefreshed`, pendant une courte durée réelle.\n\nSurvoler une carte affiche une aide contextuelle expliquant stocks,\nréservations, capacité ou rendement énergétique.\n\nLe tableau est placé entre les panneaux latéraux et reste lisible en\n1280×720. Il est masqué lorsqu’aucune colonie du joueur n’est sélectionnée.\n\nCette étape est purement visuelle :\n\n- aucune modification du domaine ;\n- aucune nouvelle commande ;\n- aucune modification de sauvegarde ;\n- aucune modification des versions d’état ;\n- aucune modification de la cadence de simulation.\n\nMVP-014 pourra réutiliser les mêmes cartes pour signaler le coût et les\nressources manquantes d’une construction.\n'


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
                / "crates/galactic_sim/src/building_catalog.rs"
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
        "MVP-013 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")
    production = (
        root / "crates/galactic_sim/src/production.rs"
    ).read_text(encoding="utf-8")
    catalog = (
        root / "crates/galactic_sim/src/building_catalog.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "GameEvent::ProductionRefreshed",
        "fn colony_economy_text(",
        "fn spawn_ui(",
        "struct AmbiguityPanelText;",
    ):
        if marker not in client:
            failures.append(
                f"marqueur client absent : {marker}"
            )
    for marker in (
        "PRODUCTION_REFRESH_SECONDS",
        "ticks_until_refresh",
    ):
        if marker not in production:
            failures.append(
                f"marqueur production absent : {marker}"
            )
    if "pub struct BuildingCatalog" not in catalog:
        failures.append("catalogue de bâtiments absent")

    if failures:
        raise SystemExit(
            "Baseline MVP-013 incohérente :\n- "
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
    if "// MVP-013-B: compact resource dashboard" in source:
        return normalize(source)

    domain_import_pattern = re.compile(
        r"use galactic_domain::\{(?P<body>.*?)\};",
        flags=re.DOTALL,
    )
    match = domain_import_pattern.search(source)
    if match is None:
        raise SystemExit(
            "Bloc d'import galactic_domain introuvable."
        )
    body = match.group("body")
    if "ResourceStock" not in body:
        body = body.rstrip() + ", ResourceStock"
        source = (
            source[: match.start()]
            + "use galactic_domain::{"
            + body
            + "};"
            + source[match.end() :]
        )

    source = replace_once(
        source,
        "        .init_resource::<PointerSelectionState>()\n",
        "        .init_resource::<PointerSelectionState>()\n"
        "        .init_resource::<ResourceDeltaState>()\n",
        "ressource de delta",
    )

    source = replace_once(
        source,
        "                handle_pointer_selection,\n"
        "                collect_presentation_events,\n",
        "                handle_pointer_selection,\n"
        "                capture_resource_deltas,\n"
        "                collect_presentation_events,\n",
        "capture des deltas",
    )
    source = replace_once(
        source,
        "                update_ambiguity_panel,\n"
        "                update_ui,\n"
        "                update_info_panel,\n",
        "                update_ambiguity_panel,\n"
        "                update_resource_dashboard,\n"
        "                update_resource_card_help,\n"
        "                update_ui,\n"
        "                update_info_panel,\n",
        "systèmes du tableau économique",
    )

    source = replace_once(
        source,
        "#[derive(Component)]\n"
        "struct AmbiguityPanelText;\n",
        "#[derive(Component)]\n"
        "struct AmbiguityPanelText;\n\n"
        + TYPES_CODE.rstrip()
        + "\n",
        "types du tableau économique",
    )

    spawn_pattern = re.compile(
        r"fn spawn_ui\(.*?\n\}\n\n(?=fn spawn_panel_heading)",
        flags=re.DOTALL,
    )
    spawn_match = spawn_pattern.search(source)
    if spawn_match is None:
        raise SystemExit(
            "Fonction spawn_ui introuvable."
        )
    spawn_block = spawn_match.group(0)
    closing = spawn_block.rfind("\n}")
    if closing < 0:
        raise SystemExit(
            "Fin de spawn_ui introuvable."
        )
    spawn_block = (
        spawn_block[:closing]
        + "\n\n    spawn_resource_dashboard(&mut commands);"
        + spawn_block[closing:]
    )
    source = (
        source[: spawn_match.start()]
        + spawn_block
        + source[spawn_match.end() :]
    )

    source = replace_once(
        source,
        "\nfn spawn_panel_heading",
        "\n" + SPAWN_CODE.rstrip()
        + "\n\nfn spawn_panel_heading",
        "construction du tableau économique",
    )

    source = replace_once(
        source,
        "\nfn update_action_buttons(",
        "\n" + SYSTEMS_CODE.rstrip()
        + "\n\nfn update_action_buttons(",
        "mise à jour du tableau économique",
    )

    test_marker = (
        "    #[test]\n"
        "    fn ui_font_uses_a_system_sans_serif()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests UI introuvable."
        )
    source = source.replace(
        test_marker,
        TESTS_CODE.rstrip()
        + "\n\n"
        + test_marker,
        1,
    )

    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-013-B — Affichage des ressources et de l’énergie" in source:
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

    markers = (
        "ResourceDashboardRoot",
        "capture_resource_deltas",
        "update_resource_dashboard",
        "selected_colony_for_resource_dashboard",
        "Dernier crédit",
        "DÉFICIT ÉNERGÉTIQUE",
    )
    missing = [
        marker for marker in markers
        if marker not in client
    ]
    if missing:
        raise SystemExit(
            "Migration MVP-013-B incomplète :\n- "
            + "\n- ".join(missing)
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
        print("MVP-013-B est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp013b-backup"
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
        "\nMVP-013-B applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

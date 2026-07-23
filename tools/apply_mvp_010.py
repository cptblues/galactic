#!/usr/bin/env python3
"""
Applique MVP-010 au dépôt Galactic.

Baseline analysée :
    23d69b6239ee050c8462a9a330621e8c2a0753b6
    feat improve ui with panels

Le script :
- remplace la police Bevy embarquée par une police sans-serif système ;
- corrige l'affichage des accents français ;
- masque les informations non autorisées dans les inspecteurs ;
- distingue placeholders, estimations et valeurs exactes ;
- affiche l'action nécessaire pour progresser ;
- ajoute des styles visuels par niveau de connaissance ;
- documente l'absence actuelle de modèle métier pour les lunes.

Usage :
    python tools/apply_mvp_010.py --dry-run
    python tools/apply_mvp_010.py
    python tools/apply_mvp_010.py --skip-checks
    python tools/apply_mvp_010.py --root /chemin/vers/galactic

Le script est idempotent.
"""

from __future__ import annotations

import argparse
import difflib
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

EXPECTED_BASELINE_COMMIT = (
    "23d69b6239ee050c8462a9a330621e8c2a0753b6"
)

INSPECTOR_CODE = '// MVP-010: partial-information inspectors must never reveal hidden data.\n#[derive(Debug, Clone, PartialEq, Eq)]\nstruct InspectorContent {\n    level: Option<KnowledgeLevel>,\n    badge: String,\n    title: String,\n    body: String,\n    hint: String,\n}\n\nimpl InspectorContent {\n    fn render(&self) -> String {\n        format!(\n            "{}\\n{}\\n\\n{}\\n\\n{}",\n            self.badge, self.title, self.body, self.hint,\n        )\n    }\n}\n'
NEW_INSPECTOR_FUNCTIONS = 'fn update_info_panel(\n    simulation: Res<SimulationResource>,\n    navigation: Res<StrategicNavigation>,\n    mut query: Query<(&mut Text, &mut TextColor), With<InfoPanelText>>,\n) {\n    let Ok((mut text, mut color)) = query.single_mut() else {\n        return;\n    };\n    let content = information_panel_content(simulation.simulation(), &navigation);\n    text.0 = content.render();\n    color.0 = knowledge_color(content.level);\n}\n\nfn information_panel_content(\n    simulation: &Simulation,\n    navigation: &StrategicNavigation,\n) -> InspectorContent {\n    match simulation.state().selected {\n        SelectionTarget::System(system_id) => {\n            system_inspector_content(simulation, system_id)\n        }\n        SelectionTarget::Planet {\n            system_id,\n            planet_id,\n        } => planet_inspector_content(\n            simulation,\n            system_id,\n            planet_id,\n        ),\n        SelectionTarget::None => home_inspector_content(simulation),\n    }\n}\n\nfn home_inspector_content(simulation: &Simulation) -> InspectorContent {\n    let state = simulation.state();\n    let Some(faction) = state.player_faction_state() else {\n        return inspector_error("Faction joueur invalide");\n    };\n    let Some(colony) = state.player_home_colony() else {\n        return inspector_error("Colonie mère introuvable");\n    };\n    let Some(system) =\n        simulation.universe().system(colony.system_id)\n    else {\n        return inspector_error("Système mère introuvable");\n    };\n    let Some(planet) =\n        simulation.universe_repository().planet(colony.planet_id)\n    else {\n        return inspector_error("Planète mère introuvable");\n    };\n\n    InspectorContent {\n        level: Some(KnowledgeLevel::Colonized),\n        badge: knowledge_badge_fr(KnowledgeLevel::Colonized)\n            .to_string(),\n        title: format!("{} — {}", system.name, planet.name),\n        body: format!(\n            "Faction : {}\\nHabitabilité : {}%\\n\\nSTOCKS EXACTS\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nPOTENTIEL EXACT\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nINFRASTRUCTURE\\nMines : {}/{}/{}\\nCentrale : {}\\nEntrepôt : {}\\nConstruction : {}\\nLaboratoire : {}\\nChantier : {}",\n            faction.name,\n            planet.habitability,\n            colony.stock.metal,\n            colony.stock.crystal,\n            colony.stock.fuel,\n            colony.stock.energy,\n            colony.resource_profile.metal,\n            colony.resource_profile.crystal,\n            colony.resource_profile.fuel,\n            colony.resource_profile.energy,\n            colony.buildings.metal_mine,\n            colony.buildings.crystal_extractor,\n            colony.buildings.fuel_refinery,\n            colony.buildings.power_plant,\n            colony.buildings.warehouse,\n            colony.buildings.construction_center,\n            colony.buildings.research_lab,\n            colony.buildings.shipyard,\n        ),\n        hint: "Colonie active : les valeurs affichées sont exactes."\n            .to_string(),\n    }\n}\n\nfn system_inspector_content(\n    simulation: &Simulation,\n    system_id: SystemId,\n) -> InspectorContent {\n    let state = simulation.state();\n    let Some(system) = simulation.universe().system(system_id)\n    else {\n        return inspector_error(&format!(\n            "Référence système invalide : {}",\n            system_id.index(),\n        ));\n    };\n\n    let level = state.system_knowledge_level(system_id);\n    let visible_planets = system\n        .planets\n        .iter()\n        .filter(|planet| {\n            state.planet_knowledge_level(planet.id).is_visible()\n        })\n        .count();\n    let visible_routes = simulation\n        .universe_repository()\n        .neighboring_systems(system_id)\n        .into_iter()\n        .filter(|neighbor| state.is_system_visible(*neighbor))\n        .count();\n\n    let (title, body) = match level {\n        KnowledgeLevel::Unknown => (\n            "Système inconnu".to_string(),\n            "Identité : ???\\nClasse stellaire : ???\\nCorps célestes : ???\\nRoutes : ???\\nPosition : inconnue"\n                .to_string(),\n        ),\n        KnowledgeLevel::Detected => (\n            format!("Signal {}", system_id.index()),\n            "Identité : ???\\nClasse stellaire : ???\\nCorps célestes : non sondés\\nRoutes : signaux partiels\\nPosition : repérée sur la carte"\n                .to_string(),\n        ),\n        KnowledgeLevel::Probed => (\n            system.name.clone(),\n            format!(\n                "Classe stellaire : {:?}\\nLuminosité estimée : {}\\nCorps détectés : {}\\nRoutes cartographiées : {}\\nPosition estimée : x {:.0}  y {:.0}  z {:.0}",\n                system.star.class,\n                luminosity_estimate(system.star.luminosity),\n                visible_planets,\n                visible_routes,\n                approximate_position(system.position.x),\n                approximate_position(system.position.y),\n                approximate_position(system.position.z),\n            ),\n        ),\n        KnowledgeLevel::Analyzed | KnowledgeLevel::Colonized => (\n            system.name.clone(),\n            format!(\n                "Classe stellaire : {:?}\\nLuminosité exacte : {:.2}\\nCorps recensés : {}\\nRoutes cartographiées : {}\\nPosition exacte : x {:.1}  y {:.1}  z {:.1}",\n                system.star.class,\n                system.star.luminosity,\n                system.planets.len(),\n                visible_routes,\n                system.position.x,\n                system.position.y,\n                system.position.z,\n            ),\n        ),\n    };\n\n    InspectorContent {\n        level: Some(level),\n        badge: knowledge_badge_fr(level).to_string(),\n        title,\n        body,\n        hint: system_knowledge_hint(level).to_string(),\n    }\n}\n\nfn planet_inspector_content(\n    simulation: &Simulation,\n    selected_system_id: SystemId,\n    planet_id: galactic_domain::PlanetId,\n) -> InspectorContent {\n    let state = simulation.state();\n    let Some((system_id, planet)) =\n        simulation.universe_repository().planet_location(planet_id)\n    else {\n        return inspector_error(&format!(\n            "Référence planète invalide : {}",\n            planet_id.index(),\n        ));\n    };\n    let Some(system) = simulation.universe().system(system_id)\n    else {\n        return inspector_error("Système de la planète introuvable");\n    };\n\n    let level = state.planet_knowledge_level(planet_id);\n    let colony = state.colony_on_planet(planet_id);\n    let system_label =\n        if state.system_knowledge_level(system_id).reveals_identity() {\n            system.name.clone()\n        } else {\n            format!("Signal {}", system_id.index())\n        };\n    let selection_note = if selected_system_id == system_id {\n        "Sélection : cohérente"\n    } else {\n        "Sélection : recoupée avec le système réel"\n    };\n\n    let (title, mut body) = match level {\n        KnowledgeLevel::Unknown => (\n            "Corps inconnu".to_string(),\n            format!(\n                "Système : {}\\nNom : ???\\nType : ???\\nHabitabilité : ???\\nPotentiel : ???\\nLunes : ???\\n{}",\n                system_label, selection_note,\n            ),\n        ),\n        KnowledgeLevel::Detected => (\n            format!("Corps détecté {}", planet_id.index()),\n            format!(\n                "Système : {}\\nNom : ???\\nType : ???\\nHabitabilité : ???\\nPotentiel : analyse requise\\nLunes : non recensées\\n{}",\n                system_label, selection_note,\n            ),\n        ),\n        KnowledgeLevel::Probed => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité estimée : {}\\nPotentiel : analyse requise\\nLunes : non recensées\\n{}",\n                system_label,\n                planet.kind,\n                habitability_estimate(planet.habitability),\n                selection_note,\n            ),\n        ),\n        KnowledgeLevel::Analyzed => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité exacte : {}%\\nStatut : non colonisée\\nPotentiel : aucune valeur économique générée pour ce corps\\nLunes : aucune donnée disponible\\n{}",\n                system_label,\n                planet.kind,\n                planet.habitability,\n                selection_note,\n            ),\n        ),\n        KnowledgeLevel::Colonized => (\n            planet.name.clone(),\n            format!(\n                "Système : {}\\nType : {:?}\\nHabitabilité exacte : {}%\\nStatut : {}\\nLunes : aucune donnée disponible\\n{}",\n                system_label,\n                planet.kind,\n                planet.habitability,\n                colony\n                    .map(|value| value.name.as_str())\n                    .unwrap_or("colonie non référencée"),\n                selection_note,\n            ),\n        ),\n    };\n\n    if let Some(colony) = colony {\n        body.push_str(&format!(\n            "\\n\\nSTOCKS EXACTS\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nPOTENTIEL EXACT\\nMétal : {}\\nCristal : {}\\nCarburant : {}\\nÉnergie : {}\\n\\nINFRASTRUCTURE\\nMines : {}/{}/{}\\nCentrale : {}\\nEntrepôt : {}\\nConstruction : {}\\nLaboratoire : {}\\nChantier : {}",\n            colony.stock.metal,\n            colony.stock.crystal,\n            colony.stock.fuel,\n            colony.stock.energy,\n            colony.resource_profile.metal,\n            colony.resource_profile.crystal,\n            colony.resource_profile.fuel,\n            colony.resource_profile.energy,\n            colony.buildings.metal_mine,\n            colony.buildings.crystal_extractor,\n            colony.buildings.fuel_refinery,\n            colony.buildings.power_plant,\n            colony.buildings.warehouse,\n            colony.buildings.construction_center,\n            colony.buildings.research_lab,\n            colony.buildings.shipyard,\n        ));\n    }\n\n    InspectorContent {\n        level: Some(level),\n        badge: knowledge_badge_fr(level).to_string(),\n        title,\n        body,\n        hint: planet_knowledge_hint(level).to_string(),\n    }\n}\n\nfn inspector_error(message: &str) -> InspectorContent {\n    InspectorContent {\n        level: None,\n        badge: "[ERREUR D’INSPECTEUR]".to_string(),\n        title: "Donnée indisponible".to_string(),\n        body: message.to_string(),\n        hint: "La sélection ne correspond pas à une donnée valide."\n            .to_string(),\n    }\n}\n\nconst fn knowledge_badge_fr(level: KnowledgeLevel) -> &\'static str {\n    match level {\n        KnowledgeLevel::Unknown => "[INCONNU — DONNÉES MASQUÉES]",\n        KnowledgeLevel::Detected => {\n            "[DÉTECTÉ — DONNÉES MASQUÉES]"\n        }\n        KnowledgeLevel::Probed => "[SONDÉ — ESTIMATIONS]",\n        KnowledgeLevel::Analyzed => {\n            "[ANALYSÉ — VALEURS EXACTES]"\n        }\n        KnowledgeLevel::Colonized => {\n            "[COLONISÉ — VALEURS EXACTES]"\n        }\n    }\n}\n\nconst fn system_knowledge_hint(level: KnowledgeLevel) -> &\'static str {\n    match level {\n        KnowledgeLevel::Unknown => {\n            "Action requise : détecter le système."\n        }\n        KnowledgeLevel::Detected => {\n            "Action requise : sonder le système pour révéler son identité."\n        }\n        KnowledgeLevel::Probed => {\n            "Action requise : analyser le système pour obtenir les valeurs exactes."\n        }\n        KnowledgeLevel::Analyzed => {\n            "Analyse terminée : les valeurs disponibles sont exactes."\n        }\n        KnowledgeLevel::Colonized => {\n            "Système colonisé : les valeurs disponibles sont exactes."\n        }\n    }\n}\n\nconst fn planet_knowledge_hint(level: KnowledgeLevel) -> &\'static str {\n    match level {\n        KnowledgeLevel::Unknown => {\n            "Action requise : détecter ce corps céleste."\n        }\n        KnowledgeLevel::Detected => {\n            "Action requise : sonder la planète pour révéler son identité."\n        }\n        KnowledgeLevel::Probed => {\n            "Action requise : analyser la planète pour obtenir les valeurs exactes."\n        }\n        KnowledgeLevel::Analyzed => {\n            "Analyse terminée : les caractéristiques disponibles sont exactes."\n        }\n        KnowledgeLevel::Colonized => {\n            "Planète colonisée : les données économiques sont exactes."\n        }\n    }\n}\n\nfn knowledge_color(level: Option<KnowledgeLevel>) -> Color {\n    match level {\n        None | Some(KnowledgeLevel::Unknown) => {\n            Color::srgb(0.72, 0.76, 0.80)\n        }\n        Some(KnowledgeLevel::Detected) => {\n            Color::srgb(0.58, 0.72, 0.88)\n        }\n        Some(KnowledgeLevel::Probed) => {\n            Color::srgb(0.56, 0.88, 0.94)\n        }\n        Some(KnowledgeLevel::Analyzed) => {\n            Color::srgb(0.96, 0.82, 0.48)\n        }\n        Some(KnowledgeLevel::Colonized) => {\n            Color::srgb(0.58, 0.94, 0.72)\n        }\n    }\n}\n\nfn luminosity_estimate(luminosity: f32) -> &\'static str {\n    if luminosity < 0.6 {\n        "faible"\n    } else if luminosity < 1.6 {\n        "moyenne"\n    } else if luminosity < 2.6 {\n        "forte"\n    } else {\n        "très forte"\n    }\n}\n\nfn habitability_estimate(habitability: u8) -> &\'static str {\n    match habitability {\n        0..=19 => "très faible",\n        20..=39 => "faible",\n        40..=59 => "moyenne",\n        60..=79 => "bonne",\n        _ => "excellente",\n    }\n}\n\nfn approximate_position(value: f32) -> f32 {\n    (value / 5.0).round() * 5.0\n}\n'
TESTS = '\n    #[test]\n    fn ui_font_uses_a_system_sans_serif() {\n        assert!(matches!(\n            ui_text_font(14.0).font,\n            FontSource::SansSerif\n        ));\n    }\n\n    #[test]\n    fn detected_system_inspector_masks_secret_values() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let state = simulation.state();\n        let detected = state\n            .system_knowledge\n            .iter()\n            .find(|entry| entry.level == KnowledgeLevel::Detected)\n            .expect("the starting frontier contains a detected system")\n            .system_id;\n        let system = simulation\n            .universe()\n            .system(detected)\n            .expect("detected system exists");\n\n        let rendered =\n            system_inspector_content(&simulation, detected).render();\n\n        assert!(rendered.contains("DÉTECTÉ"));\n        assert!(rendered.contains("Identité : ???"));\n        assert!(rendered.contains("Classe stellaire : ???"));\n        assert!(!rendered.contains(&system.name));\n        assert!(!rendered.contains(&format!("{:?}", system.star.class)));\n        assert!(!rendered.contains(&format!(\n            "{:.1}",\n            system.position.x\n        )));\n    }\n\n    #[test]\n    fn system_inspector_distinguishes_estimates_and_exact_values() {\n        let mut simulation = Simulation::new(UniverseConfig::mvp());\n        let detected = simulation\n            .state()\n            .system_knowledge\n            .iter()\n            .find(|entry| entry.level == KnowledgeLevel::Detected)\n            .expect("the starting frontier contains a detected system")\n            .system_id;\n        simulation.apply_command(GameCommand::SelectSystem(detected));\n        simulation.apply_command(\n            GameCommand::DebugAdvanceSelectedKnowledge,\n        );\n\n        let probed =\n            system_inspector_content(&simulation, detected).render();\n        assert!(probed.contains("SONDÉ"));\n        assert!(probed.contains("Luminosité estimée"));\n\n        simulation.apply_command(\n            GameCommand::DebugAdvanceSelectedKnowledge,\n        );\n        let analyzed =\n            system_inspector_content(&simulation, detected).render();\n        let system = simulation\n            .universe()\n            .system(detected)\n            .expect("analyzed system exists");\n\n        assert!(analyzed.contains("ANALYSÉ"));\n        assert!(analyzed.contains("Luminosité exacte"));\n        assert!(analyzed.contains(&format!(\n            "{:.2}",\n            system.star.luminosity\n        )));\n    }\n\n    #[test]\n    fn detected_planet_inspector_hides_identity_and_habitability() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let detected = simulation\n            .state()\n            .planet_knowledge\n            .iter()\n            .find(|entry| entry.level == KnowledgeLevel::Detected)\n            .expect("the home system contains a detected planet")\n            .planet_id;\n        let (system_id, planet) = simulation\n            .universe_repository()\n            .planet_location(detected)\n            .expect("detected planet exists");\n\n        let rendered = planet_inspector_content(\n            &simulation,\n            system_id,\n            detected,\n        )\n        .render();\n\n        assert!(rendered.contains("DÉTECTÉ"));\n        assert!(rendered.contains("Nom : ???"));\n        assert!(rendered.contains("Habitabilité : ???"));\n        assert!(!rendered.contains(&planet.name));\n        assert!(!rendered.contains(&format!("{:?}", planet.kind)));\n    }\n'
DOC_APPEND = "\n## MVP-010 — Inspecteurs et informations partielles\n\nLe panneau d'informations est désormais piloté par un contenu d'inspection\nstructuré plutôt que par un simple dump des objets du domaine.\n\nMatrice d'affichage :\n\n```text\nUnknown    aucune donnée exploitable\nDetected   signal et placeholders\nProbed     identité et estimations\nAnalyzed   valeurs exactes disponibles\nColonized  valeurs exactes et données économiques\n```\n\nRègles :\n\n- un système détecté ne révèle ni son nom, ni sa classe, ni ses coordonnées\n  chiffrées, ni le nombre exact de routes ou de corps ;\n- un système sondé révèle son identité et des estimations clairement étiquetées ;\n- un système analysé révèle les valeurs exactes disponibles ;\n- une planète détectée masque nom, type et habitabilité ;\n- une planète sondée révèle son identité et une fourchette qualitative\n  d'habitabilité ;\n- une planète analysée révèle son habitabilité exacte ;\n- les stocks, potentiels et bâtiments ne sont affichés que pour une colonie ;\n- chaque niveau indique explicitement l'action nécessaire pour progresser ;\n- la couleur et le badge de l'inspecteur changent avec le niveau de connaissance ;\n- les données absentes du modèle courant, notamment les lunes, sont annoncées\n  comme indisponibles au lieu d'être inventées.\n\nLa police embarquée par défaut de Bevy est remplacée dans l'interface par\n`FontSource::SansSerif`, avec la fonctionnalité `system_font_discovery`.\nLes caractères français tels que `é`, `è`, `à`, `É` et `Métal` sont ainsi\nrendus par une police installée sur le système.\n\nCette étape ne modifie ni l'état de simulation, ni les versions de sauvegarde,\nni la génération de l'univers.\n"


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
            end="" if result.stdout.endswith("\n") else "\n",
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
                candidate / "crates/galactic_client/src/lib.rs"
            ).exists()
            and (
                candidate / "crates/galactic_sim/src/knowledge.rs"
            ).exists()
        ):
            return candidate

    raise SystemExit(
        "Racine Galactic introuvable. Utilise --root."
    )


def normalize(text: str) -> str:
    return text.rstrip() + "\n"


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
        "UI analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_mvp9_and_ui(root: Path) -> None:
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")
    knowledge = (
        root / "crates/galactic_sim/src/knowledge.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        "struct InfoPanelText;",
        "fn spawn_action_button(",
    ):
        if marker not in client:
            failures.append(
                f"marqueur UI absent : {marker}"
            )

    legacy_inspectors = (
        "fn system_panel_text(" in client
        and "fn planet_panel_text(" in client
    )
    mvp10_inspectors = (
        "fn system_inspector_content(" in client
        and "fn planet_inspector_content(" in client
    )
    if not legacy_inspectors and not mvp10_inspectors:
        failures.append(
            "inspecteurs système/planète absents"
        )

    if "pub enum KnowledgeLevel" not in knowledge:
        failures.append(
            "niveaux de connaissance MVP-009 absents"
        )

    if failures:
        raise SystemExit(
            "Baseline MVP-009/UI incohérente :\n- "
            + "\n- ".join(failures)
        )


def patch_cargo(source: str) -> str:
    if '"system_font_discovery"' in source:
        return normalize(source)

    marker = '    "default_font",\n'
    if marker not in source:
        raise SystemExit(
            "La feature Bevy default_font attendue est absente."
        )

    return normalize(
        source.replace(
            marker,
            marker + '    "system_font_discovery",\n',
            1,
        )
    )


def patch_client(source: str) -> str:
    if "// MVP-010: partial-information inspectors" in source:
        return normalize(source)

    if "use bevy::text::FontSource;" not in source:
        marker = "use bevy::prelude::*;\n"
        if marker not in source:
            raise SystemExit(
                "Import bevy::prelude attendu introuvable."
            )
        source = source.replace(
            marker,
            marker + "use bevy::text::FontSource;\n",
            1,
        )

    component_marker = (
        "#[derive(Component)]\n"
        "struct InfoPanelText;\n"
    )
    if component_marker not in source:
        raise SystemExit(
            "Composant InfoPanelText attendu introuvable."
        )
    source = source.replace(
        component_marker,
        component_marker + "\n" + INSPECTOR_CODE + "\n",
        1,
    )

    font_pattern = re.compile(
        r"TextFont\s*\{\s*"
        r"font_size:\s*FontSize::Px\(([^)]+)\),\s*"
        r"\.\.default\(\)\s*"
        r"\}",
        flags=re.MULTILINE,
    )
    source, font_count = font_pattern.subn(
        r"ui_text_font(\1)",
        source,
    )
    if font_count < 6:
        raise SystemExit(
            "Moins de six TextFont ont été convertis. "
            "Le client a probablement évolué."
        )

    helper_marker = "fn panel_background() -> Color {"
    if helper_marker not in source:
        raise SystemExit(
            "Point d'insertion ui_text_font introuvable."
        )
    source = source.replace(
        helper_marker,
        """fn ui_text_font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::SansSerif,
        font_size: FontSize::Px(size),
        ..default()
    }
}

"""
        + helper_marker,
        1,
    )

    inspector_block = re.search(
        r"fn update_info_panel\(.*?\nfn to_vec3",
        source,
        flags=re.DOTALL,
    )
    if inspector_block is None:
        raise SystemExit(
            "Bloc des inspecteurs actuel introuvable."
        )
    source = (
        source[: inspector_block.start()]
        + NEW_INSPECTOR_FUNCTIONS.rstrip()
        + "\n\nfn to_vec3"
        + source[inspector_block.end() :]
    )

    source = source.replace(
        '"Galactic MVP | preset {:?} | {} | tick {} | vitesse {} | cible {}\\nSystèmes {}/{} | Routes {}/{} | Connaissance D/P/A/C {}/{}/{}/{} | debug {} | {}",',
        '"Galactic MVP | preset {:?} | {} | tick {} | vitesse {} | cible {}\\nSystèmes {}/{} | Routes {}/{} | Détectés/Sondés/Analysés/Colonisés {}/{}/{}/{} | debug {} | {}",',
        1,
    )

    source = source.replace(
        'SelectionTarget::None => "none".to_string(),',
        'SelectionTarget::None => "aucune".to_string(),',
        1,
    )
    source = source.replace(
        'format!("system {}", system_id.index())',
        'format!("système {}", system_id.index())',
        1,
    )
    source = source.replace(
        'format!("planet {}:{}", system_id.index(), planet_id.index())',
        'format!("planète {}:{}", system_id.index(), planet_id.index())',
        1,
    )

    test_marker = (
        "    #[test]\n"
        "    fn semantic_lod_uses_stable_distance_bands()"
    )
    if test_marker not in source:
        raise SystemExit(
            "Point d'insertion des tests client introuvable."
        )
    source = source.replace(
        test_marker,
        TESTS.rstrip() + "\n\n" + test_marker,
        1,
    )

    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-010 — Inspecteurs et informations partielles" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []

    paths_and_patchers = [
        (root / "Cargo.toml", patch_cargo),
        (
            root / "crates/galactic_client/src/lib.rs",
            patch_client,
        ),
        (
            root / "docs/mvp_architecture.md",
            patch_docs,
        ),
    ]

    for path, patcher in paths_and_patchers:
        before = path.read_text(encoding="utf-8")
        after = patcher(before)
        if before != after:
            updates.append(Update(path, before, after))

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
        print("MVP-010 est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp010-backup"
        / datetime.now().strftime("%Y%m%d-%H%M%S")
    )
    for update in updates:
        relative = update.path.relative_to(root)
        backup = backup_root / relative
        backup.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(update.path, backup)
        update.path.write_text(
            update.after,
            encoding="utf-8",
        )
        print(f"+ updated: {relative}")

    print(f"Backup directory: {backup_root}")


def fontconfig_notice() -> None:
    if not sys.platform.startswith("linux"):
        return

    pkg_config = shutil.which("pkg-config")
    if pkg_config is None:
        print(
            "WARNING: pkg-config est absent. "
            "La découverte des polices système peut nécessiter "
            "fontconfig-devel sous Fedora."
        )
        return

    result = subprocess.run(
        [pkg_config, "--exists", "fontconfig"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        print(
            "WARNING: fontconfig n'est pas détecté par pkg-config.\n"
            "Sur Fedora, installe au besoin :\n"
            "  sudo dnf install fontconfig-devel"
        )


def checks(root: Path) -> None:
    fontconfig_notice()
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
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--skip-checks",
        action="store_true",
    )
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    root = find_root(args.root.resolve())
    print(f"Repository: {root}")
    verify_baseline(root, args.force)
    verify_mvp9_and_ui(root)

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
            end="" if status.endswith("\n") else "\n",
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
        "\nMVP-010 applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

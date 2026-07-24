#!/usr/bin/env python3
"""
Applique MVP-010-B au dépôt Galactic.

Baseline analysée :
    d05f6152d88ead99d01db04251fa0b8d58f7475e
    feat mvp 10 hide informations

Le script ajoute :
- picking en espace écran à rayon constant ;
- survol avec halo et tooltip privacy-safe ;
- clic gauche pour sélectionner ;
- double-clic pour ouvrir ou recentrer ;
- classement déterministe des cibles ;
- panneau de résolution des sélections ambiguës ;
- navigation Tab / Maj+Tab / Entrée / Échap dans les ambiguïtés ;
- préparation au futur mode galaxie aplati.

Usage :
    python tools/apply_mvp_010_b.py --dry-run
    python tools/apply_mvp_010_b.py
    python tools/apply_mvp_010_b.py --skip-checks
    python tools/apply_mvp_010_b.py --root /chemin/vers/galactic

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

EXPECTED_BASELINE_COMMIT = (
    "d05f6152d88ead99d01db04251fa0b8d58f7475e"
)

TYPES_CODE = '\n// MVP-010-B: screen-space picking uses displayed transforms, not domain positions.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\nenum PickTarget {\n    System(SystemId),\n    Planet {\n        system_id: SystemId,\n        planet_id: PlanetId,\n    },\n}\n\nimpl PickTarget {\n    const fn sort_key(self) -> (u8, u64, u64) {\n        match self {\n            Self::System(system_id) => (0, system_id.raw(), 0),\n            Self::Planet {\n                system_id,\n                planet_id,\n            } => (1, system_id.raw(), planet_id.raw()),\n        }\n    }\n}\n\n#[derive(Debug, Clone, Copy)]\nstruct PointerCandidate {\n    target: PickTarget,\n    screen_position: Vec2,\n    screen_distance: f32,\n    depth: f32,\n    priority: u8,\n}\n\n#[derive(Debug, Clone)]\nstruct AmbiguitySelection {\n    targets: Vec<PickTarget>,\n    active_index: usize,\n}\n\n#[derive(Debug, Clone, Copy)]\nstruct PointerClickRecord {\n    target: PickTarget,\n    at: Duration,\n    cursor_position: Vec2,\n}\n\n#[derive(Resource, Default)]\nstruct PointerSelectionState {\n    hovered: Option<PickTarget>,\n    hovered_screen_position: Option<Vec2>,\n    candidates: Vec<PointerCandidate>,\n    ambiguity: Option<AmbiguitySelection>,\n    last_click: Option<PointerClickRecord>,\n}\n\nimpl PointerSelectionState {\n    fn clear_hover(&mut self) {\n        self.hovered = None;\n        self.hovered_screen_position = None;\n        self.candidates.clear();\n    }\n\n    fn cycle_ambiguity(&mut self, reverse: bool) -> Option<PickTarget> {\n        let ambiguity = self.ambiguity.as_mut()?;\n        if ambiguity.targets.is_empty() {\n            return None;\n        }\n\n        ambiguity.active_index = if reverse {\n            ambiguity\n                .active_index\n                .checked_sub(1)\n                .unwrap_or(ambiguity.targets.len() - 1)\n        } else {\n            (ambiguity.active_index + 1) % ambiguity.targets.len()\n        };\n        ambiguity.targets.get(ambiguity.active_index).copied()\n    }\n}\n'
COMPONENTS_CODE = '\n#[derive(Component)]\nstruct SelectableVisual {\n    target: PickTarget,\n    pick_radius_px: f32,\n    priority: u8,\n}\n\n#[derive(Component)]\nstruct PointerHalo {\n    target: PickTarget,\n}\n\n#[derive(Component)]\nstruct UiPointerBlocker;\n\n#[derive(Component)]\nstruct PointerTooltipText;\n\n#[derive(Component)]\nstruct AmbiguityPanelText;\n'
POINTER_SYSTEMS_CODE = '\nfn update_pointer_candidates(\n    windows: Query<&Window, With<PrimaryWindow>>,\n    cameras: Query<(&Camera, &Transform), With<StrategicCamera>>,\n    targets: Query<(&SelectableVisual, &Transform)>,\n    blockers: Query<&Interaction, With<UiPointerBlocker>>,\n    simulation: Res<SimulationResource>,\n    mut pointer_state: ResMut<PointerSelectionState>,\n) {\n    let Ok(window) = windows.single() else {\n        pointer_state.clear_hover();\n        return;\n    };\n    let Some(cursor_position) = window.cursor_position() else {\n        pointer_state.clear_hover();\n        return;\n    };\n    if blockers\n        .iter()\n        .any(|interaction| *interaction != Interaction::None)\n    {\n        pointer_state.clear_hover();\n        return;\n    }\n\n    let Ok((camera, camera_transform)) = cameras.single() else {\n        pointer_state.clear_hover();\n        return;\n    };\n    let camera_global = GlobalTransform::from(*camera_transform);\n    let selected = simulation.simulation().state().selected;\n    let mut candidates = Vec::new();\n\n    for (selectable, visual_transform) in &targets {\n        if !pick_target_is_visible(\n            simulation.simulation(),\n            selectable.target,\n        ) {\n            continue;\n        }\n        let world_position = visual_transform.translation;\n        let Ok(screen_position) =\n            camera.world_to_viewport(&camera_global, world_position)\n        else {\n            continue;\n        };\n        let screen_distance = cursor_position.distance(screen_position);\n        if !screen_space_hit(\n            cursor_position,\n            screen_position,\n            selectable.pick_radius_px,\n        ) {\n            continue;\n        }\n\n        let selected_bonus = if pick_target_matches_selection(\n            selectable.target,\n            selected,\n        ) {\n            32\n        } else {\n            0\n        };\n        candidates.push(PointerCandidate {\n            target: selectable.target,\n            screen_position,\n            screen_distance,\n            depth: camera_transform\n                .translation\n                .distance(world_position),\n            priority: selectable.priority.saturating_add(selected_bonus),\n        });\n    }\n\n    rank_pointer_candidates(&mut candidates);\n    pointer_state.hovered = candidates.first().map(|candidate| candidate.target);\n    pointer_state.hovered_screen_position =\n        candidates.first().map(|candidate| candidate.screen_position);\n    pointer_state.candidates = candidates;\n}\n\nfn handle_pointer_selection(\n    mouse_buttons: Res<ButtonInput<MouseButton>>,\n    time: Res<Time>,\n    mut simulation: ResMut<SimulationResource>,\n    mut navigation: ResMut<StrategicNavigation>,\n    mut rebuild: ResMut<ViewRebuildRequest>,\n    mut pointer_state: ResMut<PointerSelectionState>,\n    targets: Query<(&SelectableVisual, &Transform)>,\n) {\n    if !mouse_buttons.just_pressed(MouseButton::Left) {\n        return;\n    }\n\n    let Some(primary) = pointer_state.candidates.first().copied() else {\n        pointer_state.ambiguity = None;\n        return;\n    };\n\n    let targets_under_pointer = pointer_state\n        .candidates\n        .iter()\n        .map(|candidate| candidate.target)\n        .collect::<Vec<_>>();\n    pointer_state.ambiguity = (targets_under_pointer.len() > 1).then_some(\n        AmbiguitySelection {\n            targets: targets_under_pointer,\n            active_index: 0,\n        },\n    );\n\n    select_pick_target(&mut simulation, primary.target);\n\n    let now = time.elapsed();\n    let is_double_click = pointer_state\n        .last_click\n        .is_some_and(|previous| {\n            pointer_double_click(\n                previous,\n                primary.target,\n                now,\n                primary.screen_position,\n            )\n        });\n    pointer_state.last_click = Some(PointerClickRecord {\n        target: primary.target,\n        at: now,\n        cursor_position: primary.screen_position,\n    });\n\n    if is_double_click {\n        activate_pick_target(\n            primary.target,\n            &mut simulation,\n            &mut navigation,\n            &mut rebuild,\n            &targets,\n        );\n        pointer_state.ambiguity = None;\n        pointer_state.last_click = None;\n    }\n}\n\nfn update_pointer_halos(\n    pointer_state: Res<PointerSelectionState>,\n    mut halos: Query<(&PointerHalo, &mut Visibility)>,\n) {\n    if !pointer_state.is_changed() {\n        return;\n    }\n\n    for (halo, mut visibility) in &mut halos {\n        *visibility = if Some(halo.target) == pointer_state.hovered {\n            Visibility::Visible\n        } else {\n            Visibility::Hidden\n        };\n    }\n}\n\nfn update_pointer_tooltip(\n    windows: Query<&Window, With<PrimaryWindow>>,\n    simulation: Res<SimulationResource>,\n    pointer_state: Res<PointerSelectionState>,\n    mut tooltips: Query<\n        (&mut Text, &mut Node, &mut Visibility),\n        With<PointerTooltipText>,\n    >,\n) {\n    let Ok((mut text, mut node, mut visibility)) = tooltips.single_mut() else {\n        return;\n    };\n    let Ok(window) = windows.single() else {\n        *visibility = Visibility::Hidden;\n        return;\n    };\n    let Some(target) = pointer_state.hovered else {\n        *visibility = Visibility::Hidden;\n        return;\n    };\n    let Some(screen_position) = pointer_state.hovered_screen_position else {\n        *visibility = Visibility::Hidden;\n        return;\n    };\n\n    text.0 = pointer_tooltip_text(simulation.simulation(), target);\n    node.left = Val::Px(\n        (screen_position.x + 18.0).clamp(8.0, (window.width() - 270.0).max(8.0)),\n    );\n    node.top = Val::Px(\n        (screen_position.y + 18.0).clamp(8.0, (window.height() - 110.0).max(8.0)),\n    );\n    *visibility = Visibility::Visible;\n}\n\nfn update_ambiguity_panel(\n    simulation: Res<SimulationResource>,\n    pointer_state: Res<PointerSelectionState>,\n    mut panels: Query<(&mut Text, &mut Visibility), With<AmbiguityPanelText>>,\n) {\n    let Ok((mut text, mut visibility)) = panels.single_mut() else {\n        return;\n    };\n    let Some(ambiguity) = pointer_state.ambiguity.as_ref() else {\n        *visibility = Visibility::Hidden;\n        return;\n    };\n\n    let mut lines = vec![\n        "PLUSIEURS CIBLES SOUS LE CURSEUR".to_string(),\n        "Tab / Maj+Tab : parcourir | Entrée : valider | Échap : fermer".to_string(),\n        String::new(),\n    ];\n    for (index, target) in ambiguity.targets.iter().enumerate() {\n        let marker = if index == ambiguity.active_index {\n            "▶"\n        } else {\n            " "\n        };\n        lines.push(format!(\n            "{} {}. {}",\n            marker,\n            index + 1,\n            pick_target_label(simulation.simulation(), *target),\n        ));\n    }\n\n    text.0 = lines.join("\\n");\n    *visibility = Visibility::Visible;\n}\n\nfn select_pick_target(\n    simulation: &mut SimulationResource,\n    target: PickTarget,\n) {\n    let command = match target {\n        PickTarget::System(system_id) => {\n            GameCommand::SelectSystem(system_id)\n        }\n        PickTarget::Planet {\n            system_id,\n            planet_id,\n        } => GameCommand::SelectPlanet {\n            system_id,\n            planet_id,\n        },\n    };\n    apply_simulation_command(simulation, command);\n}\n\nfn activate_pick_target(\n    target: PickTarget,\n    simulation: &mut SimulationResource,\n    navigation: &mut StrategicNavigation,\n    rebuild: &mut ViewRebuildRequest,\n    visuals: &Query<(&SelectableVisual, &Transform)>,\n) {\n    let visual_position = visuals\n        .iter()\n        .find_map(|(selectable, transform)| {\n            (selectable.target == target).then_some(transform.translation)\n        });\n\n    match target {\n        PickTarget::System(system_id) => {\n            if let Some(position) = visual_position\n                && matches!(navigation.mode, StrategicViewMode::Universe)\n            {\n                navigation.universe_focus = position;\n            }\n            if matches!(\n                navigation.mode,\n                StrategicViewMode::System(current) if current == system_id\n            ) {\n                navigation.system_focus = Vec3::ZERO;\n            }\n            if matches!(navigation.mode, StrategicViewMode::Universe)\n                && enterable_selected_system(\n                    simulation,\n                    navigation.debug_full_graph,\n                )\n                .is_some()\n            {\n                navigation.enter_system(system_id);\n                navigation.system_focus = Vec3::ZERO;\n                rebuild.0 = true;\n            }\n        }\n        PickTarget::Planet { system_id, .. } => {\n            if matches!(\n                navigation.mode,\n                StrategicViewMode::System(current) if current == system_id\n            ) && let Some(position) = visual_position\n            {\n                navigation.system_focus = position;\n            }\n        }\n    }\n}\n\nfn pointer_tooltip_text(\n    simulation: &Simulation,\n    target: PickTarget,\n) -> String {\n    let state = simulation.state();\n    match target {\n        PickTarget::System(system_id) => {\n            let level = state.system_knowledge_level(system_id);\n            let title = simulation\n                .universe()\n                .system(system_id)\n                .map(|system| {\n                    if level.reveals_identity() {\n                        system.name.clone()\n                    } else {\n                        format!("Signal {}", system_id.index())\n                    }\n                })\n                .unwrap_or_else(|| "Système invalide".to_string());\n            format!(\n                "{}\\n{}\\nClic : sélectionner | Double-clic : ouvrir ou recentrer",\n                title,\n                knowledge_badge_fr(level),\n            )\n        }\n        PickTarget::Planet { planet_id, .. } => {\n            let level = state.planet_knowledge_level(planet_id);\n            let title = simulation\n                .universe_repository()\n                .planet(planet_id)\n                .map(|planet| {\n                    if level.reveals_identity() {\n                        planet.name.clone()\n                    } else {\n                        format!("Corps détecté {}", planet_id.index())\n                    }\n                })\n                .unwrap_or_else(|| "Planète invalide".to_string());\n            format!(\n                "{}\\n{}\\nClic : sélectionner | Double-clic : recentrer",\n                title,\n                knowledge_badge_fr(level),\n            )\n        }\n    }\n}\n\nfn pick_target_label(\n    simulation: &Simulation,\n    target: PickTarget,\n) -> String {\n    let state = simulation.state();\n    match target {\n        PickTarget::System(system_id) => simulation\n            .universe()\n            .system(system_id)\n            .map(|system| {\n                if state\n                    .system_knowledge_level(system_id)\n                    .reveals_identity()\n                {\n                    format!("Système {}", system.name)\n                } else {\n                    format!("Signal {}", system_id.index())\n                }\n            })\n            .unwrap_or_else(|| format!("Système {}", system_id.index())),\n        PickTarget::Planet { planet_id, .. } => simulation\n            .universe_repository()\n            .planet(planet_id)\n            .map(|planet| {\n                if state\n                    .planet_knowledge_level(planet_id)\n                    .reveals_identity()\n                {\n                    format!("Planète {}", planet.name)\n                } else {\n                    format!("Corps détecté {}", planet_id.index())\n                }\n            })\n            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),\n    }\n}\n\nfn rank_pointer_candidates(candidates: &mut [PointerCandidate]) {\n    candidates.sort_by(|left, right| {\n        left.screen_distance\n            .total_cmp(&right.screen_distance)\n            .then_with(|| right.priority.cmp(&left.priority))\n            .then_with(|| left.depth.total_cmp(&right.depth))\n            .then_with(|| {\n                left.target.sort_key().cmp(&right.target.sort_key())\n            })\n    });\n}\n\nfn screen_space_hit(\n    cursor_position: Vec2,\n    target_position: Vec2,\n    radius_px: f32,\n) -> bool {\n    cursor_position.distance_squared(target_position)\n        <= radius_px * radius_px\n}\n\nfn pointer_double_click(\n    previous: PointerClickRecord,\n    target: PickTarget,\n    now: Duration,\n    cursor_position: Vec2,\n) -> bool {\n    previous.target == target\n        && now.saturating_sub(previous.at)\n            <= Duration::from_millis(350)\n        && previous.cursor_position.distance(cursor_position) <= 6.0\n}\n\nfn pick_target_is_visible(\n    simulation: &Simulation,\n    target: PickTarget,\n) -> bool {\n    match target {\n        PickTarget::System(system_id) => {\n            simulation.state().is_system_visible(system_id)\n        }\n        PickTarget::Planet { planet_id, .. } => simulation\n            .state()\n            .planet_knowledge_level(planet_id)\n            .is_visible(),\n    }\n}\n\nfn pick_target_matches_selection(\n    target: PickTarget,\n    selection: SelectionTarget,\n) -> bool {\n    match (target, selection) {\n        (\n            PickTarget::System(left),\n            SelectionTarget::System(right),\n        ) => left == right,\n        (\n            PickTarget::Planet {\n                system_id: left_system,\n                planet_id: left_planet,\n            },\n            SelectionTarget::Planet {\n                system_id: right_system,\n                planet_id: right_planet,\n            },\n        ) => left_system == right_system && left_planet == right_planet,\n        _ => false,\n    }\n}\n'
HELPER_CODE = '\nfn spawn_pointer_halo(\n    commands: &mut Commands,\n    assets: &VisualAssets,\n    target: PickTarget,\n    position: Vec3,\n    scale: f32,\n) {\n    commands.spawn((\n        Mesh3d(assets.system_mesh.clone()),\n        MeshMaterial3d(assets.hover_material.clone()),\n        Transform::from_translation(position)\n            .with_scale(Vec3::splat(scale)),\n        Visibility::Hidden,\n        PointerHalo { target },\n        StrategicViewEntity,\n    ));\n}\n\nfn system_pick_priority(\n    simulation: &Simulation,\n    system_id: SystemId,\n    visibility: SystemVisibility,\n) -> u8 {\n    if simulation\n        .state()\n        .colonies\n        .iter()\n        .any(|colony| colony.system_id == system_id)\n    {\n        120\n    } else if visibility == SystemVisibility::Known {\n        90\n    } else {\n        70\n    }\n}\n\nfn planet_pick_priority(\n    simulation: &Simulation,\n    planet_id: PlanetId,\n    level: KnowledgeLevel,\n) -> u8 {\n    if simulation.state().colony_on_planet(planet_id).is_some() {\n        120\n    } else {\n        match level {\n            KnowledgeLevel::Unknown => 0,\n            KnowledgeLevel::Detected => 70,\n            KnowledgeLevel::Probed => 85,\n            KnowledgeLevel::Analyzed => 95,\n            KnowledgeLevel::Colonized => 120,\n        }\n    }\n}\n'
TESTS_CODE = '\n    #[test]\n    fn screen_space_radius_is_constant_in_pixels() {\n        assert!(screen_space_hit(\n            Vec2::new(100.0, 100.0),\n            Vec2::new(116.0, 100.0),\n            16.0,\n        ));\n        assert!(!screen_space_hit(\n            Vec2::new(100.0, 100.0),\n            Vec2::new(117.0, 100.0),\n            16.0,\n        ));\n    }\n\n    #[test]\n    fn candidate_ranking_is_deterministic() {\n        let near_system = PickTarget::System(SystemId::new(2));\n        let priority_system = PickTarget::System(SystemId::new(1));\n        let deeper_planet = PickTarget::Planet {\n            system_id: SystemId::new(0),\n            planet_id: PlanetId::new(1),\n        };\n        let mut candidates = vec![\n            PointerCandidate {\n                target: deeper_planet,\n                screen_position: Vec2::ZERO,\n                screen_distance: 4.0,\n                depth: 20.0,\n                priority: 80,\n            },\n            PointerCandidate {\n                target: priority_system,\n                screen_position: Vec2::ZERO,\n                screen_distance: 4.0,\n                depth: 15.0,\n                priority: 100,\n            },\n            PointerCandidate {\n                target: near_system,\n                screen_position: Vec2::ZERO,\n                screen_distance: 2.0,\n                depth: 30.0,\n                priority: 10,\n            },\n        ];\n\n        rank_pointer_candidates(&mut candidates);\n\n        assert_eq!(candidates[0].target, near_system);\n        assert_eq!(candidates[1].target, priority_system);\n        assert_eq!(candidates[2].target, deeper_planet);\n    }\n\n    #[test]\n    fn ambiguity_cycle_wraps_in_both_directions() {\n        let first = PickTarget::System(SystemId::new(1));\n        let second = PickTarget::System(SystemId::new(2));\n        let mut pointer_state = PointerSelectionState {\n            ambiguity: Some(AmbiguitySelection {\n                targets: vec![first, second],\n                active_index: 0,\n            }),\n            ..default()\n        };\n\n        assert_eq!(\n            pointer_state.cycle_ambiguity(false),\n            Some(second)\n        );\n        assert_eq!(\n            pointer_state.cycle_ambiguity(false),\n            Some(first)\n        );\n        assert_eq!(\n            pointer_state.cycle_ambiguity(true),\n            Some(second)\n        );\n    }\n\n    #[test]\n    fn double_click_requires_same_target_time_and_position() {\n        let target = PickTarget::System(SystemId::new(3));\n        let previous = PointerClickRecord {\n            target,\n            at: Duration::from_millis(100),\n            cursor_position: Vec2::new(40.0, 50.0),\n        };\n\n        assert!(pointer_double_click(\n            previous,\n            target,\n            Duration::from_millis(400),\n            Vec2::new(44.0, 50.0),\n        ));\n        assert!(!pointer_double_click(\n            previous,\n            PickTarget::System(SystemId::new(4)),\n            Duration::from_millis(400),\n            Vec2::new(44.0, 50.0),\n        ));\n        assert!(!pointer_double_click(\n            previous,\n            target,\n            Duration::from_millis(500),\n            Vec2::new(44.0, 50.0),\n        ));\n    }\n\n    #[test]\n    fn unknown_targets_are_not_pickable_even_in_debug_rendering() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let unknown = simulation\n            .universe()\n            .systems\n            .iter()\n            .find(|system| {\n                !simulation.state().is_system_visible(system.id)\n            })\n            .expect("the MVP universe contains an unknown system")\n            .id;\n\n        assert!(!pick_target_is_visible(\n            &simulation,\n            PickTarget::System(unknown),\n        ));\n    }\n\n    #[test]\n    fn detected_pointer_labels_do_not_reveal_identity() {\n        let simulation = Simulation::new(UniverseConfig::mvp());\n        let detected = simulation\n            .state()\n            .system_knowledge\n            .iter()\n            .find(|entry| entry.level == KnowledgeLevel::Detected)\n            .expect("a detected frontier system exists")\n            .system_id;\n        let actual_name = &simulation\n            .universe()\n            .system(detected)\n            .expect("detected system exists")\n            .name;\n\n        let label = pick_target_label(\n            &simulation,\n            PickTarget::System(detected),\n        );\n\n        assert!(label.contains("Signal"));\n        assert!(!label.contains(actual_name));\n    }\n'
DOC_APPEND = "\n## MVP-010-B — Picking, survol et ambiguïtés\n\nLa sélection des objets stratégiques utilise un test en espace écran :\n\n```text\nposition visuelle actuelle\n        ↓ projection caméra\nposition en pixels\n        ↓ distance au curseur\ncandidats dans un rayon constant\n```\n\nLe picking s'appuie sur le `Transform` réellement affiché. Il ne lit pas\ndirectement la position métier de l'univers. Le futur mode aplati pourra donc\ndéplacer visuellement les systèmes sans désynchroniser la sélection.\n\nComportement :\n\n- clic gauche : sélectionner la meilleure cible ;\n- double-clic sur un système accessible : le sélectionner puis l'ouvrir ;\n- double-clic sur une planète : recentrer la caméra système ;\n- survol : afficher un halo et un tooltip respectant le niveau de connaissance ;\n- plusieurs cibles : ouvrir un panneau d'ambiguïté ;\n- `Tab` / `Maj+Tab` : parcourir les candidats ambigus ;\n- `Entrée` : conserver la cible active et fermer le panneau ;\n- `Échap` : fermer le panneau ;\n- les contrôles clavier historiques restent disponibles.\n\nClassement déterministe des candidats :\n\n1. distance en pixels au curseur ;\n2. priorité visuelle — sélection, colonie, objet connu ;\n3. profondeur par rapport à la caméra ;\n4. identifiant métier stable pour départager les égalités.\n\nLes systèmes utilisent un rayon de sélection de 18 pixels et les planètes un\nrayon de 16 pixels. Les objets inconnus ne sont jamais instanciés et ne peuvent\ndonc pas devenir candidats.\n\nLes panneaux UI marqués comme bloqueurs empêchent les clics de traverser\nl'interface vers la scène 3D.\n"


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
        "MVP-010 analysée.\n"
        f"HEAD={head}\n"
        f"Attendu={EXPECTED_BASELINE_COMMIT}\n"
        "Synchronise le dépôt ou utilise --force après "
        "vérification."
    )


def verify_current_state(root: Path) -> None:
    cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
    client = (
        root / "crates/galactic_client/src/lib.rs"
    ).read_text(encoding="utf-8")

    failures = []
    for marker in (
        '"mesh_picking"',
        '"system_font_discovery"',
    ):
        if marker not in cargo:
            failures.append(
                f"feature Bevy absente : {marker}"
            )
    for marker in (
        "struct InspectorContent",
        "fn information_panel_content(",
        'assert_eq!(label, "planète 2:1");',
        "fn spawn_action_button(",
        "fn update_strategic_camera(",
    ):
        if marker not in client:
            failures.append(
                f"marqueur client absent : {marker}"
            )

    if failures:
        raise SystemExit(
            "Baseline MVP-010 incohérente :\n- "
            + "\n- ".join(failures)
        )


def patch_client(source: str) -> str:
    if "// MVP-010-B: screen-space picking" in source:
        return normalize(source)

    source = replace_once(
        source,
        "use std::collections::HashMap;\n",
        "use std::{collections::HashMap, time::Duration};\n",
        "import Duration",
    )
    source = replace_once(
        source,
        "use bevy::window::PresentMode;\n",
        "use bevy::window::{PresentMode, PrimaryWindow};\n",
        "import PrimaryWindow",
    )
    source = replace_once(
        source,
        "use galactic_domain::{PlanetKind, StarClass, SystemId, UniverseConfig, WorldPosition};\n",
        "use galactic_domain::{\n"
        "    PlanetId, PlanetKind, StarClass, SystemId, "
        "UniverseConfig, WorldPosition,\n"
        "};\n",
        "import PlanetId",
    )

    source = replace_once(
        source,
        "        .init_resource::<ViewRebuildRequest>()\n",
        "        .init_resource::<ViewRebuildRequest>()\n"
        "        .init_resource::<PointerSelectionState>()\n",
        "resource PointerSelectionState",
    )

    old_schedule = """        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_strategic_camera,
                collect_presentation_events,
                update_system_visuals,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                update_action_buttons,
                update_ui,
                update_info_panel,
            ),
        );
"""
    new_schedule = """        .add_systems(
            Update,
            (
                rebuild_strategic_view_if_requested,
                update_strategic_camera,
                update_pointer_candidates,
                handle_pointer_selection,
                collect_presentation_events,
                update_system_visuals,
                update_pointer_halos,
                update_system_labels,
                draw_strategic_overlays,
                handle_action_buttons,
                update_action_buttons,
                update_pointer_tooltip,
                update_ambiguity_panel,
                update_ui,
                update_info_panel,
            )
                .chain(),
        );
"""
    source = replace_once(
        source,
        old_schedule,
        new_schedule,
        "ordre des systèmes de présentation",
    )

    source = replace_once(
        source,
        "    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,\n",
        "    planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,\n"
        "    hover_material: Handle<StandardMaterial>,\n",
        "matériau de halo",
    )
    source = replace_once(
        source,
        """        let planet_materials = PlanetKind::ALL
            .into_iter()
            .map(|kind| (kind, materials.add(planet_material(kind))))
            .collect();

        Self {
""",
        """        let planet_materials = PlanetKind::ALL
            .into_iter()
            .map(|kind| (kind, materials.add(planet_material(kind))))
            .collect();
        let hover_material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.28, 0.92, 0.82, 0.18),
            emissive: LinearRgba::rgb(0.18, 1.2, 0.92),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        Self {
""",
        "création du halo",
    )
    source = replace_once(
        source,
        """            detected_material,
            planet_materials,
        }
""",
        """            detected_material,
            planet_materials,
            hover_material,
        }
""",
        "stockage du halo",
    )

    source = replace_once(
        source,
        """#[derive(Component)]
struct InfoPanelText;
""",
        """#[derive(Component)]
struct InfoPanelText;
"""
        + COMPONENTS_CODE
        + "\n",
        "composants de picking",
    )
    source = replace_once(
        source,
        """impl InspectorContent {
    fn render(&self) -> String {
        format!(
            "{}\\n{}\\n\\n{}\\n\\n{}",
            self.badge, self.title, self.body, self.hint,
        )
    }
}
""",
        """impl InspectorContent {
    fn render(&self) -> String {
        format!(
            "{}\\n{}\\n\\n{}\\n\\n{}",
            self.badge, self.title, self.body, self.hint,
        )
    }
}
"""
        + TYPES_CODE
        + "\n",
        "types de picking",
    )

    universe_visual = """            SystemVisual {
                id: system.id,
                visibility,
                base_scale: scale,
            },
            StrategicViewEntity,
        ));

        let label = match visibility {
"""
    universe_visual_replacement = """            SystemVisual {
                id: system.id,
                visibility,
                base_scale: scale,
            },
            SelectableVisual {
                target: PickTarget::System(system.id),
                pick_radius_px: 18.0,
                priority: system_pick_priority(
                    simulation,
                    system.id,
                    visibility,
                ),
            },
            StrategicViewEntity,
        ));
        spawn_pointer_halo(
            commands,
            assets,
            PickTarget::System(system.id),
            position,
            scale.x * 1.65,
        );

        let label = match visibility {
"""
    source = replace_once(
        source,
        universe_visual,
        universe_visual_replacement,
        "cibles systèmes de l'univers",
    )

    source = replace_once(
        source,
        """    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };

    let star_material = assets
""",
        """    let Some(system) = simulation.universe().system(system_id) else {
        return;
    };
    let state = simulation.state();

    let star_material = assets
""",
        "état de la vue système",
    )
    source = replace_once(
        source,
        """        Transform::from_scale(Vec3::splat(2.8)),
        StrategicViewEntity,
    ));

    commands.spawn((
""",
        """        Transform::from_scale(Vec3::splat(2.8)),
        SelectableVisual {
            target: PickTarget::System(system_id),
            pick_radius_px: 20.0,
            priority: system_pick_priority(
                simulation,
                system_id,
                SystemVisibility::Known,
            ),
        },
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
""",
        "cible étoile centrale",
    )
    source = replace_once(
        source,
        """    let state = simulation.state();
    for (index, planet) in system.planets.iter().enumerate() {
""",
        """    for (index, planet) in system.planets.iter().enumerate() {
""",
        "état dupliqué de la vue système",
    )

    planet_visual = """            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            StrategicViewEntity,
        ));

        commands.spawn((
"""
    planet_visual_replacement = """            MeshMaterial3d(material),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            SelectableVisual {
                target: PickTarget::Planet {
                    system_id,
                    planet_id: planet.id,
                },
                pick_radius_px: if level.reveals_identity()
                    && planet.kind == PlanetKind::GasGiant
                {
                    18.0
                } else {
                    16.0
                },
                priority: planet_pick_priority(
                    simulation,
                    planet.id,
                    level,
                ),
            },
            StrategicViewEntity,
        ));
        spawn_pointer_halo(
            commands,
            assets,
            PickTarget::Planet {
                system_id,
                planet_id: planet.id,
            },
            position,
            scale * 1.65,
        );

        commands.spawn((
"""
    source = replace_once(
        source,
        planet_visual,
        planet_visual_replacement,
        "cibles planètes",
    )

    source = replace_once(
        source,
        "\nfn spawn_ui(mut commands: Commands) {\n",
        "\n" + HELPER_CODE.rstrip() + "\n\n"
        "fn spawn_ui(mut commands: Commands) {\n",
        "helpers visuels",
    )

    for description, old, new in (
        (
            "blocage barre supérieure",
            """        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        TopBarText,
""",
            """        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        Interaction::None,
        UiPointerBlocker,
        TopBarText,
""",
        ),
        (
            "blocage panneau commandes",
            """            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|parent| {
            spawn_panel_heading(parent, "COMMANDES");
""",
            """            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|parent| {
            spawn_panel_heading(parent, "COMMANDES");
""",
        ),
        (
            "blocage panneau informations",
            """            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|parent| {
            parent.spawn((
""",
            """            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            Interaction::None,
            UiPointerBlocker,
        ))
        .with_children(|parent| {
            parent.spawn((
""",
        ),
        (
            "blocage aide",
            """        Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.60, 0.50, 0.34, 0.35)),
        HelpText,
""",
            """        Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(0.60, 0.50, 0.34, 0.35)),
        Interaction::None,
        UiPointerBlocker,
        HelpText,
""",
        ),
    ):
        source = replace_once(source, old, new, description)

    source = replace_once(
        source,
        """            ActionButton { action },
        ))
""",
        """            ActionButton { action },
            UiPointerBlocker,
        ))
""",
        "boutons bloqueurs",
    )

    source = replace_once(
        source,
        '"AZERTY ZQSD navigation | A/E zoom | souris: droit orbite, milieu déplacement, molette zoom",',
        '"Clic sélectionner | Double-clic ouvrir/recentrer | Tab ambiguïtés | droit orbite | milieu déplacer | molette zoom",',
        "texte d'aide",
    )

    ui_end = """        HelpText,
    ));
}

fn spawn_panel_heading"""
    ui_additions = """        HelpText,
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
}

fn spawn_panel_heading"""
    source = replace_once(
        source,
        ui_end,
        ui_additions,
        "tooltip et panneau d'ambiguïté",
    )

    old_handle_view = """fn handle_view_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
) {
    if let Some(action) = view_shortcut(&keyboard) {
        apply_ui_action(action, &mut simulation, &mut navigation, &mut rebuild);
    }
}
"""
    new_handle_view = """fn handle_view_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut navigation: ResMut<StrategicNavigation>,
    mut rebuild: ResMut<ViewRebuildRequest>,
    mut pointer_state: ResMut<PointerSelectionState>,
) {
    if pointer_state.ambiguity.is_some() {
        if keyboard.just_pressed(KeyCode::Tab) {
            let reverse = keyboard.any_pressed([
                KeyCode::ShiftLeft,
                KeyCode::ShiftRight,
            ]);
            if let Some(target) =
                pointer_state.cycle_ambiguity(reverse)
            {
                select_pick_target(&mut simulation, target);
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
        apply_ui_action(action, &mut simulation, &mut navigation, &mut rebuild);
    }
}
"""
    source = replace_once(
        source,
        old_handle_view,
        new_handle_view,
        "navigation ambiguïtés",
    )

    source = replace_once(
        source,
        "\nfn update_action_buttons(\n",
        "\n" + POINTER_SYSTEMS_CODE.rstrip()
        + "\n\nfn update_action_buttons(\n",
        "systèmes de picking",
    )

    source = replace_once(
        source,
        """    #[test]
    fn ui_font_uses_a_system_sans_serif() {
""",
        TESTS_CODE.rstrip()
        + "\n\n    #[test]\n"
        "    fn ui_font_uses_a_system_sans_serif() {\n",
        "tests de picking",
    )

    return normalize(source)


def patch_docs(source: str) -> str:
    if "## MVP-010-B — Picking, survol et ambiguïtés" in source:
        return normalize(source)
    return normalize(source + "\n" + DOC_APPEND)


def collect_updates(root: Path) -> list[Update]:
    updates = []
    for path, patcher in (
        (
            root / "crates/galactic_client/src/lib.rs",
            patch_client,
        ),
        (
            root / "docs/mvp_architecture.md",
            patch_docs,
        ),
    ):
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
        print("MVP-010-B est déjà appliqué.")
        return
    if dry_run:
        for update in updates:
            show_diff(update, root)
        return

    backup_root = (
        root
        / ".mvp010b-backup"
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
        "\nMVP-010-B applied. Review with:\n"
        "  git diff\n"
        "  cargo run --release"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

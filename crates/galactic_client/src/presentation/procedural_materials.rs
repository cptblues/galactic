use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use galactic_domain::{PlanetKind, StarClass};
use galactic_sim::{
    ColonizationBlocker, GameEvent, GameEventKind, KnowledgeTarget, PlanetAnalysisError,
    SelectionTarget, TechnologyUnlock,
};

use crate::presentation::inspector_panel::*;
use crate::*;

pub(crate) fn star_material(class: StarClass) -> StandardMaterial {
    StandardMaterial {
        base_color: star_color(class),
        emissive: star_emissive(class),
        unlit: true,
        ..default()
    }
}

pub(crate) fn star_halo_material(class: StarClass) -> StandardMaterial {
    let color = star_color(class).with_alpha(0.18);
    StandardMaterial {
        base_color: color,
        emissive: star_emissive(class) * 0.22,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

pub(crate) fn planet_material(kind: PlanetKind, texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        perceptual_roughness: match kind {
            PlanetKind::Ocean => 0.56,
            PlanetKind::Ice => 0.48,
            PlanetKind::GasGiant => 0.72,
            PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => 0.88,
        },
        metallic: 0.0,
        ..default()
    }
}

pub(crate) fn atmosphere_material(kind: PlanetKind) -> StandardMaterial {
    let base_color = match kind {
        PlanetKind::Ocean => Color::srgba(0.20, 0.66, 1.0, 0.16),
        PlanetKind::Ice => Color::srgba(0.64, 0.86, 1.0, 0.12),
        PlanetKind::GasGiant => Color::srgba(0.84, 0.72, 0.94, 0.10),
        PlanetKind::Rocky | PlanetKind::Desert | PlanetKind::Volcanic => Color::NONE,
    };
    StandardMaterial {
        base_color,
        emissive: LinearRgba::from(base_color) * 0.34,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

pub(crate) fn procedural_planet_texture(kind: PlanetKind) -> Image {
    let mut texture =
        Vec::with_capacity((PLANET_TEXTURE_WIDTH * PLANET_TEXTURE_HEIGHT * 4) as usize);
    for y in 0..PLANET_TEXTURE_HEIGHT {
        for x in 0..PLANET_TEXTURE_WIDTH {
            texture.extend_from_slice(&procedural_planet_pixel(kind, x, y));
        }
    }

    Image::new_fill(
        Extent3d {
            width: PLANET_TEXTURE_WIDTH,
            height: PLANET_TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

pub(crate) fn procedural_planet_pixel(kind: PlanetKind, x: u32, y: u32) -> [u8; 4] {
    let noise = visual_hash(x, y, planet_kind_seed(kind));
    let color = match kind {
        PlanetKind::Rocky => {
            if noise > 205 {
                [126, 112, 94]
            } else if noise > 92 {
                [92, 82, 72]
            } else {
                [58, 54, 52]
            }
        }
        PlanetKind::Ocean => {
            let polar_cap = y < 2 || y + 2 >= PLANET_TEXTURE_HEIGHT;
            let land = noise > 208 && (x + y * 3).is_multiple_of(2);
            if polar_cap {
                [220, 239, 246]
            } else if land {
                [54, 126, 88]
            } else if noise > 118 {
                [26, 104, 166]
            } else {
                [15, 61, 126]
            }
        }
        PlanetKind::Desert => {
            if (y + x / 9).is_multiple_of(7) {
                [218, 160, 72]
            } else if noise > 148 {
                [184, 118, 51]
            } else {
                [128, 74, 38]
            }
        }
        PlanetKind::Ice => {
            let ridge = (x * 3 + y * 5 + u32::from(noise)).is_multiple_of(19);
            if ridge {
                [118, 178, 205]
            } else if noise > 128 {
                [218, 239, 244]
            } else {
                [164, 207, 224]
            }
        }
        PlanetKind::GasGiant => {
            let storm_x = x.abs_diff(46);
            let storm_y = y.abs_diff(20);
            if storm_x * storm_x + storm_y * storm_y < 18 {
                [184, 106, 76]
            } else {
                match (y / 3 + u32::from(noise > 190)) % 4 {
                    0 => [188, 154, 176],
                    1 => [126, 104, 148],
                    2 => [218, 184, 150],
                    _ => [154, 124, 166],
                }
            }
        }
        PlanetKind::Volcanic => {
            let lava = (x * 7 + y * 11 + u32::from(noise)).is_multiple_of(17);
            if lava {
                [246, 101, 24]
            } else if noise > 150 {
                [105, 35, 25]
            } else {
                [40, 28, 28]
            }
        }
    };
    [color[0], color[1], color[2], 255]
}

pub(crate) fn visual_hash(x: u32, y: u32, seed: u32) -> u8 {
    let mut value = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed.wrapping_mul(0xC2B2_AE3D));
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    (value >> 24) as u8
}

pub(crate) const fn planet_kind_seed(kind: PlanetKind) -> u32 {
    match kind {
        PlanetKind::Rocky => 1,
        PlanetKind::Ocean => 2,
        PlanetKind::Desert => 3,
        PlanetKind::Ice => 4,
        PlanetKind::GasGiant => 5,
        PlanetKind::Volcanic => 6,
    }
}

pub(crate) fn star_color(class: StarClass) -> Color {
    match class {
        StarClass::Blue => Color::srgb(0.42, 0.66, 1.0),
        StarClass::White => Color::srgb(0.92, 0.96, 1.0),
        StarClass::Yellow => Color::srgb(1.0, 0.86, 0.44),
        StarClass::Orange => Color::srgb(1.0, 0.58, 0.28),
        StarClass::Red => Color::srgb(0.95, 0.28, 0.24),
    }
}

pub(crate) fn star_emissive(class: StarClass) -> LinearRgba {
    match class {
        StarClass::Blue => LinearRgba::rgb(1.2, 2.4, 5.0),
        StarClass::White => LinearRgba::rgb(2.6, 2.8, 3.0),
        StarClass::Yellow => LinearRgba::rgb(2.8, 2.1, 0.8),
        StarClass::Orange => LinearRgba::rgb(2.6, 1.2, 0.45),
        StarClass::Red => LinearRgba::rgb(2.2, 0.45, 0.35),
    }
}

pub(crate) fn selection_label(selection: SelectionTarget) -> String {
    match selection {
        SelectionTarget::None => "aucune".to_string(),
        SelectionTarget::System(system_id) => {
            format!("système {}", system_id.index())
        }
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => format!("planète {}:{}", system_id.index(), planet_id.index()),
    }
}

pub(crate) fn event_label(event: GameEvent) -> String {
    match event.kind {
        GameEventKind::CommandRejected(error) => format!("commande refusée : {:?}", error),
        GameEventKind::SpeedChanged(speed) => format!("speed {}", speed),
        GameEventKind::SelectionChanged(selection) => {
            format!("selection {}", selection_label(selection))
        }
        GameEventKind::ActiveColonyChanged(colony_id) => {
            format!("colonie active : C{}", colony_id.raw())
        }
        GameEventKind::ActiveColonySelectionRejected(rejected) => format!(
            "sélection de la colonie C{} refusée : {:?}",
            rejected.colony_id.raw(),
            rejected.error,
        ),
        GameEventKind::KnowledgeChanged(change) => {
            let target = match change.target {
                KnowledgeTarget::System(id) => {
                    format!("system {}", id.index())
                }
                KnowledgeTarget::Planet(id) => {
                    format!("planet {}", id.index())
                }
            };
            format!("{} {} -> {}", target, change.previous, change.current)
        }
        GameEventKind::PlanetAnalyzed(report) => format!(
            "analyse terminée : planète {} caractérisée, colonisabilité évaluée",
            report.planet_id.index(),
        ),
        GameEventKind::PlanetAnalysisRejected(rejected) => {
            format!(
                "analyse refusée : {}",
                planet_analysis_error_text(rejected.error)
            )
        }
        GameEventKind::TicksAdvanced {
            ticks,
            current_tick,
        } => format!("+{} ticks -> {}", ticks.ticks(), current_tick),
        GameEventKind::ProductionRefreshed(_) => "production actualisée".to_string(),
        GameEventKind::ConstructionQueued(queued) => format!(
            "construction {:?} niveau {} ajoutée ({})",
            queued.order.kind, queued.order.target_level, queued.queue_length,
        ),
        GameEventKind::ConstructionCompleted(done) => format!(
            "construction {:?} niveau {} terminée",
            done.kind, done.new_level,
        ),
        GameEventKind::ConstructionRejected(rejected) => format!(
            "construction {:?} refusée : {:?}",
            rejected.kind, rejected.error,
        ),
        GameEventKind::ResearchQueued(queued) => format!(
            "recherche {:?} ajoutée ({})",
            queued.project.technology, queued.queue_length,
        ),
        GameEventKind::ResearchCompleted(completed) => {
            format!("recherche {:?} terminée", completed.technology,)
        }
        GameEventKind::ResearchRejected(rejected) => format!(
            "recherche {:?} refusée : {:?}",
            rejected.technology, rejected.error,
        ),
        GameEventKind::CraftQueued(queued) => format!(
            "craft {:?} ajouté ({})",
            queued.order.craftable, queued.queue_length,
        ),
        GameEventKind::CraftCompleted(completed) => format!(
            "craft {:?} terminé (stock {})",
            completed.craftable, completed.inventory_quantity,
        ),
        GameEventKind::CraftRejected(rejected) => format!(
            "craft {:?} refusé : {:?}",
            rejected.craftable, rejected.error,
        ),
        GameEventKind::FleetCreated(created) => {
            format!("flotte {:?} formée", created.fleet_id)
        }
        GameEventKind::FleetCreationRejected(rejected) => {
            format!("formation de flotte refusée : {:?}", rejected.error)
        }
        GameEventKind::MissionLaunched(launched) => format!(
            "mission {:?} lancée vers {:?}",
            launched.kind, launched.target,
        ),
        GameEventKind::MissionLaunchRejected(rejected) => {
            format!("mission refusée : {}", mission_error_text(rejected.error))
        }
        GameEventKind::MissionTransitioned(transition) => format!(
            "mission {:?} : {:?} -> {:?}",
            transition.mission_id, transition.from, transition.to,
        ),
        GameEventKind::MissionResolved(resolution) => mission_result_text(resolution.result),
        GameEventKind::ColonyFoundationPrepared(foundation) => format!(
            "fondation coloniale préparée sur le corps {} au tick {}",
            foundation.planet_id.index(),
            foundation.prepared_at.value(),
        ),
        GameEventKind::ColonyEstablished(colony) => format!(
            "nouvelle colonie établie sur le corps {} : colonie {} opérationnelle",
            colony.planet_id.index(),
            colony.colony_id.raw(),
        ),
        GameEventKind::MissionReported(report) => format!(
            "rapport mission {:?} : {:?}",
            report.mission_id, report.outcome,
        ),
        GameEventKind::MissionCancellationRejected(rejected) => {
            format!("annulation de mission refusée : {:?}", rejected.error)
        }
    }
}

pub(crate) fn planet_analysis_error_text(error: PlanetAnalysisError) -> String {
    match error {
        PlanetAnalysisError::Access(_) => {
            "la faction active ne peut pas effectuer cette analyse".to_string()
        }
        PlanetAnalysisError::UnknownPlanet(_) => "la planète sélectionnée est inconnue".to_string(),
        PlanetAnalysisError::PlanetNotProbed { .. } => {
            "la planète doit d'abord être identifiée par une Sonde Luciole".to_string()
        }
        PlanetAnalysisError::MissingTechnology(TechnologyUnlock::AnalyzePlanets) => {
            "recherchez Spectrométrie planétaire avant de lancer l'analyse".to_string()
        }
        PlanetAnalysisError::MissingTechnology(unlock) => {
            format!("technologie d'analyse manquante : {unlock:?}")
        }
        PlanetAnalysisError::AlreadyAnalyzed(_) => {
            "cette planète possède déjà un rapport d'analyse exact".to_string()
        }
    }
}

pub(crate) fn colonization_arrival_failure_label(blocker: ColonizationBlocker) -> &'static str {
    match blocker {
        ColonizationBlocker::UnknownPlanet(_) => "planète inconnue",
        ColonizationBlocker::NotAnalyzed { .. } | ColonizationBlocker::MissingAnalysisReport => {
            "analyse planétaire invalide"
        }
        ColonizationBlocker::AlreadyColonized => "planète déjà colonisée",
        ColonizationBlocker::FoundationAlreadyPrepared => "fondation déjà préparée",
        ColonizationBlocker::OccupiedPlanet { .. } => "présence étrangère détectée",
        ColonizationBlocker::MissingTechnology(_) => "technologie de colonisation manquante",
        ColonizationBlocker::UnsupportedEnvironment(_) => "environnement non compatible",
        ColonizationBlocker::HabitabilityTooLow { .. } => "habitabilité insuffisante",
        ColonizationBlocker::NoAccessibleRoute => "route devenue inaccessible",
        ColonizationBlocker::ColonyLimitReached { .. } => "limite de colonies atteinte",
        ColonizationBlocker::InsufficientFoundationResources { .. } => {
            "chargement de fondation indisponible"
        }
    }
}

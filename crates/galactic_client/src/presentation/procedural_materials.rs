use bevy::prelude::*;
use galactic_domain::{PlanetKind, StarClass};
use galactic_sim::{
    ColonizationBlocker, GameEvent, GameEventKind, KnowledgeTarget, PlanetAnalysisError,
    SelectionTarget, TechnologyUnlock,
};

use crate::presentation::inspector_panel::*;

#[cfg(test)]
use crate::presentation::graphics_settings::GraphicsPreset;
#[cfg(test)]
use bevy::asset::RenderAssetUsages;
#[cfg(test)]
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// `Low` quarters the pixel count of the pre-preset default; `High`
/// quadruples it. Threaded as parameters (not consts) rather than a global
/// resource read deep in pixel-generation code, so `procedural_planet_pixel`
/// stays a pure function.
#[cfg(test)]
pub(crate) const fn planet_texture_dimensions(preset: GraphicsPreset) -> (u32, u32) {
    match preset {
        GraphicsPreset::Low => (64, 32),
        GraphicsPreset::Medium => (128, 64),
        GraphicsPreset::High => (256, 128),
    }
}

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

pub(crate) fn territory_tint_material(
    tint: crate::presentation::territory::TerritoryTint,
) -> StandardMaterial {
    use crate::presentation::territory::TerritoryTint;
    let color = match tint {
        TerritoryTint::SelfOwned => Color::srgba(0.32, 0.94, 0.48, 0.42),
        TerritoryTint::Allied => Color::srgba(0.36, 0.62, 0.98, 0.42),
        TerritoryTint::Hostile => Color::srgba(0.98, 0.30, 0.30, 0.42),
        TerritoryTint::Neutral => Color::srgba(0.58, 0.60, 0.64, 0.32),
        TerritoryTint::UnidentifiedPresence => Color::srgba(0.62, 0.40, 0.82, 0.42),
    };
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 0.9,
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

#[cfg(test)]
pub(crate) fn procedural_planet_texture(kind: PlanetKind, preset: GraphicsPreset) -> Image {
    let (width, height) = planet_texture_dimensions(preset);
    let mut texture = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            texture.extend_from_slice(&procedural_planet_pixel(kind, x, y, width, height));
        }
    }

    Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
pub(crate) fn procedural_planet_pixel(
    kind: PlanetKind,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> [u8; 4] {
    let noise = layered_noise(x, y, planet_kind_seed(kind));
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
            let polar_band = (height / 16).max(1);
            let polar_cap = y < polar_band || y + polar_band >= height;
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
            // Storm position/size scaled from the texture dimensions instead of
            // hardcoded pixel offsets, so it stays proportionally placed if the
            // resolution changes again later.
            let storm_center_x = width * 72 / 100;
            let storm_center_y = height * 62 / 100;
            let storm_radius = (width.min(height) * 7 / 100).max(2);
            let storm_x = x.abs_diff(storm_center_x);
            let storm_y = y.abs_diff(storm_center_y);
            if storm_x * storm_x + storm_y * storm_y < storm_radius * storm_radius {
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
    let lit = day_night_shading(x, width);
    let shaded = [
        ((color[0] as u32 * lit as u32) / 255) as u8,
        ((color[1] as u32 * lit as u32) / 255) as u8,
        ((color[2] as u32 * lit as u32) / 255) as u8,
    ];
    [shaded[0], shaded[1], shaded[2], 255]
}

/// Blends three octaves of `visual_hash` at different spatial frequencies
/// (coarse/mid/fine, weighted toward the coarse octave) to approximate value
/// noise with spatial continuity, instead of a single-frequency hash that
/// produces uncorrelated per-pixel static. Stays fully deterministic: same
/// `(x, y, seed)` always yields the same result, no external noise crate.
#[cfg(test)]
pub(crate) fn layered_noise(x: u32, y: u32, seed: u32) -> u8 {
    let coarse = visual_hash(x / 6, y / 6, seed) as u32;
    let mid = visual_hash(x / 2, y / 2, seed.wrapping_add(11)) as u32;
    let fine = visual_hash(x, y, seed.wrapping_add(29)) as u32;
    ((coarse * 5 + mid * 3 + fine * 2) / 10) as u8
}

/// Soft brightness falloff across the texture's `x` axis, suggesting a lit
/// hemisphere and a shadowed one rather than flat, uniformly-lit color bands.
/// Cheap approximation (linear ramp within a fixed-width terminator band), not
/// a physical lighting model.
#[cfg(test)]
pub(crate) fn day_night_shading(x: u32, width: u32) -> u8 {
    const NIGHT_FLOOR: u32 = 70;
    let width = width.max(1);
    let position = x * 255 / width;
    let terminator = 150u32;
    let band = 60u32;
    let day_edge = terminator.saturating_sub(band);
    let night_edge = terminator + band;
    if position <= day_edge {
        255
    } else if position >= night_edge {
        NIGHT_FLOOR as u8
    } else {
        let t = position - day_edge;
        (255 - (t * (255 - NIGHT_FLOOR) / (band * 2))) as u8
    }
}

#[cfg(test)]
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

#[cfg(test)]
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
        GameEventKind::ConstructionCancelled(cancelled) => format!(
            "construction {:?} annulée (remboursé {:?})",
            cancelled.kind, cancelled.refunded,
        ),
        GameEventKind::ConstructionCancellationRejected(rejected) => {
            format!("annulation de construction refusée : {:?}", rejected.error,)
        }
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
        GameEventKind::ResearchCancelled(cancelled) => format!(
            "recherche {:?} annulée ({} points perdus)",
            cancelled.technology, cancelled.accumulated_milli_points,
        ),
        GameEventKind::ResearchCancellationRejected(rejected) => {
            format!("annulation de recherche refusée : {:?}", rejected.error,)
        }
        GameEventKind::CraftQueued(queued) => format!(
            "craft {:?} ajouté ({} unité(s), {})",
            queued.craftable, queued.quantity_requested, queued.queue_length,
        ),
        GameEventKind::CraftCompleted(completed) => format!(
            "craft {:?} terminé ({}/{}, stock {})",
            completed.craftable,
            completed.quantity_completed,
            completed.quantity_completed + completed.quantity_remaining,
            completed.inventory_quantity,
        ),
        GameEventKind::CraftRejected(rejected) => format!(
            "craft {:?} refusé : {:?}",
            rejected.craftable, rejected.error,
        ),
        GameEventKind::CraftCancelled(cancelled) => format!(
            "craft {:?} annulé ({} conservée(s), {} remboursée(s))",
            cancelled.craftable, cancelled.quantity_completed, cancelled.quantity_refunded,
        ),
        GameEventKind::CraftCancellationRejected(rejected) => {
            format!("annulation de craft refusée : {:?}", rejected.error,)
        }
        GameEventKind::FleetCreated(created) => {
            format!("flotte {:?} formée", created.fleet_id)
        }
        GameEventKind::FleetCreationRejected(rejected) => {
            format!("formation de flotte refusée : {:?}", rejected.error)
        }
        GameEventKind::FleetRenamed(renamed) => {
            format!("flotte {:?} renommée", renamed.fleet_id)
        }
        GameEventKind::FleetRenameRejected(rejected) => {
            format!("renommage de flotte refusé : {:?}", rejected.error)
        }
        GameEventKind::FleetDisbanded(disbanded) => {
            format!("flotte {:?} dissoute", disbanded.fleet_id)
        }
        GameEventKind::FleetDisbandRejected(rejected) => {
            format!("dissolution de flotte refusée : {:?}", rejected.error)
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
        GameEventKind::CombatDecisionRequired(pending) => format!(
            "combat en attente de décision : mission {:?}, round {}",
            pending.mission_id, pending.round,
        ),
        GameEventKind::CombatPlanConfirmed(confirmed) => {
            format!(
                "plan de combat confirmé : mission {:?}",
                confirmed.mission_id
            )
        }
        GameEventKind::CombatPlanRejected(rejected) => format!(
            "plan de combat refusé : mission {:?} : {:?}",
            rejected.mission_id, rejected.error,
        ),
        GameEventKind::CombatRoundResolved(resolved) => format!(
            "round {} résolu : mission {:?}",
            resolved.round, resolved.mission_id,
        ),
        GameEventKind::CombatIntelUpdated(updated) => format!(
            "renseignement mis à jour : mission {:?}, {} %",
            updated.mission_id, updated.intel_percent,
        ),
        GameEventKind::CombatCompleted(completed) => format!(
            "combat terminé : mission {:?}, {:?}",
            completed.mission_id, completed.result.outcome,
        ),
        GameEventKind::CombatDoctrineRejected(rejected) => format!(
            "choix de doctrine refusé : mission {:?} : {:?}",
            rejected.mission_id, rejected.error,
        ),
        GameEventKind::CombatRetreatRejected(rejected) => format!(
            "retraite refusée : mission {:?} : {:?}",
            rejected.mission_id, rejected.error,
        ),
        GameEventKind::CombatAutoResolveRejected(rejected) => format!(
            "auto-résolution refusée : mission {:?} : {:?}",
            rejected.mission_id, rejected.error,
        ),
    }
}

pub(crate) fn planet_analysis_error_text(error: PlanetAnalysisError) -> String {
    match error {
        PlanetAnalysisError::Access(_) => {
            "la faction active ne peut pas effectuer cette analyse".to_string()
        }
        PlanetAnalysisError::UnknownPlanet(_) => "la planète sélectionnée est inconnue".to_string(),
        PlanetAnalysisError::PlanetNotProbed { .. } => format!(
            "la planète doit d'abord être identifiée par une {}",
            galactic_sim::craftable_definition(galactic_sim::CraftableId::LIGHT_PROBE).name
        ),
        PlanetAnalysisError::MissingTechnology(TechnologyUnlock::AnalyzePlanets) => format!(
            "recherchez {} avant de lancer l'analyse",
            galactic_sim::technology_definition(galactic_sim::TechnologyId::PLANETARY_ANALYSIS)
                .name
        ),
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

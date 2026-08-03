use bevy::prelude::*;
use galactic_domain::ResourceStock;

use crate::presentation::components::{ResourceHudKind, ResourceHudStatus};
use crate::presentation::inspector_panel::{format_saturation_time, format_strategic_duration};

pub(crate) struct ResourceHudView {
    pub(crate) text: String,
    pub(crate) fill_ratio: f32,
    pub(crate) status: ResourceHudStatus,
}

pub(crate) fn resource_hud_view(
    kind: ResourceHudKind,
    colony: &galactic_sim::ColonyState,
    production: galactic_sim::ColonyProductionSnapshot,
) -> ResourceHudView {
    if kind == ResourceHudKind::Energy {
        return energy_hud_view(production);
    }

    let stock = resource_value(kind, colony.resources.stock());
    let available = resource_value(kind, colony.resources.available());
    let reserved = resource_value(kind, colony.resources.reserved_total());
    let capacity = resource_value(kind, production.capacity);
    let rate = resource_rate_per_second(kind, production);
    let saturation = resource_saturation(kind, production);
    let fill_ratio = resource_fill_ratio(stock, capacity);
    let status = resource_hud_status(stock, capacity);
    let warning = if status == ResourceHudStatus::Full {
        "
PLEIN — PRODUCTION BLOQUÉE"
    } else {
        ""
    };

    ResourceHudView {
        text: format!(
            "{}  {} / {}
Disponible {}  •  Réservé {}
Production +{:.2}/s  •  plein {}{}",
            kind.title(),
            stock,
            capacity,
            available,
            reserved,
            rate,
            format_saturation_time(saturation),
            warning,
        ),
        fill_ratio,
        status,
    }
}

/// A compact single-line rendering for the always-visible top resource bar, as opposed to
/// `resource_hud_view`'s multi-line detail used by the full Colony Management panel.
pub(crate) fn resource_bar_text(
    kind: ResourceHudKind,
    colony: &galactic_sim::ColonyState,
    production: galactic_sim::ColonyProductionSnapshot,
) -> String {
    if kind == ResourceHudKind::Energy {
        let produced = production.effective_energy_production;
        let consumed = production.energy_consumption;
        return format!("{consumed} / {produced}");
    }
    let stock = resource_value(kind, colony.resources.stock());
    let rate = resource_rate_per_second(kind, production);
    format!("{stock}  +{rate:.2}/s")
}

fn energy_hud_view(production: galactic_sim::ColonyProductionSnapshot) -> ResourceHudView {
    let produced = production.effective_energy_production;
    let consumed = production.energy_consumption;
    let available = produced.saturating_sub(consumed);
    let deficit = consumed > produced;
    let status = if deficit {
        ResourceHudStatus::Deficit
    } else {
        ResourceHudStatus::Normal
    };
    let warning = if deficit {
        "
DÉFICIT — PRODUCTION RALENTIE"
    } else {
        ""
    };

    ResourceHudView {
        text: format!(
            "ÉNERGIE  {} / {}
Disponible {}  •  Bilan {:+}
Rendement {}%{}",
            consumed,
            produced,
            available,
            i128::from(produced) - i128::from(consumed),
            u32::from(production.energy_efficiency_per_mille,) / 10,
            warning,
        ),
        fill_ratio: energy_fill_ratio(consumed, produced),
        status,
    }
}

fn resource_value(kind: ResourceHudKind, stock: ResourceStock) -> u64 {
    match kind {
        ResourceHudKind::Metal => stock.metal,
        ResourceHudKind::Crystal => stock.crystal,
        ResourceHudKind::Fuel => stock.fuel,
        ResourceHudKind::Energy => 0,
    }
}

fn resource_rate_per_second(
    kind: ResourceHudKind,
    production: galactic_sim::ColonyProductionSnapshot,
) -> f64 {
    match kind {
        ResourceHudKind::Metal => production.effective_rate.metal_per_second(),
        ResourceHudKind::Crystal => production.effective_rate.crystal_per_second(),
        ResourceHudKind::Fuel => production.effective_rate.fuel_per_second(),
        ResourceHudKind::Energy => 0.0,
    }
}

fn resource_saturation(
    kind: ResourceHudKind,
    production: galactic_sim::ColonyProductionSnapshot,
) -> galactic_sim::SaturationTime {
    match kind {
        ResourceHudKind::Metal => production.saturation.metal,
        ResourceHudKind::Crystal => production.saturation.crystal,
        ResourceHudKind::Fuel => production.saturation.fuel,
        ResourceHudKind::Energy => galactic_sim::SaturationTime::Never,
    }
}

fn resource_fill_ratio(stock: u64, capacity: u64) -> f32 {
    if capacity == 0 {
        return if stock == 0 { 0.0 } else { 1.0 };
    }
    (stock as f64 / capacity as f64).clamp(0.0, 1.0) as f32
}

fn energy_fill_ratio(consumption: u64, production: u64) -> f32 {
    if production == 0 {
        return if consumption == 0 { 0.0 } else { 1.0 };
    }
    (consumption as f64 / production as f64).clamp(0.0, 1.0) as f32
}

fn resource_hud_status(stock: u64, capacity: u64) -> ResourceHudStatus {
    if capacity > 0 && stock >= capacity {
        ResourceHudStatus::Full
    } else if capacity > 0 && stock.saturating_mul(100) >= capacity.saturating_mul(90) {
        ResourceHudStatus::NearlyFull
    } else {
        ResourceHudStatus::Normal
    }
}

pub(crate) fn resource_kind_color(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgb(0.90, 0.66, 0.42),
        ResourceHudKind::Crystal => Color::srgb(0.56, 0.82, 0.96),
        ResourceHudKind::Fuel => Color::srgb(0.86, 0.82, 0.38),
        ResourceHudKind::Energy => Color::srgb(0.52, 0.92, 0.62),
    }
}

pub(crate) fn resource_outline_color(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgba(0.90, 0.66, 0.42, 0.42),
        ResourceHudKind::Crystal => Color::srgba(0.56, 0.82, 0.96, 0.42),
        ResourceHudKind::Fuel => Color::srgba(0.86, 0.82, 0.38, 0.42),
        ResourceHudKind::Energy => Color::srgba(0.52, 0.92, 0.62, 0.42),
    }
}

pub(crate) fn resource_card_background(kind: ResourceHudKind) -> Color {
    match kind {
        ResourceHudKind::Metal => Color::srgba(0.16, 0.10, 0.06, 0.94),
        ResourceHudKind::Crystal => Color::srgba(0.06, 0.11, 0.16, 0.94),
        ResourceHudKind::Fuel => Color::srgba(0.15, 0.14, 0.05, 0.94),
        ResourceHudKind::Energy => Color::srgba(0.05, 0.14, 0.08, 0.94),
    }
}

pub(crate) fn status_text_color(kind: ResourceHudKind, status: ResourceHudStatus) -> Color {
    match status {
        ResourceHudStatus::Full | ResourceHudStatus::Deficit => Color::srgb(1.0, 0.48, 0.42),
        ResourceHudStatus::NearlyFull => Color::srgb(1.0, 0.78, 0.36),
        ResourceHudStatus::Normal => resource_kind_color(kind),
    }
}

pub(crate) fn status_gauge_color(kind: ResourceHudKind, status: ResourceHudStatus) -> Color {
    status_text_color(kind, status)
}

pub(crate) fn management_building_button_color(selected: bool, interaction: &Interaction) -> Color {
    if selected {
        return Color::srgba(0.08, 0.30, 0.26, 0.98);
    }
    match interaction {
        Interaction::Pressed => Color::srgba(0.09, 0.24, 0.24, 0.98),
        Interaction::Hovered => Color::srgba(0.07, 0.16, 0.18, 0.98),
        Interaction::None => Color::srgba(0.035, 0.055, 0.070, 0.96),
    }
}

pub(crate) fn management_building_button_outline(selected: bool) -> Color {
    if selected {
        Color::srgba(0.32, 0.92, 0.76, 0.80)
    } else {
        Color::srgba(0.30, 0.44, 0.50, 0.42)
    }
}

pub(crate) fn building_management_detail_text(
    colony: &galactic_sim::ColonyState,
    kind: galactic_sim::BuildingKind,
    quote: Result<galactic_sim::BuildingUpgradeQuote, galactic_sim::ConstructionError>,
) -> String {
    let catalog = galactic_sim::default_building_catalog();
    let definition = catalog.definition(kind);
    let actual_level = colony.buildings.level(kind);
    let projected_levels = galactic_sim::projected_building_levels(colony);
    let projected_level = projected_levels.level(kind);
    let current_effect = building_effect_label(colony, kind, colony.buildings);
    let projected_effect = building_effect_label(colony, kind, projected_levels);

    let mut lines = vec![
        definition.name.to_uppercase(),
        definition.description.to_string(),
        format!(
            "Niveau actif : {}  •  après la file : {}  •  maximum : {}",
            actual_level, projected_level, definition.max_level,
        ),
        String::new(),
        "EFFET".to_string(),
        format!("Actuel : {current_effect}"),
    ];
    if projected_level != actual_level {
        lines.push(format!("Après la file : {projected_effect}",));
    }

    if projected_level >= definition.max_level {
        lines.push(String::new());
        lines.push("Niveau maximal atteint ou déjà planifié.".to_string());
        return lines.join(
            "
",
        );
    }

    let target_level = projected_level + 1;
    let mut next_levels = projected_levels;
    next_levels.set_level(kind, target_level);
    let next_effect = building_effect_label(colony, kind, next_levels);
    let cost = definition
        .cost_for_level(target_level)
        .expect("catalog target level is valid");
    let duration = definition
        .duration_for_level(target_level)
        .expect("catalog target level is valid");

    lines.extend([
        format!("Prochain niveau : {next_effect}"),
        String::new(),
        format!("AMÉLIORATION VERS LE NIVEAU {}", target_level,),
        format!("Coût : {}", construction_cost_label(cost),),
        format!(
            "Durée catalogue : {}",
            format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(duration,),),
        ),
    ]);

    match quote {
        Ok(value) => {
            lines.push(format!(
                "Durée effective : {}",
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    value.duration_ticks,
                ),),
            ));
            lines.push(format!(
                "Énergie projetée : {} produite / {} consommée",
                value.projected_energy_production, value.projected_energy_consumption,
            ));
            lines.push(String::new());
            lines.push("Prêt à être ajouté à la file.".to_string());
        }
        Err(error) => {
            lines.push(String::new());
            lines.push(format!("BLOCAGE : {}", construction_error_text(error),));
        }
    }

    lines.join(
        "
",
    )
}

fn building_effect_label(
    colony: &galactic_sim::ColonyState,
    kind: galactic_sim::BuildingKind,
    levels: galactic_sim::BuildingLevels,
) -> String {
    let mut preview = colony.clone();
    preview.buildings = levels;
    preview.energy = galactic_sim::default_building_catalog().energy_grid_for_levels(levels);
    let production = galactic_sim::colony_production_snapshot(&preview);

    match galactic_sim::default_building_catalog()
        .definition(kind)
        .effect
    {
        galactic_sim::BuildingEffect::MetalProduction { .. } => {
            format!(
                "+{:.2} métal/s",
                production.effective_rate.metal_per_second(),
            )
        }
        galactic_sim::BuildingEffect::CrystalProduction { .. } => {
            format!(
                "+{:.2} cristal/s",
                production.effective_rate.crystal_per_second(),
            )
        }
        galactic_sim::BuildingEffect::FuelProduction { .. } => {
            format!(
                "+{:.2} carburant/s",
                production.effective_rate.fuel_per_second(),
            )
        }
        galactic_sim::BuildingEffect::EnergyProduction { .. } => {
            format!(
                "{} énergie effective",
                production.effective_energy_production,
            )
        }
        galactic_sim::BuildingEffect::Storage { .. } => {
            format!(
                "capacité {}/{}/{}",
                production.capacity.metal, production.capacity.crystal, production.capacity.fuel,
            )
        }
        galactic_sim::BuildingEffect::ConstructionSpeed { permille_per_level } => {
            let level = u64::from(levels.level(kind));
            let bonus = permille_per_level.saturating_mul(level) / 10;
            format!("vitesse de construction +{bonus}%")
        }
        galactic_sim::BuildingEffect::ResearchPoints { .. } => {
            future_points_effect_label(kind, levels.level(kind), "recherche")
        }
        galactic_sim::BuildingEffect::ShipyardPoints { .. } => {
            future_points_effect_label(kind, levels.level(kind), "chantier")
        }
    }
}

fn future_points_effect_label(kind: galactic_sim::BuildingKind, level: u8, label: &str) -> String {
    let definition = galactic_sim::default_building_catalog().definition(kind);
    let milli_per_tick = match definition.effect {
        galactic_sim::BuildingEffect::ResearchPoints {
            milli_per_tick_per_level,
        }
        | galactic_sim::BuildingEffect::ShipyardPoints {
            milli_per_tick_per_level,
        } => milli_per_tick_per_level,
        _ => 0,
    };
    let per_second = milli_per_tick as f64
        * f64::from(level)
        * f64::from(galactic_sim::STRATEGIC_TICKS_PER_SECOND)
        / 1_000.0;
    format!("{per_second:.2} points de {label}/s")
}

pub(crate) fn construction_queue_detail_label(colony: &galactic_sim::ColonyState) -> String {
    if colony.construction_queue.is_empty() {
        return format!(
            "File vide

{} emplacement(s) disponible(s).",
            galactic_sim::max_construction_queue(),
        );
    }

    let catalog = galactic_sim::default_building_catalog();
    let mut lines = Vec::new();
    for (index, order) in colony.construction_queue.orders().enumerate() {
        let definition = catalog.definition(order.kind);
        if index == 0 {
            lines.push(format!(
                "EN COURS
{}. {} — niveau {}
{} restant • coût réservé {}",
                index + 1,
                definition.name,
                order.target_level,
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    order.remaining_ticks,
                ),),
                construction_cost_label(order.cost),
            ));
        } else {
            lines.push(format!(
                "
EN ATTENTE
{}. {} — niveau {}
coût réservé {}",
                index + 1,
                definition.name,
                order.target_level,
                construction_cost_label(order.cost),
            ));
        }
    }

    lines.push(format!(
        "

{} / {} emplacement(s) utilisé(s)",
        colony.construction_queue.len(),
        galactic_sim::max_construction_queue(),
    ));
    lines.join(
        "
",
    )
}

pub(crate) fn construction_progress_ratio(colony: &galactic_sim::ColonyState) -> f32 {
    let Some(active) = colony.construction_queue.active() else {
        return 0.0;
    };
    if active.total_ticks == 0 {
        return 1.0;
    }
    let completed = active.total_ticks.saturating_sub(active.remaining_ticks);
    (completed as f64 / active.total_ticks as f64).clamp(0.0, 1.0) as f32
}

pub(crate) fn construction_error_text(error: galactic_sim::ConstructionError) -> String {
    match error {
        galactic_sim::ConstructionError::UnknownColony(_)
        | galactic_sim::ConstructionError::Access(_) => "Colonie indisponible".to_string(),
        galactic_sim::ConstructionError::QueueFull { maximum } => {
            format!("File pleine ({maximum})")
        }
        galactic_sim::ConstructionError::MaximumLevel { .. } => "Niveau maximal".to_string(),
        galactic_sim::ConstructionError::InsufficientResources { available, cost } => format!(
            "Manque: {}",
            construction_missing_resources_label(available, cost,),
        ),
        galactic_sim::ConstructionError::EnergyDeficit {
            production,
            consumption,
        } => format!("Énergie insuffisante : {production}/{consumption}",),
        galactic_sim::ConstructionError::Catalog(
            galactic_sim::BuildingCatalogError::UnsatisfiedPrerequisite {
                prerequisite,
                required,
                ..
            },
        ) => {
            let name = &galactic_sim::default_building_catalog()
                .definition(prerequisite)
                .name;
            format!("Requiert {name} niveau {required}")
        }
        galactic_sim::ConstructionError::Catalog(_) => "Règle catalogue invalide".to_string(),
        galactic_sim::ConstructionError::Reservation(_) => "Réservation impossible".to_string(),
    }
}

fn construction_cost_label(cost: galactic_domain::ResourceCost) -> String {
    construction_resource_amounts_label(cost.as_stock(), "gratuit")
}

fn construction_missing_resources_label(
    available: ResourceStock,
    cost: galactic_domain::ResourceCost,
) -> String {
    construction_resource_amounts_label(cost.as_stock().saturating_sub(available), "0")
}

fn construction_resource_amounts_label(resources: ResourceStock, empty_label: &str) -> String {
    let mut parts = Vec::new();
    append_resource_amount(&mut parts, resources.metal, "métal");
    append_resource_amount(&mut parts, resources.crystal, "cristal");
    append_resource_amount(&mut parts, resources.fuel, "carburant");

    if parts.is_empty() {
        empty_label.to_string()
    } else {
        parts.join(", ")
    }
}

fn append_resource_amount(parts: &mut Vec<String>, amount: u64, label: &str) {
    if amount > 0 {
        parts.push(format!("{amount} {label}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn resource_fill_ratio_is_clamped() {
        assert_eq!(resource_fill_ratio(0, 100), 0.0);
        assert_eq!(resource_fill_ratio(50, 100), 0.5);
        assert_eq!(resource_fill_ratio(150, 100), 1.0);
        assert_eq!(resource_fill_ratio(0, 0), 0.0);
        assert_eq!(resource_fill_ratio(1, 0), 1.0);
    }

    #[test]
    fn resource_status_distinguishes_normal_nearly_full_and_full() {
        assert_eq!(resource_hud_status(40, 100), ResourceHudStatus::Normal);
        assert_eq!(resource_hud_status(90, 100), ResourceHudStatus::NearlyFull);
        assert_eq!(resource_hud_status(100, 100), ResourceHudStatus::Full);
    }

    #[test]
    fn construction_resource_labels_use_names_and_omit_zeroes() {
        assert_eq!(
            construction_cost_label(galactic_domain::ResourceCost::new(0, 315, 0)),
            "315 cristal"
        );
        assert_eq!(
            construction_missing_resources_label(
                ResourceStock::new(120, 96, 3),
                galactic_domain::ResourceCost::new(120, 300, 10),
            ),
            "204 cristal, 7 carburant"
        );

        let text =
            construction_error_text(galactic_sim::ConstructionError::InsufficientResources {
                available: ResourceStock::new(120, 96, 3),
                cost: galactic_domain::ResourceCost::new(120, 300, 10),
            });

        assert_eq!(text, "Manque: 204 cristal, 7 carburant");
        assert!(!text.contains("M0"));
        assert!(!text.contains("C0"));
        assert!(!text.contains("F0"));
    }

    #[test]
    fn resource_bar_text_is_compact_and_single_line() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists");
        let production = galactic_sim::colony_production_snapshot(colony);

        let metal = resource_bar_text(ResourceHudKind::Metal, colony, production);
        assert!(!metal.contains('\n'));
        assert!(metal.contains("+"));
        assert!(metal.contains("/s"));

        let energy = resource_bar_text(ResourceHudKind::Energy, colony, production);
        assert!(!energy.contains('\n'));
        assert!(energy.contains('/'));
    }

    #[test]
    fn energy_deficit_uses_a_full_warning_gauge() {
        assert_eq!(energy_fill_ratio(60, 40), 1.0);
        assert_eq!(energy_fill_ratio(0, 0), 0.0);
    }

    #[test]
    fn building_detail_uses_catalog_and_simulation_values() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists");
        let quote = galactic_sim::building_upgrade_quote(
            simulation.state(),
            simulation.state().player_faction,
            colony.id,
            galactic_sim::BuildingKind::METAL_MINE,
        );

        let text =
            building_management_detail_text(colony, galactic_sim::BuildingKind::METAL_MINE, quote);

        assert!(text.contains("FOSSE SIDÉRURGIQUE"));
        assert!(text.contains("Niveau actif"));
        assert!(text.contains("Coût"));
        assert!(text.contains("Actuel"));
    }

    #[test]
    fn queue_progress_is_clamped_and_empty_is_zero() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let colony_id = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .id;
        assert_eq!(
            construction_progress_ratio(
                simulation.state().colony(colony_id).expect("colony exists"),
            ),
            0.0,
        );

        simulation.apply_player_action(GameAction::QueueBuildingUpgrade {
            colony_id,
            kind: galactic_sim::BuildingKind::METAL_MINE,
        });
        let ratio = construction_progress_ratio(
            simulation.state().colony(colony_id).expect("colony exists"),
        );
        assert!((0.0..=1.0).contains(&ratio));
    }
}

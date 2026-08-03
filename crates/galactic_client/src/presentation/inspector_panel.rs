use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use galactic_domain::{PlanetId, ResourceKind, SystemId};
use galactic_sim::{
    AttackMissionOutcome, ColonizationBlocker, ColonizationMissionOutcome, CombatControlChange,
    CombatOutcome, CombatReportStatus, DiplomaticRelation, EstimateRange, InstallationConstraint,
    KnowledgeLevel, MissionKind, MissionPhase, MissionResult, MissionTarget, PlanetEnvironment,
    PlanetaryForceDomain, PlanetaryIntelPrecision, PlanetaryOccupancyIntel, SelectionTarget,
    Simulation, TechnologyUnlock, assess_planet_colonizability, default_ruleset,
};

use crate::presentation::colony_management_ui::{transport_cargo_label, transport_result_label};
use crate::presentation::input::provisional_planet_label;
use crate::presentation::procedural_materials::{
    colonization_arrival_failure_label, event_label, selection_label,
};
use crate::presentation::scene::{action_button_color, action_button_outline, known_sector_labels};
use crate::presentation::strategic_navigation::{StrategicNavigation, StrategicViewMode};
use crate::{
    InspectorContent, InspectorSection, InspectorTabBarRoot, InspectorTabButton,
    InspectorTabButtonQuery, InspectorTabLabelQuery, InspectorTabState, InspectorTextQuery,
    InspectorTextRole, PresentationLog, SimulationResource, TopBarText,
};

pub(crate) fn information_panel_content(simulation: &Simulation) -> InspectorContent {
    match simulation.state().selected {
        SelectionTarget::System(system_id) => system_inspector_content(simulation, system_id),
        SelectionTarget::Planet {
            system_id,
            planet_id,
        } => planet_inspector_content(simulation, system_id, planet_id),
        SelectionTarget::None => home_inspector_content(simulation),
    }
}

fn home_inspector_content(simulation: &Simulation) -> InspectorContent {
    let state = simulation.state();
    let Some(faction) = state.player_faction_state() else {
        return inspector_error("Faction joueur invalide");
    };
    let Some(colony) = state.active_player_colony() else {
        return inspector_error("Colonie active introuvable");
    };
    let Some(system) = simulation.universe().system(colony.system_id) else {
        return inspector_error("Système de la colonie active introuvable");
    };
    let Some(planet) = simulation.universe_repository().planet(colony.planet_id) else {
        return inspector_error("Planète de la colonie active introuvable");
    };

    InspectorContent {
        level: Some(KnowledgeLevel::Colonized),
        badge: knowledge_badge_fr(KnowledgeLevel::Colonized).to_string(),
        title: format!("{} — {}", system.name, planet.name),
        sections: vec![
            InspectorSection {
                title: "Aperçu".to_string(),
                body: format!(
                    "Faction : {}\nHabitabilité : {}%",
                    faction.name, planet.habitability
                ),
            },
            InspectorSection {
                title: "Économie".to_string(),
                body: colony_economy_text(colony),
            },
            InspectorSection {
                title: "Potentiel".to_string(),
                body: potential_section_body(colony.resource_profile),
            },
            InspectorSection {
                title: "Infrastructure".to_string(),
                body: colony_buildings_text(colony),
            },
        ],
        footer: None,
        hint: "Colonie active : ressources et énergie sont exactes.".to_string(),
    }
}

fn colony_economy_text(colony: &galactic_sim::ColonyState) -> String {
    let available = colony.resources.available();
    let production = galactic_sim::colony_production_snapshot(colony);
    let construction = colony
        .construction_queue
        .active()
        .map(|order| {
            let name = &galactic_sim::default_building_catalog()
                .definition(order.kind)
                .name;
            format!(
                "{} niveau {} — {}",
                name,
                order.target_level,
                format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(
                    order.remaining_ticks,
                ),),
            )
        })
        .unwrap_or_else(|| "aucune".to_string());

    let effective = per_hour_triplet(production.effective_rate);
    let nominal = per_hour_triplet(production.nominal_rate);

    let mut lines = vec![
        format!(
            "Disponible : {} métal, {} cristal, {} carburant",
            available.metal, available.crystal, available.fuel,
        ),
        format!(
            "Production réelle : {} métal/h, {} cristal/h, {} carburant/h",
            effective.0, effective.1, effective.2,
        ),
    ];
    if nominal != effective {
        // The colony's energy efficiency is below 100% (deficit or partial coverage): show the
        // potential production without that penalty so the player understands *why* the real
        // output is lower, per MVP-030-A2 ("formaliser la production réelle = bâtiment ×
        // potentiel planétaire × efficacité énergétique").
        lines.push(format!(
            "Production théorique (potentiel, hors déficit énergétique) : {} métal/h, {} cristal/h, {} carburant/h",
            nominal.0, nominal.1, nominal.2,
        ));
    }
    lines.push(format!(
        "Énergie : {} produite, {} consommée",
        production.effective_energy_production, production.energy_consumption,
    ));
    lines.push(format!("Construction : {construction}"));
    lines.push(String::new());
    lines.push("Gestion complète : touche C".to_string());

    lines.join("\n")
}

/// Rounded metal/crystal/fuel rates in units per hour, easier to read than raw `/s` floats.
fn per_hour_triplet(rate: galactic_sim::ProductionRate) -> (i64, i64, i64) {
    (
        (rate.metal_per_second() * 3600.0).round() as i64,
        (rate.crystal_per_second() * 3600.0).round() as i64,
        (rate.fuel_per_second() * 3600.0).round() as i64,
    )
}

/// Formats a planet's resource potential (percentages, 100 = balanced baseline) with a
/// synthesized specialization label, so the player understands *why* a planet favors a given
/// resource instead of just seeing 4 bare numbers (MVP-030-A2).
fn potential_section_body(profile: galactic_sim::PlanetResourceProfile) -> String {
    format!(
        "{}\nMétal : {}%\nCristal : {}%\nCarburant : {}%\nÉnergie : {}%",
        planet_specialization_label(profile),
        profile.metal,
        profile.crystal,
        profile.fuel,
        profile.energy,
    )
}

/// A planet is "specialized" when its strongest resource clearly outpaces the runner-up,
/// otherwise it reads as balanced. Threshold picked empirically against the 6 base planet-kind
/// profiles in `planetary_analysis.ron`: at 10 points, Rocky/Ice/GasGiant/Volcanic each resolve
/// to a specific specialization while Ocean and Desert (whose top two resources sit within 5
/// points of each other) correctly stay "équilibré".
fn planet_specialization_label(profile: galactic_sim::PlanetResourceProfile) -> &'static str {
    const SPECIALIZATION_MARGIN: u16 = 10;

    let mut values = [
        ("Monde métallurgique", profile.metal),
        ("Monde cristallin", profile.crystal),
        ("Monde carburant", profile.fuel),
        ("Monde énergétique", profile.energy),
    ];
    values.sort_by_key(|(_, value)| std::cmp::Reverse(*value));
    let (top_label, top_value) = values[0];
    let (_, runner_up_value) = values[1];

    if top_value.saturating_sub(runner_up_value) >= SPECIALIZATION_MARGIN {
        top_label
    } else {
        "Monde équilibré"
    }
}

fn colony_buildings_text(colony: &galactic_sim::ColonyState) -> String {
    galactic_sim::default_building_catalog()
        .definitions()
        .map(|definition| {
            format!(
                "{} : {}",
                definition.name,
                colony.buildings.level(definition.kind),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn format_saturation_time(saturation: galactic_sim::SaturationTime) -> String {
    match saturation {
        galactic_sim::SaturationTime::Full => "plein".to_string(),
        galactic_sim::SaturationTime::Never => "jamais".to_string(),
        galactic_sim::SaturationTime::In(duration) => format_strategic_duration(duration),
    }
}

pub(crate) fn format_strategic_duration(duration: galactic_sim::StrategicDuration) -> String {
    let seconds = duration.as_duration().as_secs();
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let remaining_seconds = seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {remaining_seconds:02}s")
    } else {
        format!("{remaining_seconds}s")
    }
}

fn system_inspector_content(simulation: &Simulation, system_id: SystemId) -> InspectorContent {
    let state = simulation.state();
    let Some(system) = simulation.universe().system(system_id) else {
        return inspector_error(&format!(
            "Référence système invalide : {}",
            system_id.index(),
        ));
    };

    let level = state.system_knowledge_level(system_id);
    let visible_planets = system
        .planets
        .iter()
        .filter(|planet| state.planet_knowledge_level(planet.id).is_visible())
        .count();
    let visible_routes = simulation
        .universe_repository()
        .neighboring_systems(system_id)
        .into_iter()
        .filter(|neighbor| state.is_system_visible(*neighbor))
        .count();

    let (title, body) = match level {
        KnowledgeLevel::Unknown => (
            "Système inconnu".to_string(),
            "Identité : ???\nClasse stellaire : ???\nCorps célestes : ???\nRoutes : ???\nPosition : inconnue"
                .to_string(),
        ),
        KnowledgeLevel::Detected => (
            format!("Signal {}", system_id.index()),
            "Identité : ???\nClasse stellaire : ???\nCorps célestes : non sondés\nRoutes : signaux partiels\nPosition : repérée sur la carte"
                .to_string(),
        ),
        KnowledgeLevel::Probed => (
            system.name.clone(),
            format!(
                "Classe stellaire : {:?}\nLuminosité estimée : {}\nCorps détectés : {}\nRoutes cartographiées : {}\nPosition estimée : x {:.0}  y {:.0}  z {:.0}",
                system.star.class,
                luminosity_estimate(system.star.luminosity),
                visible_planets,
                visible_routes,
                approximate_position(system.position.x),
                approximate_position(system.position.y),
                approximate_position(system.position.z),
            ),
        ),
        KnowledgeLevel::Analyzed | KnowledgeLevel::Colonized => (
            system.name.clone(),
            format!(
                "Classe stellaire : {:?}\nLuminosité exacte : {:.2}\nCorps recensés : {}\nRoutes cartographiées : {}\nPosition exacte : x {:.1}  y {:.1}  z {:.1}",
                system.star.class,
                system.star.luminosity,
                system.planets.len(),
                visible_routes,
                system.position.x,
                system.position.y,
                system.position.z,
            ),
        ),
    };

    InspectorContent {
        level: Some(level),
        badge: knowledge_badge_fr(level).to_string(),
        title,
        sections: vec![InspectorSection {
            title: "Aperçu".to_string(),
            body,
        }],
        footer: None,
        hint: system_knowledge_hint(level).to_string(),
    }
}

fn planet_inspector_content(
    simulation: &Simulation,
    selected_system_id: SystemId,
    planet_id: galactic_domain::PlanetId,
) -> InspectorContent {
    let state = simulation.state();
    let Some((system_id, planet)) = simulation.universe_repository().planet_location(planet_id)
    else {
        return inspector_error(&format!(
            "Référence planète invalide : {}",
            planet_id.index(),
        ));
    };
    let Some(system) = simulation.universe().system(system_id) else {
        return inspector_error("Système de la planète introuvable");
    };

    let level = state.planet_knowledge_level(planet_id);
    let colony = state.colony_on_planet(planet_id);
    let system_label = if state.system_knowledge_level(system_id).reveals_identity() {
        system.name.clone()
    } else {
        format!("Signal {}", system_id.index())
    };
    let selection_note = if selected_system_id == system_id {
        "Sélection : cohérente"
    } else {
        "Sélection : recoupée avec le système réel"
    };
    let orbit_index = system
        .planets
        .iter()
        .position(|candidate| candidate.id == planet_id)
        .unwrap_or_default();

    let (title, mut sections, footer) = match level {
        KnowledgeLevel::Unknown => (
            "Corps inconnu".to_string(),
            vec![InspectorSection {
                title: "Aperçu".to_string(),
                body: format!(
                    "Système : {system_label}\nNom : ???\nType : ???\nHabitabilité : ???\nPotentiel : ???",
                ),
            }],
            format!("Lunes : ???\n{selection_note}"),
        ),
        KnowledgeLevel::Detected => (
            provisional_planet_label(&system.name, orbit_index),
            vec![InspectorSection {
                title: "Aperçu".to_string(),
                body: format!(
                    "Système : {system_label}\nIdentité : non déterminée\nOrbite : {}\nType : ???\nHabitabilité : ???\nPotentiel : analyse requise",
                    orbit_index + 1,
                ),
            }],
            format!("Lunes : non recensées\n{selection_note}"),
        ),
        KnowledgeLevel::Probed => (
            planet.name.clone(),
            vec![
                InspectorSection {
                    title: "Aperçu".to_string(),
                    body: format!(
                        "Système : {system_label}\nType : {:?}\nHabitabilité estimée : {}\nPotentiel : analyse requise",
                        planet.kind,
                        habitability_estimate(planet.habitability),
                    ),
                },
                InspectorSection {
                    title: "Renseignement".to_string(),
                    body: planetary_intelligence_text(simulation, planet_id),
                },
            ],
            format!("Lunes : non recensées\n{selection_note}"),
        ),
        KnowledgeLevel::Analyzed => (
            planet.name.clone(),
            analyzed_planet_sections(simulation, &system_label, planet),
            format!("Lunes : aucune donnée disponible\n{selection_note}"),
        ),
        KnowledgeLevel::Colonized => (
            planet.name.clone(),
            vec![
                InspectorSection {
                    title: "Aperçu".to_string(),
                    body: format!(
                        "Système : {system_label}\nType : {:?}\nHabitabilité exacte : {}%\nStatut : {}",
                        planet.kind,
                        planet.habitability,
                        colony
                            .map(|value| value.name.as_str())
                            .unwrap_or("colonie non référencée"),
                    ),
                },
                InspectorSection {
                    title: "Renseignement".to_string(),
                    body: planetary_intelligence_text(simulation, planet_id),
                },
            ],
            format!("Lunes : aucune donnée disponible\n{selection_note}"),
        ),
    };

    if let Some(colony) = colony {
        sections.push(InspectorSection {
            title: "Économie".to_string(),
            body: colony_economy_text(colony),
        });
        sections.push(InspectorSection {
            title: "Potentiel".to_string(),
            body: potential_section_body(colony.resource_profile),
        });
        sections.push(InspectorSection {
            title: "Infrastructure".to_string(),
            body: colony_buildings_text(colony),
        });
    }

    InspectorContent {
        level: Some(level),
        badge: knowledge_badge_fr(level).to_string(),
        title,
        sections,
        footer: Some(footer),
        hint: planet_knowledge_hint(level).to_string(),
    }
}

fn analyzed_planet_sections(
    simulation: &Simulation,
    system_label: &str,
    planet: &galactic_domain::Planet,
) -> Vec<InspectorSection> {
    let Some(report) = simulation.state().planet_analysis_report(planet.id) else {
        return vec![InspectorSection {
            title: "Aperçu".to_string(),
            body: format!(
                "Système : {system_label}\nType : {:?}\nHabitabilité exacte : {}%\nRapport d'analyse : manquant",
                planet.kind, planet.habitability,
            ),
        }];
    };
    let constraints = report
        .constraints
        .iter()
        .map(installation_constraint_label)
        .collect::<Vec<_>>();
    let constraints = if constraints.is_empty() {
        "aucune".to_string()
    } else {
        constraints.join(", ")
    };
    let assessment = assess_planet_colonizability(
        simulation.state(),
        simulation.universe_repository(),
        simulation.state().player_faction,
        planet.id,
    );

    vec![
        InspectorSection {
            title: "Aperçu".to_string(),
            body: format!(
                "Système : {system_label}\nType : {:?}\nEnvironnement : {}\nHabitabilité exacte : {}%\nContraintes : {constraints}\nRapport établi au tick {}",
                planet.kind,
                planet_environment_label(report.environment),
                report.habitability,
                report.analyzed_at.value(),
            ),
        },
        InspectorSection {
            title: "Potentiel".to_string(),
            body: potential_section_body(report.resource_profile),
        },
        InspectorSection {
            title: "Renseignement".to_string(),
            body: planetary_intelligence_text(simulation, planet.id),
        },
        InspectorSection {
            title: "Colonisation".to_string(),
            body: format!(
                "{}\n\n{}",
                colonizability_text(&assessment, simulation.state()),
                extraction_site_text(simulation, planet.id),
            ),
        },
    ]
}

fn extraction_site_text(simulation: &Simulation, planet_id: PlanetId) -> String {
    let Some(site) = simulation.state().extraction_site_on_planet(planet_id) else {
        return "SITE D'EXTRACTION\nAucun gisement recensé".to_string();
    };
    let resource = match site.resource {
        ResourceKind::Metal => "métal",
        ResourceKind::Crystal => "cristal",
        ResourceKind::Fuel => "carburant",
        ResourceKind::Energy => "énergie",
    };
    let status = if site.is_depleted() {
        "épuisé".to_string()
    } else if let Some(mission_id) = site.reserved_by {
        format!("réservé par la mission {}", mission_id.raw())
    } else if simulation.state().colony_on_planet(planet_id).is_some() {
        "intégré à la colonie".to_string()
    } else if !simulation
        .state()
        .research
        .has_unlock(TechnologyUnlock::RemoteExtraction)
    {
        "Prospection autonome requise".to_string()
    } else {
        "disponible — H pour lancer la récolte".to_string()
    };
    let planet = simulation
        .universe_repository()
        .planet(planet_id)
        .expect("an extraction site references a generated planet");
    let rule = default_ruleset().extraction().rule_for(planet.kind);

    format!(
        "SITE D'EXTRACTION\nRessource : {resource}\nRéserve : {}\nRendement : {}/tick pendant {} ticks\nStatut : {status}",
        site.remaining, rule.yield_per_tick, rule.harvest_ticks,
    )
}

fn planetary_intelligence_text(simulation: &Simulation, planet_id: PlanetId) -> String {
    let state = simulation.state();
    let Some(report) = state.planetary_intelligence_report(planet_id) else {
        return "RENSEIGNEMENT PLANÉTAIRE\nRapport indisponible".to_string();
    };

    let observed = format!("Observation au tick {}", report.observed_at.value());
    let intelligence = match report.precision {
        PlanetaryIntelPrecision::Contact => {
            let presence = match report.occupancy {
                PlanetaryOccupancyIntel::Unoccupied => {
                    "aucune présence organisée détectée".to_string()
                }
                PlanetaryOccupancyIntel::OccupiedUnknown => {
                    "signature occupante détectée, identité inconnue".to_string()
                }
                PlanetaryOccupancyIntel::Occupied(faction_id) => {
                    format!("signature attribuée à la faction {}", faction_id.raw())
                }
            };
            format!(
                "RENSEIGNEMENT PLANÉTAIRE — CONTACT\n{observed}\nPrésence : {presence}\nForces terrestres : {}\nDéfenses orbitales : {}\nUne analyse est requise pour identifier les unités et leurs effectifs.",
                strategic_signal_label(report.ground_strength),
                strategic_signal_label(report.orbital_strength),
            )
        }
        PlanetaryIntelPrecision::Surveyed | PlanetaryIntelPrecision::Exact => {
            let precision_label = if report.precision == PlanetaryIntelPrecision::Exact {
                "DONNÉES LOCALES"
            } else {
                "ESTIMATION"
            };
            let presence = match report.occupancy {
                PlanetaryOccupancyIntel::Unoccupied => "aucune présence organisée".to_string(),
                PlanetaryOccupancyIntel::OccupiedUnknown => {
                    "présence occupante non attribuée".to_string()
                }
                PlanetaryOccupancyIntel::Occupied(faction_id) => {
                    let name = state
                        .faction(faction_id)
                        .map(|faction| faction.name.as_str())
                        .unwrap_or("faction inconnue");
                    let relation = state
                        .relation_between(state.player_faction, faction_id)
                        .unwrap_or(DiplomaticRelation::Unknown);
                    format!("{name} — relation {}", diplomatic_relation_label(relation))
                }
            };
            let population = report
                .population
                .map(estimate_range_text)
                .unwrap_or_else(|| "aucune".to_string());
            let force_catalog = default_ruleset().planetary_presence();
            let forces = if report.forces.is_empty() {
                "• aucune unité recensée".to_string()
            } else {
                report
                    .forces
                    .iter()
                    .map(|force| {
                        let Some(definition) = force_catalog.definition(force.definition_id) else {
                            return format!(
                                "• unité inconnue {} : {}",
                                force.definition_id,
                                estimate_range_text(force.quantity),
                            );
                        };
                        format!(
                            "• {} ({}) : {}",
                            definition.name,
                            planetary_force_domain_label(definition.domain),
                            estimate_range_text(force.quantity),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                "RENSEIGNEMENT PLANÉTAIRE — {precision_label}\n{observed}\nOccupant : {presence}\nPopulation : {population}\nIndice terrestre : {}\nIndice orbital : {}\n{forces}",
                estimate_range_text(report.ground_strength),
                estimate_range_text(report.orbital_strength),
            )
        }
    };
    state
        .latest_combat_report_for_planet(planet_id)
        .map(|combat| format!("{intelligence}\n\n{}", combat_report_text(combat)))
        .unwrap_or(intelligence)
}

fn combat_report_text(report: &galactic_sim::CombatReport) -> String {
    match &report.status {
        CombatReportStatus::TargetInvalid(reason) => format!(
            "RAPPORT DE COMBAT — CIBLE INVALIDÉE\nMission {} • tick {}\n{}.\nAucune donnée défensive supplémentaire n'a été révélée.",
            report.mission_id.raw(),
            report.resolved_at.value(),
            attack_invalid_reason_label(*reason),
        ),
        CombatReportStatus::Resolved(resolution) => {
            let control = match resolution.control {
                CombatControlChange::Unchanged => "contrôle territorial inchangé",
                CombatControlChange::Secured { .. } => "orbite et surface sécurisées par le joueur",
            };
            format!(
                "RAPPORT DE COMBAT — {}\nMission {} • tick {} • {} round(s)\nAttaquants engagés : {}\nAttaquants survivants : {}\nDéfense engagée : {}\nDéfense survivante : {}\nDommages subis/infligés : {} / {}\nRécupérable : {} métal, {} cristal, {} carburant\nRécupéré : {} métal, {} cristal, {} carburant\nContrôle : {}",
                combat_outcome_label(resolution.outcome).to_uppercase(),
                report.mission_id.raw(),
                report.resolved_at.value(),
                resolution.rounds,
                combat_ship_stacks_text(&report.attacker.ships),
                combat_ship_stacks_text(&resolution.attacker_survivors),
                planetary_force_stacks_text(&report.defender.forces),
                planetary_force_stacks_text(&resolution.defender_survivors),
                resolution.attacker_damage,
                resolution.defender_damage,
                resolution.salvage_recoverable.metal,
                resolution.salvage_recoverable.crystal,
                resolution.salvage_recoverable.fuel,
                resolution.salvage_recovered.metal,
                resolution.salvage_recovered.crystal,
                resolution.salvage_recovered.fuel,
                control,
            )
        }
    }
}

fn combat_ship_stacks_text(stacks: &[galactic_sim::CombatShipStack]) -> String {
    if stacks.is_empty() {
        return "aucun".to_string();
    }
    stacks
        .iter()
        .map(|stack| {
            format!(
                "{} × {}",
                stack.quantity,
                galactic_sim::craftable_definition(stack.craftable).name,
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn planetary_force_stacks_text(stacks: &[galactic_sim::PlanetaryForceStack]) -> String {
    if stacks.is_empty() {
        return "aucune".to_string();
    }
    let catalog = default_ruleset().planetary_presence();
    stacks
        .iter()
        .map(|stack| {
            let name = catalog
                .definition(stack.definition_id)
                .map(|definition| definition.name)
                .unwrap_or("unité inconnue");
            format!("{} × {name}", stack.quantity)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) const fn combat_outcome_label(outcome: CombatOutcome) -> &'static str {
    match outcome {
        CombatOutcome::AttackerVictory => "victoire attaquante",
        CombatOutcome::DefenderVictory => "victoire défensive",
        CombatOutcome::Stalemate => "affrontement indécis",
        CombatOutcome::MutualDestruction => "destruction mutuelle",
    }
}

const fn attack_invalid_reason_label(reason: galactic_sim::AttackInvalidReason) -> &'static str {
    match reason {
        galactic_sim::AttackInvalidReason::TargetOwnerChanged => {
            "le contrôle de la planète a changé pendant le trajet"
        }
        galactic_sim::AttackInvalidReason::TargetPresenceChanged => {
            "les forces présentes ont changé pendant le trajet"
        }
        galactic_sim::AttackInvalidReason::AttackerFleetChanged => {
            "la flotte attaquante ne correspond plus à l'engagement"
        }
    }
}

fn estimate_range_text(range: EstimateRange) -> String {
    if range.is_exact() {
        range.minimum.to_string()
    } else {
        format!("{}–{}", range.minimum, range.maximum)
    }
}

fn strategic_signal_label(range: EstimateRange) -> &'static str {
    match range.maximum {
        0 => "aucune signature",
        1..=749 => "faible",
        750..=1_999 => "modérée",
        _ => "forte",
    }
}

const fn planetary_force_domain_label(domain: PlanetaryForceDomain) -> &'static str {
    match domain {
        PlanetaryForceDomain::Ground => "sol",
        PlanetaryForceDomain::Orbital => "orbite",
    }
}

const fn diplomatic_relation_label(relation: DiplomaticRelation) -> &'static str {
    match relation {
        DiplomaticRelation::Unknown => "inconnue",
        DiplomaticRelation::Neutral => "neutre",
        DiplomaticRelation::Hostile => "hostile",
        DiplomaticRelation::Allied => "alliée",
    }
}

fn planet_environment_label(environment: PlanetEnvironment) -> &'static str {
    match environment {
        PlanetEnvironment::Temperate => "tempéré",
        PlanetEnvironment::Oceanic => "océanique",
        PlanetEnvironment::Arid => "aride",
        PlanetEnvironment::Frozen => "gelé",
        PlanetEnvironment::Volcanic => "volcanique",
        PlanetEnvironment::Gaseous => "gazeux",
    }
}

fn installation_constraint_label(constraint: InstallationConstraint) -> &'static str {
    match constraint {
        InstallationConstraint::ThinAtmosphere => "atmosphère ténue",
        InstallationConstraint::GlobalOcean => "océan global",
        InstallationConstraint::AridClimate => "climat aride",
        InstallationConstraint::CryogenicClimate => "climat cryogénique",
        InstallationConstraint::ExtremeVolcanism => "volcanisme extrême",
        InstallationConstraint::NoSolidSurface => "absence de surface solide",
    }
}

fn colonizability_text(
    assessment: &galactic_sim::ColonizabilityAssessment,
    state: &galactic_sim::GameState,
) -> String {
    let cost = assessment.foundation_cost;
    if assessment.is_colonizable() {
        let colony_ship = default_ruleset().planetary_analysis().colony_ship();
        let active_colony = state.active_player_colony();
        let origin = active_colony
            .map(|colony| colony.name.as_str())
            .unwrap_or("aucune colonie active");
        let ship_ready = active_colony.is_some_and(|colony| {
            colony.inventory.quantity(colony_ship) > 0
                || state.fleets.iter().any(|fleet| {
                    fleet.is_idle()
                        && fleet.location == galactic_sim::FleetLocation::Docked(colony.id)
                        && fleet.composition.total_ships() == 1
                        && fleet.composition.quantity(colony_ship) == 1
                })
        });
        let ship_status = if ship_ready {
            format!(
                "Depuis {origin} : Arche Pionnière disponible — appuyez sur N pour lancer la mission."
            )
        } else {
            format!(
                "Depuis {origin} : Arche Pionnière manquante — construisez-en une au chantier orbital."
            )
        };
        return format!(
            "COLONISABILITÉ — ÉLIGIBLE
Conditions remplies : analyse, environnement, habitabilité, route, technologie, limite et cargaison.
Investissement requis : {} métal, {} cristal, {} carburant
{ship_status}",
            cost.metal, cost.crystal, cost.fuel,
        );
    }

    let blockers = assessment
        .blockers
        .iter()
        .map(|blocker| format!("• {}", colonization_blocker_label(*blocker, state)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "COLONISABILITÉ — BLOQUÉE
Conditions manquantes :
{blockers}
Investissement requis : {} métal, {} cristal, {} carburant",
        cost.metal, cost.crystal, cost.fuel,
    )
}

pub(crate) fn colonization_blocker_label(
    blocker: ColonizationBlocker,
    state: &galactic_sim::GameState,
) -> String {
    match blocker {
        ColonizationBlocker::UnknownPlanet(_) => "planète inconnue".to_string(),
        ColonizationBlocker::NotAnalyzed { current } => {
            format!("analyse complète requise (niveau actuel : {current})")
        }
        ColonizationBlocker::MissingAnalysisReport => {
            "rapport d'analyse persistant introuvable".to_string()
        }
        ColonizationBlocker::AlreadyColonized => "planète déjà colonisée".to_string(),
        ColonizationBlocker::FoundationAlreadyPrepared => {
            "une fondation coloniale attend son initialisation".to_string()
        }
        ColonizationBlocker::OccupiedPlanet { occupant, relation } => {
            let name = state
                .faction(occupant)
                .map(|faction| faction.name.as_str())
                .unwrap_or("faction inconnue");
            format!(
                "présence {} non sécurisée : {name}",
                diplomatic_relation_label(relation),
            )
        }
        ColonizationBlocker::MissingTechnology(TechnologyUnlock::FoundColonies) => {
            "technologie Ingénierie d'implantation manquante".to_string()
        }
        ColonizationBlocker::MissingTechnology(unlock) => {
            format!("technologie requise manquante : {unlock:?}")
        }
        ColonizationBlocker::UnsupportedEnvironment(environment) => format!(
            "environnement {} incompatible avec une implantation au sol",
            planet_environment_label(environment),
        ),
        ColonizationBlocker::HabitabilityTooLow { minimum, found } => {
            format!("habitabilité insuffisante : {found}% (minimum {minimum}%)")
        }
        ColonizationBlocker::NoAccessibleRoute => {
            "aucune route connue depuis une colonie du joueur".to_string()
        }
        ColonizationBlocker::ColonyLimitReached { maximum } => {
            format!("limite de {maximum} colonies déjà atteinte")
        }
        ColonizationBlocker::InsufficientFoundationResources { cost } => format!(
            "aucune colonie ne dispose de la cargaison de fondation ({} métal, {} cristal, {} carburant)",
            cost.metal, cost.crystal, cost.fuel,
        ),
    }
}

fn inspector_error(message: &str) -> InspectorContent {
    InspectorContent {
        level: None,
        badge: "[ERREUR D’INSPECTEUR]".to_string(),
        title: "Donnée indisponible".to_string(),
        sections: vec![InspectorSection {
            title: "Erreur".to_string(),
            body: message.to_string(),
        }],
        footer: None,
        hint: "La sélection ne correspond pas à une donnée valide.".to_string(),
    }
}

pub(crate) const fn knowledge_badge_fr(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "[INCONNU — DONNÉES MASQUÉES]",
        KnowledgeLevel::Detected => "[DÉTECTÉ — DONNÉES MASQUÉES]",
        KnowledgeLevel::Probed => "[SONDÉ — ESTIMATIONS]",
        KnowledgeLevel::Analyzed => "[ANALYSÉ — RAPPORT COMPLET]",
        KnowledgeLevel::Colonized => "[COLONISÉ — VALEURS EXACTES]",
    }
}

const fn system_knowledge_hint(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "Action requise : détecter le système.",
        KnowledgeLevel::Detected => "Action requise : sonder le système pour révéler son identité.",
        KnowledgeLevel::Probed => {
            "Action requise : analyser le système pour obtenir les valeurs exactes."
        }
        KnowledgeLevel::Analyzed => "Analyse terminée : les valeurs disponibles sont exactes.",
        KnowledgeLevel::Colonized => "Système colonisé : les valeurs disponibles sont exactes.",
    }
}

const fn planet_knowledge_hint(level: KnowledgeLevel) -> &'static str {
    match level {
        KnowledgeLevel::Unknown => "Action requise : détecter ce corps céleste.",
        KnowledgeLevel::Detected => "Action requise : sonder la planète pour révéler son identité.",
        KnowledgeLevel::Probed => {
            "Action requise : analyser la planète pour obtenir les valeurs exactes."
        }
        KnowledgeLevel::Analyzed => {
            "Analyse terminée : les caractéristiques disponibles sont exactes."
        }
        KnowledgeLevel::Colonized => "Planète colonisée : les données économiques sont exactes.",
    }
}

pub(crate) fn knowledge_color(level: Option<KnowledgeLevel>) -> Color {
    match level {
        None | Some(KnowledgeLevel::Unknown) => Color::srgb(0.72, 0.76, 0.80),
        Some(KnowledgeLevel::Detected) => Color::srgb(0.58, 0.72, 0.88),
        Some(KnowledgeLevel::Probed) => Color::srgb(0.56, 0.88, 0.94),
        Some(KnowledgeLevel::Analyzed) => Color::srgb(0.96, 0.82, 0.48),
        Some(KnowledgeLevel::Colonized) => Color::srgb(0.58, 0.94, 0.72),
    }
}

fn luminosity_estimate(luminosity: f32) -> &'static str {
    if luminosity < 0.6 {
        "faible"
    } else if luminosity < 1.6 {
        "moyenne"
    } else if luminosity < 2.6 {
        "forte"
    } else {
        "très forte"
    }
}

fn habitability_estimate(habitability: u8) -> &'static str {
    match habitability {
        0..=19 => "très faible",
        20..=39 => "faible",
        40..=59 => "moyenne",
        60..=79 => "bonne",
        _ => "excellente",
    }
}

fn approximate_position(value: f32) -> f32 {
    (value / 5.0).round() * 5.0
}

pub(crate) fn update_ui(
    simulation: Res<SimulationResource>,
    navigation: Res<StrategicNavigation>,
    log: Res<PresentationLog>,
    mut query: Query<&mut Text, With<TopBarText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };
    let simulation = simulation.simulation();
    let universe = simulation.universe();
    let repository = simulation.universe_repository();
    let state = simulation.state();
    let selected = selection_label(state.selected);
    let active_colony = state
        .active_player_colony()
        .map(|colony| format!("C{} {}", colony.id.raw(), colony.name))
        .unwrap_or_else(|| "aucune".to_string());
    let last_event = log
        .last_event
        .map(event_label)
        .unwrap_or_else(|| "ready".to_string());
    let visible_route_count = if navigation.debug_full_graph {
        universe.routes.len()
    } else {
        state.visible_routes(repository).len()
    };
    let visible_system_count = if navigation.debug_full_graph {
        universe.systems.len()
    } else {
        state.visible_systems().len()
    };
    let known_sector_count = known_sector_labels(simulation).len();
    let knowledge = state.system_knowledge_counts();
    let mission_status = mission_status_line(simulation);
    let view_label = match navigation.mode {
        StrategicViewMode::Universe => format!(
            "univers {:?} | projection {}",
            navigation.lod,
            navigation.projection.label(),
        ),
        StrategicViewMode::System(system_id) => {
            format!("système {}", system_id.index())
        }
    };

    let next = format!(
        "Galactic MVP | échelle {} ({}) | graphique {:?} | {} | tick {} | vitesse {} | colonie active {} | cible {}\nSystèmes {}/{} | Secteurs connus {}/{} | Routes {}/{} | Détectés/Sondés/Analysés/Colonisés {}/{}/{}/{} | debug {} | {}\n{}",
        navigation.scale_preset.label(),
        navigation.scale_preset.system_count(),
        navigation.preset,
        view_label,
        state.clock.current_tick(),
        state.clock.speed(),
        active_colony,
        selected,
        visible_system_count,
        universe.systems.len(),
        known_sector_count,
        universe.sectors.len(),
        visible_route_count,
        universe.routes.len(),
        knowledge.detected,
        knowledge.probed,
        knowledge.analyzed,
        knowledge.colonized,
        navigation.debug_full_graph,
        last_event,
        mission_status,
    );
    if text.0 != next {
        text.0 = next;
    }
}

pub(crate) fn mission_kind_label(kind: MissionKind) -> &'static str {
    match kind {
        MissionKind::Probe => "Reconnaissance",
        MissionKind::Attack => "Attaque",
        MissionKind::Transport => "Transport",
        MissionKind::Harvest => "Récolte",
        MissionKind::Colonize => "Colonisation",
    }
}

pub(crate) fn mission_phase_label(phase: MissionPhase) -> &'static str {
    match phase {
        MissionPhase::Preparation => "préparation",
        MissionPhase::Outbound => "transit aller",
        MissionPhase::OnSite => "sur place",
        MissionPhase::Returning => "transit retour",
        MissionPhase::Completed => "terminée",
        MissionPhase::Cancelled => "annulée",
        MissionPhase::Failed => "échec",
    }
}

pub(crate) fn mission_next_deadline(
    mission: &galactic_sim::MissionState,
    current_tick: galactic_sim::StrategicTick,
) -> galactic_sim::StrategicTick {
    match mission.phase {
        MissionPhase::Preparation => mission.order.departure_at,
        MissionPhase::Outbound => mission.plan.outbound_arrival_at,
        MissionPhase::OnSite => mission.plan.return_departure_at,
        MissionPhase::Returning => mission.plan.return_arrival_at,
        MissionPhase::Completed | MissionPhase::Cancelled | MissionPhase::Failed => current_tick,
    }
}

pub(crate) fn mission_status_line(simulation: &Simulation) -> String {
    let state = simulation.state();
    let Some(mission) = state
        .player_missions()
        .filter(|mission| !mission.phase.is_terminal())
        .min_by_key(|mission| mission.id)
    else {
        return "Missions : aucune mission active".to_string();
    };
    let target = mission_target_label(simulation, mission.order.target);
    let phase = mission_phase_label(mission.phase);
    let deadline = mission_next_deadline(mission, state.clock.current_tick());
    let remaining = deadline
        .value()
        .saturating_sub(state.clock.current_tick().value());
    let kind = mission_kind_label(mission.order.kind);

    format!(
        "Mission {} • {} vers {} • {} • prochaine étape dans {}",
        mission.id.raw(),
        kind,
        target,
        phase,
        format_strategic_duration(galactic_sim::StrategicDuration::from_ticks(remaining)),
    )
}

pub(crate) fn mission_target_label(simulation: &Simulation, target: MissionTarget) -> String {
    let state = simulation.state();
    match target {
        MissionTarget::System(system_id) => simulation
            .universe()
            .system(system_id)
            .map(|system| {
                if state.system_knowledge_level(system_id).reveals_identity() {
                    system.name.clone()
                } else {
                    format!("Signal {}", system_id.index())
                }
            })
            .unwrap_or_else(|| format!("Système {}", system_id.index())),
        MissionTarget::Planet {
            system_id,
            planet_id,
        } => simulation
            .universe()
            .system(system_id)
            .and_then(|system| {
                system
                    .planets
                    .iter()
                    .position(|planet| planet.id == planet_id)
                    .map(|index| {
                        if state.planet_knowledge_level(planet_id).reveals_identity() {
                            system.planets[index].name.clone()
                        } else {
                            provisional_planet_label(&system.name, index)
                        }
                    })
            })
            .unwrap_or_else(|| format!("Planète {}", planet_id.index())),
    }
}

pub(crate) fn handle_inspector_tab_buttons(
    mut tab_state: ResMut<InspectorTabState>,
    interactions: Query<(&Interaction, &InspectorTabButton), Changed<Interaction>>,
) {
    for (interaction, button) in &interactions {
        if *interaction == Interaction::Pressed {
            tab_state.active = button.index;
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct InspectorPanelWidgets<'w, 's> {
    texts: InspectorTextQuery<'w, 's>,
    tab_bar:
        Query<'w, 's, &'static mut Node, (With<InspectorTabBarRoot>, Without<InspectorTabButton>)>,
    tab_buttons: InspectorTabButtonQuery<'w, 's>,
    tab_labels: InspectorTabLabelQuery<'w, 's>,
}

pub(crate) fn update_info_panel(
    simulation: Res<SimulationResource>,
    mut tab_state: ResMut<InspectorTabState>,
    mut widgets: InspectorPanelWidgets,
) {
    let content = information_panel_content(simulation.simulation());
    tab_state.sync(&content.sections);
    let active = tab_state.active;

    for (role, mut text, mut color) in &mut widgets.texts {
        let next_text = match role {
            InspectorTextRole::Title => format!("{}\n{}", content.badge, content.title),
            InspectorTextRole::Body => content
                .sections
                .get(active)
                .map(|section| section.body.clone())
                .unwrap_or_default(),
            InspectorTextRole::Footer => match &content.footer {
                Some(footer) => format!("{footer}\n\n{}", content.hint),
                None => content.hint.clone(),
            },
        };
        if text.0 != next_text {
            text.0 = next_text;
        }
        if *role == InspectorTextRole::Title {
            let next_color = knowledge_color(content.level);
            if color.0 != next_color {
                color.0 = next_color;
            }
        }
    }

    let show_tabs = content.sections.len() > 1;
    for mut node in &mut widgets.tab_bar {
        let next_display = if show_tabs {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != next_display {
            node.display = next_display;
        }
    }

    for (button, interaction, mut background, mut outline, mut node, children) in
        &mut widgets.tab_buttons
    {
        let available = show_tabs && button.index < content.sections.len();
        let is_active = button.index == active;
        let next_display = if available {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != next_display {
            node.display = next_display;
        }
        let next_background = action_button_color(available, is_active, interaction);
        if background.0 != next_background {
            background.0 = next_background;
        }
        let next_outline = action_button_outline(available, is_active, interaction);
        if outline.color != next_outline {
            outline.color = next_outline;
        }
        if let Some(section) = content.sections.get(button.index) {
            for child in children {
                if let Ok(mut text) = widgets.tab_labels.get_mut(*child)
                    && text.0 != section.title
                {
                    text.0 = section.title.clone();
                }
            }
        }
    }
}

pub(crate) fn mission_result_text(result: MissionResult) -> String {
    match result {
        MissionResult::Probe(result) => match result.target {
            MissionTarget::System(system_id) => format!(
                "reconnaissance terminée : système {} sondé, {} nouveaux signaux, {} routes et {} planètes révélées",
                system_id.index(),
                result.newly_detected_systems,
                result.revealed_routes,
                result.revealed_planets,
            ),
            MissionTarget::Planet { planet_id, .. } => format!(
                "reconnaissance planétaire terminée : corps {} identifié",
                planet_id.index(),
            ),
        },
        MissionResult::Attack(result) => match result.outcome {
            AttackMissionOutcome::Resolved(outcome) => format!(
                "combat terminé sur le corps {} : {}{}",
                result.target.index(),
                combat_outcome_label(outcome),
                if result.secured {
                    ", planète sécurisée"
                } else {
                    ""
                },
            ),
            AttackMissionOutcome::TargetInvalid(_) => format!(
                "attaque annulée sur le corps {} : cible devenue invalide",
                result.target.index(),
            ),
        },
        MissionResult::Transport(result) => transport_result_label(result),
        MissionResult::Harvest(result) => format!(
            "récolte terminée sur le site {} : {}, {} livré, {} conservé en soute, réserve restante {}",
            result.site_id.raw(),
            transport_cargo_label(result.collected),
            transport_cargo_label(result.delivered),
            transport_cargo_label(result.retained),
            result.site_remaining,
        ),
        MissionResult::Colonize(result) => match result.outcome {
            ColonizationMissionOutcome::FoundationPrepared => format!(
                "fondation prête sur le corps {} : Arche Pionnière et chargement déployés",
                result.target.index(),
            ),
            ColonizationMissionOutcome::TargetInvalid(blocker) => format!(
                "colonisation annulée sur le corps {} : {}",
                result.target.index(),
                colonization_arrival_failure_label(blocker),
            ),
        },
    }
}

pub(crate) fn mission_error_text(error: galactic_sim::MissionError) -> String {
    match error {
        galactic_sim::MissionError::ProbeUnavailable(_) => {
            "aucune Sonde Luciole disponible ; construisez-en une au chantier orbital".to_string()
        }
        galactic_sim::MissionError::ProbeRequired(_) => {
            "la flotte sélectionnée ne contient aucune Sonde Luciole".to_string()
        }
        galactic_sim::MissionError::ProbeTargetNotDetected { .. } => {
            "la reconnaissance exige un système ou une planète actuellement détecté".to_string()
        }
        galactic_sim::MissionError::AttackFleetUnavailable(_) => {
            "aucune Frégate Rempart disponible ; construisez-en au chantier orbital".to_string()
        }
        galactic_sim::MissionError::AttackTargetNotAnalyzed { .. } => {
            "la cible doit être sondée puis analysée avant une attaque".to_string()
        }
        galactic_sim::MissionError::AttackPlanetTargetRequired => {
            "une attaque doit cibler une planète".to_string()
        }
        galactic_sim::MissionError::Attack(
            galactic_sim::CombatSnapshotError::UnoccupiedTarget(_),
        ) => "la planète est inoccupée et ne peut pas être attaquée".to_string(),
        galactic_sim::MissionError::Attack(galactic_sim::CombatSnapshotError::FriendlyTarget(
            _,
        )) => "la planète est déjà sous contrôle allié".to_string(),
        galactic_sim::MissionError::Attack(
            galactic_sim::CombatSnapshotError::FleetNotCombatCapable(_),
        ) => "la flotte doit être composée de vaisseaux militaires".to_string(),
        galactic_sim::MissionError::TransportOrderRequired => {
            "utilisez l'ordre logistique avec une origine, une destination et une cargaison"
                .to_string()
        }
        galactic_sim::MissionError::TransportCargoEmpty => {
            "la cargaison de transport ne peut pas être vide".to_string()
        }
        galactic_sim::MissionError::TransportCargoAmountOverflow => {
            "la cargaison demandée est trop importante".to_string()
        }
        galactic_sim::MissionError::UnknownTransportDestination(_) => {
            "la colonie de destination n'existe plus".to_string()
        }
        galactic_sim::MissionError::TransportDestinationIsOrigin(_) => {
            "l'origine et la destination du transport doivent être différentes".to_string()
        }
        galactic_sim::MissionError::TransportDestinationTargetMismatch { .. } => {
            "la destination ne correspond plus à la colonie choisie".to_string()
        }
        galactic_sim::MissionError::TransportFleetUnavailable {
            required_capacity,
            available_capacity,
            ..
        } => format!(
            "capacité cargo insuffisante : {required_capacity} requise, {available_capacity} disponible ; construisez des Caboteurs Sillage"
        ),
        galactic_sim::MissionError::TransportFleetHasCargo(_) => {
            "la flotte sélectionnée transporte déjà une cargaison".to_string()
        }
        galactic_sim::MissionError::TransportCargoExceedsCapacity { capacity, .. } => {
            format!("la cargaison dépasse la capacité de la flotte ({capacity})")
        }
        galactic_sim::MissionError::HarvestOrderRequired => {
            "utilisez l'ordre de récolte avec une colonie d'origine et un site analysé".to_string()
        }
        galactic_sim::MissionError::UnknownExtractionSite(_) => {
            "le site d'extraction sélectionné n'existe plus".to_string()
        }
        galactic_sim::MissionError::HarvestTargetMismatch { .. } => {
            "le site ne correspond plus à la planète sélectionnée".to_string()
        }
        galactic_sim::MissionError::HarvestPlanetNotAnalyzed { .. } => {
            "la planète doit être sondée puis analysée avant toute récolte".to_string()
        }
        galactic_sim::MissionError::MissingHarvestTechnology(_) => {
            "recherchez Prospection autonome avant de lancer une récolte".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteOnColony(_) => {
            "ce gisement appartient déjà à une colonie et n'est pas un site distant".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteDepleted(_) => {
            "ce site d'extraction est épuisé".to_string()
        }
        galactic_sim::MissionError::ExtractionSiteBusy { .. } => {
            "ce site est déjà réservé par une autre mission".to_string()
        }
        galactic_sim::MissionError::HarvestFleetUnavailable(_) => {
            "aucun Caboteur Sillage disponible ; construisez-en au chantier orbital".to_string()
        }
        galactic_sim::MissionError::HarvestFleetHasCargo(_) => {
            "la flotte de récolte transporte déjà une cargaison".to_string()
        }
        galactic_sim::MissionError::ColonizationPlanetTargetRequired => {
            "une colonisation doit cibler une planète".to_string()
        }
        galactic_sim::MissionError::ColonizationShipUnavailable(_) => {
            "aucune Arche Pionnière disponible ; construisez-en une au chantier orbital".to_string()
        }
        galactic_sim::MissionError::ColonizationFleetRequired(_) => {
            "la flotte de colonisation doit contenir exactement une Arche Pionnière".to_string()
        }
        galactic_sim::MissionError::ColonizationBlocked(blocker) => {
            format!(
                "colonisation impossible : {}",
                colonization_arrival_failure_label(blocker)
            )
        }
        galactic_sim::MissionError::NoAccessibleRoute { .. } => {
            "aucune route connue ne permet d'atteindre cette destination".to_string()
        }
        galactic_sim::MissionError::InsufficientRange {
            required_hops,
            available_hops,
        } => format!(
            "portée insuffisante : {required_hops} sauts requis, {available_hops} disponibles"
        ),
        galactic_sim::MissionError::Resources(_) => {
            "ressources insuffisantes dans la colonie d'origine (carburant ou cargaison)"
                .to_string()
        }
        galactic_sim::MissionError::FleetBusy { .. } => {
            "la flotte est déjà affectée à une mission".to_string()
        }
        galactic_sim::MissionError::FleetNotDocked(_) => {
            "la flotte doit être amarrée à la colonie d'origine".to_string()
        }
        galactic_sim::MissionError::UnknownTarget(_)
        | galactic_sim::MissionError::UnknownPlanetTarget(_)
        | galactic_sim::MissionError::UnknownOrigin(_) => {
            "origine ou destination inconnue".to_string()
        }
        galactic_sim::MissionError::PlanetTargetSystemMismatch { .. } => {
            "la planète ne correspond pas au système sélectionné".to_string()
        }
        galactic_sim::MissionError::SameSystem(_) => {
            "l'origine et la destination doivent être différentes".to_string()
        }
        _ => format!("{error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn planet_specialization_matches_the_dominant_resource() {
        use galactic_sim::PlanetResourceProfile;

        // Rocky: metal: 135, crystal: 95, fuel: 70, energy: 105 (planetary_analysis.ron).
        assert_eq!(
            planet_specialization_label(PlanetResourceProfile::new(135, 95, 70, 105)),
            "Monde métallurgique",
        );
        // GasGiant: metal: 35, crystal: 85, fuel: 165, energy: 120.
        assert_eq!(
            planet_specialization_label(PlanetResourceProfile::new(35, 85, 165, 120)),
            "Monde carburant",
        );
        // Ice: metal: 90, crystal: 135, fuel: 125, energy: 70.
        assert_eq!(
            planet_specialization_label(PlanetResourceProfile::new(90, 135, 125, 70)),
            "Monde cristallin",
        );
    }

    #[test]
    fn planet_specialization_is_balanced_when_the_top_two_resources_are_close() {
        use galactic_sim::PlanetResourceProfile;

        assert_eq!(
            planet_specialization_label(PlanetResourceProfile::BALANCED),
            "Monde équilibré",
        );
        // Ocean: metal: 80, crystal: 115, fuel: 105, energy: 115 — crystal and energy tie.
        assert_eq!(
            planet_specialization_label(PlanetResourceProfile::new(80, 115, 105, 115)),
            "Monde équilibré",
        );
    }

    #[test]
    fn detected_system_inspector_masks_secret_values() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let state = simulation.state();
        let detected = state
            .system_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the starting frontier contains a detected system")
            .system_id;
        let system = simulation
            .universe()
            .system(detected)
            .expect("detected system exists");

        let rendered = system_inspector_content(&simulation, detected).render();

        assert!(rendered.contains("DÉTECTÉ"));
        assert!(rendered.contains("Identité : ???"));
        assert!(rendered.contains("Classe stellaire : ???"));
        assert!(!rendered.contains(&system.name));
        assert!(!rendered.contains(&format!("{:?}", system.star.class)));
        assert!(!rendered.contains(&format!("{:.1}", system.position.x)));
    }

    #[test]
    fn system_inspector_distinguishes_estimates_and_exact_values() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let detected = simulation
            .state()
            .system_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the starting frontier contains a detected system")
            .system_id;
        simulation.apply_player_action(GameAction::SelectSystem(detected));
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);

        let probed = system_inspector_content(&simulation, detected).render();
        assert!(probed.contains("SONDÉ"));
        assert!(probed.contains("Luminosité estimée"));

        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        let analyzed = system_inspector_content(&simulation, detected).render();
        let system = simulation
            .universe()
            .system(detected)
            .expect("analyzed system exists");

        assert!(analyzed.contains("ANALYSÉ"));
        assert!(analyzed.contains("Luminosité exacte"));
        assert!(analyzed.contains(&format!("{:.2}", system.star.luminosity)));
    }

    #[test]
    fn detected_planet_inspector_hides_identity_and_habitability() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let detected = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, planet) = simulation
            .universe_repository()
            .planet_location(detected)
            .expect("detected planet exists");

        let rendered = planet_inspector_content(&simulation, system_id, detected).render();
        let system = simulation
            .universe()
            .system(system_id)
            .expect("detected planet system exists");
        let orbit_index = system
            .planets
            .iter()
            .position(|candidate| candidate.id == detected)
            .expect("detected planet belongs to its system");

        assert!(rendered.contains("DÉTECTÉ"));
        assert!(rendered.contains(&provisional_planet_label(&system.name, orbit_index)));
        assert!(rendered.contains("Identité : non déterminée"));
        assert!(rendered.contains("Habitabilité : ???"));
        assert!(!rendered.contains(&planet.name));
        assert!(!rendered.contains(&format!("{:?}", planet.kind)));
    }

    #[test]
    fn analyzed_planet_inspector_shows_exact_report_and_colonization_status() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let planet_id = simulation
            .state()
            .planet_knowledge
            .iter()
            .find(|entry| entry.level == KnowledgeLevel::Detected)
            .expect("the home system contains a detected planet")
            .planet_id;
        let (system_id, planet) = simulation
            .universe_repository()
            .planet_location(planet_id)
            .expect("detected planet exists");
        let planet_name = planet.name.clone();
        let habitability = planet.habitability;
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        simulation.apply_player_action(GameAction::DebugAdvanceSelectedKnowledge);
        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
            galactic_sim::TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.apply_player_action(GameAction::AnalyzePlanet { planet_id });

        let rendered = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert!(rendered.contains("ANALYSÉ"));
        assert!(rendered.contains(&planet_name));
        assert!(rendered.contains(&format!("Habitabilité exacte : {habitability}%")));
        assert!(rendered.contains("Rapport établi au tick"));
        assert!(rendered.contains("Potentiel"));
        assert!(rendered.contains("COLONISABILITÉ — BLOQUÉE"));
        assert!(rendered.contains("SITE D'EXTRACTION"));
        assert!(rendered.contains("Prospection autonome requise"));
        assert!(!rendered.contains("Potentiel : analyse requise"));
    }

    #[test]
    fn planetary_intelligence_progresses_without_leaking_real_forces() {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let presence = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                presence.occupant != galactic_domain::Owner::Unowned
                    && !presence.forces.is_empty()
                    && simulation
                        .state()
                        .colony_on_planet(presence.planet_id)
                        .is_none()
            })
            .expect("the MVP seed contains an occupied remote planet")
            .clone();
        let planet_id = presence.planet_id;
        let system_id = planet_id.system_id();
        let occupant = presence
            .occupant
            .faction()
            .expect("selected presence is occupied");
        let occupant_name = simulation
            .state()
            .faction(occupant)
            .expect("occupant exists")
            .name
            .clone();
        let force_names = presence
            .forces
            .iter()
            .map(|force| {
                default_ruleset()
                    .planetary_presence()
                    .definition(force.definition_id)
                    .expect("force definition exists")
                    .name
            })
            .collect::<Vec<_>>();
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            planet_id,
            KnowledgeLevel::Probed,
        );
        simulation.apply_player_action(GameAction::SelectPlanet {
            system_id,
            planet_id,
        });
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            planet_id,
            PlanetaryIntelPrecision::Contact,
            galactic_sim::StrategicTick::ZERO,
        )
        .expect("contact report");

        let contact = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert!(contact.contains("RENSEIGNEMENT PLANÉTAIRE — CONTACT"));
        assert!(contact.contains("identité inconnue"));
        assert!(!contact.contains(&occupant_name));
        assert!(force_names.iter().all(|name| !contact.contains(name)));
        assert!(!contact.contains("RAPPORT DE COMBAT"));
        assert_eq!(selected_attack_context(&simulation), None);

        simulation.state_mut().research = galactic_sim::ResearchState::from_completed([
            galactic_sim::TechnologyId::SPATIAL_DETECTION,
            galactic_sim::TechnologyId::PLANETARY_ANALYSIS,
        ]);
        simulation.apply_player_action(GameAction::AnalyzePlanet { planet_id });
        let report = simulation
            .state()
            .planetary_intelligence_report(planet_id)
            .expect("analysis refreshes intelligence");
        let surveyed = planet_inspector_content(&simulation, system_id, planet_id).render();

        assert_eq!(report.precision, PlanetaryIntelPrecision::Surveyed);
        assert!(surveyed.contains("RENSEIGNEMENT PLANÉTAIRE — ESTIMATION"));
        assert!(surveyed.contains(&occupant_name));
        assert!(report.forces.iter().all(|force| {
            !force.quantity.is_exact() && surveyed.contains(&estimate_range_text(force.quantity))
        }));
        assert!(!surveyed.contains("DONNÉES LOCALES"));
        assert!(!surveyed.contains("RAPPORT DE COMBAT"));
        assert_eq!(
            selected_attack_context(&simulation),
            Some((
                simulation
                    .state()
                    .player_home_colony()
                    .expect("the player home colony exists")
                    .id,
                MissionTarget::Planet {
                    system_id,
                    planet_id,
                },
            ))
        );
    }

    #[test]
    fn planet_information_panel_includes_home_colony_details() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let panel = information_panel_content(&simulation);
        let rendered = panel.render();

        assert_eq!(panel.level, Some(KnowledgeLevel::Colonized));
        assert!(rendered.contains("Port-Sillage"));
        assert!(rendered.contains("Économie"));
        assert!(rendered.contains("Gestion complète : touche C"));
        assert!(rendered.contains("Infrastructure"));
        assert!(rendered.contains("Production réelle"));
        assert!(
            rendered.contains("Monde"),
            "the Potentiel tab must name a specialization"
        );
        assert!(
            rendered.contains("Métal : 100%"),
            "potential values must be labeled as a percentage"
        );
    }

    #[test]
    fn theoretical_production_is_shown_only_when_it_differs_from_the_real_output() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let colony = simulation
            .state()
            .player_home_colony()
            .expect("home colony exists");

        // The starting colony has no energy deficit: nominal and effective rates match, so the
        // theoretical line must not appear (it would be pure noise).
        assert_eq!(
            galactic_sim::colony_production_snapshot(colony).nominal_rate,
            galactic_sim::colony_production_snapshot(colony).effective_rate,
        );
        let text = colony_economy_text(colony);
        assert!(!text.contains("Production théorique"));

        let mut deficit_colony = colony.clone();
        deficit_colony
            .buildings
            .set_level(galactic_sim::BuildingKind::METAL_MINE, 10);
        deficit_colony
            .buildings
            .set_level(galactic_sim::BuildingKind::CRYSTAL_EXTRACTOR, 10);
        deficit_colony.energy = galactic_sim::default_building_catalog()
            .energy_grid_for_levels(deficit_colony.buildings);
        let snapshot = galactic_sim::colony_production_snapshot(&deficit_colony);
        assert!(
            snapshot.energy_consumption > snapshot.effective_energy_production,
            "test setup must actually create an energy deficit",
        );
        let text = colony_economy_text(&deficit_colony);
        assert!(
            text.contains("Production théorique"),
            "an energy deficit must reveal the theoretical (undamped) production",
        );
    }
}

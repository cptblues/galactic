// MVP-033: session-current vertical slice success condition.
use galactic_domain::{FactionId, Owner};
use serde::Deserialize;

use crate::{
    AttackMissionOutcome, CombatOutcome, CombatReportStatus, GameState, HarvestMissionResult,
    KnowledgeLevel, MissionKind, MissionReportOutcome, MissionResult, PlanetaryOccupancyIntel,
    StartingFactionConfig, TechnologyCatalog, TechnologyId, UniverseRepository, default_ruleset,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VictoryRules {
    version: u32,
    pub required_colonies: usize,
    pub required_probed_systems: usize,
    pub required_technology: TechnologyId,
    pub required_completed_harvests: usize,
    pub required_sylve_analysis_reports: usize,
    pub required_sylve_attack_victories: usize,
    pub sylve_faction_id: FactionId,
}

impl VictoryRules {
    pub(crate) fn from_config(
        config: VictoryRulesConfig,
        factions: &[StartingFactionConfig],
        technologies: &TechnologyCatalog,
    ) -> Result<Self, VictoryRulesError> {
        if config.version != 1 {
            return Err(VictoryRulesError::UnsupportedVersion(config.version));
        }
        require_positive(config.required_colonies, "required_colonies")?;
        require_positive(config.required_probed_systems, "required_probed_systems")?;
        require_positive(
            config.required_completed_harvests,
            "required_completed_harvests",
        )?;
        require_positive(
            config.required_sylve_analysis_reports,
            "required_sylve_analysis_reports",
        )?;
        require_positive(
            config.required_sylve_attack_victories,
            "required_sylve_attack_victories",
        )?;

        let sylve_faction_id = FactionId::new(config.sylve_faction_id);
        if !factions
            .iter()
            .any(|faction| faction.id == sylve_faction_id)
        {
            return Err(VictoryRulesError::UnknownFaction(sylve_faction_id));
        }
        let Some(required_technology) = technologies.id_by_key(&config.required_technology) else {
            return Err(VictoryRulesError::UnknownTechnology);
        };

        Ok(Self {
            version: config.version,
            required_colonies: config.required_colonies,
            required_probed_systems: config.required_probed_systems,
            required_technology,
            required_completed_harvests: config.required_completed_harvests,
            required_sylve_analysis_reports: config.required_sylve_analysis_reports,
            required_sylve_attack_victories: config.required_sylve_attack_victories,
            sylve_faction_id,
        })
    }

    pub const fn version(self) -> u32 {
        self.version
    }

    pub(crate) fn append_structure(&self, output: &mut String) {
        output.push_str("victory:");
        output.push_str(&self.version.to_string());
        output.push(':');
        output.push_str(&self.required_colonies.to_string());
        output.push(':');
        output.push_str(&self.required_probed_systems.to_string());
        output.push(':');
        output.push_str(self.required_technology.key());
        output.push(':');
        output.push_str(&self.required_completed_harvests.to_string());
        output.push(':');
        output.push_str(&self.required_sylve_analysis_reports.to_string());
        output.push(':');
        output.push_str(&self.required_sylve_attack_victories.to_string());
        output.push(':');
        output.push_str(&self.sylve_faction_id.raw().to_string());
        output.push(';');
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VictoryRulesConfig {
    version: u32,
    required_colonies: usize,
    required_probed_systems: usize,
    required_technology: String,
    required_completed_harvests: usize,
    required_sylve_analysis_reports: usize,
    required_sylve_attack_victories: usize,
    sylve_faction_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryRulesError {
    UnsupportedVersion(u32),
    EmptyThreshold(&'static str),
    UnknownFaction(FactionId),
    UnknownTechnology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VictoryConditionProgress {
    pub current: usize,
    pub required: usize,
}

impl VictoryConditionProgress {
    pub const fn new(current: usize, required: usize) -> Self {
        Self { current, required }
    }

    pub const fn complete(self) -> bool {
        self.current >= self.required
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VictoryProgress {
    pub colonies: VictoryConditionProgress,
    pub probed_systems: VictoryConditionProgress,
    pub required_technology: VictoryConditionProgress,
    pub completed_harvests: VictoryConditionProgress,
    pub sylve_analysis_reports: VictoryConditionProgress,
    pub sylve_attack_victories: VictoryConditionProgress,
}

impl VictoryProgress {
    pub const fn is_complete(self) -> bool {
        self.colonies.complete()
            && self.probed_systems.complete()
            && self.required_technology.complete()
            && self.completed_harvests.complete()
            && self.sylve_analysis_reports.complete()
            && self.sylve_attack_victories.complete()
    }
}

pub fn victory_rules() -> &'static VictoryRules {
    default_ruleset().victory()
}

pub fn evaluate_victory_progress(
    state: &GameState,
    universe: &UniverseRepository,
    rules: &VictoryRules,
) -> VictoryProgress {
    VictoryProgress {
        colonies: VictoryConditionProgress::new(
            state.player_colonies().count(),
            rules.required_colonies,
        ),
        probed_systems: VictoryConditionProgress::new(
            probed_system_count(state, universe),
            rules.required_probed_systems,
        ),
        required_technology: VictoryConditionProgress::new(
            usize::from(state.research.has_completed(rules.required_technology)),
            1,
        ),
        completed_harvests: VictoryConditionProgress::new(
            completed_harvest_count(state),
            rules.required_completed_harvests,
        ),
        sylve_analysis_reports: VictoryConditionProgress::new(
            sylve_analysis_report_count(state, rules.sylve_faction_id),
            rules.required_sylve_analysis_reports,
        ),
        sylve_attack_victories: VictoryConditionProgress::new(
            sylve_attack_victory_count(state, rules.sylve_faction_id),
            rules.required_sylve_attack_victories,
        ),
    }
}

fn require_positive(value: usize, name: &'static str) -> Result<(), VictoryRulesError> {
    if value == 0 {
        Err(VictoryRulesError::EmptyThreshold(name))
    } else {
        Ok(())
    }
}

fn probed_system_count(state: &GameState, universe: &UniverseRepository) -> usize {
    state
        .system_knowledge
        .iter()
        .filter(|knowledge| {
            knowledge.level >= KnowledgeLevel::Probed
                && universe.system(knowledge.system_id).is_some()
        })
        .count()
}

fn completed_harvest_count(state: &GameState) -> usize {
    state
        .mission_reports
        .iter()
        .filter(|report| {
            report.kind == MissionKind::Harvest
                && report.outcome == MissionReportOutcome::Completed
                && matches!(
                    report.result,
                    Some(MissionResult::Harvest(HarvestMissionResult { delivered, .. }))
                        if !delivered.is_zero()
                )
        })
        .count()
}

fn sylve_analysis_report_count(state: &GameState, sylve_faction_id: FactionId) -> usize {
    state
        .planet_analysis_reports
        .iter()
        .filter(|report| {
            planet_is_currently_known_as_sylve(state, report.planet_id, sylve_faction_id)
                || planet_was_sylve_when_secured_after_analysis(state, report, sylve_faction_id)
        })
        .count()
}

fn planet_is_currently_known_as_sylve(
    state: &GameState,
    planet_id: galactic_domain::PlanetId,
    sylve_faction_id: FactionId,
) -> bool {
    state
        .planetary_intelligence_report(planet_id)
        .is_some_and(|intel| intel.occupancy == PlanetaryOccupancyIntel::Occupied(sylve_faction_id))
}

fn planet_was_sylve_when_secured_after_analysis(
    state: &GameState,
    analysis: &crate::PlanetAnalysisReport,
    sylve_faction_id: FactionId,
) -> bool {
    state.combat_reports.iter().any(|combat| {
        combat.planet_id == analysis.planet_id
            && combat.resolved_at >= analysis.analyzed_at
            && combat.defender.occupant == Owner::Faction(sylve_faction_id)
    })
}

fn sylve_attack_victory_count(state: &GameState, sylve_faction_id: FactionId) -> usize {
    state
        .mission_reports
        .iter()
        .filter(|report| {
            if report.kind != MissionKind::Attack
                || report.outcome != MissionReportOutcome::Completed
            {
                return false;
            }
            let Some(MissionResult::Attack(result)) = report.result else {
                return false;
            };
            if !matches!(
                result.outcome,
                AttackMissionOutcome::Resolved(CombatOutcome::AttackerVictory)
            ) {
                return false;
            }
            state
                .combat_report(report.mission_id)
                .is_some_and(|combat| {
                    combat.defender.occupant == Owner::Faction(sylve_faction_id)
                        && matches!(
                            combat.status,
                            CombatReportStatus::Resolved(ref resolution)
                                if resolution.outcome == CombatOutcome::AttackerVictory
                        )
                })
        })
        .count()
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FleetId, MissionId, ResourceStock, UniverseConfig};

    use super::*;
    use crate::{
        AttackMissionResult, CombatControlChange, CombatFleetSnapshot, CombatReport,
        CombatResolution, MissionReport, PlanetAnalysisReport, PlanetDefenseSnapshot,
        PlanetEnvironment, PlanetResourceProfile, ResearchState, StrategicTick,
    };

    #[test]
    fn default_victory_rules_are_loaded_from_ruleset() {
        let rules = victory_rules();

        assert_eq!(rules.version(), 1);
        assert_eq!(rules.required_colonies, 3);
        assert_eq!(rules.required_probed_systems, 8);
        assert_eq!(rules.required_technology, TechnologyId::COLONIZATION);
        assert_eq!(rules.sylve_faction_id, FactionId::new(2));
    }

    #[test]
    fn incomplete_state_does_not_trigger_victory() {
        let simulation = crate::Simulation::new(UniverseConfig::test());
        let progress = evaluate_victory_progress(
            simulation.state(),
            simulation.universe_repository(),
            victory_rules(),
        );

        assert!(!progress.is_complete());
        assert_eq!(progress.colonies.current, 1);
    }

    #[test]
    fn victory_requires_every_configured_condition() {
        let mut simulation = crate::Simulation::new(UniverseConfig::test());
        satisfy_all_conditions(&mut simulation);

        let progress = evaluate_victory_progress(
            simulation.state(),
            simulation.universe_repository(),
            victory_rules(),
        );

        assert!(progress.is_complete());
        assert_eq!(progress.probed_systems.current, 8);
        assert_eq!(progress.sylve_attack_victories.current, 1);
    }

    #[test]
    fn non_sylve_reports_do_not_satisfy_sylve_conditions() {
        let mut simulation = crate::Simulation::new(UniverseConfig::test());
        satisfy_all_conditions(&mut simulation);
        let state = simulation.state_mut();
        let player = state.player_faction;
        state
            .planetary_intelligence_reports
            .iter_mut()
            .for_each(|intel| intel.occupancy = PlanetaryOccupancyIntel::Occupied(player));
        state
            .combat_reports
            .iter_mut()
            .for_each(|report| report.defender.occupant = Owner::Faction(player));

        let progress = evaluate_victory_progress(
            simulation.state(),
            simulation.universe_repository(),
            victory_rules(),
        );

        assert!(!progress.is_complete());
        assert_eq!(progress.sylve_analysis_reports.current, 0);
        assert_eq!(progress.sylve_attack_victories.current, 0);
    }

    #[test]
    fn sylve_analysis_remains_satisfied_after_the_planet_is_secured() {
        let mut simulation = crate::Simulation::new(UniverseConfig::test());
        satisfy_all_conditions(&mut simulation);
        let state = simulation.state_mut();
        let player = state.player_faction;
        state
            .planetary_intelligence_reports
            .iter_mut()
            .for_each(|intel| intel.occupancy = PlanetaryOccupancyIntel::Occupied(player));

        let progress = evaluate_victory_progress(
            simulation.state(),
            simulation.universe_repository(),
            victory_rules(),
        );

        assert!(progress.is_complete());
        assert_eq!(progress.sylve_analysis_reports.current, 1);
        assert_eq!(progress.sylve_attack_victories.current, 1);
    }

    fn satisfy_all_conditions(simulation: &mut crate::Simulation) {
        let rules = victory_rules();
        let universe = simulation.universe_repository().clone();
        let system_ids = universe
            .definition()
            .systems
            .iter()
            .take(rules.required_probed_systems)
            .map(|system| system.id)
            .collect::<Vec<_>>();
        for system_id in system_ids {
            simulation.state_mut().advance_system_knowledge(
                &universe,
                system_id,
                KnowledgeLevel::Probed,
            );
        }

        {
            let state = simulation.state_mut();
            let home = state.player_home_colony().expect("starting colony").clone();
            for id in [ColonyId::new(1), ColonyId::new(2)] {
                let mut colony = home.clone();
                colony.id = id;
                colony.name = format!("Colonie test {}", id.raw());
                state.colonies.push(colony);
            }
            state.research = ResearchState::from_completed([rules.required_technology]);
        }

        let planet_id = simulation
            .universe()
            .systems
            .iter()
            .flat_map(|system| system.planets.iter())
            .find(|planet| planet.id != crate::MVP_HOME_PLANET_ID)
            .expect("test universe contains a non-home planet")
            .id;
        let sylve = rules.sylve_faction_id;
        let state = simulation.state_mut();
        state.planet_analysis_reports.push(PlanetAnalysisReport {
            planet_id,
            system_id: crate::MVP_HOME_SYSTEM_ID,
            analyzed_at: StrategicTick::ZERO,
            habitability: 60,
            environment: PlanetEnvironment::Volcanic,
            resource_profile: PlanetResourceProfile::new(100, 100, 100, 100),
            constraints: Default::default(),
        });
        crate::refresh_planetary_intelligence(
            state,
            planet_id,
            crate::PlanetaryIntelPrecision::Exact,
            StrategicTick::ZERO,
        )
        .expect("planet has generated presence");
        state
            .planetary_intelligence_reports
            .iter_mut()
            .find(|intel| intel.planet_id == planet_id)
            .expect("intelligence was refreshed")
            .occupancy = PlanetaryOccupancyIntel::Occupied(sylve);

        let harvest_mission_id = MissionId::new(42);
        state.mission_reports.push(MissionReport {
            mission_id: harvest_mission_id,
            fleet_id: FleetId::new(10),
            kind: MissionKind::Harvest,
            outcome: MissionReportOutcome::Completed,
            occurred_at: StrategicTick::ZERO,
            result: Some(MissionResult::Harvest(HarvestMissionResult {
                site_id: galactic_domain::ExtractionSiteId::for_planet(planet_id),
                collected: ResourceStock::new(0, 100, 0),
                delivered: ResourceStock::new(0, 100, 0),
                retained: ResourceStock::ZERO,
                site_remaining: 0,
                status: crate::HarvestCollectionStatus::Collected,
            })),
        });

        let attack_mission_id = MissionId::new(43);
        state.mission_reports.push(MissionReport {
            mission_id: attack_mission_id,
            fleet_id: FleetId::new(11),
            kind: MissionKind::Attack,
            outcome: MissionReportOutcome::Completed,
            occurred_at: StrategicTick::ZERO,
            result: Some(MissionResult::Attack(AttackMissionResult {
                target: planet_id,
                outcome: AttackMissionOutcome::Resolved(CombatOutcome::AttackerVictory),
                secured: true,
                attackers_destroyed: false,
            })),
        });
        state.combat_reports.push(CombatReport {
            mission_id: attack_mission_id,
            planet_id,
            resolved_at: StrategicTick::ZERO,
            rules_version: crate::combat_rules().version(),
            seed: 1,
            attacker: CombatFleetSnapshot {
                fleet_id: FleetId::new(11),
                owner: Owner::Faction(state.player_faction),
                ships: Vec::new(),
                cargo: ResourceStock::ZERO,
                cargo_capacity: 0,
            },
            defender: PlanetDefenseSnapshot {
                planet_id,
                occupant: Owner::Faction(sylve),
                population: 1,
                forces: Vec::new(),
                revision: 0,
            },
            round_history: Vec::new(),
            initial_plan: None,
            final_plan: None,
            intervention_history: Vec::new(),
            status: CombatReportStatus::Resolved(CombatResolution {
                outcome: CombatOutcome::AttackerVictory,
                rounds: 1,
                attacker_losses: Vec::new(),
                attacker_survivors: Vec::new(),
                defender_losses: Vec::new(),
                defender_survivors: Vec::new(),
                attacker_damage: 0,
                defender_damage: 1,
                salvage_recoverable: ResourceStock::ZERO,
                salvage_recovered: ResourceStock::ZERO,
                control: CombatControlChange::Secured {
                    previous: Owner::Faction(sylve),
                    current: Owner::Faction(state.player_faction),
                },
            }),
        });
    }
}

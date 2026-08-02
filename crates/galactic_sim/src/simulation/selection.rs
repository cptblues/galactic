use galactic_domain::{ColonyId, FactionId, PlanetId, SystemId};

use crate::{
    ColonySelectionError, GameEventKind, KnowledgeLevel, PlanetaryIntelPrecision, SelectionTarget,
    TimeSpeed, build_planet_analysis_report, planetary_analysis_rules,
    refresh_planetary_intelligence,
};

use super::Simulation;

impl Simulation {
    pub(crate) fn set_speed(&mut self, speed: TimeSpeed) -> Vec<GameEventKind> {
        if !self.state.clock.set_speed(speed) {
            return Vec::new();
        }

        vec![GameEventKind::SpeedChanged(speed)]
    }

    pub(crate) fn select_system(&mut self, system_id: SystemId) -> Vec<GameEventKind> {
        if self.universe.system(system_id).is_none() {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::System(system_id))
    }

    pub(crate) fn select_planet(
        &mut self,
        system_id: SystemId,
        planet_id: PlanetId,
    ) -> Vec<GameEventKind> {
        let Some((planet_system_id, _)) = self.universe.planet_location(planet_id) else {
            return Vec::new();
        };
        if planet_system_id != system_id {
            return Vec::new();
        }

        self.set_selection(SelectionTarget::Planet {
            system_id,
            planet_id,
        })
    }

    pub(crate) fn debug_advance_selected_knowledge(&mut self) -> Vec<GameEventKind> {
        let changes = match self.state.selected {
            SelectionTarget::None => Vec::new(),
            SelectionTarget::System(system_id) => {
                let current = self.state.system_knowledge_level(system_id);
                let Some(next) = current.next_exploration_level() else {
                    return Vec::new();
                };
                self.state
                    .advance_system_knowledge(&self.universe, system_id, next)
            }
            SelectionTarget::Planet { planet_id, .. } => {
                let current = self.state.planet_knowledge_level(planet_id);
                let Some(next) = current.next_exploration_level() else {
                    return Vec::new();
                };
                if next == KnowledgeLevel::Analyzed {
                    let Some((system_id, planet)) = self.universe.planet_location(planet_id) else {
                        return Vec::new();
                    };
                    let report = build_planet_analysis_report(
                        planet,
                        system_id,
                        self.state.clock.current_tick(),
                        planetary_analysis_rules(),
                    );
                    let changes =
                        self.state
                            .advance_planet_knowledge(&self.universe, planet_id, next);
                    self.state.planet_analysis_reports.push(report);
                    self.state
                        .planet_analysis_reports
                        .sort_by_key(|entry| entry.planet_id);
                    let observed_at = self.state.clock.current_tick();
                    refresh_planetary_intelligence(
                        &mut self.state,
                        planet_id,
                        PlanetaryIntelPrecision::Surveyed,
                        observed_at,
                    )
                    .expect("an analyzed planet has a deterministic presence");
                    changes
                } else {
                    let changes =
                        self.state
                            .advance_planet_knowledge(&self.universe, planet_id, next);
                    if next == KnowledgeLevel::Probed {
                        let observed_at = self.state.clock.current_tick();
                        refresh_planetary_intelligence(
                            &mut self.state,
                            planet_id,
                            PlanetaryIntelPrecision::Contact,
                            observed_at,
                        )
                        .expect("a probed planet has a deterministic presence");
                    }
                    changes
                }
            }
        };

        changes
            .into_iter()
            .map(GameEventKind::KnowledgeChanged)
            .collect()
    }

    pub(crate) fn set_selection(&mut self, selection: SelectionTarget) -> Vec<GameEventKind> {
        if self.state.selected == selection {
            return Vec::new();
        }

        self.state.selected = selection;
        vec![GameEventKind::SelectionChanged(selection)]
    }

    pub(crate) fn select_active_colony(
        &mut self,
        actor: FactionId,
        colony_id: ColonyId,
    ) -> Result<Vec<GameEventKind>, ColonySelectionError> {
        if actor != self.state.player_faction {
            return Err(ColonySelectionError::NotPlayerFaction(actor));
        }
        let colony = self
            .state
            .colony(colony_id)
            .ok_or(ColonySelectionError::UnknownColony(colony_id))?;
        self.state
            .authorize_management(actor, colony.owner)
            .map_err(ColonySelectionError::Access)?;
        let selection = SelectionTarget::Planet {
            system_id: colony.system_id,
            planet_id: colony.planet_id,
        };

        let mut events = Vec::with_capacity(2);
        if self.state.active_colony_id != Some(colony_id) {
            self.state.active_colony_id = Some(colony_id);
            events.push(GameEventKind::ActiveColonyChanged(colony_id));
        }
        events.extend(self.set_selection(selection));
        Ok(events)
    }
}

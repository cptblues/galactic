use galactic_domain::{Owner, SystemId, UniverseDefinition};
use galactic_sim::{DiplomaticRelation, GameState, PlanetaryOccupancyIntel, SystemVisibility};

/// Territory tint derived strictly from information the player already has
/// access to. `None` (no tint) is always the safe default when a system is
/// not `Known` or when no faction presence has been identified there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TerritoryTint {
    SelfOwned,
    Allied,
    Hostile,
    Neutral,
    /// A presence has been detected but its owning faction has not been
    /// identified yet — deliberately distinct from `Neutral` so the map
    /// never implies "nothing here" when something unidentified is there.
    UnidentifiedPresence,
}

impl TerritoryTint {
    pub(crate) const ALL: [Self; 5] = [
        Self::SelfOwned,
        Self::Allied,
        Self::Hostile,
        Self::Neutral,
        Self::UnidentifiedPresence,
    ];
}

/// Computes the territory tint for a system, gated strictly by what the
/// player currently knows. A system must be at `SystemVisibility::Known`
/// (the same gate used everywhere else for identity-revealing rendering) to
/// receive any tint at all; below that, this always returns `None`. Beyond
/// the player's own colonies, presence is only ever read from
/// `GameState::planetary_intelligence_report`, never from the raw,
/// all-factions `GameState::colonies` list.
pub(crate) fn system_territory_tint(
    state: &GameState,
    universe: &UniverseDefinition,
    system_id: SystemId,
) -> Option<TerritoryTint> {
    if state.system_visibility(system_id) != Some(SystemVisibility::Known) {
        return None;
    }

    let is_player_colony = state.colonies.iter().any(|colony| {
        colony.system_id == system_id && colony.owner == Owner::Faction(state.player_faction)
    });
    if is_player_colony {
        return Some(TerritoryTint::SelfOwned);
    }

    let system = universe.system(system_id)?;
    let mut unidentified_presence = false;
    for planet in &system.planets {
        let Some(report) = state.planetary_intelligence_report(planet.id) else {
            continue;
        };
        match report.occupancy {
            PlanetaryOccupancyIntel::Occupied(faction_id) => {
                let relation = state
                    .relation_between(state.player_faction, faction_id)
                    .unwrap_or(DiplomaticRelation::Unknown);
                return Some(match relation {
                    DiplomaticRelation::Allied => TerritoryTint::Allied,
                    DiplomaticRelation::Hostile => TerritoryTint::Hostile,
                    DiplomaticRelation::Neutral | DiplomaticRelation::Unknown => {
                        TerritoryTint::Neutral
                    }
                });
            }
            PlanetaryOccupancyIntel::OccupiedUnknown => {
                unidentified_presence = true;
            }
            PlanetaryOccupancyIntel::Unoccupied => {}
        }
    }

    if unidentified_presence {
        return Some(TerritoryTint::UnidentifiedPresence);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use galactic_domain::{FactionId, PlanetId, UniverseConfig};
    use galactic_sim::{
        FactionData, FactionKind, KnowledgeLevel, PlanetaryIntelPrecision,
        PlanetaryIntelligenceReport, Simulation,
    };

    fn other_system_id(simulation: &Simulation) -> SystemId {
        let home_system = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists")
            .system_id;
        simulation
            .universe_repository()
            .definition()
            .systems
            .iter()
            .map(|system| system.id)
            .find(|id| *id != home_system)
            .expect("universe has more than one system")
    }

    fn first_planet_id(simulation: &Simulation, system_id: SystemId) -> PlanetId {
        simulation
            .universe()
            .system(system_id)
            .expect("system exists")
            .planets
            .first()
            .expect("system has at least one planet")
            .id
    }

    fn intelligence_report(
        planet_id: PlanetId,
        occupancy: PlanetaryOccupancyIntel,
    ) -> PlanetaryIntelligenceReport {
        PlanetaryIntelligenceReport {
            planet_id,
            observed_at: Default::default(),
            precision: PlanetaryIntelPrecision::Contact,
            occupancy,
            population: None,
            ground_strength: galactic_sim::EstimateRange::exact(0),
            orbital_strength: galactic_sim::EstimateRange::exact(0),
            forces: Vec::new(),
        }
    }

    #[test]
    fn a_system_below_known_tier_never_receives_a_tint_even_with_a_hostile_report_behind_it() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let system_id = other_system_id(&simulation);
        assert_ne!(
            simulation.state().system_visibility(system_id),
            Some(SystemVisibility::Known)
        );

        let planet_id = first_planet_id(&simulation, system_id);
        let mut state = simulation.state().clone();
        // The underlying data objectively contains a hostile occupant, but
        // the player has no discovery of this system at all: the function
        // must not "peek" at planetary_intelligence_reports regardless.
        let hostile_faction = simulation
            .state()
            .factions
            .iter()
            .map(|faction| faction.id)
            .find(|id| *id != state.player_faction)
            .expect("universe has another faction");
        state
            .diplomacy
            .set_relation(
                state.player_faction,
                hostile_faction,
                DiplomaticRelation::Hostile,
            )
            .unwrap();
        state
            .planetary_intelligence_reports
            .push(intelligence_report(
                planet_id,
                PlanetaryOccupancyIntel::Occupied(hostile_faction),
            ));

        let tint = system_territory_tint(&state, simulation.universe(), system_id);
        assert_eq!(tint, None);
    }

    #[test]
    fn an_occupied_planet_with_unknown_diplomatic_relation_never_produces_a_misleading_tint() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let system_id = other_system_id(&simulation);
        let universe = simulation.universe_repository().clone();
        let planet_id = first_planet_id(&simulation, system_id);

        let mut state = simulation.state().clone();
        state.advance_system_knowledge(&universe, system_id, KnowledgeLevel::Colonized);
        assert_eq!(
            state.system_visibility(system_id),
            Some(SystemVisibility::Known)
        );

        // The default ruleset gives every predefined faction an explicit
        // relation (Neutral / Hostile) — push a synthetic faction with no
        // relation entry at all, so `relation_between` genuinely falls back
        // to `DiplomacyState::default_relation` (Unknown in the ruleset),
        // rather than assuming which of the two existing factions happens
        // to be unconfigured.
        let unknown_faction = FactionId::new(9_999);
        state.factions.push(FactionData {
            id: unknown_faction,
            name: "Contact non identifié".to_string(),
            kind: FactionKind::Neutral,
            active: true,
        });
        assert_eq!(
            state.relation_between(state.player_faction, unknown_faction),
            Ok(DiplomaticRelation::Unknown)
        );
        state
            .planetary_intelligence_reports
            .push(intelligence_report(
                planet_id,
                PlanetaryOccupancyIntel::Occupied(unknown_faction),
            ));

        let tint = system_territory_tint(&state, simulation.universe(), system_id);
        assert_eq!(tint, Some(TerritoryTint::Neutral));
    }

    #[test]
    fn a_player_colony_is_always_self_owned_regardless_of_intelligence_reports() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let home_colony = simulation
            .state()
            .active_player_colony()
            .expect("home colony exists");
        let system_id = home_colony.system_id;

        let tint = system_territory_tint(simulation.state(), simulation.universe(), system_id);
        assert_eq!(tint, Some(TerritoryTint::SelfOwned));
    }

    #[test]
    fn unidentified_presence_is_distinct_from_neutral() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let system_id = other_system_id(&simulation);
        let universe = simulation.universe_repository().clone();
        let planet_id = first_planet_id(&simulation, system_id);

        let mut state = simulation.state().clone();
        state.advance_system_knowledge(&universe, system_id, KnowledgeLevel::Colonized);
        state
            .planetary_intelligence_reports
            .push(intelligence_report(
                planet_id,
                PlanetaryOccupancyIntel::OccupiedUnknown,
            ));

        let tint = system_territory_tint(&state, simulation.universe(), system_id);
        assert_eq!(tint, Some(TerritoryTint::UnidentifiedPresence));
        assert_ne!(tint, Some(TerritoryTint::Neutral));
    }
}

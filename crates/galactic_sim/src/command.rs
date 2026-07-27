use galactic_domain::{ColonyId, FactionId, PlanetId, SystemId};

use crate::{BuildingKind, CraftableId, StrategicTick, TechnologyId, TimeSpeed};

/// An action requested from the deterministic simulation.
///
/// Actions contain no implicit player identity. The issuing faction and tick
/// live in [`GameCommand`], so a future AI can use the exact same input path as
/// the player UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameAction {
    TogglePause,
    SetSpeed(TimeSpeed),
    SelectSystem(SystemId),
    SelectPlanet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
    ClearSelection,
    QueueBuildingUpgrade {
        colony_id: ColonyId,
        kind: BuildingKind,
    },
    QueueResearch {
        technology: TechnologyId,
    },
    QueueCraft {
        colony_id: ColonyId,
        craftable: CraftableId,
    },
    /// Temporary validation action until the probe mission loop is added.
    DebugAdvanceSelectedKnowledge,
}

/// Generic command envelope shared by player and future AI command sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameCommand {
    pub issuer: FactionId,
    pub issued_at: StrategicTick,
    pub action: GameAction,
}

impl GameCommand {
    pub const fn new(issuer: FactionId, issued_at: StrategicTick, action: GameAction) -> Self {
        Self {
            issuer,
            issued_at,
            action,
        }
    }
}

/// Extension point for future player, replay, network or AI command producers.
///
/// The simulation does not poll command sources by itself. Inactive seeded
/// factions therefore remain data-only until a later MVP explicitly wires a
/// source into the game loop.
pub trait CommandSource {
    fn faction_id(&self) -> FactionId;

    fn actions_for_tick(&mut self, tick: StrategicTick, output: &mut Vec<GameAction>);
}

pub fn commands_from_source(
    source: &mut impl CommandSource,
    tick: StrategicTick,
) -> Vec<GameCommand> {
    let mut actions = Vec::new();
    source.actions_for_tick(tick, &mut actions);
    actions
        .into_iter()
        .map(|action| GameCommand::new(source.faction_id(), tick, action))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRejection {
    UnknownIssuer(FactionId),
    InactiveIssuer(FactionId),
    TickMismatch {
        expected: StrategicTick,
        found: StrategicTick,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DormantAiSource {
        faction_id: FactionId,
    }

    impl CommandSource for DormantAiSource {
        fn faction_id(&self) -> FactionId {
            self.faction_id
        }

        fn actions_for_tick(&mut self, _tick: StrategicTick, output: &mut Vec<GameAction>) {
            output.push(GameAction::SetSpeed(TimeSpeed::X2));
        }
    }

    #[test]
    fn a_future_source_builds_the_same_command_envelope() {
        let mut source = DormantAiSource {
            faction_id: FactionId::new(2),
        };
        let tick = StrategicTick::new(42);

        let commands = commands_from_source(&mut source, tick);

        assert_eq!(
            commands,
            vec![GameCommand::new(
                FactionId::new(2),
                tick,
                GameAction::SetSpeed(TimeSpeed::X2),
            )],
        );
    }
}

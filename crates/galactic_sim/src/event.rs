use galactic_domain::{FactionId, PlanetId, SystemId};

use crate::{
    ColonyProductionReport, CommandRejection, ConstructionCompleted, ConstructionQueued,
    ConstructionRejected, CraftCompleted, CraftQueued, CraftRejected, FleetCreated,
    FleetCreationRejected, KnowledgeChange, MissionCancellationRejected, MissionLaunchRejected,
    MissionLaunched, MissionReport, MissionResolution, MissionTransition, PlanetAnalysisRejected,
    PlanetAnalysisReport, ResearchCompleted, ResearchQueued, ResearchRejected, StrategicDuration,
    StrategicTick, TimeSpeed,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SelectionTarget {
    #[default]
    None,
    System(SystemId),
    Planet {
        system_id: SystemId,
        planet_id: PlanetId,
    },
}

/// Faction-addressed output from the deterministic simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameEvent {
    pub recipient: FactionId,
    pub occurred_at: StrategicTick,
    pub kind: GameEventKind,
}

impl GameEvent {
    pub const fn new(
        recipient: FactionId,
        occurred_at: StrategicTick,
        kind: GameEventKind,
    ) -> Self {
        Self {
            recipient,
            occurred_at,
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEventKind {
    CommandRejected(CommandRejection),
    SpeedChanged(TimeSpeed),
    SelectionChanged(SelectionTarget),
    KnowledgeChanged(KnowledgeChange),
    PlanetAnalyzed(PlanetAnalysisReport),
    PlanetAnalysisRejected(PlanetAnalysisRejected),
    TicksAdvanced {
        ticks: StrategicDuration,
        current_tick: StrategicTick,
    },
    ProductionRefreshed(ColonyProductionReport),
    ConstructionQueued(ConstructionQueued),
    ConstructionCompleted(ConstructionCompleted),
    ConstructionRejected(ConstructionRejected),
    ResearchQueued(ResearchQueued),
    ResearchCompleted(ResearchCompleted),
    ResearchRejected(ResearchRejected),
    CraftQueued(CraftQueued),
    CraftCompleted(CraftCompleted),
    CraftRejected(CraftRejected),
    FleetCreated(FleetCreated),
    FleetCreationRejected(FleetCreationRejected),
    MissionLaunched(MissionLaunched),
    MissionLaunchRejected(MissionLaunchRejected),
    MissionTransitioned(MissionTransition),
    MissionResolved(MissionResolution),
    MissionReported(MissionReport),
    MissionCancellationRejected(MissionCancellationRejected),
}

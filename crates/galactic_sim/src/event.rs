use galactic_domain::{ColonyId, FactionId, PlanetId, SystemId};

use crate::{
    ColonyEstablished, ColonyFoundation, ColonyProductionReport, ColonySelectionRejected,
    CommandRejection, ConstructionCancellationRejected, ConstructionCancelled,
    ConstructionCompleted, ConstructionQueued, ConstructionRejected, CraftCancellationRejected,
    CraftCancelled, CraftCompleted, CraftQueued, CraftRejected, FleetCreated,
    FleetCreationRejected, FleetDisbandRejected, FleetDisbanded, FleetRenameRejected, FleetRenamed,
    KnowledgeChange, MissionCancellationRejected, MissionLaunchRejected, MissionLaunched,
    MissionReport, MissionResolution, MissionTransition, PlanetAnalysisRejected,
    PlanetAnalysisReport, ResearchCancellationRejected, ResearchCancelled, ResearchCompleted,
    ResearchQueued, ResearchRejected, StrategicDuration, StrategicTick, TimeSpeed,
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
    ActiveColonyChanged(ColonyId),
    ActiveColonySelectionRejected(ColonySelectionRejected),
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
    ConstructionCancelled(ConstructionCancelled),
    ConstructionCancellationRejected(ConstructionCancellationRejected),
    ResearchQueued(ResearchQueued),
    ResearchCompleted(ResearchCompleted),
    ResearchRejected(ResearchRejected),
    ResearchCancelled(ResearchCancelled),
    ResearchCancellationRejected(ResearchCancellationRejected),
    CraftQueued(CraftQueued),
    CraftCompleted(CraftCompleted),
    CraftRejected(CraftRejected),
    CraftCancelled(CraftCancelled),
    CraftCancellationRejected(CraftCancellationRejected),
    FleetCreated(FleetCreated),
    FleetCreationRejected(FleetCreationRejected),
    FleetRenamed(FleetRenamed),
    FleetRenameRejected(FleetRenameRejected),
    FleetDisbanded(FleetDisbanded),
    FleetDisbandRejected(FleetDisbandRejected),
    MissionLaunched(MissionLaunched),
    MissionLaunchRejected(MissionLaunchRejected),
    MissionTransitioned(MissionTransition),
    MissionResolved(MissionResolution),
    ColonyFoundationPrepared(ColonyFoundation),
    ColonyEstablished(ColonyEstablished),
    MissionReported(MissionReport),
    MissionCancellationRejected(MissionCancellationRejected),
}

use galactic_domain::{PlanetId, SystemId};

use crate::{
    ColonyProductionReport, ConstructionCompleted, ConstructionQueued, ConstructionRejected,
    CraftCompleted, CraftQueued, CraftRejected, KnowledgeChange, ResearchCompleted, ResearchQueued,
    ResearchRejected, StrategicDuration, StrategicTick, TimeSpeed,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEvent {
    SpeedChanged(TimeSpeed),
    SelectionChanged(SelectionTarget),
    KnowledgeChanged(KnowledgeChange),
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
}

use galactic_domain::PlanetId;

use crate::{KnowledgeLevel, MissionTarget, PlanetAnalysisReport};

use super::{MissionPhase, MissionResult, MissionState, MissionStateError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalyzeMissionResult {
    pub planet_id: PlanetId,
    pub previous: KnowledgeLevel,
    pub current: KnowledgeLevel,
    pub report: PlanetAnalysisReport,
}

pub(crate) fn validate_analyze_result(mission: &MissionState) -> Result<(), MissionStateError> {
    match (mission.phase, mission.result) {
        (MissionPhase::Completed, None) => Err(MissionStateError::MissingAnalyzeResult),
        (
            MissionPhase::Preparation
            | MissionPhase::Outbound
            | MissionPhase::OnSite
            | MissionPhase::Returning
            | MissionPhase::Cancelled,
            Some(_),
        ) => Err(MissionStateError::UnexpectedMissionResult),
        (_, Some(MissionResult::Analyze(result))) => match mission.order.target {
            MissionTarget::Planet { planet_id, .. } if planet_id == result.planet_id => Ok(()),
            _ => Err(MissionStateError::AnalyzeResultTargetMismatch {
                expected: mission.order.target.planet_id(),
                found: result.planet_id,
            }),
        },
        (_, Some(_)) => Err(MissionStateError::UnexpectedMissionResult),
        (_, None) => Ok(()),
    }
}

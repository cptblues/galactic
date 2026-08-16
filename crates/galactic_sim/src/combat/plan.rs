use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::doctrine::CombatDoctrineId;
use super::state::{CombatSideState, CombatStackId, CombatTacticalRole};

pub const MAX_COMBAT_GROUPS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CombatGroupPlanId {
    Alpha,
    Beta,
    Gamma,
}

impl CombatGroupPlanId {
    pub const ALL: [Self; MAX_COMBAT_GROUPS] = [Self::Alpha, Self::Beta, Self::Gamma];

    pub const fn index(self) -> u64 {
        match self {
            Self::Alpha => 0,
            Self::Beta => 1,
            Self::Gamma => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatGroupRole {
    Assault,
    Screen,
    Bombardment,
    Reserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatTargetPriority {
    Any,
    Light,
    Medium,
    Heavy,
    Damaged,
    Support,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatGroupPlan {
    pub id: CombatGroupPlanId,
    pub stacks: Vec<CombatStackId>,
    pub role: CombatGroupRole,
    pub target_priority: CombatTargetPriority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatPlan {
    pub doctrine: CombatDoctrineId,
    pub groups: Vec<CombatGroupPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatIntervention {
    FocusFire { priority: CombatTargetPriority },
    CommitReserve { group_id: CombatGroupPlanId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatInterventionEffect {
    FocusFireApplied { priority: CombatTargetPriority },
    ReserveCommitted { group_id: CombatGroupPlanId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatInterventionRecord {
    pub round: u16,
    pub doctrine: CombatDoctrineId,
    pub intervention: CombatIntervention,
    pub command_point_cost: u8,
    pub effect: CombatInterventionEffect,
}

impl CombatInterventionRecord {
    pub(crate) const fn new(
        round: u16,
        doctrine: CombatDoctrineId,
        intervention: CombatIntervention,
        command_point_cost: u8,
    ) -> Self {
        Self {
            round,
            doctrine,
            intervention,
            command_point_cost,
            effect: match intervention {
                CombatIntervention::FocusFire { priority } => {
                    CombatInterventionEffect::FocusFireApplied { priority }
                }
                CombatIntervention::CommitReserve { group_id } => {
                    CombatInterventionEffect::ReserveCommitted { group_id }
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatPlanValidationError {
    EmptyPlan,
    TooManyGroups { found: usize, maximum: usize },
    DuplicateGroup(CombatGroupPlanId),
    EmptyGroup(CombatGroupPlanId),
    UnknownStack(CombatStackId),
    DuplicateStack(CombatStackId),
    MissingStack(CombatStackId),
}

impl CombatPlan {
    pub(crate) fn default_for_side(side: &CombatSideState, doctrine: CombatDoctrineId) -> Self {
        let mut assault = Vec::new();
        let mut support = Vec::new();
        for stack in side
            .stacks
            .iter()
            .filter(|stack| stack.surviving_quantity > 0)
        {
            match stack.tactical_role {
                CombatTacticalRole::Line => assault.push(stack.stack_id),
                CombatTacticalRole::Support => support.push(stack.stack_id),
            }
        }

        let mut groups = Vec::new();
        if !assault.is_empty() {
            groups.push(CombatGroupPlan {
                id: CombatGroupPlanId::Alpha,
                stacks: assault,
                role: CombatGroupRole::Assault,
                target_priority: CombatTargetPriority::Any,
            });
        }
        if !support.is_empty() {
            groups.push(CombatGroupPlan {
                id: CombatGroupPlanId::Beta,
                stacks: support,
                role: CombatGroupRole::Screen,
                target_priority: CombatTargetPriority::Support,
            });
        }

        Self { doctrine, groups }
    }

    pub(crate) fn validate_for_side(
        &self,
        side: &CombatSideState,
    ) -> Result<(), CombatPlanValidationError> {
        if self.groups.is_empty() {
            return Err(CombatPlanValidationError::EmptyPlan);
        }
        if self.groups.len() > MAX_COMBAT_GROUPS {
            return Err(CombatPlanValidationError::TooManyGroups {
                found: self.groups.len(),
                maximum: MAX_COMBAT_GROUPS,
            });
        }

        let known: BTreeSet<CombatStackId> = side
            .stacks
            .iter()
            .filter(|stack| stack.surviving_quantity > 0)
            .map(|stack| stack.stack_id)
            .collect();
        let mut seen_groups = BTreeSet::new();
        let mut seen_stacks = BTreeSet::new();

        for group in &self.groups {
            if !seen_groups.insert(group.id) {
                return Err(CombatPlanValidationError::DuplicateGroup(group.id));
            }
            if group.stacks.is_empty() {
                return Err(CombatPlanValidationError::EmptyGroup(group.id));
            }
            for &stack_id in &group.stacks {
                if !known.contains(&stack_id) {
                    return Err(CombatPlanValidationError::UnknownStack(stack_id));
                }
                if !seen_stacks.insert(stack_id) {
                    return Err(CombatPlanValidationError::DuplicateStack(stack_id));
                }
            }
        }

        for stack_id in known {
            if !seen_stacks.contains(&stack_id) {
                return Err(CombatPlanValidationError::MissingStack(stack_id));
            }
        }

        Ok(())
    }

    pub(crate) fn group_for_stack(&self, stack_id: CombatStackId) -> Option<&CombatGroupPlan> {
        self.groups
            .iter()
            .find(|group| group.stacks.contains(&stack_id))
    }

    pub(crate) fn group_mut(&mut self, id: CombatGroupPlanId) -> Option<&mut CombatGroupPlan> {
        self.groups.iter_mut().find(|group| group.id == id)
    }
}

#[cfg(test)]
mod tests {
    use galactic_domain::{FactionId, Owner};

    use super::super::state::{CombatSideState, CombatStackState, CombatUnitRef, effective_hull};
    use super::*;
    use crate::{CombatTargetBonuses, CombatTargetClass, CraftableId, combat_rules};

    fn stack(id: u32, role: CombatTacticalRole) -> CombatStackState {
        let maximum_hull = effective_hull(10, 10, combat_rules().defense_weight_per_mille);
        CombatStackState {
            stack_id: CombatStackId(id),
            source: CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK),
            initial_quantity: 1,
            surviving_quantity: 1,
            current_hull: maximum_hull,
            maximum_hull,
            offense: 10,
            defense: 10,
            durability: 10,
            target_class: CombatTargetClass::Medium,
            bonuses: CombatTargetBonuses::default(),
            tactical_role: role,
        }
    }

    fn side() -> CombatSideState {
        CombatSideState {
            owner: Owner::Faction(FactionId::new(0)),
            stacks: vec![
                stack(0, CombatTacticalRole::Line),
                stack(1, CombatTacticalRole::Support),
            ],
            last_doctrine: None,
            consecutive_doctrine_uses: 0,
            retreated: false,
        }
    }

    #[test]
    fn default_plan_covers_every_operational_stack_once() {
        let side = side();
        let plan = CombatPlan::default_for_side(&side, CombatDoctrineId::BalancedEngagement);

        assert_eq!(plan.doctrine, CombatDoctrineId::BalancedEngagement);
        assert!(plan.validate_for_side(&side).is_ok());
        assert_eq!(plan.groups.len(), 2);
    }

    #[test]
    fn duplicate_stack_is_rejected() {
        let side = side();
        let plan = CombatPlan {
            doctrine: CombatDoctrineId::BalancedEngagement,
            groups: vec![CombatGroupPlan {
                id: CombatGroupPlanId::Alpha,
                stacks: vec![CombatStackId(0), CombatStackId(0), CombatStackId(1)],
                role: CombatGroupRole::Assault,
                target_priority: CombatTargetPriority::Any,
            }],
        };

        assert_eq!(
            plan.validate_for_side(&side),
            Err(CombatPlanValidationError::DuplicateStack(CombatStackId(0)))
        );
    }

    #[test]
    fn missing_stack_is_rejected() {
        let side = side();
        let plan = CombatPlan {
            doctrine: CombatDoctrineId::BalancedEngagement,
            groups: vec![CombatGroupPlan {
                id: CombatGroupPlanId::Alpha,
                stacks: vec![CombatStackId(0)],
                role: CombatGroupRole::Assault,
                target_priority: CombatTargetPriority::Any,
            }],
        };

        assert_eq!(
            plan.validate_for_side(&side),
            Err(CombatPlanValidationError::MissingStack(CombatStackId(1)))
        );
    }
}

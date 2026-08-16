// COMBAT-001-C: pending-combat orchestration layer — the interactive
// counterpart to `resolve_combat`'s synchronous auto-resolve façade. Calls
// into `state`/`rounds`/`ai` (never the other way), mirroring how B's
// `intel`/`ai` modules are pure leaves consumed by `combat.rs`.

use galactic_domain::{FactionId, FleetId, MissionId, PlanetId};

use crate::{AttackMissionResult, AuthorizationError, GameState, StrategicTick};
use crate::{combat_rules, default_ruleset};

use super::ai::choose_ai_plan;
use super::doctrine::CombatDoctrineId;
use super::rounds::{apply_retreat_penalty, apply_round_resolution, resolve_combat_round};
use super::state::{
    CombatSide, FleetCombatPhase, FleetCombatState, finalize_fleet_combat, retreat_side,
};
use super::{
    AttackMissionCommitment, CombatCommandRules, CombatFleetSnapshot, CombatGroupPlanId,
    CombatGroupRole, CombatIntervention, CombatInterventionRecord, CombatPlan,
    CombatPlanValidationError, CombatTargetPriority, PlanetDefenseSnapshot,
    apply_combat_resolution, intel_inputs_from_state, run_combat_to_completion,
};

fn default_command_points_remaining() -> u8 {
    combat_rules().command().starting_points()
}

/// A tactical combat that has begun (the attacker fleet arrived, the target
/// was still valid) but has not yet finalized — the persisted, save/load-safe
/// counterpart to a `FleetCombatState` that used to live only for the
/// duration of a single `resolve_combat` call. One entry per in-flight
/// attack mission; several can coexist (doc §11, "plusieurs combats
/// simultanés").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PendingCombat {
    pub mission_id: MissionId,
    pub planet_id: PlanetId,
    pub(crate) fleet_id: FleetId,
    /// Frozen commitment snapshot — reused unchanged at finalization, exactly
    /// like `resolve_and_apply_attack` already reuses `commitment.attacker`/
    /// `commitment.defender` today.
    pub(crate) attacker: CombatFleetSnapshot,
    pub(crate) defender: PlanetDefenseSnapshot,
    pub(crate) seed: u64,
    pub(crate) state: FleetCombatState,
    #[serde(default)]
    pub(crate) initial_plan: Option<CombatPlan>,
    #[serde(default)]
    pub(crate) plan: Option<CombatPlan>,
    #[serde(default)]
    pub(crate) intervention_history: Vec<CombatInterventionRecord>,
    #[serde(default = "default_command_points_remaining")]
    pub(crate) command_points_remaining: u8,
}

impl PendingCombat {
    /// The round about to be played, i.e. the value `GameAction::
    /// ChooseCombatDoctrine{round}` must carry to be accepted — COMBAT-001-D
    /// (the combat UI) reads this directly rather than trusting the `round`
    /// field of a `CombatDecisionRequired` event, which is always `1`
    /// regardless of how many rounds have actually been played.
    pub fn round(&self) -> u16 {
        self.state.round
    }

    pub fn maximum_rounds(&self) -> u16 {
        self.state.maximum_rounds
    }

    /// Attacker's knowledge of the defender's composition, 0-100 — see
    /// `combat/intel.rs`'s module doc for why this is one-directional.
    pub fn intel_percent(&self) -> u8 {
        self.state.intel_percent
    }

    pub fn plan(&self) -> Option<&CombatPlan> {
        self.plan.as_ref()
    }

    pub fn command_points_remaining(&self) -> u8 {
        self.command_points_remaining
    }

    pub fn command_points_maximum(&self) -> u8 {
        combat_rules().command().starting_points()
    }

    /// Save-integrity accessor for `reconstruction.rs`'s validation pass —
    /// `combat::state`'s own `FleetCombatPhase` stays `pub(crate)` (a
    /// `PendingCombat` still present in `state.pending_combats` is always
    /// `AwaitingDoctrine` in practice — finalizing removes the entry in the
    /// same atomic step), so this exposes only the boolean reconstruction.rs
    /// actually needs instead of widening that boundary.
    pub(crate) fn is_completed(&self) -> bool {
        self.state.phase == FleetCombatPhase::Completed
    }

    /// `true` if every group id on both sides is unique — a corrupted or
    /// hand-edited save could otherwise collide two groups under one id.
    pub(crate) fn has_unique_stack_ids(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.state
            .attacker
            .stacks
            .iter()
            .chain(self.state.defender.stacks.iter())
            .all(|stack| seen.insert(stack.stack_id))
    }

    /// `true` if every group's current hull sits within its own maximum —
    /// the same invariant the engine itself maintains every round (A), here
    /// re-checked defensively against whatever a loaded save actually
    /// contains.
    pub(crate) fn every_stack_hull_within_bounds(&self) -> bool {
        self.state
            .attacker
            .stacks
            .iter()
            .chain(self.state.defender.stacks.iter())
            .all(|stack| stack.current_hull <= stack.maximum_hull)
    }

    pub(crate) fn command_points_are_valid(&self) -> bool {
        self.command_points_remaining <= self.command_points_maximum()
    }
}

/// Starts a new pending combat from a validated commitment and pushes it
/// onto `state.pending_combats`. The caller (`mission.rs`'s
/// `Outbound → OnSite` handling) has already confirmed the target is still
/// valid — an invalid target never reaches this function (doc §11: "si la
/// cible devient invalide avant l'arrivée, aucun écran tactique ne doit être
/// créé").
pub(crate) fn begin_pending_combat(
    state: &mut GameState,
    mission_id: MissionId,
    planet_id: PlanetId,
    fleet_id: FleetId,
    resolved_at: StrategicTick,
    commitment: &AttackMissionCommitment,
) {
    let (intel_precision, intel_report_age_ticks) =
        intel_inputs_from_state(state, planet_id, resolved_at);
    let fleet_combat_state = super::state::prepare_fleet_combat(
        &commitment.attacker,
        &commitment.defender,
        commitment.seed,
        combat_rules(),
        default_ruleset().planetary_presence(),
        intel_precision,
        intel_report_age_ticks,
    )
    .expect("a commitment valid enough to reach begin_pending_combat always prepares successfully");
    let plan = CombatPlan::default_for_side(
        &fleet_combat_state.attacker,
        CombatDoctrineId::BalancedEngagement,
    );
    state.pending_combats.push(PendingCombat {
        mission_id,
        planet_id,
        fleet_id,
        attacker: commitment.attacker.clone(),
        defender: commitment.defender.clone(),
        seed: commitment.seed,
        state: fleet_combat_state,
        initial_plan: Some(plan.clone()),
        plan: Some(plan),
        intervention_history: Vec::new(),
        command_points_remaining: combat_rules().command().starting_points(),
    });
}

/// Every way a `ChooseCombatDoctrine`/`RetreatFromCombat`/`AutoResolveCombat`
/// command can fail (doc §10's constraints, all in one shared type since the
/// three commands share the same failure surface): no combat is pending for
/// that mission (also covers "already finished" — finalizing removes the
/// entry, replacing the old `CombatApplicationError::AlreadyApplied` guard);
/// the issuer doesn't control the attacking side; the requested round is
/// stale (only `ChooseCombatDoctrine` can produce this one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatCommandError {
    UnknownCombat(MissionId),
    Access(AuthorizationError),
    StaleRound { expected: u16, found: u16 },
    InvalidPlan(CombatPlanValidationError),
    InsufficientCommandPoints { required: u8, available: u8 },
    InvalidIntervention(CombatInterventionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatInterventionError {
    InvalidFocusPriority,
    NoActiveGroup,
    ReserveGroupMissing(CombatGroupPlanId),
    ReserveGroupEmpty(CombatGroupPlanId),
    GroupIsNotReserve(CombatGroupPlanId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatDecisionRequired {
    pub mission_id: MissionId,
    pub planet_id: PlanetId,
    pub round: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatRoundResolved {
    pub mission_id: MissionId,
    pub round: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatIntelUpdated {
    pub mission_id: MissionId,
    pub intel_percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatCompleted {
    pub mission_id: MissionId,
    pub result: AttackMissionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatDoctrineRejected {
    pub mission_id: MissionId,
    pub round: u16,
    pub doctrine: Option<CombatDoctrineId>,
    pub intervention: Option<CombatIntervention>,
    pub error: CombatCommandError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatRetreatRejected {
    pub mission_id: MissionId,
    pub error: CombatCommandError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatAutoResolveRejected {
    pub mission_id: MissionId,
    pub error: CombatCommandError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatPlanConfirmed {
    pub mission_id: MissionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatPlanRejected {
    pub mission_id: MissionId,
    pub error: CombatCommandError,
}

/// What `choose_combat_doctrine` produced, in enough detail for the caller
/// (`Simulation::apply_command`) to build every `GameEventKind` doc §10
/// expects from a single round: the round just resolved, whether intel moved
/// (`CombatIntelUpdated` is only emitted when it did), and — if that round
/// happened to finish the combat — the terminal result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CombatDoctrineOutcome {
    pub(crate) round: u16,
    pub(crate) intel_before: u8,
    pub(crate) intel_after: u8,
    pub(crate) completed: Option<AttackMissionResult>,
}

/// Finalizes a `PendingCombat` whose `FleetCombatState` has already reached
/// `Completed` — the shared tail of all three commands once their engine
/// work is done. Removes the entry via `apply_combat_resolution`.
fn finalize_pending_combat(state: &mut GameState, mission_id: MissionId) -> AttackMissionResult {
    let pending = state
        .pending_combat(mission_id)
        .expect("the caller only finalizes a combat it just confirmed is pending")
        .clone();
    let rules = combat_rules();
    let resolution = finalize_fleet_combat(
        &pending.state,
        pending.attacker.cargo,
        pending.attacker.cargo_capacity,
        rules,
    )
    .expect("a completed combat always finalizes");
    let resolved_at = state.clock.current_tick();
    let (_, result) = apply_combat_resolution(
        state,
        mission_id,
        resolved_at,
        pending.fleet_id,
        pending.planet_id,
        &pending.attacker,
        &pending.defender,
        pending.seed,
        resolution,
        pending.state.history.clone(),
        pending.initial_plan.clone(),
        pending.plan.clone(),
        pending.intervention_history.clone(),
    )
    .expect("a validated pending combat always finalizes");
    result
}

pub(crate) fn confirm_combat_plan(
    state: &mut GameState,
    actor: FactionId,
    mission_id: MissionId,
    plan: CombatPlan,
) -> Result<(), CombatCommandError> {
    let pending = state
        .pending_combat(mission_id)
        .ok_or(CombatCommandError::UnknownCombat(mission_id))?;
    state
        .authorize_management(actor, pending.attacker.owner)
        .map_err(CombatCommandError::Access)?;
    plan.validate_for_side(&pending.state.attacker)
        .map_err(CombatCommandError::InvalidPlan)?;

    let pending_mut = state.pending_combat_mut(mission_id).expect("checked above");
    pending_mut.plan = Some(plan);

    Ok(())
}

struct PreparedRoundCommand {
    persistent_plan: CombatPlan,
    round_plan: CombatPlan,
    command_point_cost: u8,
    intervention_record: Option<CombatInterventionRecord>,
}

fn default_or_persisted_plan(pending: &PendingCombat) -> CombatPlan {
    pending.plan.clone().unwrap_or_else(|| {
        CombatPlan::default_for_side(
            &pending.state.attacker,
            CombatDoctrineId::BalancedEngagement,
        )
    })
}

fn add_command_point_cost(total: &mut u8, cost: u8) {
    *total = total.saturating_add(cost);
}

fn prepare_round_command(
    pending: &PendingCombat,
    doctrine: Option<CombatDoctrineId>,
    intervention: Option<CombatIntervention>,
    command_rules: &CombatCommandRules,
) -> Result<PreparedRoundCommand, CombatCommandError> {
    let mut persistent_plan = default_or_persisted_plan(pending);
    let mut command_point_cost = 0_u8;
    let mut intervention_cost = 0_u8;

    if let Some(doctrine) = doctrine
        && persistent_plan.doctrine != doctrine
    {
        add_command_point_cost(
            &mut command_point_cost,
            command_rules.change_doctrine_cost(),
        );
        persistent_plan.doctrine = doctrine;
    }

    let mut round_plan = persistent_plan.clone();

    match intervention {
        None => {}
        Some(CombatIntervention::FocusFire { priority }) => {
            if priority == CombatTargetPriority::Any {
                return Err(CombatCommandError::InvalidIntervention(
                    CombatInterventionError::InvalidFocusPriority,
                ));
            }
            intervention_cost = command_rules.focus_fire_cost();
            add_command_point_cost(&mut command_point_cost, intervention_cost);
            let mut active_group_count = 0;
            for group in &mut round_plan.groups {
                if group.role != CombatGroupRole::Reserve && !group.stacks.is_empty() {
                    group.target_priority = priority;
                    active_group_count += 1;
                }
            }
            if active_group_count == 0 {
                return Err(CombatCommandError::InvalidIntervention(
                    CombatInterventionError::NoActiveGroup,
                ));
            }
        }
        Some(CombatIntervention::CommitReserve { group_id }) => {
            intervention_cost = command_rules.commit_reserve_cost();
            add_command_point_cost(&mut command_point_cost, intervention_cost);
            let group = persistent_plan.group_mut(group_id).ok_or(
                CombatCommandError::InvalidIntervention(
                    CombatInterventionError::ReserveGroupMissing(group_id),
                ),
            )?;
            if group.stacks.is_empty() {
                return Err(CombatCommandError::InvalidIntervention(
                    CombatInterventionError::ReserveGroupEmpty(group_id),
                ));
            }
            if group.role != CombatGroupRole::Reserve {
                return Err(CombatCommandError::InvalidIntervention(
                    CombatInterventionError::GroupIsNotReserve(group_id),
                ));
            }
            group.role = CombatGroupRole::Assault;
            round_plan = persistent_plan.clone();
        }
    }

    if command_point_cost > pending.command_points_remaining {
        return Err(CombatCommandError::InsufficientCommandPoints {
            required: command_point_cost,
            available: pending.command_points_remaining,
        });
    }

    persistent_plan
        .validate_for_side(&pending.state.attacker)
        .map_err(CombatCommandError::InvalidPlan)?;
    round_plan
        .validate_for_side(&pending.state.attacker)
        .map_err(CombatCommandError::InvalidPlan)?;

    Ok(PreparedRoundCommand {
        intervention_record: intervention.map(|intervention| {
            CombatInterventionRecord::new(
                pending.state.round.saturating_add(1),
                round_plan.doctrine,
                intervention,
                intervention_cost,
            )
        }),
        persistent_plan,
        round_plan,
        command_point_cost,
    })
}

/// Resolves exactly one round for the player-controlled attacker, mirroring
/// the shape of every other `apply_command` arm (`Ok` → success event(s),
/// `Err` → a rejection event). The persisted plan is the default; the command
/// may optionally change doctrine and/or spend command points on a one-round
/// intervention before resolution.
pub(crate) fn choose_combat_doctrine(
    state: &mut GameState,
    actor: FactionId,
    mission_id: MissionId,
    round: u16,
    doctrine: Option<CombatDoctrineId>,
    intervention: Option<CombatIntervention>,
) -> Result<CombatDoctrineOutcome, CombatCommandError> {
    let rules = combat_rules();
    let pending = state
        .pending_combat(mission_id)
        .ok_or(CombatCommandError::UnknownCombat(mission_id))?;
    state
        .authorize_management(actor, pending.attacker.owner)
        .map_err(CombatCommandError::Access)?;
    let pending = state.pending_combat(mission_id).expect("checked above");
    let expected_round = pending.state.round.saturating_add(1);
    if round != expected_round {
        return Err(CombatCommandError::StaleRound {
            expected: expected_round,
            found: round,
        });
    }
    let intel_before = pending.state.intel_percent;
    let defender_plan = choose_ai_plan(
        &pending.state.defender,
        &pending.state.attacker,
        round,
        rules.ai(),
    );
    let prepared = prepare_round_command(pending, doctrine, intervention, rules.command())?;
    let attacker_doctrine = prepared.round_plan.doctrine;
    let round_plan = prepared.round_plan;

    let pending_mut = state.pending_combat_mut(mission_id).expect("checked above");
    pending_mut.command_points_remaining -= prepared.command_point_cost;
    pending_mut.plan = Some(prepared.persistent_plan);
    if let Some(record) = prepared.intervention_record {
        pending_mut.intervention_history.push(record);
    }
    let resolution = resolve_combat_round(
        &pending_mut.state,
        attacker_doctrine,
        defender_plan.doctrine,
        Some(&round_plan),
        Some(&defender_plan),
        rules,
        rules.tactics(),
    )
    .expect("a pending combat awaiting a doctrine is never already completed");
    apply_round_resolution(&mut pending_mut.state, resolution)
        .expect("a pending combat awaiting a doctrine is never already completed");

    let new_round = pending_mut.state.round;
    let intel_after = pending_mut.state.intel_percent;
    let is_completed = pending_mut.state.phase == FleetCombatPhase::Completed;

    let completed = is_completed.then(|| finalize_pending_combat(state, mission_id));

    Ok(CombatDoctrineOutcome {
        round: new_round,
        intel_before,
        intel_after,
        completed,
    })
}

/// Retreats the attacker (the only side a real player controls today — see
/// this module's doc), applying the configured damage penalty before
/// finalizing (doc §16: retreating "peut subir une pénalité configurable").
pub(crate) fn retreat_from_combat(
    state: &mut GameState,
    actor: FactionId,
    mission_id: MissionId,
) -> Result<AttackMissionResult, CombatCommandError> {
    let rules = combat_rules();
    let attacker_owner = state
        .pending_combat(mission_id)
        .ok_or(CombatCommandError::UnknownCombat(mission_id))?
        .attacker
        .owner;
    state
        .authorize_management(actor, attacker_owner)
        .map_err(CombatCommandError::Access)?;

    let pending_mut = state.pending_combat_mut(mission_id).expect("checked above");
    apply_retreat_penalty(
        &mut pending_mut.state,
        CombatSide::Attacker,
        rules,
        rules.retreat(),
    );
    retreat_side(&mut pending_mut.state, CombatSide::Attacker)
        .expect("a pending combat is never already completed while a player can still retreat");

    Ok(finalize_pending_combat(state, mission_id))
}

/// Runs the same engine `resolve_combat` uses (`run_combat_to_completion`,
/// `combat.rs`) to completion on an already-started `PendingCombat`,
/// satisfying doc §21's "l'auto-résolution utilise le même moteur" from the
/// interactive entry point too, not just the legacy synchronous façade.
pub(crate) fn auto_resolve_combat(
    state: &mut GameState,
    actor: FactionId,
    mission_id: MissionId,
) -> Result<AttackMissionResult, CombatCommandError> {
    let rules = combat_rules();
    let attacker_owner = state
        .pending_combat(mission_id)
        .ok_or(CombatCommandError::UnknownCombat(mission_id))?
        .attacker
        .owner;
    state
        .authorize_management(actor, attacker_owner)
        .map_err(CombatCommandError::Access)?;

    let pending_mut = state.pending_combat_mut(mission_id).expect("checked above");
    run_combat_to_completion(&mut pending_mut.state, rules);

    Ok(finalize_pending_combat(state, mission_id))
}

#[cfg(test)]
mod tests {
    use galactic_domain::{ColonyId, FactionId, Owner, ResourceStock, UniverseConfig};

    use super::*;
    use crate::{
        CraftableId, FleetComposition, FleetLocation, FleetState, ShipStack, Simulation,
        prepare_attack_commitment,
    };

    fn fixture_state_and_commitment() -> (GameState, AttackMissionCommitment) {
        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                presence.occupant == Owner::Faction(FactionId::new(2))
                    && !presence.forces.is_empty()
            })
            .expect("a hostile target exists")
            .planet_id;
        let colony_id = ColonyId::new(0);
        let composition =
            FleetComposition::from_stacks([ShipStack::new(CraftableId::FRIGATE_BULWARK, 3)])
                .expect("combat composition");
        simulation.state_mut().fleets.push(FleetState {
            id: FleetId::new(0),
            name: "Force de test".to_string(),
            owner: Owner::Faction(FactionId::new(0)),
            location: FleetLocation::Docked(colony_id),
            composition,
            cargo: ResourceStock::ZERO,
            assignment: crate::FleetAssignment::Idle,
        });
        simulation.state_mut().next_fleet_id = 1;
        let commitment = prepare_attack_commitment(simulation.state(), FleetId::new(0), target, 91)
            .expect("attack snapshot");
        (simulation.state().clone(), commitment)
    }

    #[test]
    fn begin_pending_combat_creates_an_entry_awaiting_the_first_doctrine() {
        let (mut state, commitment) = fixture_state_and_commitment();
        let mission_id = MissionId::new(1);

        begin_pending_combat(
            &mut state,
            mission_id,
            commitment.defender.planet_id,
            commitment.attacker.fleet_id,
            StrategicTick::ZERO,
            &commitment,
        );

        let pending = state.pending_combat(mission_id).expect("just created");
        assert_eq!(pending.mission_id, mission_id);
        assert_eq!(pending.planet_id, commitment.defender.planet_id);
        assert_eq!(pending.state.round, 0);
        assert!(pending.plan().is_some());
        assert_eq!(
            pending.command_points_remaining(),
            combat_rules().command().starting_points()
        );
        assert_eq!(
            pending.state.phase,
            super::super::state::FleetCombatPhase::AwaitingDoctrine
        );
    }

    #[test]
    fn multiple_pending_combats_coexist_independently() {
        let (mut state, commitment) = fixture_state_and_commitment();

        begin_pending_combat(
            &mut state,
            MissionId::new(1),
            commitment.defender.planet_id,
            commitment.attacker.fleet_id,
            StrategicTick::ZERO,
            &commitment,
        );
        begin_pending_combat(
            &mut state,
            MissionId::new(2),
            commitment.defender.planet_id,
            commitment.attacker.fleet_id,
            StrategicTick::ZERO,
            &commitment,
        );

        assert!(state.pending_combat(MissionId::new(1)).is_some());
        assert!(state.pending_combat(MissionId::new(2)).is_some());
        assert_eq!(state.pending_combats.len(), 2);
    }

    fn pending_combat_state() -> (GameState, FactionId, MissionId) {
        let (mut state, commitment) = fixture_state_and_commitment();
        let mission_id = MissionId::new(1);
        let actor = commitment
            .attacker
            .owner
            .faction()
            .expect("the fixture attacker is faction-owned");
        begin_pending_combat(
            &mut state,
            mission_id,
            commitment.defender.planet_id,
            commitment.attacker.fleet_id,
            StrategicTick::ZERO,
            &commitment,
        );
        (state, actor, mission_id)
    }

    #[test]
    fn choose_combat_doctrine_rejects_a_stale_round() {
        let (mut state, actor, mission_id) = pending_combat_state();
        assert_eq!(
            choose_combat_doctrine(
                &mut state,
                actor,
                mission_id,
                2,
                Some(CombatDoctrineId::BalancedEngagement),
                None,
            ),
            Err(CombatCommandError::StaleRound {
                expected: 1,
                found: 2,
            })
        );
        // The state is untouched by a rejected command.
        assert_eq!(state.pending_combat(mission_id).unwrap().state.round, 0);
    }

    #[test]
    fn confirm_combat_plan_accepts_a_valid_plan() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let plan = state
            .pending_combat(mission_id)
            .and_then(PendingCombat::plan)
            .expect("beginning combat creates a default plan")
            .clone();

        assert_eq!(
            confirm_combat_plan(&mut state, actor, mission_id, plan.clone()),
            Ok(())
        );
        assert_eq!(
            state.pending_combat(mission_id).unwrap().plan(),
            Some(&plan)
        );
    }

    #[test]
    fn confirm_combat_plan_rejects_an_invalid_plan() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let plan = CombatPlan {
            doctrine: CombatDoctrineId::BalancedEngagement,
            groups: Vec::new(),
        };

        assert_eq!(
            confirm_combat_plan(&mut state, actor, mission_id, plan),
            Err(CombatCommandError::InvalidPlan(
                CombatPlanValidationError::EmptyPlan
            ))
        );
    }

    #[test]
    fn choose_combat_doctrine_rejects_the_wrong_actor() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let wrong_actor = FactionId::new(999);
        state.factions.push(crate::FactionData {
            id: wrong_actor,
            name: "Rival de test".to_string(),
            kind: crate::FactionKind::Neutral,
            active: true,
        });
        assert_eq!(
            choose_combat_doctrine(
                &mut state,
                wrong_actor,
                mission_id,
                1,
                Some(CombatDoctrineId::BalancedEngagement),
                None,
            ),
            Err(CombatCommandError::Access(AuthorizationError::NotOwner {
                actor: wrong_actor,
                owner: actor,
            }))
        );
    }

    #[test]
    fn choosing_a_doctrine_twice_for_the_same_round_does_not_apply_two_rounds() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let first = choose_combat_doctrine(
            &mut state,
            actor,
            mission_id,
            1,
            Some(CombatDoctrineId::BalancedEngagement),
            None,
        )
        .expect("the first round resolves");
        assert_eq!(first.round, 1);

        // A resubmission carries the same (now stale) round number.
        assert_eq!(
            choose_combat_doctrine(
                &mut state,
                actor,
                mission_id,
                1,
                Some(CombatDoctrineId::BalancedEngagement),
                None,
            ),
            Err(CombatCommandError::StaleRound {
                expected: 2,
                found: 1,
            })
        );
    }

    #[test]
    fn continuing_the_current_plan_spends_no_command_points() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let starting_points = state
            .pending_combat(mission_id)
            .expect("combat starts pending")
            .command_points_remaining();

        choose_combat_doctrine(&mut state, actor, mission_id, 1, None, None)
            .expect("continuing the default plan resolves a round");

        let pending = state
            .pending_combat(mission_id)
            .expect("the fixture spans several rounds");
        assert_eq!(pending.command_points_remaining(), starting_points);
        assert_eq!(
            pending.plan().map(|plan| plan.doctrine),
            Some(CombatDoctrineId::BalancedEngagement)
        );
    }

    #[test]
    fn changing_doctrine_spends_and_persists_one_command_point() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let starting_points = state
            .pending_combat(mission_id)
            .expect("combat starts pending")
            .command_points_remaining();

        choose_combat_doctrine(
            &mut state,
            actor,
            mission_id,
            1,
            Some(CombatDoctrineId::ConcentratedAssault),
            None,
        )
        .expect("a doctrine change resolves a round");

        let pending = state
            .pending_combat(mission_id)
            .expect("the fixture spans several rounds");
        assert_eq!(pending.command_points_remaining(), starting_points - 1);
        assert_eq!(
            pending.plan().map(|plan| plan.doctrine),
            Some(CombatDoctrineId::ConcentratedAssault)
        );
    }

    #[test]
    fn focus_fire_spends_command_point_without_persisting_group_priority() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let original_priority = state
            .pending_combat(mission_id)
            .and_then(PendingCombat::plan)
            .and_then(|plan| plan.groups.first())
            .expect("default plan has an active group")
            .target_priority;

        choose_combat_doctrine(
            &mut state,
            actor,
            mission_id,
            1,
            None,
            Some(CombatIntervention::FocusFire {
                priority: CombatTargetPriority::Heavy,
            }),
        )
        .expect("focus fire resolves the round");

        let pending = state
            .pending_combat(mission_id)
            .expect("the fixture spans several rounds");
        assert_eq!(
            pending.command_points_remaining(),
            combat_rules().command().starting_points() - 1
        );
        assert_eq!(
            pending
                .plan()
                .and_then(|plan| plan.groups.first())
                .map(|group| group.target_priority),
            Some(original_priority)
        );
        assert_eq!(
            pending.intervention_history,
            vec![CombatInterventionRecord {
                round: 1,
                doctrine: CombatDoctrineId::BalancedEngagement,
                intervention: CombatIntervention::FocusFire {
                    priority: CombatTargetPriority::Heavy,
                },
                command_point_cost: combat_rules().command().focus_fire_cost(),
                effect: super::super::CombatInterventionEffect::FocusFireApplied {
                    priority: CombatTargetPriority::Heavy,
                },
            }]
        );
    }

    #[test]
    fn committing_reserve_spends_command_point_and_persists_group_role() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let mut plan = state
            .pending_combat(mission_id)
            .and_then(PendingCombat::plan)
            .expect("default plan exists")
            .clone();
        plan.groups[0].role = CombatGroupRole::Reserve;
        confirm_combat_plan(&mut state, actor, mission_id, plan).expect("reserve plan is valid");

        choose_combat_doctrine(
            &mut state,
            actor,
            mission_id,
            1,
            None,
            Some(CombatIntervention::CommitReserve {
                group_id: CombatGroupPlanId::Alpha,
            }),
        )
        .expect("committing reserve resolves the round");

        let pending = state
            .pending_combat(mission_id)
            .expect("the fixture spans several rounds");
        assert_eq!(
            pending.command_points_remaining(),
            combat_rules().command().starting_points() - 1
        );
        assert_eq!(
            pending
                .plan()
                .and_then(|plan| plan
                    .groups
                    .iter()
                    .find(|group| group.id == CombatGroupPlanId::Alpha))
                .map(|group| group.role),
            Some(CombatGroupRole::Assault)
        );
        assert_eq!(
            pending.intervention_history,
            vec![CombatInterventionRecord {
                round: 1,
                doctrine: CombatDoctrineId::BalancedEngagement,
                intervention: CombatIntervention::CommitReserve {
                    group_id: CombatGroupPlanId::Alpha,
                },
                command_point_cost: combat_rules().command().commit_reserve_cost(),
                effect: super::super::CombatInterventionEffect::ReserveCommitted {
                    group_id: CombatGroupPlanId::Alpha,
                },
            }]
        );
    }

    #[test]
    fn insufficient_command_points_reject_without_mutating_the_pending_round() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let original_plan = state
            .pending_combat(mission_id)
            .and_then(PendingCombat::plan)
            .expect("default plan exists")
            .clone();
        state
            .pending_combat_mut(mission_id)
            .expect("combat starts pending")
            .command_points_remaining = 0;

        assert_eq!(
            choose_combat_doctrine(
                &mut state,
                actor,
                mission_id,
                1,
                None,
                Some(CombatIntervention::FocusFire {
                    priority: CombatTargetPriority::Heavy,
                }),
            ),
            Err(CombatCommandError::InsufficientCommandPoints {
                required: combat_rules().command().focus_fire_cost(),
                available: 0,
            })
        );

        let pending = state.pending_combat(mission_id).expect("still pending");
        assert_eq!(pending.round(), 0);
        assert_eq!(pending.command_points_remaining(), 0);
        assert_eq!(pending.plan(), Some(&original_plan));
        assert!(pending.intervention_history.is_empty());
    }

    #[test]
    fn retreating_after_finalization_is_rejected() {
        let (mut state, actor, mission_id) = pending_combat_state();
        auto_resolve_combat(&mut state, actor, mission_id).expect("auto-resolves to completion");
        assert!(state.pending_combat(mission_id).is_none());

        assert_eq!(
            retreat_from_combat(&mut state, actor, mission_id),
            Err(CombatCommandError::UnknownCombat(mission_id))
        );
    }

    #[test]
    fn a_completed_combat_no_longer_appears_pending() {
        let (mut state, actor, mission_id) = pending_combat_state();
        let result = auto_resolve_combat(&mut state, actor, mission_id)
            .expect("auto-resolves to completion");
        assert!(state.pending_combat(mission_id).is_none());
        assert!(state.combat_report(mission_id).is_some());
        assert!(matches!(
            result.outcome,
            crate::AttackMissionOutcome::Resolved(_)
        ));
    }
}

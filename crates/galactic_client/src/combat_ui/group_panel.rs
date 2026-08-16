use bevy::prelude::*;
use galactic_domain::MissionId;
use galactic_sim::{
    CombatGroupPlan, CombatGroupPlanId, CombatGroupRole, CombatPlan, CombatStackId,
    CombatTargetPriority, GameAction, PendingCombat, allied_stacks,
};

use crate::{
    SimulationResource,
    presentation::{
        components::UiPointerBlocker,
        scene::{
            action_button_color, action_button_outline, panel_background, panel_outline,
            ui_text_font,
        },
        shortcuts::apply_simulation_command,
    },
};

use super::{CombatPreReportControls, CombatUiPhase, CombatUiState, combat_unit_name};

const MAX_DRAFT_STACK_ROWS: usize = 8;
pub(super) const COMBAT_PLAN_PANEL_HEIGHT_PX: f32 = 136.0;
pub(super) const COMBAT_PLAN_CONTENT_COLUMN_GAP_PX: f32 = 8.0;
pub(super) const STACK_ASSIGNMENT_WIDTH_PERCENT: f32 = 36.0;
pub(super) const GROUP_CARDS_WIDTH_PERCENT: f32 = 60.0;
pub(super) const DRAFT_GROUP_CARD_WIDTH_PERCENT: f32 = 32.0;
pub(super) const DRAFT_GROUP_CARD_GAP_PX: f32 = 6.0;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DraftGroup {
    id: CombatGroupPlanId,
    stacks: Vec<CombatStackId>,
    role: CombatGroupRole,
    target_priority: CombatTargetPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CombatPlanDraftGroupView<'a> {
    pub(super) id: CombatGroupPlanId,
    pub(super) stacks: &'a [CombatStackId],
    pub(super) role: CombatGroupRole,
    pub(super) target_priority: CombatTargetPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CombatPlanDraft {
    doctrine: galactic_sim::CombatDoctrineId,
    groups: Vec<DraftGroup>,
}

impl CombatPlanDraft {
    pub(super) fn from_pending(pending: &PendingCombat) -> Self {
        let allied = allied_stacks(pending);
        let live_stacks: Vec<CombatStackId> = allied
            .iter()
            .filter(|stack| stack.surviving_quantity > 0)
            .map(|stack| stack.stack_id)
            .collect();
        let source_plan = pending.plan();
        let doctrine = source_plan
            .map(|plan| plan.doctrine)
            .unwrap_or(galactic_sim::CombatDoctrineId::BalancedEngagement);
        let mut groups = CombatGroupPlanId::ALL
            .into_iter()
            .map(|id| {
                let source_group =
                    source_plan.and_then(|plan| plan.groups.iter().find(|group| group.id == id));
                DraftGroup {
                    id,
                    stacks: source_group
                        .map(|group| {
                            group
                                .stacks
                                .iter()
                                .copied()
                                .filter(|stack_id| live_stacks.contains(stack_id))
                                .collect()
                        })
                        .unwrap_or_default(),
                    role: source_group
                        .map(|group| group.role)
                        .unwrap_or_else(|| default_group_role(id)),
                    target_priority: source_group
                        .map(|group| group.target_priority)
                        .unwrap_or_else(|| default_group_priority(id)),
                }
            })
            .collect::<Vec<_>>();

        for stack_id in live_stacks {
            if !groups.iter().any(|group| group.stacks.contains(&stack_id))
                && let Some(alpha) = groups.first_mut()
            {
                alpha.stacks.push(stack_id);
            }
        }

        Self { doctrine, groups }
    }

    pub(super) fn to_plan(&self) -> CombatPlan {
        CombatPlan {
            doctrine: self.doctrine,
            groups: self
                .groups
                .iter()
                .filter(|group| !group.stacks.is_empty())
                .map(|group| CombatGroupPlan {
                    id: group.id,
                    stacks: group.stacks.clone(),
                    role: group.role,
                    target_priority: group.target_priority,
                })
                .collect(),
        }
    }

    pub(super) fn doctrine(&self) -> galactic_sim::CombatDoctrineId {
        self.doctrine
    }

    pub(super) fn groups(&self) -> impl Iterator<Item = CombatPlanDraftGroupView<'_>> {
        self.groups.iter().map(|group| CombatPlanDraftGroupView {
            id: group.id,
            stacks: &group.stacks,
            role: group.role,
            target_priority: group.target_priority,
        })
    }

    pub(super) fn selected_group(&self, stack_id: CombatStackId) -> Option<CombatGroupPlanId> {
        self.groups
            .iter()
            .find(|group| group.stacks.contains(&stack_id))
            .map(|group| group.id)
    }

    pub(super) fn assign_stack(&mut self, stack_id: CombatStackId, target: CombatGroupPlanId) {
        for group in &mut self.groups {
            group.stacks.retain(|candidate| *candidate != stack_id);
        }
        if let Some(group) = self.groups.iter_mut().find(|group| group.id == target) {
            group.stacks.push(stack_id);
        }
    }

    fn group(&self, id: CombatGroupPlanId) -> Option<&DraftGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    fn group_mut(&mut self, id: CombatGroupPlanId) -> Option<&mut DraftGroup> {
        self.groups.iter_mut().find(|group| group.id == id)
    }

    fn cycle_role(&mut self, id: CombatGroupPlanId) {
        if let Some(group) = self.group_mut(id) {
            group.role = next_group_role(group.role);
        }
    }

    fn cycle_priority(&mut self, id: CombatGroupPlanId) {
        if let Some(group) = self.group_mut(id) {
            group.target_priority = next_target_priority(group.target_priority);
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct CombatPlanDraftState {
    mission_id: Option<MissionId>,
    draft: Option<CombatPlanDraft>,
    selected_stack: Option<CombatStackId>,
    dirty: bool,
}

impl CombatPlanDraftState {
    pub(super) fn rebuild(&mut self, mission_id: MissionId, pending: &PendingCombat) {
        self.mission_id = Some(mission_id);
        self.draft = Some(CombatPlanDraft::from_pending(pending));
        self.selected_stack = None;
        self.dirty = false;
    }

    fn clear(&mut self) {
        self.mission_id = None;
        self.draft = None;
        self.selected_stack = None;
        self.dirty = false;
    }

    pub(super) fn draft(&self) -> Option<&CombatPlanDraft> {
        self.draft.as_ref()
    }
}

#[derive(Component)]
pub(super) struct CombatPlanPanelRoot;

#[derive(Component)]
pub(super) struct DraftStackRow(usize);

#[derive(Component)]
pub(super) struct DraftStackText(usize);

#[derive(Component)]
pub(super) struct DraftGroupText(CombatGroupPlanId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DraftAction {
    SelectStack(usize),
    AssignSelected(CombatGroupPlanId),
    CycleRole(CombatGroupPlanId),
    CyclePriority(CombatGroupPlanId),
    Confirm,
    Reset,
}

#[derive(Component, Clone, Copy)]
pub(super) struct DraftActionButton(DraftAction);

pub(super) fn spawn_group_panel(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(COMBAT_PLAN_PANEL_HEIGHT_PX),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            padding: UiRect::all(Val::Px(7.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        CombatPlanPanelRoot,
        CombatPreReportControls,
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|heading| {
                heading.spawn((
                    Text::new("PLAN DE BATAILLE"),
                    ui_text_font(12.0),
                    TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
                ));
                heading
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|actions| {
                        spawn_draft_button(actions, DraftAction::Reset, "Réinitialiser");
                        spawn_draft_button(actions, DraftAction::Confirm, "Confirmer le plan");
                    });
            });

        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(COMBAT_PLAN_CONTENT_COLUMN_GAP_PX),
                ..default()
            })
            .with_children(|content| {
                spawn_stack_assignment_list(content);
                spawn_group_cards(content);
            });
    });
}

fn spawn_stack_assignment_list(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            width: Val::Percent(STACK_ASSIGNMENT_WIDTH_PERCENT),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            overflow: Overflow::clip_y(),
            ..default()
        })
        .with_children(|list| {
            for slot in 0..MAX_DRAFT_STACK_ROWS {
                list.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(action_button_color(true, false, &Interaction::None)),
                    Outline::new(
                        Val::Px(1.0),
                        Val::ZERO,
                        action_button_outline(true, false, &Interaction::None),
                    ),
                    Visibility::Hidden,
                    DraftActionButton(DraftAction::SelectStack(slot)),
                    DraftStackRow(slot),
                    UiPointerBlocker,
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(""),
                        ui_text_font(10.0),
                        TextColor(Color::srgb(0.82, 0.92, 0.88)),
                        DraftStackText(slot),
                    ));
                });
            }
        });
}

fn spawn_group_cards(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            width: Val::Percent(GROUP_CARDS_WIDTH_PERCENT),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(DRAFT_GROUP_CARD_GAP_PX),
            ..default()
        })
        .with_children(|groups| {
            for id in CombatGroupPlanId::ALL {
                groups
                    .spawn((
                        Node {
                            width: Val::Percent(DRAFT_GROUP_CARD_WIDTH_PERCENT),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.06, 0.08, 0.74)),
                        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(""),
                            ui_text_font(10.0),
                            TextColor(Color::srgb(0.84, 0.90, 0.96)),
                            DraftGroupText(id),
                        ));
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|buttons| {
                            spawn_draft_button(
                                buttons,
                                DraftAction::AssignSelected(id),
                                "Assigner",
                            );
                            spawn_draft_button(buttons, DraftAction::CycleRole(id), "Rôle");
                            spawn_draft_button(buttons, DraftAction::CyclePriority(id), "Cible");
                        });
                    });
            }
        });
}

fn spawn_draft_button(parent: &mut ChildSpawnerCommands, action: DraftAction, label: &str) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                action_button_outline(true, false, &Interaction::None),
            ),
            DraftActionButton(action),
            UiPointerBlocker,
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                ui_text_font(10.0),
                TextColor(Color::srgb(0.94, 0.96, 1.0)),
            ));
        });
}

pub(super) fn sync_combat_plan_draft(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    mut draft: ResMut<CombatPlanDraftState>,
) {
    if ui.phase == CombatUiPhase::FinalReport {
        draft.clear();
        return;
    }
    let Some(mission_id) = ui.current else {
        draft.clear();
        return;
    };
    if draft.mission_id == Some(mission_id) && draft.draft.is_some() {
        return;
    }
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        draft.clear();
        return;
    };
    draft.rebuild(mission_id, pending);
}

pub(super) fn handle_combat_plan_buttons(
    mut interactions: Query<(&Interaction, &DraftActionButton), Changed<Interaction>>,
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<CombatUiState>,
    mut draft: ResMut<CombatPlanDraftState>,
) {
    if ui.phase != CombatUiPhase::AwaitingDoctrine {
        return;
    }
    for (interaction, button) in &mut interactions {
        if !matches!(interaction, Interaction::Pressed) {
            continue;
        }
        apply_draft_action(button.0, &mut simulation, &mut ui, &mut draft);
    }
}

fn apply_draft_action(
    action: DraftAction,
    simulation: &mut SimulationResource,
    ui: &mut CombatUiState,
    state: &mut CombatPlanDraftState,
) {
    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        state.clear();
        return;
    };

    match action {
        DraftAction::SelectStack(slot) => {
            if let Some(stack) = allied_stacks(pending).get(slot) {
                state.selected_stack = Some(stack.stack_id);
            }
        }
        DraftAction::AssignSelected(group_id) => {
            let Some(stack_id) = state.selected_stack else {
                ui.feedback = "Sélectionnez une unité avant de l'assigner.".to_string();
                return;
            };
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.assign_stack(stack_id, group_id);
            state.dirty = true;
        }
        DraftAction::CycleRole(group_id) => {
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.cycle_role(group_id);
            state.dirty = true;
        }
        DraftAction::CyclePriority(group_id) => {
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.cycle_priority(group_id);
            state.dirty = true;
        }
        DraftAction::Confirm => {
            confirm_current_draft(simulation, ui, state);
        }
        DraftAction::Reset => {
            state.rebuild(mission_id, pending);
        }
    }
}

pub(super) fn confirm_current_draft(
    simulation: &mut SimulationResource,
    ui: &mut CombatUiState,
    state: &mut CombatPlanDraftState,
) {
    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(plan) = state.draft.as_ref().map(CombatPlanDraft::to_plan) else {
        return;
    };
    if plan.groups.is_empty() {
        ui.feedback = "Le plan de bataille est vide.".to_string();
        return;
    }
    apply_simulation_command(
        simulation,
        GameAction::ConfirmCombatPlan {
            mission_id,
            plan: plan.clone(),
        },
    );
    if simulation
        .simulation()
        .state()
        .pending_combat(mission_id)
        .and_then(PendingCombat::plan)
        == Some(&plan)
    {
        state.dirty = false;
    }
}

type DraftStackTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static DraftStackText, &'static mut Text),
    (
        Without<DraftGroupText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type DraftGroupTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static DraftGroupText, &'static mut Text),
    (
        Without<DraftStackText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type DraftStackRowQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static DraftStackRow,
        &'static DraftActionButton,
        &'static mut Visibility,
        &'static mut BackgroundColor,
        &'static mut Outline,
        &'static Interaction,
    ),
    Without<CombatPlanPanelRoot>,
>;

type DraftButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static DraftActionButton,
        &'static mut BackgroundColor,
        &'static mut Outline,
        &'static Interaction,
    ),
    Without<DraftStackRow>,
>;

#[allow(clippy::too_many_arguments)]
pub(super) fn update_combat_plan_panel(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    draft: Res<CombatPlanDraftState>,
    mut roots: Query<&mut Visibility, With<CombatPlanPanelRoot>>,
    mut stack_rows: DraftStackRowQuery,
    mut stack_texts: DraftStackTextQuery,
    mut group_texts: DraftGroupTextQuery,
    mut buttons: DraftButtonQuery,
) {
    let visible = ui.current.is_some() && ui.phase == CombatUiPhase::AwaitingDoctrine;
    for mut visibility in &mut roots {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        return;
    };
    let allied = allied_stacks(pending);
    let draft_ref = draft.draft.as_ref();

    for (row, button, mut visibility, mut background, mut outline, interaction) in &mut stack_rows {
        let Some(stack) = allied.get(row.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        let active = draft.selected_stack == Some(stack.stack_id);
        background.0 = action_button_color(true, active, interaction);
        outline.color = action_button_outline(true, active, interaction);
        debug_assert_eq!(button.0, DraftAction::SelectStack(row.0));
    }

    for (marker, mut text) in &mut stack_texts {
        if let Some(stack) = allied.get(marker.0) {
            let group = draft_ref
                .and_then(|draft| draft.selected_group(stack.stack_id))
                .map(group_label)
                .unwrap_or("?");
            let selected = if draft.selected_stack == Some(stack.stack_id) {
                "> "
            } else {
                ""
            };
            text.0 = format!(
                "{selected}{} x{} -> {group}",
                combat_unit_name(stack.identity),
                stack.surviving_quantity
            );
        } else {
            text.0.clear();
        }
    }

    for (marker, mut text) in &mut group_texts {
        let Some(group) = draft_ref.and_then(|draft| draft.group(marker.0)) else {
            text.0.clear();
            continue;
        };
        text.0 = format!(
            "{}\n{}\n{}\n{} unité(s)",
            group_label(group.id),
            role_label(group.role),
            priority_label(group.target_priority),
            group.stacks.len(),
        );
    }

    for (button, mut background, mut outline, interaction) in &mut buttons {
        let (available, active) = draft_button_state(button.0, &draft);
        background.0 = action_button_color(available, active, interaction);
        outline.color = action_button_outline(available, active, interaction);
    }
}

fn draft_button_state(action: DraftAction, state: &CombatPlanDraftState) -> (bool, bool) {
    match action {
        DraftAction::SelectStack(_) => (true, false),
        DraftAction::AssignSelected(group_id) => {
            let active = state.selected_stack.is_some_and(|stack_id| {
                state
                    .draft
                    .as_ref()
                    .and_then(|draft| draft.selected_group(stack_id))
                    == Some(group_id)
            });
            (state.selected_stack.is_some(), active)
        }
        DraftAction::CycleRole(group_id) | DraftAction::CyclePriority(group_id) => {
            let available = state
                .draft
                .as_ref()
                .and_then(|draft| draft.group(group_id))
                .is_some_and(|group| !group.stacks.is_empty());
            (available, false)
        }
        DraftAction::Confirm => (state.draft.is_some(), state.dirty),
        DraftAction::Reset => (state.draft.is_some(), false),
    }
}

fn default_group_role(id: CombatGroupPlanId) -> CombatGroupRole {
    match id {
        CombatGroupPlanId::Alpha => CombatGroupRole::Assault,
        CombatGroupPlanId::Beta => CombatGroupRole::Screen,
        CombatGroupPlanId::Gamma => CombatGroupRole::Reserve,
    }
}

fn default_group_priority(id: CombatGroupPlanId) -> CombatTargetPriority {
    match id {
        CombatGroupPlanId::Alpha => CombatTargetPriority::Any,
        CombatGroupPlanId::Beta => CombatTargetPriority::Light,
        CombatGroupPlanId::Gamma => CombatTargetPriority::Heavy,
    }
}

fn next_group_role(role: CombatGroupRole) -> CombatGroupRole {
    match role {
        CombatGroupRole::Assault => CombatGroupRole::Screen,
        CombatGroupRole::Screen => CombatGroupRole::Bombardment,
        CombatGroupRole::Bombardment => CombatGroupRole::Reserve,
        CombatGroupRole::Reserve => CombatGroupRole::Assault,
    }
}

fn next_target_priority(priority: CombatTargetPriority) -> CombatTargetPriority {
    match priority {
        CombatTargetPriority::Any => CombatTargetPriority::Light,
        CombatTargetPriority::Light => CombatTargetPriority::Medium,
        CombatTargetPriority::Medium => CombatTargetPriority::Heavy,
        CombatTargetPriority::Heavy => CombatTargetPriority::Damaged,
        CombatTargetPriority::Damaged => CombatTargetPriority::Support,
        CombatTargetPriority::Support => CombatTargetPriority::Any,
    }
}

pub(super) fn group_label(id: CombatGroupPlanId) -> &'static str {
    match id {
        CombatGroupPlanId::Alpha => "Alpha",
        CombatGroupPlanId::Beta => "Beta",
        CombatGroupPlanId::Gamma => "Gamma",
    }
}

pub(super) fn role_label(role: CombatGroupRole) -> &'static str {
    match role {
        CombatGroupRole::Assault => "Assaut",
        CombatGroupRole::Screen => "Écran",
        CombatGroupRole::Bombardment => "Bombardement",
        CombatGroupRole::Reserve => "Réserve",
    }
}

pub(super) fn priority_label(priority: CombatTargetPriority) -> &'static str {
    match priority {
        CombatTargetPriority::Any => "Cible : toute",
        CombatTargetPriority::Light => "Cible : légère",
        CombatTargetPriority::Medium => "Cible : moyenne",
        CombatTargetPriority::Heavy => "Cible : lourde",
        CombatTargetPriority::Damaged => "Cible : endommagée",
        CombatTargetPriority::Support => "Cible : soutien",
    }
}

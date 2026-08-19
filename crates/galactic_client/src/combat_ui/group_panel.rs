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
        entity_visuals::EntityVisualCatalog,
        scene::{action_button_color, action_button_outline, panel_outline, ui_text_font},
        shortcuts::apply_simulation_command,
    },
};

use super::battlefield;
use super::{CombatUiPhase, CombatUiState, combat_unit_name};

pub(super) const MAX_DRAFT_STACK_ROWS: usize = 8;
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

    fn set_role(&mut self, id: CombatGroupPlanId, role: CombatGroupRole) {
        if let Some(group) = self.group_mut(id) {
            group.role = role;
        }
    }

    fn set_priority(&mut self, id: CombatGroupPlanId, priority: CombatTargetPriority) {
        if let Some(group) = self.group_mut(id) {
            group.target_priority = priority;
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct CombatPlanDraftState {
    mission_id: Option<MissionId>,
    synced_round: Option<u16>,
    draft: Option<CombatPlanDraft>,
    selected_stack: Option<CombatStackId>,
    /// Which group's Rôle/Priorité controls the "PARAMÈTRES SÉLECTIONNÉS"
    /// column currently shows (doc §20/§21) — purely presentational, not
    /// part of the plan itself. Named distinctly from
    /// `CombatPlanDraft::selected_group(stack_id)` (which answers "which
    /// group is this *stack* in") to avoid confusing the two. Defaults to
    /// Alpha the first time a draft is built and otherwise survives
    /// rebuilds (round changes), only reset by `clear()`.
    focused_group: Option<CombatGroupPlanId>,
    dirty: bool,
}

impl CombatPlanDraftState {
    pub(super) fn rebuild(&mut self, mission_id: MissionId, pending: &PendingCombat) {
        self.mission_id = Some(mission_id);
        self.synced_round = Some(pending.round());
        self.draft = Some(CombatPlanDraft::from_pending(pending));
        self.selected_stack = None;
        if self.focused_group.is_none() {
            self.focused_group = Some(CombatGroupPlanId::Alpha);
        }
        self.dirty = false;
    }

    fn clear(&mut self) {
        self.mission_id = None;
        self.synced_round = None;
        self.draft = None;
        self.selected_stack = None;
        self.focused_group = None;
        self.dirty = false;
    }

    pub(super) fn draft(&self) -> Option<&CombatPlanDraft> {
        self.draft.as_ref()
    }

    pub(super) fn focused_group(&self) -> Option<CombatGroupPlanId> {
        self.focused_group
    }

    pub(super) fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[cfg(test)]
    pub(super) fn mark_dirty_for_tests(&mut self) {
        self.dirty = true;
    }

    #[cfg(test)]
    pub(super) fn synced_round_for_tests(&self) -> Option<u16> {
        self.synced_round
    }

    #[cfg(test)]
    pub(super) fn selected_stack_for_tests(&self) -> Option<CombatStackId> {
        self.selected_stack
    }
}

#[derive(Component)]
pub(super) struct CombatPlanPanelRoot;

#[derive(Component)]
pub(super) struct DraftHeadingText;

#[derive(Component)]
pub(super) struct DraftStackRow(usize);

#[derive(Component)]
pub(super) struct DraftStackText(usize);

/// "VOS FORCES" column: one per `CombatGroupPlanId`, the dynamic
/// role/count/integrity line under the (static) group name.
#[derive(Component)]
pub(super) struct ForcesGroupSummaryText(pub(super) CombatGroupPlanId);

#[derive(Component)]
pub(super) struct ForcesGroupIcon(pub(super) CombatGroupPlanId);

/// "PARAMÈTRES SÉLECTIONNÉS" column: names whichever group
/// `CombatPlanDraftState::focused_group` currently points at.
#[derive(Component)]
pub(super) struct SelectedGroupHeaderText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DraftAction {
    SelectStack(usize),
    /// Assigns the selected stack (if any) to this group, *and* focuses the
    /// "PARAMÈTRES SÉLECTIONNÉS" column on it regardless — the same click
    /// works whether the user is assigning a stack or just wants to check/
    /// change this group's role and priority.
    AssignSelected(CombatGroupPlanId),
    /// Sets `CombatPlanDraftState::focused_group`'s role/priority directly
    /// (doc §21: "l'utilisateur doit voir toutes les valeurs" — a grid of
    /// direct-select buttons, not a single cycling button).
    SetRole(CombatGroupRole),
    SetPriority(CombatTargetPriority),
    Confirm,
    Reset,
}

#[derive(Component, Clone, Copy)]
pub(super) struct DraftActionButton(pub(super) DraftAction);

/// The "PARAMÈTRES SÉLECTIONNÉS" column heading — just the title. Reset/
/// Confirm used to live here too, but that put them at the top of a
/// scrolling column above every control they act on — playtest feedback:
/// "Confirmer le plan tout en haut n'est pas logique, surtout quand il y a
/// du scroll." They're now a pinned footer outside the scroll area, see
/// `spawn_plan_footer`.
pub(super) fn spawn_plan_heading(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new("PARAMÈTRES SÉLECTIONNÉS"),
        ui_text_font(super::FONT_SECTION_TITLE_PX),
        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
        DraftHeadingText,
    ));
}

/// Plan-wide Reset/Confirm actions — spawned as a sibling of the
/// Paramètres column's scrollable content (not inside it), so they stay
/// visible regardless of scroll position. See `spawn_plan_heading`.
pub(super) fn spawn_plan_footer(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::top(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(panel_outline()),
        ))
        .with_children(|actions| {
            spawn_draft_button(actions, DraftAction::Reset, "Réinitialiser");
            spawn_draft_button(actions, DraftAction::Confirm, "Confirmer le plan");
        });
}

/// One row of the "VOS FORCES" stack-assignment list — a plain function
/// (not a closure) so it can be passed directly as `spawn_column_frame`'s
/// `spawn_row` callback from `combat_ui.rs`.
pub(super) fn spawn_stack_row(list: &mut ChildSpawnerCommands, slot: usize) {
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
            ui_text_font(super::FONT_SECONDARY_PX),
            TextColor(Color::srgb(0.82, 0.92, 0.88)),
            DraftStackText(slot),
        ));
    });
}

const ALL_GROUP_ROLES: [CombatGroupRole; 4] = [
    CombatGroupRole::Assault,
    CombatGroupRole::Screen,
    CombatGroupRole::Bombardment,
    CombatGroupRole::Reserve,
];
const ALL_TARGET_PRIORITIES: [CombatTargetPriority; 6] = [
    CombatTargetPriority::Any,
    CombatTargetPriority::Light,
    CombatTargetPriority::Medium,
    CombatTargetPriority::Heavy,
    CombatTargetPriority::Damaged,
    CombatTargetPriority::Support,
];

/// "VOS FORCES" column content (doc §14): one compact card per group, with
/// the group's dominant ship image — reusing `battlefield`'s exact
/// aggregate/identity logic rather than recomputing it, so the two panels
/// can never disagree. Each card is `DraftAction::AssignSelected` — clicking
/// it both assigns the currently-selected stack (if any) *and* focuses the
/// "PARAMÈTRES SÉLECTIONNÉS" column on this group (doc §20's "selected
/// state").
pub(super) fn spawn_forces_group_cards(
    parent: &mut ChildSpawnerCommands,
    placeholder: Handle<Image>,
) {
    for id in CombatGroupPlanId::ALL {
        parent
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(7.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.06, 0.08, 0.74)),
                Outline::new(
                    Val::Px(1.0),
                    Val::ZERO,
                    battlefield::group_identity_color(id).with_alpha(0.5),
                ),
                DraftActionButton(DraftAction::AssignSelected(id)),
                UiPointerBlocker,
            ))
            .with_children(|card| {
                card.spawn((
                    ImageNode {
                        image: placeholder.clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Node {
                        width: Val::Px(36.0),
                        height: Val::Px(36.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ForcesGroupIcon(id),
                ));
                card.spawn((Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },))
                    .with_children(|texts| {
                        texts.spawn((
                            Text::new(group_label(id)),
                            ui_text_font(super::FONT_SECTION_TITLE_PX),
                            TextColor(battlefield::group_identity_color(id)),
                        ));
                        texts.spawn((
                            Text::new(""),
                            ui_text_font(super::FONT_SECONDARY_PX),
                            TextColor(Color::srgba(0.80, 0.86, 0.90, 0.88)),
                            ForcesGroupSummaryText(id),
                        ));
                    });
            });
    }
}

/// "PARAMÈTRES SÉLECTIONNÉS" column: role/priority controls for whichever
/// group is currently selected (doc §21) — a grid of direct-select buttons
/// per value rather than one button cycling through them, so every option
/// is visible at once and the active one is obviously highlighted.
pub(super) fn spawn_selected_group_params(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new(""),
        ui_text_font(super::FONT_SECTION_TITLE_PX),
        TextColor(Color::srgb(0.92, 0.96, 1.0)),
        SelectedGroupHeaderText,
    ));

    parent.spawn((
        Text::new("RÔLE"),
        ui_text_font(super::FONT_SECONDARY_PX),
        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|grid| {
            for role in ALL_GROUP_ROLES {
                spawn_draft_button(grid, DraftAction::SetRole(role), role_label(role));
            }
        });

    parent.spawn((
        Text::new("PRIORITÉ"),
        ui_text_font(super::FONT_SECONDARY_PX),
        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
    ));
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|grid| {
            for priority in ALL_TARGET_PRIORITIES {
                spawn_draft_button(
                    grid,
                    DraftAction::SetPriority(priority),
                    priority_label(priority),
                );
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
                ui_text_font(super::FONT_SECONDARY_PX),
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
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        draft.clear();
        return;
    };
    if draft.mission_id == Some(mission_id)
        && draft.draft.is_some()
        && (pending.round() == 0 || draft.synced_round == Some(pending.round()))
    {
        return;
    }
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
    if pending.round() > 0 {
        ui.feedback = "Le plan actif est verrouillé ; utilisez le commandement.".to_string();
        state.rebuild(mission_id, pending);
        return;
    }

    match action {
        DraftAction::SelectStack(slot) => {
            if let Some(stack) = allied_stacks(pending).get(slot) {
                state.selected_stack = Some(stack.stack_id);
            }
        }
        DraftAction::AssignSelected(group_id) => {
            state.focused_group = Some(group_id);
            let Some(stack_id) = state.selected_stack else {
                // No stack selected — a bare click just focuses this
                // group's params on the right, which is a normal, silent
                // action, not an error worth a feedback message.
                return;
            };
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.assign_stack(stack_id, group_id);
            state.dirty = true;
        }
        DraftAction::SetRole(role) => {
            let Some(group_id) = state.focused_group else {
                return;
            };
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.set_role(group_id, role);
            state.dirty = true;
        }
        DraftAction::SetPriority(priority) => {
            let Some(group_id) = state.focused_group else {
                return;
            };
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            draft.set_priority(group_id, priority);
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
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        state.clear();
        return;
    };
    if pending.round() > 0 {
        ui.feedback = "Le plan actif est verrouillé ; utilisez le commandement.".to_string();
        state.rebuild(mission_id, pending);
        return;
    }
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
        Without<ForcesGroupSummaryText>,
        Without<DraftHeadingText>,
        Without<SelectedGroupHeaderText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type DraftHeadingTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<DraftHeadingText>,
        Without<DraftStackText>,
        Without<ForcesGroupSummaryText>,
        Without<SelectedGroupHeaderText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type ForcesGroupSummaryTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static ForcesGroupSummaryText, &'static mut Text),
    (
        Without<DraftStackText>,
        Without<DraftHeadingText>,
        Without<SelectedGroupHeaderText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type SelectedGroupHeaderTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<SelectedGroupHeaderText>,
        Without<DraftStackText>,
        Without<DraftHeadingText>,
        Without<ForcesGroupSummaryText>,
        Without<super::CombatHeaderText>,
        Without<super::CombatIntelBarText>,
        Without<super::CombatRoundLogText>,
        Without<super::CombatFeedbackText>,
    ),
>;

type ForcesGroupIconQuery<'w, 's> =
    Query<'w, 's, (&'static ForcesGroupIcon, &'static mut ImageNode)>;

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
    entity_visuals: Res<EntityVisualCatalog>,
    mut roots: Query<(&mut Visibility, &mut Node), With<CombatPlanPanelRoot>>,
    mut heading_texts: DraftHeadingTextQuery,
    mut stack_rows: DraftStackRowQuery,
    mut stack_texts: DraftStackTextQuery,
    mut forces_icons: ForcesGroupIconQuery,
    mut forces_texts: ForcesGroupSummaryTextQuery,
    mut selected_header: SelectedGroupHeaderTextQuery,
    mut buttons: DraftButtonQuery,
) {
    let visible = ui.current.is_some() && ui.phase == CombatUiPhase::AwaitingDoctrine;
    for (mut visibility, mut node) in &mut roots {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }

    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        return;
    };
    let locked = pending.round() > 0;
    let allied = allied_stacks(pending);
    let draft_ref = draft.draft.as_ref();

    for mut text in &mut heading_texts {
        text.0 = if locked {
            "PARAMÈTRES — VERROUILLÉ".to_string()
        } else {
            "PARAMÈTRES SÉLECTIONNÉS".to_string()
        };
    }

    for (row, button, mut visibility, mut background, mut outline, interaction) in &mut stack_rows {
        let Some(stack) = allied.get(row.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        let active = !locked && draft.selected_stack == Some(stack.stack_id);
        background.0 = action_button_color(!locked, active, interaction);
        outline.color = action_button_outline(!locked, active, interaction);
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

    for (marker, mut text) in &mut forces_texts {
        let Some(group) = draft_ref.and_then(|draft| draft.group(marker.0)) else {
            text.0.clear();
            continue;
        };
        let aggregate = battlefield::group_aggregate(
            CombatPlanDraftGroupView {
                id: group.id,
                stacks: &group.stacks,
                role: group.role,
                target_priority: group.target_priority,
            },
            &allied,
        );
        text.0 = if aggregate.quantity > 0 {
            format!(
                "{} · {}\n{} unité(s) · intégrité {}%",
                role_label(group.role),
                priority_label(group.target_priority),
                aggregate.quantity,
                aggregate.integrity_percent,
            )
        } else {
            format!(
                "{} · {}\nvide",
                role_label(group.role),
                priority_label(group.target_priority),
            )
        };
    }

    for (marker, mut icon) in &mut forces_icons {
        let identity = draft_ref
            .and_then(|draft| draft.group(marker.0))
            .and_then(|group| {
                battlefield::group_aggregate(
                    CombatPlanDraftGroupView {
                        id: group.id,
                        stacks: &group.stacks,
                        role: group.role,
                        target_priority: group.target_priority,
                    },
                    &allied,
                )
                .primary
            });
        if let Some(identity) = identity {
            icon.image = battlefield::unit_image(identity, &entity_visuals);
            icon.color = Color::WHITE;
        } else {
            icon.color = Color::srgba(1.0, 1.0, 1.0, 0.22);
        }
    }

    if let Ok(mut text) = selected_header.single_mut() {
        // Explicit sentence, not a bare group name — playtest feedback: the
        // link between clicking a group on the left and this column
        // updating on the right wasn't obvious from color-matching alone.
        text.0 = match draft.focused_group() {
            Some(id) => format!("Groupe sélectionné : {}", group_label(id)),
            None => "Aucun groupe sélectionné".to_string(),
        };
    }

    for (button, mut background, mut outline, interaction) in &mut buttons {
        let (available, active) = draft_button_state(button.0, &draft, locked);
        background.0 = action_button_color(available, active, interaction);
        outline.color = action_button_outline(available, active, interaction);
    }
}

fn draft_button_state(
    action: DraftAction,
    state: &CombatPlanDraftState,
    locked: bool,
) -> (bool, bool) {
    if locked {
        return (false, false);
    }
    match action {
        DraftAction::SelectStack(_) => (true, false),
        DraftAction::AssignSelected(group_id) => {
            // Always clickable (a bare click just focuses the group), and
            // highlighted when it's the one "PARAMÈTRES SÉLECTIONNÉS" shows.
            (true, state.focused_group == Some(group_id))
        }
        DraftAction::SetRole(role) => {
            let active = state.focused_group.is_some_and(|group_id| {
                state
                    .draft
                    .as_ref()
                    .and_then(|draft| draft.group(group_id))
                    .is_some_and(|group| group.role == role)
            });
            (state.focused_group.is_some(), active)
        }
        DraftAction::SetPriority(priority) => {
            let active = state.focused_group.is_some_and(|group_id| {
                state
                    .draft
                    .as_ref()
                    .and_then(|draft| draft.group(group_id))
                    .is_some_and(|group| group.target_priority == priority)
            });
            (state.focused_group.is_some(), active)
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

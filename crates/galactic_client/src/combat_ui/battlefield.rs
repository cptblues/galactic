use bevy::prelude::*;
use galactic_sim::{
    AlliedStackView, CombatGroupPlanId, CombatGroupRole, CombatRoundEvent, CombatRoundRecord,
    CombatSide, CombatStackExchange, CombatStackId, CombatStackLoss, CombatStackView,
    CombatTargetPriority, CombatUnitRef, PlanetaryForceDomain, allied_stacks, default_ruleset,
    enemy_intel, round_history,
};

use crate::{
    SimulationResource,
    presentation::{
        entity_visuals::EntityVisualCatalog,
        icons::{IconAssets, IconKind},
        scene::{panel_background, panel_outline, ui_text_font},
    },
};

use super::group_panel::{
    self, CombatPlanDraft, CombatPlanDraftGroupView, CombatPlanDraftState, DraftAction,
    DraftActionButton,
};
use super::{
    CombatRoundLogText, CombatUiPhase, CombatUiState, UiPointerBlocker, combat_unit_name,
    doctrine_name, integrity_reveal_text, quantity_reveal_text, target_class_label,
};

const MAX_BATTLEFIELD_CONTACTS: usize = 5;
pub(super) const BATTLEFIELD_PANEL_WIDTH_PERCENT: f32 = 52.0;
pub(super) const BATTLEFIELD_GROUP_LANE_WIDTH_PERCENT: f32 = 38.0;
pub(super) const BATTLEFIELD_ORBIT_LANE_WIDTH_PERCENT: f32 = 23.0;
pub(super) const BATTLEFIELD_CONTACT_LANE_WIDTH_PERCENT: f32 = 35.0;
pub(super) const BATTLEFIELD_INNER_COLUMN_GAP_PX: f32 = 8.0;

#[derive(Component)]
pub(super) struct BattlefieldPanelRoot;

#[derive(Component)]
pub(super) struct BattlefieldMapContentRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) struct BattlefieldText(pub(super) BattlefieldTextKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BattlefieldTextKind {
    Plan,
    Planet,
    Group(CombatGroupPlanId),
    Trajectory(CombatGroupPlanId),
    Enemy(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) struct BattlefieldIcon(pub(super) BattlefieldIconKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BattlefieldIconKind {
    Group(CombatGroupPlanId),
    Enemy(usize),
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) struct BattlefieldRow(pub(super) BattlefieldRowKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BattlefieldRowKind {
    Group(CombatGroupPlanId),
    Enemy(usize),
}

struct GroupAggregate {
    quantity: u64,
    integrity_percent: u8,
    primary: Option<CombatUnitRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoundGroupVisual {
    pub(super) group_id: CombatGroupPlanId,
    pub(super) outgoing_damage: u128,
    pub(super) incoming_damage: u128,
    pub(super) lost_quantity: u64,
    pub(super) primary_target: Option<CombatStackId>,
    pub(super) stayed_in_reserve: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoundTargetVisual {
    pub(super) stack_id: CombatStackId,
    pub(super) damage: u128,
    pub(super) lost_quantity: u64,
    pub(super) destroyed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RoundVisualSummary {
    groups: Vec<RoundGroupVisual>,
    targets: Vec<RoundTargetVisual>,
}

impl RoundVisualSummary {
    pub(super) fn group(&self, group_id: CombatGroupPlanId) -> Option<&RoundGroupVisual> {
        self.groups.iter().find(|group| group.group_id == group_id)
    }

    pub(super) fn target(&self, stack_id: CombatStackId) -> Option<&RoundTargetVisual> {
        self.targets
            .iter()
            .find(|target| target.stack_id == stack_id)
    }
}

pub(super) fn spawn_battlefield_panel(parent: &mut ChildSpawnerCommands, icon_assets: &IconAssets) {
    let placeholder = icon_assets.handle(IconKind::CombatUnit);
    parent
        .spawn((
            Node {
                width: Val::Percent(BATTLEFIELD_PANEL_WIDTH_PERCENT),
                height: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
            BattlefieldPanelRoot,
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
                        Text::new("CARTE TACTIQUE"),
                        ui_text_font(12.0),
                        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
                    ));
                    heading.spawn((
                        Text::new("Zone orbitale"),
                        ui_text_font(11.0),
                        TextColor(Color::srgba(0.70, 0.78, 0.86, 0.78)),
                        CombatRoundLogText,
                    ));
                });

            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    BattlefieldMapContentRoot,
                ))
                .with_children(|content| {
                    content.spawn((
                        Text::new("PLAN :"),
                        ui_text_font(11.0),
                        TextColor(Color::srgb(0.88, 0.92, 1.0)),
                        BattlefieldText(BattlefieldTextKind::Plan),
                    ));

                    content
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(BATTLEFIELD_INNER_COLUMN_GAP_PX),
                            ..default()
                        })
                        .with_children(|map| {
                            spawn_group_lane(map, placeholder.clone());
                            spawn_orbit_lane(map);
                            spawn_contact_lane(map, placeholder);
                        });
                });
        });
}

/// COMBAT-UX-001-C: the 3 group tokens cluster vertically centered (doc §18
/// — "les groupes doivent occuper une petite zone autour de la planète")
/// instead of stretching to fill the lane, and are real click targets: each
/// carries `DraftAction::AssignSelected` — the exact same action/system
/// `group_panel.rs`'s "VOS FORCES" buttons already use (doc §5.4's MVP
/// assignment flow: "stack sélectionné + gros boutons Alpha/Beta/Gamma"),
/// so clicking a token assigns the selected stack for free, no new state.
fn spawn_group_lane(parent: &mut ChildSpawnerCommands, placeholder: Handle<Image>) {
    parent
        .spawn(Node {
            width: Val::Percent(BATTLEFIELD_GROUP_LANE_WIDTH_PERCENT),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|lane| {
            for id in CombatGroupPlanId::ALL {
                lane.spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        min_height: Val::Px(64.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.07, 0.08, 0.78)),
                    Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                    BattlefieldRow(BattlefieldRowKind::Group(id)),
                    DraftActionButton(DraftAction::AssignSelected(id)),
                    UiPointerBlocker,
                ))
                .with_children(|row| {
                    row.spawn((
                        ImageNode {
                            image: placeholder.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        BattlefieldIcon(BattlefieldIconKind::Group(id)),
                    ));
                    row.spawn((Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },))
                        .with_children(|texts| {
                            texts.spawn((
                                Text::new(""),
                                ui_text_font(10.0),
                                TextColor(Color::srgb(0.84, 0.92, 0.94)),
                                BattlefieldText(BattlefieldTextKind::Group(id)),
                            ));
                            texts.spawn((
                                Text::new(""),
                                ui_text_font(10.0),
                                TextColor(Color::srgba(0.76, 0.84, 0.90, 0.82)),
                                BattlefieldText(BattlefieldTextKind::Trajectory(id)),
                            ));
                        });
                });
            }
        });
}

fn spawn_orbit_lane(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(Node {
            width: Val::Percent(BATTLEFIELD_ORBIT_LANE_WIDTH_PERCENT),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|lane| {
            lane.spawn((
                Node {
                    width: Val::Px(132.0),
                    height: Val::Px(132.0),
                    max_width: Val::Percent(100.0),
                    max_height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.09, 0.82)),
                Outline::new(
                    Val::Px(1.0),
                    Val::Px(3.0),
                    Color::srgba(0.42, 0.70, 0.86, 0.38),
                ),
            ))
            .with_children(|orbit| {
                orbit.spawn((
                    Text::new("PLANÈTE\nORBITE"),
                    ui_text_font(11.0),
                    TextColor(Color::srgb(0.86, 0.92, 0.96)),
                    BattlefieldText(BattlefieldTextKind::Planet),
                ));
            });
        });
}

/// Enemy tokens mirror the group lane's compact/centered treatment (see
/// `spawn_group_lane`) for visual symmetry around the planet — no click
/// action here, contacts aren't assignable.
fn spawn_contact_lane(parent: &mut ChildSpawnerCommands, placeholder: Handle<Image>) {
    parent
        .spawn(Node {
            width: Val::Percent(BATTLEFIELD_CONTACT_LANE_WIDTH_PERCENT),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(5.0),
            ..default()
        })
        .with_children(|lane| {
            lane.spawn((
                Text::new("CONTACTS"),
                ui_text_font(11.0),
                TextColor(Color::srgba(1.0, 0.84, 0.78, 0.88)),
            ));
            for slot in 0..MAX_BATTLEFIELD_CONTACTS {
                lane.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        min_height: Val::Px(52.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.05, 0.05, 0.72)),
                    Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                    Visibility::Hidden,
                    BattlefieldRow(BattlefieldRowKind::Enemy(slot)),
                ))
                .with_children(|row| {
                    row.spawn((
                        ImageNode {
                            image: placeholder.clone(),
                            color: Color::WHITE,
                            ..default()
                        },
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        BattlefieldIcon(BattlefieldIconKind::Enemy(slot)),
                    ));
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            ..default()
                        },
                        Text::new(""),
                        ui_text_font(10.0),
                        TextColor(Color::srgb(0.94, 0.84, 0.80)),
                        BattlefieldText(BattlefieldTextKind::Enemy(slot)),
                    ));
                });
            }
        });
}

type BattlefieldRootQuery<'w, 's> =
    Query<'w, 's, &'static mut Visibility, With<BattlefieldPanelRoot>>;

type BattlefieldMapContentQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<BattlefieldMapContentRoot>,
        Without<BattlefieldPanelRoot>,
    ),
>;

type BattlefieldTextQuery<'w, 's> = Query<'w, 's, (&'static BattlefieldText, &'static mut Text)>;

type BattlefieldIconQuery<'w, 's> =
    Query<'w, 's, (&'static BattlefieldIcon, &'static mut ImageNode)>;

type BattlefieldRowQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static BattlefieldRow,
        &'static mut Visibility,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
    (
        Without<BattlefieldPanelRoot>,
        Without<BattlefieldMapContentRoot>,
    ),
>;

pub(super) fn round_animation_progress(ui: &CombatUiState) -> f32 {
    if ui.phase != CombatUiPhase::RoundPause {
        return 1.0;
    }
    let duration = ui.round_pause_timer.duration().as_secs_f32();
    if duration <= f32::EPSILON {
        return 1.0;
    }
    (ui.round_pause_timer.elapsed_secs() / duration).clamp(0.0, 1.0)
}

pub(super) fn round_animation_intensity(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    let pulse = (progress * std::f32::consts::TAU * 2.0).sin().abs();
    0.35 + pulse * 0.65
}

fn exchange_damage(exchanges: &[CombatStackExchange], target: CombatStackId) -> u128 {
    exchanges
        .iter()
        .filter(|exchange| exchange.target == target)
        .fold(0, |total, exchange| {
            total.saturating_add(exchange.allocated_damage)
        })
}

fn loss_quantity(losses: &[CombatStackLoss], stack_id: CombatStackId) -> u64 {
    losses
        .iter()
        .filter(|loss| loss.stack_id == stack_id)
        .fold(0, |total, loss| total.saturating_add(loss.quantity))
}

fn primary_target_for_group(
    exchanges: &[CombatStackExchange],
    group_id: CombatGroupPlanId,
) -> Option<CombatStackId> {
    exchanges
        .iter()
        .filter(|exchange| exchange.source_group == group_id)
        .max_by_key(|exchange| exchange.allocated_damage)
        .map(|exchange| exchange.target)
}

fn outgoing_damage_for_group(
    exchanges: &[CombatStackExchange],
    group_id: CombatGroupPlanId,
) -> u128 {
    exchanges
        .iter()
        .filter(|exchange| exchange.source_group == group_id)
        .fold(0, |total, exchange| {
            total.saturating_add(exchange.allocated_damage)
        })
}

fn stack_is_destroyed(
    events: &[CombatRoundEvent],
    side: CombatSide,
    stack_id: CombatStackId,
) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            CombatRoundEvent::StackDestroyed {
                side: event_side,
                stack_id: event_stack,
            } if *event_side == side && *event_stack == stack_id
        )
    })
}

pub(super) fn round_visual_summary(
    record: &CombatRoundRecord,
    plan: &CombatPlanDraft,
) -> RoundVisualSummary {
    let groups = plan
        .groups()
        .map(|group| {
            let outgoing_damage = outgoing_damage_for_group(&record.attacker_exchanges, group.id);
            let incoming_damage = group.stacks.iter().fold(0_u128, |total, stack_id| {
                total.saturating_add(exchange_damage(&record.defender_exchanges, *stack_id))
            });
            let lost_quantity = group.stacks.iter().fold(0_u64, |total, stack_id| {
                total.saturating_add(loss_quantity(&record.attacker_losses, *stack_id))
            });
            RoundGroupVisual {
                group_id: group.id,
                outgoing_damage,
                incoming_damage,
                lost_quantity,
                primary_target: primary_target_for_group(&record.attacker_exchanges, group.id),
                stayed_in_reserve: group.role == CombatGroupRole::Reserve
                    && !group.stacks.is_empty()
                    && outgoing_damage == 0,
            }
        })
        .collect();

    let mut targets = Vec::<RoundTargetVisual>::new();
    for exchange in &record.attacker_exchanges {
        if let Some(target) = targets
            .iter_mut()
            .find(|target| target.stack_id == exchange.target)
        {
            target.damage = target.damage.saturating_add(exchange.allocated_damage);
        } else {
            targets.push(RoundTargetVisual {
                stack_id: exchange.target,
                damage: exchange.allocated_damage,
                lost_quantity: 0,
                destroyed: false,
            });
        }
    }
    for target in &mut targets {
        target.lost_quantity = loss_quantity(&record.defender_losses, target.stack_id);
        target.destroyed = stack_is_destroyed(
            &record.notable_events,
            CombatSide::Defender,
            target.stack_id,
        );
    }

    RoundVisualSummary { groups, targets }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_battlefield_panel(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    draft_state: Res<CombatPlanDraftState>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut roots: BattlefieldRootQuery,
    mut map_contents: BattlefieldMapContentQuery,
    mut rows: BattlefieldRowQuery,
    mut texts: BattlefieldTextQuery,
    mut icons: BattlefieldIconQuery,
) {
    let root_visibility = if ui.current.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut roots {
        *visibility = root_visibility;
    }

    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        for mut visibility in &mut map_contents {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let map_visibility = if ui.phase == CombatUiPhase::FinalReport {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut map_contents {
        *visibility = map_visibility;
    }
    if map_visibility == Visibility::Hidden {
        return;
    }

    let fallback_draft = CombatPlanDraft::from_pending(pending);
    let active_plan = if ui.phase == CombatUiPhase::AwaitingDoctrine {
        draft_state.draft().unwrap_or(&fallback_draft)
    } else {
        &fallback_draft
    };
    let allied = allied_stacks(pending);
    let enemy = enemy_intel(pending);
    let round_record = (ui.phase == CombatUiPhase::RoundPause)
        .then(|| round_history(pending).last())
        .flatten();
    let round_visual = round_record.map(|record| round_visual_summary(record, active_plan));
    let animation_intensity = if round_record.is_some() {
        round_animation_intensity(round_animation_progress(&ui))
    } else {
        0.0
    };

    for (row, mut visibility, mut background, mut outline) in &mut rows {
        match row.0 {
            BattlefieldRowKind::Group(id) => {
                *visibility = Visibility::Inherited;
                let group = active_plan.groups().find(|candidate| candidate.id == id);
                let active = group
                    .as_ref()
                    .is_some_and(|candidate| !candidate.stacks.is_empty());
                let role = group
                    .as_ref()
                    .map(|candidate| candidate.role)
                    .unwrap_or(CombatGroupRole::Reserve);
                let visual = round_visual.as_ref().and_then(|summary| summary.group(id));
                background.0 = animated_group_background(role, active, visual, animation_intensity);
                outline.color = animated_group_outline(role, active, visual, animation_intensity);
            }
            BattlefieldRowKind::Enemy(slot) => {
                let stack = enemy.stacks.get(slot);
                *visibility = if stack.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                let visual = stack.and_then(|stack| {
                    round_visual
                        .as_ref()
                        .and_then(|summary| summary.target(stack.stack_id))
                });
                background.0 = enemy_background(visual, animation_intensity);
                outline.color = enemy_outline(visual, animation_intensity);
            }
        }
    }

    for (marker, mut text) in &mut texts {
        text.0 = match marker.0 {
            BattlefieldTextKind::Plan => {
                format!("PLAN : {}", doctrine_name(active_plan.doctrine()))
            }
            BattlefieldTextKind::Planet => format!(
                "PLANÈTE\nORBITE\n{}",
                planet_round_label(ui.phase, pending.round(), pending.maximum_rounds())
            ),
            BattlefieldTextKind::Group(id) => active_plan
                .groups()
                .find(|group| group.id == id)
                .map(|group| {
                    group_summary_text(
                        group,
                        &allied,
                        round_visual.as_ref().and_then(|summary| summary.group(id)),
                        &enemy.stacks,
                    )
                })
                .unwrap_or_else(|| empty_group_text(id)),
            BattlefieldTextKind::Trajectory(id) => active_plan
                .groups()
                .find(|group| group.id == id)
                .map(|group| {
                    trajectory_text(
                        group,
                        round_visual.as_ref().and_then(|summary| summary.group(id)),
                        &enemy.stacks,
                    )
                })
                .unwrap_or_else(|| "RÉSERVE inactive".to_string()),
            BattlefieldTextKind::Enemy(slot) => enemy
                .stacks
                .get(slot)
                .map(|stack| {
                    enemy_contact_text(
                        stack,
                        round_visual
                            .as_ref()
                            .and_then(|summary| summary.target(stack.stack_id)),
                    )
                })
                .unwrap_or_default(),
        };
    }

    for (marker, mut icon) in &mut icons {
        match marker.0 {
            BattlefieldIconKind::Group(id) => {
                let visual = round_visual.as_ref().and_then(|summary| summary.group(id));
                if let Some(identity) = active_plan
                    .groups()
                    .find(|group| group.id == id)
                    .and_then(|group| primary_group_identity(group, &allied))
                {
                    icon.image = unit_image(identity, &entity_visuals);
                    icon.color = group_icon_color(visual, animation_intensity);
                } else {
                    icon.color = Color::srgba(1.0, 1.0, 1.0, 0.22);
                }
            }
            BattlefieldIconKind::Enemy(slot) => {
                if let Some(stack) = enemy.stacks.get(slot) {
                    let visual = round_visual
                        .as_ref()
                        .and_then(|summary| summary.target(stack.stack_id));
                    icon.image = entity_visuals.enemy_contact(stack);
                    icon.color = enemy_icon_color(visual, animation_intensity);
                } else {
                    icon.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
                }
            }
        }
    }
}

fn planet_round_label(phase: CombatUiPhase, round: u16, maximum_rounds: u16) -> String {
    if phase == CombatUiPhase::RoundPause {
        format!("Round {round}/{maximum_rounds} résolu")
    } else {
        format!("Round {}/{}", round.saturating_add(1), maximum_rounds)
    }
}

fn unit_image(identity: CombatUnitRef, entity_visuals: &EntityVisualCatalog) -> Handle<Image> {
    match identity {
        CombatUnitRef::Ship(craftable) => entity_visuals.ship(craftable),
        CombatUnitRef::PlanetaryForce(id) => entity_visuals.force(id),
    }
}

fn group_aggregate(
    group: CombatPlanDraftGroupView<'_>,
    allied: &[AlliedStackView],
) -> GroupAggregate {
    let mut quantity = 0_u64;
    let mut integrity_total = 0_u64;
    let mut primary = None;
    for stack_id in group.stacks {
        let Some(stack) = allied_stack(*stack_id, allied) else {
            continue;
        };
        if stack.surviving_quantity == 0 {
            continue;
        }
        quantity = quantity.saturating_add(stack.surviving_quantity);
        integrity_total = integrity_total.saturating_add(
            u64::from(stack.integrity_percent).saturating_mul(stack.surviving_quantity),
        );
        primary.get_or_insert(stack.identity);
    }
    let integrity_percent =
        u8::try_from(integrity_total.checked_div(quantity).unwrap_or(0).min(100))
            .expect("a percent clamped to 100 fits in u8");
    GroupAggregate {
        quantity,
        integrity_percent,
        primary,
    }
}

fn group_summary_text(
    group: CombatPlanDraftGroupView<'_>,
    allied: &[AlliedStackView],
    round: Option<&RoundGroupVisual>,
    enemy: &[CombatStackView],
) -> String {
    let aggregate = group_aggregate(group, allied);
    let status = aggregate
        .primary
        .map(|identity| {
            format!(
                "{} x{} · intégrité {}%",
                combat_unit_name(identity),
                aggregate.quantity,
                aggregate.integrity_percent
            )
        })
        .unwrap_or_else(|| "vide".to_string());
    let mut text = format!(
        "{} · {}\n{}\n{}",
        group_panel::group_label(group.id),
        group_panel::role_label(group.role),
        group_panel::priority_label(group.target_priority),
        status
    );
    if let Some(round_line) = group_round_text(round, enemy) {
        text.push('\n');
        text.push_str(&round_line);
    }
    text
}

fn empty_group_text(id: CombatGroupPlanId) -> String {
    format!(
        "{} · Réserve\nCible : toute\nvide",
        group_panel::group_label(id)
    )
}

fn trajectory_text(
    group: CombatPlanDraftGroupView<'_>,
    round: Option<&RoundGroupVisual>,
    enemy: &[CombatStackView],
) -> String {
    if group.stacks.is_empty() || !trajectory_is_active(group.role) {
        return "RÉSERVE inactive".to_string();
    }
    if let Some(round) = round
        && round.outgoing_damage > 0
    {
        let target = round
            .primary_target
            .and_then(|stack_id| enemy.iter().find(|stack| stack.stack_id == stack_id))
            .map(enemy_contact_short_label)
            .unwrap_or_else(|| "contact".to_string());
        return format!(
            "{}\nimpact {} vers {target}",
            trajectory_label(group.role),
            compact_damage(round.outgoing_damage),
        );
    }
    format!(
        "{}\n{}",
        trajectory_label(group.role),
        trajectory_priority_label(group.target_priority)
    )
}

pub(super) fn trajectory_label(role: CombatGroupRole) -> &'static str {
    match role {
        CombatGroupRole::Assault => "attaque ──────────►",
        CombatGroupRole::Screen => "écran - - - - ◯",
        CombatGroupRole::Bombardment => "bombardement ╌╌╌╌╌►",
        CombatGroupRole::Reserve => "RÉSERVE inactive",
    }
}

pub(super) fn trajectory_is_active(role: CombatGroupRole) -> bool {
    !matches!(role, CombatGroupRole::Reserve)
}

fn trajectory_priority_label(priority: CombatTargetPriority) -> &'static str {
    match priority {
        CombatTargetPriority::Any => "vers opportunité",
        CombatTargetPriority::Light => "vers cible légère",
        CombatTargetPriority::Medium => "vers cible moyenne",
        CombatTargetPriority::Heavy => "vers cible lourde",
        CombatTargetPriority::Damaged => "vers cible endommagée",
        CombatTargetPriority::Support => "vers soutien",
    }
}

fn group_round_text(round: Option<&RoundGroupVisual>, enemy: &[CombatStackView]) -> Option<String> {
    let round = round?;
    let mut lines = Vec::new();
    if round.outgoing_damage > 0 {
        let target = round
            .primary_target
            .and_then(|stack_id| enemy.iter().find(|stack| stack.stack_id == stack_id))
            .map(enemy_contact_short_label)
            .unwrap_or_else(|| "contact".to_string());
        lines.push(format!(
            "Tir : {} vers {target}",
            compact_damage(round.outgoing_damage)
        ));
    } else if round.stayed_in_reserve {
        lines.push("Round : reste en réserve".to_string());
    }
    if round.incoming_damage > 0 {
        lines.push(format!(
            "Impact : {}",
            compact_damage(round.incoming_damage)
        ));
    }
    if round.lost_quantity > 0 {
        lines.push(format!("Pertes : {}", round.lost_quantity));
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn enemy_contact_text(stack: &CombatStackView, round: Option<&RoundTargetVisual>) -> String {
    let mut identity = stack
        .identity
        .map(combat_unit_name)
        .map(str::to_string)
        .or_else(|| {
            stack
                .target_class
                .map(|class| format!("Contact {}", target_class_label(class)))
        })
        .unwrap_or_else(|| "Contact inconnu".to_string());
    identity.push_str(" · ");
    identity.push_str(enemy_contact_domain_label(stack));
    let mut text = format!(
        "{}\n{}\n{}",
        identity,
        quantity_reveal_text(stack.quantity),
        integrity_reveal_text(stack.integrity)
    );
    if let Some(round) = round {
        text.push('\n');
        text.push_str(&target_round_text(round));
    }
    text
}

pub(super) fn enemy_contact_short_label(stack: &CombatStackView) -> String {
    let mut label = stack
        .identity
        .map(combat_unit_name)
        .map(str::to_string)
        .or_else(|| {
            stack
                .target_class
                .map(|class| format!("contact {}", target_class_label(class)))
        })
        .unwrap_or_else(|| "contact inconnu".to_string());
    if let Some(domain) = stack.identity.map(combat_unit_domain_label) {
        label.push_str(" [");
        label.push_str(domain);
        label.push(']');
    }
    label
}

fn enemy_contact_domain_label(stack: &CombatStackView) -> &'static str {
    stack
        .identity
        .map(combat_unit_domain_label)
        .unwrap_or("domaine masqué")
}

fn combat_unit_domain_label(identity: CombatUnitRef) -> &'static str {
    match identity {
        CombatUnitRef::Ship(_) => "ORBITE",
        CombatUnitRef::PlanetaryForce(force_id) => default_ruleset()
            .planetary_presence()
            .definition(force_id)
            .map(|definition| match definition.domain {
                PlanetaryForceDomain::Ground => "SOL",
                PlanetaryForceDomain::Orbital => "ORBITE",
            })
            .unwrap_or("SOL"),
    }
}

fn target_round_text(round: &RoundTargetVisual) -> String {
    let mut lines = vec![format!("Impact : {}", compact_damage(round.damage))];
    if round.lost_quantity > 0 {
        lines.push(format!("Pertes : {}", round.lost_quantity));
    }
    if round.destroyed {
        lines.push("Détruit".to_string());
    }
    lines.join("\n")
}

pub(super) fn compact_damage(damage: u128) -> String {
    if damage >= 1_000_000 {
        format!("{}M dégâts", damage / 1_000_000)
    } else if damage >= 1_000 {
        format!("{}k dégâts", damage / 1_000)
    } else {
        format!("{damage} dégâts")
    }
}

fn primary_group_identity(
    group: CombatPlanDraftGroupView<'_>,
    allied: &[AlliedStackView],
) -> Option<CombatUnitRef> {
    group
        .stacks
        .iter()
        .filter_map(|stack_id| allied_stack(*stack_id, allied))
        .find(|stack| stack.surviving_quantity > 0)
        .map(|stack| stack.identity)
}

fn allied_stack(stack_id: CombatStackId, allied: &[AlliedStackView]) -> Option<&AlliedStackView> {
    allied.iter().find(|stack| stack.stack_id == stack_id)
}

fn group_background(role: CombatGroupRole, active: bool) -> Color {
    let alpha = if active { 0.82 } else { 0.52 };
    match role {
        CombatGroupRole::Assault => Color::srgba(0.08, 0.12, 0.12, alpha),
        CombatGroupRole::Screen => Color::srgba(0.05, 0.10, 0.14, alpha),
        CombatGroupRole::Bombardment => Color::srgba(0.12, 0.09, 0.05, alpha),
        CombatGroupRole::Reserve => Color::srgba(0.08, 0.08, 0.09, alpha),
    }
}

fn group_outline(role: CombatGroupRole, active: bool) -> Color {
    let alpha = if active { 0.62 } else { 0.28 };
    match role {
        CombatGroupRole::Assault => Color::srgba(0.38, 0.92, 0.78, alpha),
        CombatGroupRole::Screen => Color::srgba(0.54, 0.78, 0.96, alpha),
        CombatGroupRole::Bombardment => Color::srgba(0.96, 0.72, 0.42, alpha),
        CombatGroupRole::Reserve => Color::srgba(0.72, 0.74, 0.78, alpha),
    }
}

fn group_took_hit(visual: &RoundGroupVisual) -> bool {
    visual.incoming_damage > 0 || visual.lost_quantity > 0
}

fn animated_group_background(
    role: CombatGroupRole,
    active: bool,
    visual: Option<&RoundGroupVisual>,
    intensity: f32,
) -> Color {
    let Some(visual) = visual else {
        return group_background(role, active);
    };
    let alpha = 0.68 + intensity * 0.18;
    if group_took_hit(visual) {
        Color::srgba(0.20, 0.06, 0.05, alpha)
    } else if visual.outgoing_damage > 0 {
        Color::srgba(0.06, 0.16, 0.13, alpha)
    } else if visual.stayed_in_reserve {
        Color::srgba(0.08, 0.08, 0.10, 0.68)
    } else {
        group_background(role, active)
    }
}

fn animated_group_outline(
    role: CombatGroupRole,
    active: bool,
    visual: Option<&RoundGroupVisual>,
    intensity: f32,
) -> Color {
    let Some(visual) = visual else {
        return group_outline(role, active);
    };
    let alpha = 0.60 + intensity * 0.35;
    if group_took_hit(visual) {
        Color::srgba(1.0, 0.36, 0.28, alpha)
    } else if visual.outgoing_damage > 0 {
        Color::srgba(0.52, 1.0, 0.78, alpha)
    } else if visual.stayed_in_reserve {
        Color::srgba(0.72, 0.74, 0.78, 0.44)
    } else {
        group_outline(role, active)
    }
}

fn enemy_background(visual: Option<&RoundTargetVisual>, intensity: f32) -> Color {
    if visual.is_some() {
        Color::srgba(0.20, 0.06, 0.05, 0.70 + intensity * 0.18)
    } else {
        Color::srgba(0.09, 0.05, 0.05, 0.72)
    }
}

fn enemy_outline(visual: Option<&RoundTargetVisual>, intensity: f32) -> Color {
    if visual.is_some() {
        Color::srgba(1.0, 0.42, 0.30, 0.60 + intensity * 0.35)
    } else {
        panel_outline()
    }
}

fn group_icon_color(visual: Option<&RoundGroupVisual>, intensity: f32) -> Color {
    let Some(visual) = visual else {
        return Color::WHITE;
    };
    let alpha = 0.78 + intensity * 0.22;
    if group_took_hit(visual) {
        Color::srgba(1.0, 0.62, 0.52, alpha)
    } else if visual.outgoing_damage > 0 {
        Color::srgba(0.66, 1.0, 0.82, alpha)
    } else if visual.stayed_in_reserve {
        Color::srgba(0.76, 0.80, 0.84, 0.72)
    } else {
        Color::WHITE
    }
}

fn enemy_icon_color(visual: Option<&RoundTargetVisual>, intensity: f32) -> Color {
    if visual.is_some() {
        Color::srgba(1.0, 0.58, 0.48, 0.78 + intensity * 0.22)
    } else {
        Color::WHITE
    }
}

#[cfg(test)]
mod tests {
    use galactic_sim::{CombatUnitRef, CraftableId};

    use super::*;

    #[test]
    fn contact_domain_labels_distinguish_ground_and_orbital_forces() {
        let planetary = default_ruleset().planetary_presence();
        let ground = planetary
            .id_by_key("confins_militia")
            .expect("default ground force exists");
        let orbital = planetary
            .id_by_key("confins_dock_sentry")
            .expect("default orbital force exists");

        assert_eq!(
            combat_unit_domain_label(CombatUnitRef::PlanetaryForce(ground)),
            "SOL"
        );
        assert_eq!(
            combat_unit_domain_label(CombatUnitRef::PlanetaryForce(orbital)),
            "ORBITE"
        );
        assert_eq!(
            combat_unit_domain_label(CombatUnitRef::Ship(CraftableId::FRIGATE_BULWARK)),
            "ORBITE"
        );
    }
}

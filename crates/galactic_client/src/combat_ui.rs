// COMBAT-001-D: the player-facing combat screen — replaces the temporary
// auto-pilot bridge (`presentation/combat_autopilot.rs`, now deleted). Full-
// screen modal, following `spawn_intro_pitch_modal`/`spawn_victory_modal`'s
// structural precedent — the only two existing screens that must be able to
// appear over any open panel, which a mandatory combat screen also needs.

use std::collections::HashSet;
use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use galactic_domain::MissionId;
use galactic_sim::{
    AuthorizationError, CombatAutoResolveRejected, CombatCommandError, CombatCompleted,
    CombatDecisionRequired, CombatDoctrineId, CombatDoctrineRejected, CombatGroupPlanId,
    CombatGroupRole, CombatIntervention, CombatInterventionError, CombatPlanConfirmed,
    CombatPlanRejected, CombatPlanValidationError, CombatReport, CombatReportStatus,
    CombatRetreatRejected, CombatRoundEvent, CombatRoundRecord, CombatRoundResolved, CombatSide,
    CombatTargetClass, CombatTargetPriority, CombatUnitRef, DoctrineOverview, EnemyIntelView,
    GameAction, GameEventKind, IntegrityBand, IntegrityReveal, MissionTarget, PendingCombat,
    QualitativePrediction, QuantityBand, QuantityReveal, ThreatLevel, allied_stacks, combat_rules,
    craftable_definition, default_ruleset, doctrine_overview, enemy_intel, qualitative_prediction,
    repetition_penalty_preview, round_history,
};

mod battlefield;
mod group_panel;

use super::{
    PresentationUpdateSet, SimulationResource, UiPointerBlocker, action_button_color,
    action_button_outline, apply_simulation_command, collect_presentation_events,
    combat_outcome_label, combat_report_statistics_text, combat_report_summary_text,
    combat_report_timeline_text, combat_report_units_text, mission_target_label, panel_background,
    panel_outline, ui_text_font,
};
use crate::presentation::{
    components::{ScrollIndicatorArea, ScrollIndicatorId},
    entity_visuals::EntityVisualCatalog,
    icons::{IconAssets, IconKind},
    scene::spawn_scroll_indicator,
};
use battlefield::{spawn_battlefield_panel, update_battlefield_panel};
use group_panel::{
    CombatPlanDraftState, handle_combat_plan_buttons, sync_combat_plan_draft,
    update_combat_plan_panel,
};

const MAX_ALLIED_ROWS: usize = 8;

/// Between navigation (140) and the intro-pitch modal (220) — must surface
/// above any open gameplay panel, but stay below the two true end-to-end
/// modals (intro-pitch, victory).
const COMBAT_UI_Z_INDEX: i32 = 200;
const COMBAT_ROOT_PADDING_PX: f32 = 12.0;
const COMBAT_ROOT_ROW_GAP_PX: f32 = 8.0;
const COMBAT_BODY_COLUMN_GAP_PX: f32 = 10.0;
/// COMBAT-UX-001-C: "VOS FORCES" — left column of the 3-column
/// Planification layout (doc §5.1).
const COMBAT_FORCES_COLUMN_WIDTH_PERCENT: f32 = 20.0;
/// "PARAMÈTRES SÉLECTIONNÉS" — right column of the same layout.
const COMBAT_PARAMETERS_COLUMN_WIDTH_PERCENT: f32 = 26.0;
const COMBAT_DOCTRINE_CARD_MIN_WIDTH_PX: f32 = 176.0;
const COMBAT_DOCTRINE_CARD_GAP_PX: f32 = 8.0;
/// COMBAT-UX-001-D: the 3 doctrines shown directly (doc §6 — "PRUDENT /
/// ÉQUILIBRÉ / AGRESSIF"), each mapped to one existing engine doctrine.
/// Purely a client-side grouping — `galactic_sim` has no tier/category
/// concept on `CombatDoctrineId`.
const SIMPLE_DOCTRINES: [(CombatDoctrineId, &str); 3] = [
    (CombatDoctrineId::DefensiveScreen, "PRUDENT"),
    (CombatDoctrineId::BalancedEngagement, "ÉQUILIBRÉ"),
    (CombatDoctrineId::ConcentratedAssault, "AGRESSIF"),
];
/// The remaining 3, behind "Tactiques avancées".
const ADVANCED_DOCTRINES: [CombatDoctrineId; 3] = [
    CombatDoctrineId::FlankingManeuver,
    CombatDoctrineId::DispersedFormation,
    CombatDoctrineId::TacticalAnalysis,
];
/// Display/shortcut order: simple doctrines first (keys 1-3), then advanced
/// (keys 4-6) — independent of `ALL_COMBAT_DOCTRINES`'s engine-internal
/// order, which no longer matches what's drawn on screen. See
/// `doctrine_shortcut_digit`.
const DOCTRINE_DISPLAY_ORDER: [CombatDoctrineId; 6] = [
    SIMPLE_DOCTRINES[0].0,
    SIMPLE_DOCTRINES[1].0,
    SIMPLE_DOCTRINES[2].0,
    ADVANCED_DOCTRINES[0],
    ADVANCED_DOCTRINES[1],
    ADVANCED_DOCTRINES[2],
];
const FOCUS_FIRE_PRIORITIES: [CombatTargetPriority; 5] = [
    CombatTargetPriority::Light,
    CombatTargetPriority::Medium,
    CombatTargetPriority::Heavy,
    CombatTargetPriority::Damaged,
    CombatTargetPriority::Support,
];

pub(crate) struct CombatUiPlugin;

impl Plugin for CombatUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatUiState>()
            .init_resource::<CombatPlanDraftState>()
            .add_systems(Startup, spawn_combat_screen)
            .add_systems(
                Update,
                (
                    resume_combat_queue_after_reload,
                    capture_combat_events,
                    sync_combat_plan_draft,
                )
                    .chain()
                    .before(collect_presentation_events)
                    .in_set(PresentationUpdateSet::View),
            )
            .add_systems(
                Update,
                (
                    tick_round_pause,
                    handle_combat_plan_buttons,
                    handle_combat_shortcuts,
                    handle_combat_buttons,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Interaction),
            )
            .add_systems(
                Update,
                (
                    update_combat_visibility,
                    update_combat_columns,
                    update_combat_plan_panel,
                    update_battlefield_panel,
                    update_doctrine_cards,
                    update_command_panel,
                    update_action_buttons,
                    update_reserve_intervention_visibility,
                    update_combat_feedback,
                    update_final_report,
                    update_final_report_visibility,
                    update_combat_planning_controls_visibility,
                    update_advanced_doctrines_visibility,
                    update_report_tabs_visibility,
                    update_combat_briefing_visibility,
                    update_combat_briefing,
                )
                    .chain()
                    .in_set(PresentationUpdateSet::Management),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatUiPhase {
    /// COMBAT-UX-001-B: shown once per combat, before planning — see
    /// `initial_combat_phase`.
    Briefing,
    AwaitingDoctrine,
    RoundPause,
    FinalReport,
}

/// COMBAT-UX-001-H: the detailed report's 4 tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatReportTab {
    Summary,
    Timeline,
    Statistics,
    Units,
}

/// Client-local state only — every field here is either derived from
/// `state.pending_combats`/`GameEventKind::Combat*` events, or purely
/// presentational (never a substitute source of truth for combat outcome;
/// see doc §16, "recharger pendant une animation doit reprendre sur un état
/// métier stable").
#[derive(Resource)]
pub(crate) struct CombatUiState {
    /// Combats awaiting a decision, in arrival order (doc §14.2, "nombre de
    /// combats en attente" / §11, "plusieurs combats simultanés").
    pub(crate) queue: Vec<MissionId>,
    /// The one currently displayed — Previous/Next cycles this (step 7).
    pub(crate) current: Option<MissionId>,
    pub(crate) phase: CombatUiPhase,
    /// 1-2s pause after a round resolves (doc §14.8), `Space` fast-forwards
    /// it to completion rather than taking a separate skip code path.
    pub(crate) round_pause_timer: Timer,
    pub(crate) chosen_doctrine: Option<CombatDoctrineId>,
    pub(crate) selected_intervention: Option<CombatIntervention>,
    /// Two-press retreat confirmation (doc §15, "retraite avec
    /// confirmation") — `Some(deadline)` while armed; expires back to `None`.
    pub(crate) retreat_armed_until: Option<Duration>,
    pub(crate) feedback: String,
    pub(crate) last_round_summary: Option<String>,
    /// COMBAT-UX-001 §12/§32 priority 2: the final report now defaults to a
    /// concise victory/defeat/retreat summary, with the pre-existing
    /// technical/tactical-timeline dump demoted behind this toggle.
    pub(crate) showing_detailed_report: bool,
    /// COMBAT-UX-001-B: missions that have already passed through
    /// `CombatUiPhase::Briefing` once — revisiting via queue-cycling must not
    /// re-show it. See `initial_combat_phase`.
    pub(crate) briefed: HashSet<MissionId>,
    /// COMBAT-UX-001-D: "Tactiques avancées" collapsible section — the 3
    /// non-simple doctrines only show when this is `true`.
    pub(crate) advanced_doctrines_expanded: bool,
    /// COMBAT-UX-001-H: which detailed-report tab is active.
    pub(crate) report_tab: CombatReportTab,
}

impl Default for CombatUiState {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            current: None,
            phase: CombatUiPhase::AwaitingDoctrine,
            round_pause_timer: Timer::from_seconds(1.5, TimerMode::Once),
            chosen_doctrine: None,
            selected_intervention: None,
            retreat_armed_until: None,
            feedback: String::new(),
            last_round_summary: None,
            showing_detailed_report: false,
            briefed: HashSet::new(),
            advanced_doctrines_expanded: false,
            report_tab: CombatReportTab::Summary,
        }
    }
}

/// The phase a mission should (re-)enter `ui.current` in — `Briefing` the
/// first time, `AwaitingDoctrine` on any later revisit (queue-cycling back to
/// a combat already briefed, or a reload where the player had already moved
/// past it).
fn initial_combat_phase(ui: &mut CombatUiState, mission_id: MissionId) -> CombatUiPhase {
    if ui.briefed.insert(mission_id) {
        CombatUiPhase::Briefing
    } else {
        CombatUiPhase::AwaitingDoctrine
    }
}

#[derive(Component)]
struct CombatUiRoot;

#[derive(Component)]
struct CombatHeaderText;

#[derive(Component)]
struct CombatIntelBarText;

#[derive(Component)]
struct CombatRoundLogText;

/// The concise victory/defeat/retreat summary shown by default in
/// `CombatUiPhase::FinalReport` — see `combat_result_summary_text`.
#[derive(Component)]
struct CombatResultSummaryText;

#[derive(Component)]
struct CombatFeedbackText;

#[derive(Component)]
struct CombatCommandText;

/// The feedback line and Retreat/AutoResolve/Validate row — hidden during
/// `CombatUiPhase::FinalReport`, replaced by `CombatReturnRow`.
#[derive(Component)]
struct CombatPreReportControls;

/// The doctrine bar and intervention row — hidden whenever the phase isn't
/// `AwaitingDoctrine` (i.e. during both `RoundPause` and `FinalReport`), so
/// the round-resolution moment clears down to just the battlefield instead
/// of leaving the planning controls visible-but-disabled (COMBAT-UX-001
/// §9/§32 priority 3 — "pendant la résolution, la configuration disparaît").
/// Mirrors `CombatPlanPanelRoot`'s existing `phase == AwaitingDoctrine`-only
/// visibility.
#[derive(Component)]
struct CombatPlanningControls;

/// The "Retour galaxie" row — visible only during `CombatUiPhase::FinalReport`.
#[derive(Component)]
struct CombatReturnRow;

/// Wraps the header text and the Previous/Next queue-cycling buttons (doc
/// §14.2, "plusieurs combats en attente").
#[derive(Component)]
struct CombatQueueNavRow;

/// COMBAT-UX-001-B: the briefing block (title/objectif/flotte/renseignement/
/// estimation/CTA) — visible only during `CombatUiPhase::Briefing`.
#[derive(Component)]
struct CombatBriefingControls;

/// The tactical body row (allied/battlefield/enemy columns) and the intel
/// bar — hidden only during `CombatUiPhase::Briefing`, so the tactical
/// screen doesn't show through behind the briefing card (doc §2.1/§4.2).
#[derive(Component)]
struct CombatHiddenDuringBriefing;

#[derive(Component)]
struct BriefingTitleText;

#[derive(Component)]
struct BriefingObjectiveText;

#[derive(Component)]
struct BriefingFleetRow(usize);

#[derive(Component)]
struct BriefingFleetIcon(usize);

#[derive(Component)]
struct BriefingFleetText(usize);

#[derive(Component)]
struct BriefingIntelText;

#[derive(Component)]
struct BriefingEstimateText;

fn spawn_column_frame(
    parent: &mut ChildSpawnerCommands,
    width_percent: f32,
    header: &str,
    scroll_id: ScrollIndicatorId,
    row_count: usize,
    spawn_row: impl Fn(&mut ChildSpawnerCommands, usize),
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(width_percent),
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
            group_panel::CombatPlanPanelRoot,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(header),
                ui_text_font(12.0),
                TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
            ));
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    ScrollIndicatorArea { id: scroll_id },
                ))
                .with_children(|list| {
                    for slot in 0..row_count {
                        spawn_row(list, slot);
                    }
                });
            spawn_scroll_indicator(panel, scroll_id);
        });
}

fn spawn_combat_screen(mut commands: Commands, icon_assets: Res<IconAssets>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(COMBAT_ROOT_PADDING_PX)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(COMBAT_ROOT_ROW_GAP_PX),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(COMBAT_UI_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Hidden,
            CombatUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                // Opaque panel, not just the 0.72-alpha root's translucent
                // backdrop — the header sits in the same screen band as the
                // always-visible resource bar (`presentation/scene.rs`'s
                // `spawn_resource_bar`, never hidden by any modal), so a
                // see-through background here doubles up its text with
                // combat's own header text underneath.
                BackgroundColor(panel_background()),
                Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                CombatQueueNavRow,
            ))
            .with_children(|header| {
                spawn_action_button(header, CombatUiAction::PreviousCombat, "◀");
                header.spawn((
                    Text::new(""),
                    ui_text_font(16.0),
                    TextColor(Color::srgb(0.92, 0.96, 1.0)),
                    CombatHeaderText,
                ));
                spawn_action_button(header, CombatUiAction::NextCombat, "▶");
            });

            spawn_combat_briefing(root, &icon_assets);

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(COMBAT_BODY_COLUMN_GAP_PX),
                    ..default()
                },
                CombatHiddenDuringBriefing,
            ))
            .with_children(|body| {
                spawn_column_frame(
                    body,
                    COMBAT_FORCES_COLUMN_WIDTH_PERCENT,
                    "VOS FORCES",
                    ScrollIndicatorId::CombatForcesColumn,
                    group_panel::MAX_DRAFT_STACK_ROWS,
                    group_panel::spawn_stack_row,
                );

                spawn_battlefield_panel(body, &icon_assets);

                body.spawn((
                    Node {
                        width: Val::Percent(COMBAT_PARAMETERS_COLUMN_WIDTH_PERCENT),
                        height: Val::Percent(100.0),
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(9.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(panel_background()),
                    Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                    group_panel::CombatPlanPanelRoot,
                ))
                .with_children(|panel| {
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollPosition::default(),
                            RelativeCursorPosition::default(),
                            ScrollIndicatorArea {
                                id: ScrollIndicatorId::CombatParametersColumn,
                            },
                        ))
                        .with_children(|content| {
                            group_panel::spawn_plan_heading(content);
                            group_panel::spawn_group_cards(content);

                            content.spawn((
                                Text::new("CHOISIR UNE DOCTRINE"),
                                ui_text_font(12.0),
                                TextColor(Color::srgba(0.78, 0.86, 1.0, 0.80)),
                                CombatPlanningControls,
                            ));

                            content
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_wrap: FlexWrap::Wrap,
                                        column_gap: Val::Px(COMBAT_DOCTRINE_CARD_GAP_PX),
                                        row_gap: Val::Px(8.0),
                                        ..default()
                                    },
                                    CombatPlanningControls,
                                ))
                                .with_children(|row| {
                                    for (doctrine, label) in SIMPLE_DOCTRINES {
                                        spawn_action_button(
                                            row,
                                            CombatUiAction::SelectDoctrine(doctrine),
                                            &format!(
                                                "{label} [{}]",
                                                doctrine_shortcut_digit(doctrine)
                                            ),
                                        );
                                    }
                                });

                            content
                                .spawn((Node::default(), CombatPlanningControls))
                                .with_children(|row| {
                                    spawn_action_button(
                                        row,
                                        CombatUiAction::ToggleAdvancedDoctrines,
                                        "Tactiques avancées ▼",
                                    );
                                });

                            content
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_wrap: FlexWrap::Wrap,
                                        column_gap: Val::Px(COMBAT_DOCTRINE_CARD_GAP_PX),
                                        row_gap: Val::Px(8.0),
                                        ..default()
                                    },
                                    Visibility::Hidden,
                                    AdvancedDoctrinesRow,
                                ))
                                .with_children(|cards| {
                                    for doctrine in ADVANCED_DOCTRINES {
                                        spawn_doctrine_card(cards, doctrine);
                                    }
                                });

                            content.spawn((
                                Text::new(""),
                                ui_text_font(12.0),
                                TextColor(Color::srgb(0.78, 0.86, 1.0)),
                                CombatCommandText,
                                CombatPlanningControls,
                            ));

                            content
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_wrap: FlexWrap::Wrap,
                                        column_gap: Val::Px(8.0),
                                        row_gap: Val::Px(8.0),
                                        ..default()
                                    },
                                    CombatPlanningControls,
                                ))
                                .with_children(|interventions| {
                                    for priority in FOCUS_FIRE_PRIORITIES {
                                        spawn_action_button(
                                            interventions,
                                            CombatUiAction::SelectFocusPriority(priority),
                                            focus_fire_button_label(priority),
                                        );
                                    }
                                    for group_id in CombatGroupPlanId::ALL {
                                        spawn_action_button(
                                            interventions,
                                            CombatUiAction::CommitReserve(group_id),
                                            reserve_button_label(group_id),
                                        );
                                    }
                                });
                        });
                    spawn_scroll_indicator(panel, ScrollIndicatorId::CombatParametersColumn);
                });
            });

            root.spawn((
                Text::new(""),
                ui_text_font(12.0),
                TextColor(Color::srgb(0.78, 0.86, 1.0)),
                CombatIntelBarText,
                CombatHiddenDuringBriefing,
            ));

            root.spawn((
                Text::new(""),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.92, 0.62, 0.56)),
                CombatFeedbackText,
                CombatPreReportControls,
            ));

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
                CombatPreReportControls,
            ))
            .with_children(|actions| {
                spawn_action_button(actions, CombatUiAction::Retreat, "Retraite [R]");
                actions
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|right| {
                        spawn_action_button(
                            right,
                            CombatUiAction::AutoResolve,
                            "Résolution auto [A]",
                        );
                        spawn_action_button(
                            right,
                            CombatUiAction::Validate,
                            "Continuer / valider [Entrée]",
                        );
                    });
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                Visibility::Hidden,
                CombatReturnRow,
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new(""),
                    ui_text_font(12.0),
                    TextColor(Color::srgb(0.86, 0.92, 0.98)),
                    Node {
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    CombatResultSummaryText,
                ));
                spawn_action_button(
                    row,
                    CombatUiAction::ToggleDetailedReport,
                    "Rapport détaillé",
                );
                row.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    Visibility::Hidden,
                    ReportTabBar,
                ))
                .with_children(|tabs| {
                    spawn_action_button(
                        tabs,
                        CombatUiAction::SelectReportTab(CombatReportTab::Summary),
                        "RÉSUMÉ",
                    );
                    spawn_action_button(
                        tabs,
                        CombatUiAction::SelectReportTab(CombatReportTab::Timeline),
                        "DÉROULEMENT",
                    );
                    spawn_action_button(
                        tabs,
                        CombatUiAction::SelectReportTab(CombatReportTab::Statistics),
                        "STATISTIQUES",
                    );
                    spawn_action_button(
                        tabs,
                        CombatUiAction::SelectReportTab(CombatReportTab::Units),
                        "UNITÉS",
                    );
                });
                spawn_action_button(
                    row,
                    CombatUiAction::ReturnToGalaxy,
                    "Retour galaxie [Entrée]",
                );
            });
        });
}

/// COMBAT-UX-001-B: the briefing card — title/objectif/flotte/renseignement/
/// estimation ennemie/CTA, everything sourced from data already computed
/// elsewhere in this file (`allied_stacks`, `enemy_intel`) — see
/// `update_combat_briefing`.
fn spawn_combat_briefing(root: &mut ChildSpawnerCommands, icon_assets: &IconAssets) {
    root.spawn((
        Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(620.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            align_self: AlignSelf::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(18.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        Visibility::Hidden,
        CombatBriefingControls,
    ))
    .with_children(|card| {
        card.spawn((
            Text::new(""),
            ui_text_font(20.0),
            TextColor(Color::srgb(0.92, 0.96, 1.0)),
            BriefingTitleText,
        ));
        card.spawn((
            Text::new("OBJECTIF"),
            ui_text_font(11.0),
            TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
        ));
        card.spawn((
            Text::new(""),
            ui_text_font(13.0),
            TextColor(Color::srgb(0.86, 0.92, 0.98)),
            BriefingObjectiveText,
        ));
        card.spawn((
            Text::new("VOTRE FLOTTE"),
            ui_text_font(11.0),
            TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
        ));
        for slot in 0..MAX_ALLIED_ROWS {
            card.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
                Visibility::Hidden,
                BriefingFleetRow(slot),
            ))
            .with_children(|row| {
                row.spawn((
                    ImageNode {
                        image: icon_assets.handle(IconKind::CombatUnit),
                        color: Color::WHITE,
                        ..default()
                    },
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BriefingFleetIcon(slot),
                ));
                row.spawn((
                    Text::new(""),
                    ui_text_font(13.0),
                    TextColor(Color::srgb(0.86, 0.92, 0.98)),
                    BriefingFleetText(slot),
                ));
            });
        }
        card.spawn((
            Text::new(""),
            ui_text_font(13.0),
            TextColor(Color::srgb(0.78, 0.86, 1.0)),
            BriefingIntelText,
        ));
        card.spawn((
            Text::new(""),
            ui_text_font(13.0),
            TextColor(Color::srgb(0.92, 0.80, 0.80)),
            BriefingEstimateText,
        ));
        spawn_action_button(
            card,
            CombatUiAction::StartPlanning,
            "PRÉPARER L'ASSAUT [Entrée]",
        );
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CombatUiAction {
    SelectDoctrine(CombatDoctrineId),
    SelectFocusPriority(CombatTargetPriority),
    CommitReserve(CombatGroupPlanId),
    Validate,
    Retreat,
    AutoResolve,
    ReturnToGalaxy,
    PreviousCombat,
    NextCombat,
    ToggleDetailedReport,
    StartPlanning,
    ToggleAdvancedDoctrines,
    SelectReportTab(CombatReportTab),
}

#[derive(Component, Clone, Copy)]
struct CombatActionButton(CombatUiAction);

#[derive(Component)]
struct DoctrineExtraText(CombatDoctrineId);

/// COMBAT-UX-001-D: the 3 advanced-doctrine cards, hidden unless
/// `CombatUiState.advanced_doctrines_expanded` — see
/// `update_advanced_doctrines_visibility`.
#[derive(Component)]
struct AdvancedDoctrinesRow;

/// COMBAT-UX-001-H: the 4 report-tab buttons — visible only when
/// `CombatUiState.showing_detailed_report`.
#[derive(Component)]
struct ReportTabBar;

#[derive(Component)]
struct RetreatButtonLabel;

#[derive(Component)]
struct CombatValidateButtonLabel;

#[derive(Component)]
struct AdvancedDoctrinesToggleLabel;

fn doctrine_name(doctrine: CombatDoctrineId) -> &'static str {
    match doctrine {
        CombatDoctrineId::BalancedEngagement => "ENGAGEMENT ÉQUILIBRÉ",
        CombatDoctrineId::ConcentratedAssault => "ASSAUT CONCENTRÉ",
        CombatDoctrineId::DefensiveScreen => "ÉCRAN DÉFENSIF",
        CombatDoctrineId::FlankingManeuver => "MANŒUVRE DE CONTOURNEMENT",
        CombatDoctrineId::DispersedFormation => "FORMATION DISPERSÉE",
        CombatDoctrineId::TacticalAnalysis => "ANALYSE TACTIQUE",
    }
}

fn doctrine_shortcut_digit(doctrine: CombatDoctrineId) -> usize {
    DOCTRINE_DISPLAY_ORDER
        .iter()
        .position(|candidate| *candidate == doctrine)
        .expect("every doctrine appears in DOCTRINE_DISPLAY_ORDER")
        + 1
}

fn focus_fire_button_label(priority: CombatTargetPriority) -> &'static str {
    match priority {
        CombatTargetPriority::Any => "Focus libre",
        CombatTargetPriority::Light => "Focus léger",
        CombatTargetPriority::Medium => "Focus moyen",
        CombatTargetPriority::Heavy => "Focus lourd",
        CombatTargetPriority::Damaged => "Focus endommagé",
        CombatTargetPriority::Support => "Focus soutien",
    }
}

fn reserve_button_label(group_id: CombatGroupPlanId) -> &'static str {
    match group_id {
        CombatGroupPlanId::Alpha => "Réserve Alpha",
        CombatGroupPlanId::Beta => "Réserve Beta",
        CombatGroupPlanId::Gamma => "Réserve Gamma",
    }
}

/// The one doctrine-specific mechanical effect `DoctrineOverview`'s
/// multipliers don't capture (targeting priority, support protection/bonus,
/// intel gain) — the only part of the card that's still hand-written prose;
/// everything else is derived from the real ruleset numbers (see
/// `doctrine_card_text`) so it can't drift from them the way the old fully
/// hand-written text did (playtest feedback: "Assaut concentré" claimed
/// "défense réduite" while its real multiplier *reduces* damage taken).
fn doctrine_mechanic_line(doctrine: CombatDoctrineId) -> &'static str {
    match doctrine {
        CombatDoctrineId::BalancedEngagement => "Aucun bonus ni malus marqué.",
        CombatDoctrineId::ConcentratedAssault => "Priorité aux unités lourdes ou endommagées.",
        CombatDoctrineId::DefensiveScreen => "Protège les groupes de soutien.",
        CombatDoctrineId::FlankingManeuver => "Cible les groupes de soutien adverses.",
        CombatDoctrineId::DispersedFormation => {
            "Limite la concentration des dégâts reçus sur une seule pile."
        }
        CombatDoctrineId::TacticalAnalysis => "Améliore le renseignement ce round.",
    }
}

fn per_mille_multiplier_text(value: u32) -> String {
    format!("×{:.2}", f64::from(value) / 1000.0)
}

/// Doc §14.6 card body — the real per-mille multipliers straight from the
/// ruleset (`combat_rules()`, read once at spawn time since the ruleset
/// never changes mid-session), plus the one doctrine-specific mechanical
/// line above and the counter relationship with its real multiplier.
fn doctrine_card_text(doctrine: CombatDoctrineId) -> String {
    let overview: DoctrineOverview = doctrine_overview(doctrine, combat_rules());
    let mut lines = vec![
        format!(
            "Offense {} · Dégâts reçus {}",
            per_mille_multiplier_text(overview.offense_multiplier_per_mille),
            per_mille_multiplier_text(overview.damage_taken_multiplier_per_mille),
        ),
        doctrine_mechanic_line(doctrine).to_string(),
    ];
    if overview.repetition_exempt {
        lines.push("Jamais pénalisée par la répétition.".to_string());
    }
    if let (Some(countered_by), Some(multiplier)) = (
        overview.countered_by,
        overview.counter_damage_dealt_multiplier_per_mille,
    ) {
        lines.push(format!(
            "Contrée par {} (dégâts infligés {} si contrée).",
            doctrine_name(countered_by),
            per_mille_multiplier_text(multiplier),
        ));
    }
    lines.join("\n")
}

/// The one dynamic "menace connue" hint doc §14.6's example shows — derived
/// only from already-revealed intel (`EnemyIntelView`), never from hidden
/// data. Not every doctrine has one; that's fine, the doc only shows it for
/// `ConcentratedAssault`.
fn known_threat_line(doctrine: CombatDoctrineId, enemy: &EnemyIntelView) -> Option<&'static str> {
    match doctrine {
        CombatDoctrineId::ConcentratedAssault => enemy
            .stacks
            .iter()
            .any(|stack| stack.target_class == Some(CombatTargetClass::Heavy))
            .then_some("Menace connue : des unités lourdes ont été détectées."),
        _ => None,
    }
}

fn spawn_doctrine_card(parent: &mut ChildSpawnerCommands, doctrine: CombatDoctrineId) {
    parent
        .spawn((
            Button,
            Node {
                flex_basis: Val::Percent(15.0),
                min_width: Val::Px(COMBAT_DOCTRINE_CARD_MIN_WIDTH_PX),
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(action_button_color(true, false, &Interaction::None)),
            Outline::new(
                Val::Px(1.0),
                Val::ZERO,
                action_button_outline(true, false, &Interaction::None),
            ),
            CombatActionButton(CombatUiAction::SelectDoctrine(doctrine)),
            UiPointerBlocker,
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(format!(
                    "{} [{}]",
                    doctrine_name(doctrine),
                    doctrine_shortcut_digit(doctrine)
                )),
                ui_text_font(11.0),
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
            ));
            card.spawn((
                Text::new(doctrine_card_text(doctrine)),
                ui_text_font(9.0),
                TextColor(Color::srgba(0.80, 0.86, 0.90, 0.90)),
            ));
            card.spawn((
                Text::new(""),
                ui_text_font(9.0),
                TextColor(Color::srgb(0.94, 0.72, 0.42)),
                DoctrineExtraText(doctrine),
            ));
        });
}

fn spawn_action_button(parent: &mut ChildSpawnerCommands, action: CombatUiAction, label: &str) {
    let mut entity = parent.spawn((
        Button,
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(action_button_color(true, false, &Interaction::None)),
        Outline::new(
            Val::Px(1.0),
            Val::ZERO,
            action_button_outline(true, false, &Interaction::None),
        ),
        CombatActionButton(action),
        UiPointerBlocker,
    ));
    entity.with_children(|button| {
        let mut text = button.spawn((
            Text::new(label),
            ui_text_font(12.0),
            TextColor(Color::WHITE),
        ));
        if action == CombatUiAction::Retreat {
            text.insert(RetreatButtonLabel);
        }
        if action == CombatUiAction::Validate {
            text.insert(CombatValidateButtonLabel);
        }
        if action == CombatUiAction::ToggleAdvancedDoctrines {
            text.insert(AdvancedDoctrinesToggleLabel);
        }
    });
}

/// Doc §14.7/§18-E "prévision qualitative" — five-tier reading only, never a
/// number: `qualitative_prediction` (`galactic_sim::combat::view`) is
/// deliberately coarse and only ever sees already-revealed data.
fn qualitative_prediction_label(prediction: QualitativePrediction) -> &'static str {
    match prediction {
        QualitativePrediction::VeryUnfavorable => "très défavorable",
        QualitativePrediction::Unfavorable => "défavorable",
        QualitativePrediction::Uncertain => "incertaine",
        QualitativePrediction::Favorable => "favorable",
        QualitativePrediction::VeryFavorable => "très favorable",
    }
}

fn target_class_label(class: CombatTargetClass) -> &'static str {
    match class {
        CombatTargetClass::Light => "légère",
        CombatTargetClass::Medium => "moyenne",
        CombatTargetClass::Heavy => "lourde",
    }
}

fn combat_unit_name(identity: CombatUnitRef) -> &'static str {
    match identity {
        CombatUnitRef::Ship(craftable) => craftable_definition(craftable).name,
        CombatUnitRef::PlanetaryForce(id) => default_ruleset()
            .planetary_presence()
            .definition(id)
            .map(|definition| definition.name)
            .unwrap_or("forces inconnues"),
    }
}

fn quantity_reveal_text(reveal: QuantityReveal) -> String {
    match reveal {
        QuantityReveal::Unknown => "quantité inconnue".to_string(),
        QuantityReveal::Verbal(band) => match band {
            QuantityBand::Few => "quelques unités".to_string(),
            QuantityBand::Some => "plusieurs unités".to_string(),
            QuantityBand::Many => "nombreuses unités".to_string(),
            QuantityBand::Numerous => "très nombreuses unités".to_string(),
        },
        QuantityReveal::Range { minimum, maximum } => format!("environ {minimum} à {maximum}"),
        QuantityReveal::NearExact(value) => format!("environ {value}"),
        QuantityReveal::Exact(value) => format!("{value}"),
    }
}

fn integrity_reveal_text(reveal: IntegrityReveal) -> String {
    match reveal {
        IntegrityReveal::Unknown => "intégrité inconnue".to_string(),
        IntegrityReveal::Qualitative(band) => format!(
            "intégrité {}",
            match band {
                IntegrityBand::Critical => "critique",
                IntegrityBand::Low => "faible",
                IntegrityBand::Moderate => "moyenne",
                IntegrityBand::High => "élevée",
            }
        ),
        IntegrityReveal::Exact(percent) => format!("intégrité {percent} %"),
    }
}

// Every `&mut Text` query in this module filters on `With<Self>` plus
// `Without<every other marker>` — not just the ones it happens to share a
// system with today. Bevy's query-conflict check is per-system-signature and
// structural (it can't see that two marker components never coexist on an
// entity unless the filters say so), so a pair that's disjoint by omission
// in one system panics the moment the same two query types are combined in
// a different system (as `update_final_report` did, combining
// `CombatHeaderTextQuery` with `CombatRoundLogTextQuery`, neither of which
// excluded the other). Keeping every alias symmetric against the full
// marker set avoids re-hitting this every time a new system recombines them.
type CombatHeaderTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatCommandText>,
        Without<CombatValidateButtonLabel>,
        Without<CombatResultSummaryText>,
    ),
>;

type CombatIntelBarTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatIntelBarText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatCommandText>,
        Without<CombatValidateButtonLabel>,
    ),
>;

fn update_combat_columns(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    mut header_texts: CombatHeaderTextQuery,
    mut intel_texts: CombatIntelBarTextQuery,
) {
    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        return;
    };

    let target_label = mission_target_label(
        simulation.simulation(),
        MissionTarget::Planet {
            system_id: pending.planet_id.system_id(),
            planet_id: pending.planet_id,
        },
    );
    let phase_label = match ui.phase {
        CombatUiPhase::Briefing => "Briefing",
        CombatUiPhase::AwaitingDoctrine => "En attente de votre choix",
        CombatUiPhase::RoundPause => "Résolution du round…",
        CombatUiPhase::FinalReport => "Combat terminé",
    };
    if let Ok(mut text) = header_texts.single_mut() {
        text.0 = format!(
            "ASSAUT ORBITAL — {target_label}      ROUND {}/{}      {phase_label}      File : {}",
            pending.round() + 1,
            pending.maximum_rounds(),
            ui.queue.len(),
        );
    }

    let enemy = enemy_intel(pending);
    let allied = allied_stacks(pending);
    if let Ok(mut text) = intel_texts.single_mut() {
        let note = if enemy.intel_percent >= 95 {
            "Renseignement quasi complet."
        } else {
            "Certaines unités et capacités restent inconnues."
        };
        let prediction = qualitative_prediction_label(qualitative_prediction(&allied, &enemy));
        text.0 = format!(
            "RENSEIGNEMENT : {} %\n{note}\nPrévision : {prediction}",
            enemy.intel_percent
        );
    }
}

type BriefingTitleTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<BriefingTitleText>,
        Without<BriefingObjectiveText>,
        Without<BriefingFleetText>,
        Without<BriefingIntelText>,
        Without<BriefingEstimateText>,
    ),
>;

type BriefingObjectiveTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<BriefingObjectiveText>,
        Without<BriefingTitleText>,
        Without<BriefingFleetText>,
        Without<BriefingIntelText>,
        Without<BriefingEstimateText>,
    ),
>;

type BriefingFleetRowQuery<'w, 's> =
    Query<'w, 's, (&'static BriefingFleetRow, &'static mut Visibility)>;

type BriefingFleetIconQuery<'w, 's> =
    Query<'w, 's, (&'static BriefingFleetIcon, &'static mut ImageNode)>;

type BriefingFleetTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BriefingFleetText, &'static mut Text),
    (
        Without<BriefingTitleText>,
        Without<BriefingObjectiveText>,
        Without<BriefingIntelText>,
        Without<BriefingEstimateText>,
    ),
>;

type BriefingIntelTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<BriefingIntelText>,
        Without<BriefingTitleText>,
        Without<BriefingObjectiveText>,
        Without<BriefingFleetText>,
        Without<BriefingEstimateText>,
    ),
>;

type BriefingEstimateTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<BriefingEstimateText>,
        Without<BriefingTitleText>,
        Without<BriefingObjectiveText>,
        Without<BriefingFleetText>,
        Without<BriefingIntelText>,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn update_combat_briefing(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut title_text: BriefingTitleTextQuery,
    mut objective_text: BriefingObjectiveTextQuery,
    mut fleet_rows: BriefingFleetRowQuery,
    mut fleet_icons: BriefingFleetIconQuery,
    mut fleet_texts: BriefingFleetTextQuery,
    mut intel_text: BriefingIntelTextQuery,
    mut estimate_text: BriefingEstimateTextQuery,
) {
    if ui.phase != CombatUiPhase::Briefing {
        return;
    }
    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
        return;
    };

    let target_label = mission_target_label(
        simulation.simulation(),
        MissionTarget::Planet {
            system_id: pending.planet_id.system_id(),
            planet_id: pending.planet_id,
        },
    );
    if let Ok(mut text) = title_text.single_mut() {
        text.0 = format!("ASSAUT ORBITAL\n{target_label}");
    }
    if let Ok(mut text) = objective_text.single_mut() {
        text.0 = "Neutraliser les défenses orbitales de la planète.".to_string();
    }

    let allied = allied_stacks(pending);
    for (row, mut visibility) in &mut fleet_rows {
        let next = if allied.get(row.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (marker, mut icon) in &mut fleet_icons {
        if let Some(stack) = allied.get(marker.0) {
            icon.image = match stack.identity {
                CombatUnitRef::Ship(craftable) => entity_visuals.ship(craftable),
                CombatUnitRef::PlanetaryForce(id) => entity_visuals.force(id),
            };
            icon.color = Color::WHITE;
        }
    }
    for (marker, mut text) in &mut fleet_texts {
        if let Some(stack) = allied.get(marker.0) {
            text.0 = format!(
                "{} × {}",
                stack.surviving_quantity,
                combat_unit_name(stack.identity)
            );
        }
    }

    let enemy = enemy_intel(pending);
    if let Ok(mut text) = intel_text.single_mut() {
        text.0 = format!("RENSEIGNEMENT : {} %", enemy.intel_percent);
    }
    if let Ok(mut text) = estimate_text.single_mut() {
        text.0 = format!(
            "ESTIMATION ENNEMIE\n{} contact(s) détecté(s)\nComposition incertaine.\nForces estimées : {}",
            enemy.stacks.len(),
            threat_level_label(enemy.threat_level),
        );
    }
}

fn threat_level_label(level: ThreatLevel) -> &'static str {
    match level {
        ThreatLevel::Low => "faibles",
        ThreatLevel::Moderate => "moyennes",
        ThreatLevel::High => "élevées",
        ThreatLevel::Overwhelming => "écrasantes",
    }
}

fn update_combat_visibility(
    ui: Res<CombatUiState>,
    mut roots: Query<&mut Visibility, With<CombatUiRoot>>,
) {
    let visibility = if ui.current.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut root in &mut roots {
        if *root != visibility {
            *root = visibility;
        }
    }
}

/// Doc §16: reloading a save with a pending combat must reopen the screen —
/// `CombatDecisionRequired` only fires once, at the moment a combat begins,
/// so a freshly loaded `Simulation` (whose pending combats already existed
/// before the load) never re-emits it. Runs every frame — cheap (at most a
/// handful of pending combats) and idempotent: it only ever adds mission ids
/// `capture_combat_events` hasn't already tracked, so it never fights normal
/// event-driven flow, including after `ReturnToGalaxy` intentionally empties
/// `current` with no pending combats left.
fn resume_combat_queue_after_reload(
    mut ui: ResMut<CombatUiState>,
    simulation: Res<SimulationResource>,
) {
    let pending_ids: Vec<MissionId> = simulation
        .simulation()
        .state()
        .pending_combats
        .iter()
        .map(|pending| pending.mission_id)
        .collect();
    if pending_ids.is_empty() {
        return;
    }
    for mission_id in pending_ids {
        if !ui.queue.contains(&mission_id) {
            ui.queue.push(mission_id);
        }
    }
    if ui.current.is_none()
        && let Some(mission_id) = ui.queue.first().copied()
    {
        ui.current = Some(mission_id);
        ui.phase = initial_combat_phase(&mut ui, mission_id);
        ui.chosen_doctrine = None;
        ui.selected_intervention = None;
    }
}

/// Opens the screen on `CombatDecisionRequired` (one pending combat has just
/// appeared) and closes it cleanly on `CombatCompleted` — the latter can
/// arrive without any intermediate `CombatRoundResolved` (e.g. after
/// `AutoResolveCombat`/`RetreatFromCombat`), so this must work from
/// whatever `phase` the screen is currently in, never assume one (doc §16:
/// "l'interface doit se fermer proprement si un événement de finalisation
/// est reçu avant son rafraîchissement").
fn capture_combat_events(mut ui: ResMut<CombatUiState>, simulation: Res<SimulationResource>) {
    for event in &simulation.pending_events {
        match event.kind {
            GameEventKind::CombatDecisionRequired(CombatDecisionRequired {
                mission_id, ..
            }) => {
                if !ui.queue.contains(&mission_id) {
                    ui.queue.push(mission_id);
                }
                if ui.current.is_none() {
                    ui.current = Some(mission_id);
                    ui.phase = initial_combat_phase(&mut ui, mission_id);
                    ui.chosen_doctrine = None;
                    ui.selected_intervention = None;
                }
            }
            GameEventKind::CombatRoundResolved(CombatRoundResolved { mission_id, .. }) => {
                if ui.current != Some(mission_id) {
                    continue;
                }
                ui.last_round_summary = simulation
                    .simulation()
                    .state()
                    .pending_combat(mission_id)
                    .and_then(|pending| {
                        round_history(pending)
                            .last()
                            .map(|record| combat_round_summary(record, pending))
                    });
                ui.phase = CombatUiPhase::RoundPause;
                ui.round_pause_timer = Timer::from_seconds(1.5, TimerMode::Once);
                ui.chosen_doctrine = None;
                ui.selected_intervention = None;
                ui.retreat_armed_until = None;
                ui.feedback.clear();
            }
            GameEventKind::CombatPlanConfirmed(CombatPlanConfirmed { mission_id })
                if ui.current == Some(mission_id) =>
            {
                ui.feedback = "Plan de bataille confirmé.".to_string();
            }
            GameEventKind::CombatPlanRejected(CombatPlanRejected { mission_id, error })
                if ui.current == Some(mission_id) =>
            {
                ui.feedback = combat_command_error_text(error);
            }
            GameEventKind::CombatCompleted(CombatCompleted { mission_id, .. }) => {
                ui.queue.retain(|id| *id != mission_id);
                if ui.current != Some(mission_id) {
                    continue;
                }
                ui.phase = CombatUiPhase::FinalReport;
                ui.chosen_doctrine = None;
                ui.selected_intervention = None;
                ui.retreat_armed_until = None;
                ui.showing_detailed_report = false;
            }
            GameEventKind::CombatDoctrineRejected(CombatDoctrineRejected {
                mission_id,
                error,
                ..
            }) => {
                if ui.current != Some(mission_id) {
                    continue;
                }
                ui.feedback = combat_command_error_text(error);
            }
            GameEventKind::CombatRetreatRejected(CombatRetreatRejected { mission_id, error }) => {
                if ui.current != Some(mission_id) {
                    continue;
                }
                ui.feedback = combat_command_error_text(error);
            }
            GameEventKind::CombatAutoResolveRejected(CombatAutoResolveRejected {
                mission_id,
                error,
            }) => {
                if ui.current != Some(mission_id) {
                    continue;
                }
                ui.feedback = combat_command_error_text(error);
            }
            _ => {}
        }
    }
}

/// French text for every `CombatCommandError` variant (doc §10), mirroring
/// `fleet_ui.rs`'s `fleet_error_text` pattern.
fn combat_command_error_text(error: CombatCommandError) -> String {
    match error {
        CombatCommandError::UnknownCombat(_) => {
            "Ce combat n'est plus en attente (déjà résolu ou retiré).".to_string()
        }
        CombatCommandError::Access(AuthorizationError::NotOwner { .. }) => {
            "Ce combat n'appartient pas à votre faction.".to_string()
        }
        CombatCommandError::Access(_) => {
            "Action refusée : faction invalide ou inactive.".to_string()
        }
        CombatCommandError::StaleRound { expected, found } => {
            format!("Ce round ({found}) a déjà été résolu — round attendu : {expected}.")
        }
        CombatCommandError::PlanLocked { round } => {
            format!("Le plan est verrouillé depuis le round {round}.")
        }
        CombatCommandError::InvalidPlan(error) => match error {
            CombatPlanValidationError::EmptyPlan => "Le plan de bataille est vide.".to_string(),
            CombatPlanValidationError::TooManyGroups { maximum, .. } => {
                format!("Le plan dépasse la limite de {maximum} groupes.")
            }
            CombatPlanValidationError::DuplicateGroup(_) => {
                "Un groupe tactique est défini plusieurs fois.".to_string()
            }
            CombatPlanValidationError::EmptyGroup(_) => {
                "Un groupe tactique ne contient aucune unité.".to_string()
            }
            CombatPlanValidationError::UnknownStack(_) => {
                "Le plan référence une unité inconnue.".to_string()
            }
            CombatPlanValidationError::DuplicateStack(_) => {
                "Une unité est assignée à plusieurs groupes.".to_string()
            }
            CombatPlanValidationError::MissingStack(_) => {
                "Toutes les unités opérationnelles doivent être assignées.".to_string()
            }
        },
        CombatCommandError::InsufficientCommandPoints {
            required,
            available,
        } => format!(
            "Points de commandement insuffisants : {required} requis, {available} disponible(s)."
        ),
        CombatCommandError::InvalidIntervention(error) => match error {
            CombatInterventionError::CombatNotStarted => {
                "Les ordres de commandement seront disponibles après le premier engagement."
                    .to_string()
            }
            CombatInterventionError::InvalidFocusPriority => {
                "Cette priorité de tir ne peut pas être concentrée.".to_string()
            }
            CombatInterventionError::NoActiveGroup => {
                "Aucun groupe actif ne peut recevoir cet ordre.".to_string()
            }
            CombatInterventionError::ReserveGroupMissing(_) => {
                "Ce groupe de réserve n'existe pas dans le plan.".to_string()
            }
            CombatInterventionError::ReserveGroupEmpty(_) => {
                "Ce groupe de réserve ne contient aucune unité.".to_string()
            }
            CombatInterventionError::ReserveGroupInoperable(_) => {
                "Cette réserve n'a plus d'unité opérationnelle.".to_string()
            }
            CombatInterventionError::GroupIsNotReserve(_) => {
                "Ce groupe est déjà engagé.".to_string()
            }
        },
    }
}

fn primary_target_from_exchanges(
    record: &CombatRoundRecord,
    group_id: CombatGroupPlanId,
) -> Option<galactic_sim::CombatStackId> {
    record
        .attacker_exchanges
        .iter()
        .filter(|exchange| exchange.source_group == group_id)
        .max_by_key(|exchange| exchange.allocated_damage)
        .map(|exchange| exchange.target)
}

fn group_damage_from_exchanges(record: &CombatRoundRecord, group_id: CombatGroupPlanId) -> u128 {
    record
        .attacker_exchanges
        .iter()
        .filter(|exchange| exchange.source_group == group_id)
        .fold(0, |total, exchange| {
            total.saturating_add(exchange.allocated_damage)
        })
}

/// Average `AlliedStackView::integrity_percent` across a group's currently
/// assigned stacks — `None` if the group has no stacks (nothing to average).
fn group_aggregate_integrity_percent(
    pending: &PendingCombat,
    group_id: CombatGroupPlanId,
) -> Option<u8> {
    let stacks = &pending
        .plan()?
        .groups
        .iter()
        .find(|group| group.id == group_id)?
        .stacks;
    if stacks.is_empty() {
        return None;
    }
    let allied = allied_stacks(pending);
    let mut total: u32 = 0;
    let mut count: u32 = 0;
    for stack_id in stacks {
        if let Some(stack) = allied.iter().find(|stack| stack.stack_id == *stack_id) {
            total += u32::from(stack.integrity_percent);
            count += 1;
        }
    }
    (count > 0).then(|| (total / count) as u8)
}

/// COMBAT-UX-001-F: one line per group actually in the plan — name, role,
/// aggregate integrity, and what it did this round (engaged a target,
/// stayed in reserve, or sat out). Groups with nothing assigned are skipped
/// entirely rather than shown empty (doc §10.3 — "l'UI doit filtrer").
fn combat_round_group_status_lines(
    record: &CombatRoundRecord,
    pending: &PendingCombat,
) -> Vec<String> {
    let enemy = enemy_intel(pending);
    let mut lines = Vec::new();
    for group_id in CombatGroupPlanId::ALL {
        let Some(group) = pending
            .plan()
            .and_then(|plan| plan.groups.iter().find(|group| group.id == group_id))
        else {
            continue;
        };
        if group.stacks.is_empty() {
            continue;
        }
        let integrity = group_aggregate_integrity_percent(pending, group_id)
            .map(|percent| format!("{percent}%"))
            .unwrap_or_else(|| "—".to_string());
        let damage = group_damage_from_exchanges(record, group_id);
        let action = if damage > 0 {
            let target = primary_target_from_exchanges(record, group_id)
                .and_then(|stack_id| enemy.stacks.iter().find(|stack| stack.stack_id == stack_id))
                .map(battlefield::enemy_contact_short_label)
                .unwrap_or_else(|| "contact".to_string());
            format!(
                "a engagé {target} ({})",
                battlefield::compact_damage(damage)
            )
        } else if group.role == CombatGroupRole::Reserve {
            "est resté en réserve".to_string()
        } else {
            "n'a pas engagé ce round".to_string()
        };
        lines.push(format!(
            "{} — {} — intégrité {integrity} — {action}.",
            group_panel::group_label(group_id),
            group_panel::role_label(group.role),
        ));
    }
    lines
}

/// Enemy stacks destroyed this round (from `StackDestroyed` events) vs.
/// merely damaged (a distinct target in `attacker_exchanges` that wasn't
/// destroyed) — doc §10's "2 détruits / 1 endommagé" summary.
fn enemy_destroyed_and_damaged_counts(record: &CombatRoundRecord) -> (usize, usize) {
    let destroyed: std::collections::HashSet<_> = record
        .notable_events
        .iter()
        .filter_map(|event| match event {
            CombatRoundEvent::StackDestroyed {
                side: CombatSide::Defender,
                stack_id,
            } => Some(*stack_id),
            _ => None,
        })
        .collect();
    let damaged = record
        .attacker_exchanges
        .iter()
        .map(|exchange| exchange.target)
        .filter(|target| !destroyed.contains(target))
        .collect::<std::collections::HashSet<_>>()
        .len();
    (destroyed.len(), damaged)
}

/// Doc §10 "ROUND TERMINÉ" — a structured per-group/enemy recap of the round
/// just played, replacing the old one-line dump.
fn combat_round_summary(record: &CombatRoundRecord, pending: &PendingCombat) -> String {
    let mut lines = vec![format!(
        "ROUND {} TERMINÉ — {} : dégâts infligés {}, dégâts subis {}.",
        record.round,
        doctrine_name(record.attacker_doctrine),
        record.attacker_damage,
        record.defender_damage,
    )];
    lines.extend(combat_round_group_status_lines(record, pending));

    let (destroyed, damaged) = enemy_destroyed_and_damaged_counts(record);
    lines.push(format!(
        "ENNEMI — {destroyed} détruit(s), {damaged} endommagé(s)."
    ));

    if record
        .notable_events
        .iter()
        .any(|event| matches!(event, CombatRoundEvent::CounterTriggered { .. }))
    {
        lines.push("Une doctrine a été contrée ce round.".to_string());
    }
    lines.join("\n")
}

/// Advances the post-round pause (doc §14.8), `Space` fast-forwarding it to
/// completion rather than a separate skip code path.
fn tick_round_pause(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<CombatUiState>,
) {
    if ui.phase != CombatUiPhase::RoundPause {
        return;
    }
    if keyboard.just_pressed(KeyCode::Space)
        || ui.round_pause_timer.tick(time.delta()).just_finished()
    {
        ui.phase = CombatUiPhase::AwaitingDoctrine;
    }
}

fn persistent_doctrine(pending: &PendingCombat) -> CombatDoctrineId {
    pending
        .plan()
        .map(|plan| plan.doctrine)
        .unwrap_or(CombatDoctrineId::BalancedEngagement)
}

fn doctrine_change_cost(pending: &PendingCombat, doctrine: Option<CombatDoctrineId>) -> u8 {
    let Some(doctrine) = doctrine else {
        return 0;
    };
    if pending.round() == 0 || persistent_doctrine(pending) == doctrine {
        0
    } else {
        combat_rules().command().change_doctrine_cost()
    }
}

fn intervention_command_cost(intervention: CombatIntervention) -> u8 {
    match intervention {
        CombatIntervention::FocusFire { .. } => combat_rules().command().focus_fire_cost(),
        CombatIntervention::CommitReserve { .. } => combat_rules().command().commit_reserve_cost(),
    }
}

fn round_command_cost(
    pending: &PendingCombat,
    doctrine: Option<CombatDoctrineId>,
    intervention: Option<CombatIntervention>,
) -> u8 {
    let mut cost = doctrine_change_cost(pending, doctrine);
    if let Some(intervention) = intervention {
        cost = cost.saturating_add(intervention_command_cost(intervention));
    }
    cost
}

fn reserve_group_is_available(pending: &PendingCombat, group_id: CombatGroupPlanId) -> bool {
    pending
        .plan()
        .and_then(|plan| plan.groups.iter().find(|group| group.id == group_id))
        .is_some_and(|group| {
            group.role == CombatGroupRole::Reserve
                && group.stacks.iter().any(|stack_id| {
                    allied_stacks(pending)
                        .iter()
                        .any(|stack| stack.stack_id == *stack_id && stack.surviving_quantity > 0)
                })
        })
}

fn focus_fire_is_available(pending: &PendingCombat, priority: CombatTargetPriority) -> bool {
    priority != CombatTargetPriority::Any
        && pending.plan().is_some_and(|plan| {
            plan.groups.iter().any(|group| {
                group.role != CombatGroupRole::Reserve
                    && group.stacks.iter().any(|stack_id| {
                        allied_stacks(pending).iter().any(|stack| {
                            stack.stack_id == *stack_id && stack.surviving_quantity > 0
                        })
                    })
            })
        })
}

fn intervention_is_available(
    pending: &PendingCombat,
    selected_doctrine: Option<CombatDoctrineId>,
    intervention: CombatIntervention,
) -> bool {
    if pending.round() == 0 {
        return false;
    }
    let intervention_is_valid = match intervention {
        CombatIntervention::FocusFire { priority } => focus_fire_is_available(pending, priority),
        CombatIntervention::CommitReserve { group_id } => {
            reserve_group_is_available(pending, group_id)
        }
    };
    intervention_is_valid
        && round_command_cost(pending, selected_doctrine, Some(intervention))
            <= pending.command_points_remaining()
}

/// COMBAT-UX-001-F §10.3 "l'UI doit filtrer selon... l'état des groupes" —
/// a "Réserve X" button only makes sense to show at all when X is actually a
/// populated reserve group; unlike `update_action_buttons`'s greying (which
/// also covers the separate round-0/command-point gates), this hides the
/// button entirely. `intervention_is_available`'s "no enemy support known"
/// case for Focus-Fire priorities is intentionally NOT covered here —
/// `CombatStackView` doesn't expose enemy tactical role client-side, and
/// adding that would be a `galactic_sim` change, out of scope for a
/// client-only UX ticket (doc §25).
fn update_reserve_intervention_visibility(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    mut buttons: Query<(&CombatActionButton, &mut Visibility)>,
) {
    let pending = ui
        .current
        .and_then(|mission_id| simulation.simulation().state().pending_combat(mission_id));
    for (button, mut visibility) in &mut buttons {
        let CombatUiAction::CommitReserve(group_id) = button.0 else {
            continue;
        };
        let next = if pending.is_some_and(|pending| reserve_group_is_available(pending, group_id)) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

fn action_intervention(action: CombatUiAction) -> Option<CombatIntervention> {
    match action {
        CombatUiAction::SelectFocusPriority(priority) => {
            Some(CombatIntervention::FocusFire { priority })
        }
        CombatUiAction::CommitReserve(group_id) => {
            Some(CombatIntervention::CommitReserve { group_id })
        }
        _ => None,
    }
}

fn apply_combat_action(
    action: CombatUiAction,
    simulation: &mut SimulationResource,
    ui: &mut CombatUiState,
    draft: &CombatPlanDraftState,
    now: Duration,
) {
    let required_phase = match action {
        CombatUiAction::ReturnToGalaxy
        | CombatUiAction::ToggleDetailedReport
        | CombatUiAction::SelectReportTab(_) => CombatUiPhase::FinalReport,
        CombatUiAction::StartPlanning => CombatUiPhase::Briefing,
        _ => CombatUiPhase::AwaitingDoctrine,
    };
    if ui.phase != required_phase {
        return;
    }

    match action {
        CombatUiAction::SelectDoctrine(doctrine) => {
            let Some(mission_id) = ui.current else {
                return;
            };
            let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
                return;
            };
            if round_command_cost(pending, Some(doctrine), ui.selected_intervention)
                > pending.command_points_remaining()
            {
                return;
            }
            ui.chosen_doctrine = Some(doctrine);
            if let Some(intervention) = ui.selected_intervention
                && !intervention_is_available(pending, ui.chosen_doctrine, intervention)
            {
                ui.selected_intervention = None;
            }
            ui.retreat_armed_until = None;
        }
        CombatUiAction::SelectFocusPriority(priority) => {
            let Some(mission_id) = ui.current else {
                return;
            };
            let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
                return;
            };
            let intervention = CombatIntervention::FocusFire { priority };
            if !intervention_is_available(pending, ui.chosen_doctrine, intervention) {
                return;
            }
            ui.selected_intervention = if ui.selected_intervention == Some(intervention) {
                None
            } else {
                Some(intervention)
            };
            ui.retreat_armed_until = None;
        }
        CombatUiAction::CommitReserve(group_id) => {
            let Some(mission_id) = ui.current else {
                return;
            };
            let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
                return;
            };
            let intervention = CombatIntervention::CommitReserve { group_id };
            if !intervention_is_available(pending, ui.chosen_doctrine, intervention) {
                return;
            }
            ui.selected_intervention = if ui.selected_intervention == Some(intervention) {
                None
            } else {
                Some(intervention)
            };
            ui.retreat_armed_until = None;
        }
        CombatUiAction::Validate => {
            let Some(mission_id) = ui.current else {
                return;
            };
            let Some(pending) = simulation.simulation().state().pending_combat(mission_id) else {
                return;
            };
            if pending.round() == 0 && draft.is_dirty() {
                ui.feedback = "Confirmez le plan avant l'assaut.".to_string();
                return;
            }
            if ui.selected_intervention.is_some_and(|intervention| {
                !intervention_is_available(pending, ui.chosen_doctrine, intervention)
            }) || round_command_cost(pending, ui.chosen_doctrine, ui.selected_intervention)
                > pending.command_points_remaining()
            {
                return;
            }
            let round = pending.round() + 1;
            apply_simulation_command(
                simulation,
                GameAction::ChooseCombatDoctrine {
                    mission_id,
                    round,
                    doctrine: ui.chosen_doctrine,
                    intervention: ui.selected_intervention,
                },
            );
        }
        CombatUiAction::Retreat => {
            let Some(mission_id) = ui.current else {
                return;
            };
            match ui.retreat_armed_until {
                Some(deadline) if now <= deadline => {
                    apply_simulation_command(
                        simulation,
                        GameAction::RetreatFromCombat { mission_id },
                    );
                    ui.retreat_armed_until = None;
                }
                _ => {
                    ui.retreat_armed_until = Some(now + Duration::from_secs(3));
                }
            }
        }
        CombatUiAction::AutoResolve => {
            let Some(mission_id) = ui.current else {
                return;
            };
            apply_simulation_command(simulation, GameAction::AutoResolveCombat { mission_id });
        }
        CombatUiAction::ReturnToGalaxy => {
            if let Some(next) = ui.queue.first().copied() {
                ui.current = Some(next);
                ui.phase = initial_combat_phase(ui, next);
            } else {
                ui.current = None;
            }
            ui.chosen_doctrine = None;
            ui.selected_intervention = None;
            ui.retreat_armed_until = None;
            ui.feedback.clear();
            ui.last_round_summary = None;
            ui.showing_detailed_report = false;
        }
        CombatUiAction::PreviousCombat => cycle_combat_queue(ui, true),
        CombatUiAction::NextCombat => cycle_combat_queue(ui, false),
        CombatUiAction::ToggleDetailedReport => {
            ui.showing_detailed_report = !ui.showing_detailed_report;
        }
        CombatUiAction::StartPlanning => {
            ui.phase = CombatUiPhase::AwaitingDoctrine;
        }
        CombatUiAction::ToggleAdvancedDoctrines => {
            ui.advanced_doctrines_expanded = !ui.advanced_doctrines_expanded;
        }
        CombatUiAction::SelectReportTab(tab) => {
            ui.report_tab = tab;
        }
    }
}

/// Previous/Next across simultaneous pending combats (doc §14.2) — purely
/// client-local navigation among already-known missions, mirroring
/// `cycle_management_colony`'s local-index pattern but with no `GameAction`
/// (which combat is *displayed* is not simulation state).
fn cycle_combat_queue(ui: &mut CombatUiState, reverse: bool) {
    if ui.queue.len() < 2 {
        return;
    }
    let current_index = ui
        .current
        .and_then(|id| ui.queue.iter().position(|candidate| *candidate == id))
        .unwrap_or(0);
    let next_index = if reverse {
        current_index.checked_sub(1).unwrap_or(ui.queue.len() - 1)
    } else {
        (current_index + 1) % ui.queue.len()
    };
    let next = ui.queue[next_index];
    ui.current = Some(next);
    ui.phase = initial_combat_phase(ui, next);
    ui.chosen_doctrine = None;
    ui.selected_intervention = None;
    ui.retreat_armed_until = None;
    ui.feedback.clear();
    ui.last_round_summary = None;
    ui.showing_detailed_report = false;
}

type CombatActionButtonQuery<'w, 's> =
    Query<'w, 's, (&'static Interaction, &'static CombatActionButton), Changed<Interaction>>;

fn handle_combat_buttons(
    mut interactions: CombatActionButtonQuery,
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<CombatUiState>,
    draft: Res<CombatPlanDraftState>,
    time: Res<Time>,
) {
    for (interaction, button) in &mut interactions {
        if matches!(interaction, Interaction::Pressed) {
            apply_combat_action(button.0, &mut simulation, &mut ui, &draft, time.elapsed());
        }
    }
}

fn combat_shortcut(
    keyboard: &ButtonInput<KeyCode>,
    phase: CombatUiPhase,
) -> Option<CombatUiAction> {
    if phase == CombatUiPhase::Briefing {
        return keyboard
            .just_pressed(KeyCode::Enter)
            .then_some(CombatUiAction::StartPlanning);
    }
    if phase == CombatUiPhase::FinalReport {
        // Doc §15: `Escape` is inert on the tactical screen while a decision
        // is pending or a round is resolving (never silently abandons a
        // mandatory combat) — only meaningful once there is nothing left to
        // decide, where it's equivalent to the "Retour galaxie" button.
        return (keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Escape))
            .then_some(CombatUiAction::ReturnToGalaxy);
    }
    if phase != CombatUiPhase::AwaitingDoctrine {
        return None;
    }
    const DOCTRINE_KEYS: [KeyCode; 6] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
    ];
    for (key, doctrine) in DOCTRINE_KEYS.iter().zip(DOCTRINE_DISPLAY_ORDER) {
        if keyboard.just_pressed(*key) {
            return Some(CombatUiAction::SelectDoctrine(doctrine));
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) {
        Some(CombatUiAction::Validate)
    } else if keyboard.just_pressed(KeyCode::KeyR) {
        Some(CombatUiAction::Retreat)
    } else if keyboard.just_pressed(KeyCode::KeyA) {
        Some(CombatUiAction::AutoResolve)
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        Some(CombatUiAction::PreviousCombat)
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        Some(CombatUiAction::NextCombat)
    } else {
        None
    }
}

fn handle_combat_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut simulation: ResMut<SimulationResource>,
    mut ui: ResMut<CombatUiState>,
    draft: Res<CombatPlanDraftState>,
    time: Res<Time>,
) {
    if ui.current.is_none() {
        return;
    }
    if let Some(action) = combat_shortcut(&keyboard, ui.phase) {
        apply_combat_action(action, &mut simulation, &mut ui, &draft, time.elapsed());
    }
}

type DoctrineCardStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CombatActionButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
>;

type DoctrineExtraTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static DoctrineExtraText, &'static mut Text),
    (
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatCommandText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
    ),
>;

fn update_doctrine_cards(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    mut cards: DoctrineCardStyleQuery,
    mut extra_texts: DoctrineExtraTextQuery,
) {
    let mission_id = ui.current;
    let pending = mission_id.and_then(|id| simulation.simulation().state().pending_combat(id));
    for (button, interaction, mut background, mut outline) in &mut cards {
        let CombatUiAction::SelectDoctrine(doctrine) = button.0 else {
            continue;
        };
        let available = ui.phase == CombatUiPhase::AwaitingDoctrine
            && pending.is_some_and(|pending| {
                round_command_cost(pending, Some(doctrine), ui.selected_intervention)
                    <= pending.command_points_remaining()
            });
        let selected = ui
            .chosen_doctrine
            .or_else(|| pending.map(persistent_doctrine))
            == Some(doctrine);
        background.0 = action_button_color(available, selected, interaction);
        outline.color = action_button_outline(available, selected, interaction);
    }

    let enemy_view = pending.map(enemy_intel);

    for (marker, mut text) in &mut extra_texts {
        let Some(id) = mission_id else {
            text.0 = String::new();
            continue;
        };
        let Some(pending) = simulation.simulation().state().pending_combat(id) else {
            text.0 = String::new();
            continue;
        };
        let mut lines = Vec::new();
        if let Some(preview) = repetition_penalty_preview(pending, marker.0, combat_rules()) {
            lines.push(format!(
                "Doctrine répétée : dégâts sortants ×{:.2} ce round.",
                preview.outgoing_damage_multiplier_per_mille as f32 / 1000.0
            ));
        }
        if let Some(hint) = enemy_view
            .as_ref()
            .and_then(|enemy| known_threat_line(marker.0, enemy))
        {
            lines.push(hint.to_string());
        }
        text.0 = lines.join("\n");
    }
}

type CombatCommandTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatCommandText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
    ),
>;

type CombatValidateButtonLabelQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatValidateButtonLabel>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatCommandText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
    ),
>;

fn command_points_meter(available: u8, maximum: u8) -> String {
    let mut meter = String::new();
    for index in 0..maximum {
        if index > 0 {
            meter.push(' ');
        }
        meter.push(if index < available { '●' } else { '○' });
    }
    meter
}

fn target_priority_order_label(priority: CombatTargetPriority) -> &'static str {
    match priority {
        CombatTargetPriority::Any => "cible libre",
        CombatTargetPriority::Light => "unités légères",
        CombatTargetPriority::Medium => "unités moyennes",
        CombatTargetPriority::Heavy => "unités lourdes",
        CombatTargetPriority::Damaged => "unités endommagées",
        CombatTargetPriority::Support => "unités de soutien",
    }
}

fn intervention_order_label(intervention: CombatIntervention) -> String {
    match intervention {
        CombatIntervention::FocusFire { priority } => {
            format!(
                "tir concentré sur {}",
                target_priority_order_label(priority)
            )
        }
        CombatIntervention::CommitReserve { group_id } => {
            format!("engager {}", reserve_button_label(group_id).to_lowercase())
        }
    }
}

fn prepared_order_label(
    pending: &PendingCombat,
    doctrine: Option<CombatDoctrineId>,
    intervention: Option<CombatIntervention>,
) -> String {
    let mut lines = Vec::new();
    if let Some(doctrine) = doctrine
        && doctrine != persistent_doctrine(pending)
    {
        lines.push(format!(
            "doctrine {}",
            doctrine_name(doctrine).to_lowercase()
        ));
    }
    if let Some(intervention) = intervention {
        lines.push(intervention_order_label(intervention));
    }
    if lines.is_empty() {
        "continuer le plan actuel".to_string()
    } else {
        lines.join(" + ")
    }
}

fn update_command_panel(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    draft: Res<CombatPlanDraftState>,
    mut command_texts: CombatCommandTextQuery,
    mut validate_labels: CombatValidateButtonLabelQuery,
) {
    let pending = ui
        .current
        .and_then(|mission_id| simulation.simulation().state().pending_combat(mission_id));
    let Some(pending) = pending else {
        if let Ok(mut text) = command_texts.single_mut() {
            text.0.clear();
        }
        return;
    };

    let cost = round_command_cost(pending, ui.chosen_doctrine, ui.selected_intervention);
    let initial_plan_dirty = pending.round() == 0 && draft.is_dirty();
    if let Ok(mut text) = command_texts.single_mut() {
        let maximum = pending.command_points_maximum();
        let available = pending.command_points_remaining();
        let meter = command_points_meter(available, maximum);
        if pending.round() == 0 {
            text.0 = format!(
                "COMMANDEMENT : {meter}  {available}/{maximum} PC\nDisponible après le premier engagement."
            );
        } else {
            let prepared =
                prepared_order_label(pending, ui.chosen_doctrine, ui.selected_intervention);
            let cost_label = if cost == 0 {
                "gratuit".to_string()
            } else {
                format!("-{cost} PC")
            };
            text.0 = format!(
                "COMMANDEMENT : {meter}  {available}/{maximum} PC\nOrdre : {prepared} ({cost_label})"
            );
        }
    }

    if let Ok(mut text) = validate_labels.single_mut() {
        text.0 = if initial_plan_dirty {
            "Confirmez le plan avant l'assaut".to_string()
        } else if cost > 0 {
            "Exécuter l'ordre [Entrée]".to_string()
        } else if pending.round() == 0 {
            "Lancer l'assaut [Entrée]".to_string()
        } else {
            "Continuer le plan [Entrée]".to_string()
        };
    }
}

type CombatActionButtonStyleQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CombatActionButton,
        &'static Interaction,
        &'static mut BackgroundColor,
        &'static mut Outline,
    ),
>;

type RetreatButtonLabelQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<RetreatButtonLabel>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<DoctrineExtraText>,
        Without<CombatCommandText>,
        Without<CombatValidateButtonLabel>,
    ),
>;

fn update_action_buttons(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    draft: Res<CombatPlanDraftState>,
    time: Res<Time>,
    mut buttons: CombatActionButtonStyleQuery,
    mut retreat_label: RetreatButtonLabelQuery,
) {
    let retreat_armed = ui
        .retreat_armed_until
        .is_some_and(|deadline| time.elapsed() <= deadline);
    let pending = ui
        .current
        .and_then(|mission_id| simulation.simulation().state().pending_combat(mission_id));
    let round_command_available = pending.is_some_and(|pending| {
        let intervention_available = match ui.selected_intervention {
            None => true,
            Some(intervention) => {
                intervention_is_available(pending, ui.chosen_doctrine, intervention)
            }
        };
        intervention_available
            && !(pending.round() == 0 && draft.is_dirty())
            && round_command_cost(pending, ui.chosen_doctrine, ui.selected_intervention)
                <= pending.command_points_remaining()
    });

    for (button, interaction, mut background, mut outline) in &mut buttons {
        let (available, active) = match button.0 {
            CombatUiAction::SelectDoctrine(_) => continue,
            CombatUiAction::SelectFocusPriority(_) | CombatUiAction::CommitReserve(_) => {
                let intervention = action_intervention(button.0)
                    .expect("intervention actions always map to an intervention");
                (
                    ui.phase == CombatUiPhase::AwaitingDoctrine
                        && pending.is_some_and(|pending| {
                            intervention_is_available(pending, ui.chosen_doctrine, intervention)
                        }),
                    ui.selected_intervention == Some(intervention),
                )
            }
            CombatUiAction::Validate => (
                ui.phase == CombatUiPhase::AwaitingDoctrine && round_command_available,
                true,
            ),
            CombatUiAction::Retreat => (ui.phase == CombatUiPhase::AwaitingDoctrine, retreat_armed),
            CombatUiAction::AutoResolve => (ui.phase == CombatUiPhase::AwaitingDoctrine, false),
            CombatUiAction::ReturnToGalaxy => (ui.phase == CombatUiPhase::FinalReport, true),
            CombatUiAction::ToggleDetailedReport => (
                ui.phase == CombatUiPhase::FinalReport,
                ui.showing_detailed_report,
            ),
            CombatUiAction::PreviousCombat | CombatUiAction::NextCombat => (
                ui.phase == CombatUiPhase::AwaitingDoctrine && ui.queue.len() > 1,
                false,
            ),
            CombatUiAction::StartPlanning => (ui.phase == CombatUiPhase::Briefing, true),
            CombatUiAction::ToggleAdvancedDoctrines => (
                ui.phase == CombatUiPhase::AwaitingDoctrine,
                ui.advanced_doctrines_expanded,
            ),
            CombatUiAction::SelectReportTab(tab) => (
                ui.phase == CombatUiPhase::FinalReport && ui.showing_detailed_report,
                ui.report_tab == tab,
            ),
        };
        background.0 = action_button_color(available, active, interaction);
        outline.color = action_button_outline(available, active, interaction);
    }

    if let Ok(mut text) = retreat_label.single_mut() {
        text.0 = if retreat_armed {
            "Confirmer la retraite ? [R]".to_string()
        } else {
            "Retraite [R]".to_string()
        };
    }
}

type CombatRoundLogTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatCommandText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
        Without<CombatResultSummaryText>,
    ),
>;

type CombatResultSummaryTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatResultSummaryText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
    ),
>;

type CombatFeedbackTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatFeedbackText>,
        Without<CombatRoundLogText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatCommandText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
    ),
>;

fn update_combat_feedback(
    ui: Res<CombatUiState>,
    mut round_log: CombatRoundLogTextQuery,
    mut feedback: CombatFeedbackTextQuery,
) {
    if ui.phase != CombatUiPhase::FinalReport
        && let Ok(mut text) = round_log.single_mut()
    {
        text.0 = ui
            .last_round_summary
            .clone()
            .unwrap_or_else(|| "Zone orbitale".to_string());
    }
    if let Ok(mut text) = feedback.single_mut() {
        text.0 = ui.feedback.clone();
    }
}

/// COMBAT-UX-001-H: the detailed report picks its content from
/// `ui.report_tab`, each tab reusing an `inspector_panel.rs` function that
/// also backs `combat_report_text` (the un-tabbed full dump `fleet_ui.rs`'s
/// mission-reports viewer still renders) — no duplicated formatting logic.
fn update_final_report(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    mut header_texts: CombatHeaderTextQuery,
    mut round_log: CombatRoundLogTextQuery,
    mut result_summary: CombatResultSummaryTextQuery,
) {
    if ui.phase != CombatUiPhase::FinalReport {
        return;
    }
    let Some(mission_id) = ui.current else {
        return;
    };
    let Some(report) = simulation.simulation().state().combat_report(mission_id) else {
        return;
    };
    let target_label = mission_target_label(
        simulation.simulation(),
        MissionTarget::Planet {
            system_id: report.planet_id.system_id(),
            planet_id: report.planet_id,
        },
    );
    if let Ok(mut text) = header_texts.single_mut() {
        text.0 = format!(
            "ASSAUT ORBITAL — {target_label}      COMBAT TERMINÉ      File : {}",
            ui.queue.len(),
        );
    }
    if let Ok(mut text) = result_summary.single_mut() {
        text.0 = combat_result_summary_text(report, &target_label);
    }
    if let Ok(mut text) = round_log.single_mut() {
        text.0 = if ui.showing_detailed_report {
            match ui.report_tab {
                CombatReportTab::Summary => combat_report_summary_text(report),
                CombatReportTab::Timeline => combat_report_timeline_text(report),
                CombatReportTab::Statistics => combat_report_statistics_text(report),
                CombatReportTab::Units => combat_report_units_text(report),
            }
        } else {
            String::new()
        };
    }
}

/// COMBAT-UX-001 §12/§32 priority 2: the concise victory/defeat/retreat
/// summary shown by default in `CombatUiPhase::FinalReport`, ahead of the
/// pre-existing technical `combat_report_text` dump (now gated behind the
/// "Rapport détaillé" toggle). Every figure comes straight from
/// `CombatResolution` — nothing here is invented (doc §25).
fn combat_result_summary_text(report: &CombatReport, target_label: &str) -> String {
    let CombatReportStatus::Resolved(resolution) = &report.status else {
        return format!("{target_label}\nCombat annulé : la cible n'était plus valide.");
    };
    let banner = combat_outcome_label(resolution.outcome).to_uppercase();
    let attacker_engaged: u64 = report
        .attacker
        .ships
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let attacker_survivors: u64 = resolution
        .attacker_survivors
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let attacker_losses: u64 = resolution
        .attacker_losses
        .iter()
        .map(|loss| loss.quantity)
        .sum();
    let defender_engaged: u32 = report
        .defender
        .forces
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let defender_survivors: u32 = resolution
        .defender_survivors
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let defender_losses: u32 = resolution
        .defender_losses
        .iter()
        .map(|loss| loss.quantity)
        .sum();
    let salvage = resolution.salvage_recovered;
    let loot = if salvage.is_zero() {
        "Aucun butin récupéré.".to_string()
    } else {
        format!(
            "Butin récupéré : +{} métal, +{} cristal, +{} carburant.",
            salvage.metal, salvage.crystal, salvage.fuel
        )
    };
    let moment = combat_decisive_moment_line(&report.round_history);

    format!(
        "{banner}\n{target_label}\n\n\
         Votre flotte : {attacker_engaged} engagés, {attacker_survivors} survivants, {attacker_losses} perdus.\n\
         Ennemi : {defender_engaged} détectés, {defender_survivors} restants, {defender_losses} neutralisés.\n\n\
         {loot}{moment}"
    )
}

/// V1 heuristic (doc §12.5): the round with the most combined damage, or the
/// first round with a notable destruction if one stands out earlier.
fn combat_decisive_moment_line(round_history: &[CombatRoundRecord]) -> String {
    let first_destruction = round_history.iter().find(|record| {
        record
            .notable_events
            .iter()
            .any(|event| matches!(event, CombatRoundEvent::StackDestroyed { .. }))
    });
    let heaviest = round_history
        .iter()
        .max_by_key(|record| record.attacker_damage + record.defender_damage);
    match first_destruction.or(heaviest) {
        Some(record) => format!("\n\nMOMENT DÉCISIF — Round {}.", record.round),
        None => String::new(),
    }
}

type CombatPreReportControlsQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (With<CombatPreReportControls>, Without<CombatReturnRow>),
>;

type CombatReturnRowQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (With<CombatReturnRow>, Without<CombatPreReportControls>),
>;

fn update_final_report_visibility(
    ui: Res<CombatUiState>,
    mut controls: CombatPreReportControlsQuery,
    mut return_row: CombatReturnRowQuery,
) {
    let in_final_report = ui.phase == CombatUiPhase::FinalReport;
    let controls_visibility = if in_final_report {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut controls {
        if *visibility != controls_visibility {
            *visibility = controls_visibility;
        }
    }
    let return_visibility = if in_final_report {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut return_row {
        if *visibility != return_visibility {
            *visibility = return_visibility;
        }
    }
}

fn update_combat_planning_controls_visibility(
    ui: Res<CombatUiState>,
    mut controls: Query<&mut Visibility, With<CombatPlanningControls>>,
) {
    let visible = ui.phase == CombatUiPhase::AwaitingDoctrine;
    let next = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut controls {
        if *visibility != next {
            *visibility = next;
        }
    }
}

type AdvancedDoctrinesToggleLabelQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<AdvancedDoctrinesToggleLabel>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
    ),
>;

fn update_advanced_doctrines_visibility(
    ui: Res<CombatUiState>,
    mut row: Query<&mut Visibility, With<AdvancedDoctrinesRow>>,
    mut toggle_label: AdvancedDoctrinesToggleLabelQuery,
) {
    let visible = ui.phase == CombatUiPhase::AwaitingDoctrine && ui.advanced_doctrines_expanded;
    let next = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut row {
        if *visibility != next {
            *visibility = next;
        }
    }
    if let Ok(mut text) = toggle_label.single_mut() {
        text.0 = if ui.advanced_doctrines_expanded {
            "Tactiques avancées ▲".to_string()
        } else {
            "Tactiques avancées ▼".to_string()
        };
    }
}

fn update_report_tabs_visibility(
    ui: Res<CombatUiState>,
    mut bar: Query<&mut Visibility, With<ReportTabBar>>,
) {
    let next = if ui.showing_detailed_report {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut bar {
        if *visibility != next {
            *visibility = next;
        }
    }
}

type CombatBriefingControlsQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CombatBriefingControls>,
        Without<CombatHiddenDuringBriefing>,
    ),
>;

type CombatHiddenDuringBriefingQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CombatHiddenDuringBriefing>,
        Without<CombatBriefingControls>,
    ),
>;

fn update_combat_briefing_visibility(
    ui: Res<CombatUiState>,
    mut briefing: CombatBriefingControlsQuery,
    mut hidden_during_briefing: CombatHiddenDuringBriefingQuery,
) {
    let in_briefing = ui.phase == CombatUiPhase::Briefing;
    let briefing_visibility = if in_briefing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut briefing {
        if *visibility != briefing_visibility {
            *visibility = briefing_visibility;
        }
    }
    let rest_visibility = if in_briefing {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut hidden_during_briefing {
        if *visibility != rest_visibility {
            *visibility = rest_visibility;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bevy::ecs::system::RunSystemOnce;
    use galactic_domain::{FactionId, MissionId};
    use galactic_sim::{
        CombatGroupPlanId, CombatGroupRole, CombatSide, CombatStackExchange, CombatStackLoss,
        CombatStackView,
    };
    use group_panel::{DraftAction, DraftActionButton};

    use super::*;

    /// COMBAT-001-E "plusieurs combats simultanés" (UI level) — the engine
    /// side is already covered by `combat::session::tests::
    /// multiple_pending_combats_coexist_independently`; this exercises the
    /// screen's own Previous/Next queue-cycling logic with more than one
    /// combat queued, including wraparound in both directions.
    #[test]
    fn cycle_combat_queue_wraps_in_both_directions_across_several_combats() {
        let first = MissionId::new(1);
        let second = MissionId::new(2);
        let third = MissionId::new(3);
        let mut ui = CombatUiState {
            queue: vec![first, second, third],
            current: Some(first),
            ..Default::default()
        };

        cycle_combat_queue(&mut ui, false);
        assert_eq!(ui.current, Some(second));
        cycle_combat_queue(&mut ui, false);
        assert_eq!(ui.current, Some(third));
        // Forward from the last entry wraps back to the first.
        cycle_combat_queue(&mut ui, false);
        assert_eq!(ui.current, Some(first));

        // Reverse from the first entry wraps back to the last.
        cycle_combat_queue(&mut ui, true);
        assert_eq!(ui.current, Some(third));
        cycle_combat_queue(&mut ui, true);
        assert_eq!(ui.current, Some(second));
    }

    #[test]
    fn cycle_combat_queue_resets_per_combat_state_on_switch() {
        let first = MissionId::new(1);
        let second = MissionId::new(2);
        let mut ui = CombatUiState {
            queue: vec![first, second],
            current: Some(first),
            chosen_doctrine: Some(CombatDoctrineId::ConcentratedAssault),
            selected_intervention: Some(CombatIntervention::FocusFire {
                priority: CombatTargetPriority::Heavy,
            }),
            retreat_armed_until: Some(Duration::from_secs(1)),
            feedback: "une erreur précédente".to_string(),
            phase: CombatUiPhase::RoundPause,
            // Both already briefed — this test is about the per-combat
            // state reset, not about `Briefing` (see the dedicated
            // `cycle_combat_queue`/briefing tests for that).
            briefed: HashSet::from([first, second]),
            ..Default::default()
        };

        cycle_combat_queue(&mut ui, false);

        assert_eq!(ui.current, Some(second));
        assert_eq!(ui.phase, CombatUiPhase::AwaitingDoctrine);
        assert_eq!(ui.chosen_doctrine, None);
        assert_eq!(ui.selected_intervention, None);
        assert_eq!(ui.retreat_armed_until, None);
        assert!(ui.feedback.is_empty());
    }

    #[test]
    fn cycle_combat_queue_is_a_no_op_with_fewer_than_two_combats() {
        let only = MissionId::new(1);
        let mut ui = CombatUiState {
            queue: vec![only],
            current: Some(only),
            ..Default::default()
        };

        cycle_combat_queue(&mut ui, false);

        assert_eq!(ui.current, Some(only));
    }

    #[test]
    fn qualitative_prediction_label_translates_every_variant_to_french() {
        assert_eq!(
            qualitative_prediction_label(QualitativePrediction::VeryUnfavorable),
            "très défavorable"
        );
        assert_eq!(
            qualitative_prediction_label(QualitativePrediction::Unfavorable),
            "défavorable"
        );
        assert_eq!(
            qualitative_prediction_label(QualitativePrediction::Uncertain),
            "incertaine"
        );
        assert_eq!(
            qualitative_prediction_label(QualitativePrediction::Favorable),
            "favorable"
        );
        assert_eq!(
            qualitative_prediction_label(QualitativePrediction::VeryFavorable),
            "très favorable"
        );
    }

    fn resolved_combat_report_fixture() -> CombatReport {
        CombatReport {
            mission_id: MissionId::new(7),
            planet_id: galactic_domain::PlanetId::new(3),
            resolved_at: galactic_sim::StrategicTick::new(42),
            rules_version: 2,
            seed: 11,
            attacker: galactic_sim::CombatFleetSnapshot {
                fleet_id: galactic_domain::FleetId::new(2),
                owner: galactic_domain::Owner::Faction(FactionId::new(0)),
                ships: vec![galactic_sim::CombatShipStack {
                    craftable: galactic_sim::CraftableId::FRIGATE_BULWARK,
                    quantity: 2,
                    offense: 70,
                    defense: 45,
                    durability: 60,
                    target_class: CombatTargetClass::Medium,
                    bonuses: galactic_sim::CombatTargetBonuses::default(),
                }],
                cargo: galactic_domain::ResourceStock::ZERO,
                cargo_capacity: 100,
            },
            defender: galactic_sim::PlanetDefenseSnapshot {
                planet_id: galactic_domain::PlanetId::new(3),
                occupant: galactic_domain::Owner::Faction(FactionId::new(1)),
                population: 120,
                forces: vec![galactic_sim::PlanetaryForceStack {
                    definition_id: galactic_sim::PlanetaryForceId::from_static("confins_militia"),
                    quantity: 6,
                }],
                revision: 1,
            },
            round_history: vec![
                CombatRoundRecord {
                    round: 0,
                    attacker_doctrine: galactic_sim::CombatDoctrineId::BalancedEngagement,
                    defender_doctrine: galactic_sim::CombatDoctrineId::BalancedEngagement,
                    attacker_damage: 10,
                    defender_damage: 20,
                    attacker_exchanges: Vec::new(),
                    defender_exchanges: Vec::new(),
                    attacker_losses: Vec::new(),
                    defender_losses: Vec::new(),
                    notable_events: Vec::new(),
                    intel_after: 20,
                },
                CombatRoundRecord {
                    round: 1,
                    attacker_doctrine: galactic_sim::CombatDoctrineId::BalancedEngagement,
                    defender_doctrine: galactic_sim::CombatDoctrineId::BalancedEngagement,
                    attacker_damage: 30,
                    defender_damage: 90,
                    attacker_exchanges: Vec::new(),
                    defender_exchanges: Vec::new(),
                    attacker_losses: Vec::new(),
                    defender_losses: Vec::new(),
                    // `CombatStackId` has no public constructor outside
                    // `galactic_sim`, so this fixture can't build a real
                    // `StackDestroyed` event — round 1 is still picked as
                    // the decisive moment via the damage-heuristic fallback
                    // (it's the heaviest round either way).
                    notable_events: Vec::new(),
                    intel_after: 25,
                },
            ],
            initial_plan: None,
            final_plan: None,
            intervention_history: Vec::new(),
            status: CombatReportStatus::Resolved(galactic_sim::CombatResolution {
                outcome: galactic_sim::CombatOutcome::AttackerVictory,
                rounds: 3,
                attacker_losses: vec![galactic_sim::CombatShipLoss {
                    craftable: galactic_sim::CraftableId::FRIGATE_BULWARK,
                    quantity: 1,
                }],
                attacker_survivors: vec![galactic_sim::CombatShipStack {
                    craftable: galactic_sim::CraftableId::FRIGATE_BULWARK,
                    quantity: 1,
                    offense: 70,
                    defense: 45,
                    durability: 60,
                    target_class: CombatTargetClass::Medium,
                    bonuses: galactic_sim::CombatTargetBonuses::default(),
                }],
                defender_losses: vec![galactic_sim::PlanetaryForceLoss {
                    definition_id: galactic_sim::PlanetaryForceId::from_static("confins_militia"),
                    quantity: 4,
                }],
                defender_survivors: vec![galactic_sim::PlanetaryForceStack {
                    definition_id: galactic_sim::PlanetaryForceId::from_static("confins_militia"),
                    quantity: 2,
                }],
                attacker_damage: 40,
                defender_damage: 110,
                salvage_recoverable: galactic_domain::ResourceStock::new(10, 5, 0),
                salvage_recovered: galactic_domain::ResourceStock::new(7, 3, 0),
                control: galactic_sim::CombatControlChange::Unchanged,
            }),
        }
    }

    #[test]
    fn combat_result_summary_text_reports_real_counts_loot_and_decisive_round() {
        let report = resolved_combat_report_fixture();

        let summary = combat_result_summary_text(&report, "Hélianthe d");

        assert!(summary.contains("VICTOIRE ATTAQUANTE"));
        assert!(summary.contains("Hélianthe d"));
        assert!(summary.contains("2 engagés, 1 survivants, 1 perdus"));
        assert!(summary.contains("6 détectés, 2 restants, 4 neutralisés"));
        assert!(summary.contains("+7 métal, +3 cristal, +0 carburant"));
        // The destruction happens in round 1, which is also the heavier
        // round — both heuristics agree, so it must be picked.
        assert!(summary.contains("MOMENT DÉCISIF — Round 1."));
    }

    #[test]
    fn combat_result_summary_text_reports_no_loot_when_nothing_was_recovered() {
        let mut report = resolved_combat_report_fixture();
        let CombatReportStatus::Resolved(resolution) = &mut report.status else {
            unreachable!()
        };
        resolution.salvage_recovered = galactic_domain::ResourceStock::ZERO;

        let summary = combat_result_summary_text(&report, "Hélianthe d");

        assert!(summary.contains("Aucun butin récupéré."));
    }

    #[test]
    fn combat_command_error_text_translates_every_variant_to_french() {
        assert_eq!(
            combat_command_error_text(CombatCommandError::UnknownCombat(MissionId::new(0))),
            "Ce combat n'est plus en attente (déjà résolu ou retiré)."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::Access(AuthorizationError::NotOwner {
                actor: FactionId::new(0),
                owner: FactionId::new(1),
            })),
            "Ce combat n'appartient pas à votre faction."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::Access(
                AuthorizationError::InactiveActor(FactionId::new(0))
            )),
            "Action refusée : faction invalide ou inactive."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::StaleRound {
                expected: 2,
                found: 1
            }),
            "Ce round (1) a déjà été résolu — round attendu : 2."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::PlanLocked { round: 1 }),
            "Le plan est verrouillé depuis le round 1."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::InvalidPlan(
                CombatPlanValidationError::EmptyPlan
            )),
            "Le plan de bataille est vide."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::InsufficientCommandPoints {
                required: 2,
                available: 1,
            }),
            "Points de commandement insuffisants : 2 requis, 1 disponible(s)."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::InvalidIntervention(
                CombatInterventionError::CombatNotStarted
            )),
            "Les ordres de commandement seront disponibles après le premier engagement."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::InvalidIntervention(
                CombatInterventionError::ReserveGroupInoperable(CombatGroupPlanId::Gamma)
            )),
            "Cette réserve n'a plus d'unité opérationnelle."
        );
        assert_eq!(
            combat_command_error_text(CombatCommandError::InvalidIntervention(
                CombatInterventionError::GroupIsNotReserve(CombatGroupPlanId::Alpha)
            )),
            "Ce groupe est déjà engagé."
        );
    }

    #[test]
    fn command_cost_helpers_match_the_current_pending_plan() {
        let simulation = simulation_with_pending_combat();
        let pending = &simulation.state().pending_combats[0];

        assert_eq!(round_command_cost(pending, None, None), 0);
        assert_eq!(
            round_command_cost(pending, Some(CombatDoctrineId::ConcentratedAssault), None),
            0
        );
        assert_eq!(
            round_command_cost(
                pending,
                None,
                Some(CombatIntervention::FocusFire {
                    priority: CombatTargetPriority::Heavy,
                })
            ),
            combat_rules().command().focus_fire_cost()
        );
        assert!(!intervention_is_available(
            pending,
            None,
            CombatIntervention::FocusFire {
                priority: CombatTargetPriority::Heavy,
            }
        ));
    }

    #[test]
    fn space_skips_round_pause_animation() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Space);
        world.insert_resource(keyboard);
        world.insert_resource(CombatUiState {
            phase: CombatUiPhase::RoundPause,
            ..Default::default()
        });

        world
            .run_system_once(tick_round_pause)
            .expect("tick_round_pause runs");

        assert_eq!(
            world.resource::<CombatUiState>().phase,
            CombatUiPhase::AwaitingDoctrine
        );
    }

    /// COMBAT-UX-001-B: a combat becoming current for the first time (the
    /// primary `CombatDecisionRequired` path, not a reload) must open on the
    /// briefing, not straight into planning.
    #[test]
    fn a_new_combat_decision_opens_on_the_briefing() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let planet_id = simulation.state().pending_combats[0].planet_id;
        let actor = simulation.state().player_faction;

        let mut world = World::new();
        world.insert_resource(SimulationResource {
            simulation,
            pending_events: vec![galactic_sim::GameEvent::new(
                actor,
                galactic_sim::StrategicTick::new(0),
                GameEventKind::CombatDecisionRequired(CombatDecisionRequired {
                    mission_id,
                    planet_id,
                    round: 0,
                }),
            )],
        });
        world.init_resource::<CombatUiState>();

        world
            .run_system_once(capture_combat_events)
            .expect("capture_combat_events runs");

        let ui = world.resource::<CombatUiState>();
        assert_eq!(ui.current, Some(mission_id));
        assert_eq!(ui.phase, CombatUiPhase::Briefing);
        assert!(ui.briefed.contains(&mission_id));
    }

    /// A combat already briefed must not show the briefing again when the
    /// player cycles back to it via Previous/Next.
    #[test]
    fn cycling_back_to_an_already_briefed_combat_skips_the_briefing() {
        let first = MissionId::new(1);
        let second = MissionId::new(2);
        let mut ui = CombatUiState {
            queue: vec![first, second],
            current: Some(first),
            phase: CombatUiPhase::Briefing,
            briefed: HashSet::from([first]),
            ..Default::default()
        };

        // First visit to `second` — not yet briefed.
        cycle_combat_queue(&mut ui, false);
        assert_eq!(ui.current, Some(second));
        assert_eq!(ui.phase, CombatUiPhase::Briefing);
        assert!(ui.briefed.contains(&second));

        // Back to `first`, already briefed from the initial state above.
        cycle_combat_queue(&mut ui, true);
        assert_eq!(ui.current, Some(first));
        assert_eq!(ui.phase, CombatUiPhase::AwaitingDoctrine);

        // And `second` again — briefed on the first visit, must not repeat.
        cycle_combat_queue(&mut ui, false);
        assert_eq!(ui.current, Some(second));
        assert_eq!(ui.phase, CombatUiPhase::AwaitingDoctrine);
    }

    #[test]
    fn start_planning_advances_from_briefing_to_awaiting_doctrine() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let mut simulation = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::Briefing,
            briefed: HashSet::from([mission_id]),
            ..Default::default()
        };
        let draft_state = group_panel::CombatPlanDraftState::default();

        apply_combat_action(
            CombatUiAction::StartPlanning,
            &mut simulation,
            &mut ui,
            &draft_state,
            Duration::ZERO,
        );

        assert_eq!(ui.phase, CombatUiPhase::AwaitingDoctrine);
    }

    /// Ported from the deleted `presentation::combat_autopilot`'s own test
    /// fixture (COMBAT-001-D's final step) — the same end-to-end scenario
    /// (build a colony, queue frigates, launch and arrive at a hostile
    /// outpost) now exercises the real screen's reload-resume path instead
    /// of the temporary auto-pilot it used to prove.
    fn simulation_with_pending_combat() -> galactic_sim::Simulation {
        simulation_with_pending_combat_using(3)
    }

    fn simulation_with_pending_combat_using(ship_count: u64) -> galactic_sim::Simulation {
        use galactic_domain::{Owner, UniverseConfig};
        use galactic_sim::{
            BuildingKind, CraftableId, KnowledgeLevel, ResearchState, Simulation, TechnologyId,
            default_building_catalog,
        };

        let mut simulation = Simulation::new(UniverseConfig::mvp());
        let actor = simulation.state().player_faction;
        let colony_id = simulation.state().colonies[0].id;
        let origin = simulation.state().colonies[0].system_id;
        let neighboring_systems = simulation
            .universe_repository()
            .neighboring_systems(origin)
            .to_vec();
        let target = simulation
            .state()
            .planetary_presences
            .iter()
            .find(|presence| {
                neighboring_systems.contains(&presence.planet_id.system_id())
                    && presence.occupant != Owner::Faction(actor)
                    && presence.occupant != Owner::Unowned
                    && !presence.forces.is_empty()
            })
            .expect("the home neighborhood guarantees a hostile outpost")
            .planet_id;
        let repository = simulation.universe_repository().clone();
        simulation.state_mut().advance_system_knowledge(
            &repository,
            target.system_id(),
            KnowledgeLevel::Probed,
        );
        simulation.state_mut().advance_planet_knowledge(
            &repository,
            target,
            KnowledgeLevel::Analyzed,
        );
        let precision =
            galactic_sim::intelligence_precision_for_knowledge(KnowledgeLevel::Analyzed)
                .expect("Analyzed knowledge always maps to an intelligence precision");
        let current_tick = simulation.state().clock.current_tick();
        galactic_sim::refresh_planetary_intelligence(
            simulation.state_mut(),
            target,
            precision,
            current_tick,
        )
        .expect("the target planet has a validated planetary presence");
        {
            let colony = &mut simulation.state_mut().colonies[0];
            colony
                .buildings
                .set_level(BuildingKind::CONSTRUCTION_CENTER, 2);
            colony.buildings.set_level(BuildingKind::METAL_MINE, 2);
            colony
                .buildings
                .set_level(BuildingKind::CRYSTAL_EXTRACTOR, 2);
            colony.buildings.set_level(BuildingKind::SHIPYARD, 1);
            colony.energy = default_building_catalog().energy_grid_for_levels(colony.buildings);
            colony
                .resources
                .credit(galactic_domain::ResourceStock::new(1_000, 1_000, 1_000))
                .expect("the test resources fit the starting storage");
        }
        simulation.state_mut().research = ResearchState::from_completed([
            TechnologyId::SPATIAL_DETECTION,
            TechnologyId::PROPULSION,
            TechnologyId::PLANETARY_ANALYSIS,
        ]);
        for _ in 0..ship_count {
            simulation.apply_player_action(GameAction::QueueCraft {
                colony_id,
                craftable: CraftableId::FRIGATE_BULWARK,
                quantity: 1,
            });
        }
        simulation.advance(std::time::Duration::from_secs(200));
        simulation.apply_player_action(GameAction::LaunchAttack {
            colony_id,
            target: MissionTarget::Planet {
                system_id: target.system_id(),
                planet_id: target,
            },
        });
        simulation.advance(std::time::Duration::from_secs(120));
        assert_eq!(simulation.state().pending_combats.len(), 1);
        simulation
    }

    #[test]
    fn combat_plan_draft_covers_each_live_allied_stack_once_and_can_reassign() {
        let simulation = simulation_with_pending_combat();
        let pending = &simulation.state().pending_combats[0];
        let live_stack_ids = allied_stacks(pending)
            .iter()
            .filter(|stack| stack.surviving_quantity > 0)
            .map(|stack| stack.stack_id)
            .collect::<BTreeSet<_>>();

        let mut draft = group_panel::CombatPlanDraft::from_pending(pending);
        let plan = draft.to_plan();
        let planned_stack_ids = plan
            .groups
            .iter()
            .flat_map(|group| group.stacks.iter().copied())
            .collect::<Vec<_>>();

        assert_eq!(planned_stack_ids.len(), live_stack_ids.len());
        assert_eq!(
            planned_stack_ids.iter().copied().collect::<BTreeSet<_>>(),
            live_stack_ids
        );

        let reassigned = *live_stack_ids
            .iter()
            .next()
            .expect("the fixture launches at least one allied combat stack");
        draft.assign_stack(reassigned, CombatGroupPlanId::Beta);

        assert_eq!(
            draft.selected_group(reassigned),
            Some(CombatGroupPlanId::Beta)
        );
        let reassigned_plan = draft.to_plan();
        assert!(
            reassigned_plan
                .groups
                .iter()
                .any(|group| group.id == CombatGroupPlanId::Beta
                    && group.stacks.contains(&reassigned))
        );
        assert_eq!(
            reassigned_plan
                .groups
                .iter()
                .flat_map(|group| group.stacks.iter().copied())
                .collect::<BTreeSet<_>>(),
            live_stack_ids
        );
    }

    #[test]
    fn confirming_combat_plan_draft_updates_the_pending_combat_plan() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let mut simulation = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::AwaitingDoctrine,
            ..Default::default()
        };
        let mut draft_state = group_panel::CombatPlanDraftState::default();
        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("the combat fixture is still pending");
        draft_state.rebuild(mission_id, pending);
        let expected = draft_state
            .draft()
            .expect("rebuilding from a pending combat creates a draft")
            .to_plan();

        group_panel::confirm_current_draft(&mut simulation, &mut ui, &mut draft_state);

        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("confirming a plan does not resolve the combat");
        assert_eq!(pending.plan(), Some(&expected));
        assert!(!draft_state.is_dirty());
        assert!(simulation.pending_events.iter().any(|event| matches!(
            event.kind,
            GameEventKind::CombatPlanConfirmed(CombatPlanConfirmed { mission_id: id })
                if id == mission_id
        )));
    }

    #[test]
    fn dirty_initial_plan_blocks_launch() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let mut simulation = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::AwaitingDoctrine,
            ..Default::default()
        };
        let mut draft_state = group_panel::CombatPlanDraftState::default();
        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("the combat fixture is still pending");
        draft_state.rebuild(mission_id, pending);
        draft_state.mark_dirty_for_tests();

        apply_combat_action(
            CombatUiAction::Validate,
            &mut simulation,
            &mut ui,
            &draft_state,
            Duration::ZERO,
        );

        assert_eq!(ui.feedback, "Confirmez le plan avant l'assaut.");
        assert!(simulation.pending_events.is_empty());
        assert_eq!(
            simulation
                .simulation()
                .state()
                .pending_combat(mission_id)
                .expect("combat stays pending")
                .round(),
            0
        );
    }

    #[test]
    fn confirmed_initial_plan_allows_launch() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let mut simulation = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::AwaitingDoctrine,
            ..Default::default()
        };
        let mut draft_state = group_panel::CombatPlanDraftState::default();
        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("the combat fixture is still pending");
        draft_state.rebuild(mission_id, pending);

        apply_combat_action(
            CombatUiAction::Validate,
            &mut simulation,
            &mut ui,
            &draft_state,
            Duration::ZERO,
        );

        assert!(simulation.pending_events.iter().any(|event| matches!(
            event.kind,
            GameEventKind::CombatRoundResolved(CombatRoundResolved { mission_id: id, round: 1 })
                if id == mission_id
        )));
    }

    /// COMBAT-UX-001-F: the structured "ROUND TERMINÉ" summary — real
    /// plan/round data, not a hand-fabricated record, so the per-group
    /// integrity aggregation actually exercises `allied_stacks`/`pending.plan()`.
    #[test]
    fn combat_round_summary_reports_group_status_and_enemy_counts() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let mut simulation = SimulationResource {
            simulation,
            pending_events: Vec::new(),
        };
        let mut ui = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::AwaitingDoctrine,
            ..Default::default()
        };
        let mut draft_state = group_panel::CombatPlanDraftState::default();
        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("the combat fixture is still pending");
        draft_state.rebuild(mission_id, pending);

        apply_combat_action(
            CombatUiAction::Validate,
            &mut simulation,
            &mut ui,
            &draft_state,
            Duration::ZERO,
        );

        let pending = simulation
            .simulation()
            .state()
            .pending_combat(mission_id)
            .expect("combat stays pending after one round");
        let record = round_history(pending).last().expect("a round resolved");

        let summary = combat_round_summary(record, pending);
        assert!(summary.contains("ROUND 1 TERMINÉ"));
        assert!(summary.contains("intégrité"));
        assert!(summary.contains("ENNEMI —"));
        // The whole default fixture fleet defaults into Alpha (Assault) —
        // it must have engaged, not sat idle or shown as reserve.
        assert!(summary.contains(&format!(
            "{} — {}",
            group_panel::group_label(CombatGroupPlanId::Alpha),
            group_panel::role_label(CombatGroupRole::Assault)
        )));
    }

    /// A group with nothing assigned is not "in reserve with 0 stacks" —
    /// its `CommitReserve` button must be hidden entirely, not just greyed.
    #[test]
    fn reserve_button_hides_for_a_group_with_no_stacks_assigned() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                briefed: HashSet::from([mission_id]),
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, update_reserve_intervention_visibility);

        app.update();

        let world = app.world_mut();
        let mut buttons = world.query::<(&CombatActionButton, &Visibility)>();
        // The fixture's default draft has no confirmed plan yet (`pending.plan()`
        // is `None` before the first `Validate`), so every reserve button must
        // be hidden — nothing is available to be "in reserve" yet.
        let reserve_buttons: Vec<_> = buttons
            .iter(world)
            .filter(|(button, _)| matches!(button.0, CombatUiAction::CommitReserve(_)))
            .collect();
        assert_eq!(reserve_buttons.len(), 3);
        assert!(
            reserve_buttons
                .iter()
                .all(|(_, visibility)| **visibility == Visibility::Hidden)
        );
    }

    /// COMBAT-UX-001-C: clicking a group token directly on the battlefield
    /// map assigns the selected stack — the same `DraftAction::AssignSelected`
    /// / `handle_combat_plan_buttons` plumbing `group_panel.rs`'s "VOS FORCES"
    /// buttons already use, driven here through a real `Interaction::Pressed`
    /// on the actual spawned map token, not a direct state mutation.
    #[test]
    fn clicking_a_battlefield_group_token_assigns_the_selected_stack() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                briefed: HashSet::from([mission_id]),
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    group_panel::sync_combat_plan_draft,
                    group_panel::handle_combat_plan_buttons,
                )
                    .chain(),
            );

        // Let the draft populate from the pending combat first.
        app.update();

        let stack_id = {
            let world = app.world();
            let draft_state = world.resource::<group_panel::CombatPlanDraftState>();
            draft_state
                .draft()
                .expect("draft populated from the pending combat")
                .groups()
                .flat_map(|group| group.stacks.iter().copied())
                .next()
                .expect("the fixture launches at least one allied stack")
        };

        // Step 1: select that stack via its `SelectStack` button.
        press_draft_action(&mut app, DraftAction::SelectStack(0));
        assert_eq!(
            app.world()
                .resource::<group_panel::CombatPlanDraftState>()
                .selected_stack_for_tests(),
            Some(stack_id)
        );

        // Step 2: click the Beta token directly on the battlefield map.
        press_draft_action(
            &mut app,
            DraftAction::AssignSelected(CombatGroupPlanId::Beta),
        );

        let world = app.world();
        let draft_state = world.resource::<group_panel::CombatPlanDraftState>();
        assert_eq!(
            draft_state
                .draft()
                .and_then(|draft| draft.selected_group(stack_id)),
            Some(CombatGroupPlanId::Beta)
        );
    }

    /// Presses (and un-presses, so the next `press_draft_action` registers as
    /// a fresh `Changed<Interaction>`) the first `DraftActionButton` entity
    /// found carrying the given action — used to drive real button clicks in
    /// tests instead of calling the private `apply_draft_action` directly.
    fn press_draft_action(app: &mut bevy::app::App, action: DraftAction) {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &DraftActionButton)>();
        let entity = buttons
            .iter(world)
            .find(|(_, button)| button.0 == action)
            .map(|(entity, _)| entity)
            .unwrap_or_else(|| panic!("no DraftActionButton entity for {action:?}"));
        *world.get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        app.update();
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::None;
    }

    #[test]
    fn combat_plan_panel_updates_without_a_query_conflict() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    group_panel::sync_combat_plan_draft,
                    group_panel::update_combat_plan_panel,
                )
                    .chain(),
            );

        app.update();

        let world = app.world_mut();
        let draft = world.resource::<group_panel::CombatPlanDraftState>();
        assert!(draft.draft().is_some());

        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("Alpha"));
        assert!(rendered_text.contains("PARAMÈTRES SÉLECTIONNÉS"));
    }

    #[test]
    fn plan_buttons_are_disabled_after_round_zero() {
        let mut simulation = simulation_with_pending_combat_using(2);
        let mission_id = simulation.state().pending_combats[0].mission_id;
        simulation.apply_player_action(GameAction::ChooseCombatDoctrine {
            mission_id,
            round: 1,
            doctrine: None,
            intervention: None,
        });
        assert!(
            simulation.state().pending_combat(mission_id).is_some(),
            "the 2-frigate fixture should still be pending after one round"
        );

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    group_panel::sync_combat_plan_draft,
                    group_panel::update_combat_plan_panel,
                )
                    .chain(),
            );

        app.update();

        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("PARAMÈTRES — VERROUILLÉ"));

        let mut buttons = world.query::<(&group_panel::DraftActionButton, &BackgroundColor)>();
        let disabled = action_button_color(false, false, &Interaction::None);
        assert!(buttons.iter(world).any(|(button, background)| {
            button.0 == group_panel::DraftAction::Confirm && background.0 == disabled
        }));
        assert!(buttons.iter(world).any(|(button, background)| {
            matches!(button.0, group_panel::DraftAction::CycleRole(_)) && background.0 == disabled
        }));
    }

    #[test]
    fn draft_rebuilds_when_round_changes() {
        let simulation = simulation_with_pending_combat_using(2);
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    group_panel::sync_combat_plan_draft,
                    group_panel::update_combat_plan_panel,
                )
                    .chain(),
            );

        app.update();
        assert_eq!(
            app.world()
                .resource::<group_panel::CombatPlanDraftState>()
                .synced_round_for_tests(),
            Some(0)
        );
        {
            let mut simulation = app.world_mut().resource_mut::<SimulationResource>();
            simulation.pending_events =
                simulation
                    .simulation
                    .apply_player_action(GameAction::ChooseCombatDoctrine {
                        mission_id,
                        round: 1,
                        doctrine: None,
                        intervention: None,
                    });
        }
        app.update();

        assert_eq!(
            app.world()
                .resource::<group_panel::CombatPlanDraftState>()
                .synced_round_for_tests(),
            Some(1)
        );
    }

    #[test]
    fn round_pause_hides_planning_controls_and_awaiting_doctrine_restores_them() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::RoundPause,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                update_combat_planning_controls_visibility,
            );

        app.update();
        let world = app.world_mut();
        let mut hidden = world.query_filtered::<&Visibility, With<CombatPlanningControls>>();
        assert!(hidden.iter(world).count() > 0);
        assert!(
            hidden
                .iter(world)
                .all(|visibility| *visibility == Visibility::Hidden)
        );

        world.resource_mut::<CombatUiState>().phase = CombatUiPhase::AwaitingDoctrine;
        app.update();
        let world = app.world_mut();
        let mut visible = world.query_filtered::<&Visibility, With<CombatPlanningControls>>();
        assert!(
            visible
                .iter(world)
                .all(|visibility| *visibility == Visibility::Inherited)
        );
    }

    /// COMBAT-UX-001-D: the advanced-doctrine row starts collapsed, and
    /// toggling it flips both its `Visibility` and the toggle button's own
    /// label (▼/▲) — a real end-to-end run, not just a compile check.
    #[test]
    fn toggling_advanced_doctrines_shows_the_row_and_flips_the_label() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, update_advanced_doctrines_visibility);

        app.update();
        {
            let world = app.world_mut();
            let mut row = world.query_filtered::<&Visibility, With<AdvancedDoctrinesRow>>();
            assert_eq!(row.single(world).unwrap(), Visibility::Hidden);
            let mut label = world.query_filtered::<&Text, With<AdvancedDoctrinesToggleLabel>>();
            assert_eq!(label.single(world).unwrap().0, "Tactiques avancées ▼");
        }

        app.world_mut()
            .resource_mut::<CombatUiState>()
            .advanced_doctrines_expanded = true;
        app.update();
        let world = app.world_mut();
        let mut row = world.query_filtered::<&Visibility, With<AdvancedDoctrinesRow>>();
        assert_eq!(row.single(world).unwrap(), Visibility::Inherited);
        let mut label = world.query_filtered::<&Text, With<AdvancedDoctrinesToggleLabel>>();
        assert_eq!(label.single(world).unwrap().0, "Tactiques avancées ▲");
    }

    #[test]
    fn command_panel_updates_without_a_query_conflict() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, update_command_panel);

        app.update();

        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let starting_points = combat_rules().command().starting_points();
        assert!(rendered_text.contains("COMMANDEMENT"));
        assert!(rendered_text.contains(&format!("{starting_points}/{starting_points} PC")));
        assert!(rendered_text.contains("Disponible après le premier engagement"));
        assert!(rendered_text.contains("Lancer l'assaut"));
    }

    /// Real end-to-end run (not just a compile-time check — see the
    /// module-level B0001 note on the other `*_without_a_query_conflict`
    /// tests) of the briefing's own content-population system, on a real
    /// pending combat.
    #[test]
    fn update_combat_briefing_runs_without_a_query_conflict_and_renders_real_data() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::Briefing,
                briefed: HashSet::from([mission_id]),
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, update_combat_briefing);
        let entity_visuals = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EntityVisualCatalog::for_tests(&mut images)
        };
        app.insert_resource(entity_visuals);

        app.update();

        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("ASSAUT ORBITAL"));
        assert!(rendered_text.contains("Neutraliser les défenses orbitales"));
        assert!(rendered_text.contains("RENSEIGNEMENT"));
        assert!(rendered_text.contains("ESTIMATION ENNEMIE"));
        assert!(rendered_text.contains("Forces estimées"));
    }

    /// COMBAT-UX-001-H: a real end-to-end run of `update_final_report`
    /// across all 4 tabs, on a real resolved `CombatReport` — the detailed
    /// text must genuinely change per tab, and each tab's distinctive
    /// content must actually be reachable through the ECS system, not just
    /// the pure `inspector_panel.rs` functions in isolation.
    #[test]
    fn detailed_report_tabs_render_distinct_content_and_tab_bar_respects_the_toggle() {
        let mut simulation = simulation_with_pending_combat();
        let report = resolved_combat_report_fixture();
        let mission_id = report.mission_id;
        simulation.state_mut().combat_reports.push(report);

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::FinalReport,
                briefed: HashSet::from([mission_id]),
                showing_detailed_report: true,
                report_tab: CombatReportTab::Summary,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (update_final_report, update_report_tabs_visibility).chain(),
            );

        app.update();
        {
            let world = app.world_mut();
            let mut round_log = world.query_filtered::<&Text, With<CombatRoundLogText>>();
            assert!(
                round_log
                    .single(world)
                    .unwrap()
                    .0
                    .contains("RAPPORT DE COMBAT")
            );
            let mut bar = world.query_filtered::<&Visibility, With<ReportTabBar>>();
            assert_eq!(bar.single(world).unwrap(), Visibility::Inherited);
        }

        app.world_mut().resource_mut::<CombatUiState>().report_tab = CombatReportTab::Timeline;
        app.update();
        {
            let world = app.world_mut();
            let mut round_log = world.query_filtered::<&Text, With<CombatRoundLogText>>();
            assert!(
                round_log
                    .single(world)
                    .unwrap()
                    .0
                    .contains("Chronologie tactique")
            );
        }

        app.world_mut().resource_mut::<CombatUiState>().report_tab = CombatReportTab::Statistics;
        app.update();
        {
            let world = app.world_mut();
            let mut round_log = world.query_filtered::<&Text, With<CombatRoundLogText>>();
            assert!(
                round_log
                    .single(world)
                    .unwrap()
                    .0
                    .contains("DÉGÂTS PAR GROUPE")
            );
        }

        app.world_mut().resource_mut::<CombatUiState>().report_tab = CombatReportTab::Units;
        app.update();
        {
            let world = app.world_mut();
            let mut round_log = world.query_filtered::<&Text, With<CombatRoundLogText>>();
            assert!(round_log.single(world).unwrap().0.contains("VOTRE FLOTTE"));
        }

        app.world_mut()
            .resource_mut::<CombatUiState>()
            .showing_detailed_report = false;
        app.update();
        let world = app.world_mut();
        let mut bar = world.query_filtered::<&Visibility, With<ReportTabBar>>();
        assert_eq!(bar.single(world).unwrap(), Visibility::Hidden);
        let mut round_log = world.query_filtered::<&Text, With<CombatRoundLogText>>();
        assert!(round_log.single(world).unwrap().0.is_empty());
    }

    fn width_after_root_padding(window_width: f32) -> f32 {
        window_width - COMBAT_ROOT_PADDING_PX * 2.0
    }

    fn width_budget_from_percent(parent_width: f32, percents: &[f32], gap_px: f32) -> f32 {
        parent_width * percents.iter().sum::<f32>() / 100.0
            + gap_px * percents.len().saturating_sub(1) as f32
    }

    #[test]
    fn combat_layout_width_budgets_fit_720p_and_1080p() {
        for width in [1280.0, 1920.0] {
            let content_width = width_after_root_padding(width);
            let body_width = width_budget_from_percent(
                content_width,
                &[
                    COMBAT_FORCES_COLUMN_WIDTH_PERCENT,
                    battlefield::BATTLEFIELD_PANEL_WIDTH_PERCENT,
                    COMBAT_PARAMETERS_COLUMN_WIDTH_PERCENT,
                ],
                COMBAT_BODY_COLUMN_GAP_PX,
            );
            assert!(
                body_width <= content_width,
                "combat body columns must fit at {width}px"
            );

            let battlefield_width =
                content_width * battlefield::BATTLEFIELD_PANEL_WIDTH_PERCENT / 100.0;
            let tactical_map_width = width_budget_from_percent(
                battlefield_width,
                &[
                    battlefield::BATTLEFIELD_GROUP_LANE_WIDTH_PERCENT,
                    battlefield::BATTLEFIELD_ORBIT_LANE_WIDTH_PERCENT,
                    battlefield::BATTLEFIELD_CONTACT_LANE_WIDTH_PERCENT,
                ],
                battlefield::BATTLEFIELD_INNER_COLUMN_GAP_PX,
            );
            assert!(
                tactical_map_width <= battlefield_width,
                "battlefield lanes must fit at {width}px"
            );

            // COMBAT-UX-001-C: group cards, the doctrine picker, and the
            // intervention row now stack vertically inside the narrow
            // "PARAMÈTRES SÉLECTIONNÉS" column instead of sharing a wide
            // row — the only remaining width constraint is that the widest
            // fixed-min-width element in that column (an advanced-doctrine
            // card) still fits.
            let parameters_column_width =
                content_width * COMBAT_PARAMETERS_COLUMN_WIDTH_PERCENT / 100.0;
            assert!(
                COMBAT_DOCTRINE_CARD_MIN_WIDTH_PX <= parameters_column_width,
                "an advanced doctrine card must fit within the PARAMÈTRES column at {width}px"
            );
        }
    }

    #[test]
    fn battlefield_trajectory_labels_match_group_roles() {
        assert!(battlefield::trajectory_is_active(CombatGroupRole::Assault));
        assert!(battlefield::trajectory_is_active(CombatGroupRole::Screen));
        assert!(battlefield::trajectory_is_active(
            CombatGroupRole::Bombardment
        ));
        assert!(!battlefield::trajectory_is_active(CombatGroupRole::Reserve));
        assert!(battlefield::trajectory_label(CombatGroupRole::Assault).contains("attaque"));
        assert!(battlefield::trajectory_label(CombatGroupRole::Reserve).contains("RÉSERVE"));
    }

    #[test]
    fn battlefield_round_visual_summary_maps_exchanges_losses_and_reserves() {
        let simulation = simulation_with_pending_combat();
        let pending = &simulation.state().pending_combats[0];
        let mut draft = group_panel::CombatPlanDraft::from_pending(pending);
        let allied_stack = draft
            .groups()
            .find(|group| group.id == CombatGroupPlanId::Alpha)
            .and_then(|group| group.stacks.first().copied())
            .expect("the fixture has an allied stack in Alpha");
        let enemy_stack = enemy_intel(pending)
            .stacks
            .first()
            .expect("the fixture has an enemy contact")
            .stack_id;

        let record = CombatRoundRecord {
            round: 1,
            attacker_doctrine: CombatDoctrineId::BalancedEngagement,
            defender_doctrine: CombatDoctrineId::DefensiveScreen,
            attacker_damage: 120,
            defender_damage: 25,
            attacker_exchanges: vec![CombatStackExchange {
                source_group: CombatGroupPlanId::Alpha,
                target: enemy_stack,
                allocated_damage: 120,
            }],
            defender_exchanges: vec![CombatStackExchange {
                source_group: CombatGroupPlanId::Alpha,
                target: allied_stack,
                allocated_damage: 25,
            }],
            attacker_losses: vec![CombatStackLoss {
                stack_id: allied_stack,
                quantity: 1,
            }],
            defender_losses: vec![CombatStackLoss {
                stack_id: enemy_stack,
                quantity: 2,
            }],
            notable_events: vec![CombatRoundEvent::StackDestroyed {
                side: CombatSide::Defender,
                stack_id: enemy_stack,
            }],
            intel_after: pending.intel_percent(),
        };

        let summary = battlefield::round_visual_summary(&record, &draft);
        let alpha = summary
            .group(CombatGroupPlanId::Alpha)
            .expect("Alpha appears in the visual summary");
        assert_eq!(alpha.outgoing_damage, 120);
        assert_eq!(alpha.incoming_damage, 25);
        assert_eq!(alpha.lost_quantity, 1);
        assert_eq!(alpha.primary_target, Some(enemy_stack));

        let target = summary
            .target(enemy_stack)
            .expect("the enemy target appears in the visual summary");
        assert_eq!(target.damage, 120);
        assert_eq!(target.lost_quantity, 2);
        assert!(target.destroyed);

        draft.assign_stack(allied_stack, CombatGroupPlanId::Gamma);
        let reserve_summary = battlefield::round_visual_summary(
            &CombatRoundRecord {
                attacker_exchanges: Vec::new(),
                defender_exchanges: Vec::new(),
                attacker_losses: Vec::new(),
                defender_losses: Vec::new(),
                notable_events: Vec::new(),
                ..record
            },
            &draft,
        );
        assert!(
            reserve_summary
                .group(CombatGroupPlanId::Gamma)
                .expect("Gamma appears in the visual summary")
                .stayed_in_reserve
        );
    }

    #[test]
    fn battlefield_panel_updates_without_a_query_conflict() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::AwaitingDoctrine,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    group_panel::sync_combat_plan_draft,
                    battlefield::update_battlefield_panel,
                )
                    .chain(),
            );
        let entity_visuals = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EntityVisualCatalog::for_tests(&mut images)
        };
        app.insert_resource(entity_visuals);

        app.update();

        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("CARTE TACTIQUE"));
        assert!(rendered_text.contains("Alpha"));
        assert!(rendered_text.contains("Beta"));
        assert!(rendered_text.contains("Gamma"));
        assert!(rendered_text.contains("ORBITE"));
        assert!(rendered_text.contains("CONTACTS"));

        let mut rows = world.query::<(&battlefield::BattlefieldRow, &Visibility)>();
        assert!(rows.iter(world).any(|(row, visibility)| {
            row.0 == battlefield::BattlefieldRowKind::Enemy(0)
                && *visibility == Visibility::Inherited
        }));
    }

    #[test]
    fn battlefield_panel_renders_round_pause_exchange_annotations_without_a_query_conflict() {
        let mut simulation = simulation_with_pending_combat_using(2);
        let mission_id = simulation.state().pending_combats[0].mission_id;
        let events = simulation.apply_player_action(GameAction::ChooseCombatDoctrine {
            mission_id,
            round: 1,
            doctrine: None,
            intervention: None,
        });
        assert!(events.iter().any(|event| {
            matches!(
                event.kind,
                GameEventKind::CombatRoundResolved(CombatRoundResolved {
                    mission_id: id,
                    round: 1,
                }) if id == mission_id
            )
        }));
        assert!(
            simulation.state().pending_combat(mission_id).is_some(),
            "the 2-frigate fixture should still be pending after one round"
        );

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                phase: CombatUiPhase::RoundPause,
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, battlefield::update_battlefield_panel);
        let entity_visuals = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            EntityVisualCatalog::for_tests(&mut images)
        };
        app.insert_resource(entity_visuals);

        app.update();

        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("Round 1"));
        assert!(rendered_text.contains("résolu"));
        assert!(rendered_text.contains("Tir :") || rendered_text.contains("impact"));
    }

    #[test]
    fn a_pending_combat_reopens_the_screen_without_any_event() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.insert_resource(SimulationResource {
            simulation,
            pending_events: Vec::new(),
        })
        .init_resource::<CombatUiState>()
        .add_systems(bevy::app::Update, resume_combat_queue_after_reload);

        app.update();

        let ui = app.world().resource::<CombatUiState>();
        assert_eq!(ui.current, Some(mission_id));
        // A fresh `CombatUiState` (post-reload) has never briefed this
        // mission — COMBAT-UX-001-B shows the briefing once per combat.
        assert_eq!(ui.phase, CombatUiPhase::Briefing);
        assert_eq!(ui.queue, vec![mission_id]);
    }

    /// Real end-to-end run of `spawn_combat_screen` (Startup) + the icon-
    /// bearing `update_combat_columns` (Update) on a real pending combat —
    /// not just a compile-time check. Bevy's query-conflict panic (B0001,
    /// hit once already in this file — see the module-level comment on the
    /// `*TextQuery` aliases) only fires when a system with conflicting query
    /// params actually *runs*, so `cargo build` succeeding is not enough
    /// evidence the new icon queries are safe; this test running to
    /// completion is.
    #[test]
    fn update_combat_columns_runs_without_a_query_conflict_and_updates_header_and_intel_text() {
        let simulation = simulation_with_pending_combat();
        let mission_id = simulation.state().pending_combats[0].mission_id;

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(CombatUiState {
                current: Some(mission_id),
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, update_combat_columns);

        app.update();

        let world = app.world_mut();
        let mut header_text = world.query_filtered::<&Text, With<CombatHeaderText>>();
        assert!(
            header_text
                .single(world)
                .unwrap()
                .0
                .contains("ASSAUT ORBITAL"),
            "header text reflects the current pending combat"
        );
        let mut intel_text = world.query_filtered::<&Text, With<CombatIntelBarText>>();
        assert!(
            intel_text
                .single(world)
                .unwrap()
                .0
                .contains("RENSEIGNEMENT"),
            "intel bar text reflects the current pending combat"
        );
    }

    #[test]
    fn hidden_enemy_contact_uses_class_fallback_instead_of_exact_force_visual() {
        let simulation = simulation_with_pending_combat();
        let pending = &simulation.state().pending_combats[0];
        let base_stack = *enemy_intel(pending)
            .stacks
            .first()
            .expect("the combat fixture has at least one enemy stack");
        let force_id = galactic_sim::PlanetaryForceId::from_static("sylve_thorn");

        let mut images = Assets::<Image>::default();
        let (catalog, handles) = EntityVisualCatalog::for_tests_with_force(&mut images, force_id);
        let hidden_stack = CombatStackView {
            target_class: Some(CombatTargetClass::Medium),
            identity: None,
            ..base_stack
        };
        let revealed_stack = CombatStackView {
            identity: Some(CombatUnitRef::PlanetaryForce(force_id)),
            ..hidden_stack
        };

        assert_eq!(catalog.enemy_contact(&hidden_stack), handles.contact_medium);
        assert_ne!(catalog.enemy_contact(&hidden_stack), handles.force);
        assert_eq!(catalog.enemy_contact(&revealed_stack), handles.force);
    }
}

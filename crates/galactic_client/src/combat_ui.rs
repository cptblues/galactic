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
    CombatGroupRole, CombatIntervention, CombatInterventionError, CombatOutcome,
    CombatPlanConfirmed, CombatPlanRejected, CombatPlanValidationError, CombatReport,
    CombatReportStatus, CombatResolution, CombatRetreatRejected, CombatRoundEvent,
    CombatRoundRecord, CombatRoundResolved, CombatSide, CombatTargetClass, CombatTargetPriority,
    CombatUnitRef, DoctrineOverview, EnemyIntelView, GameAction, GameEventKind, IntegrityBand,
    IntegrityReveal, MissionTarget, PendingCombat, QualitativePrediction, QuantityBand,
    QuantityReveal, RetreatingSide, ThreatLevel, allied_stacks, combat_rules, craftable_definition,
    default_ruleset, doctrine_overview, enemy_intel, qualitative_prediction,
    repetition_penalty_preview, round_history,
};

mod battlefield;
mod group_panel;

use super::{
    PresentationUpdateSet, SimulationResource, UiPointerBlocker, action_button_color,
    action_button_outline, apply_simulation_command, collect_presentation_events,
    combat_report_statistics_text, combat_report_summary_text, combat_report_timeline_text,
    combat_report_units_text, mission_target_label, panel_background, panel_outline, ui_text_font,
};
use crate::presentation::{
    components::{
        BreadcrumbBarRoot, ResourceBarRoot, ResourceHudKind, ScrollIndicatorArea,
        ScrollIndicatorId, TabBarRoot,
    },
    entity_visuals::EntityVisualCatalog,
    icons::{IconAssets, IconKind},
    scene::{ResourceIconAssets, spawn_scroll_indicator},
};
use battlefield::{spawn_battlefield_panel, update_battlefield_panel};
use group_panel::{
    CombatPlanDraftState, handle_combat_plan_buttons, sync_combat_plan_draft,
    update_combat_plan_panel,
};

const MAX_ALLIED_ROWS: usize = 8;
/// Covers the 3 current combat-capable ship types with headroom, mirroring
/// `MAX_BATTLEFIELD_CONTACTS`/`group_panel::MAX_DRAFT_STACK_ROWS`'s fixed
/// slot count.
const MAX_COMBAT_LOSS_CARDS: usize = 6;

/// Between navigation (140) and the intro-pitch modal (220) — must surface
/// above any open gameplay panel, but stay below the two true end-to-end
/// modals (intro-pitch, victory).
const COMBAT_UI_Z_INDEX: i32 = 200;
const COMBAT_ROOT_PADDING_PX: f32 = 12.0;
const COMBAT_ROOT_ROW_GAP_PX: f32 = 8.0;
/// COMBAT-UX-001-J §5: the content column occupies most of the window
/// (90-96% width, 82-90% height) rather than the previous full-bleed
/// edge-to-edge layout — leaves visible breathing room around the screen.
const COMBAT_CONTENT_WIDTH_PERCENT: f32 = 94.0;
const COMBAT_CONTENT_HEIGHT_PERCENT: f32 = 88.0;
/// §5.1: capped so ultrawide monitors get more breathing room and a bigger
/// tactical map, not giant stretched side columns.
const COMBAT_CONTENT_MAX_WIDTH_PX: f32 = 2400.0;
const COMBAT_BODY_COLUMN_GAP_PX: f32 = 10.0;
/// COMBAT-UX-001-C/J: "VOS FORCES" — left column of the 3-column
/// Planification layout (doc §13 target: 18-22%).
const COMBAT_FORCES_COLUMN_WIDTH_PERCENT: f32 = 18.0;
/// "PARAMÈTRES SÉLECTIONNÉS" — right column of the same layout (doc §13
/// target: 20-24%).
const COMBAT_PARAMETERS_COLUMN_WIDTH_PERCENT: f32 = 20.0;
const COMBAT_DOCTRINE_CARD_MIN_WIDTH_PX: f32 = 176.0;
const COMBAT_DOCTRINE_CARD_GAP_PX: f32 = 8.0;
/// COMBAT-UX-001-J §7.2 typography scale — replaces ~7 ad hoc font sizes
/// (9/10/11/12/13/16/20px, scattered with no shared meaning) with 5 named
/// tiers. Screen/important titles bumped slightly to the doc's target
/// ranges; the 3 smallest prior sizes (9/10/11) collapse into one secondary
/// tier at the top of the doc's stated 11-12px floor for readability.
const FONT_SCREEN_TITLE_PX: f32 = 22.0;
const FONT_IMPORTANT_TITLE_PX: f32 = 17.0;
const FONT_SECTION_TITLE_PX: f32 = 14.0;
const FONT_INFO_PX: f32 = 13.0;
const FONT_SECONDARY_PX: f32 = 12.0;
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
                    update_final_report_cards,
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

/// Wraps `CombatRoundLogText` — visible during `AwaitingDoctrine`,
/// `RoundPause`, and `FinalReport` (unlike `CombatHiddenDuringBriefing`,
/// which also hides for `FinalReport`), since this entity doubles as the
/// `FinalReport` detailed-report dump. See `update_combat_briefing_visibility`.
#[derive(Component)]
struct CombatRoundLogPanelRoot;

/// COMBAT-UX-001-J §33: the Résultat screen's planet image, banner, and
/// two-column fleet/enemy summary — see `update_final_report`.
#[derive(Component)]
struct CombatResultPlanetIcon;

#[derive(Component)]
struct CombatResultBannerText;

/// Planet name + `combat_outcome_flavor_line`, below the banner.
#[derive(Component)]
struct CombatResultSubtitleText;

#[derive(Component)]
struct CombatFleetSummaryText;

#[derive(Component)]
struct CombatEnemySummaryText;

/// Doc §35 "avec assets" — up to `MAX_COMBAT_LOSS_CARDS` fixed slots, hidden
/// individually when there's no corresponding loss (mirrors the established
/// fixed-slot pattern, e.g. `group_panel::spawn_stack_row`).
#[derive(Component, Clone, Copy)]
struct CombatLossCardRow(usize);

#[derive(Component, Clone, Copy)]
struct CombatLossCardIcon(usize);

#[derive(Component, Clone, Copy)]
struct CombatLossCardText(usize);

/// Doc §36 — 3 fixed resource cards, the whole block hidden when
/// `salvage_recovered.is_zero()` in favor of `CombatLootEmptyText`.
#[derive(Component)]
struct CombatLootCardsRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct CombatLootCardText(ResourceHudKind);

#[derive(Component)]
struct CombatLootEmptyText;

/// Doc §37 — hidden (not just empty) when `combat_decisive_moment_line`
/// finds nothing notable.
#[derive(Component)]
struct CombatDecisiveMomentText;

#[derive(Component)]
struct CombatFeedbackText;

/// COMBAT-UX-001-J §29: the command-points widget is its own visual unit
/// (header + meter + count), visible from round 1 onward regardless of
/// phase — not tied to `CombatPlanningControls`, since watching PC while a
/// round resolves is meaningful even though there's nothing left to plan.
#[derive(Component)]
struct CombatCommandPointsRoot;

#[derive(Component)]
struct CombatCommandPointsMeterText;

#[derive(Component)]
struct CombatCommandPointsAvailableText;

/// The "next order" preview — only meaningful while planning, so still
/// gated by `CombatPlanningControls` unlike the PC widget itself.
#[derive(Component)]
struct CombatPreparedOrderText;

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
struct BriefingPlanetIcon;

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

fn spawn_combat_screen(
    mut commands: Commands,
    icon_assets: Res<IconAssets>,
    resource_icons: Res<ResourceIconAssets>,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            // COMBAT-UX-001-J §9.1: the strategic scene stays as a faint
            // depth cue, not a semi-legible "ghost" competing with combat's
            // own content — 0.72 alpha let orbit rings/planets from the
            // scene behind read as a confusing, unintentional overlap.
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.94)),
            GlobalZIndex(COMBAT_UI_Z_INDEX),
            Interaction::None,
            UiPointerBlocker,
            Visibility::Hidden,
            CombatUiRoot,
        ))
        .with_children(|backdrop| {
            // COMBAT-UX-001-J §5: the backdrop always covers the full
            // window, but the actual content is a centered column capped at
            // `COMBAT_CONTENT_MAX_WIDTH_PX` so it doesn't stretch into
            // absurdly wide side columns on ultrawide monitors (§5.1).
            backdrop
                .spawn(Node {
                    width: Val::Percent(COMBAT_CONTENT_WIDTH_PERCENT),
                    max_width: Val::Px(COMBAT_CONTENT_MAX_WIDTH_PX),
                    height: Val::Percent(COMBAT_CONTENT_HEIGHT_PERCENT),
                    padding: UiRect::all(Val::Px(COMBAT_ROOT_PADDING_PX)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(COMBAT_ROOT_ROW_GAP_PX),
                    overflow: Overflow::clip_y(),
                    ..default()
                })
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
                        spawn_action_button(header, CombatUiAction::PreviousCombat, "<");
                        header.spawn((
                            Text::new(""),
                            ui_text_font(FONT_IMPORTANT_TITLE_PX),
                            TextColor(Color::srgb(0.92, 0.96, 1.0)),
                            CombatHeaderText,
                        ));
                        spawn_action_button(header, CombatUiAction::NextCombat, ">");
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
                        let forces_placeholder = icon_assets.handle(IconKind::CombatUnit);
                        body.spawn((
                            Node {
                                width: Val::Percent(COMBAT_FORCES_COLUMN_WIDTH_PERCENT),
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
                                Text::new("VOS FORCES"),
                                ui_text_font(FONT_SECTION_TITLE_PX),
                                TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
                            ));
                            panel
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(group_panel::DRAFT_GROUP_CARD_GAP_PX),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ScrollPosition::default(),
                                    RelativeCursorPosition::default(),
                                    ScrollIndicatorArea {
                                        id: ScrollIndicatorId::CombatForcesColumn,
                                    },
                                ))
                                .with_children(|content| {
                                    // Reads top-to-bottom in the same order
                                    // as the actual interaction: pick a unit
                                    // here first, then click a group card
                                    // below to assign it — the reverse
                                    // spawn order (groups above units) had
                                    // the player working bottom-to-top.
                                    content.spawn((
                                        Text::new("UNITÉS"),
                                        ui_text_font(FONT_SECONDARY_PX),
                                        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
                                    ));
                                    for slot in 0..group_panel::MAX_DRAFT_STACK_ROWS {
                                        group_panel::spawn_stack_row(content, slot);
                                    }
                                    content.spawn((
                                        Text::new("GROUPES"),
                                        ui_text_font(FONT_SECONDARY_PX),
                                        TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
                                    ));
                                    group_panel::spawn_forces_group_cards(
                                        content,
                                        forces_placeholder,
                                    );
                                });
                            spawn_scroll_indicator(panel, ScrollIndicatorId::CombatForcesColumn);
                        });

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
                                    group_panel::spawn_selected_group_params(content);

                                    content.spawn((
                                        Text::new("CHOISIR UNE DOCTRINE"),
                                        ui_text_font(FONT_SECTION_TITLE_PX),
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
                                                spawn_simple_doctrine_card(row, doctrine, label);
                                            }
                                        });

                                    content
                                        .spawn((Node::default(), CombatPlanningControls))
                                        .with_children(|row| {
                                            spawn_action_button(
                                                row,
                                                CombatUiAction::ToggleAdvancedDoctrines,
                                                "Tactiques avancées [+]",
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

                                    content
                                        .spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(2.0),
                                                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                                                border: UiRect::all(Val::Px(1.0)),
                                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                                ..default()
                                            },
                                            BackgroundColor(panel_background()),
                                            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                                            Visibility::Hidden,
                                            CombatCommandPointsRoot,
                                        ))
                                        .with_children(|widget| {
                                            widget.spawn((
                                                Text::new("POINTS DE COMMANDEMENT"),
                                                ui_text_font(FONT_SECONDARY_PX),
                                                TextColor(Color::srgb(0.6, 0.68, 0.76)),
                                            ));
                                            widget.spawn((
                                                Text::new(""),
                                                ui_text_font(FONT_IMPORTANT_TITLE_PX),
                                                TextColor(Color::srgb(0.78, 0.86, 1.0)),
                                                CombatCommandPointsMeterText,
                                            ));
                                            widget.spawn((
                                                Text::new(""),
                                                ui_text_font(FONT_SECONDARY_PX),
                                                TextColor(Color::srgb(0.78, 0.86, 1.0)),
                                                CombatCommandPointsAvailableText,
                                            ));
                                        });

                                    content.spawn((
                                        Text::new(""),
                                        ui_text_font(FONT_SECONDARY_PX),
                                        TextColor(Color::srgb(0.7, 0.78, 0.86)),
                                        CombatPreparedOrderText,
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
                                                spawn_intervention_button(
                                                    interventions,
                                                    CombatUiAction::SelectFocusPriority(priority),
                                                    focus_fire_button_label(priority),
                                                    intervention_command_cost(
                                                        CombatIntervention::FocusFire { priority },
                                                    ),
                                                );
                                            }
                                            for group_id in CombatGroupPlanId::ALL {
                                                spawn_intervention_button(
                                                    interventions,
                                                    CombatUiAction::CommitReserve(group_id),
                                                    reserve_button_label(group_id),
                                                    intervention_command_cost(
                                                        CombatIntervention::CommitReserve {
                                                            group_id,
                                                        },
                                                    ),
                                                );
                                            }
                                        });
                                });
                            group_panel::spawn_plan_footer(panel);
                            spawn_scroll_indicator(
                                panel,
                                ScrollIndicatorId::CombatParametersColumn,
                            );
                        });
                    });

                    // COMBAT-UX-001-J §28: framed like every other panel in
                    // this screen, not a caption crammed into "CARTE
                    // TACTIQUE"'s header row. This entity is reused for 3
                    // different contents depending on phase (default "Zone
                    // orbitale" caption, the post-round summary, and the
                    // `FinalReport` detailed-report dump — see
                    // `update_combat_feedback`/`update_final_report`), so it
                    // needs its own visibility rule: unlike the tactical body
                    // row it must stay visible during `FinalReport` (that's
                    // exactly when the detailed-report dump is shown), only
                    // hidden during `Briefing` — see
                    // `update_combat_briefing_visibility`.
                    root.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(panel_background()),
                        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                        CombatRoundLogPanelRoot,
                    ))
                    .with_children(|panel| {
                        panel.spawn((
                            Text::new("Zone orbitale"),
                            ui_text_font(FONT_SECONDARY_PX),
                            TextColor(Color::srgb(0.80, 0.88, 0.94)),
                            CombatRoundLogText,
                        ));
                    });

                    root.spawn((
                        Text::new(""),
                        ui_text_font(FONT_SECTION_TITLE_PX),
                        TextColor(Color::srgb(0.78, 0.86, 1.0)),
                        CombatIntelBarText,
                        CombatHiddenDuringBriefing,
                    ));

                    root.spawn((
                        Text::new(""),
                        ui_text_font(FONT_SECONDARY_PX),
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
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            padding: UiRect::all(Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        // Opaque, like every other panel in this screen —
                        // without it, the 3D strategic scene's own star
                        // glow bleeds through right behind the flotte/
                        // ennemi cards (found from a playtest screenshot,
                        // reads as a rendering glitch, not intentional).
                        BackgroundColor(panel_background()),
                        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
                        Visibility::Hidden,
                        CombatReturnRow,
                    ))
                    .with_children(|outer| {
                        outer
                            .spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(10.0),
                                    flex_grow: 1.0,
                                    min_height: Val::Px(0.0),
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                                ScrollPosition::default(),
                                RelativeCursorPosition::default(),
                                ScrollIndicatorArea {
                                    id: ScrollIndicatorId::CombatResult,
                                },
                            ))
                            .with_children(|row| {
                                let planet_placeholder = icon_assets.handle(IconKind::CombatUnit);
                                row.spawn((
                                    Node {
                                        width: Val::Px(150.0),
                                        height: Val::Px(150.0),
                                        // Without this, a Column flex item
                                        // with insufficient vertical room
                                        // shrinks along the main axis only —
                                        // squeezing a fixed-width circle
                                        // into an oval/pill shape instead of
                                        // clipping or scrolling. Confirmed
                                        // via playtest screenshot.
                                        flex_shrink: 0.0,
                                        border: UiRect::all(Val::Px(2.0)),
                                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.06, 0.08, 0.09, 0.82)),
                                    Outline::new(
                                        Val::Px(1.0),
                                        Val::Px(3.0),
                                        Color::srgba(0.42, 0.70, 0.86, 0.38),
                                    ),
                                ))
                                .with_children(|circle| {
                                    circle.spawn((
                                        ImageNode {
                                            image: planet_placeholder,
                                            color: Color::WHITE,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Percent(115.0),
                                            height: Val::Percent(115.0),
                                            position_type: PositionType::Absolute,
                                            // See battlefield.rs's spawn_orbit_lane: center
                                            // the oversized image instead of the top-left
                                            // default, or the circular clip crops the
                                            // top-left arc.
                                            left: Val::Percent(-7.5),
                                            top: Val::Percent(-7.5),
                                            // `Overflow::clip()` on the parent
                                            // only clips to its rectangular
                                            // bounding box, not a circle — the
                                            // image needs its own
                                            // border_radius to actually render
                                            // round instead of square-cornered.
                                            border_radius: BorderRadius::all(Val::Percent(50.0)),
                                            ..default()
                                        },
                                        CombatResultPlanetIcon,
                                    ));
                                });
                                row.spawn((
                                    Text::new(""),
                                    ui_text_font(FONT_SCREEN_TITLE_PX),
                                    TextColor(Color::WHITE),
                                    CombatResultBannerText,
                                ));
                                row.spawn((
                                    Text::new(""),
                                    ui_text_font(FONT_INFO_PX),
                                    TextColor(Color::srgb(0.80, 0.86, 0.92)),
                                    Node {
                                        width: Val::Percent(100.0),
                                        ..default()
                                    },
                                    CombatResultSubtitleText,
                                ));

                                row.spawn(Node {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(16.0),
                                    ..default()
                                })
                                .with_children(|columns| {
                                    spawn_result_summary_column(
                                        columns,
                                        "VOTRE FLOTTE",
                                        CombatFleetSummaryText,
                                    );
                                    spawn_result_summary_column(
                                        columns,
                                        "ENNEMI",
                                        CombatEnemySummaryText,
                                    );
                                });

                                row.spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::Center,
                                    flex_wrap: FlexWrap::Wrap,
                                    column_gap: Val::Px(8.0),
                                    row_gap: Val::Px(8.0),
                                    ..default()
                                })
                                .with_children(|losses| {
                                    let loss_placeholder = icon_assets.handle(IconKind::CombatUnit);
                                    for slot in 0..MAX_COMBAT_LOSS_CARDS {
                                        spawn_loss_card(losses, slot, loss_placeholder.clone());
                                    }
                                });

                                row.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::Center,
                                        column_gap: Val::Px(10.0),
                                        ..default()
                                    },
                                    Visibility::Hidden,
                                    CombatLootCardsRoot,
                                ))
                                .with_children(|loot| {
                                    for kind in [
                                        ResourceHudKind::Metal,
                                        ResourceHudKind::Crystal,
                                        ResourceHudKind::Fuel,
                                    ] {
                                        spawn_loot_card(loot, kind, &resource_icons);
                                    }
                                });
                                row.spawn((
                                    Text::new("Aucun butin récupéré."),
                                    ui_text_font(FONT_SECONDARY_PX),
                                    TextColor(Color::srgba(0.72, 0.78, 0.84, 0.80)),
                                    CombatLootEmptyText,
                                ));

                                row.spawn((
                                    Text::new(""),
                                    ui_text_font(FONT_SECONDARY_PX),
                                    TextColor(Color::srgb(0.86, 0.80, 0.60)),
                                    CombatDecisiveMomentText,
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
                                        CombatUiAction::SelectReportTab(
                                            CombatReportTab::Statistics,
                                        ),
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
                        spawn_scroll_indicator(outer, ScrollIndicatorId::CombatResult);
                    });
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
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(panel_background()),
        Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        Visibility::Hidden,
        CombatBriefingControls,
    ))
    .with_children(|outer| {
        outer
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                ScrollIndicatorArea {
                    id: ScrollIndicatorId::CombatBriefing,
                },
            ))
            .with_children(|scroll_area| {
                // `margin: auto` (not `justify_content: Center`, which would
                // center the content past *both* edges when it overflows —
                // a real regression caught via screenshot) centers the card
                // vertically only when there's spare room; CSS auto-margins
                // collapse to zero once content is taller than the viewport,
                // so it safely falls back to top-aligned + scrollable.
                scroll_area
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        margin: UiRect::vertical(Val::Auto),
                        ..default()
                    })
                    .with_children(|card| {
                        let planet_placeholder = icon_assets.handle(IconKind::CombatUnit);
                        card.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(14.0),
                            ..default()
                        })
                        .with_children(|heading| {
                            heading
                                .spawn((
                                    Node {
                                        width: Val::Px(96.0),
                                        height: Val::Px(96.0),
                                        flex_shrink: 0.0,
                                        border: UiRect::all(Val::Px(2.0)),
                                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.06, 0.08, 0.09, 0.82)),
                                    Outline::new(
                                        Val::Px(1.0),
                                        Val::Px(3.0),
                                        Color::srgba(0.42, 0.70, 0.86, 0.38),
                                    ),
                                ))
                                .with_children(|circle| {
                                    circle.spawn((
                                        ImageNode {
                                            image: planet_placeholder,
                                            color: Color::WHITE,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Percent(115.0),
                                            height: Val::Percent(115.0),
                                            position_type: PositionType::Absolute,
                                            left: Val::Percent(-7.5),
                                            top: Val::Percent(-7.5),
                                            // `Overflow::clip()` on the parent only
                                            // clips to its rectangular bounding box
                                            // — border_radius doesn't affect clip
                                            // shape, only how *this node's own*
                                            // background/image is drawn. Without
                                            // its own border_radius, the image
                                            // renders as a square whose corners
                                            // just happen to sit inside the round
                                            // frame, not an actual circle.
                                            border_radius: BorderRadius::all(Val::Percent(50.0)),
                                            ..default()
                                        },
                                        BriefingPlanetIcon,
                                    ));
                                });
                            heading.spawn((
                                Text::new(""),
                                ui_text_font(FONT_SCREEN_TITLE_PX),
                                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                                BriefingTitleText,
                            ));
                        });
                        card.spawn((
                            Text::new("OBJECTIF"),
                            ui_text_font(FONT_SECONDARY_PX),
                            TextColor(Color::srgba(0.78, 0.86, 1.0, 0.72)),
                        ));
                        card.spawn((
                            Text::new(""),
                            ui_text_font(FONT_INFO_PX),
                            TextColor(Color::srgb(0.86, 0.92, 0.98)),
                            BriefingObjectiveText,
                        ));
                        card.spawn((
                            Text::new("VOTRE FLOTTE"),
                            ui_text_font(FONT_SECONDARY_PX),
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
                                        width: Val::Px(32.0),
                                        height: Val::Px(32.0),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    BriefingFleetIcon(slot),
                                ));
                                row.spawn((
                                    Text::new(""),
                                    ui_text_font(FONT_INFO_PX),
                                    TextColor(Color::srgb(0.86, 0.92, 0.98)),
                                    BriefingFleetText(slot),
                                ));
                            });
                        }
                        card.spawn((
                            Text::new(""),
                            ui_text_font(FONT_INFO_PX),
                            TextColor(Color::srgb(0.78, 0.86, 1.0)),
                            BriefingIntelText,
                        ));
                        card.spawn((
                            Text::new(""),
                            ui_text_font(FONT_INFO_PX),
                            TextColor(Color::srgb(0.92, 0.80, 0.80)),
                            BriefingEstimateText,
                        ));
                        card.spawn(Node {
                            margin: UiRect::top(Val::Px(10.0)),
                            ..default()
                        })
                        .with_children(|footer| {
                            spawn_action_button(
                                footer,
                                CombatUiAction::StartPlanning,
                                "PRÉPARER L'ASSAUT [Entrée]",
                            );
                        });
                    });
            });
        spawn_scroll_indicator(outer, ScrollIndicatorId::CombatBriefing);
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

/// The core offense/damage-taken tradeoff, straight from the ruleset
/// (`combat_rules()`, read once at spawn time since the ruleset never
/// changes mid-session) — real numbers, not invented flavor text. Shared by
/// both the simple doctrine cards (this line alone) and the advanced ones
/// (`doctrine_card_text`, this line plus mechanic/counter detail).
fn doctrine_offense_defense_line(doctrine: CombatDoctrineId) -> String {
    let overview: DoctrineOverview = doctrine_overview(doctrine, combat_rules());
    format!(
        "Offense {} · Dégâts reçus {}",
        per_mille_multiplier_text(overview.offense_multiplier_per_mille),
        per_mille_multiplier_text(overview.damage_taken_multiplier_per_mille),
    )
}

/// Doc §14.6 card body — `doctrine_offense_defense_line` plus the one
/// doctrine-specific mechanical line and the counter relationship with its
/// real multiplier.
fn doctrine_card_text(doctrine: CombatDoctrineId) -> String {
    let overview: DoctrineOverview = doctrine_overview(doctrine, combat_rules());
    let mut lines = vec![
        doctrine_offense_defense_line(doctrine),
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
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
            ));
            card.spawn((
                Text::new(doctrine_card_text(doctrine)),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgba(0.80, 0.86, 0.90, 0.90)),
            ));
            card.spawn((
                Text::new(""),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgb(0.94, 0.72, 0.42)),
                DoctrineExtraText(doctrine),
            ));
        });
}

/// Playtest feedback: "Doctrine équilibré, prudent ou agressif, je ne sais
/// pas ce que ça fait" — the simple doctrine picker was a bare label with
/// no indication of effect. Same 2-line card shape as
/// `spawn_intervention_button`, but with `doctrine_offense_defense_line`'s
/// real ruleset numbers instead of a cost — the core stance tradeoff, not
/// the full advanced-card breakdown (mechanic/counter detail stays
/// exclusive to "Tactiques avancées", keeping the simple picker simple).
fn spawn_simple_doctrine_card(
    parent: &mut ChildSpawnerCommands,
    doctrine: CombatDoctrineId,
    label: &str,
) {
    parent
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
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
                Text::new(format!("{label} [{}]", doctrine_shortcut_digit(doctrine))),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::WHITE),
            ));
            card.spawn((
                Text::new(doctrine_offense_defense_line(doctrine)),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgba(0.80, 0.86, 0.90, 0.90)),
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
            ui_text_font(FONT_SECTION_TITLE_PX),
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

/// COMBAT-UX-001-J §30: each intervention is a 2-line card (name, then its
/// PC cost) instead of a bare label — availability/greying is unaffected,
/// `update_action_buttons`/`update_reserve_intervention_visibility` key off
/// `CombatActionButton` on the button entity itself, not its children.
fn spawn_intervention_button(
    parent: &mut ChildSpawnerCommands,
    action: CombatUiAction,
    name: &str,
    cost: u8,
) {
    parent
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
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
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(name),
                ui_text_font(FONT_INFO_PX),
                TextColor(Color::WHITE),
            ));
            let cost_label = if cost == 0 {
                "Gratuit".to_string()
            } else {
                format!("{cost} PC")
            };
            button.spawn((
                Text::new(cost_label),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgb(0.7, 0.78, 0.86)),
            ));
        });
}

/// COMBAT-UX-001-J §33's "VOTRE FLOTTE"/"ENNEMI" columns.
fn spawn_result_summary_column(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    marker: impl Component,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|column| {
            column.spawn((
                Text::new(title),
                ui_text_font(FONT_SECTION_TITLE_PX),
                TextColor(Color::srgba(0.78, 0.86, 1.0, 0.88)),
            ));
            column.spawn((
                Text::new(""),
                ui_text_font(FONT_INFO_PX),
                TextColor(Color::srgb(0.88, 0.92, 0.96)),
                marker,
            ));
        });
}

/// Doc §35 — one fixed slot; hidden by default, shown/populated by
/// `update_final_report` for each real `CombatShipLoss`.
fn spawn_loss_card(parent: &mut ChildSpawnerCommands, slot: usize, placeholder: Handle<Image>) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.20, 0.06, 0.05, 0.55)),
            Outline::new(Val::Px(1.0), Val::ZERO, Color::srgba(1.0, 0.42, 0.36, 0.42)),
            Visibility::Hidden,
            CombatLossCardRow(slot),
        ))
        .with_children(|card| {
            card.spawn((
                ImageNode {
                    image: placeholder,
                    color: Color::WHITE,
                    ..default()
                },
                Node {
                    width: Val::Px(24.0),
                    height: Val::Px(24.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                CombatLossCardIcon(slot),
            ));
            card.spawn((
                Text::new(""),
                ui_text_font(FONT_SECONDARY_PX),
                TextColor(Color::srgb(0.94, 0.86, 0.84)),
                CombatLossCardText(slot),
            ));
        });
}

/// Doc §36 — one of the 3 fixed loot cards; the icon never changes per
/// frame (kind-determined at spawn), only the value text does. Uses the
/// same real resource art the always-visible resource bar shows
/// (`ResourceIconAssets`, pre-loaded from `resource_bar_icon_path`), not
/// `IconKind`'s procedural masks — those were defined for
/// `IconKind::Metal/Crystal/Fuel` but never actually consumed anywhere in
/// the client, real per-resource art already existed.
fn spawn_loot_card(
    parent: &mut ChildSpawnerCommands,
    kind: ResourceHudKind,
    resource_icons: &ResourceIconAssets,
) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(panel_background()),
            Outline::new(Val::Px(1.0), Val::ZERO, panel_outline()),
        ))
        .with_children(|card| {
            card.spawn((
                ImageNode {
                    image: resource_icons.handle(kind),
                    color: Color::WHITE,
                    ..default()
                },
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    ..default()
                },
            ));
            card.spawn((
                Text::new(""),
                ui_text_font(FONT_INFO_PX),
                TextColor(Color::srgb(0.90, 0.94, 0.98)),
                CombatLootCardText(kind),
            ));
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
        Without<CombatValidateButtonLabel>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
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
            "ASSAUT ORBITAL — {target_label}\nRound {}/{} · {phase_label}",
            pending.round() + 1,
            pending.maximum_rounds(),
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
        // Playtest feedback: no visible way to improve enemy intel — this
        // describes the real, already-implemented mechanic
        // (`apply_round_intel_gain`, +5%/round), not a new one.
        let progression = if enemy.intel_percent < 95 {
            "\nS'améliore automatiquement à chaque round."
        } else {
            ""
        };
        text.0 = format!(
            "RENSEIGNEMENT : {} %\n{note}\nPrévision : {prediction}{progression}",
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

type BriefingFleetRowQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static BriefingFleetRow,
        &'static mut Visibility,
        &'static mut Node,
    ),
>;

type BriefingFleetIconQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BriefingFleetIcon, &'static mut ImageNode),
    Without<BriefingPlanetIcon>,
>;

type BriefingPlanetIconQuery<'w, 's> =
    Query<'w, 's, &'static mut ImageNode, (With<BriefingPlanetIcon>, Without<BriefingFleetIcon>)>;

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
    mut planet_icon: BriefingPlanetIconQuery,
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
    if let Ok(mut icon) = planet_icon.single_mut()
        && let Some(planet) = simulation
            .simulation()
            .universe_repository()
            .planet(pending.planet_id)
    {
        icon.image = entity_visuals.planet(pending.planet_id, planet.kind);
        icon.color = Color::WHITE;
    }
    if let Ok(mut text) = objective_text.single_mut() {
        text.0 = "Neutraliser les défenses orbitales de la planète.".to_string();
    }

    let allied = allied_stacks(pending);
    for (row, mut visibility, mut node) in &mut fleet_rows {
        let present = allied.get(row.0).is_some();
        // Unpopulated slots need `Display::None`, not just
        // `Visibility::Hidden` — otherwise up to 5 empty rows still reserve
        // their layout space, pushing the intel/estimate text and the CTA
        // button below the card's clipped bottom edge.
        let next_visibility = if present {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let next_display = if present {
            Display::Flex
        } else {
            Display::None
        };
        if *visibility != next_visibility {
            *visibility = next_visibility;
        }
        if node.display != next_display {
            node.display = next_display;
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
        text.0 = if enemy.intel_percent < 95 {
            format!(
                "RENSEIGNEMENT : {} %\nS'améliorera automatiquement une fois le combat engagé.",
                enemy.intel_percent
            )
        } else {
            format!("RENSEIGNEMENT : {} %", enemy.intel_percent)
        };
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

type CombatUiRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CombatUiRoot>,
        Without<TabBarRoot>,
        Without<ResourceBarRoot>,
        Without<BreadcrumbBarRoot>,
    ),
>;

type TabBarRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<TabBarRoot>,
        Without<CombatUiRoot>,
        Without<ResourceBarRoot>,
        Without<BreadcrumbBarRoot>,
    ),
>;

type ResourceBarRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<ResourceBarRoot>,
        Without<CombatUiRoot>,
        Without<TabBarRoot>,
        Without<BreadcrumbBarRoot>,
    ),
>;

type BreadcrumbBarRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<BreadcrumbBarRoot>,
        Without<CombatUiRoot>,
        Without<TabBarRoot>,
        Without<ResourceBarRoot>,
    ),
>;

/// COMBAT-UX-001-J §9: the strategic chrome behind combat (bottom command
/// dock, resource bar, breadcrumb) must be masked, not just visually covered
/// by the backdrop — none of these had any visibility gate before (confirmed:
/// no existing system toggled any of them). The inspector panel gets the
/// same treatment directly in `update_info_panel` instead, since it already
/// owns a more nuanced visibility condition and a second system fighting
/// over the same `Visibility` component would race depending on system
/// order.
fn update_combat_visibility(
    ui: Res<CombatUiState>,
    mut roots: CombatUiRootQuery,
    mut tab_bar: TabBarRootQuery,
    mut resource_bar: ResourceBarRootQuery,
    mut breadcrumb_bar: BreadcrumbBarRootQuery,
) {
    let combat_visible = if ui.current.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut root in &mut roots {
        if *root != combat_visible {
            *root = combat_visible;
        }
    }

    let strategic_chrome_visible = if ui.current.is_some() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut tab_bar {
        if *visibility != strategic_chrome_visible {
            *visibility = strategic_chrome_visible;
        }
    }
    for mut visibility in &mut resource_bar {
        if *visibility != strategic_chrome_visible {
            *visibility = strategic_chrome_visible;
        }
    }
    for mut visibility in &mut breadcrumb_bar {
        if *visibility != strategic_chrome_visible {
            *visibility = strategic_chrome_visible;
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
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

type CombatCommandPointsMeterTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatCommandPointsMeterText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
    ),
>;

type CombatCommandPointsAvailableTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatCommandPointsAvailableText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
        Without<CombatCommandPointsMeterText>,
        Without<CombatPreparedOrderText>,
    ),
>;

type CombatPreparedOrderTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatPreparedOrderText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<DoctrineExtraText>,
        Without<RetreatButtonLabel>,
        Without<CombatValidateButtonLabel>,
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
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
        meter.push(if index < available { '#' } else { '.' });
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

#[allow(clippy::too_many_arguments)]
fn update_command_panel(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    draft: Res<CombatPlanDraftState>,
    mut command_points_root: Query<&mut Visibility, With<CombatCommandPointsRoot>>,
    mut meter_texts: CombatCommandPointsMeterTextQuery,
    mut available_texts: CombatCommandPointsAvailableTextQuery,
    mut prepared_texts: CombatPreparedOrderTextQuery,
    mut validate_labels: CombatValidateButtonLabelQuery,
) {
    let pending = ui
        .current
        .and_then(|mission_id| simulation.simulation().state().pending_combat(mission_id));
    let Some(pending) = pending else {
        for mut visibility in &mut command_points_root {
            *visibility = Visibility::Hidden;
        }
        if let Ok(mut text) = prepared_texts.single_mut() {
            text.0.clear();
        }
        return;
    };

    let cost = round_command_cost(pending, ui.chosen_doctrine, ui.selected_intervention);
    let initial_plan_dirty = pending.round() == 0 && draft.is_dirty();

    // COMBAT-UX-001-J §29: "visible uniquement après le premier round."
    let pc_widget_visible = pending.round() > 0;
    for mut visibility in &mut command_points_root {
        *visibility = if pc_widget_visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if pc_widget_visible {
        let maximum = pending.command_points_maximum();
        let available = pending.command_points_remaining();
        if let Ok(mut text) = meter_texts.single_mut() {
            text.0 = command_points_meter(available, maximum);
        }
        if let Ok(mut text) = available_texts.single_mut() {
            text.0 = format!("{available} disponible(s)");
        }
    }

    if let Ok(mut text) = prepared_texts.single_mut() {
        text.0 = if pending.round() == 0 {
            String::new()
        } else {
            let prepared =
                prepared_order_label(pending, ui.chosen_doctrine, ui.selected_intervention);
            let cost_label = if cost == 0 {
                "gratuit".to_string()
            } else {
                format!("-{cost} PC")
            };
            format!("Prochaine action : {prepared} ({cost_label})")
        };
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
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
            // Not restricted to `AwaitingDoctrine` — the queue nav row is
            // visible in every phase, so its buttons should reflect whether
            // there's actually another combat to cycle to, not which phase
            // the current one happens to be in (playtest feedback: the
            // arrows looked clickable but did nothing during Briefing even
            // when they'd have worked seconds later).
            CombatUiAction::PreviousCombat | CombatUiAction::NextCombat => {
                (ui.queue.len() > 1, false)
            }
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

// Bevy's `QueryFilter` tuple impl tops out at 15 elements, so the tail is
// nested one level down (a tuple of filters is itself a valid filter).
type CombatRoundLogTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatRoundLogText>,
        Without<CombatFeedbackText>,
        Without<CombatHeaderText>,
        Without<CombatIntelBarText>,
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
        Without<DoctrineExtraText>,
        (
            Without<RetreatButtonLabel>,
            Without<CombatValidateButtonLabel>,
            Without<CombatResultBannerText>,
            Without<CombatResultSubtitleText>,
            Without<CombatFleetSummaryText>,
            Without<CombatEnemySummaryText>,
            Without<CombatDecisiveMomentText>,
            Without<CombatLossCardText>,
            Without<CombatLootCardText>,
        ),
    ),
>;

// COMBAT-UX-001-J §33: the Résultat screen's own Text/Visibility/ImageNode
// marker set — same B0001-avoidance discipline as the header/round-log
// aliases above (see that module comment), scoped to the markers this
// screen's own `update_final_report` combines in one system.
type CombatResultBannerTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static mut TextColor),
    (
        With<CombatResultBannerText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
    ),
>;

type CombatResultSubtitleTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatResultSubtitleText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
    ),
>;

type CombatFleetSummaryTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatFleetSummaryText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
    ),
>;

type CombatEnemySummaryTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<CombatEnemySummaryText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
    ),
>;

type CombatDecisiveMomentTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Text, &'static mut Visibility),
    (
        With<CombatDecisiveMomentText>,
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatLossCardText>,
        Without<CombatLootCardText>,
        Without<CombatLossCardRow>,
        Without<CombatLootCardsRoot>,
        Without<CombatLootEmptyText>,
    ),
>;

type CombatLossCardRowQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CombatLossCardRow, &'static mut Visibility),
    (
        Without<CombatDecisiveMomentText>,
        Without<CombatLootCardsRoot>,
        Without<CombatLootEmptyText>,
    ),
>;

type CombatLossCardIconQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CombatLossCardIcon, &'static mut ImageNode),
    Without<CombatResultPlanetIcon>,
>;

type CombatLossCardTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CombatLossCardText, &'static mut Text),
    (
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLootCardText>,
    ),
>;

type CombatLootCardsRootQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CombatLootCardsRoot>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardRow>,
        Without<CombatLootEmptyText>,
    ),
>;

type CombatLootEmptyTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Visibility,
    (
        With<CombatLootEmptyText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardRow>,
        Without<CombatLootCardsRoot>,
    ),
>;

type CombatLootCardTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static CombatLootCardText, &'static mut Text),
    (
        Without<CombatHeaderText>,
        Without<CombatRoundLogText>,
        Without<CombatResultBannerText>,
        Without<CombatResultSubtitleText>,
        Without<CombatFleetSummaryText>,
        Without<CombatEnemySummaryText>,
        Without<CombatDecisiveMomentText>,
        Without<CombatLossCardText>,
    ),
>;

type CombatResultPlanetIconQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut ImageNode,
    (With<CombatResultPlanetIcon>, Without<CombatLossCardIcon>),
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
        Without<CombatCommandPointsMeterText>,
        Without<CombatCommandPointsAvailableText>,
        Without<CombatPreparedOrderText>,
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
#[allow(clippy::too_many_arguments)]
fn update_final_report(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut header_texts: CombatHeaderTextQuery,
    mut round_log: CombatRoundLogTextQuery,
    mut planet_icon: CombatResultPlanetIconQuery,
    mut banner: CombatResultBannerTextQuery,
    mut subtitle: CombatResultSubtitleTextQuery,
    mut fleet: CombatFleetSummaryTextQuery,
    mut enemy: CombatEnemySummaryTextQuery,
    mut decisive_moment: CombatDecisiveMomentTextQuery,
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
        text.0 = format!("ASSAUT ORBITAL — {target_label}\nCombat terminé");
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
    if let Ok(mut icon) = planet_icon.single_mut()
        && let Some(planet) = simulation
            .simulation()
            .universe_repository()
            .planet(report.planet_id)
    {
        icon.image = entity_visuals.planet(report.planet_id, planet.kind);
        icon.color = Color::WHITE;
    }

    let CombatReportStatus::Resolved(resolution) = &report.status else {
        if let Ok((mut text, mut color)) = banner.single_mut() {
            text.0 = "COMBAT ANNULÉ".to_string();
            color.0 = Color::srgb(0.78, 0.82, 0.86);
        }
        if let Ok(mut text) = subtitle.single_mut() {
            text.0 = format!("{target_label}\nLa cible n'était plus valide.");
        }
        if let Ok(mut text) = fleet.single_mut() {
            text.0.clear();
        }
        if let Ok(mut text) = enemy.single_mut() {
            text.0.clear();
        }
        if let Ok((mut text, mut visibility)) = decisive_moment.single_mut() {
            text.0.clear();
            *visibility = Visibility::Hidden;
        }
        return;
    };

    if let Ok((mut text, mut color)) = banner.single_mut() {
        text.0 = combat_outcome_banner_label(resolution.outcome).to_string();
        color.0 = combat_outcome_banner_color(resolution.outcome);
    }
    if let Ok(mut text) = subtitle.single_mut() {
        text.0 = format!(
            "{target_label}\n{}",
            combat_outcome_flavor_line(resolution.outcome)
        );
    }
    if let Ok(mut text) = fleet.single_mut() {
        text.0 = combat_fleet_summary_lines(report, resolution);
    }
    if let Ok(mut text) = enemy.single_mut() {
        text.0 = combat_enemy_summary_lines(report, resolution);
    }

    let moment_line = combat_decisive_moment_line(&report.round_history)
        .trim_start()
        .to_string();
    if let Ok((mut text, mut visibility)) = decisive_moment.single_mut() {
        *visibility = if moment_line.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        text.0 = moment_line;
    }
}

/// Doc §35/§36 — split out of `update_final_report` purely because Bevy
/// caps a system function at 16 parameters; same `FinalReport`-only gating,
/// same data source (`CombatResolution`).
#[allow(clippy::too_many_arguments)]
fn update_final_report_cards(
    ui: Res<CombatUiState>,
    simulation: Res<SimulationResource>,
    entity_visuals: Res<EntityVisualCatalog>,
    mut loss_rows: CombatLossCardRowQuery,
    mut loss_icons: CombatLossCardIconQuery,
    mut loss_texts: CombatLossCardTextQuery,
    mut loot_root: CombatLootCardsRootQuery,
    mut loot_empty: CombatLootEmptyTextQuery,
    mut loot_texts: CombatLootCardTextQuery,
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
    let CombatReportStatus::Resolved(resolution) = &report.status else {
        for (_, mut visibility) in &mut loss_rows {
            *visibility = Visibility::Hidden;
        }
        if let Ok(mut visibility) = loot_root.single_mut() {
            *visibility = Visibility::Hidden;
        }
        if let Ok(mut visibility) = loot_empty.single_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    for (marker, mut visibility) in &mut loss_rows {
        *visibility = if resolution.attacker_losses.get(marker.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for (marker, mut icon) in &mut loss_icons {
        if let Some(loss) = resolution.attacker_losses.get(marker.0) {
            icon.image = entity_visuals.ship(loss.craftable);
            icon.color = Color::WHITE;
        }
    }
    for (marker, mut text) in &mut loss_texts {
        text.0 = resolution
            .attacker_losses
            .get(marker.0)
            .map(|loss| {
                format!(
                    "{}\n{} perdu(s)",
                    craftable_definition(loss.craftable).name,
                    loss.quantity
                )
            })
            .unwrap_or_default();
    }

    let salvage = resolution.salvage_recovered;
    let has_loot = !salvage.is_zero();
    if let Ok(mut visibility) = loot_root.single_mut() {
        *visibility = if has_loot {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut visibility) = loot_empty.single_mut() {
        *visibility = if has_loot {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (marker, mut text) in &mut loot_texts {
        text.0 = match marker.0 {
            ResourceHudKind::Metal => format!("+{}", salvage.metal),
            ResourceHudKind::Crystal => format!("+{}", salvage.crystal),
            ResourceHudKind::Fuel => format!("+{}", salvage.fuel),
            ResourceHudKind::Energy => String::new(),
        };
    }
}

/// COMBAT-UX-001-J §33: the banner's large screen title — distinct from
/// `combat_outcome_label` (lowercase prose, used by the detailed-report
/// dump's technical summary line).
fn combat_outcome_banner_label(outcome: CombatOutcome) -> &'static str {
    match outcome {
        CombatOutcome::AttackerVictory => "VICTOIRE",
        CombatOutcome::DefenderVictory => "DÉFAITE",
        CombatOutcome::Stalemate => "AFFRONTEMENT INDÉCIS",
        CombatOutcome::MutualDestruction => "DESTRUCTION MUTUELLE",
        CombatOutcome::Retreat { .. } => "REPLI",
    }
}

fn combat_outcome_banner_color(outcome: CombatOutcome) -> Color {
    match outcome {
        CombatOutcome::AttackerVictory => Color::srgb(0.42, 0.92, 0.78),
        CombatOutcome::DefenderVictory => Color::srgb(1.0, 0.42, 0.36),
        CombatOutcome::Retreat { .. } => Color::srgb(0.95, 0.78, 0.42),
        CombatOutcome::Stalemate | CombatOutcome::MutualDestruction => {
            Color::srgb(0.78, 0.82, 0.86)
        }
    }
}

/// Doc §33/§38's own example phrasing — one fixed line per outcome, not
/// fabricated per-battle data (doc §25 "rien n'est inventé"): `galactic_sim`
/// exposes no "what actually happened narratively" field to draw from.
fn combat_outcome_flavor_line(outcome: CombatOutcome) -> &'static str {
    match outcome {
        CombatOutcome::AttackerVictory => "Défenses orbitales neutralisées.",
        CombatOutcome::DefenderVictory => "Flotte repoussée.",
        CombatOutcome::Stalemate => "Aucune des deux flottes n'a pris l'avantage.",
        CombatOutcome::MutualDestruction => "Aucune des deux flottes n'a survécu.",
        CombatOutcome::Retreat {
            retreating_side: RetreatingSide::Attacker,
        } => "Flotte extraite avant l'anéantissement.",
        CombatOutcome::Retreat {
            retreating_side: RetreatingSide::Defender,
        } => "L'ennemi s'est replié.",
    }
}

/// COMBAT-UX-001-J §33's "VOTRE FLOTTE" column.
fn combat_fleet_summary_lines(report: &CombatReport, resolution: &CombatResolution) -> String {
    let engaged: u64 = report
        .attacker
        .ships
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let survivors: u64 = resolution
        .attacker_survivors
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let losses: u64 = resolution
        .attacker_losses
        .iter()
        .map(|loss| loss.quantity)
        .sum();
    if losses > 0 {
        format!("{engaged} engagés\n{survivors} survivants\n{losses} perte(s)")
    } else {
        format!("{engaged} engagés\n{survivors} survivants")
    }
}

/// COMBAT-UX-001-J §33's "ENNEMI" column.
fn combat_enemy_summary_lines(report: &CombatReport, resolution: &CombatResolution) -> String {
    let engaged: u32 = report
        .defender
        .forces
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let survivors: u32 = resolution
        .defender_survivors
        .iter()
        .map(|stack| stack.quantity)
        .sum();
    let losses: u32 = resolution
        .defender_losses
        .iter()
        .map(|loss| loss.quantity)
        .sum();
    format!("{engaged} détectés\n{survivors} restants\n{losses} neutralisé(s)")
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
    (&'static mut Visibility, &'static mut Node),
    (With<CombatPreReportControls>, Without<CombatReturnRow>),
>;

type CombatReturnRowQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Node),
    (With<CombatReturnRow>, Without<CombatPreReportControls>),
>;

/// See `update_combat_briefing_visibility`'s doc comment: `Visibility::Hidden`
/// alone doesn't free flex layout space, so the hidden side of this
/// mutually-exclusive pair also needs `Display::None` or it keeps reserving
/// room in the content column for the briefing/planning card above it.
///
/// Retreat/AutoResolve/Validate aren't meaningful before a plan even exists
/// (doc §11.3 — the briefing's only CTA is "PRÉPARER L'ASSAUT"), so this also
/// hides them during `Briefing`, not just `FinalReport`.
fn update_final_report_visibility(
    ui: Res<CombatUiState>,
    mut controls: CombatPreReportControlsQuery,
    mut return_row: CombatReturnRowQuery,
) {
    let in_final_report = ui.phase == CombatUiPhase::FinalReport;
    let controls_shown = !in_final_report && ui.phase != CombatUiPhase::Briefing;
    let controls_visibility = if controls_shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let controls_display = if controls_shown {
        Display::Flex
    } else {
        Display::None
    };
    for (mut visibility, mut node) in &mut controls {
        if *visibility != controls_visibility {
            *visibility = controls_visibility;
        }
        if node.display != controls_display {
            node.display = controls_display;
        }
    }
    let return_visibility = if in_final_report {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let return_display = if in_final_report {
        Display::Flex
    } else {
        Display::None
    };
    for (mut visibility, mut node) in &mut return_row {
        if *visibility != return_visibility {
            *visibility = return_visibility;
        }
        if node.display != return_display {
            node.display = return_display;
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
            "Tactiques avancées [-]".to_string()
        } else {
            "Tactiques avancées [+]".to_string()
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
    (&'static mut Visibility, &'static mut Node),
    (
        With<CombatBriefingControls>,
        Without<CombatHiddenDuringBriefing>,
        Without<CombatRoundLogPanelRoot>,
    ),
>;

type CombatHiddenDuringBriefingQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Node),
    (
        With<CombatHiddenDuringBriefing>,
        Without<CombatBriefingControls>,
        Without<CombatRoundLogPanelRoot>,
    ),
>;

type CombatRoundLogPanelRootQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Node),
    (
        With<CombatRoundLogPanelRoot>,
        Without<CombatBriefingControls>,
        Without<CombatHiddenDuringBriefing>,
    ),
>;

/// `Visibility::Hidden` alone doesn't free flex layout space in Bevy UI — a
/// hidden sibling still claims its `flex_grow` share. The body row and the
/// briefing card both have `flex_grow: 1.0` in the same column, so without
/// also setting `Display::None` on the hidden one, the visible one gets
/// squeezed to half the available height instead of all of it.
fn update_combat_briefing_visibility(
    ui: Res<CombatUiState>,
    mut briefing: CombatBriefingControlsQuery,
    mut hidden_during_briefing: CombatHiddenDuringBriefingQuery,
    mut round_log_panel: CombatRoundLogPanelRootQuery,
) {
    let in_briefing = ui.phase == CombatUiPhase::Briefing;
    let briefing_visibility = if in_briefing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let briefing_display = if in_briefing {
        Display::Flex
    } else {
        Display::None
    };
    for (mut visibility, mut node) in &mut briefing {
        if *visibility != briefing_visibility {
            *visibility = briefing_visibility;
        }
        if node.display != briefing_display {
            node.display = briefing_display;
        }
    }
    // The tactical body (3-column row + intel bar) belongs to planning/round
    // phases only — it must also disappear during `FinalReport`, not just
    // `Briefing`, or the empty "CARTE TACTIQUE" frame and the intel bar text
    // are left floating above the victory/defeat screen (the battlefield
    // panel's own content already hides itself for `FinalReport`, but its
    // outer frame and heading don't, and `CombatPlanPanelRoot`'s columns are
    // only `Visibility::Hidden`, not `Display::None`, so without this the
    // whole row still reserves its flex space too).
    let tactical_body_visible = matches!(
        ui.phase,
        CombatUiPhase::AwaitingDoctrine | CombatUiPhase::RoundPause
    );
    let rest_visibility = if tactical_body_visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let rest_display = if tactical_body_visible {
        Display::Flex
    } else {
        Display::None
    };
    for (mut visibility, mut node) in &mut hidden_during_briefing {
        if *visibility != rest_visibility {
            *visibility = rest_visibility;
        }
        if node.display != rest_display {
            node.display = rest_display;
        }
    }
    // Unlike the tactical body row, the round-log panel stays visible during
    // `FinalReport` — it doubles as the detailed-report dump there. But only
    // once "Rapport détaillé" is actually toggled on: `update_final_report`
    // clears its text otherwise, and an empty bordered box sitting under
    // the header reads as a rendering glitch, not an intentional panel.
    let round_log_visible =
        !in_briefing && (ui.phase != CombatUiPhase::FinalReport || ui.showing_detailed_report);
    let round_log_visibility = if round_log_visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let round_log_display = if round_log_visible {
        Display::Flex
    } else {
        Display::None
    };
    for (mut visibility, mut node) in &mut round_log_panel {
        if *visibility != round_log_visibility {
            *visibility = round_log_visibility;
        }
        if node.display != round_log_display {
            node.display = round_log_display;
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
    fn combat_outcome_helpers_report_real_counts_and_decisive_round() {
        let report = resolved_combat_report_fixture();
        let CombatReportStatus::Resolved(resolution) = &report.status else {
            unreachable!()
        };

        assert_eq!(combat_outcome_banner_label(resolution.outcome), "VICTOIRE");
        assert_eq!(
            combat_fleet_summary_lines(&report, resolution),
            "2 engagés\n1 survivants\n1 perte(s)"
        );
        assert_eq!(
            combat_enemy_summary_lines(&report, resolution),
            "6 détectés\n2 restants\n4 neutralisé(s)"
        );
        // The destruction happens in round 1, which is also the heavier
        // round — both heuristics agree, so it must be picked.
        assert_eq!(
            combat_decisive_moment_line(&report.round_history).trim_start(),
            "MOMENT DÉCISIF — Round 1."
        );
    }

    /// COMBAT-UX-001-J §33/§35/§36: a real end-to-end run (not just the
    /// pure-function checks above) — the visibility wiring across
    /// `update_final_report`/`update_final_report_cards` is exactly what a
    /// pure-function test can't catch.
    #[test]
    fn final_report_shows_banner_loss_cards_and_loot_cards_for_a_real_victory() {
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
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (
                    update_final_report,
                    update_final_report_cards,
                    update_combat_briefing_visibility,
                )
                    .chain(),
            );
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

        app.update();

        let world = app.world_mut();
        let mut banner =
            world.query_filtered::<(&Text, &TextColor), With<CombatResultBannerText>>();
        let (banner_text, banner_color) = banner.single(world).unwrap();
        assert_eq!(banner_text.0, "VICTOIRE");
        assert_eq!(
            banner_color.0,
            combat_outcome_banner_color(CombatOutcome::AttackerVictory)
        );

        // Real bug found from a playtest screenshot: an empty bordered box
        // was left sitting under the header whenever "Rapport détaillé"
        // hadn't been toggled on yet.
        let mut round_log_panel =
            world.query_filtered::<&Visibility, With<CombatRoundLogPanelRoot>>();
        assert_eq!(*round_log_panel.single(world).unwrap(), Visibility::Hidden);

        let mut loss_rows = world.query::<(&CombatLossCardRow, &Visibility)>();
        let visible_losses: Vec<_> = loss_rows
            .iter(world)
            .filter(|(_, visibility)| **visibility == Visibility::Inherited)
            .collect();
        assert_eq!(visible_losses.len(), 1, "exactly one lost ship type");

        let mut loss_texts = world.query::<(&CombatLossCardText, &Text)>();
        assert!(
            loss_texts
                .iter(world)
                .any(|(_, text)| text.0.contains("1 perdu"))
        );

        let mut loot_root = world.query_filtered::<&Visibility, With<CombatLootCardsRoot>>();
        assert_eq!(*loot_root.single(world).unwrap(), Visibility::Inherited);
        let mut loot_empty = world.query_filtered::<&Visibility, With<CombatLootEmptyText>>();
        assert_eq!(*loot_empty.single(world).unwrap(), Visibility::Hidden);

        let mut loot_texts = world.query::<(&CombatLootCardText, &Text)>();
        let metal = loot_texts
            .iter(world)
            .find(|(marker, _)| marker.0 == ResourceHudKind::Metal)
            .unwrap()
            .1;
        assert_eq!(metal.0, "+7");
    }

    #[test]
    fn final_report_hides_loss_and_loot_cards_when_there_are_none() {
        let mut simulation = simulation_with_pending_combat();
        let mut report = resolved_combat_report_fixture();
        let CombatReportStatus::Resolved(resolution) = &mut report.status else {
            unreachable!()
        };
        resolution.attacker_losses.clear();
        resolution.salvage_recovered = galactic_domain::ResourceStock::ZERO;
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
                ..Default::default()
            })
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(
                bevy::app::Update,
                (update_final_report, update_final_report_cards).chain(),
            );
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

        app.update();

        let world = app.world_mut();
        let mut loss_rows = world.query::<(&CombatLossCardRow, &Visibility)>();
        assert!(
            loss_rows
                .iter(world)
                .all(|(_, visibility)| *visibility == Visibility::Hidden)
        );
        let mut loot_root = world.query_filtered::<&Visibility, With<CombatLootCardsRoot>>();
        assert_eq!(*loot_root.single(world).unwrap(), Visibility::Hidden);
        let mut loot_empty = world.query_filtered::<&Visibility, With<CombatLootEmptyText>>();
        assert_eq!(*loot_empty.single(world).unwrap(), Visibility::Inherited);
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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
            matches!(button.0, group_panel::DraftAction::SetRole(_)) && background.0 == disabled
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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

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
    /// label ([+]/[-]) — a real end-to-end run, not just a compile check.
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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

        app.update();
        {
            let world = app.world_mut();
            let mut row = world.query_filtered::<&Visibility, With<AdvancedDoctrinesRow>>();
            assert_eq!(row.single(world).unwrap(), Visibility::Hidden);
            let mut label = world.query_filtered::<&Text, With<AdvancedDoctrinesToggleLabel>>();
            assert_eq!(label.single(world).unwrap().0, "Tactiques avancées [+]");
        }

        app.world_mut()
            .resource_mut::<CombatUiState>()
            .advanced_doctrines_expanded = true;
        app.update();
        let world = app.world_mut();
        let mut row = world.query_filtered::<&Visibility, With<AdvancedDoctrinesRow>>();
        assert_eq!(row.single(world).unwrap(), Visibility::Inherited);
        let mut label = world.query_filtered::<&Text, With<AdvancedDoctrinesToggleLabel>>();
        assert_eq!(label.single(world).unwrap().0, "Tactiques avancées [-]");
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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

        app.update();

        let world = app.world_mut();
        // COMBAT-UX-001-J §29: "visible uniquement après le premier round" —
        // at round 0 the PC widget stays hidden entirely, not shown with a
        // placeholder sentence.
        let mut pc_root = world.query_filtered::<&Visibility, With<CombatCommandPointsRoot>>();
        assert_eq!(*pc_root.single(world).unwrap(), Visibility::Hidden);

        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("Lancer l'assaut"));
    }

    /// COMBAT-UX-001-J §29: once a round has resolved, the PC widget becomes
    /// visible and shows the header/meter/count as 3 distinct lines.
    #[test]
    fn command_panel_shows_pc_widget_after_round_one() {
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
            .add_systems(bevy::app::Update, update_command_panel);
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

        app.update();

        let world = app.world_mut();
        let mut pc_root = world.query_filtered::<&Visibility, With<CombatCommandPointsRoot>>();
        assert_eq!(*pc_root.single(world).unwrap(), Visibility::Inherited);

        let mut meter = world.query_filtered::<&Text, With<CombatCommandPointsMeterText>>();
        assert!(meter.single(world).unwrap().0.contains('#'));

        let mut available = world.query_filtered::<&Text, With<CombatCommandPointsAvailableText>>();
        assert!(available.single(world).unwrap().0.contains("disponible"));

        let mut texts = world.query::<&Text>();
        let rendered_text = texts
            .iter(world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered_text.contains("POINTS DE COMMANDEMENT"));
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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
                (
                    update_final_report,
                    update_final_report_cards,
                    update_report_tabs_visibility,
                    update_combat_briefing_visibility,
                )
                    .chain(),
            );
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
            // Real regression: the detailed-report dump above is worthless
            // if its container is hidden — `CombatRoundLogPanelRoot` must
            // stay visible during `FinalReport`, unlike the tactical body
            // row it used to live inside.
            let mut panel = world.query_filtered::<&Visibility, With<CombatRoundLogPanelRoot>>();
            assert_eq!(panel.single(world).unwrap(), Visibility::Inherited);
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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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
        assert!(rendered_text.contains("Round"));
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
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

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

    /// COMBAT-UX-001-J §27: rows that actually dealt or took damage this
    /// round get a `UiTransform` scale pulse — a real end-to-end run (not a
    /// pure-function check on `round_scale_pulse` alone) confirming the
    /// query wiring in `update_battlefield_panel`'s row loop is correct.
    #[test]
    fn battlefield_panel_applies_round_animation_transform_to_rows_that_acted() {
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

        let mut ui_state = CombatUiState {
            current: Some(mission_id),
            phase: CombatUiPhase::RoundPause,
            ..Default::default()
        };
        // Land inside the impact window — see `round_scale_pulse`.
        ui_state.round_pause_timer.tick(Duration::from_millis(400));

        let mut app = bevy::app::App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<crate::presentation::icons::IconAssets>()
            .init_resource::<group_panel::CombatPlanDraftState>()
            .insert_resource(SimulationResource {
                simulation,
                pending_events: Vec::new(),
            })
            .insert_resource(ui_state)
            .add_systems(bevy::app::Startup, spawn_combat_screen)
            .add_systems(bevy::app::Update, battlefield::update_battlefield_panel);
        let (entity_visuals, resource_icons) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (
                EntityVisualCatalog::for_tests(&mut images),
                ResourceIconAssets::for_tests(&mut images),
            )
        };
        app.insert_resource(entity_visuals);
        app.insert_resource(resource_icons);

        app.update();

        let world = app.world_mut();
        let mut rows = world.query::<(&battlefield::BattlefieldRow, &UiTransform)>();
        assert!(
            rows.iter(world)
                .any(|(_, transform)| transform.scale != Vec2::ONE),
            "at least one row that took part in the resolved round should show the scale pulse"
        );
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
        let resource_icons = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            ResourceIconAssets::for_tests(&mut images)
        };
        app.insert_resource(resource_icons);

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

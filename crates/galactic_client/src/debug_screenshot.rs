// Dev-only automated screenshot: captures the primary window's own render
// output to a PNG and exits. This is deliberately NOT an OS-level screen
// capture tool (spectacle/import/etc) — Bevy's `Screenshot` API reads back
// only its own GPU render target, so it structurally cannot capture the
// desktop or any other application, unlike the OS tool that caused a prior
// incident (see project memory `feedback_visual_verification`). Used for the
// COMBAT-UX-001-J visual-polish contract's verification loop.
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

#[derive(Resource)]
pub(crate) struct AutoScreenshotState {
    pub(crate) path: PathBuf,
    pub(crate) timer: Timer,
}

impl AutoScreenshotState {
    pub(crate) fn new(path: PathBuf, delay_secs: f32) -> Self {
        Self {
            path,
            timer: Timer::from_seconds(delay_secs, TimerMode::Once),
        }
    }
}

pub(crate) fn tick_auto_screenshot(
    time: Res<Time>,
    mut commands: Commands,
    state: Option<ResMut<AutoScreenshotState>>,
) {
    let Some(mut state) = state else {
        return;
    };
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(state.path.clone()))
        .observe(exit_after_screenshot);
}

fn exit_after_screenshot(_captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>) {
    exit.write(AppExit::Success);
}

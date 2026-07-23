mod app;
mod camera;
mod data;
mod diagnostics;
mod generation;
mod interaction;
mod map;
mod navigation;
mod rendering;
mod state;
mod strategic;
mod ui;
mod usability;
mod views;

use app::AppPlugin;
use bevy::prelude::*;

fn main() {
    App::new().add_plugins(AppPlugin).run();
}

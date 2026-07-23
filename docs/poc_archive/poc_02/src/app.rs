use bevy::prelude::*;
use bevy::window::PresentMode;

use crate::camera::CameraControlPlugin;
use crate::data::DataPlugin;
use crate::diagnostics::GalacticDiagnosticsPlugin;
use crate::generation::GalaxyGenerationPlugin;
use crate::interaction::InteractionPlugin;
use crate::map::MapPlugin;
use crate::navigation::NavigationPlugin;
use crate::rendering::RenderingPlugin;
use crate::state::ViewState;
use crate::strategic::StrategicMapPlugin;
use crate::ui::GalacticUiPlugin;
use crate::usability::UsabilityPlugin;
use crate::views::{GalaxyViewPlugin, SystemViewPlugin};

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Galactic POC".to_string(),
                resolution: (1600, 900).into(),
                resizable: true,
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.005, 0.007, 0.014)))
        .init_state::<ViewState>()
        .add_plugins((
            DataPlugin,
            GalaxyGenerationPlugin,
            StrategicMapPlugin,
            MapPlugin,
            NavigationPlugin,
            RenderingPlugin,
            CameraControlPlugin,
            InteractionPlugin,
            GalaxyViewPlugin,
            SystemViewPlugin,
            GalacticUiPlugin,
            GalacticDiagnosticsPlugin,
            UsabilityPlugin,
        ))
        .add_systems(Startup, log_startup);
    }
}

fn log_startup() {
    info!("Galactic POC starting on Bevy 0.19");
}

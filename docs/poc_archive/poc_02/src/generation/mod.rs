pub mod galaxy;
pub mod names;
pub mod routes;
pub mod system;

use bevy::prelude::*;
use std::time::Instant;

use crate::data::{GalaxyConfig, GalaxyData};

pub struct GalaxyGenerationPlugin;

impl Plugin for GalaxyGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GalaxyData>();
    }
}

impl FromWorld for GalaxyData {
    fn from_world(world: &mut World) -> Self {
        let config = world
            .get_resource::<GalaxyConfig>()
            .cloned()
            .unwrap_or_default()
            .sanitized();
        let started = Instant::now();
        let galaxy = galaxy::generate_galaxy(&config);
        info!(
            "generated galaxy seed={} systems={} routes={} in {:?}",
            galaxy.seed,
            galaxy.systems.len(),
            galaxy.routes.len(),
            started.elapsed()
        );
        galaxy
    }
}

pub mod alerts;
pub mod control;
pub mod factions;
pub mod fleets;
pub mod generation;
pub mod mission;
pub mod routes;
pub mod sectors;

use bevy::prelude::*;

use crate::data::{GalaxyConfig, GalaxyData};

pub use alerts::*;
pub use control::*;
pub use factions::*;
pub use fleets::*;
pub use generation::*;
pub use mission::*;
pub use routes::*;
pub use sectors::*;

pub struct StrategicMapPlugin;

impl Plugin for StrategicMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StrategicGalaxyData>()
            .add_systems(Update, regenerate_strategic_when_galaxy_changes);
    }
}

impl FromWorld for StrategicGalaxyData {
    fn from_world(world: &mut World) -> Self {
        let galaxy = world.resource::<GalaxyData>();
        let config = world.resource::<GalaxyConfig>();
        generation::generate_strategic_galaxy(galaxy, config)
    }
}

fn regenerate_strategic_when_galaxy_changes(
    galaxy: Res<GalaxyData>,
    config: Res<GalaxyConfig>,
    mut strategic: ResMut<StrategicGalaxyData>,
) {
    if galaxy.is_added() || !galaxy.is_changed() {
        return;
    }
    *strategic = generation::generate_strategic_galaxy(&galaxy, &config);
    info!(
        "generated strategic layer factions={} sectors={} alerts={} fleets={} target={:?}",
        strategic.factions.len(),
        strategic.sectors.len(),
        strategic.alerts.len(),
        strategic.fleets.len(),
        strategic.validation_target
    );
}

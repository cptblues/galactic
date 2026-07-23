pub mod materials;
pub mod orbits;
pub mod overlays;
pub mod routes;
pub mod starfield;

use bevy::prelude::*;

pub use materials::*;

use crate::state::ViewState;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisualAssets>()
            .add_systems(Startup, starfield::spawn_starfield)
            .add_systems(
                Update,
                (
                    routes::draw_galaxy_routes,
                    overlays::draw_territory_halos,
                    overlays::draw_map_markers,
                    overlays::draw_fleets,
                )
                    .run_if(in_state(ViewState::Galaxy)),
            )
            .add_systems(
                Update,
                orbits::draw_system_orbits.run_if(in_state(ViewState::System)),
            );
    }
}

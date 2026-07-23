pub mod galaxy;
pub mod system;

use bevy::prelude::*;

use crate::state::ViewState;

pub struct GalaxyViewPlugin;

impl Plugin for GalaxyViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ViewState::Galaxy), galaxy::spawn_galaxy_view)
            .add_systems(OnExit(ViewState::Galaxy), galaxy::cleanup_galaxy_view)
            .add_systems(
                Update,
                galaxy::respawn_galaxy_when_changed.run_if(in_state(ViewState::Galaxy)),
            );
    }
}

pub struct SystemViewPlugin;

impl Plugin for SystemViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ViewState::System), system::spawn_system_view)
            .add_systems(OnExit(ViewState::System), system::cleanup_system_view)
            .add_systems(
                Update,
                system::animate_system_bodies.run_if(in_state(ViewState::System)),
            );
    }
}

#[derive(Component)]
pub struct GalaxyViewEntity;

#[derive(Component)]
pub struct SystemViewEntity;

#[derive(Component)]
pub struct StarSystemVisual;

#[derive(Component)]
pub struct StarVisual;

#[derive(Component)]
pub struct PlanetVisual {
    pub id: crate::data::PlanetId,
}

#[derive(Component)]
pub struct MoonVisual {
    pub planet_id: crate::data::PlanetId,
}

#[derive(Component)]
pub struct OrbitMotion {
    pub radius: f32,
    pub speed: f32,
    pub phase: f32,
    pub inclination: f32,
}

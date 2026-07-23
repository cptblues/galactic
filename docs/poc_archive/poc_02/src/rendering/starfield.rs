use bevy::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::f32::consts::TAU;

use crate::rendering::VisualAssets;

#[derive(Component)]
pub struct StarfieldEntity;

pub fn spawn_starfield(mut commands: Commands, assets: Res<VisualAssets>) {
    let mut rng = ChaCha8Rng::seed_from_u64(0x51A7_F1E1D);
    for _ in 0..1800 {
        let y = rng.random_range(-1.0_f32..1.0);
        let angle = rng.random_range(0.0..TAU);
        let ring = (1.0 - y * y).sqrt();
        let radius = rng.random_range(520.0..680.0);
        let position = Vec3::new(ring * angle.cos(), y, ring * angle.sin()) * radius;
        let scale = rng.random_range(0.035..0.115);
        commands.spawn((
            Mesh3d(assets.starfield_mesh.clone()),
            MeshMaterial3d(assets.starfield_material.clone()),
            Transform::from_translation(position).with_scale(Vec3::splat(scale)),
            Pickable::IGNORE,
            StarfieldEntity,
        ));
    }
}

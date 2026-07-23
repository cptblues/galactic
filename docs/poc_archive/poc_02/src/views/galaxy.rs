use bevy::prelude::*;

use crate::data::{GalaxyData, SelectableId};
use crate::interaction::{Selectable, selectable_click, selectable_out, selectable_over};
use crate::map::GalaxyMapPosition;
use crate::rendering::{BaseScale, VisualAssets, VisualMaterialSet};
use crate::views::{GalaxyViewEntity, StarSystemVisual};

#[derive(Component)]
pub struct GalaxyLabel {
    pub id: crate::data::SystemId,
    pub name: String,
    pub was_visible: bool,
}

pub fn spawn_galaxy_view(
    mut commands: Commands,
    galaxy: Res<GalaxyData>,
    assets: Res<VisualAssets>,
    existing: Query<Entity, With<GalaxyViewEntity>>,
) {
    despawn_entities(&mut commands, &existing);
    spawn_galaxy_entities(&mut commands, &galaxy, &assets);
    info!(
        "spawned galaxy view systems={} routes={}",
        galaxy.systems.len(),
        galaxy.routes.len()
    );
}

pub fn cleanup_galaxy_view(mut commands: Commands, query: Query<Entity, With<GalaxyViewEntity>>) {
    despawn_entities(&mut commands, &query);
}

pub fn respawn_galaxy_when_changed(
    mut commands: Commands,
    galaxy: Res<GalaxyData>,
    assets: Res<VisualAssets>,
    query: Query<Entity, With<GalaxyViewEntity>>,
) {
    if !galaxy.is_changed() || galaxy.is_added() {
        return;
    }
    despawn_entities(&mut commands, &query);
    spawn_galaxy_entities(&mut commands, &galaxy, &assets);
}

fn spawn_galaxy_entities(commands: &mut Commands, galaxy: &GalaxyData, assets: &VisualAssets) {
    for system in &galaxy.systems {
        let scale = Vec3::splat((0.34 + system.star.visual_radius * 0.13).min(0.7));
        let normal = assets
            .star_materials
            .get(&system.star.class)
            .expect("star class material exists")
            .clone();
        commands
            .spawn((
                Mesh3d(assets.system_mesh.clone()),
                MeshMaterial3d(normal.clone()),
                Transform::from_translation(system.position).with_scale(scale),
                GalaxyMapPosition {
                    original: system.position,
                },
                StarSystemVisual,
                Selectable {
                    id: SelectableId::System(system.id),
                },
                VisualMaterialSet::new(normal, assets),
                BaseScale(scale),
                GalaxyViewEntity,
            ))
            .observe(selectable_over)
            .observe(selectable_out)
            .observe(selectable_click);

        let label_position = system.position + Vec3::new(0.0, 1.35, 0.0);
        commands.spawn((
            Text2d::new(system.name.clone()),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgba(0.76, 0.9, 1.0, 0.82)),
            Transform::from_translation(label_position).with_scale(Vec3::splat(0.24)),
            GalaxyMapPosition {
                original: label_position,
            },
            Visibility::Hidden,
            GalaxyLabel {
                id: system.id,
                name: system.name.clone(),
                was_visible: false,
            },
            GalaxyViewEntity,
        ));
    }
}

fn despawn_entities(commands: &mut Commands, query: &Query<Entity, With<GalaxyViewEntity>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

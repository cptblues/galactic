use bevy::prelude::*;
use std::collections::HashMap;

use crate::data::{PlanetKind, StarClass, planet_color, star_color};

#[derive(Resource, Clone)]
pub struct VisualAssets {
    pub star_mesh: Handle<Mesh>,
    pub planet_mesh: Handle<Mesh>,
    pub moon_mesh: Handle<Mesh>,
    pub system_mesh: Handle<Mesh>,
    pub asteroid_mesh: Handle<Mesh>,
    pub starfield_mesh: Handle<Mesh>,
    pub star_materials: HashMap<StarClass, Handle<StandardMaterial>>,
    pub planet_materials: HashMap<PlanetKind, Handle<StandardMaterial>>,
    pub moon_material: Handle<StandardMaterial>,
    pub asteroid_material: Handle<StandardMaterial>,
    pub starfield_material: Handle<StandardMaterial>,
    pub hover_material: Handle<StandardMaterial>,
    pub selected_material: Handle<StandardMaterial>,
}

impl FromWorld for VisualAssets {
    fn from_world(world: &mut World) -> Self {
        let (star_mesh, planet_mesh, moon_mesh, system_mesh, asteroid_mesh, starfield_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Sphere::default().mesh().ico(5).unwrap()),
                meshes.add(Sphere::default().mesh().uv(32, 18)),
                meshes.add(Sphere::default().mesh().ico(3).unwrap()),
                meshes.add(Sphere::default().mesh().ico(3).unwrap()),
                meshes.add(Cuboid::new(0.55, 0.32, 0.38)),
                meshes.add(Sphere::default().mesh().ico(1).unwrap()),
            )
        };

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

        let star_materials = StarClass::ALL
            .into_iter()
            .map(|class| (class, materials.add(star_material(class))))
            .collect();
        let planet_materials = PlanetKind::ALL
            .into_iter()
            .map(|kind| (kind, materials.add(planet_material(kind))))
            .collect();

        let moon_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.76, 0.8),
            perceptual_roughness: 0.86,
            ..default()
        });
        let asteroid_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.34, 0.31, 0.28),
            perceptual_roughness: 0.92,
            ..default()
        });
        let starfield_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.8, 1.0),
            emissive: LinearRgba::rgb(0.8, 0.9, 1.4),
            unlit: true,
            ..default()
        });
        let hover_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.92, 1.0),
            emissive: LinearRgba::rgb(1.6, 2.6, 3.8),
            unlit: true,
            ..default()
        });
        let selected_material = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.94, 0.42),
            emissive: LinearRgba::rgb(4.5, 3.8, 0.9),
            unlit: true,
            ..default()
        });

        Self {
            star_mesh,
            planet_mesh,
            moon_mesh,
            system_mesh,
            asteroid_mesh,
            starfield_mesh,
            star_materials,
            planet_materials,
            moon_material,
            asteroid_material,
            starfield_material,
            hover_material,
            selected_material,
        }
    }
}

fn star_material(class: StarClass) -> StandardMaterial {
    let color = star_color(class);
    let emissive = match class {
        StarClass::Blue => LinearRgba::rgb(3.0, 5.5, 11.0),
        StarClass::White => LinearRgba::rgb(6.0, 6.3, 7.0),
        StarClass::Yellow => LinearRgba::rgb(6.5, 4.8, 1.7),
        StarClass::Orange => LinearRgba::rgb(6.0, 2.4, 0.8),
        StarClass::Red => LinearRgba::rgb(4.8, 0.7, 0.55),
    };
    StandardMaterial {
        base_color: color,
        emissive,
        unlit: true,
        ..default()
    }
}

fn planet_material(kind: PlanetKind) -> StandardMaterial {
    StandardMaterial {
        base_color: planet_color(kind),
        perceptual_roughness: match kind {
            PlanetKind::GasGiant => 0.68,
            PlanetKind::Ocean => 0.42,
            _ => 0.82,
        },
        metallic: 0.0,
        ..default()
    }
}

#[derive(Component, Clone)]
pub struct VisualMaterialSet {
    pub normal: Handle<StandardMaterial>,
    pub hovered: Handle<StandardMaterial>,
    pub selected: Handle<StandardMaterial>,
}

impl VisualMaterialSet {
    pub fn new(normal: Handle<StandardMaterial>, assets: &VisualAssets) -> Self {
        Self {
            normal,
            hovered: assets.hover_material.clone(),
            selected: assets.selected_material.clone(),
        }
    }
}

#[derive(Component, Clone, Copy)]
pub struct BaseScale(pub Vec3);

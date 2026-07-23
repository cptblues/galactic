use bevy::prelude::*;

use crate::data::ids::{MoonId, PlanetId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StarClass {
    Blue,
    White,
    Yellow,
    Orange,
    Red,
}

impl StarClass {
    pub const ALL: [Self; 5] = [
        Self::Blue,
        Self::White,
        Self::Yellow,
        Self::Orange,
        Self::Red,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blue => "Bleue",
            Self::White => "Blanche",
            Self::Yellow => "Jaune",
            Self::Orange => "Orange",
            Self::Red => "Rouge",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PlanetKind {
    Rocky,
    Desert,
    Ocean,
    Ice,
    Volcanic,
    GasGiant,
}

impl PlanetKind {
    pub const ALL: [Self; 6] = [
        Self::Rocky,
        Self::Desert,
        Self::Ocean,
        Self::Ice,
        Self::Volcanic,
        Self::GasGiant,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rocky => "Rocheuse",
            Self::Desert => "Desertique",
            Self::Ocean => "Oceanique",
            Self::Ice => "Glacee",
            Self::Volcanic => "Volcanique",
            Self::GasGiant => "Geante gazeuse",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StarData {
    pub class: StarClass,
    pub visual_radius: f32,
    pub luminosity: f32,
    pub temperature_kelvin: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanetData {
    pub id: PlanetId,
    pub name: String,
    pub kind: PlanetKind,
    pub visual_radius: f32,
    pub orbit_radius: f32,
    pub orbit_speed: f32,
    pub orbit_phase: f32,
    pub orbit_inclination: f32,
    pub habitability: u8,
    pub moons: Vec<MoonData>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoonData {
    pub id: MoonId,
    pub name: String,
    pub visual_radius: f32,
    pub orbit_radius: f32,
    pub orbit_speed: f32,
    pub orbit_phase: f32,
    pub orbit_inclination: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AsteroidBeltData {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub asteroid_count: usize,
}

pub fn star_color(class: StarClass) -> Color {
    match class {
        StarClass::Blue => Color::srgb(0.45, 0.66, 1.0),
        StarClass::White => Color::srgb(0.92, 0.96, 1.0),
        StarClass::Yellow => Color::srgb(1.0, 0.88, 0.45),
        StarClass::Orange => Color::srgb(1.0, 0.48, 0.18),
        StarClass::Red => Color::srgb(1.0, 0.22, 0.18),
    }
}

pub fn planet_color(kind: PlanetKind) -> Color {
    match kind {
        PlanetKind::Rocky => Color::srgb(0.58, 0.54, 0.48),
        PlanetKind::Desert => Color::srgb(0.9, 0.66, 0.32),
        PlanetKind::Ocean => Color::srgb(0.1, 0.44, 0.9),
        PlanetKind::Ice => Color::srgb(0.72, 0.9, 1.0),
        PlanetKind::Volcanic => Color::srgb(0.88, 0.18, 0.08),
        PlanetKind::GasGiant => Color::srgb(0.74, 0.56, 0.92),
    }
}

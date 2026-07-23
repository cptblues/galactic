use crate::data::{FactionId, SystemId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FactionDisposition {
    Player,
    Allied,
    Neutral,
    Rival,
    Hostile,
}

impl FactionDisposition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Player => "Joueur",
            Self::Allied => "Allie",
            Self::Neutral => "Neutre",
            Self::Rival => "Rival",
            Self::Hostile => "Hostile",
        }
    }

    pub fn is_friendly(self) -> bool {
        matches!(self, Self::Player | Self::Allied)
    }

    pub fn is_hostile(self) -> bool {
        matches!(self, Self::Hostile | Self::Rival)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FactionColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl FactionColor {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FactionData {
    pub id: FactionId,
    pub name: String,
    pub capital: SystemId,
    pub disposition: FactionDisposition,
    pub ui_color: FactionColor,
    pub symbol: char,
}

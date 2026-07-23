use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ViewState {
    #[default]
    Galaxy,
    System,
}

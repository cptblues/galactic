use bevy::prelude::*;

use crate::camera::CameraPose;
use crate::state::ViewState;

#[derive(Resource, Debug)]
pub struct CameraTransition {
    pub active: bool,
    pub elapsed: f32,
    pub duration: f32,
    pub from: CameraPose,
    pub to: CameraPose,
    pub pending_view: Option<ViewState>,
}

impl Default for CameraTransition {
    fn default() -> Self {
        Self {
            active: false,
            elapsed: 0.0,
            duration: 1.0,
            from: CameraPose::default(),
            to: CameraPose::default(),
            pending_view: None,
        }
    }
}

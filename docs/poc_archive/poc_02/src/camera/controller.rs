use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub target_focus: Vec3,
    pub yaw: f32,
    pub target_yaw: f32,
    pub pitch: f32,
    pub target_pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rotate_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub smoothing: f32,
    pub input_enabled: bool,
}

impl OrbitCamera {
    pub fn galaxy() -> Self {
        Self {
            focus: Vec3::ZERO,
            target_focus: Vec3::ZERO,
            yaw: 0.0,
            target_yaw: 0.0,
            pitch: -0.55,
            target_pitch: -0.55,
            distance: 170.0,
            target_distance: 170.0,
            min_distance: 8.0,
            max_distance: 400.0,
            rotate_sensitivity: 0.005,
            pan_sensitivity: 0.00125,
            zoom_sensitivity: 0.12,
            smoothing: 12.0,
            input_enabled: true,
        }
    }

    pub fn set_bounds(&mut self, min_distance: f32, max_distance: f32) {
        self.min_distance = min_distance;
        self.max_distance = max_distance;
        self.target_distance = self.target_distance.clamp(min_distance, max_distance);
        self.distance = self.distance.clamp(min_distance, max_distance);
    }

    pub fn set_pose(&mut self, pose: CameraPose) {
        self.focus = pose.focus;
        self.target_focus = pose.focus;
        self.yaw = pose.yaw;
        self.target_yaw = pose.yaw;
        self.pitch = pose.pitch;
        self.target_pitch = pose.pitch;
        self.distance = pose.distance;
        self.target_distance = pose.distance;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraPose {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

pub mod body;
pub mod galaxy;
pub mod ids;
pub mod system;

use bevy::prelude::*;

pub use body::*;
pub use galaxy::*;
pub use ids::*;
pub use system::*;

pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GalaxyConfig>()
            .init_resource::<Selection>()
            .init_resource::<ViewOptions>()
            .init_resource::<ActiveSystem>()
            .init_resource::<OrbitAnimation>()
            .init_resource::<Notifications>();
    }
}

#[derive(Resource, Clone, Debug)]
pub struct GalaxyConfig {
    pub seed: u64,
    pub system_count: usize,
    pub arm_count: usize,
    pub radius: f32,
    pub thickness: f32,
    pub spiral_turns: f32,
    pub arm_spread: f32,
    pub min_system_distance: f32,
}

impl Default for GalaxyConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            system_count: 500,
            arm_count: 4,
            radius: 100.0,
            thickness: 8.0,
            spiral_turns: 2.2,
            arm_spread: 0.22,
            min_system_distance: 1.8,
        }
    }
}

impl GalaxyConfig {
    pub fn sanitized(&self) -> Self {
        Self {
            seed: self.seed,
            system_count: self.system_count.max(1),
            arm_count: self.arm_count.max(1),
            radius: self.radius.max(1.0),
            thickness: self.thickness.max(0.1),
            spiral_turns: self.spiral_turns.max(0.1),
            arm_spread: self.arm_spread.max(0.0),
            min_system_distance: self.min_system_distance.max(0.0),
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct Selection {
    pub hovered: Option<SelectableId>,
    pub selected: Option<SelectableId>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.hovered = None;
        self.selected = None;
    }

    pub fn selected_system(&self) -> Option<SystemId> {
        match self.selected {
            Some(SelectableId::System(id) | SelectableId::Star(id)) => Some(id),
            Some(SelectableId::Planet(system_id, _) | SelectableId::Moon(system_id, _, _)) => {
                Some(system_id)
            }
            None => None,
        }
    }
}

#[derive(Resource, Debug)]
pub struct ViewOptions {
    pub show_routes: bool,
    pub show_orbits: bool,
    pub show_labels: bool,
    pub show_debug: bool,
    pub show_help: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            show_routes: true,
            show_orbits: true,
            show_labels: true,
            show_debug: false,
            show_help: true,
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct ActiveSystem {
    pub id: Option<SystemId>,
}

#[derive(Resource, Debug)]
pub struct OrbitAnimation {
    pub paused: bool,
    pub elapsed: f32,
}

impl Default for OrbitAnimation {
    fn default() -> Self {
        Self {
            paused: false,
            elapsed: 0.0,
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct Notifications {
    pub message: Option<String>,
    pub remaining: f32,
}

impl Notifications {
    pub fn show(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
        self.remaining = 2.6;
    }
}

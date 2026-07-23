pub mod filters;
pub mod labels;
pub mod projection;
pub mod selection;
pub mod semantic_zoom;

use bevy::prelude::*;

use crate::state::ViewState;

pub use filters::*;
pub use labels::*;
pub use projection::*;
pub use selection::*;
pub use semantic_zoom::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapFilters>()
            .init_resource::<SemanticZoomState>()
            .init_resource::<MapProjectionState>()
            .init_resource::<LabelDiagnostics>()
            .init_resource::<LabelHighlight>()
            .init_resource::<AmbiguousSelection>()
            .init_resource::<GraphicsSettings>()
            .add_systems(
                Update,
                (
                    projection::toggle_projection,
                    filters::apply_system_visibility,
                    semantic_zoom::update_semantic_zoom,
                    projection::update_projection,
                    projection::apply_galaxy_projection,
                    labels::update_dynamic_labels,
                )
                    .chain()
                    .run_if(in_state(ViewState::Galaxy)),
            );
        app.add_systems(
            Update,
            (
                selection::screen_space_selection,
                selection::cycle_ambiguous_selection,
            )
                .run_if(in_state(ViewState::Galaxy)),
        );
    }
}

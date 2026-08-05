// MVP-034: current graphics preset, read by every preset-driven rendering
// system (bloom/HDR, shadows, particles, nebulae, labels, procedural texture
// quality). The preset type itself is canonical in `galactic_persistence`
// (not here) so it can derive Serialize/Deserialize and be persisted without
// a circular dependency back into this crate.
use bevy::prelude::Resource;
pub(crate) use galactic_persistence::GraphicsPreset;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct GraphicsSettings {
    pub(crate) preset: GraphicsPreset,
}

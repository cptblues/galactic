// MVP-034: client graphics preset, canonical here (not in galactic_client) so it
// can derive Serialize/Deserialize directly and share this crate's disk-access
// conventions, without a circular dependency back into the client crate.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsPreset {
    Low,
    #[default]
    Medium,
    High,
}

const SETTINGS_DIR_ENV: &str = "GALACTIC_SETTINGS_DIR";

/// Mirrors `default_save_directory`'s env-override-or-default convention, but
/// deliberately uses `dirs::config_dir()` rather than `dirs::data_dir()` —
/// this is app *preferences*, not save-game *data*, the first thing in this
/// crate to make that distinction.
pub fn default_settings_path() -> PathBuf {
    if let Ok(overridden) = std::env::var(SETTINGS_DIR_ENV) {
        return PathBuf::from(overridden).join("settings.ron");
    }
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("galactic")
        .join("settings.ron")
}

/// Externally-tagged by RON on the variant name — same self-dispatching
/// migration mechanism as `SaveFileEnvelope` in `file.rs`, kept even though
/// today's payload is a single field, for the same forward-compatibility
/// reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SettingsFileEnvelope {
    V1(SettingsFileEnvelopeV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsFileEnvelopeV1 {
    graphics_preset: GraphicsPreset,
}

/// Writes are atomic via a `.tmp` sibling + `fs::rename`, mirroring
/// `save_to_path` in `file.rs`.
pub fn save_settings(
    path: &std::path::Path,
    graphics_preset: GraphicsPreset,
) -> std::io::Result<()> {
    let envelope = SettingsFileEnvelope::V1(SettingsFileEnvelopeV1 { graphics_preset });
    let text = ron::ser::to_string_pretty(&envelope, ron::ser::PrettyConfig::default())
        .expect("SettingsFileEnvelope always serializes");

    let tmp_path = PathBuf::from(format!("{}.tmp", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&tmp_path, text)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Never surfaces an error to the caller: a missing, corrupted, or
/// version-mismatched settings file just means "use the default preset" —
/// there is nothing gameplay-critical here worth blocking startup over.
pub fn load_settings(path: &std::path::Path) -> GraphicsPreset {
    let Ok(text) = std::fs::read_to_string(path) else {
        return GraphicsPreset::default();
    };
    let Ok(envelope) = ron::de::from_str::<SettingsFileEnvelope>(&text) else {
        return GraphicsPreset::default();
    };
    match envelope {
        SettingsFileEnvelope::V1(SettingsFileEnvelopeV1 { graphics_preset }) => graphics_preset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_round_trips_the_chosen_preset() {
        let dir =
            std::env::temp_dir().join(format!("galactic-settings-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir is creatable");
        let path = dir.join("settings.ron");

        save_settings(&path, GraphicsPreset::High).expect("save succeeds");
        assert_eq!(load_settings(&path), GraphicsPreset::High);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loading_a_missing_file_defaults_to_medium() {
        let path = std::env::temp_dir().join(format!(
            "galactic-settings-test-missing-{}/settings.ron",
            std::process::id()
        ));

        assert_eq!(load_settings(&path), GraphicsPreset::Medium);
    }

    #[test]
    fn loading_a_corrupted_file_defaults_to_medium_instead_of_panicking() {
        let dir = std::env::temp_dir().join(format!(
            "galactic-settings-test-corrupted-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir is creatable");
        let path = dir.join("settings.ron");
        std::fs::write(&path, b"not ron at all").expect("write succeeds");

        assert_eq!(load_settings(&path), GraphicsPreset::Medium);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_settings_path_honors_the_env_override() {
        // SAFETY: test-only env var, no other test in this crate reads SETTINGS_DIR_ENV.
        unsafe {
            std::env::set_var(SETTINGS_DIR_ENV, "/tmp/galactic-settings-dir-override");
        }
        assert_eq!(
            default_settings_path(),
            PathBuf::from("/tmp/galactic-settings-dir-override/settings.ron")
        );
        unsafe {
            std::env::remove_var(SETTINGS_DIR_ENV);
        }
    }
}

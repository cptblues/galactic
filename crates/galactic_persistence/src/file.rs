// MVP-031: disk persistence for `SaveGame` — atomic writes, corrupted-file and
// incompatible-version handling, and a migration-ready envelope.
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::save::{SaveError, SaveGame};

const SAVE_FILE_EXTENSION: &str = "ron";

/// Presentation metadata that sits outside the versioned gameplay payload, so
/// gameplay-shape migrations never need to reason about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveFileHeader {
    pub display_name: String,
    pub saved_at_unix_seconds: u64,
}

/// Externally-tagged by RON on the variant name, so `ron::de::from_str`
/// self-dispatches to the shape matching the file's own `SAVE_VERSION` without
/// any manual "peek the version first" step. Adding a breaking migration later
/// means: freeze this variant's payload type under its own name, add a new
/// variant for the new shape, and write a `migrate_vNN_to_vMM` function that
/// the deserialize path calls for the old variant before returning the new
/// shape. An additive, backward-compatible field never needs a new variant at
/// all — `#[serde(default)]` on the new field is enough (see `file` module
/// tests for a demonstration of that mechanism).
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SaveFileEnvelope {
    V29(SaveFileEnvelopeV29),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaveFileEnvelopeV29 {
    header: SaveFileHeader,
    payload: SaveGame,
}

#[derive(Debug)]
pub enum SaveFileError {
    Io { path: PathBuf, message: String },
    Corrupted { path: PathBuf, message: String },
    Incompatible(SaveError),
}

impl fmt::Display for SaveFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "save file I/O error at {}: {message}",
                    path.display()
                )
            }
            Self::Corrupted { path, message } => {
                write!(
                    formatter,
                    "corrupted save file at {}: {message}",
                    path.display()
                )
            }
            Self::Incompatible(error) => {
                write!(formatter, "incompatible save file: {error:?}")
            }
        }
    }
}

impl std::error::Error for SaveFileError {}

impl From<SaveError> for SaveFileError {
    fn from(error: SaveError) -> Self {
        Self::Incompatible(error)
    }
}

fn tmp_sibling(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp", path.display()))
}

/// Writes via a `.tmp` sibling then `fs::rename`, which is atomic on the same
/// filesystem: a crash or power loss mid-write can never leave a half-written
/// file at `path`, the most common real-world cause of a "corrupted save".
pub fn save_to_path(
    path: &Path,
    header: SaveFileHeader,
    save: &SaveGame,
) -> Result<(), SaveFileError> {
    let envelope = SaveFileEnvelope::V29(SaveFileEnvelopeV29 {
        header,
        payload: save.clone(),
    });
    let text = ron::ser::to_string_pretty(&envelope, ron::ser::PrettyConfig::default()).map_err(
        |error| SaveFileError::Corrupted {
            path: path.to_path_buf(),
            message: error.to_string(),
        },
    )?;

    let tmp_path = tmp_sibling(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SaveFileError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    fs::write(&tmp_path, text).map_err(|error| SaveFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    fs::rename(&tmp_path, path).map_err(|error| SaveFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    Ok(())
}

/// Removes a save file from disk. The persistence crate only knows about the
/// `.ron` payload itself — a client-side sidecar (e.g. `galactic_client`'s
/// navigation-view `.nav` file) is that caller's own concern to remove
/// alongside this, exactly as it's already that caller's concern to write.
pub fn delete_save(path: &Path) -> Result<(), SaveFileError> {
    fs::remove_file(path).map_err(|error| SaveFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

/// Deserializes the envelope only — does not validate the payload against the
/// current ruleset/universe. Callers that need a playable `Simulation` must
/// follow up with `restore_from_snapshot`, converting its `Err(SaveError)`
/// with `SaveFileError::from`/`?`.
pub fn load_from_path(path: &Path) -> Result<(SaveFileHeader, SaveGame), SaveFileError> {
    let text = fs::read_to_string(path).map_err(|error| SaveFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let envelope: SaveFileEnvelope =
        ron::de::from_str(&text).map_err(|error| SaveFileError::Corrupted {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    match envelope {
        SaveFileEnvelope::V29(SaveFileEnvelopeV29 { header, payload }) => Ok((header, payload)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSlotMetadata {
    pub path: PathBuf,
    pub display_name: String,
    pub saved_at_unix_seconds: u64,
    pub save_version: u32,
    pub playtime_seconds: u64,
    pub colony_count: usize,
    pub corrupted: bool,
}

/// Never fails as a whole: a file that fails to even deserialize produces a
/// `corrupted: true` entry (so the browser can show a disabled row) instead
/// of silently vanishing from the list or aborting the scan. Deliberately
/// does not call `restore_from_snapshot` — that would regenerate the whole
/// procedural universe just to populate a metadata list.
pub fn list_save_slots(directory: &Path) -> Vec<SaveSlotMetadata> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut slots: Vec<SaveSlotMetadata> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(SAVE_FILE_EXTENSION))
        .map(|path| match load_from_path(&path) {
            Ok((header, save)) => SaveSlotMetadata {
                display_name: header.display_name,
                saved_at_unix_seconds: header.saved_at_unix_seconds,
                save_version: save.version,
                playtime_seconds: save.state.clock.current_tick.value()
                    / u64::from(galactic_sim::STRATEGIC_TICKS_PER_SECOND),
                colony_count: save.state.colonies.len(),
                corrupted: false,
                path,
            },
            Err(_) => SaveSlotMetadata {
                display_name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                saved_at_unix_seconds: 0,
                save_version: 0,
                playtime_seconds: 0,
                colony_count: 0,
                corrupted: true,
                path,
            },
        })
        .collect();

    slots.sort_by_key(|slot| std::cmp::Reverse(slot.saved_at_unix_seconds));
    slots
}

const SAVE_DIR_ENV: &str = "GALACTIC_SAVE_DIR";

/// Mirrors `GALACTIC_RULESET_DIR`'s env-override-or-default convention
/// (`galactic_sim::ruleset`): overridable for tests/packaging, a real
/// per-user data directory by default.
pub fn default_save_directory() -> PathBuf {
    if let Ok(overridden) = std::env::var(SAVE_DIR_ENV) {
        return PathBuf::from(overridden);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("galactic")
        .join("saves")
}

#[cfg(test)]
mod tests {
    use galactic_domain::UniverseConfig;
    use galactic_sim::Simulation;

    use super::*;
    use crate::restore::restore_from_snapshot;
    use crate::save::SAVE_VERSION;
    use crate::snapshot::snapshot_from_simulation;

    fn header(name: &str) -> SaveFileHeader {
        SaveFileHeader {
            display_name: name.to_string(),
            saved_at_unix_seconds: 1_700_000_000,
        }
    }

    #[test]
    fn save_and_load_round_trip_bytes_identically() {
        let temp_dir =
            std::env::temp_dir().join(format!("galactic-save-test-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");
        let path = temp_dir.join("slot.ron");

        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);

        save_to_path(&path, header("Test"), &save).expect("save succeeds");
        assert!(
            !tmp_sibling(&path).exists(),
            "the .tmp sibling is renamed away"
        );

        let (loaded_header, loaded_save) = load_from_path(&path).expect("load succeeds");
        assert_eq!(loaded_header, header("Test"));
        assert_eq!(loaded_save, save);

        let restored = restore_from_snapshot(&loaded_save).expect("restore succeeds");
        assert_eq!(
            restored.state().colonies.len(),
            simulation.state().colonies.len()
        );

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn harvest_mission_phase_survives_a_real_disk_round_trip() {
        let temp_dir =
            std::env::temp_dir().join(format!("galactic-save-test-harvest-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");
        let path = temp_dir.join("slot.ron");

        let (simulation, mission_id, site_id) = crate::tests::simulation_with_launched_harvest();
        let before = simulation.state().mission(mission_id).cloned();
        let save = snapshot_from_simulation(&simulation);

        save_to_path(&path, header("Mid-mission"), &save).expect("save succeeds");
        let (_, loaded_save) = load_from_path(&path).expect("load succeeds");
        let restored = restore_from_snapshot(&loaded_save).expect("restore succeeds");

        assert_eq!(restored.state().mission(mission_id).cloned(), before);
        assert!(
            restored
                .state()
                .extraction_sites
                .iter()
                .any(|site| site.id == site_id)
        );

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn garbage_bytes_are_reported_as_corrupted_not_a_panic() {
        let temp_dir =
            std::env::temp_dir().join(format!("galactic-save-test-garbage-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");
        let path = temp_dir.join("slot.ron");
        fs::write(&path, b"not a valid ron document at all {{{").expect("write succeeds");

        let result = load_from_path(&path);

        assert!(matches!(result, Err(SaveFileError::Corrupted { .. })));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn truncated_valid_ron_is_reported_as_corrupted_not_a_panic() {
        let temp_dir = std::env::temp_dir().join(format!(
            "galactic-save-test-truncated-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");
        let path = temp_dir.join("slot.ron");

        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        save_to_path(&path, header("Truncated"), &save).expect("save succeeds");
        let full = fs::read_to_string(&path).expect("read succeeds");
        fs::write(&path, &full[..full.len() / 2]).expect("truncated write succeeds");

        let result = load_from_path(&path);

        assert!(matches!(result, Err(SaveFileError::Corrupted { .. })));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn unsupported_version_is_reported_as_incompatible_not_a_crash() {
        let simulation = Simulation::new(UniverseConfig::mvp());
        let mut save = snapshot_from_simulation(&simulation);
        save.version = SAVE_VERSION + 1;

        let result = restore_from_snapshot(&save).map_err(SaveFileError::from);

        assert!(matches!(
            result,
            Err(SaveFileError::Incompatible(SaveError::UnsupportedVersion(
                _
            )))
        ));
    }

    #[test]
    fn additive_field_migration_defaults_when_absent_from_an_older_file() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct OlderShape {
            a: u32,
        }
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct NewerShape {
            a: u32,
            #[serde(default)]
            b: u32,
        }

        let older_bytes = ron::ser::to_string(&OlderShape { a: 7 }).expect("serializes");

        let migrated: NewerShape =
            ron::de::from_str(&older_bytes).expect("older bytes still deserialize");

        assert_eq!(migrated, NewerShape { a: 7, b: 0 });
    }

    #[test]
    fn list_save_slots_reports_corrupted_files_as_disabled_rows_not_omissions() {
        let temp_dir =
            std::env::temp_dir().join(format!("galactic-save-test-list-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");

        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        save_to_path(&temp_dir.join("good.ron"), header("Good"), &save).expect("save succeeds");
        fs::write(temp_dir.join("bad.ron"), b"not ron").expect("write succeeds");
        fs::write(temp_dir.join("ignored.txt"), b"not a save at all").expect("write succeeds");

        let mut slots = list_save_slots(&temp_dir);
        slots.sort_by(|a, b| a.display_name.cmp(&b.display_name));

        assert_eq!(slots.len(), 2, "the .txt file is not a save slot at all");
        let good = slots
            .iter()
            .find(|slot| !slot.corrupted)
            .expect("one good slot");
        assert_eq!(good.display_name, "Good");
        assert_eq!(good.colony_count, simulation.state().colonies.len());
        let bad = slots
            .iter()
            .find(|slot| slot.corrupted)
            .expect("one corrupted slot");
        assert_eq!(bad.display_name, "bad.ron");

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn delete_save_removes_the_file_and_reports_a_missing_one() {
        let temp_dir =
            std::env::temp_dir().join(format!("galactic-save-test-delete-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir is creatable");
        let path = temp_dir.join("slot.ron");

        let simulation = Simulation::new(UniverseConfig::mvp());
        let save = snapshot_from_simulation(&simulation);
        save_to_path(&path, header("To delete"), &save).expect("save succeeds");
        assert!(path.exists());

        delete_save(&path).expect("delete succeeds");
        assert!(!path.exists());

        assert!(matches!(delete_save(&path), Err(SaveFileError::Io { .. })));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn default_save_directory_honors_the_env_override() {
        // SAFETY: test-only env var set/removal, no other test reads it concurrently within this crate.
        unsafe {
            std::env::set_var(SAVE_DIR_ENV, "/tmp/galactic-save-dir-override");
        }
        assert_eq!(
            default_save_directory(),
            PathBuf::from("/tmp/galactic-save-dir-override")
        );
        unsafe {
            std::env::remove_var(SAVE_DIR_ENV);
        }
    }
}

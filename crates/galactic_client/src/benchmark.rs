//! MVP-035: a reproducible, scripted benchmark mode. Not wired into any CI
//! pipeline (no GPU runner in this repo) — a tool the user runs locally
//! before a release to compare FPS/frame-time/memory across resolutions and
//! graphics presets. See the crate root for the CLI flags that enable it.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::math::Vec3;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use galactic_sim::MVP_HOME_SYSTEM_ID;
use serde::Serialize;

use crate::presentation::graphics_settings::{GraphicsPreset, GraphicsSettings};
use crate::presentation::strategic_navigation::{StrategicNavigation, StrategicViewMode};
use crate::{MemoryDiagnosticSources, process_memory_snapshot};

const BENCHMARK_DIR_ENV: &str = "GALACTIC_BENCHMARK_DIR";

/// Independent of `GraphicsPreset` on purpose — the ticket asks to test both
/// axes crossed, and window resolution was tied 1:1 to preset in MVP-034.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BenchmarkResolution {
    Hd720,
    Hd1080,
}

impl BenchmarkResolution {
    pub(crate) fn from_slug(value: &str) -> Option<Self> {
        match value {
            "720p" => Some(Self::Hd720),
            "1080p" => Some(Self::Hd1080),
            _ => None,
        }
    }

    pub(crate) const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Hd720 => (1280, 720),
            Self::Hd1080 => (1920, 1080),
        }
    }
}

const ALL_RESOLUTIONS: [BenchmarkResolution; 2] =
    [BenchmarkResolution::Hd720, BenchmarkResolution::Hd1080];
const ALL_PRESETS: [GraphicsPreset; 3] = [
    GraphicsPreset::Low,
    GraphicsPreset::Medium,
    GraphicsPreset::High,
];

pub(crate) fn graphics_preset_from_slug(value: &str) -> Option<GraphicsPreset> {
    match value {
        "low" => Some(GraphicsPreset::Low),
        "medium" => Some(GraphicsPreset::Medium),
        "high" => Some(GraphicsPreset::High),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkConfig {
    pub(crate) resolutions: Vec<BenchmarkResolution>,
    pub(crate) presets: Vec<GraphicsPreset>,
    pub(crate) export_dir: PathBuf,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            resolutions: ALL_RESOLUTIONS.to_vec(),
            presets: ALL_PRESETS.to_vec(),
            export_dir: default_benchmark_dir(),
        }
    }
}

/// Env-override-or-relative-default convention, mirroring
/// `default_settings_path` in `galactic_persistence` — but deliberately
/// *not* using `dirs::data_dir()`: this is a dev/QA artifact meant to be
/// compared across runs in a repo checkout, not user data persisted in the
/// OS profile.
pub(crate) fn default_benchmark_dir() -> PathBuf {
    if let Ok(overridden) = std::env::var(BENCHMARK_DIR_ENV) {
        return PathBuf::from(overridden);
    }
    PathBuf::from("benchmark-results")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LegView {
    System,
    Universe,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CameraState {
    pub(crate) focus: Vec3,
    pub(crate) distance: f32,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

#[derive(Debug, Clone, Copy)]
struct BenchmarkLeg {
    view: LegView,
    target: CameraState,
    duration_secs: f32,
}

const START_STATE: CameraState = CameraState {
    focus: Vec3::ZERO,
    distance: 34.0,
    yaw: 0.0,
    pitch: -0.62,
};

/// Fixed, reproducible camera path: two close-orbit legs in System view
/// (exercises shadows/planet-texture quality), then three Universe-view legs
/// sweeping through the Overview/Regional/Local LOD bands (thresholds from
/// `UniverseLod::from_distance`) plus a pan, then a long steady rotation for
/// a stable final average. All Universe legs stay well inside
/// `universe_max_distance` for every `UniverseScalePreset` — the exact
/// values are not meant to look good (never visually verified), only to
/// reproducibly exercise the same geometry every run.
const BENCHMARK_LEGS: [BenchmarkLeg; 6] = [
    BenchmarkLeg {
        view: LegView::System,
        target: CameraState {
            focus: Vec3::ZERO,
            distance: 16.0,
            yaw: 1.2,
            pitch: -0.45,
        },
        duration_secs: 2.5,
    },
    BenchmarkLeg {
        view: LegView::System,
        target: CameraState {
            focus: Vec3::ZERO,
            distance: 55.0,
            yaw: 2.6,
            pitch: -0.2,
        },
        duration_secs: 2.0,
    },
    BenchmarkLeg {
        view: LegView::Universe,
        target: CameraState {
            focus: Vec3::ZERO,
            distance: 150.0,
            yaw: 0.0,
            pitch: -0.95,
        },
        duration_secs: 2.0,
    },
    BenchmarkLeg {
        view: LegView::Universe,
        target: CameraState {
            focus: Vec3::ZERO,
            distance: 65.0,
            yaw: std::f32::consts::PI,
            pitch: -0.5,
        },
        duration_secs: 2.5,
    },
    BenchmarkLeg {
        view: LegView::Universe,
        target: CameraState {
            focus: Vec3::new(30.0, 0.0, 18.0),
            distance: 30.0,
            yaw: 4.6,
            pitch: -0.35,
        },
        duration_secs: 2.5,
    },
    BenchmarkLeg {
        view: LegView::Universe,
        target: CameraState {
            focus: Vec3::new(30.0, 0.0, 18.0),
            distance: 30.0,
            yaw: 4.6 + std::f32::consts::TAU,
            pitch: -0.35,
        },
        duration_secs: 3.0,
    },
];

pub(crate) fn total_sequence_duration() -> f32 {
    BENCHMARK_LEGS.iter().map(|leg| leg.duration_secs).sum()
}

/// Pure and deterministic: same `elapsed_secs` always yields the same
/// state. Clamps to the final leg's target once `elapsed_secs` exceeds
/// `total_sequence_duration()`.
pub(crate) fn camera_state_at(elapsed_secs: f32) -> (LegView, CameraState) {
    let mut remaining = elapsed_secs.max(0.0);
    let mut previous = START_STATE;
    let last_index = BENCHMARK_LEGS.len() - 1;

    for (index, leg) in BENCHMARK_LEGS.iter().enumerate() {
        if remaining < leg.duration_secs || index == last_index {
            let t = if leg.duration_secs > 0.0 {
                (remaining / leg.duration_secs).clamp(0.0, 1.0)
            } else {
                1.0
            };
            return (leg.view, lerp_state(previous, leg.target, t));
        }
        remaining -= leg.duration_secs;
        previous = leg.target;
    }
    unreachable!("BENCHMARK_LEGS is never empty")
}

fn lerp_state(from: CameraState, to: CameraState, t: f32) -> CameraState {
    CameraState {
        focus: from.focus.lerp(to.focus, t),
        distance: from.distance + (to.distance - from.distance) * t,
        yaw: from.yaw + (to.yaw - from.yaw) * t,
        pitch: from.pitch + (to.pitch - from.pitch) * t,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameSample {
    pub(crate) elapsed_secs: f32,
    pub(crate) frame_time_ms: f32,
    pub(crate) fps: f32,
    pub(crate) entity_count: usize,
    pub(crate) mesh_count: usize,
    pub(crate) material_count: usize,
    pub(crate) image_count: usize,
    pub(crate) memory_rss_kib: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct RangeStats {
    pub(crate) average: f32,
    pub(crate) minimum: f32,
    pub(crate) p95: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(crate) struct CountStats {
    pub(crate) average: f32,
    pub(crate) maximum: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AggregatedMetrics {
    pub(crate) frame_time_ms: RangeStats,
    pub(crate) fps: RangeStats,
    pub(crate) entities: CountStats,
    pub(crate) meshes: CountStats,
    pub(crate) materials: CountStats,
    pub(crate) images: CountStats,
    pub(crate) memory_rss_kib: CountStats,
}

fn range_stats(mut values: Vec<f32>) -> RangeStats {
    if values.is_empty() {
        return RangeStats {
            average: 0.0,
            minimum: 0.0,
            p95: 0.0,
        };
    }
    let average = values.iter().sum::<f32>() / values.len() as f32;
    values.sort_by(|a, b| a.partial_cmp(b).expect("frame metrics are never NaN"));
    let minimum = values[0];
    let p95_index = (((values.len() - 1) as f32) * 0.95).round() as usize;
    let p95 = values[p95_index.min(values.len() - 1)];
    RangeStats {
        average,
        minimum,
        p95,
    }
}

fn count_stats(values: &[u64]) -> CountStats {
    if values.is_empty() {
        return CountStats {
            average: 0.0,
            maximum: 0,
        };
    }
    let average = values.iter().sum::<u64>() as f32 / values.len() as f32;
    let maximum = *values.iter().max().expect("values is non-empty");
    CountStats { average, maximum }
}

pub(crate) fn aggregate(samples: &[FrameSample]) -> AggregatedMetrics {
    let frame_time_ms = range_stats(samples.iter().map(|s| s.frame_time_ms).collect());
    let fps = range_stats(samples.iter().map(|s| s.fps).collect());
    let entities = count_stats(
        &samples
            .iter()
            .map(|s| s.entity_count as u64)
            .collect::<Vec<_>>(),
    );
    let meshes = count_stats(
        &samples
            .iter()
            .map(|s| s.mesh_count as u64)
            .collect::<Vec<_>>(),
    );
    let materials = count_stats(
        &samples
            .iter()
            .map(|s| s.material_count as u64)
            .collect::<Vec<_>>(),
    );
    let images = count_stats(
        &samples
            .iter()
            .map(|s| s.image_count as u64)
            .collect::<Vec<_>>(),
    );
    let memory_rss_kib = count_stats(&samples.iter().map(|s| s.memory_rss_kib).collect::<Vec<_>>());
    AggregatedMetrics {
        frame_time_ms,
        fps,
        entities,
        meshes,
        materials,
        images,
        memory_rss_kib,
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BenchmarkReport {
    pub(crate) graphics_preset: GraphicsPreset,
    pub(crate) resolution_width: u32,
    pub(crate) resolution_height: u32,
    pub(crate) seed: u64,
    pub(crate) generated_at_unix_secs: u64,
    pub(crate) frame_count: usize,
    pub(crate) sequence_duration_secs: f32,
    pub(crate) frame_time_ms: RangeStats,
    pub(crate) fps: RangeStats,
    pub(crate) entities: CountStats,
    pub(crate) meshes: CountStats,
    pub(crate) materials: CountStats,
    pub(crate) images: CountStats,
    pub(crate) memory_rss_kib: CountStats,
}

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(crate) fn format_text(report: &BenchmarkReport) -> String {
    format!(
        "Galactic — rapport de benchmark\n\
         Preset graphique       : {:?}\n\
         Résolution              : {}x{}\n\
         Seed                    : {}\n\
         Frames échantillonnées  : {}\n\
         Durée de la séquence    : {:.2}s\n\
         \n\
         Temps de frame (ms)     — moyenne {:.3} / minimum {:.3} / p95 {:.3}\n\
         FPS                     — moyenne {:.1} / minimum {:.1} / p95 {:.1}\n\
         Entités                 — moyenne {:.1} / maximum {}\n\
         Meshes                  — moyenne {:.1} / maximum {}\n\
         Matériaux                — moyenne {:.1} / maximum {}\n\
         Images                  — moyenne {:.1} / maximum {}\n\
         Mémoire RSS (KiB)       — moyenne {:.1} / maximum {}\n",
        report.graphics_preset,
        report.resolution_width,
        report.resolution_height,
        report.seed,
        report.frame_count,
        report.sequence_duration_secs,
        report.frame_time_ms.average,
        report.frame_time_ms.minimum,
        report.frame_time_ms.p95,
        report.fps.average,
        report.fps.minimum,
        report.fps.p95,
        report.entities.average,
        report.entities.maximum,
        report.meshes.average,
        report.meshes.maximum,
        report.materials.average,
        report.materials.maximum,
        report.images.average,
        report.images.maximum,
        report.memory_rss_kib.average,
        report.memory_rss_kib.maximum,
    )
}

pub(crate) fn format_csv(samples: &[FrameSample]) -> String {
    let mut out = String::from(
        "elapsed_secs,frame_time_ms,fps,entity_count,mesh_count,material_count,image_count,memory_rss_kib\n",
    );
    for sample in samples {
        out.push_str(&format!(
            "{:.4},{:.4},{:.4},{},{},{},{},{}\n",
            sample.elapsed_secs,
            sample.frame_time_ms,
            sample.fps,
            sample.entity_count,
            sample.mesh_count,
            sample.material_count,
            sample.image_count,
            sample.memory_rss_kib,
        ));
    }
    out
}

pub(crate) fn format_json(report: &BenchmarkReport) -> String {
    serde_json::to_string_pretty(report).expect("BenchmarkReport always serializes")
}

fn benchmark_file_stem(report: &BenchmarkReport) -> String {
    format!(
        "benchmark_{:?}_{}x{}_{}",
        report.graphics_preset,
        report.resolution_width,
        report.resolution_height,
        report.generated_at_unix_secs
    )
    .to_ascii_lowercase()
}

pub(crate) fn write_benchmark_report(
    dir: &Path,
    report: &BenchmarkReport,
    samples: &[FrameSample],
) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let stem = benchmark_file_stem(report);
    std::fs::write(dir.join(format!("{stem}.txt")), format_text(report))?;
    std::fs::write(dir.join(format!("{stem}.csv")), format_csv(samples))?;
    std::fs::write(dir.join(format!("{stem}.json")), format_json(report))?;
    Ok(())
}

#[derive(Resource)]
pub(crate) struct BenchmarkState {
    config: BenchmarkConfig,
    combinations: Vec<(GraphicsPreset, BenchmarkResolution)>,
    combination_index: usize,
    elapsed_in_sequence: f32,
    samples: Vec<FrameSample>,
}

impl BenchmarkState {
    pub(crate) fn new(config: BenchmarkConfig) -> Self {
        let mut combinations = Vec::new();
        for preset in &config.presets {
            for resolution in &config.resolutions {
                combinations.push((*preset, *resolution));
            }
        }
        Self {
            config,
            combinations,
            combination_index: 0,
            elapsed_in_sequence: 0.0,
            samples: Vec::new(),
        }
    }

    fn current_combination(&self) -> Option<(GraphicsPreset, BenchmarkResolution)> {
        self.combinations.get(self.combination_index).copied()
    }
}

fn finish_current_combination(
    state: &BenchmarkState,
    preset: GraphicsPreset,
    resolution: BenchmarkResolution,
) {
    let (width, height) = resolution.dimensions();
    let metrics = aggregate(&state.samples);
    let report = BenchmarkReport {
        graphics_preset: preset,
        resolution_width: width,
        resolution_height: height,
        seed: galactic_domain::MVP_UNIVERSE_SEED,
        generated_at_unix_secs: unix_now_secs(),
        frame_count: state.samples.len(),
        sequence_duration_secs: total_sequence_duration(),
        frame_time_ms: metrics.frame_time_ms,
        fps: metrics.fps,
        entities: metrics.entities,
        meshes: metrics.meshes,
        materials: metrics.materials,
        images: metrics.images,
        memory_rss_kib: metrics.memory_rss_kib,
    };
    match write_benchmark_report(&state.config.export_dir, &report, &state.samples) {
        Ok(()) => info!(
            target: "galactic_benchmark",
            "rapport de benchmark écrit dans {}",
            state.config.export_dir.display(),
        ),
        Err(error) => error!(
            target: "galactic_benchmark",
            "échec de l'export du rapport de benchmark : {error}",
        ),
    }
}

/// Records one raw sample per frame. Reuses `MemoryDiagnosticSources` /
/// `process_memory_snapshot` (crate root, MVP-034's `MemoryDiagnostics`)
/// rather than duplicating those counters. No-op if no benchmark is active.
pub(crate) fn sample_benchmark_metrics(
    time: Res<Time>,
    benchmark: Option<ResMut<BenchmarkState>>,
    sources: MemoryDiagnosticSources,
) {
    let Some(mut benchmark) = benchmark else {
        return;
    };
    if benchmark.current_combination().is_none() {
        return;
    }
    let delta = time.delta_secs();
    // The very first `Update` tick after a window is (re)created always has
    // a zero delta (no previous frame to diff against) — recording it would
    // drag `minimum` frame-time/FPS down to a meaningless 0.
    if delta <= 0.0 {
        return;
    }
    let process = process_memory_snapshot();
    let elapsed_secs = benchmark.elapsed_in_sequence;
    benchmark.samples.push(FrameSample {
        elapsed_secs,
        frame_time_ms: delta * 1000.0,
        fps: 1.0 / delta,
        entity_count: sources.entities.iter().count(),
        mesh_count: sources.meshes.len(),
        material_count: sources.materials.len(),
        image_count: sources.images.len(),
        memory_rss_kib: process.rss_kib,
    });
}

/// Drives the scripted camera and the resolution/preset matrix. Writes
/// `StrategicNavigation` fields directly; `update_strategic_camera`
/// (unmodified) recomputes the `Transform` from them exactly as it does for
/// mouse/keyboard input. Runs after `update_window_resolution_preset` in the
/// system chain so its explicit resolution always wins over the
/// preset-derived one for the frame a preset changes.
pub(crate) fn drive_benchmark_sequence(
    time: Res<Time>,
    benchmark: Option<ResMut<BenchmarkState>>,
    mut navigation: ResMut<StrategicNavigation>,
    mut graphics: ResMut<GraphicsSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let Some(mut benchmark) = benchmark else {
        return;
    };
    let Some((preset, resolution)) = benchmark.current_combination() else {
        return;
    };

    if benchmark.elapsed_in_sequence == 0.0 {
        navigation.mode = StrategicViewMode::System(MVP_HOME_SYSTEM_ID);
        if graphics.preset != preset {
            graphics.preset = preset;
        }
        let (width, height) = resolution.dimensions();
        if let Ok(mut window) = windows.single_mut() {
            let target = Vec2::new(width as f32, height as f32);
            if window.resolution.size() != target {
                window.resolution.set(target.x, target.y);
            }
        }
    }

    benchmark.elapsed_in_sequence += time.delta_secs();
    let (leg_view, state) = camera_state_at(benchmark.elapsed_in_sequence);
    match leg_view {
        LegView::System => {
            navigation.mode = StrategicViewMode::System(MVP_HOME_SYSTEM_ID);
            navigation.system_focus = state.focus;
            navigation.system_distance = state.distance;
            navigation.system_yaw = state.yaw;
            navigation.system_pitch = state.pitch;
        }
        LegView::Universe => {
            navigation.mode = StrategicViewMode::Universe;
            navigation.universe_focus = state.focus;
            navigation.universe_distance = state.distance;
            navigation.universe_yaw = state.yaw;
            navigation.universe_pitch = state.pitch;
        }
    }

    if benchmark.elapsed_in_sequence >= total_sequence_duration() {
        finish_current_combination(&benchmark, preset, resolution);
        benchmark.combination_index += 1;
        benchmark.elapsed_in_sequence = 0.0;
        benchmark.samples.clear();
        if benchmark.current_combination().is_none() {
            app_exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(frame_time_ms: f32, fps: f32) -> FrameSample {
        FrameSample {
            elapsed_secs: 0.0,
            frame_time_ms,
            fps,
            entity_count: 100,
            mesh_count: 4,
            material_count: 6,
            image_count: 8,
            memory_rss_kib: 200_000,
        }
    }

    #[test]
    fn range_stats_computes_average_minimum_and_p95() {
        // 20 values 1..=20: p95 index = round(19 * 0.95) = 18 -> sorted[18] = 19.0
        let values: Vec<f32> = (1..=20).map(|value| value as f32).collect();
        let stats = range_stats(values);
        assert_eq!(stats.average, 10.5);
        assert_eq!(stats.minimum, 1.0);
        assert_eq!(stats.p95, 19.0);
    }

    #[test]
    fn range_stats_on_empty_input_is_all_zero() {
        assert_eq!(
            range_stats(Vec::new()),
            RangeStats {
                average: 0.0,
                minimum: 0.0,
                p95: 0.0
            }
        );
    }

    #[test]
    fn aggregate_reports_average_and_peak_counts() {
        let samples = vec![sample(16.0, 60.0), sample(20.0, 50.0), sample(12.0, 80.0)];
        let metrics = aggregate(&samples);
        assert_eq!(metrics.frame_time_ms.minimum, 12.0);
        assert_eq!(metrics.fps.minimum, 50.0);
        assert_eq!(metrics.entities.maximum, 100);
        assert_eq!(metrics.entities.average, 100.0);
    }

    #[test]
    fn camera_state_at_start_matches_start_state() {
        let (view, state) = camera_state_at(0.0);
        assert_eq!(view, LegView::System);
        assert_eq!(state, START_STATE);
    }

    #[test]
    fn camera_state_at_is_deterministic() {
        assert_eq!(camera_state_at(3.7), camera_state_at(3.7));
    }

    #[test]
    fn camera_state_at_reaches_first_leg_target_at_its_boundary() {
        let (view, state) = camera_state_at(BENCHMARK_LEGS[0].duration_secs);
        assert_eq!(view, BENCHMARK_LEGS[0].view);
        assert_eq!(state, BENCHMARK_LEGS[0].target);
    }

    #[test]
    fn camera_state_at_clamps_past_total_duration_to_final_leg_target() {
        let past_the_end = total_sequence_duration() + 10.0;
        let (view, state) = camera_state_at(past_the_end);
        let last_leg = BENCHMARK_LEGS.last().expect("legs is non-empty");
        assert_eq!(view, last_leg.view);
        assert_eq!(state, last_leg.target);
    }

    #[test]
    fn benchmark_resolution_slug_round_trips() {
        assert_eq!(
            BenchmarkResolution::from_slug("720p"),
            Some(BenchmarkResolution::Hd720)
        );
        assert_eq!(
            BenchmarkResolution::from_slug("1080p"),
            Some(BenchmarkResolution::Hd1080)
        );
        assert_eq!(BenchmarkResolution::from_slug("4k"), None);
    }

    #[test]
    fn graphics_preset_slug_round_trips() {
        assert_eq!(graphics_preset_from_slug("low"), Some(GraphicsPreset::Low));
        assert_eq!(
            graphics_preset_from_slug("medium"),
            Some(GraphicsPreset::Medium)
        );
        assert_eq!(
            graphics_preset_from_slug("high"),
            Some(GraphicsPreset::High)
        );
        assert_eq!(graphics_preset_from_slug("ultra"), None);
    }

    #[test]
    fn default_benchmark_dir_honors_the_env_override() {
        // SAFETY: test-only env var, no other test in this crate reads BENCHMARK_DIR_ENV.
        unsafe {
            std::env::set_var(BENCHMARK_DIR_ENV, "/tmp/galactic-benchmark-dir-override");
        }
        assert_eq!(
            default_benchmark_dir(),
            PathBuf::from("/tmp/galactic-benchmark-dir-override")
        );
        unsafe {
            std::env::remove_var(BENCHMARK_DIR_ENV);
        }
    }

    #[test]
    fn default_benchmark_dir_without_override_is_relative() {
        // SAFETY: ensure no leftover override from another test in this process.
        unsafe {
            std::env::remove_var(BENCHMARK_DIR_ENV);
        }
        assert_eq!(default_benchmark_dir(), PathBuf::from("benchmark-results"));
    }

    fn fixture_report() -> BenchmarkReport {
        BenchmarkReport {
            graphics_preset: GraphicsPreset::High,
            resolution_width: 1920,
            resolution_height: 1080,
            seed: 42,
            generated_at_unix_secs: 1_700_000_000,
            frame_count: 3,
            sequence_duration_secs: 12.5,
            frame_time_ms: RangeStats {
                average: 16.0,
                minimum: 12.0,
                p95: 20.0,
            },
            fps: RangeStats {
                average: 62.0,
                minimum: 50.0,
                p95: 83.0,
            },
            entities: CountStats {
                average: 100.0,
                maximum: 120,
            },
            meshes: CountStats {
                average: 4.0,
                maximum: 4,
            },
            materials: CountStats {
                average: 6.0,
                maximum: 6,
            },
            images: CountStats {
                average: 8.0,
                maximum: 8,
            },
            memory_rss_kib: CountStats {
                average: 200_000.0,
                maximum: 210_000,
            },
        }
    }

    #[test]
    fn format_text_includes_the_key_figures() {
        let text = format_text(&fixture_report());
        assert!(text.contains("High"));
        assert!(text.contains("1920x1080"));
        assert!(text.contains("42"));
        assert!(text.contains("16.000"));
        assert!(text.contains("83.0"));
    }

    #[test]
    fn format_csv_has_a_header_and_one_row_per_sample() {
        let samples = vec![sample(16.0, 60.0), sample(20.0, 50.0)];
        let csv = format_csv(&samples);
        let mut lines = csv.lines();
        assert_eq!(
            lines.next(),
            Some(
                "elapsed_secs,frame_time_ms,fps,entity_count,mesh_count,material_count,image_count,memory_rss_kib"
            )
        );
        assert_eq!(lines.clone().count(), 2);
        assert!(lines.next().unwrap().starts_with("0.0000,16.0000,60.0000,"));
    }

    #[test]
    fn format_json_round_trips_the_report_shape() {
        let json = format_json(&fixture_report());
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(value["graphics_preset"], "High");
        assert_eq!(value["resolution_width"], 1920);
        assert_eq!(value["fps"]["p95"], 83.0);
    }
}

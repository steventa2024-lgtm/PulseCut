//! FFmpeg-backed probe, preview, and export.
//!
//! PulseCut does not link libav. It runs the `ffmpeg` and `ffprobe` executables
//! and builds the filter graph from the canonical project model, so preview and
//! export describe the same edits.

mod error;
mod graph;
mod probe;
mod render;
mod tools;
mod waveform;

pub use error::EngineError;
pub use graph::{build_plan, RenderPlan, RenderSettings};
pub use probe::{probe_file, MediaInfo};
pub use render::{render_plan, render_project, report_output, RenderReport};
pub use tools::{engine_status, ffmpeg_path, ffprobe_path, EngineStatus};
pub use waveform::{thumbnail_jpeg, waveform_peaks};

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use project_model::Project;
use sha2::{Digest, Sha256};

pub fn preview_cache_key(project: &Project, settings: &RenderSettings) -> String {
    let mut hasher = Sha256::new();
    if let Ok(bytes) = serde_json::to_vec(project) {
        hasher.update(bytes);
    }
    hasher.update(settings.cache_token().as_bytes());
    for asset in &project.assets {
        hasher.update(asset.id.as_bytes());
        hasher.update(file_stamp(Path::new(&asset.path)).as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn file_stamp(path: &Path) -> String {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let created = meta
                .created()
                .ok()
                .and_then(|time: SystemTime| time.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{}:{}:{}", meta.len(), modified, created)
        }
        Err(_) => "missing".to_string(),
    }
}

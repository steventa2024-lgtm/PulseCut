//! PulseCut media engine (FFmpeg filter-graph backend). See docs/ENGINE_EVALUATION.md.

pub mod analysis;
pub mod plan;
pub mod probe;
pub mod render;
pub mod tools;
pub mod verify;

pub use plan::{compile, RenderPlan, RenderRequest};
pub use probe::probe;
pub use render::{
    export, render_audio_pcm, render_still, ExportProgress, ExportQuality, ExportResult,
    ExportSettings, PlaybackStream,
};
pub use tools::FfmpegTools;
pub use verify::{verify_export, VerifyReport};

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MediaError {
    #[error("FFmpeg was not found. Install FFmpeg (Windows: `winget install Gyan.FFmpeg`) or set PULSECUT_FFMPEG_DIR to the folder containing ffmpeg and ffprobe")]
    ToolMissing,
    #[error("cannot import {path}: {message}")]
    Probe { path: String, message: String },
    #[error("{0}")]
    Unsupported(String),
    #[error("{message}")]
    Render { message: String, log: String },
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(String),
}

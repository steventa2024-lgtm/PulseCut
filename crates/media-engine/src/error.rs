use thiserror::Error;

use project_model::ProjectError;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("{0}")]
    Message(String),
    #[error(
        "FFmpeg was not found. Install FFmpeg and make sure `ffmpeg` is on PATH, or set PULSECUT_FFMPEG to the executable."
    )]
    FfmpegMissing,
    #[error(
        "ffprobe was not found. Install FFmpeg and make sure `ffprobe` is on PATH, or set PULSECUT_FFPROBE to the executable."
    )]
    FfprobeMissing,
    #[error("These media files are missing or offline: {0}")]
    MissingMedia(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl EngineError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

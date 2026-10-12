use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::EngineError;

pub fn ffmpeg_path() -> Result<PathBuf, EngineError> {
    find_tool("PULSECUT_FFMPEG", "ffmpeg").ok_or(EngineError::FfmpegMissing)
}

pub fn ffprobe_path() -> Result<PathBuf, EngineError> {
    find_tool("PULSECUT_FFPROBE", "ffprobe").ok_or(EngineError::FfprobeMissing)
}

fn find_tool(env_key: &str, name: &str) -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(env_key) {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

pub fn ffmpeg_version_line(ffmpeg: &Path) -> Result<String, EngineError> {
    let output = Command::new(ffmpeg).arg("-version").output()?;
    if !output.status.success() {
        return Err(EngineError::msg(
            "FFmpeg did not report its version. Check that the executable can run.",
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().next().unwrap_or("").trim().to_string())
}

pub fn has_encoder(ffmpeg: &Path, name: &str) -> bool {
    let Ok(output) = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().any(|line| {
        line.split_whitespace()
            .nth(1)
            .map(|encoder| encoder == name)
            .unwrap_or(false)
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub version: String,
    pub libx264: bool,
    pub aac: bool,
}

pub fn engine_status() -> Result<EngineStatus, EngineError> {
    let ffmpeg = ffmpeg_path()?;
    let ffprobe = ffprobe_path()?;
    Ok(EngineStatus {
        version: ffmpeg_version_line(&ffmpeg)?,
        libx264: has_encoder(&ffmpeg, "libx264"),
        aac: has_encoder(&ffmpeg, "aac"),
        ffmpeg_path: ffmpeg.to_string_lossy().to_string(),
        ffprobe_path: ffprobe.to_string_lossy().to_string(),
    })
}

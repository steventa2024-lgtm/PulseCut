//! Thumbnails and waveform peaks for the media browser and timeline.

use crate::tools::{command, FfmpegTools};
use crate::MediaError;
use std::path::Path;

/// JPEG thumbnail of the frame at `at_seconds`, scaled to `width` (aspect preserved).
pub fn thumbnail(
    tools: &FfmpegTools,
    path: &Path,
    at_seconds: f64,
    width: u32,
) -> Result<Vec<u8>, MediaError> {
    let out = command(&tools.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &format!("{:.3}", at_seconds.max(0.0)),
            "-i",
        ])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={width}:-2"),
            "-f",
            "image2pipe",
            "-c:v",
            "mjpeg",
            "-q:v",
            "4",
            "pipe:1",
        ])
        .output()
        .map_err(|e| MediaError::Io(format!("could not run ffmpeg: {e}")))?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(MediaError::Render {
            message: "thumbnail extraction failed".into(),
            log: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out.stdout)
}

pub const WAVEFORM_PEAKS_PER_SECOND: u32 = 50;
const WAVEFORM_RATE: u32 = 8000;

/// Peak amplitudes (0..1) of the first audio stream, `WAVEFORM_PEAKS_PER_SECOND` per second.
pub fn waveform(tools: &FfmpegTools, path: &Path) -> Result<Vec<f32>, MediaError> {
    let out = command(&tools.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-vn",
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-ar",
            &WAVEFORM_RATE.to_string(),
            "-f",
            "s16le",
            "-c:a",
            "pcm_s16le",
            "pipe:1",
        ])
        .output()
        .map_err(|e| MediaError::Io(format!("could not run ffmpeg: {e}")))?;
    if !out.status.success() {
        return Err(MediaError::Render {
            message: "waveform extraction failed".into(),
            log: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    let bucket = (WAVEFORM_RATE / WAVEFORM_PEAKS_PER_SECOND) as usize;
    Ok(out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b).unsigned_abs())
        .collect::<Vec<u16>>()
        .chunks(bucket)
        .map(|c| *c.iter().max().unwrap_or(&0) as f32 / 32768.0)
        .collect())
}

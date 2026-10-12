use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use project_model::format_seconds;

use crate::error::EngineError;
use crate::tools::ffmpeg_path;

/// Peak amplitude in the range 0.0 to 1.0 for each bucket of the requested range.
pub fn waveform_peaks(
    path: &Path,
    source_in_us: i64,
    duration_us: i64,
    buckets: usize,
) -> Result<Vec<f32>, EngineError> {
    if !path.is_file() {
        return Err(EngineError::msg(format!(
            "Cannot draw a waveform because the file is missing: {}",
            path.display()
        )));
    }
    if duration_us <= 0 || buckets == 0 {
        return Err(EngineError::msg("The waveform range is empty."));
    }
    let buckets = buckets.min(2048);
    let ffmpeg = ffmpeg_path()?;
    let mut child = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-ss")
        .arg(format_seconds(source_in_us))
        .arg("-t")
        .arg(format_seconds(duration_us))
        .arg("-i")
        .arg(path)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("4000")
        .arg("-f")
        .arg("f32le")
        .arg("pipe:1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| EngineError::msg("FFmpeg did not return audio samples for the waveform."))?;
    let expected = (4_000.0 * (duration_us as f64 / 1_000_000.0)).max(1.0);
    let mut peaks = vec![0f32; buckets];
    let mut byte_buf = [0u8; 4096];
    let mut pending = Vec::new();
    let mut sample_index = 0usize;
    loop {
        let read = stdout.read(&mut byte_buf)?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&byte_buf[..read]);
        let usable = pending.len() - (pending.len() % 4);
        for chunk in pending[..usable].chunks_exact(4) {
            let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).abs();
            let bucket = ((sample_index as f64 / expected) * buckets as f64) as usize;
            let bucket = bucket.min(buckets - 1);
            if sample > peaks[bucket] {
                peaks[bucket] = sample;
            }
            sample_index += 1;
        }
        pending.drain(..usable);
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(EngineError::msg(
            "FFmpeg could not decode audio for this waveform.",
        ));
    }
    if sample_index == 0 {
        return Err(EngineError::msg(
            "This file did not produce any audio samples for a waveform.",
        ));
    }
    let max = peaks.iter().copied().fold(0f32, f32::max).max(0.000_1);
    for peak in &mut peaks {
        *peak = (*peak / max).clamp(0.0, 1.0);
    }
    Ok(peaks)
}

pub fn thumbnail_jpeg(path: &Path, time_us: i64, width: u32) -> Result<Vec<u8>, EngineError> {
    if !path.is_file() {
        return Err(EngineError::msg(format!(
            "Cannot make a thumbnail because the file is missing: {}",
            path.display()
        )));
    }
    let width = width.clamp(32, 640);
    let ffmpeg = ffmpeg_path()?;
    let output = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-ss")
        .arg(format_seconds(time_us.max(0)))
        .arg("-i")
        .arg(path)
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={width}:-2"))
        .arg("-f")
        .arg("image2pipe")
        .arg("-vcodec")
        .arg("mjpeg")
        .arg("pipe:1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(EngineError::msg(format!(
            "No thumbnail could be decoded from \"{}\".",
            path.display()
        )));
    }
    Ok(output.stdout)
}

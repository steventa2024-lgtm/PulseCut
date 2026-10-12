use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use project_model::Project;
use serde::Serialize;

use crate::error::EngineError;
use crate::graph::{build_plan, RenderPlan, RenderSettings};
use crate::probe::probe_file;
use crate::tools::ffmpeg_path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderReport {
    pub output: String,
    pub duration_us: i64,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub fps_num: Option<u32>,
    pub fps_den: Option<u32>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
}

pub fn render_project(
    project: &Project,
    settings: &RenderSettings,
    output: &Path,
    progress: impl Fn(f64),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<RenderReport, EngineError> {
    let plan = build_plan(project, settings, true)?;
    render_plan(&plan, settings, output, progress, cancel)
}

pub fn render_plan(
    plan: &RenderPlan,
    settings: &RenderSettings,
    output: &Path,
    progress: impl Fn(f64),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<RenderReport, EngineError> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let ffmpeg = ffmpeg_path()?;
    if !crate::tools::has_encoder(&ffmpeg, "libx264") {
        return Err(EngineError::msg(
            "This FFmpeg build has no libx264 encoder, so PulseCut cannot write H.264 MP4 files. Install an FFmpeg build with libx264.",
        ));
    }
    let script_path =
        std::env::temp_dir().join(format!("pulsecut-filter-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&script_path, &plan.filter)?;
    let script_guard = ScriptGuard(script_path.clone());

    let mut command = Command::new(&ffmpeg);
    command
        .arg("-y")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-loglevel")
        .arg("error");
    for input in &plan.inputs {
        command.arg("-i").arg(input);
    }
    command
        .arg("-filter_complex_script")
        .arg(&script_path)
        .arg("-map")
        .arg("[vout]")
        .arg("-map")
        .arg("[aout]")
        .arg("-t")
        .arg(project_model::format_seconds(plan.duration_us))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg(&settings.preset)
        .arg("-crf")
        .arg(settings.crf.to_string())
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-r")
        .arg(format!("{}/{}", settings.fps_num, settings.fps_den))
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg(format!("{}k", settings.audio_bitrate_kbps))
        .arg("-movflags")
        .arg("+faststart")
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| EngineError::msg(format!("Could not start FFmpeg: {err}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| EngineError::msg("FFmpeg did not provide a progress stream."))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| EngineError::msg("FFmpeg did not provide an error stream."))?;
    let stderr_text = Arc::new(Mutex::new(String::new()));
    let stderr_slot = Arc::clone(&stderr_text);
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(mut slot) = stderr_slot.lock() {
                        slot.push_str(&line);
                        if slot.len() > 8_000 {
                            let drain = slot.len() - 6_000;
                            slot.drain(..drain);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let mut cancelled = false;
    loop {
        if cancel
            .as_ref()
            .map(|flag| flag.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            cancelled = true;
            let _ = child.kill();
            break;
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Some(value) = line.trim().strip_prefix("out_time_us=") {
                    if let Ok(us) = value.parse::<f64>() {
                        if plan.duration_us > 0 {
                            let ratio = (us / plan.duration_us as f64).clamp(0.0, 1.0);
                            progress(ratio);
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
    let status = child.wait()?;
    drop(script_guard);
    if cancelled {
        let _ = std::fs::remove_file(output);
        return Err(EngineError::msg("Render cancelled."));
    }
    if !status.success() {
        let detail = stderr_text
            .lock()
            .map(|text| text.trim().to_string())
            .unwrap_or_default();
        let detail = if detail.is_empty() {
            format!("FFmpeg exited with {status}.")
        } else {
            detail
        };
        return Err(EngineError::msg(format!("Export failed. {detail}")));
    }
    progress(1.0);
    // Give the file system a moment on network disks. Local disks are immediate.
    let _ = Duration::from_millis(0);
    report_output(output)
}

pub fn report_output(output: &Path) -> Result<RenderReport, EngineError> {
    let meta = std::fs::metadata(output).map_err(|_| {
        EngineError::msg(format!(
            "FFmpeg finished but the output file is missing: {}",
            output.display()
        ))
    })?;
    let info = probe_file(output)?;
    Ok(RenderReport {
        output: output.to_string_lossy().to_string(),
        duration_us: info.duration_us,
        width: info.width.unwrap_or(0),
        height: info.height.unwrap_or(0),
        bytes: meta.len(),
        video_codec: info.video_codec,
        audio_codec: info.audio_codec,
        fps_num: info.fps_num,
        fps_den: info.fps_den,
        audio_sample_rate: info.audio_sample_rate,
        audio_channels: info.audio_channels,
    })
}

struct ScriptGuard(PathBuf);

impl Drop for ScriptGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

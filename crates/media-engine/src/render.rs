use crate::plan::{compile, RenderPlan, RenderRequest};
use crate::tools::{command, FfmpegTools};
use crate::MediaError;
use pulsecut_project_model::{AssetId, Project};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const STDERR_TAIL: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportQuality {
    High,
    Standard,
    Draft,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    pub quality: ExportQuality,
    /// 1.0 = sequence resolution, 0.5 = half.
    pub scale: f64,
    pub audio_bitrate_kbps: u32,
}

impl Default for ExportSettings {
    fn default() -> Self {
        ExportSettings {
            quality: ExportQuality::Standard,
            scale: 1.0,
            audio_bitrate_kbps: 192,
        }
    }
}

/// Picks the best available H.264 encoder and its quality arguments.
pub fn video_encoder_args(
    tools: &FfmpegTools,
    q: ExportQuality,
) -> Result<(String, Vec<String>), MediaError> {
    let (crf, preset) = match q {
        ExportQuality::High => ("18", "slow"),
        ExportQuality::Standard => ("21", "medium"),
        ExportQuality::Draft => ("28", "veryfast"),
    };
    if tools.has_encoder("libx264") {
        return Ok((
            "libx264".into(),
            vec![
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                preset.into(),
                "-crf".into(),
                crf.into(),
                "-profile:v".into(),
                "high".into(),
            ],
        ));
    }
    let bitrate = match q {
        ExportQuality::High => "16M",
        ExportQuality::Standard => "8M",
        ExportQuality::Draft => "3M",
    };
    for enc in ["libopenh264", "h264_mf"] {
        if tools.has_encoder(enc) {
            return Ok((
                enc.into(),
                vec!["-c:v".into(), enc.into(), "-b:v".into(), bitrate.into()],
            ));
        }
    }
    Err(MediaError::Unsupported(
        "this FFmpeg build has no H.264 encoder (libx264, libopenh264 or h264_mf); install an FFmpeg build that includes one".into(),
    ))
}

fn spawn(tools: &FfmpegTools, args: &[String]) -> Result<Child, MediaError> {
    command(&tools.ffmpeg)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| MediaError::Render {
            message: format!("could not start ffmpeg: {e}"),
            log: String::new(),
        })
}

/// Collects the last `STDERR_TAIL` bytes of a child's stderr on a background thread.
fn drain_stderr(child: &mut Child) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut err) = child.stderr.take() {
        let b = buf.clone();
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = err.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut v = b.lock().unwrap();
                v.extend_from_slice(&chunk[..n]);
                if v.len() > STDERR_TAIL {
                    let cut = v.len() - STDERR_TAIL;
                    v.drain(..cut);
                }
            }
        });
    }
    buf
}

fn tail(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&buf.lock().unwrap()).into_owned()
}

fn run_to_bytes(tools: &FfmpegTools, args: &[String], what: &str) -> Result<Vec<u8>, MediaError> {
    let out = command(&tools.ffmpeg)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| MediaError::Render {
            message: format!("could not start ffmpeg: {e}"),
            log: String::new(),
        })?;
    if !out.status.success() {
        return Err(MediaError::Render {
            message: format!("{what} failed"),
            log: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out.stdout)
}

fn map_args(plan: &RenderPlan) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(l) = &plan.video_label {
        v.extend(["-map".into(), l.clone()]);
    }
    if let Some(l) = &plan.audio_label {
        v.extend(["-map".into(), l.clone()]);
    }
    v
}

/// Renders one composited timeline frame as JPEG (or PNG when `lossless`).
pub fn render_still(
    tools: &FfmpegTools,
    project: &Project,
    frame: i64,
    scale: f64,
    offline: &HashSet<AssetId>,
    lossless: bool,
) -> Result<Vec<u8>, MediaError> {
    let plan = compile(
        project,
        &RenderRequest {
            start_frame: frame,
            end_frame: frame + 1,
            scale,
            video: true,
            audio: false,
            offline: offline.clone(),
        },
    )
    .map_err(MediaError::Unsupported)?;
    let mut args = plan.args.clone();
    args.extend(["-loglevel".into(), "error".into()]);
    args.extend(map_args(&plan));
    args.extend([
        "-frames:v".into(),
        "1".into(),
        "-f".into(),
        "image2pipe".into(),
    ]);
    if lossless {
        args.extend(["-c:v".into(), "png".into()]);
    } else {
        args.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), "3".into()]);
    }
    args.push("pipe:1".into());
    run_to_bytes(tools, &args, "rendering the preview frame")
}

/// Interleaved signed 16-bit stereo PCM of the timeline mix for `[start, end)`.
pub fn render_audio_pcm(
    tools: &FfmpegTools,
    project: &Project,
    start: i64,
    end: i64,
    offline: &HashSet<AssetId>,
) -> Result<Vec<u8>, MediaError> {
    let plan = compile(
        project,
        &RenderRequest {
            start_frame: start,
            end_frame: end,
            scale: 1.0,
            video: false,
            audio: true,
            offline: offline.clone(),
        },
    )
    .map_err(MediaError::Unsupported)?;
    let mut args = plan.args.clone();
    args.extend(["-loglevel".into(), "error".into()]);
    args.extend(map_args(&plan));
    args.extend([
        "-f".into(),
        "s16le".into(),
        "-c:a".into(),
        "pcm_s16le".into(),
        "-ac".into(),
        "2".into(),
    ]);
    args.extend(["-ar".into(), plan.sample_rate.to_string(), "pipe:1".into()]);
    run_to_bytes(tools, &args, "rendering the audio mix")
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub fraction: f64,
    pub out_seconds: f64,
    pub total_seconds: f64,
    pub speed: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub encoder: String,
    pub elapsed_seconds: f64,
    pub command_line: String,
}

/// Exports the whole sequence to an MP4/MOV file. Writes to a sibling temp file and
/// renames on success, so a cancelled or failed export never leaves a truncated file
/// at `dest`.
pub fn export(
    tools: &FfmpegTools,
    project: &Project,
    dest: &Path,
    settings: &ExportSettings,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(ExportProgress),
) -> Result<ExportResult, MediaError> {
    let end = project.sequence.duration_frames();
    if end == 0 {
        return Err(MediaError::Unsupported(
            "the timeline is empty; add clips before exporting".into(),
        ));
    }
    let ext = dest
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext != "mp4" && ext != "mov" {
        return Err(MediaError::Unsupported(
            "export path must end in .mp4 or .mov".into(),
        ));
    }
    let offline: Vec<String> = project
        .assets
        .iter()
        .filter(|a| !Path::new(&a.path).is_file())
        .map(|a| a.name.clone())
        .collect();
    if !offline.is_empty() {
        return Err(MediaError::Unsupported(format!(
            "relink offline media before exporting: {}",
            offline.join(", ")
        )));
    }
    let plan = compile(
        project,
        &RenderRequest {
            start_frame: 0,
            end_frame: end,
            scale: settings.scale,
            video: true,
            audio: true,
            offline: HashSet::new(),
        },
    )
    .map_err(MediaError::Unsupported)?;
    let (encoder, venc) = video_encoder_args(tools, settings.quality)?;
    let partial: PathBuf = dest.with_file_name(format!(
        ".{}.partial.{ext}",
        dest.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("export")
    ));
    let mut args = plan.args.clone();
    args.extend([
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
    ]);
    args.extend(map_args(&plan));
    args.extend(venc);
    let fps = format!("{}/{}", plan.frame_rate.num(), plan.frame_rate.den());
    args.extend(["-pix_fmt".into(), "yuv420p".into(), "-r".into(), fps]);
    args.extend([
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        format!("{}k", settings.audio_bitrate_kbps),
    ]);
    args.extend([
        "-ar".into(),
        plan.sample_rate.to_string(),
        "-ac".into(),
        "2".into(),
    ]);
    args.extend([
        "-t".into(),
        format!("{:.6}", plan.duration_seconds),
        "-movflags".into(),
        "+faststart".into(),
    ]);
    args.extend([
        "-metadata".into(),
        "encoder=PulseCut".into(),
        "-f".into(),
        if ext == "mov" {
            "mov".into()
        } else {
            "mp4".into()
        },
    ]);
    args.push(partial.to_string_lossy().into_owned());
    let command_line = format!("{} {}", tools.ffmpeg.display(), args.join(" "));

    let started = std::time::Instant::now();
    let mut child = spawn(tools, &args)?;
    let log = drain_stderr(&mut child);
    let stdout = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let total = plan.duration_seconds;
    let mut speed = None;
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&partial);
            return Err(MediaError::Cancelled);
        }
        while let Ok(line) = rx.try_recv() {
            if let Some(v) = line.strip_prefix("speed=") {
                speed = Some(v.trim().to_string());
            }
            if let Some(v) = line.strip_prefix("out_time_us=") {
                if let Ok(us) = v.trim().parse::<i64>() {
                    let out = (us.max(0) as f64) / 1e6;
                    on_progress(ExportProgress {
                        fraction: (out / total).clamp(0.0, 1.0),
                        out_seconds: out,
                        total_seconds: total,
                        speed: speed.clone(),
                    });
                }
            }
        }
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                return Err(MediaError::Render {
                    message: format!("lost track of ffmpeg: {e}"),
                    log: tail(&log),
                })
            }
        }
    };
    if !status.success() {
        let _ = std::fs::remove_file(&partial);
        thread::sleep(Duration::from_millis(50));
        return Err(MediaError::Render {
            message: format!("ffmpeg exited with {status}"),
            log: tail(&log),
        });
    }
    std::fs::rename(&partial, dest).map_err(|e| {
        MediaError::Io(format!(
            "could not move the finished export into place: {e}"
        ))
    })?;
    on_progress(ExportProgress {
        fraction: 1.0,
        out_seconds: total,
        total_seconds: total,
        speed,
    });
    Ok(ExportResult {
        path: dest.to_string_lossy().into_owned(),
        encoder,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        command_line,
    })
}

/// A running real-time preview stream: the same render plan as export, encoded as
/// MJPEG at preview scale and paced to wall-clock time.
pub struct PlaybackStream {
    child: Child,
    stop: Arc<AtomicBool>,
}

impl PlaybackStream {
    /// Starts streaming frames from `start_frame` to the end of the sequence. `on_frame`
    /// receives `(timeline frame index, jpeg bytes)` and runs on a worker thread.
    /// Frames are released no earlier than `lead` before their presentation time,
    /// bounding memory to roughly `lead * fps` frames downstream.
    pub fn start(
        tools: &FfmpegTools,
        project: &Project,
        start_frame: i64,
        scale: f64,
        offline: &HashSet<AssetId>,
        lead: Duration,
        mut on_frame: impl FnMut(i64, Vec<u8>) -> bool + Send + 'static,
        on_end: impl FnOnce(Option<String>) + Send + 'static,
    ) -> Result<Self, MediaError> {
        let end = project.sequence.duration_frames();
        if start_frame >= end {
            return Err(MediaError::Unsupported(
                "the playhead is at the end of the timeline".into(),
            ));
        }
        let plan = compile(
            project,
            &RenderRequest {
                start_frame,
                end_frame: end,
                scale,
                video: true,
                audio: false,
                offline: offline.clone(),
            },
        )
        .map_err(MediaError::Unsupported)?;
        let mut args = plan.args.clone();
        args.extend(["-loglevel".into(), "error".into()]);
        args.extend(map_args(&plan));
        args.extend([
            "-f".into(),
            "image2pipe".into(),
            "-c:v".into(),
            "mjpeg".into(),
            "-q:v".into(),
            "5".into(),
            "pipe:1".into(),
        ]);
        let mut child = spawn(tools, &args)?;
        let log = drain_stderr(&mut child);
        let mut stdout = child.stdout.take().expect("piped stdout");
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let fps = plan.frame_rate.as_f64();
        thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let mut buf: Vec<u8> = Vec::with_capacity(1 << 20);
            let mut chunk = vec![0u8; 256 * 1024];
            let mut index = start_frame;
            let mut scan_from = 0usize;
            'outer: loop {
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
                let n = match stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&chunk[..n]);
                while let Some(end) = find_eoi(&buf, scan_from) {
                    let frame: Vec<u8> = buf.drain(..end).collect();
                    scan_from = 0;
                    let due = Duration::from_secs_f64((index - start_frame) as f64 / fps);
                    let now = t0.elapsed();
                    if due > now + lead {
                        thread::sleep(due - now - lead);
                    }
                    if stop2.load(Ordering::Relaxed) || !on_frame(index, frame) {
                        break 'outer;
                    }
                    index += 1;
                }
                scan_from = buf.len().saturating_sub(1);
            }
            let stopped = stop2.load(Ordering::Relaxed);
            let msg = tail(&log);
            on_end(if stopped || msg.trim().is_empty() {
                None
            } else {
                Some(msg)
            });
        });
        Ok(PlaybackStream { child, stop })
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PlaybackStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
    }
}

/// End index (exclusive) of the first complete JPEG (FFD8 ... FFD9) in `buf`.
fn find_eoi(buf: &[u8], from: usize) -> Option<usize> {
    if buf.len() < 4 || buf[0] != 0xFF || buf[1] != 0xD8 {
        return None;
    }
    let start = from.max(2);
    buf[start..]
        .windows(2)
        .position(|w| w == [0xFF, 0xD9])
        .map(|p| start + p + 2)
}

#[cfg(test)]
mod tests {
    use super::find_eoi;

    #[test]
    fn jpeg_framing() {
        let a = [0xFF, 0xD8, 1, 2, 0xFF, 0xD9, 0xFF, 0xD8];
        assert_eq!(find_eoi(&a, 0), Some(6));
        assert_eq!(find_eoi(&a[..5], 0), None);
    }
}

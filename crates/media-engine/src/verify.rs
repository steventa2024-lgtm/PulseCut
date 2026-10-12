//! Post-export validation: re-probe and fully decode the output file.

use crate::probe::probe;
use crate::tools::{command, FfmpegTools};
use crate::MediaError;
use pulsecut_project_model::Project;
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub pass: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub pass: bool,
    pub checks: Vec<Check>,
}

/// Verifies dimensions, frame rate, duration (±1 frame + one AAC frame), audio
/// presence and that every packet decodes without error.
pub fn verify_export(
    tools: &FfmpegTools,
    project: &Project,
    path: &Path,
    scale: f64,
) -> Result<VerifyReport, MediaError> {
    let info = probe(tools, path)?;
    let seq = &project.sequence;
    let (w, h) = crate::plan::output_size(project, scale);
    let expected_dur = seq.frames_to_seconds(seq.duration_frames()).as_f64();
    let frame = 1.0 / seq.frame_rate.as_f64();
    let mut checks = Vec::new();
    let mut add = |name: &str, expected: String, actual: String, pass: bool| {
        checks.push(Check {
            name: name.into(),
            expected,
            actual,
            pass,
        })
    };
    match &info.video {
        Some(v) => {
            add(
                "Frame size",
                format!("{w}x{h}"),
                format!("{}x{}", v.width, v.height),
                v.width == w && v.height == h,
            );
            add(
                "Frame rate",
                format!("{:.3} fps", seq.frame_rate.as_f64()),
                format!("{:.3} fps", v.frame_rate.as_f64()),
                (v.frame_rate.as_f64() - seq.frame_rate.as_f64()).abs() < 0.01,
            );
            add(
                "Video codec",
                "h264".into(),
                v.codec.clone(),
                v.codec == "h264",
            );
        }
        None => add("Video stream", "present".into(), "missing".into(), false),
    }
    let dur = info.duration.as_f64();
    let tol = frame + 1024.0 / seq.sample_rate as f64 + 0.001;
    add(
        "Duration",
        format!("{expected_dur:.3} s (±{tol:.3})"),
        format!("{dur:.3} s"),
        (dur - expected_dur).abs() <= tol,
    );
    match &info.audio {
        Some(a) => add(
            "Audio",
            format!("{} Hz, 2 ch", seq.sample_rate),
            format!("{} Hz, {} ch ({})", a.sample_rate, a.channels, a.codec),
            a.sample_rate == seq.sample_rate && a.channels == 2,
        ),
        None => add("Audio stream", "present".into(), "missing".into(), false),
    }
    let decode = command(&tools.ffmpeg)
        .args(["-hide_banner", "-v", "error", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()
        .map_err(|e| MediaError::Io(format!("could not run ffmpeg for decode check: {e}")))?;
    let errors = String::from_utf8_lossy(&decode.stderr).trim().to_string();
    add(
        "Full decode",
        "no errors".into(),
        if errors.is_empty() {
            "no errors".into()
        } else {
            errors.lines().take(3).collect::<Vec<_>>().join(" | ")
        },
        decode.status.success() && errors.is_empty(),
    );
    Ok(VerifyReport {
        pass: checks.iter().all(|c| c.pass),
        checks,
    })
}

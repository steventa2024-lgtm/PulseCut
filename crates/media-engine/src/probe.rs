use std::path::{Path, PathBuf};
use std::process::Command;

use project_model::{new_id, Asset};
use serde::Deserialize;

use crate::error::EngineError;
use crate::tools::ffprobe_path;

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub name: String,
    pub duration_us: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps_num: Option<u32>,
    pub fps_den: Option<u32>,
    pub has_video: bool,
    pub has_audio: bool,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub format_name: String,
    pub vfr_suspected: bool,
    pub hdr: bool,
}

impl MediaInfo {
    pub fn into_asset(self) -> Asset {
        Asset {
            id: new_id(),
            name: self.name,
            path: self.path.to_string_lossy().to_string(),
            relative_path: None,
            duration_us: self.duration_us,
            width: self.width,
            height: self.height,
            fps_num: self.fps_num,
            fps_den: self.fps_den,
            has_video: self.has_video,
            has_audio: self.has_audio,
            audio_sample_rate: self.audio_sample_rate,
            audio_channels: self.audio_channels,
            video_codec: self.video_codec,
            audio_codec: self.audio_codec,
            format_name: self.format_name,
            vfr_suspected: self.vfr_suspected,
            hdr: self.hdr,
            offline: false,
        }
    }
}

pub fn probe_file(path: &Path) -> Result<MediaInfo, EngineError> {
    if !path.is_file() {
        return Err(EngineError::msg(format!(
            "File not found: {}",
            path.display()
        )));
    }
    let ffprobe = ffprobe_path()?;
    let output = Command::new(&ffprobe)
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        let extra = if detail.is_empty() {
            "FFmpeg could not read it.".to_string()
        } else {
            truncate(detail, 500)
        };
        return Err(EngineError::msg(format!(
            "Could not read \"{}\". {extra}",
            path.display()
        )));
    }
    let parsed: FfProbe = serde_json::from_slice(&output.stdout).map_err(|err| {
        EngineError::msg(format!(
            "Could not understand the media description for \"{}\": {err}",
            path.display()
        ))
    })?;
    interpret(path, parsed)
}

fn interpret(path: &Path, parsed: FfProbe) -> Result<MediaInfo, EngineError> {
    let mut videos = Vec::new();
    let mut audios = Vec::new();
    for stream in parsed.streams {
        match stream.codec_type.as_deref() {
            Some("video") => videos.push(stream),
            Some("audio") => audios.push(stream),
            _ => {}
        }
    }
    let video_index = videos.iter().position(|stream| {
        stream.codec_name.as_deref() != Some("mjpeg") && stream.width.unwrap_or(0) >= 16
    });
    let video = video_index
        .or_else(|| if videos.is_empty() { None } else { Some(0) })
        .map(|index| videos.swap_remove(index));
    let audio = if audios.is_empty() {
        None
    } else {
        Some(audios.swap_remove(0))
    };
    if video.is_none() && audio.is_none() {
        return Err(EngineError::msg(format!(
            "\"{}\" has neither a video nor an audio stream PulseCut can use.",
            file_name(path)
        )));
    }
    let duration_us = parsed
        .format
        .duration
        .as_deref()
        .and_then(parse_duration_us)
        .or_else(|| {
            video
                .as_ref()
                .and_then(|s| s.duration.as_deref())
                .and_then(parse_duration_us)
        })
        .or_else(|| {
            audio
                .as_ref()
                .and_then(|s| s.duration.as_deref())
                .and_then(parse_duration_us)
        })
        .ok_or_else(|| {
            EngineError::msg(format!(
                "\"{}\" has no duration. PulseCut cannot place it on a timeline.",
                file_name(path)
            ))
        })?;
    if duration_us <= 0 {
        return Err(EngineError::msg(format!(
            "\"{}\" has a zero duration.",
            file_name(path)
        )));
    }

    let (fps_num, fps_den, vfr) = if let Some(video) = &video {
        let avg = video.avg_frame_rate.as_deref().and_then(parse_ratio);
        let raw = video.r_frame_rate.as_deref().and_then(parse_ratio);
        let chosen = avg.or(raw);
        let vfr = match (avg, raw) {
            (Some((an, ad)), Some((rn, rd))) => rates_differ(an, ad, rn, rd),
            _ => false,
        };
        (chosen.map(|(n, _)| n), chosen.map(|(_, d)| d), vfr)
    } else {
        (None, None, false)
    };

    let hdr = video.as_ref().is_some_and(is_hdr);
    let name = file_name(path);
    let width = video.as_ref().and_then(|stream| stream.width);
    let height = video.as_ref().and_then(|stream| stream.height);
    let has_video = video.is_some();
    let has_audio = audio.is_some();
    let audio_sample_rate = audio
        .as_ref()
        .and_then(|stream| stream.sample_rate.as_deref())
        .and_then(|text| text.parse().ok());
    let audio_channels = audio.as_ref().and_then(|stream| stream.channels);
    let video_codec = video.and_then(|stream| stream.codec_name);
    let audio_codec = audio.and_then(|stream| stream.codec_name);

    Ok(MediaInfo {
        path: path.to_path_buf(),
        name,
        duration_us,
        width,
        height,
        fps_num,
        fps_den,
        has_video,
        has_audio,
        audio_sample_rate,
        audio_channels,
        video_codec,
        audio_codec,
        format_name: parsed
            .format
            .format_name
            .unwrap_or_else(|| "unknown".into()),
        vfr_suspected: vfr,
        hdr,
    })
}

fn is_hdr(stream: &FfStream) -> bool {
    let transfer = stream.color_transfer.as_deref();
    let primaries = stream.color_primaries.as_deref();
    matches!(transfer, Some("smpte2084" | "arib-std-b67")) || matches!(primaries, Some("bt2020"))
}

fn rates_differ(an: u32, ad: u32, rn: u32, rd: u32) -> bool {
    let a = f64::from(an) / f64::from(ad);
    let b = f64::from(rn) / f64::from(rd);
    if a == 0.0 || b == 0.0 {
        return false;
    }
    (a - b).abs() / a.max(b) > 0.01
}

fn parse_ratio(text: &str) -> Option<(u32, u32)> {
    let (num, den) = text.split_once('/')?;
    let num: u32 = num.parse().ok()?;
    let den: u32 = den.parse().ok()?;
    if num == 0 || den == 0 {
        return None;
    }
    Some((num, den))
}

fn parse_duration_us(text: &str) -> Option<i64> {
    let seconds: f64 = text.parse().ok()?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    Some((seconds * 1_000_000.0).round() as i64)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "media".into())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

#[derive(Debug, Deserialize)]
struct FfProbe {
    streams: Vec<FfStream>,
    format: FfFormat,
}

#[derive(Debug, Deserialize)]
struct FfFormat {
    duration: Option<String>,
    format_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    duration: Option<String>,
}

use crate::tools::{command, FfmpegTools};
use crate::MediaError;
use pulsecut_project_model::{AudioStreamInfo, MediaInfo, MediaKind, Rational, VideoStreamInfo};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct ProbeOut {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Format {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    pix_fmt: Option<String>,
    color_transfer: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    duration: Option<String>,
    nb_frames: Option<String>,
    #[serde(default)]
    disposition: Disposition,
    #[serde(default)]
    side_data_list: Vec<serde_json::Value>,
    #[serde(default)]
    tags: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, Default)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

const IMAGE_CODECS: &[&str] = &["png", "mjpeg", "webp", "bmp", "tiff", "gif", "jpegxl"];
const IMAGE_CONTAINERS: &[&str] = &[
    "png_pipe",
    "jpeg_pipe",
    "webp_pipe",
    "bmp_pipe",
    "tiff_pipe",
    "image2",
    "jpegxl_pipe",
];

/// Probes a media file. Errors are user-presentable and say what to do next.
pub fn probe(tools: &FfmpegTools, path: &Path) -> Result<MediaInfo, MediaError> {
    let meta = std::fs::metadata(path).map_err(|e| MediaError::Probe {
        path: path.display().to_string(),
        message: format!("cannot read file: {e}"),
    })?;
    if !meta.is_file() {
        return Err(MediaError::Probe {
            path: path.display().to_string(),
            message: "not a regular file".into(),
        });
    }
    let out = command(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| MediaError::Probe {
            path: path.display().to_string(),
            message: format!("could not run ffprobe: {e}"),
        })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().last().unwrap_or("unknown error").trim();
        return Err(MediaError::Probe {
            path: path.display().to_string(),
            message: format!("the file is damaged or in an unsupported format ({first})"),
        });
    }
    let parsed: ProbeOut = serde_json::from_slice(&out.stdout).map_err(|e| MediaError::Probe {
        path: path.display().to_string(),
        message: format!("unexpected ffprobe output: {e}"),
    })?;
    interpret(parsed, meta.len()).map_err(|message| MediaError::Probe {
        path: path.display().to_string(),
        message,
    })
}

fn interpret(p: ProbeOut, size_bytes: u64) -> Result<MediaInfo, String> {
    let container = p
        .format
        .as_ref()
        .and_then(|f| f.format_name.clone())
        .unwrap_or_default();
    let vstream = p
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0);
    let astreams: Vec<&Stream> = p
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .collect();

    let is_image = vstream.is_some_and(|v| {
        let codec = v.codec_name.as_deref().unwrap_or_default();
        IMAGE_CONTAINERS
            .iter()
            .any(|c| container.split(',').any(|x| x == *c))
            || (IMAGE_CODECS.contains(&codec)
                && v.nb_frames.as_deref().is_none_or(|n| n == "1")
                && astreams.is_empty()
                && container != "gif")
    });

    let video = match vstream {
        Some(v) => {
            let (w, h) = (v.width.unwrap_or(0), v.height.unwrap_or(0));
            if w == 0 || h == 0 {
                return Err("video stream has no frame size".into());
            }
            let r = v
                .r_frame_rate
                .as_deref()
                .and_then(Rational::parse)
                .filter(|r| r.is_positive());
            let avg = v
                .avg_frame_rate
                .as_deref()
                .and_then(Rational::parse)
                .filter(|r| r.is_positive());
            let frame_rate = avg.or(r).unwrap_or(Rational::from_int(25));
            let vfr = matches!((r, avg), (Some(a), Some(b)) if (a.as_f64() - b.as_f64()).abs() > 0.01 * a.as_f64());
            let rotation = v
                .side_data_list
                .iter()
                .find_map(|s| s.get("rotation").and_then(|r| r.as_f64()))
                .or_else(|| {
                    v.tags
                        .get("rotate")
                        .and_then(|r| r.as_str())
                        .and_then(|s| s.parse().ok())
                })
                .map(|r| (r.round() as i32).rem_euclid(360))
                .unwrap_or(0);
            let hdr = matches!(
                v.color_transfer.as_deref(),
                Some("smpte2084") | Some("arib-std-b67")
            );
            Some(VideoStreamInfo {
                codec: v.codec_name.clone().unwrap_or_default(),
                width: w,
                height: h,
                frame_rate,
                pixel_format: v.pix_fmt.clone(),
                variable_frame_rate: vfr && !is_image,
                rotation,
                hdr,
            })
        }
        None => None,
    };
    let audio = astreams.first().map(|a| AudioStreamInfo {
        codec: a.codec_name.clone().unwrap_or_default(),
        sample_rate: a
            .sample_rate
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(48000),
        channels: a.channels.unwrap_or(2),
    });
    if video.is_none() && audio.is_none() {
        return Err("the file contains no video or audio streams PulseCut can use".into());
    }
    let duration = p
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| vstream.and_then(|v| v.duration.as_deref()))
        .and_then(Rational::parse)
        .unwrap_or(Rational::ZERO);
    let kind = if is_image {
        MediaKind::Image
    } else if video.is_some() {
        MediaKind::Video
    } else {
        MediaKind::Audio
    };
    if kind != MediaKind::Image && !duration.is_positive() {
        return Err("the file reports no duration (it may be truncated or a live stream)".into());
    }
    Ok(MediaInfo {
        kind,
        container,
        duration: if kind == MediaKind::Image {
            Rational::ZERO
        } else {
            duration
        },
        size_bytes,
        video,
        audio,
        audio_stream_count: astreams.len() as u32,
    })
}

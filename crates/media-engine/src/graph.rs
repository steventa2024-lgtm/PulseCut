use std::collections::HashMap;
use std::path::{Path, PathBuf};

use project_model::{format_seconds, Asset, Clip, Project, TrackKind};

use crate::error::EngineError;

const PRESETS: &[&str] = &[
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
];

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub preset: String,
    pub crf: u8,
    pub audio_bitrate_kbps: u32,
}

impl RenderSettings {
    pub fn export_from_sequence(project: &Project) -> Self {
        Self {
            width: project.sequence.width,
            height: project.sequence.height,
            fps_num: project.sequence.fps_num,
            fps_den: project.sequence.fps_den,
            preset: "veryfast".to_string(),
            crf: 18,
            audio_bitrate_kbps: 160,
        }
    }

    pub fn preview_from_sequence(project: &Project) -> Self {
        Self {
            width: even_half(project.sequence.width),
            height: even_half(project.sequence.height),
            fps_num: project.sequence.fps_num,
            fps_den: project.sequence.fps_den,
            preset: "ultrafast".to_string(),
            crf: 28,
            audio_bitrate_kbps: 96,
        }
    }

    pub fn cache_token(&self) -> String {
        format!(
            "{}x{}@{}/{}:{}:crf{}:a{}",
            self.width,
            self.height,
            self.fps_num,
            self.fps_den,
            self.preset,
            self.crf,
            self.audio_bitrate_kbps
        )
    }
}

fn even_half(value: u32) -> u32 {
    let half = (value / 2).max(2);
    half - (half % 2)
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
    pub inputs: Vec<PathBuf>,
    pub filter: String,
    pub duration_us: i64,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
}

pub fn build_plan(
    project: &Project,
    settings: &RenderSettings,
    check_files: bool,
) -> Result<RenderPlan, EngineError> {
    validate_settings(settings)?;
    let duration_us = project.duration_us();
    if duration_us <= 0 {
        return Err(EngineError::msg(
            "The timeline is empty. Add a clip before rendering.",
        ));
    }

    let mut video_clips: Vec<ClipUse> = Vec::new();
    let mut audio_clips: Vec<ClipUse> = Vec::new();
    for track in &project.sequence.tracks {
        if track.muted && track.kind == TrackKind::Video {
            continue;
        }
        if track.muted && track.kind == TrackKind::Audio {
            continue;
        }
        let mut clips: Vec<&Clip> = track.clips.iter().collect();
        clips.sort_by(|a, b| {
            a.timeline_start_us
                .cmp(&b.timeline_start_us)
                .then_with(|| a.id.cmp(&b.id))
        });
        for clip in clips {
            let asset = project.asset(&clip.asset_id).ok_or_else(|| {
                EngineError::msg(format!(
                    "Clip \"{}\" points at media that is not in the project.",
                    clip.name
                ))
            })?;
            if check_files {
                ensure_media_present(asset)?;
            }
            match track.kind {
                TrackKind::Video => {
                    if !asset.has_video {
                        return Err(EngineError::msg(format!(
                            "\"{}\" is on a video track but the file has no video stream.",
                            asset.name
                        )));
                    }
                    video_clips.push(ClipUse {
                        clip: clip.clone(),
                        asset_id: asset.id.clone(),
                    });
                }
                TrackKind::Audio => {
                    if !asset.has_audio {
                        return Err(EngineError::msg(format!(
                            "\"{}\" is on an audio track but the file has no audio stream.",
                            asset.name
                        )));
                    }
                    audio_clips.push(ClipUse {
                        clip: clip.clone(),
                        asset_id: asset.id.clone(),
                    });
                }
            }
        }
    }

    if video_clips.is_empty() && audio_clips.is_empty() {
        return Err(EngineError::msg(
            "Every track in this timeline is muted, so there is nothing to render.",
        ));
    }

    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut input_of: HashMap<String, usize> = HashMap::new();
    let mut touch = |asset_id: &str, project: &Project| -> Result<usize, EngineError> {
        if let Some(index) = input_of.get(asset_id) {
            return Ok(*index);
        }
        let asset = project
            .asset(asset_id)
            .ok_or_else(|| EngineError::msg("A timeline clip lost its media reference."))?;
        let index = inputs.len();
        inputs.push(PathBuf::from(&asset.path));
        input_of.insert(asset_id.to_string(), index);
        Ok(index)
    };
    for item in video_clips.iter().chain(audio_clips.iter()) {
        touch(&item.asset_id, project)?;
    }

    let mut video_uses: HashMap<usize, usize> = HashMap::new();
    let mut audio_uses: HashMap<usize, usize> = HashMap::new();
    for item in &video_clips {
        let index = input_of[&item.asset_id];
        *video_uses.entry(index).or_insert(0) += 1;
    }
    for item in &audio_clips {
        let index = input_of[&item.asset_id];
        *audio_uses.entry(index).or_insert(0) += 1;
    }

    let mut lines = Vec::new();
    for (index, count) in &video_uses {
        if *count > 1 {
            let pads: String = (0..*count).map(|n| format!("[vs{index}_{n}]")).collect();
            lines.push(format!("[{index}:v]split={count}{pads}"));
        }
    }
    for (index, count) in &audio_uses {
        if *count > 1 {
            let pads: String = (0..*count).map(|n| format!("[as{index}_{n}]")).collect();
            lines.push(format!("[{index}:a]asplit={count}{pads}"));
        }
    }

    let width = settings.width;
    let height = settings.height;
    let fps = format!("{}/{}", settings.fps_num, settings.fps_den);
    let duration = format_seconds(duration_us);
    lines.push(format!(
        "color=c=black:s={width}x{height}:r={fps}:d={duration},format=rgba[base]"
    ));

    let mut video_cursor: HashMap<usize, usize> = HashMap::new();
    let mut current = "base".to_string();
    for (ordinal, item) in video_clips.iter().enumerate() {
        let input = input_of[&item.asset_id];
        let use_index = *video_cursor.entry(input).or_insert(0);
        *video_cursor.get_mut(&input).unwrap() += 1;
        let uses = video_uses[&input];
        let src = if uses == 1 {
            format!("[{input}:v]")
        } else {
            format!("[vs{input}_{use_index}]")
        };
        let label = format!("v{ordinal}");
        let source_in = format_seconds(item.clip.source_in_us);
        let source_out = format_seconds(item.clip.source_in_us + item.clip.duration_us);
        let start = format_seconds(item.clip.timeline_start_us);
        lines.push(format!(
            "{src}trim=start={source_in}:end={source_out},setpts=PTS-STARTPTS,scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:color=black@0,setsar=1,fps={fps},format=rgba,setpts=PTS-STARTPTS+{start}/TB[{label}]"
        ));
        let next = format!("ov{ordinal}");
        lines.push(format!(
            "[{current}][{label}]overlay=format=auto:eof_action=pass:repeatlast=0[{next}]"
        ));
        current = next;
    }
    lines.push(format!("[{current}]format=yuv420p[vout]"));

    if audio_clips.is_empty() {
        lines.push(format!(
            "anullsrc=channel_layout=stereo:sample_rate=48000,atrim=end={duration},aformat=sample_fmts=fltp:channel_layouts=stereo[aout]"
        ));
    } else {
        let mut audio_cursor: HashMap<usize, usize> = HashMap::new();
        let mut labels = Vec::new();
        for (ordinal, item) in audio_clips.iter().enumerate() {
            let input = input_of[&item.asset_id];
            let use_index = *audio_cursor.entry(input).or_insert(0);
            *audio_cursor.get_mut(&input).unwrap() += 1;
            let uses = audio_uses[&input];
            let src = if uses == 1 {
                format!("[{input}:a]")
            } else {
                format!("[as{input}_{use_index}]")
            };
            let label = format!("a{ordinal}");
            let source_in = format_seconds(item.clip.source_in_us);
            let source_out = format_seconds(item.clip.source_in_us + item.clip.duration_us);
            let delay_ms = (item.clip.timeline_start_us.max(0) / 1_000).to_string();
            let volume = linear_from_db(item.clip.gain_db);
            lines.push(format!(
                "{src}atrim=start={source_in}:end={source_out},asetpts=PTS-STARTPTS,aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo,volume={volume:.6},adelay={delay_ms}|{delay_ms}[{label}]"
            ));
            labels.push(format!("[{label}]"));
        }
        if labels.len() == 1 {
            lines.push(format!(
                "{}atrim=end={duration},apad=whole_dur={duration},aformat=sample_fmts=fltp:channel_layouts=stereo[aout]",
                labels[0]
            ));
        } else {
            lines.push(format!(
                "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,atrim=end={duration},apad=whole_dur={duration},aformat=sample_fmts=fltp:channel_layouts=stereo[aout]",
                labels.join(""),
                labels.len()
            ));
        }
    }

    Ok(RenderPlan {
        inputs,
        filter: lines.join(";\n"),
        duration_us,
        width,
        height,
        fps_num: settings.fps_num,
        fps_den: settings.fps_den,
    })
}

struct ClipUse {
    clip: Clip,
    asset_id: String,
}

fn ensure_media_present(asset: &Asset) -> Result<(), EngineError> {
    if asset.offline || !Path::new(&asset.path).is_file() {
        return Err(EngineError::MissingMedia(asset.name.clone()));
    }
    Ok(())
}

fn validate_settings(settings: &RenderSettings) -> Result<(), EngineError> {
    if settings.width < 160
        || settings.height < 160
        || settings.width > 7680
        || settings.height > 4320
        || settings.width % 2 != 0
        || settings.height % 2 != 0
    {
        return Err(EngineError::msg(
            "Export size must be an even size between 160x160 and 7680x4320.",
        ));
    }
    if settings.fps_num == 0 || settings.fps_den == 0 {
        return Err(EngineError::msg("Export frame rate is not valid."));
    }
    if !PRESETS.contains(&settings.preset.as_str()) {
        return Err(EngineError::msg(
            "Choose a supported x264 preset: ultrafast, superfast, veryfast, faster, fast, medium, or slow.",
        ));
    }
    if settings.crf > 51 {
        return Err(EngineError::msg("CRF must be between 0 and 51."));
    }
    if !(32..=512).contains(&settings.audio_bitrate_kbps) {
        return Err(EngineError::msg(
            "Audio bitrate must be between 32 and 512 kbps.",
        ));
    }
    Ok(())
}

fn linear_from_db(db: f64) -> f64 {
    let db = if db.is_finite() { db } else { 0.0 };
    10f64.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use project_model::{Asset, Command, Project, Session};

    fn asset(id: &str, video: bool, audio: bool) -> Asset {
        Asset {
            id: id.into(),
            name: id.into(),
            path: format!("/tmp/{id}.mp4"),
            relative_path: None,
            duration_us: 5_000_000,
            width: Some(1280),
            height: Some(720),
            fps_num: Some(30),
            fps_den: Some(1),
            has_video: video,
            has_audio: audio,
            audio_sample_rate: Some(48_000),
            audio_channels: Some(2),
            video_codec: Some("h264".into()),
            audio_codec: Some("aac".into()),
            format_name: "mp4".into(),
            vfr_suspected: false,
            hdr: false,
            offline: false,
        }
    }

    #[test]
    fn graph_contains_trim_overlay_and_gain() {
        let mut session = Session::new_empty("Graph");
        session
            .dispatch(Command::ImportAsset {
                asset: asset("red", true, true),
            })
            .unwrap();
        session
            .dispatch(Command::ImportAsset {
                asset: asset("music", false, true),
            })
            .unwrap();
        let v1 = session.project().sequence.tracks[0].id.clone();
        let a1 = session.project().sequence.tracks[2].id.clone();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: "red".into(),
                video_track_id: Some(v1),
                audio_track_id: Some(a1.clone()),
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: Some(2_000_000),
            })
            .unwrap();
        let audio_id = session.project().sequence.tracks[2].clips[0].id.clone();
        session
            .dispatch(Command::SetClipGain {
                clip_id: audio_id,
                gain_db: -6.0,
            })
            .unwrap();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: "music".into(),
                video_track_id: None,
                audio_track_id: Some(a1),
                timeline_start_us: 1_000_000,
                source_in_us: 0,
                duration_us: Some(2_000_000),
            })
            .unwrap();
        let plan = build_plan(
            session.project(),
            &RenderSettings::export_from_sequence(session.project()),
            false,
        )
        .unwrap();
        assert!(plan.filter.contains("trim=start=0.000000:end=2.000000"));
        assert!(plan.filter.contains("overlay=format=auto"));
        assert!(plan.filter.contains("adelay=1000|1000"));
        assert!(plan.filter.contains("amix=inputs=2"));
        assert!(plan.filter.contains("volume=0.501187"));
        assert_eq!(plan.inputs.len(), 2);
        assert_eq!(plan.duration_us, 3_000_000);
    }

    #[test]
    fn missing_file_is_reported() {
        let project = Project::empty("Missing");
        let mut session = Session::new(project).unwrap();
        let mut missing = asset("gone", true, false);
        missing.path = "/tmp/pulsecut-does-not-exist-cd9f.mp4".into();
        session
            .dispatch(Command::ImportAsset { asset: missing })
            .unwrap();
        let video = session.project().sequence.tracks[0].id.clone();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: "gone".into(),
                video_track_id: Some(video),
                audio_track_id: None,
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: Some(1_000_000),
            })
            .unwrap();
        let err = build_plan(
            session.project(),
            &RenderSettings::export_from_sequence(session.project()),
            true,
        )
        .unwrap_err();
        assert!(matches!(err, EngineError::MissingMedia(_)));
    }

    #[test]
    fn muted_video_track_is_omitted() {
        let mut session = Session::new_empty("Mute");
        session
            .dispatch(Command::ImportAsset {
                asset: asset("red", true, false),
            })
            .unwrap();
        let v1 = session.project().sequence.tracks[0].id.clone();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: "red".into(),
                video_track_id: Some(v1.clone()),
                audio_track_id: None,
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: Some(1_000_000),
            })
            .unwrap();
        session
            .dispatch(Command::SetTrackMuted {
                track_id: v1,
                muted: true,
            })
            .unwrap();
        let err = build_plan(
            session.project(),
            &RenderSettings::preview_from_sequence(session.project()),
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("muted"));
    }
}

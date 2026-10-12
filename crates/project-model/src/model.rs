use crate::time::{frames_to_seconds, seconds_to_frames_floor, Rational};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;
pub const FILE_FORMAT: &str = "pulsecut.project";

pub type AssetId = Uuid;
pub type ClipId = Uuid;
pub type TrackId = Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub format: String,
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub assets: Vec<Asset>,
    pub sequence: Sequence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoStreamInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: Rational,
    pub pixel_format: Option<String>,
    /// `r_frame_rate` and `avg_frame_rate` disagree, so the stream is probably VFR.
    pub variable_frame_rate: bool,
    /// Rotation in degrees from display-matrix side data (0 when absent).
    pub rotation: i32,
    /// PQ / HLG transfer characteristics detected; preview/export are SDR-only.
    pub hdr: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStreamInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub kind: MediaKind,
    pub container: String,
    pub duration: Rational,
    pub size_bytes: u64,
    pub video: Option<VideoStreamInfo>,
    pub audio: Option<AudioStreamInfo>,
    pub audio_stream_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    /// Absolute path at last successful resolution.
    pub path: String,
    /// Path relative to the project file directory (written on save, used for relinking
    /// when a project folder is moved).
    pub relative_path: Option<String>,
    pub media: MediaInfo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    pub id: Uuid,
    pub name: String,
    pub frame_rate: Rational,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    /// Video tracks composite bottom-to-top in vector order (first video track is lowest).
    pub tracks: Vec<Track>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    pub muted: bool,
    pub locked: bool,
    pub solo: bool,
    /// Sorted by `start`, never overlapping.
    pub clips: Vec<Clip>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipParams {
    /// 0.0 – 1.0
    pub opacity: f64,
    /// Uniform scale applied after fit-to-frame (1.0 = fit).
    pub scale: f64,
    /// Offset of the clip centre from the frame centre, in sequence pixels.
    pub position_x: f64,
    pub position_y: f64,
    /// Linear gain in dB applied to the clip's audio.
    pub gain_db: f64,
}

impl Default for ClipParams {
    fn default() -> Self {
        ClipParams {
            opacity: 1.0,
            scale: 1.0,
            position_x: 0.0,
            position_y: 0.0,
            gain_db: 0.0,
        }
    }
}

impl ClipParams {
    pub fn validate(&self) -> Result<(), String> {
        let finite = [
            self.opacity,
            self.scale,
            self.position_x,
            self.position_y,
            self.gain_db,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite {
            return Err("clip parameters must be finite numbers".into());
        }
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err("opacity must be between 0 and 1".into());
        }
        if !(0.01..=10.0).contains(&self.scale) {
            return Err("scale must be between 1% and 1000%".into());
        }
        if self.position_x.abs() > 100_000.0 || self.position_y.abs() > 100_000.0 {
            return Err("position is out of range".into());
        }
        if !(-60.0..=24.0).contains(&self.gain_db) {
            return Err("gain must be between -60 dB and +24 dB".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: ClipId,
    pub asset_id: AssetId,
    /// Timeline start, in sequence frames.
    pub start: i64,
    /// Length on the timeline, in sequence frames (> 0).
    pub duration: i64,
    /// Source-relative in-point, in exact seconds.
    pub source_in: Rational,
    /// Clips sharing a link id (e.g. a video clip and its audio) are edited together.
    pub link: Option<Uuid>,
    pub params: ClipParams,
}

impl Clip {
    pub fn end(&self) -> i64 {
        self.start + self.duration
    }
}

impl Project {
    pub fn new(name: impl Into<String>, sequence: Sequence) -> Self {
        Project {
            format: FILE_FORMAT.into(),
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: name.into(),
            assets: Vec::new(),
            sequence,
        }
    }

    pub fn asset(&self, id: AssetId) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }
}

impl Sequence {
    pub fn new(
        name: impl Into<String>,
        width: u32,
        height: u32,
        frame_rate: Rational,
        sample_rate: u32,
    ) -> Self {
        let track = |kind, name: &str| Track {
            id: Uuid::new_v4(),
            kind,
            name: name.into(),
            muted: false,
            locked: false,
            solo: false,
            clips: Vec::new(),
        };
        Sequence {
            id: Uuid::new_v4(),
            name: name.into(),
            frame_rate,
            width,
            height,
            sample_rate,
            tracks: vec![
                track(TrackKind::Video, "V1"),
                track(TrackKind::Video, "V2"),
                track(TrackKind::Audio, "A1"),
                track(TrackKind::Audio, "A2"),
            ],
        }
    }

    /// End of the last clip, in frames.
    pub fn duration_frames(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(Clip::end))
            .max()
            .unwrap_or(0)
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Returns `(track index, clip index)`.
    pub fn locate_clip(&self, id: ClipId) -> Option<(usize, usize)> {
        self.tracks
            .iter()
            .enumerate()
            .find_map(|(ti, t)| t.clips.iter().position(|c| c.id == id).map(|ci| (ti, ci)))
    }

    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.locate_clip(id).map(|(t, c)| &self.tracks[t].clips[c])
    }

    pub fn frames_to_seconds(&self, frames: i64) -> Rational {
        frames_to_seconds(frames, self.frame_rate)
    }

    /// Max number of sequence frames available in `asset` starting at `source_in`.
    pub fn available_frames(&self, asset: &Asset, source_in: Rational) -> Option<i64> {
        if asset.media.kind == MediaKind::Image {
            return None;
        }
        let remaining = asset.media.duration.checked_sub(source_in)?;
        Some(seconds_to_frames_floor(remaining, self.frame_rate))
    }
}

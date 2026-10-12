use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::time::Rational;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub path: String,
    pub relative_path: Option<String>,
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
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub asset_id: String,
    pub timeline_start_us: i64,
    pub source_in_us: i64,
    pub duration_us: i64,
    pub name: String,
    /// Clips that share a link id move, trim, split, and delete together.
    pub link_id: Option<String>,
    pub gain_db: f64,
}

impl Clip {
    pub fn end_us(&self) -> i64 {
        self.timeline_start_us.saturating_add(self.duration_us)
    }

    pub fn contains_us(&self, time_us: i64) -> bool {
        time_us >= self.timeline_start_us && time_us < self.end_us()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub muted: bool,
    pub locked: bool,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub tracks: Vec<Track>,
}

impl Sequence {
    pub fn frame_rate(&self) -> Result<Rational, ProjectError> {
        Rational::new(self.fps_num, self.fps_den)
    }

    pub fn duration_us(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(Clip::end_us)
            .max()
            .unwrap_or(0)
    }
}

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub assets: Vec<Asset>,
    pub sequence: Sequence,
}

impl Project {
    pub fn empty(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            schema_version: SCHEMA_VERSION,
            id: new_id(),
            name: name.clone(),
            assets: Vec::new(),
            sequence: Sequence {
                id: new_id(),
                name: "Sequence 1".to_string(),
                width: 1920,
                height: 1080,
                fps_num: 30,
                fps_den: 1,
                tracks: vec![
                    Track {
                        id: new_id(),
                        name: "V1".to_string(),
                        kind: TrackKind::Video,
                        muted: false,
                        locked: false,
                        clips: Vec::new(),
                    },
                    Track {
                        id: new_id(),
                        name: "V2".to_string(),
                        kind: TrackKind::Video,
                        muted: false,
                        locked: false,
                        clips: Vec::new(),
                    },
                    Track {
                        id: new_id(),
                        name: "A1".to_string(),
                        kind: TrackKind::Audio,
                        muted: false,
                        locked: false,
                        clips: Vec::new(),
                    },
                    Track {
                        id: new_id(),
                        name: "A2".to_string(),
                        kind: TrackKind::Audio,
                        muted: false,
                        locked: false,
                        clips: Vec::new(),
                    },
                ],
            },
        }
    }

    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn duration_us(&self) -> i64 {
        self.sequence.duration_us()
    }

    /// Highest video track wins. Tracks earlier in the list are underneath.
    pub fn top_video_clip_at(&self, time_us: i64) -> Option<&Clip> {
        self.sequence
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Video && !track.muted)
            .rev()
            .find_map(|track| {
                track
                    .clips
                    .iter()
                    .filter(|clip| clip.contains_us(time_us))
                    .max_by_key(|clip| (clip.timeline_start_us, clip.id.clone()))
            })
    }
}

pub fn canonical_json(project: &Project) -> Result<String, ProjectError> {
    Ok(serde_json::to_string(project)?)
}

pub fn to_json_pretty(project: &Project) -> Result<String, ProjectError> {
    Ok(serde_json::to_string_pretty(project)?)
}

pub fn from_json(text: &str) -> Result<Project, ProjectError> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let version = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ProjectError::msg(
                "This file has no schemaVersion field, so it is not a PulseCut project.",
            )
        })?;
    if version != u64::from(SCHEMA_VERSION) {
        return Err(ProjectError::msg(format!(
            "This project uses schema version {version}. PulseCut 0.1.0 opens schema version {SCHEMA_VERSION} only."
        )));
    }
    let project: Project = serde_json::from_value(value)?;
    validate_structure(&project)?;
    Ok(project)
}

pub fn validate_structure(project: &Project) -> Result<(), ProjectError> {
    if project.schema_version != SCHEMA_VERSION {
        return Err(ProjectError::msg("Unsupported project schema version."));
    }
    let fps = project.sequence.frame_rate()?;
    validate_dimensions(project.sequence.width, project.sequence.height)?;
    let mut ids = std::collections::BTreeSet::new();
    for track in &project.sequence.tracks {
        if !ids.insert(track.id.clone()) {
            return Err(ProjectError::msg("Duplicate track id in the project."));
        }
        for clip in &track.clips {
            if !ids.insert(clip.id.clone()) {
                return Err(ProjectError::msg("Duplicate clip id in the project."));
            }
            if clip.duration_us <= 0 {
                return Err(ProjectError::msg(format!(
                    "Clip \"{}\" has no duration.",
                    clip.name
                )));
            }
            if clip.timeline_start_us < 0 || clip.source_in_us < 0 {
                return Err(ProjectError::msg(format!(
                    "Clip \"{}\" has a negative time.",
                    clip.name
                )));
            }
            if !clip.gain_db.is_finite() {
                return Err(ProjectError::msg(format!(
                    "Clip \"{}\" has a gain value that is not a real number.",
                    clip.name
                )));
            }
            if project.asset(&clip.asset_id).is_none() {
                return Err(ProjectError::msg(format!(
                    "Clip \"{}\" points at missing asset {}.",
                    clip.name, clip.asset_id
                )));
            }
        }
    }
    for asset in &project.assets {
        if !ids.insert(asset.id.clone()) {
            return Err(ProjectError::msg("Duplicate asset id in the project."));
        }
        validate_media_path(&asset.path)?;
        if asset.duration_us <= 0 {
            return Err(ProjectError::msg(format!(
                "Asset \"{}\" has no duration.",
                asset.name
            )));
        }
    }
    let _ = fps;
    Ok(())
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), ProjectError> {
    if !(160..=7680).contains(&width) || !(160..=4320).contains(&height) {
        return Err(ProjectError::msg(
            "Sequence size must be between 160x160 and 7680x4320.",
        ));
    }
    if width % 2 != 0 || height % 2 != 0 {
        return Err(ProjectError::msg(
            "Sequence width and height must be even numbers.",
        ));
    }
    Ok(())
}

pub fn validate_media_path(path: &str) -> Result<(), ProjectError> {
    if path.trim().is_empty() || path.contains('\0') {
        return Err(ProjectError::msg("Media path is empty."));
    }
    let lower = path.trim().to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("rtsp://")
    {
        return Err(ProjectError::msg(
            "Remote media URLs are not supported. Import a file from this computer.",
        ));
    }
    Ok(())
}

pub fn sanitize_name(name: &str) -> Result<String, ProjectError> {
    let cleaned: String = name
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(120)
        .collect();
    if cleaned.is_empty() {
        return Err(ProjectError::msg("Name cannot be empty."));
    }
    Ok(cleaned)
}

/// Point absolute paths at the project folder when possible, so the file can move with its media.
pub fn prepare_for_save(project: &mut Project, project_dir: &std::path::Path) {
    for asset in &mut project.assets {
        let path = std::path::Path::new(&asset.path);
        asset.relative_path = path.strip_prefix(project_dir).ok().map(|rel| {
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/")
        });
    }
}

/// Restore asset paths after a project file moves. Returns the names of assets that are still missing.
pub fn resolve_after_load(project: &mut Project, project_dir: &std::path::Path) -> Vec<String> {
    let mut missing = Vec::new();
    for asset in &mut project.assets {
        let absolute = std::path::Path::new(&asset.path);
        if absolute.is_file() {
            asset.offline = false;
            continue;
        }
        if let Some(rel) = asset.relative_path.clone() {
            let candidate = project_dir.join(rel);
            if candidate.is_file() {
                asset.path = candidate.to_string_lossy().to_string();
                asset.offline = false;
                continue;
            }
        }
        asset.offline = true;
        missing.push(asset.name.clone());
    }
    missing
}

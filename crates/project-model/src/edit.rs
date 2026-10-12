use serde::{Deserialize, Serialize};

use crate::error::ProjectError;
use crate::model::{
    sanitize_name, validate_dimensions, validate_media_path, validate_structure, Asset, Clip,
    Project, Track, TrackKind,
};
use crate::new_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrimEdge {
    Start,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    #[serde(rename_all = "camelCase")]
    ImportAsset {
        asset: Asset,
    },
    #[serde(rename_all = "camelCase")]
    UpdateAsset {
        asset: Asset,
    },
    #[serde(rename_all = "camelCase")]
    RemoveAsset {
        asset_id: String,
    },
    #[serde(rename_all = "camelCase")]
    PlaceAsset {
        asset_id: String,
        video_track_id: Option<String>,
        audio_track_id: Option<String>,
        timeline_start_us: i64,
        source_in_us: i64,
        duration_us: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    MoveClip {
        clip_id: String,
        timeline_start_us: i64,
    },
    #[serde(rename_all = "camelCase")]
    TrimClip {
        clip_id: String,
        edge: TrimEdge,
        timeline_us: i64,
    },
    #[serde(rename_all = "camelCase")]
    SetClipTiming {
        clip_id: String,
        timeline_start_us: i64,
        source_in_us: i64,
        duration_us: i64,
    },
    #[serde(rename_all = "camelCase")]
    SetClipName {
        clip_id: String,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    SetClipGain {
        clip_id: String,
        gain_db: f64,
    },
    #[serde(rename_all = "camelCase")]
    SplitClip {
        clip_id: String,
        timeline_us: i64,
    },
    #[serde(rename_all = "camelCase")]
    DeleteClip {
        clip_id: String,
    },
    #[serde(rename_all = "camelCase")]
    UnlinkClip {
        clip_id: String,
    },
    #[serde(rename_all = "camelCase")]
    SetTrackMuted {
        track_id: String,
        muted: bool,
    },
    #[serde(rename_all = "camelCase")]
    SetTrackLocked {
        track_id: String,
        locked: bool,
    },
    #[serde(rename_all = "camelCase")]
    AddTrack {
        kind: TrackKind,
        name: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    SetSequence {
        name: Option<String>,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
    },
    Undo,
    Redo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub project: Project,
    pub revision: u64,
    pub can_undo: bool,
    pub can_redo: bool,
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub struct Session {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    clean_json: String,
    revision: u64,
    limit: usize,
}

impl Session {
    pub fn new(project: Project) -> Result<Self, ProjectError> {
        validate_structure(&project)?;
        let clean_json = serde_json::to_string(&project)?;
        Ok(Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            clean_json,
            revision: 1,
            limit: 100,
        })
    }

    pub fn new_empty(name: impl Into<String>) -> Self {
        let project = Project::empty(name);
        let clean_json = serde_json::to_string(&project).expect("empty project serializes");
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
            clean_json,
            revision: 1,
            limit: 100,
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn project_mut(&mut self) -> &mut Project {
        &mut self.project
    }

    pub fn mark_saved(&mut self) -> Result<(), ProjectError> {
        self.clean_json = serde_json::to_string(&self.project)?;
        Ok(())
    }

    pub fn mark_dirty(&mut self) {
        self.clean_json.clear();
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            project: self.project.clone(),
            revision: self.revision,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            dirty: serde_json::to_string(&self.project).ok().as_ref() != Some(&self.clean_json),
        }
    }

    pub fn dispatch(&mut self, command: Command) -> Result<SessionSnapshot, ProjectError> {
        match command {
            Command::Undo => self.undo_one(),
            Command::Redo => self.redo_one(),
            command => {
                let mut next = self.project.clone();
                apply(&mut next, command)?;
                validate_structure(&next)?;
                if next != self.project {
                    self.undo.push(self.project.clone());
                    if self.undo.len() > self.limit {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                    self.project = next;
                    self.revision = self.revision.saturating_add(1);
                }
                Ok(self.snapshot())
            }
        }
    }

    fn undo_one(&mut self) -> Result<SessionSnapshot, ProjectError> {
        let previous = self
            .undo
            .pop()
            .ok_or_else(|| ProjectError::msg("Nothing to undo."))?;
        self.redo.push(self.project.clone());
        self.project = previous;
        self.revision = self.revision.saturating_add(1);
        Ok(self.snapshot())
    }

    fn redo_one(&mut self) -> Result<SessionSnapshot, ProjectError> {
        let next = self
            .redo
            .pop()
            .ok_or_else(|| ProjectError::msg("Nothing to redo."))?;
        self.undo.push(self.project.clone());
        self.project = next;
        self.revision = self.revision.saturating_add(1);
        Ok(self.snapshot())
    }
}

fn apply(project: &mut Project, command: Command) -> Result<(), ProjectError> {
    match command {
        Command::Undo | Command::Redo => Ok(()),
        Command::ImportAsset { asset } => import_asset(project, asset),
        Command::UpdateAsset { asset } => update_asset(project, asset),
        Command::RemoveAsset { asset_id } => remove_asset(project, &asset_id),
        Command::PlaceAsset {
            asset_id,
            video_track_id,
            audio_track_id,
            timeline_start_us,
            source_in_us,
            duration_us,
        } => place_asset(
            project,
            &asset_id,
            video_track_id.as_deref(),
            audio_track_id.as_deref(),
            timeline_start_us,
            source_in_us,
            duration_us,
        ),
        Command::MoveClip {
            clip_id,
            timeline_start_us,
        } => move_clip(project, &clip_id, timeline_start_us),
        Command::TrimClip {
            clip_id,
            edge,
            timeline_us,
        } => trim_clip(project, &clip_id, edge, timeline_us),
        Command::SetClipTiming {
            clip_id,
            timeline_start_us,
            source_in_us,
            duration_us,
        } => set_timing(
            project,
            &clip_id,
            timeline_start_us,
            source_in_us,
            duration_us,
        ),
        Command::SetClipName { clip_id, name } => set_name(project, &clip_id, &name),
        Command::SetClipGain { clip_id, gain_db } => set_gain(project, &clip_id, gain_db),
        Command::SplitClip {
            clip_id,
            timeline_us,
        } => split_clip(project, &clip_id, timeline_us),
        Command::DeleteClip { clip_id } => delete_clip(project, &clip_id),
        Command::UnlinkClip { clip_id } => unlink_clip(project, &clip_id),
        Command::SetTrackMuted { track_id, muted } => {
            let track = track_mut(project, &track_id)?;
            track.muted = muted;
            Ok(())
        }
        Command::SetTrackLocked { track_id, locked } => {
            let track = track_mut(project, &track_id)?;
            track.locked = locked;
            Ok(())
        }
        Command::AddTrack { kind, name } => add_track(project, kind, name),
        Command::SetSequence {
            name,
            width,
            height,
            fps_num,
            fps_den,
        } => set_sequence(project, name, width, height, fps_num, fps_den),
    }
}

fn import_asset(project: &mut Project, asset: Asset) -> Result<(), ProjectError> {
    validate_media_path(&asset.path)?;
    if asset.duration_us <= 0 {
        return Err(ProjectError::msg(format!(
            "\"{}\" has no duration, so it cannot be imported.",
            asset.name
        )));
    }
    if !asset.has_video && !asset.has_audio {
        return Err(ProjectError::msg(format!(
            "\"{}\" has neither a video nor an audio stream.",
            asset.name
        )));
    }
    if project
        .assets
        .iter()
        .any(|existing| existing.id == asset.id)
    {
        return Err(ProjectError::msg(
            "That media item is already in the project.",
        ));
    }
    if let Some(existing) = project
        .assets
        .iter()
        .find(|existing| paths_equal(&existing.path, &asset.path))
    {
        return Err(ProjectError::msg(format!(
            "\"{}\" is already in the project bin.",
            existing.name
        )));
    }
    project.assets.push(asset);
    Ok(())
}

fn update_asset(project: &mut Project, asset: Asset) -> Result<(), ProjectError> {
    validate_media_path(&asset.path)?;
    let slot = project
        .assets
        .iter_mut()
        .find(|existing| existing.id == asset.id)
        .ok_or_else(|| ProjectError::msg("That media item is not in the project."))?;
    *slot = asset;
    Ok(())
}

fn remove_asset(project: &mut Project, asset_id: &str) -> Result<(), ProjectError> {
    if project
        .sequence
        .tracks
        .iter()
        .any(|track| track.clips.iter().any(|clip| clip.asset_id == asset_id))
    {
        return Err(ProjectError::msg(
            "Remove the timeline clips that use this media before deleting it from the bin.",
        ));
    }
    let before = project.assets.len();
    project.assets.retain(|asset| asset.id != asset_id);
    if project.assets.len() == before {
        return Err(ProjectError::msg("That media item is not in the project."));
    }
    Ok(())
}

fn place_asset(
    project: &mut Project,
    asset_id: &str,
    video_track_id: Option<&str>,
    audio_track_id: Option<&str>,
    timeline_start_us: i64,
    source_in_us: i64,
    duration_us: Option<i64>,
) -> Result<(), ProjectError> {
    if video_track_id.is_none() && audio_track_id.is_none() {
        return Err(ProjectError::msg("Choose a track to place this media on."));
    }
    let asset = project
        .asset(asset_id)
        .cloned()
        .ok_or_else(|| ProjectError::msg("That media item is not in the project."))?;
    if asset.offline {
        return Err(ProjectError::msg(format!(
            "\"{}\" is offline. Relink the file before placing it on the timeline.",
            asset.name
        )));
    }
    let fps = project.sequence.frame_rate()?;
    let start = fps.snap_floor_us(timeline_start_us);
    let source_in = source_in_us.max(0);
    if source_in >= asset.duration_us {
        return Err(ProjectError::msg(
            "The source in-point is at or past the end of the media.",
        ));
    }
    let remaining = asset.duration_us - source_in;
    let requested = duration_us.unwrap_or(remaining).min(remaining);
    let duration = fps.snap_floor_us(requested).max(fps.min_clip_us());
    if duration > remaining + 2_000 {
        return Err(ProjectError::msg(format!(
            "\"{}\" is shorter than the clip you tried to create.",
            asset.name
        )));
    }
    let duration = duration.min(remaining);
    if duration <= 0 {
        return Err(ProjectError::msg("The clip duration is zero."));
    }

    let link_id = if video_track_id.is_some() && audio_track_id.is_some() {
        Some(new_id())
    } else {
        None
    };

    if let Some(track_id) = video_track_id {
        if !asset.has_video {
            return Err(ProjectError::msg(format!(
                "\"{}\" has no video stream to place on a video track.",
                asset.name
            )));
        }
        push_clip(
            project,
            track_id,
            TrackKind::Video,
            &asset,
            start,
            source_in,
            duration,
            link_id.clone(),
        )?;
    }
    if let Some(track_id) = audio_track_id {
        if !asset.has_audio {
            return Err(ProjectError::msg(format!(
                "\"{}\" has no audio stream to place on an audio track.",
                asset.name
            )));
        }
        push_clip(
            project,
            track_id,
            TrackKind::Audio,
            &asset,
            start,
            source_in,
            duration,
            link_id,
        )?;
    }
    Ok(())
}

fn push_clip(
    project: &mut Project,
    track_id: &str,
    kind: TrackKind,
    asset: &Asset,
    start: i64,
    source_in: i64,
    duration: i64,
    link_id: Option<String>,
) -> Result<(), ProjectError> {
    let track = track_mut(project, track_id)?;
    if track.locked {
        return Err(ProjectError::msg(format!(
            "Track \"{}\" is locked. Unlock it before adding clips.",
            track.name
        )));
    }
    if track.kind != kind {
        return Err(ProjectError::msg(format!(
            "Track \"{}\" is not a {} track.",
            track.name,
            match kind {
                TrackKind::Video => "video",
                TrackKind::Audio => "audio",
            }
        )));
    }
    track.clips.push(Clip {
        id: new_id(),
        asset_id: asset.id.clone(),
        timeline_start_us: start,
        source_in_us: source_in,
        duration_us: duration,
        name: asset.name.clone(),
        link_id,
        gain_db: 0.0,
    });
    Ok(())
}

fn move_clip(
    project: &mut Project,
    clip_id: &str,
    timeline_start_us: i64,
) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let fps = project.sequence.frame_rate()?;
    let (pti, pci) = clip_addr(project, clip_id)?;
    let primary_start = project.sequence.tracks[pti].clips[pci].timeline_start_us;
    let mut delta = fps.snap_floor_us(timeline_start_us) - primary_start;
    let earliest = group
        .iter()
        .map(|(ti, ci)| project.sequence.tracks[*ti].clips[*ci].timeline_start_us)
        .min()
        .unwrap_or(0);
    if earliest + delta < 0 {
        delta = -earliest;
    }
    for (ti, ci) in group {
        let clip = &mut project.sequence.tracks[ti].clips[ci];
        clip.timeline_start_us += delta;
    }
    Ok(())
}

fn trim_clip(
    project: &mut Project,
    clip_id: &str,
    edge: TrimEdge,
    timeline_us: i64,
) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let fps = project.sequence.frame_rate()?;
    let time = fps.snap_floor_us(timeline_us);
    let (pti, pci) = clip_addr(project, clip_id)?;
    let primary = project.sequence.tracks[pti].clips[pci].clone();
    let min_us = fps.min_clip_us();
    match edge {
        TrimEdge::Start => {
            if time >= primary.end_us() - min_us + 1 && time >= primary.end_us() {
                return Err(ProjectError::msg(
                    "The new in-point leaves no media in the clip.",
                ));
            }
            let new_start = time.min(primary.end_us() - min_us).max(0);
            if new_start < primary.timeline_start_us
                && primary.source_in_us < primary.timeline_start_us - new_start
            {
                return Err(ProjectError::msg(
                    "The trim would start before the beginning of the media.",
                ));
            }
            let delta = new_start - primary.timeline_start_us;
            let new_duration = primary.duration_us - delta;
            if new_duration < min_us {
                return Err(ProjectError::msg("A clip must be at least one frame long."));
            }
            apply_timing_delta(project, &group, delta, delta, new_duration)?;
        }
        TrimEdge::End => {
            let new_end = time.max(primary.timeline_start_us + min_us);
            let new_duration = new_end - primary.timeline_start_us;
            apply_timing_delta(project, &group, 0, 0, new_duration)?;
        }
    }
    Ok(())
}

fn set_timing(
    project: &mut Project,
    clip_id: &str,
    timeline_start_us: i64,
    source_in_us: i64,
    duration_us: i64,
) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let fps = project.sequence.frame_rate()?;
    let (pti, pci) = clip_addr(project, clip_id)?;
    let primary = project.sequence.tracks[pti].clips[pci].clone();
    let new_start = fps.snap_floor_us(timeline_start_us);
    let new_source = source_in_us.max(0);
    let new_duration = fps.snap_floor_us(duration_us).max(fps.min_clip_us());
    let start_delta = new_start - primary.timeline_start_us;
    let source_delta = new_source - primary.source_in_us;
    apply_timing_delta(project, &group, start_delta, source_delta, new_duration)
}

fn apply_timing_delta(
    project: &mut Project,
    group: &[(usize, usize)],
    start_delta: i64,
    source_delta: i64,
    new_duration: i64,
) -> Result<(), ProjectError> {
    if new_duration <= 0 {
        return Err(ProjectError::msg("A clip must be at least one frame long."));
    }
    for &(ti, ci) in group {
        let clip = &project.sequence.tracks[ti].clips[ci];
        let asset = project.asset(&clip.asset_id).ok_or_else(|| {
            ProjectError::msg(format!("Clip \"{}\" points at missing media.", clip.name))
        })?;
        let start = clip.timeline_start_us + start_delta;
        let source = clip.source_in_us + source_delta;
        if start < 0 {
            return Err(ProjectError::msg(
                "That edit would move a clip before the start of the timeline.",
            ));
        }
        if source < 0 {
            return Err(ProjectError::msg(
                "That edit would start before the beginning of the media.",
            ));
        }
        if source + new_duration > asset.duration_us + 2_000 {
            return Err(ProjectError::msg(format!(
                "\"{}\" does not contain enough media for that trim.",
                asset.name
            )));
        }
    }
    for &(ti, ci) in group {
        let clip = &mut project.sequence.tracks[ti].clips[ci];
        clip.timeline_start_us += start_delta;
        clip.source_in_us += source_delta;
        clip.duration_us = new_duration;
    }
    Ok(())
}

fn set_name(project: &mut Project, clip_id: &str, name: &str) -> Result<(), ProjectError> {
    let name = sanitize_name(name)?;
    let (ti, ci) = clip_addr(project, clip_id)?;
    ensure_unlocked(project, &[(ti, ci)])?;
    project.sequence.tracks[ti].clips[ci].name = name;
    Ok(())
}

fn set_gain(project: &mut Project, clip_id: &str, gain_db: f64) -> Result<(), ProjectError> {
    if !gain_db.is_finite() {
        return Err(ProjectError::msg("Gain must be a real number of decibels."));
    }
    let (ti, ci) = clip_addr(project, clip_id)?;
    ensure_unlocked(project, &[(ti, ci)])?;
    if project.sequence.tracks[ti].kind != TrackKind::Audio {
        return Err(ProjectError::msg(
            "Gain applies to audio clips. Select the linked audio clip to change it.",
        ));
    }
    let clamped = gain_db.clamp(-60.0, 12.0);
    project.sequence.tracks[ti].clips[ci].gain_db = clamped;
    Ok(())
}

fn split_clip(project: &mut Project, clip_id: &str, timeline_us: i64) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let fps = project.sequence.frame_rate()?;
    let time = fps.snap_floor_us(timeline_us);
    let old_link = {
        let (ti, ci) = clip_addr(project, clip_id)?;
        project.sequence.tracks[ti].clips[ci].link_id.clone()
    };
    let new_link = if old_link.is_some() {
        Some(new_id())
    } else {
        None
    };
    let mut created = 0;
    let targets = group.clone();
    for (ti, ci) in targets {
        let clip = project.sequence.tracks[ti].clips[ci].clone();
        if time <= clip.timeline_start_us || time >= clip.end_us() {
            continue;
        }
        let left = time - clip.timeline_start_us;
        let right_source = clip.source_in_us + left;
        let right_duration = clip.duration_us - left;
        if left <= 0 || right_duration <= 0 {
            continue;
        }
        project.sequence.tracks[ti].clips[ci].duration_us = left;
        project.sequence.tracks[ti].clips.push(Clip {
            id: new_id(),
            asset_id: clip.asset_id,
            timeline_start_us: time,
            source_in_us: right_source,
            duration_us: right_duration,
            name: clip.name,
            link_id: new_link.clone(),
            gain_db: clip.gain_db,
        });
        created += 1;
    }
    if created == 0 {
        return Err(ProjectError::msg(
            "The playhead is not inside the selected clip, so there is nothing to split.",
        ));
    }
    Ok(())
}

fn delete_clip(project: &mut Project, clip_id: &str) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let ids: Vec<String> = group
        .iter()
        .map(|(ti, ci)| project.sequence.tracks[*ti].clips[*ci].id.clone())
        .collect();
    for track in &mut project.sequence.tracks {
        track
            .clips
            .retain(|clip| !ids.iter().any(|id| id == &clip.id));
    }
    Ok(())
}

fn unlink_clip(project: &mut Project, clip_id: &str) -> Result<(), ProjectError> {
    let group = group_indices(project, clip_id)?;
    ensure_unlocked(project, &group)?;
    let linked = group.len() > 1
        || project.sequence.tracks[group[0].0].clips[group[0].1]
            .link_id
            .is_some();
    if !linked {
        return Err(ProjectError::msg("The selected clip is not linked."));
    }
    for (ti, ci) in group {
        project.sequence.tracks[ti].clips[ci].link_id = None;
    }
    Ok(())
}

fn add_track(
    project: &mut Project,
    kind: TrackKind,
    name: Option<String>,
) -> Result<(), ProjectError> {
    let count = project
        .sequence
        .tracks
        .iter()
        .filter(|track| track.kind == kind)
        .count();
    if count >= 16 {
        return Err(ProjectError::msg(
            "This sequence already has 16 tracks of that type.",
        ));
    }
    let default_name = match kind {
        TrackKind::Video => format!("V{}", count + 1),
        TrackKind::Audio => format!("A{}", count + 1),
    };
    let name = match name {
        Some(name) => sanitize_name(&name)?,
        None => default_name,
    };
    let track = Track {
        id: new_id(),
        name,
        kind,
        muted: false,
        locked: false,
        clips: Vec::new(),
    };
    match kind {
        TrackKind::Video => {
            let insert_at = project
                .sequence
                .tracks
                .iter()
                .rposition(|track| track.kind == TrackKind::Video)
                .map(|index| index + 1)
                .unwrap_or(0);
            project.sequence.tracks.insert(insert_at, track);
        }
        TrackKind::Audio => project.sequence.tracks.push(track),
    }
    Ok(())
}

fn set_sequence(
    project: &mut Project,
    name: Option<String>,
    width: u32,
    height: u32,
    fps_num: u32,
    fps_den: u32,
) -> Result<(), ProjectError> {
    validate_dimensions(width, height)?;
    let fps = crate::time::Rational::new(fps_num, fps_den)?;
    if let Some(name) = name {
        project.sequence.name = sanitize_name(&name)?;
    }
    project.sequence.width = width;
    project.sequence.height = height;
    project.sequence.fps_num = fps.num;
    project.sequence.fps_den = fps.den;
    Ok(())
}

fn track_mut<'a>(project: &'a mut Project, track_id: &str) -> Result<&'a mut Track, ProjectError> {
    project
        .sequence
        .tracks
        .iter_mut()
        .find(|track| track.id == track_id)
        .ok_or_else(|| ProjectError::msg("That track is not in the sequence."))
}

fn clip_addr(project: &Project, clip_id: &str) -> Result<(usize, usize), ProjectError> {
    for (ti, track) in project.sequence.tracks.iter().enumerate() {
        if let Some(ci) = track.clips.iter().position(|clip| clip.id == clip_id) {
            return Ok((ti, ci));
        }
    }
    Err(ProjectError::msg("That clip is not on the timeline."))
}

fn group_indices(project: &Project, clip_id: &str) -> Result<Vec<(usize, usize)>, ProjectError> {
    let (ti, ci) = clip_addr(project, clip_id)?;
    let link = project.sequence.tracks[ti].clips[ci].link_id.clone();
    let Some(link) = link else {
        return Ok(vec![(ti, ci)]);
    };
    let mut indices = Vec::new();
    for (ti, track) in project.sequence.tracks.iter().enumerate() {
        for (ci, clip) in track.clips.iter().enumerate() {
            if clip.link_id.as_deref() == Some(link.as_str()) {
                indices.push((ti, ci));
            }
        }
    }
    Ok(indices)
}

fn ensure_unlocked(project: &Project, indices: &[(usize, usize)]) -> Result<(), ProjectError> {
    for (ti, _) in indices {
        let track = &project.sequence.tracks[*ti];
        if track.locked {
            return Err(ProjectError::msg(format!(
                "Track \"{}\" is locked. Unlock it before editing clips on it.",
                track.name
            )));
        }
    }
    Ok(())
}

fn paths_equal(a: &str, b: &str) -> bool {
    std::path::Path::new(a) == std::path::Path::new(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video_asset(id: &str, duration_us: i64) -> Asset {
        Asset {
            id: id.to_string(),
            name: format!("asset-{id}"),
            path: format!("/tmp/{id}.mp4"),
            relative_path: None,
            duration_us,
            width: Some(1280),
            height: Some(720),
            fps_num: Some(30),
            fps_den: Some(1),
            has_video: true,
            has_audio: true,
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

    fn session_with_asset() -> (Session, String, String, String) {
        let mut session = Session::new_empty("Test");
        let asset = video_asset("a1", 5_000_000);
        let asset_id = asset.id.clone();
        session.dispatch(Command::ImportAsset { asset }).unwrap();
        let video = session.project().sequence.tracks[0].id.clone();
        let audio = session.project().sequence.tracks[2].id.clone();
        (session, asset_id, video, audio)
    }

    #[test]
    fn place_undo_redo_restores_exact_document() {
        let (mut session, asset_id, video, audio) = session_with_asset();
        session.mark_saved().unwrap();
        let before = serde_json::to_string(session.project()).unwrap();
        session
            .dispatch(Command::PlaceAsset {
                asset_id,
                video_track_id: Some(video),
                audio_track_id: Some(audio),
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: None,
            })
            .unwrap();
        assert_eq!(session.project().sequence.tracks[0].clips.len(), 1);
        assert_eq!(session.project().sequence.tracks[2].clips.len(), 1);
        let link_v = session.project().sequence.tracks[0].clips[0]
            .link_id
            .clone();
        let link_a = session.project().sequence.tracks[2].clips[0]
            .link_id
            .clone();
        assert!(link_v.is_some() && link_v == link_a);
        session.dispatch(Command::Undo).unwrap();
        assert_eq!(serde_json::to_string(session.project()).unwrap(), before);
        assert!(!session.snapshot().dirty);
        session.dispatch(Command::Redo).unwrap();
        assert_eq!(session.project().sequence.tracks[0].clips.len(), 1);
        assert!(session.snapshot().dirty);
    }

    #[test]
    fn split_trim_and_locked_track() {
        let (mut session, asset_id, video, audio) = session_with_asset();
        session
            .dispatch(Command::PlaceAsset {
                asset_id,
                video_track_id: Some(video.clone()),
                audio_track_id: Some(audio),
                timeline_start_us: 100,
                source_in_us: 0,
                duration_us: Some(4_000_000),
            })
            .unwrap();
        let start = session.project().sequence.tracks[0].clips[0].timeline_start_us;
        assert_eq!(start, 0, "timeline start snaps to the frame grid");
        let clip_id = session.project().sequence.tracks[0].clips[0].id.clone();
        session
            .dispatch(Command::SplitClip {
                clip_id: clip_id.clone(),
                timeline_us: 2_000_000,
            })
            .unwrap();
        assert_eq!(session.project().sequence.tracks[0].clips.len(), 2);
        assert_eq!(session.project().sequence.tracks[2].clips.len(), 2);
        let left = &session.project().sequence.tracks[0].clips[0];
        assert_eq!(left.duration_us, 2_000_000);
        let right = session.project().sequence.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.timeline_start_us == 2_000_000)
            .unwrap();
        assert_eq!(right.source_in_us, 2_000_000);
        assert_eq!(right.duration_us, 2_000_000);
        let right_id = right.id.clone();
        session
            .dispatch(Command::TrimClip {
                clip_id: right_id,
                edge: TrimEdge::Start,
                timeline_us: 2_500_000,
            })
            .unwrap();
        let trimmed = session.project().sequence.tracks[0]
            .clips
            .iter()
            .find(|clip| clip.id != clip_id)
            .unwrap();
        assert_eq!(trimmed.timeline_start_us, 2_500_000);
        assert_eq!(trimmed.source_in_us, 2_500_000);
        assert_eq!(trimmed.duration_us, 1_500_000);
        session
            .dispatch(Command::SetTrackLocked {
                track_id: video,
                locked: true,
            })
            .unwrap();
        let err = session
            .dispatch(Command::DeleteClip {
                clip_id: clip_id.clone(),
            })
            .unwrap_err();
        assert!(err.to_string().contains("locked"));
        assert_eq!(session.project().sequence.tracks[0].clips.len(), 2);
    }

    #[test]
    fn delete_removes_linked_group_and_schema_roundtrip() {
        let (mut session, asset_id, video, audio) = session_with_asset();
        session
            .dispatch(Command::PlaceAsset {
                asset_id,
                video_track_id: Some(video),
                audio_track_id: Some(audio),
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: None,
            })
            .unwrap();
        let clip_id = session.project().sequence.tracks[0].clips[0].id.clone();
        let json = crate::to_json_pretty(session.project()).unwrap();
        let reloaded = crate::from_json(&json).unwrap();
        assert_eq!(&reloaded, session.project());
        session.dispatch(Command::DeleteClip { clip_id }).unwrap();
        assert!(session
            .project()
            .sequence
            .tracks
            .iter()
            .all(|t| t.clips.is_empty()));
        let bad = "{\"schemaVersion\":2}";
        assert!(crate::from_json(bad).is_err());
    }

    #[test]
    fn command_json_uses_camel_case() {
        let cmd = Command::MoveClip {
            clip_id: "c".into(),
            timeline_start_us: 10,
        };
        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(json["type"], "moveClip");
        assert_eq!(json["clipId"], "c");
        assert_eq!(json["timelineStartUs"], 10);
    }

    #[test]
    fn gain_is_clamped_and_rejected_on_video() {
        let (mut session, asset_id, video, audio) = session_with_asset();
        session
            .dispatch(Command::PlaceAsset {
                asset_id,
                video_track_id: Some(video),
                audio_track_id: Some(audio),
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: Some(1_000_000),
            })
            .unwrap();
        let video_id = session.project().sequence.tracks[0].clips[0].id.clone();
        let audio_id = session.project().sequence.tracks[2].clips[0].id.clone();
        assert!(session
            .dispatch(Command::SetClipGain {
                clip_id: video_id,
                gain_db: -3.0,
            })
            .is_err());
        session
            .dispatch(Command::SetClipGain {
                clip_id: audio_id,
                gain_db: -90.0,
            })
            .unwrap();
        assert_eq!(session.project().sequence.tracks[2].clips[0].gain_db, -60.0);
    }

    #[test]
    fn top_video_respects_mute_and_stacking() {
        let mut session = Session::new_empty("Stack");
        let lower = video_asset("low", 5_000_000);
        let upper = video_asset("up", 5_000_000);
        let low_id = lower.id.clone();
        let up_id = upper.id.clone();
        session
            .dispatch(Command::ImportAsset { asset: lower })
            .unwrap();
        session
            .dispatch(Command::ImportAsset { asset: upper })
            .unwrap();
        let v1 = session.project().sequence.tracks[0].id.clone();
        let v2 = session.project().sequence.tracks[1].id.clone();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: low_id,
                video_track_id: Some(v1),
                audio_track_id: None,
                timeline_start_us: 0,
                source_in_us: 0,
                duration_us: Some(5_000_000),
            })
            .unwrap();
        session
            .dispatch(Command::PlaceAsset {
                asset_id: up_id.clone(),
                video_track_id: Some(v2.clone()),
                audio_track_id: None,
                timeline_start_us: 1_000_000,
                source_in_us: 0,
                duration_us: Some(1_000_000),
            })
            .unwrap();
        assert_eq!(
            session
                .project()
                .top_video_clip_at(1_200_000)
                .unwrap()
                .asset_id,
            up_id
        );
        assert_eq!(
            session
                .project()
                .top_video_clip_at(100_000)
                .unwrap()
                .asset_id,
            "low"
        );
        session
            .dispatch(Command::SetTrackMuted {
                track_id: v2,
                muted: true,
            })
            .unwrap();
        assert_eq!(
            session
                .project()
                .top_video_clip_at(1_200_000)
                .unwrap()
                .asset_id,
            "low"
        );
    }

    #[test]
    fn remote_paths_are_rejected() {
        let mut session = Session::new_empty("Net");
        let mut asset = video_asset("net", 1_000_000);
        asset.path = "https://example.com/a.mp4".into();
        let err = session
            .dispatch(Command::ImportAsset { asset })
            .unwrap_err();
        assert!(err.to_string().contains("Remote"));
    }
}

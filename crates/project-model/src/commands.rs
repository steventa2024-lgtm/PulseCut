//! Non-destructive edit commands. Every command is applied to a working copy of the
//! project and only committed if it succeeds and the result validates, so a failed
//! command can never leave the model half-edited.

use crate::model::*;
use crate::time::{frames_to_seconds, seconds_to_frames_floor, Rational};
use crate::validate::validate_project;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

/// Default on-timeline length for still images, in seconds.
pub const IMAGE_DEFAULT_SECONDS: i64 = 5;

#[derive(Debug, Error, PartialEq)]
pub enum EditError {
    #[error("asset {0} is not in the project")]
    UnknownAsset(Uuid),
    #[error("track {0} does not exist")]
    UnknownTrack(Uuid),
    #[error("clip {0} does not exist")]
    UnknownClip(Uuid),
    #[error("track \"{0}\" is locked")]
    TrackLocked(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrimEdge {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EditCommand {
    /// Place an asset on a track (overwrite edit). When the asset has both picture and
    /// sound and the target is a video track, a linked audio clip is also placed.
    AddClip {
        asset_id: AssetId,
        track_id: TrackId,
        start: i64,
        #[serde(default)]
        source_in: Option<Rational>,
        #[serde(default)]
        duration: Option<i64>,
        #[serde(default = "default_true")]
        include_linked_audio: bool,
    },
    /// Move a clip (and its linked clips by the same delta). Overwrites what it lands on.
    MoveClip {
        clip_id: ClipId,
        track_id: Option<TrackId>,
        start: i64,
    },
    /// Move one edge of a clip (and linked clips) to `frame`, clamped to source media
    /// limits and neighbouring clips.
    TrimClip {
        clip_id: ClipId,
        edge: TrimEdge,
        frame: i64,
    },
    /// Split clips at `frame`. Empty `clip_ids` splits every clip under the frame on
    /// unlocked tracks.
    Split {
        frame: i64,
        #[serde(default)]
        clip_ids: Vec<ClipId>,
    },
    DeleteClips {
        clip_ids: Vec<ClipId>,
        #[serde(default)]
        ripple: bool,
    },
    SetClipParams {
        clip_id: ClipId,
        params: ClipParams,
    },
    SetTrackState {
        track_id: TrackId,
        #[serde(default)]
        muted: Option<bool>,
        #[serde(default)]
        locked: Option<bool>,
        #[serde(default)]
        solo: Option<bool>,
    },
    AddTrack {
        kind: TrackKind,
    },
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOutcome {
    pub label: String,
    /// Clips created by the command (useful for selection).
    pub created: Vec<ClipId>,
}

impl EditCommand {
    pub fn label(&self) -> &'static str {
        match self {
            EditCommand::AddClip { .. } => "Add clip",
            EditCommand::MoveClip { .. } => "Move clip",
            EditCommand::TrimClip { .. } => "Trim clip",
            EditCommand::Split { .. } => "Split",
            EditCommand::DeleteClips { ripple: true, .. } => "Ripple delete",
            EditCommand::DeleteClips { .. } => "Delete",
            EditCommand::SetClipParams { .. } => "Change clip properties",
            EditCommand::SetTrackState { .. } => "Change track state",
            EditCommand::AddTrack { .. } => "Add track",
        }
    }
}

/// Applies `cmd` to `project` atomically.
pub fn apply_command(project: &mut Project, cmd: &EditCommand) -> Result<EditOutcome, EditError> {
    let mut work = project.clone();
    let created = apply_inner(&mut work, cmd)?;
    validate_project(&work).map_err(|e| EditError::Invalid(format!("edit rejected: {e}")))?;
    *project = work;
    Ok(EditOutcome {
        label: cmd.label().into(),
        created,
    })
}

fn ensure_unlocked(seq: &Sequence, ti: usize) -> Result<(), EditError> {
    let t = &seq.tracks[ti];
    if t.locked {
        Err(EditError::TrackLocked(t.name.clone()))
    } else {
        Ok(())
    }
}

fn track_index(seq: &Sequence, id: TrackId) -> Result<usize, EditError> {
    seq.tracks
        .iter()
        .position(|t| t.id == id)
        .ok_or(EditError::UnknownTrack(id))
}

/// The clip plus every clip sharing its link id.
fn link_group(seq: &Sequence, clip_id: ClipId) -> Result<Vec<ClipId>, EditError> {
    let clip = seq.clip(clip_id).ok_or(EditError::UnknownClip(clip_id))?;
    let Some(link) = clip.link else {
        return Ok(vec![clip_id]);
    };
    let mut ids = vec![clip_id];
    for t in &seq.tracks {
        for c in &t.clips {
            if c.link == Some(link) && c.id != clip_id {
                ids.push(c.id);
            }
        }
    }
    Ok(ids)
}

fn sort_track(t: &mut Track) {
    t.clips.sort_by_key(|c| c.start);
}

fn advance_source(source_in: Rational, frames: i64, fps: Rational) -> Rational {
    source_in.add(frames_to_seconds(frames, fps))
}

/// Overwrite: remove/trim/split whatever lies in `[s, e)` on track `ti`, except `exclude`.
fn clear_range(seq: &mut Sequence, ti: usize, s: i64, e: i64, exclude: &HashSet<ClipId>) {
    let fps = seq.frame_rate;
    let track = &mut seq.tracks[ti];
    let mut out = Vec::with_capacity(track.clips.len() + 1);
    for c in track.clips.drain(..) {
        if exclude.contains(&c.id) || c.end() <= s || c.start >= e {
            out.push(c);
            continue;
        }
        let covers_head = c.start >= s;
        let covers_tail = c.end() <= e;
        match (covers_head, covers_tail) {
            (true, true) => {}
            (false, false) => {
                let mut right = c.clone();
                right.id = Uuid::new_v4();
                right.link = None;
                right.source_in = advance_source(c.source_in, e - c.start, fps);
                right.start = e;
                right.duration = c.end() - e;
                let mut left = c;
                left.duration = s - left.start;
                out.push(left);
                out.push(right);
            }
            (false, true) => {
                let mut left = c;
                left.duration = s - left.start;
                out.push(left);
            }
            (true, false) => {
                let mut right = c;
                let cut = e - right.start;
                right.source_in = advance_source(right.source_in, cut, fps);
                right.start = e;
                right.duration -= cut;
                out.push(right);
            }
        }
    }
    track.clips = out;
    sort_track(track);
}

fn apply_inner(project: &mut Project, cmd: &EditCommand) -> Result<Vec<ClipId>, EditError> {
    match cmd {
        EditCommand::AddClip {
            asset_id,
            track_id,
            start,
            source_in,
            duration,
            include_linked_audio,
        } => add_clip(
            project,
            *asset_id,
            *track_id,
            *start,
            *source_in,
            *duration,
            *include_linked_audio,
        ),
        EditCommand::MoveClip {
            clip_id,
            track_id,
            start,
        } => move_clip(&mut project.sequence, *clip_id, *track_id, *start).map(|_| vec![]),
        EditCommand::TrimClip {
            clip_id,
            edge,
            frame,
        } => trim_clip(project, *clip_id, *edge, *frame).map(|_| vec![]),
        EditCommand::Split { frame, clip_ids } => split(&mut project.sequence, *frame, clip_ids),
        EditCommand::DeleteClips { clip_ids, ripple } => {
            delete(&mut project.sequence, clip_ids, *ripple).map(|_| vec![])
        }
        EditCommand::SetClipParams { clip_id, params } => {
            params.validate().map_err(EditError::Invalid)?;
            let seq = &mut project.sequence;
            let (ti, ci) = seq
                .locate_clip(*clip_id)
                .ok_or(EditError::UnknownClip(*clip_id))?;
            ensure_unlocked(seq, ti)?;
            seq.tracks[ti].clips[ci].params = params.clone();
            Ok(vec![])
        }
        EditCommand::SetTrackState {
            track_id,
            muted,
            locked,
            solo,
        } => {
            let t = project
                .sequence
                .track_mut(*track_id)
                .ok_or(EditError::UnknownTrack(*track_id))?;
            if let Some(v) = muted {
                t.muted = *v;
            }
            if let Some(v) = locked {
                t.locked = *v;
            }
            if let Some(v) = solo {
                t.solo = *v;
            }
            Ok(vec![])
        }
        EditCommand::AddTrack { kind } => {
            let seq = &mut project.sequence;
            let n = seq.tracks.iter().filter(|t| t.kind == *kind).count() + 1;
            let name = match kind {
                TrackKind::Video => format!("V{n}"),
                TrackKind::Audio => format!("A{n}"),
            };
            let track = Track {
                id: Uuid::new_v4(),
                kind: *kind,
                name,
                muted: false,
                locked: false,
                solo: false,
                clips: vec![],
            };
            // Keep video tracks grouped before audio tracks.
            let pos = match kind {
                TrackKind::Video => seq
                    .tracks
                    .iter()
                    .rposition(|t| t.kind == TrackKind::Video)
                    .map_or(0, |i| i + 1),
                TrackKind::Audio => seq.tracks.len(),
            };
            seq.tracks.insert(pos, track);
            Ok(vec![])
        }
    }
}

fn add_clip(
    project: &mut Project,
    asset_id: AssetId,
    track_id: TrackId,
    start: i64,
    source_in: Option<Rational>,
    duration: Option<i64>,
    include_linked_audio: bool,
) -> Result<Vec<ClipId>, EditError> {
    let asset = project
        .asset(asset_id)
        .ok_or(EditError::UnknownAsset(asset_id))?
        .clone();
    let seq = &mut project.sequence;
    let ti = track_index(seq, track_id)?;
    ensure_unlocked(seq, ti)?;
    if start < 0 {
        return Err(EditError::Invalid(
            "clips cannot start before 00:00:00:00".into(),
        ));
    }
    let kind = seq.tracks[ti].kind;
    let has_picture = asset.media.video.is_some();
    let has_sound = asset.media.audio.is_some();
    match kind {
        TrackKind::Video if !has_picture => {
            return Err(EditError::Invalid(format!(
                "\"{}\" has no video; place it on an audio track",
                asset.name
            )))
        }
        TrackKind::Audio if !has_sound => {
            return Err(EditError::Invalid(format!(
                "\"{}\" has no audio stream",
                asset.name
            )))
        }
        _ => {}
    }
    let source_in = source_in.unwrap_or(Rational::ZERO);
    if source_in < Rational::ZERO {
        return Err(EditError::Invalid(
            "source in-point cannot be negative".into(),
        ));
    }
    let available = seq.available_frames(&asset, source_in);
    let default_len =
        available.unwrap_or(IMAGE_DEFAULT_SECONDS * seq.frame_rate.num() / seq.frame_rate.den());
    let duration = duration.unwrap_or(default_len);
    if duration <= 0 {
        return Err(EditError::Invalid(
            "the selected source range is shorter than one frame".into(),
        ));
    }
    if let Some(avail) = available {
        if duration > avail {
            return Err(EditError::Invalid(format!(
                "only {avail} frames of media are available from the in-point"
            )));
        }
    }

    let link = (kind == TrackKind::Video && has_sound && include_linked_audio).then(Uuid::new_v4);
    let mut created = Vec::new();
    let mut place = |seq: &mut Sequence, ti: usize, link: Option<Uuid>| {
        let clip = Clip {
            id: Uuid::new_v4(),
            asset_id,
            start,
            duration,
            source_in,
            link,
            params: ClipParams::default(),
        };
        clear_range(seq, ti, start, start + duration, &HashSet::new());
        created.push(clip.id);
        seq.tracks[ti].clips.push(clip);
        sort_track(&mut seq.tracks[ti]);
    };
    place(seq, ti, link);

    if link.is_some() {
        // Pair Vn with An (same ordinal), else the first unlocked audio track.
        let ordinal = seq.tracks[..ti]
            .iter()
            .filter(|t| t.kind == TrackKind::Video)
            .count();
        let audio: Vec<usize> = (0..seq.tracks.len())
            .filter(|&i| seq.tracks[i].kind == TrackKind::Audio)
            .collect();
        let target = audio
            .get(ordinal)
            .copied()
            .filter(|&i| !seq.tracks[i].locked)
            .or_else(|| audio.iter().copied().find(|&i| !seq.tracks[i].locked));
        match target {
            Some(ai) => place(seq, ai, link),
            None => {
                return Err(EditError::Invalid(
                    "no unlocked audio track is available for the linked audio".into(),
                ))
            }
        }
    }
    Ok(created)
}

fn move_clip(
    seq: &mut Sequence,
    clip_id: ClipId,
    track_id: Option<TrackId>,
    start: i64,
) -> Result<(), EditError> {
    let group = link_group(seq, clip_id)?;
    let (pti, pci) = seq
        .locate_clip(clip_id)
        .ok_or(EditError::UnknownClip(clip_id))?;
    let dest_ti = match track_id {
        Some(id) => track_index(seq, id)?,
        None => pti,
    };
    if seq.tracks[dest_ti].kind != seq.tracks[pti].kind {
        return Err(EditError::Invalid(
            "clips can only move between tracks of the same kind".into(),
        ));
    }
    ensure_unlocked(seq, dest_ti)?;
    let mut delta = start - seq.tracks[pti].clips[pci].start;
    let mut entries = Vec::new();
    for id in &group {
        let (ti, ci) = seq.locate_clip(*id).ok_or(EditError::UnknownClip(*id))?;
        ensure_unlocked(seq, ti)?;
        let c = &seq.tracks[ti].clips[ci];
        delta = delta.max(-c.start);
        entries.push((*id, ti));
    }
    let exclude: HashSet<ClipId> = group.iter().copied().collect();
    let mut moved = Vec::new();
    for (id, ti) in entries {
        let (ti_now, ci) = seq.locate_clip(id).ok_or(EditError::UnknownClip(id))?;
        debug_assert_eq!(ti_now, ti);
        let mut c = seq.tracks[ti].clips.remove(ci);
        c.start += delta;
        let target = if id == clip_id { dest_ti } else { ti };
        moved.push((target, c));
    }
    for (ti, c) in &moved {
        clear_range(seq, *ti, c.start, c.end(), &exclude);
    }
    for (ti, c) in moved {
        seq.tracks[ti].clips.push(c);
        sort_track(&mut seq.tracks[ti]);
    }
    Ok(())
}

fn trim_clip(
    project: &mut Project,
    clip_id: ClipId,
    edge: TrimEdge,
    frame: i64,
) -> Result<(), EditError> {
    let seq = &project.sequence;
    let primary = seq.clip(clip_id).ok_or(EditError::UnknownClip(clip_id))?;
    let requested = match edge {
        TrimEdge::Start => frame - primary.start,
        TrimEdge::End => frame - primary.end(),
    };
    let group = link_group(seq, clip_id)?;
    let (mut lo, mut hi) = (i64::MIN, i64::MAX);
    for id in &group {
        let (ti, ci) = seq.locate_clip(*id).ok_or(EditError::UnknownClip(*id))?;
        ensure_unlocked(seq, ti)?;
        let track = &seq.tracks[ti];
        let c = &track.clips[ci];
        let asset = project
            .asset(c.asset_id)
            .ok_or(EditError::UnknownAsset(c.asset_id))?;
        let is_image = asset.media.kind == MediaKind::Image;
        match edge {
            TrimEdge::Start => {
                let prev_end = if ci > 0 { track.clips[ci - 1].end() } else { 0 };
                let mut l = prev_end - c.start;
                if !is_image {
                    l = l.max(-seconds_to_frames_floor(c.source_in, seq.frame_rate));
                }
                lo = lo.max(l);
                hi = hi.min(c.duration - 1);
            }
            TrimEdge::End => {
                let next_start = track.clips.get(ci + 1).map_or(i64::MAX / 4, |n| n.start);
                let mut h = next_start - c.end();
                if let Some(avail) = seq.available_frames(asset, c.source_in) {
                    h = h.min(avail - c.duration);
                }
                lo = lo.max(-(c.duration - 1));
                hi = hi.min(h);
            }
        }
    }
    if lo > hi {
        return Err(EditError::Invalid(
            "this trim is not possible with the linked clips' media".into(),
        ));
    }
    let delta = requested.clamp(lo, hi);
    if delta == 0 {
        return Ok(());
    }
    let fps = seq.frame_rate;
    let seq = &mut project.sequence;
    for id in group {
        let (ti, ci) = seq.locate_clip(id).ok_or(EditError::UnknownClip(id))?;
        let c = &mut seq.tracks[ti].clips[ci];
        match edge {
            TrimEdge::Start => {
                c.start += delta;
                c.duration -= delta;
                let shifted = c.source_in.add(frames_to_seconds(delta, fps));
                c.source_in = shifted.max(Rational::ZERO);
            }
            TrimEdge::End => c.duration += delta,
        }
    }
    Ok(())
}

fn split(seq: &mut Sequence, frame: i64, clip_ids: &[ClipId]) -> Result<Vec<ClipId>, EditError> {
    let targets: Vec<ClipId> = if clip_ids.is_empty() {
        seq.tracks
            .iter()
            .filter(|t| !t.locked)
            .flat_map(|t| {
                t.clips
                    .iter()
                    .filter(|c| c.start < frame && frame < c.end())
                    .map(|c| c.id)
            })
            .collect()
    } else {
        let mut set = Vec::new();
        for id in clip_ids {
            for g in link_group(seq, *id)? {
                if !set.contains(&g) {
                    set.push(g);
                }
            }
        }
        set
    };
    let fps = seq.frame_rate;
    let mut relink: HashMap<Uuid, Uuid> = HashMap::new();
    let mut created = Vec::new();
    for id in targets {
        let (ti, ci) = seq.locate_clip(id).ok_or(EditError::UnknownClip(id))?;
        ensure_unlocked(seq, ti)?;
        let c = &seq.tracks[ti].clips[ci];
        if !(c.start < frame && frame < c.end()) {
            continue;
        }
        let mut right = c.clone();
        right.id = Uuid::new_v4();
        right.start = frame;
        right.duration = c.end() - frame;
        right.source_in = advance_source(c.source_in, frame - c.start, fps);
        right.link = c
            .link
            .map(|l| *relink.entry(l).or_insert_with(Uuid::new_v4));
        seq.tracks[ti].clips[ci].duration = frame - seq.tracks[ti].clips[ci].start;
        created.push(right.id);
        seq.tracks[ti].clips.push(right);
        sort_track(&mut seq.tracks[ti]);
    }
    if created.is_empty() {
        return Err(EditError::Invalid(
            "there is no clip under the playhead to split".into(),
        ));
    }
    Ok(created)
}

fn delete(seq: &mut Sequence, clip_ids: &[ClipId], ripple: bool) -> Result<(), EditError> {
    let mut all = HashSet::new();
    for id in clip_ids {
        all.extend(link_group(seq, *id)?);
    }
    for id in &all {
        let (ti, _) = seq.locate_clip(*id).ok_or(EditError::UnknownClip(*id))?;
        ensure_unlocked(seq, ti)?;
    }
    for track in &mut seq.tracks {
        let mut removed: Vec<(i64, i64)> = track
            .clips
            .iter()
            .filter(|c| all.contains(&c.id))
            .map(|c| (c.start, c.duration))
            .collect();
        track.clips.retain(|c| !all.contains(&c.id));
        if ripple {
            removed.sort();
            for c in &mut track.clips {
                let shift: i64 = removed
                    .iter()
                    .filter(|(s, _)| *s < c.start)
                    .map(|(_, d)| d)
                    .sum();
                c.start -= shift;
            }
        }
    }
    Ok(())
}

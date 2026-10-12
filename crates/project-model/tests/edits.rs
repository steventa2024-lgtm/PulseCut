use proptest::prelude::*;
use pulsecut_project_model::*;
use uuid::Uuid;

fn video_asset(name: &str, seconds: i64, fps: i64) -> Asset {
    Asset {
        id: Uuid::new_v4(),
        name: name.into(),
        path: format!("/media/{name}"),
        relative_path: None,
        media: MediaInfo {
            kind: MediaKind::Video,
            container: "mov,mp4".into(),
            duration: Rational::from_int(seconds),
            size_bytes: 1,
            video: Some(VideoStreamInfo {
                codec: "h264".into(),
                width: 640,
                height: 360,
                frame_rate: Rational::from_int(fps),
                pixel_format: Some("yuv420p".into()),
                variable_frame_rate: false,
                rotation: 0,
                hdr: false,
            }),
            audio: Some(AudioStreamInfo {
                codec: "aac".into(),
                sample_rate: 48000,
                channels: 2,
            }),
            audio_stream_count: 1,
        },
    }
}

fn audio_asset(name: &str, seconds: i64) -> Asset {
    let mut a = video_asset(name, seconds, 30);
    a.media.kind = MediaKind::Audio;
    a.media.video = None;
    a
}

struct Fx {
    editor: Editor,
    clip_a: Uuid,
    clip_b: Uuid,
    music: Uuid,
}

fn tracks(p: &Project) -> (Uuid, Uuid, Uuid, Uuid) {
    let t = &p.sequence.tracks;
    (t[0].id, t[1].id, t[2].id, t[3].id)
}

fn fixture() -> Fx {
    let mut p = Project::new(
        "t",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let (a, b, m) = (
        video_asset("a.mp4", 4, 30),
        video_asset("b.mov", 3, 25),
        audio_asset("m.wav", 6),
    );
    let (clip_a, clip_b, music) = (a.id, b.id, m.id);
    p.assets = vec![a, b, m];
    Fx {
        editor: Editor::new(p),
        clip_a,
        clip_b,
        music,
    }
}

fn add(editor: &mut Editor, asset: Uuid, track: Uuid, start: i64) -> Vec<Uuid> {
    editor
        .apply(&EditCommand::AddClip {
            asset_id: asset,
            track_id: track,
            start,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        })
        .unwrap()
        .created
}

#[test]
fn add_video_creates_linked_audio_on_paired_track() {
    let mut fx = fixture();
    let (v1, _, a1, _) = tracks(fx.editor.project());
    let created = add(&mut fx.editor, fx.clip_a, v1, 0);
    assert_eq!(created.len(), 2);
    let seq = &fx.editor.project().sequence;
    let v = seq.clip(created[0]).unwrap();
    let a = seq.clip(created[1]).unwrap();
    assert_eq!(
        seq.locate_clip(created[1]).unwrap().0,
        seq.tracks.iter().position(|t| t.id == a1).unwrap()
    );
    assert_eq!((v.start, v.duration), (0, 120));
    assert_eq!(v.link, a.link);
    assert!(v.link.is_some());
}

#[test]
fn mixed_rate_source_length_is_exact() {
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    let ids = add(&mut fx.editor, fx.clip_b, v1, 0);
    assert_eq!(
        fx.editor.project().sequence.clip(ids[0]).unwrap().duration,
        90
    );
}

#[test]
fn overwrite_splits_underlying_clip() {
    let mut fx = fixture();
    let (_, _, _, a2) = tracks(fx.editor.project());
    add(&mut fx.editor, fx.music, a2, 0);
    fx.editor
        .apply(&EditCommand::AddClip {
            asset_id: fx.music,
            track_id: a2,
            start: 30,
            source_in: None,
            duration: Some(30),
            include_linked_audio: true,
        })
        .unwrap();
    let clips = &fx.editor.project().sequence.tracks[3].clips;
    assert_eq!(
        clips
            .iter()
            .map(|c| (c.start, c.duration))
            .collect::<Vec<_>>(),
        vec![(0, 30), (30, 30), (60, 120)]
    );
    assert_eq!(clips[2].source_in, Rational::from_int(2));
}

#[test]
fn split_trim_move_delete_and_undo_roundtrip() {
    let mut fx = fixture();
    let (v1, _, _, _) = tracks(fx.editor.project());
    let ids = add(&mut fx.editor, fx.clip_a, v1, 0);
    let original = fx.editor.project().clone();

    let right = fx
        .editor
        .apply(&EditCommand::Split {
            frame: 45,
            clip_ids: vec![],
        })
        .unwrap()
        .created;
    assert_eq!(right.len(), 2, "video and linked audio both split");
    let seq = &fx.editor.project().sequence;
    let r = seq.clip(right[0]).unwrap();
    assert_eq!(
        (r.start, r.duration, r.source_in),
        (45, 75, Rational::new(3, 2))
    );
    let r_partner = seq.clip(right[1]).unwrap();
    assert_eq!(r.link, r_partner.link);
    assert_ne!(r.link, seq.clip(ids[0]).unwrap().link);

    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: ids[0],
            edge: TrimEdge::Start,
            frame: 10,
        })
        .unwrap();
    let left = fx.editor.project().sequence.clip(ids[0]).unwrap().clone();
    assert_eq!(
        (left.start, left.duration, left.source_in),
        (10, 35, Rational::new(1, 3))
    );
    let partner = fx.editor.project().sequence.clip(ids[1]).unwrap();
    assert_eq!((partner.start, partner.duration), (10, 35));

    // Trimming the start earlier than the source head is clamped.
    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: ids[0],
            edge: TrimEdge::Start,
            frame: -50,
        })
        .unwrap();
    assert_eq!(fx.editor.project().sequence.clip(ids[0]).unwrap().start, 0);

    fx.editor
        .apply(&EditCommand::MoveClip {
            clip_id: right[0],
            track_id: None,
            start: 200,
        })
        .unwrap();
    assert_eq!(
        fx.editor.project().sequence.clip(right[1]).unwrap().start,
        200
    );

    fx.editor
        .apply(&EditCommand::DeleteClips {
            clip_ids: vec![ids[0]],
            ripple: false,
        })
        .unwrap();
    assert!(fx.editor.project().sequence.clip(ids[1]).is_none());

    for _ in 0..5 {
        fx.editor.undo().unwrap();
    }
    assert_eq!(fx.editor.project(), &original);
    for _ in 0..5 {
        fx.editor.redo().unwrap();
    }
    assert!(fx.editor.project().sequence.clip(ids[0]).is_none());
}

#[test]
fn end_trim_clamps_to_media_and_neighbour() {
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    let a = add(&mut fx.editor, fx.clip_a, v1, 0);
    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: a[0],
            edge: TrimEdge::End,
            frame: 60,
        })
        .unwrap();
    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: a[0],
            edge: TrimEdge::End,
            frame: 1000,
        })
        .unwrap();
    assert_eq!(
        fx.editor.project().sequence.clip(a[0]).unwrap().duration,
        120
    );
    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: a[0],
            edge: TrimEdge::End,
            frame: 60,
        })
        .unwrap();
    add(&mut fx.editor, fx.clip_b, v1, 80);
    fx.editor
        .apply(&EditCommand::TrimClip {
            clip_id: a[0],
            edge: TrimEdge::End,
            frame: 1000,
        })
        .unwrap();
    assert_eq!(fx.editor.project().sequence.clip(a[0]).unwrap().end(), 80);
}

#[test]
fn locked_tracks_reject_edits_and_failed_edits_do_not_mutate() {
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    let a = add(&mut fx.editor, fx.clip_a, v1, 0);
    fx.editor
        .apply(&EditCommand::SetTrackState {
            track_id: v1,
            muted: None,
            locked: Some(true),
            solo: None,
        })
        .unwrap();
    let before = fx.editor.project().clone();
    let err = fx
        .editor
        .apply(&EditCommand::MoveClip {
            clip_id: a[0],
            track_id: None,
            start: 10,
        })
        .unwrap_err();
    assert!(matches!(err, EditError::TrackLocked(_)));
    assert_eq!(fx.editor.project(), &before);
    let bad = ClipParams {
        opacity: 2.0,
        ..Default::default()
    };
    assert!(fx
        .editor
        .apply(&EditCommand::SetClipParams {
            clip_id: a[1],
            params: bad
        })
        .is_err());
    assert_eq!(fx.editor.project(), &before);
}

#[test]
fn audio_only_asset_rejected_on_video_track() {
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    let r = fx.editor.apply(&EditCommand::AddClip {
        asset_id: fx.music,
        track_id: v1,
        start: 0,
        source_in: None,
        duration: None,
        include_linked_audio: true,
    });
    assert!(r.is_err());
}

#[test]
fn ripple_delete_closes_gap() {
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    let a = add(&mut fx.editor, fx.clip_a, v1, 0);
    let b = add(&mut fx.editor, fx.clip_b, v1, 120);
    fx.editor
        .apply(&EditCommand::DeleteClips {
            clip_ids: vec![a[0]],
            ripple: true,
        })
        .unwrap();
    assert_eq!(fx.editor.project().sequence.clip(b[0]).unwrap().start, 0);
    assert_eq!(fx.editor.project().sequence.clip(b[1]).unwrap().start, 0);
}

#[test]
fn serialization_roundtrip_and_relative_relink() {
    let dir = tempfile::tempdir().unwrap();
    let media = dir.path().join("media");
    std::fs::create_dir_all(&media).unwrap();
    std::fs::write(media.join("a.mp4"), b"x").unwrap();
    let mut fx = fixture();
    let (v1, ..) = tracks(fx.editor.project());
    add(&mut fx.editor, fx.clip_a, v1, 0);
    let mut project = fx.editor.project().clone();
    project.assets[0].path = media.join("a.mp4").to_string_lossy().into_owned();
    let file = dir.path().join("proj.pulsecut");
    let saved = io::save(&project, &file).unwrap();
    assert_eq!(
        saved.assets[0].relative_path.as_deref(),
        Some("media/a.mp4")
    );
    assert_eq!(
        io::to_json(&saved).unwrap(),
        std::fs::read_to_string(&file).unwrap()
    );

    // Move the whole folder: absolute path breaks, relative path relinks.
    let moved = tempfile::tempdir().unwrap();
    let new_root = moved.path().join("copy");
    std::fs::rename(dir.path(), &new_root).unwrap_or_else(|_| {
        std::fs::create_dir_all(new_root.join("media")).unwrap();
        std::fs::copy(media.join("a.mp4"), new_root.join("media/a.mp4")).unwrap();
        std::fs::copy(&file, new_root.join("proj.pulsecut")).unwrap();
        std::fs::remove_dir_all(dir.path()).unwrap();
    });
    let (loaded, report) = io::load(&new_root.join("proj.pulsecut")).unwrap();
    assert_eq!(report.relinked, vec![saved.assets[0].id]);
    assert_eq!(report.offline.len(), 2);
    assert_eq!(loaded.sequence, saved.sequence);
}

#[test]
fn rejects_newer_schema_and_garbage() {
    let p = Project::new(
        "x",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let json = io::to_json(&p)
        .unwrap()
        .replace("\"schemaVersion\": 1", "\"schemaVersion\": 99");
    assert!(matches!(
        io::from_json(&json),
        Err(io::ProjectIoError::NewerSchema { found: 99, .. })
    ));
    assert!(io::from_json("{}").is_err());
    assert!(io::from_json("not json").is_err());
}

#[derive(Debug, Clone)]
enum Op {
    Add(usize, usize, i64),
    Move(usize, i64, bool),
    Trim(usize, bool, i64),
    Split(i64),
    Delete(usize, bool),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..3, 0usize..4, 0i64..400).prop_map(|(a, t, s)| Op::Add(a, t, s)),
        (0usize..20, -50i64..400, any::<bool>()).prop_map(|(c, s, x)| Op::Move(c, s, x)),
        (0usize..20, any::<bool>(), -50i64..400).prop_map(|(c, e, f)| Op::Trim(c, e, f)),
        (0i64..400).prop_map(Op::Split),
        (0usize..20, any::<bool>()).prop_map(|(c, r)| Op::Delete(c, r)),
        Just(Op::Undo),
        Just(Op::Redo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]
    #[test]
    fn random_edits_preserve_invariants_and_undo_is_exact(ops in proptest::collection::vec(op(), 1..40)) {
        let mut fx = fixture();
        let assets = [fx.clip_a, fx.clip_b, fx.music];
        let mut snapshots = vec![fx.editor.project().clone()];
        for o in ops {
            let p = fx.editor.project().clone();
            let all: Vec<Uuid> = p.sequence.tracks.iter().flat_map(|t| t.clips.iter().map(|c| c.id)).collect();
            let pick = |i: usize| (!all.is_empty()).then(|| all[i % all.len()]);
            let cmd = match o {
                Op::Add(a, t, s) => Some(EditCommand::AddClip { asset_id: assets[a], track_id: p.sequence.tracks[t].id, start: s, source_in: None, duration: None, include_linked_audio: true }),
                Op::Move(c, s, cross) => pick(c).map(|id| {
                    let (ti, _) = p.sequence.locate_clip(id).unwrap();
                    let other = p.sequence.tracks.iter().enumerate().find(|(i, t)| *i != ti && t.kind == p.sequence.tracks[ti].kind).map(|(_, t)| t.id);
                    EditCommand::MoveClip { clip_id: id, track_id: if cross { other } else { None }, start: s }
                }),
                Op::Trim(c, e, f) => pick(c).map(|id| EditCommand::TrimClip { clip_id: id, edge: if e { TrimEdge::Start } else { TrimEdge::End }, frame: f }),
                Op::Split(f) => Some(EditCommand::Split { frame: f, clip_ids: vec![] }),
                Op::Delete(c, r) => pick(c).map(|id| EditCommand::DeleteClips { clip_ids: vec![id], ripple: r }),
                Op::Undo => { if fx.editor.undo().is_some() { snapshots.pop(); } None }
                Op::Redo => { if fx.editor.redo().is_some() { snapshots.push(fx.editor.project().clone()); } None }
            };
            if let Some(cmd) = cmd {
                let before = fx.editor.project().clone();
                match fx.editor.apply(&cmd) {
                    Ok(_) => snapshots.push(fx.editor.project().clone()),
                    Err(_) => prop_assert_eq!(fx.editor.project(), &before),
                }
            }
            prop_assert!(validate::validate_project(fx.editor.project()).is_ok());
            let json = io::to_json(fx.editor.project()).unwrap();
            prop_assert_eq!(&io::from_json(&json).unwrap(), fx.editor.project());
        }
        while fx.editor.undo().is_some() {}
        prop_assert_eq!(fx.editor.project(), &snapshots[0]);
    }
}

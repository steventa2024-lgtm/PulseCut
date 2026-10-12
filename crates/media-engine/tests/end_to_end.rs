//! Acceptance tests against real media (fixtures/media, synthesized by
//! scripts/generate-fixtures.sh). Requires ffmpeg + ffprobe (see README).

use pulsecut_media_engine::*;
use pulsecut_project_model::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/media")
        .canonicalize()
        .unwrap()
}

fn tools() -> FfmpegTools {
    FfmpegTools::locate().expect("ffmpeg and ffprobe must be installed to run media-engine tests")
}

fn import(t: &FfmpegTools, name: &str) -> Asset {
    let path = fixtures().join(name);
    let media = probe(t, &path).unwrap();
    Asset {
        id: uuid::Uuid::new_v4(),
        name: name.into(),
        path: path.to_string_lossy().into_owned(),
        relative_path: None,
        media,
    }
}

fn psnr(t: &FfmpegTools, a: &[u8], b: &[u8]) -> f64 {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.png"), dir.path().join("b.png"));
    std::fs::write(&pa, a).unwrap();
    std::fs::write(&pb, b).unwrap();
    let out = std::process::Command::new(&t.ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(&pa)
        .arg("-i")
        .arg(&pb)
        .args(["-lavfi", "psnr", "-f", "null", "-"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stderr);
    let v = s
        .split("average:")
        .nth(1)
        .expect("psnr output")
        .split_whitespace()
        .next()
        .unwrap();
    if v == "inf" {
        f64::INFINITY
    } else {
        v.parse().unwrap()
    }
}

fn decode_frame(t: &FfmpegTools, path: &Path, n: i64) -> Vec<u8> {
    let out = std::process::Command::new(&t.ffmpeg)
        .args(["-hide_banner", "-v", "error", "-i"])
        .arg(path)
        .args([
            "-vf",
            &format!("select=eq(n\\,{n})"),
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(!out.stdout.is_empty(), "could not decode frame {n}");
    out.stdout
}

fn source_frame(t: &FfmpegTools, path: &Path, seconds: f64) -> Vec<u8> {
    let out = std::process::Command::new(&t.ffmpeg)
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-ss",
            &format!("{seconds:.6}"),
            "-i",
        ])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ])
        .output()
        .unwrap();
    out.stdout
}

/// `silence_end` timestamps (seconds) of the exported audio.
fn silence_ends(t: &FfmpegTools, path: &Path) -> Vec<f64> {
    let out = std::process::Command::new(&t.ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(path)
        .args([
            "-af",
            "silencedetect=noise=-60dB:d=0.2",
            "-vn",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter_map(|l| l.split("silence_end: ").nth(1))
        .map(|v| v.split_whitespace().next().unwrap().parse().unwrap())
        .collect()
}

#[test]
fn probe_reports_real_metadata() {
    let t = tools();
    let a = import(&t, "clip_a_640x360_30fps.mp4").media;
    let v = a.video.unwrap();
    assert_eq!(
        (v.width, v.height, v.frame_rate),
        (640, 360, Rational::from_int(30))
    );
    assert!((a.duration.as_f64() - 4.0).abs() < 0.05);
    let au = a.audio.unwrap();
    assert_eq!((au.sample_rate, au.channels), (48000, 2));

    let b = import(&t, "clip_b_640x360_25fps.mov").media;
    assert_eq!(b.video.unwrap().frame_rate, Rational::from_int(25));
    assert_eq!(b.audio.unwrap().sample_rate, 44100);

    let m = import(&t, "music_bed_48k.wav").media;
    assert_eq!(m.kind, MediaKind::Audio);
    assert!(m.video.is_none());
}

#[test]
fn corrupt_and_missing_media_give_explicit_errors() {
    let t = tools();
    let err = probe(&t, &fixtures().join("corrupt.mp4")).unwrap_err();
    assert!(matches!(err, MediaError::Probe { .. }), "{err:?}");
    assert!(err
        .to_string()
        .contains("damaged or in an unsupported format"));
    let err = probe(&t, &fixtures().join("does-not-exist.mp4")).unwrap_err();
    assert!(err.to_string().contains("cannot read file"));
}

#[test]
fn full_editing_session_exports_verified_matching_output() {
    let t = tools();
    let work = tempfile::tempdir().unwrap();
    let mut project = Project::new(
        "E2E",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let (a, b, m) = (
        import(&t, "clip_a_640x360_30fps.mp4"),
        import(&t, "clip_b_640x360_25fps.mov"),
        import(&t, "music_bed_48k.wav"),
    );
    let (aid, bid, mid) = (a.id, b.id, m.id);
    project.assets = vec![a.clone(), b.clone(), m];
    let mut ed = Editor::new(project);
    let tr: Vec<TrackId> = ed.project().sequence.tracks.iter().map(|t| t.id).collect();
    let (v1, v2, a2) = (tr[0], tr[1], tr[3]);

    // Clip A at 0, clip B at 3 s (1 s gap), music on A2.
    let ca = ed
        .apply(&EditCommand::AddClip {
            asset_id: aid,
            track_id: v1,
            start: 0,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        })
        .unwrap()
        .created;
    let cb = ed
        .apply(&EditCommand::AddClip {
            asset_id: bid,
            track_id: v1,
            start: 150,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        })
        .unwrap()
        .created;
    let music = ed
        .apply(&EditCommand::AddClip {
            asset_id: mid,
            track_id: a2,
            start: 0,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        })
        .unwrap()
        .created;
    let after_adds = ed.project().clone();

    // Split A at 2 s, delete its right half (frames 60–120), trim the head of A to 0.5 s.
    let right = ed
        .apply(&EditCommand::Split {
            frame: 60,
            clip_ids: vec![ca[0]],
        })
        .unwrap()
        .created;
    ed.apply(&EditCommand::DeleteClips {
        clip_ids: vec![right[0]],
        ripple: false,
    })
    .unwrap();
    ed.apply(&EditCommand::TrimClip {
        clip_id: ca[0],
        edge: TrimEdge::Start,
        frame: 15,
    })
    .unwrap();
    // Move B to 2.5 s; mute the music so the gap is truly silent.
    ed.apply(&EditCommand::MoveClip {
        clip_id: cb[0],
        track_id: None,
        start: 75,
    })
    .unwrap();
    ed.apply(&EditCommand::SetTrackState {
        track_id: a2,
        muted: Some(true),
        locked: None,
        solo: None,
    })
    .unwrap();
    // Picture-in-picture copy of B on V2 at half scale, 80% opacity, top-left quadrant.
    let pip = ed
        .apply(&EditCommand::AddClip {
            asset_id: bid,
            track_id: v2,
            start: 90,
            source_in: Some(Rational::from_int(1)),
            duration: Some(30),
            include_linked_audio: false,
        })
        .unwrap()
        .created;
    ed.apply(&EditCommand::SetClipParams {
        clip_id: pip[0],
        params: ClipParams {
            opacity: 0.8,
            scale: 0.5,
            position_x: -160.0,
            position_y: -90.0,
            gain_db: 0.0,
        },
    })
    .unwrap();

    // Undo/redo exactness.
    let edited = ed.project().clone();
    for _ in 0..7 {
        ed.undo().unwrap();
    }
    assert_eq!(ed.project(), &after_adds);
    for _ in 0..7 {
        ed.redo().unwrap();
    }
    assert_eq!(ed.project(), &edited);
    let seq = &edited.sequence;
    let a_clip = seq.clip(ca[0]).unwrap();
    assert_eq!(
        (a_clip.start, a_clip.duration, a_clip.source_in),
        (15, 45, Rational::new(1, 2))
    );
    assert_eq!(
        seq.clip(cb[1]).unwrap().start,
        75,
        "linked audio moved with B"
    );
    assert_eq!(
        seq.duration_frames(),
        180,
        "muted tracks still define sequence length"
    );
    assert!(seq.clip(music[0]).is_some());

    // Save, then reopen from disk.
    let proj_path = work.path().join("e2e.pulsecut");
    pulsecut_project_model::io::save(ed.project(), &proj_path).unwrap();
    let (reopened, report) = pulsecut_project_model::io::load(&proj_path).unwrap();
    assert!(report.offline.is_empty());
    assert_eq!(reopened.sequence, edited.sequence);

    // Export and verify.
    let out = work.path().join("e2e.mp4");
    let mut progress = Vec::new();
    let res = export(
        &t,
        &reopened,
        &out,
        &ExportSettings::default(),
        &AtomicBool::new(false),
        |p| progress.push(p.fraction),
    )
    .unwrap();
    assert!(out.is_file());
    assert!(progress.len() >= 2 && *progress.last().unwrap() == 1.0);
    let report = verify_export(&t, &reopened, &out, 1.0).unwrap();
    for c in &report.checks {
        eprintln!(
            "verify {:<14} expected {:<22} actual {:<30} {}",
            c.name,
            c.expected,
            c.actual,
            if c.pass { "PASS" } else { "FAIL" }
        );
    }
    assert!(report.pass, "export verification failed");
    eprintln!("export via {} in {:.2}s", res.encoder, res.elapsed_seconds);

    // Preview/export parity: lossless preview stills vs decoded export frames.
    let offline = HashSet::new();
    for n in [0i64, 20, 59, 75, 100, 119, 164, 179] {
        let preview = render_still(&t, &reopened, n, 1.0, &offline, true).unwrap();
        let exported = decode_frame(&t, &out, n);
        let p = psnr(&t, &preview, &exported);
        eprintln!("frame {n:>3}: preview vs export PSNR {p:.2} dB");
        assert!(p > 35.0, "frame {n} preview/export mismatch ({p:.2} dB)");
    }

    // Edit correctness: frame 20 is clip A at source 0.5 s + 5/30 s; the gap (frames 0–14) is black.
    let preview20 = render_still(&t, &reopened, 20, 1.0, &offline, true).unwrap();
    let src = source_frame(&t, Path::new(&a.path), 0.5 + 5.0 / 30.0);
    let p = psnr(&t, &preview20, &src);
    assert!(
        p > 40.0,
        "frame 20 does not show the trimmed source frame ({p:.2} dB)"
    );
    let gap = render_still(&t, &reopened, 5, 1.0, &offline, true).unwrap();
    assert!(
        psnr(&t, &gap, &black_png(&t)) > 50.0,
        "frame 5 should be black (gap)"
    );

    // Audio sync: leading 0.5 s gap is silent, audio starts at frame 15 (0.5 s) within one frame.
    let ends = silence_ends(&t, &out);
    eprintln!("silence ends at {ends:?}");
    assert!(
        ends.first()
            .is_some_and(|s| (s - 0.5).abs() <= 1.0 / 30.0 + 0.005),
        "audio onset {ends:?} != 0.5 s"
    );
}

fn black_png(t: &FfmpegTools) -> Vec<u8> {
    let out = std::process::Command::new(&t.ffmpeg)
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=640x360",
            "-vf",
            "format=yuv420p",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ])
        .output()
        .unwrap();
    out.stdout
}

#[test]
fn export_cancel_leaves_no_partial_file() {
    let t = tools();
    let work = tempfile::tempdir().unwrap();
    let mut project = Project::new(
        "C",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let a = import(&t, "clip_a_640x360_30fps.mp4");
    let aid = a.id;
    project.assets = vec![a];
    let v1 = project.sequence.tracks[0].id;
    apply_command(
        &mut project,
        &EditCommand::AddClip {
            asset_id: aid,
            track_id: v1,
            start: 0,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        },
    )
    .unwrap();
    let out = work.path().join("cancel.mp4");
    let cancel = AtomicBool::new(true);
    let err = export(
        &t,
        &project,
        &out,
        &ExportSettings::default(),
        &cancel,
        |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, MediaError::Cancelled));
    assert_eq!(
        std::fs::read_dir(work.path()).unwrap().count(),
        0,
        "no partial or final file left behind"
    );
}

#[test]
fn offline_media_blocks_export_with_actionable_error() {
    let t = tools();
    let work = tempfile::tempdir().unwrap();
    let mut project = Project::new(
        "O",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let mut a = import(&t, "clip_a_640x360_30fps.mp4");
    let aid = a.id;
    a.path = work.path().join("gone.mp4").to_string_lossy().into_owned();
    project.assets = vec![a];
    let v1 = project.sequence.tracks[0].id;
    apply_command(
        &mut project,
        &EditCommand::AddClip {
            asset_id: aid,
            track_id: v1,
            start: 0,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        },
    )
    .unwrap();
    let err = export(
        &t,
        &project,
        &work.path().join("x.mp4"),
        &ExportSettings::default(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap_err();
    assert!(err.to_string().contains("relink offline media"), "{err}");
}

#[test]
fn playback_stream_delivers_ordered_frames() {
    let t = tools();
    let mut project = Project::new(
        "P",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let a = import(&t, "clip_a_640x360_30fps.mp4");
    let aid = a.id;
    project.assets = vec![a];
    let v1 = project.sequence.tracks[0].id;
    apply_command(
        &mut project,
        &EditCommand::AddClip {
            asset_id: aid,
            track_id: v1,
            start: 0,
            source_in: None,
            duration: Some(45),
            include_linked_audio: true,
        },
    )
    .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let (etx, erx) = std::sync::mpsc::channel();
    let started = std::time::Instant::now();
    let stream = PlaybackStream::start(
        &t,
        &project,
        15,
        0.5,
        &HashSet::new(),
        std::time::Duration::from_millis(200),
        move |i, jpg| tx.send((i, jpg.len())).is_ok(),
        move |err| {
            let _ = etx.send(err);
        },
    )
    .unwrap();
    assert_eq!(
        erx.recv_timeout(std::time::Duration::from_secs(10))
            .unwrap(),
        None
    );
    let elapsed = started.elapsed().as_secs_f64();
    let frames: Vec<(i64, usize)> = rx.try_iter().collect();
    stream.stop();
    assert_eq!(
        frames.iter().map(|f| f.0).collect::<Vec<_>>(),
        (15..45).collect::<Vec<_>>()
    );
    assert!(frames.iter().all(|f| f.1 > 1000));
    // 30 frames at 30 fps, released up to 0.2 s early: ~0.8 s wall-clock at minimum.
    assert!(elapsed > 0.7, "stream was not paced ({elapsed:.2}s)");
}

#[test]
fn audio_pcm_has_exact_length() {
    let t = tools();
    let mut project = Project::new(
        "A",
        Sequence::new("Main", 640, 360, Rational::from_int(30), 48000),
    );
    let m = import(&t, "music_bed_48k.wav");
    let mid = m.id;
    project.assets = vec![m];
    let a1 = project.sequence.tracks[2].id;
    apply_command(
        &mut project,
        &EditCommand::AddClip {
            asset_id: mid,
            track_id: a1,
            start: 0,
            source_in: None,
            duration: None,
            include_linked_audio: true,
        },
    )
    .unwrap();
    let pcm = render_audio_pcm(&t, &project, 30, 90, &HashSet::new()).unwrap();
    assert_eq!(pcm.len(), 2 * 48000 * 2 * 2, "2 s of 48 kHz stereo s16");
}

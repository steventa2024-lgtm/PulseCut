use std::path::Path;
use std::process::Command;

use media_engine::{
    build_plan, probe_file, render_project, thumbnail_jpeg, waveform_peaks, RenderSettings,
};
use project_model::{from_json, to_json_pretty, Command as Edit, Session};

fn ffmpeg() -> std::path::PathBuf {
    media_engine::ffmpeg_path().expect("ffmpeg is required for pipeline tests")
}

fn synth(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let jobs = [
        (
            "red.mp4",
            vec![
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0xE23B3B:s=1280x720:r=30:d=5,format=yuv420p",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
                "red.mp4",
            ],
        ),
        (
            "blue.mp4",
            vec![
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x2F6FED:s=1920x1080:r=24:d=4,format=yuv420p",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:sample_rate=44100:duration=4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
                "blue.mp4",
            ],
        ),
        (
            "music.m4a",
            vec![
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=220:sample_rate=48000:duration=8",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=330:sample_rate=48000:duration=8",
                "-filter_complex",
                "[0:a][1:a]amix=inputs=2:normalize=0,aformat=channel_layouts=stereo",
                "-c:a",
                "aac",
                "-b:a",
                "160k",
                "music.m4a",
            ],
        ),
    ];
    for (name, mut args) in jobs {
        let dest = dir.join(name);
        if let Some(last) = args.last_mut() {
            *last = dest.to_str().unwrap();
        }
        let status = Command::new(ffmpeg()).args(&args).status().unwrap();
        assert!(status.success(), "failed to synthesize {name}");
    }
    let status = Command::new(ffmpeg())
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            dir.join("red.mp4").to_str().unwrap(),
            "-c",
            "copy",
            dir.join("red.mov").to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "failed to write mov");
}

fn sample_rgb(path: &Path, seconds: f64) -> (u8, u8, u8) {
    let probed = probe_file(path).unwrap();
    let width = probed.width.unwrap() as usize;
    let height = probed.height.unwrap() as usize;
    let raw = Command::new(ffmpeg())
        .args([
            "-v",
            "error",
            "-ss",
            &format!("{seconds:.3}"),
            "-i",
            path.to_str().unwrap(),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap()
        .stdout;
    assert!(raw.len() >= width * height * 3, "no frame at {seconds}");
    let x = width / 2;
    let y = height / 2;
    let i = (y * width + x) * 3;
    (raw[i], raw[i + 1], raw[i + 2])
}

fn is_red(px: (u8, u8, u8)) -> bool {
    px.0 > 160 && px.0 > px.1 + 60 && px.0 > px.2 + 60
}
fn is_blue(px: (u8, u8, u8)) -> bool {
    px.2 > 160 && px.2 > px.0 + 60 && px.2 > px.1 + 40
}
fn is_black(px: (u8, u8, u8)) -> bool {
    px.0 < 30 && px.1 < 30 && px.2 < 30
}

fn is_green(px: (u8, u8, u8)) -> bool {
    px.1 > 80 && px.1 > px.0 + 40 && px.1 > px.2 + 40
}

#[test]
fn import_timeline_trim_save_and_export() {
    let root = std::env::temp_dir().join(format!("pulsecut-pipe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    synth(&root);
    let red = probe_file(&root.join("red.mp4")).unwrap();
    assert_eq!(red.width, Some(1280));
    assert_eq!(red.height, Some(720));
    assert_eq!(red.fps_num, Some(30));
    assert_eq!(red.fps_den, Some(1));
    assert!(red.has_audio);
    assert!((red.duration_us - 5_000_000).abs() < 50_000);
    let mov = probe_file(&root.join("red.mov")).unwrap();
    assert!(mov.has_video && mov.format_name.contains("mov"));
    let blue = probe_file(&root.join("blue.mp4")).unwrap();
    assert_eq!(blue.fps_num, Some(24));
    assert_eq!(blue.width, Some(1920));

    let mut session = Session::new_empty("Pipeline");
    session
        .dispatch(Edit::SetSequence {
            name: Some("Main".into()),
            width: 1280,
            height: 720,
            fps_num: 30,
            fps_den: 1,
        })
        .unwrap();
    let mut red_asset = red.into_asset();
    red_asset.path = root.join("red.mp4").to_string_lossy().to_string();
    let mut blue_asset = blue.into_asset();
    blue_asset.path = root.join("blue.mp4").to_string_lossy().to_string();
    let music = probe_file(&root.join("music.m4a")).unwrap().into_asset();
    let red_id = red_asset.id.clone();
    let blue_id = blue_asset.id.clone();
    let music_id = music.id.clone();
    for asset in [red_asset, blue_asset, music] {
        session.dispatch(Edit::ImportAsset { asset }).unwrap();
    }
    let v1 = session.project().sequence.tracks[0].id.clone();
    let v2 = session.project().sequence.tracks[1].id.clone();
    let a1 = session.project().sequence.tracks[2].id.clone();
    let a2 = session.project().sequence.tracks[3].id.clone();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: red_id,
            video_track_id: Some(v1),
            audio_track_id: Some(a1),
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: None,
        })
        .unwrap();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: blue_id,
            video_track_id: Some(v2),
            audio_track_id: None,
            timeline_start_us: 2_000_000,
            source_in_us: 500_000,
            duration_us: Some(2_000_000),
        })
        .unwrap();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: music_id,
            video_track_id: None,
            audio_track_id: Some(a2),
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: Some(6_000_000),
        })
        .unwrap();

    let json = to_json_pretty(session.project()).unwrap();
    let restored = from_json(&json).unwrap();
    assert_eq!(restored.duration_us(), 6_000_000);

    let export_path = root.join("export.mp4");
    let settings = RenderSettings {
        width: 1280,
        height: 720,
        fps_num: 30,
        fps_den: 1,
        preset: "veryfast".into(),
        crf: 18,
        audio_bitrate_kbps: 160,
    };
    let report = render_project(&restored, &settings, &export_path, |_| {}, None).unwrap();
    assert_eq!(report.width, 1280);
    assert_eq!(report.height, 720);
    assert_eq!(report.video_codec.as_deref(), Some("h264"));
    assert_eq!(report.audio_codec.as_deref(), Some("aac"));
    assert_eq!(report.audio_sample_rate, Some(48_000));
    assert_eq!(report.audio_channels, Some(2));
    assert!((report.duration_us - 6_000_000).abs() < 50_000);
    assert!(report.bytes > 1_000);

    let at_1 = sample_rgb(&export_path, 1.0);
    let at_3 = sample_rgb(&export_path, 3.0);
    let at_45 = sample_rgb(&export_path, 4.5);
    let at_55 = sample_rgb(&export_path, 5.5);
    assert!(is_red(at_1), "t=1s expected red, got {at_1:?}");
    assert!(is_blue(at_3), "t=3s expected blue overlay, got {at_3:?}");
    assert!(
        is_red(at_45),
        "t=4.5s expected red after overlay ends, got {at_45:?}"
    );
    assert!(is_black(at_55), "t=5.5s expected black gap, got {at_55:?}");

    let loud = Command::new(ffmpeg())
        .args([
            "-hide_banner",
            "-ss",
            "5.2",
            "-t",
            "0.5",
            "-i",
            export_path.to_str().unwrap(),
            "-af",
            "volumedetect",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&loud.stderr);
    let max_volume = log
        .lines()
        .find(|line| line.contains("max_volume"))
        .unwrap_or_else(|| panic!("no volumedetect output: {log}"));
    assert!(
        !max_volume.contains("-inf"),
        "music tail should be audible: {max_volume}"
    );

    let preview_settings = RenderSettings::preview_from_sequence(&restored);
    let preview_path = root.join("preview.mp4");
    render_project(&restored, &preview_settings, &preview_path, |_| {}, None).unwrap();
    assert!(psnr_average(&export_path, &preview_path, 1.0) > 30.0);
    assert!(psnr_average(&export_path, &preview_path, 3.0) > 30.0);

    let video_clip = restored.sequence.tracks[0].clips[0].id.clone();
    let mut edited = Session::new(restored).unwrap();
    edited
        .dispatch(Edit::TrimClip {
            clip_id: video_clip,
            edge: project_model::TrimEdge::End,
            timeline_us: 2_000_000,
        })
        .unwrap();
    let trimmed_path = root.join("trimmed.mp4");
    render_project(edited.project(), &settings, &trimmed_path, |_| {}, None).unwrap();
    let trimmed_late = sample_rgb(&trimmed_path, 4.5);
    assert!(
        is_black(trimmed_late),
        "trim should remove the red tail, got {trimmed_late:?}"
    );
    assert!(is_blue(sample_rgb(&trimmed_path, 3.0)));

    let thumbs = thumbnail_jpeg(&root.join("red.mp4"), 500_000, 160).unwrap();
    assert!(thumbs.len() > 100);
    let peaks = waveform_peaks(&root.join("music.m4a"), 0, 2_000_000, 32).unwrap();
    assert_eq!(peaks.len(), 32);
    assert!(peaks.iter().any(|p| *p > 0.2));

    let mut junk = root.join("junk.mp4");
    std::fs::write(&junk, b"this is not media").unwrap();
    assert!(probe_file(&junk).is_err());
    junk.set_extension("txt");

    let _ = build_plan(edited.project(), &settings, true).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

fn psnr_average(export: &Path, preview: &Path, seconds: f64) -> f64 {
    let dir = export.parent().unwrap();
    let a = dir.join(format!("psnr-a-{seconds}.png"));
    let b = dir.join(format!("psnr-b-{seconds}.png"));
    for (src, dest) in [(export, &a), (preview, &b)] {
        let status = Command::new(ffmpeg())
            .args([
                "-y",
                "-v",
                "error",
                "-ss",
                &format!("{seconds:.3}"),
                "-i",
                src.to_str().unwrap(),
                "-frames:v",
                "1",
                "-vf",
                "scale=640:360",
                dest.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }
    let output = Command::new(ffmpeg())
        .args([
            "-hide_banner",
            "-i",
            a.to_str().unwrap(),
            "-i",
            b.to_str().unwrap(),
            "-lavfi",
            "psnr",
            "-f",
            "null",
            "-",
        ])
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&output.stderr);
    let line = log
        .lines()
        .find(|line| line.contains("average:"))
        .unwrap_or_else(|| panic!("psnr missing: {log}"));
    let value = line
        .split("average:")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    value.parse().unwrap()
}

#[test]
fn video_only_timeline_still_writes_audio() {
    let root = std::env::temp_dir().join(format!("pulsecut-silent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let clip = root.join("solid.mp4");
    let status = Command::new(ffmpeg())
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=640x360:r=30:d=1,format=yuv420p",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-an",
            clip.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let info = probe_file(&clip).unwrap();
    assert!(info.has_video && !info.has_audio);
    let mut session = Session::new_empty("Silent");
    let asset = info.into_asset();
    let id = asset.id.clone();
    session.dispatch(Edit::ImportAsset { asset }).unwrap();
    let video = session.project().sequence.tracks[0].id.clone();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: id,
            video_track_id: Some(video),
            audio_track_id: None,
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: None,
        })
        .unwrap();
    let output = root.join("out.mp4");
    let report = render_project(
        session.project(),
        &RenderSettings::export_from_sequence(session.project()),
        &output,
        |_| {},
        None,
    )
    .unwrap();
    assert_eq!(report.audio_codec.as_deref(), Some("aac"));
    assert!((report.duration_us - 1_000_000).abs() < 50_000);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn portrait_clip_letterboxes_without_covering_the_lower_track() {
    let root = std::env::temp_dir().join(format!("pulsecut-box-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let red = root.join("red.mp4");
    let tall = root.join("tall.mp4");
    for (path, spec) in [
        (&red, "color=c=0xE23B3B:s=640x360:r=30:d=2,format=yuv420p"),
        (&tall, "color=c=0x3DDC97:s=360x640:r=30:d=2,format=yuv420p"),
    ] {
        let status = Command::new(ffmpeg())
            .args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                spec,
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-an",
                path.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }
    let mut session = Session::new_empty("Box");
    session
        .dispatch(Edit::SetSequence {
            name: None,
            width: 640,
            height: 360,
            fps_num: 30,
            fps_den: 1,
        })
        .unwrap();
    let red_info = probe_file(&red).unwrap().into_asset();
    let tall_info = probe_file(&tall).unwrap().into_asset();
    let red_id = red_info.id.clone();
    let tall_id = tall_info.id.clone();
    session
        .dispatch(Edit::ImportAsset { asset: red_info })
        .unwrap();
    session
        .dispatch(Edit::ImportAsset { asset: tall_info })
        .unwrap();
    let v1 = session.project().sequence.tracks[0].id.clone();
    let v2 = session.project().sequence.tracks[1].id.clone();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: red_id,
            video_track_id: Some(v1),
            audio_track_id: None,
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: None,
        })
        .unwrap();
    session
        .dispatch(Edit::PlaceAsset {
            asset_id: tall_id,
            video_track_id: Some(v2),
            audio_track_id: None,
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: None,
        })
        .unwrap();
    let output = root.join("out.mp4");
    render_project(
        session.project(),
        &RenderSettings {
            width: 640,
            height: 360,
            fps_num: 30,
            fps_den: 1,
            preset: "ultrafast".into(),
            crf: 18,
            audio_bitrate_kbps: 96,
        },
        &output,
        |_| {},
        None,
    )
    .unwrap();
    let center = sample_rgb(&output, 0.4);
    let probed = probe_file(&output).unwrap();
    let width = probed.width.unwrap() as usize;
    let raw = Command::new(ffmpeg())
        .args([
            "-v",
            "error",
            "-ss",
            "0.400",
            "-i",
            output.to_str().unwrap(),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap()
        .stdout;
    let corner = (
        raw[2 * width * 3 + 6],
        raw[2 * width * 3 + 7],
        raw[2 * width * 3 + 8],
    );
    assert!(
        is_green(center),
        "center should be the portrait clip, got {center:?}"
    );
    assert!(
        is_red(corner),
        "side bars should show the lower track, got {corner:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

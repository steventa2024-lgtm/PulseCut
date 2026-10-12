//! Compiles a [`Project`] time range into one FFmpeg filter graph.
//!
//! This is the only place timeline semantics are translated for rendering. Preview
//! stills, real-time playback and export all call [`compile`], so what you see while
//! editing is produced by the same graph that produces the exported file (only the
//! output scale and encoder differ).

use pulsecut_project_model::{frames_to_seconds, AssetId, MediaKind, Project, Rational, TrackKind};
use std::collections::HashSet;
use std::fmt::Write;

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub start_frame: i64,
    /// Exclusive.
    pub end_frame: i64,
    /// Output scale relative to the sequence size (1.0 = full, 0.5 = half...).
    pub scale: f64,
    pub video: bool,
    pub audio: bool,
    /// Assets whose files are missing; their clips are skipped.
    pub offline: HashSet<AssetId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPlan {
    /// Input options and `-filter_complex`; append output options after this.
    pub args: Vec<String>,
    pub video_label: Option<String>,
    pub audio_label: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frame_rate: Rational,
    pub sample_rate: u32,
    pub duration_seconds: f64,
    /// Clips rendered, for diagnostics.
    pub clip_count: usize,
}

const PREROLL: Rational = Rational::HALF;

fn secs(r: Rational) -> String {
    format!("{:.6}", r.as_f64())
}

fn even(v: f64) -> u32 {
    let v = v.round().max(2.0) as u32;
    v + (v % 2)
}

pub fn output_size(project: &Project, scale: f64) -> (u32, u32) {
    let s = &project.sequence;
    (even(s.width as f64 * scale), even(s.height as f64 * scale))
}

pub fn compile(project: &Project, req: &RenderRequest) -> Result<RenderPlan, String> {
    let seq = &project.sequence;
    if req.end_frame <= req.start_frame {
        return Err("render range is empty".into());
    }
    if !(0.05..=1.0).contains(&req.scale) {
        return Err("render scale must be between 0.05 and 1".into());
    }
    let fps = seq.frame_rate;
    let fps_s = format!("{}/{}", fps.num(), fps.den());
    let (w, h) = output_size(project, req.scale);
    let k = w as f64 / seq.width as f64;
    let range_dur = frames_to_seconds(req.end_frame - req.start_frame, fps);
    let dur_s = secs(range_dur);
    let sr = seq.sample_rate;

    let mut args: Vec<String> = vec!["-hide_banner".into(), "-nostdin".into(), "-y".into()];
    let mut graph = String::new();
    let mut input = 0usize;
    let mut clip_count = 0;
    let mut push_input = |args: &mut Vec<String>, opts: Vec<String>| {
        args.extend(opts);
        input += 1;
        input - 1
    };

    let mut video_label = None;
    if req.video {
        let base = push_input(
            &mut args,
            vec![
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                format!("color=c=black:s={w}x{h}:r={fps_s}:d={dur_s}"),
            ],
        );
        let mut cur = format!("[{base}:v]");
        let mut n = 0;
        for track in seq
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Video && !t.muted)
        {
            for clip in &track.clips {
                let (vs, ve) = (
                    clip.start.max(req.start_frame),
                    clip.end().min(req.end_frame),
                );
                if vs >= ve || req.offline.contains(&clip.asset_id) {
                    continue;
                }
                let Some(asset) = project.asset(clip.asset_id) else {
                    continue;
                };
                let Some(vinfo) = &asset.media.video else {
                    continue;
                };
                let seek = clip.source_in + frames_to_seconds(vs - clip.start, fps);
                let len = frames_to_seconds(ve - vs, fps);
                let offset = frames_to_seconds(vs - req.start_frame, fps);
                // Decode from up to PREROLL before the in-point and let `fps` pick frames on
                // the sequence grid, then `trim` exactly. A still frame and a continuous
                // export therefore select the identical source frame for every output frame
                // (seeking straight to the in-point would drop the frame still on screen).
                let is_image = asset.media.kind == MediaKind::Image;
                let pre = if is_image {
                    Rational::ZERO
                } else {
                    seek.min(PREROLL)
                };
                let idx = if is_image {
                    push_input(
                        &mut args,
                        vec![
                            "-loop".into(),
                            "1".into(),
                            "-framerate".into(),
                            fps_s.clone(),
                            "-t".into(),
                            secs(len),
                            "-i".into(),
                            asset.path.clone(),
                        ],
                    )
                } else {
                    push_input(
                        &mut args,
                        vec![
                            "-ss".into(),
                            secs(seek - pre),
                            "-t".into(),
                            secs(len + pre),
                            "-i".into(),
                            asset.path.clone(),
                        ],
                    )
                };
                let (sw, sh) = if vinfo.rotation % 180 == 90 {
                    (vinfo.height, vinfo.width)
                } else {
                    (vinfo.width, vinfo.height)
                };
                let fit = (seq.width as f64 / sw as f64).min(seq.height as f64 / sh as f64)
                    * clip.params.scale
                    * k;
                let (tw, th) = (even(sw as f64 * fit), even(sh as f64 * fit));
                let x = ((w as f64 - tw as f64) / 2.0 + clip.params.position_x * k).round() as i64;
                let y = ((h as f64 - th as f64) / 2.0 + clip.params.position_y * k).round() as i64;
                let off = offset.as_f64();
                let end = off + len.as_f64();
                let shift = (offset - pre).as_f64();
                let q = 0.25 / fps.as_f64();
                let (t_start, t_end) = (off - q, end - q);
                let _ = write!(
                    graph,
                    "[{idx}:v:0]setpts=PTS+{shift:.6}/TB,fps={fps_s},trim=start={t_start:.6}:end={t_end:.6},scale={tw}:{th}:flags=bicubic,setsar=1,format=yuva420p"
                );
                if clip.params.opacity < 1.0 {
                    let _ = write!(graph, ",colorchannelmixer=aa={:.4}", clip.params.opacity);
                }
                let _ = writeln!(graph, "[c{n}];");
                // enable= bounds the clip to its frames even if the decoder emits extra.
                let _ = writeln!(
                    graph,
                    "{cur}[c{n}]overlay=x={x}:y={y}:eof_action=pass:enable='between(t,{:.6},{:.6})'[o{n}];",
                    t_start,
                    t_end
                );
                cur = format!("[o{n}]");
                n += 1;
                clip_count += 1;
            }
        }
        let _ = writeln!(
            graph,
            "{cur}format=yuv420p,trim=duration={dur_s},setpts=PTS-STARTPTS[vout];"
        );
        video_label = Some("[vout]".to_string());
    }

    let mut audio_label = None;
    if req.audio {
        let base = push_input(
            &mut args,
            vec![
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                format!("anullsrc=r={sr}:cl=stereo:d={dur_s}"),
            ],
        );
        let any_solo = seq
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.solo);
        let mut labels = vec![format!("[{base}:a]")];
        let mut n = 0;
        for track in seq
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && !t.muted && (!any_solo || t.solo))
        {
            for clip in &track.clips {
                let (vs, ve) = (
                    clip.start.max(req.start_frame),
                    clip.end().min(req.end_frame),
                );
                if vs >= ve || req.offline.contains(&clip.asset_id) {
                    continue;
                }
                let Some(asset) = project.asset(clip.asset_id) else {
                    continue;
                };
                if asset.media.audio.is_none() {
                    continue;
                }
                let seek = clip.source_in + frames_to_seconds(vs - clip.start, fps);
                let len = frames_to_seconds(ve - vs, fps);
                let offset = frames_to_seconds(vs - req.start_frame, fps);
                let delay_samples = (offset.as_f64() * sr as f64).round() as i64;
                let idx = push_input(
                    &mut args,
                    vec![
                        "-ss".into(),
                        secs(seek),
                        "-t".into(),
                        secs(len),
                        "-i".into(),
                        asset.path.clone(),
                    ],
                );
                let _ = writeln!(
                    graph,
                    "[{idx}:a:0]aresample={sr},aformat=sample_fmts=fltp:channel_layouts=stereo,asetpts=PTS-STARTPTS,atrim=duration={},volume={:.3}dB,adelay=delays={delay_samples}S:all=1[s{n}];",
                    secs(len),
                    clip.params.gain_db
                );
                labels.push(format!("[s{n}]"));
                n += 1;
                clip_count += 1;
            }
        }
        let _ = writeln!(
            graph,
            "{}amix=inputs={}:normalize=0:duration=first:dropout_transition=0,atrim=duration={dur_s},asetpts=PTS-STARTPTS[aout]",
            labels.concat(),
            labels.len()
        );
        audio_label = Some("[aout]".to_string());
    }

    let graph = graph.trim_end().trim_end_matches(';').to_string();
    args.push("-filter_complex".into());
    args.push(graph);
    Ok(RenderPlan {
        args,
        video_label,
        audio_label,
        width: w,
        height: h,
        frame_rate: fps,
        sample_rate: sr,
        duration_seconds: range_dur.as_f64(),
        clip_count,
    })
}

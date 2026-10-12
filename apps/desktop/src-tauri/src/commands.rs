//! Typed IPC surface. The frontend can only reach the project through these commands;
//! every argument is validated here and FFmpeg arguments are built by the engine, never
//! taken from the UI.

use crate::state::{AppState, ProjectState, Session};
use pulsecut_media_engine::{
    self as engine, analysis, ExportSettings, MediaError, PlaybackStream, VerifyReport,
};
use pulsecut_project_model::{
    io as pio, Asset, AssetId, EditCommand, EditError, EditOutcome, Project, Rational, Sequence,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::ipc::{Channel, Response};
use tauri::State;

type CmdResult<T> = Result<T, String>;

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "mkv", "webm", "avi", "mxf", "mts", "m2ts", "wav", "mp3", "aac", "m4a",
    "flac", "ogg", "opus", "png", "jpg", "jpeg", "webp", "bmp",
];

fn user_path(path: &str, exts: &[&str], must_exist: bool) -> CmdResult<PathBuf> {
    if path.contains('\0') || path.len() > 4096 {
        return Err("invalid path".into());
    }
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err("a full file path is required".into());
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !exts.contains(&ext.as_str()) {
        return Err(format!(
            "unsupported file type \".{ext}\" (expected {})",
            exts.join(", ")
        ));
    }
    if must_exist {
        if !p.is_file() {
            return Err(format!("{} does not exist or is not a file", p.display()));
        }
    } else if !p.parent().is_some_and(Path::is_dir) {
        return Err("the destination folder does not exist".into());
    }
    Ok(p)
}

fn edit_err(e: EditError) -> String {
    e.to_string()
}

fn autosave(state: &AppState, session: &Session) {
    let Some(dir) = &state.autosave_dir else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = pio::save(session.editor.project(), &dir.join("recovery.pulsecut"));
    let meta = serde_json::json!({ "originalPath": session.path });
    let _ = std::fs::write(dir.join("recovery.json"), meta.to_string());
}

fn clear_autosave(state: &AppState) {
    if let Some(dir) = &state.autosave_dir {
        let _ = std::fs::remove_file(dir.join("recovery.pulsecut"));
        let _ = std::fs::remove_file(dir.join("recovery.json"));
    }
}

fn stop_playback(state: &AppState) {
    if let Some(p) = state.playback.lock().unwrap().take() {
        p.stop();
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ok: bool,
    pub version: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub h264_encoder: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub fn engine_status(state: State<'_, AppState>) -> EngineStatus {
    match &state.tools {
        Ok(t) => EngineStatus {
            ok: true,
            version: Some(t.version.clone()),
            ffmpeg_path: Some(t.ffmpeg.to_string_lossy().into_owned()),
            h264_encoder: engine::render::video_encoder_args(t, engine::ExportQuality::Standard)
                .ok()
                .map(|(e, _)| e),
            error: None,
        },
        Err(e) => EngineStatus {
            ok: false,
            version: None,
            ffmpeg_path: None,
            h264_encoder: None,
            error: Some(e.to_string()),
        },
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> ProjectState {
    state.session.lock().unwrap().snapshot()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectArgs {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps_num: i64,
    pub fps_den: i64,
}

#[tauri::command]
pub fn new_project(state: State<'_, AppState>, args: NewProjectArgs) -> CmdResult<ProjectState> {
    let fps = Rational::try_new(args.fps_num, args.fps_den).ok_or("invalid frame rate")?;
    let name = args.name.trim();
    let name = if name.is_empty() { "Untitled" } else { name };
    let project = Project::new(
        name.chars().take(120).collect::<String>(),
        Sequence::new("Sequence 1", args.width, args.height, fps, 48000),
    );
    pulsecut_project_model::validate::validate_project(&project)?;
    stop_playback(&state);
    let mut s = state.session.lock().unwrap();
    *s = Session::new(project);
    clear_autosave(&state);
    Ok(s.snapshot())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub state: ProjectState,
    pub imported: Vec<AssetId>,
    pub failures: Vec<ImportFailure>,
}

#[tauri::command]
pub async fn import_media(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CmdResult<ImportResult> {
    let tools = state.tools()?;
    if paths.len() > 500 {
        return Err("import at most 500 files at a time".into());
    }
    let probed = tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .map(|raw| {
                let res = user_path(&raw, MEDIA_EXTENSIONS, true).and_then(|p| {
                    let p = p.canonicalize().unwrap_or(p);
                    engine::probe(&tools, &p)
                        .map(|m| (p, m))
                        .map_err(|e| e.to_string())
                });
                (raw, res)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut s = state.session.lock().unwrap();
    let mut failures = Vec::new();
    let mut new_assets = Vec::new();
    for (raw, res) in probed {
        match res {
            Ok((path, media)) => {
                let path_s = path.to_string_lossy().into_owned();
                let exists = s.editor.project().assets.iter().any(|a| a.path == path_s)
                    || new_assets.iter().any(|a: &Asset| a.path == path_s);
                if exists {
                    failures.push(ImportFailure {
                        path: raw,
                        message: "already in the project".into(),
                    });
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path_s.clone());
                new_assets.push(Asset {
                    id: uuid::Uuid::new_v4(),
                    name,
                    path: path_s,
                    relative_path: None,
                    media,
                });
            }
            Err(message) => failures.push(ImportFailure { path: raw, message }),
        }
    }
    let imported: Vec<AssetId> = new_assets.iter().map(|a| a.id).collect();
    if !new_assets.is_empty() {
        let label = if new_assets.len() == 1 {
            "Import media".to_string()
        } else {
            format!("Import {} files", new_assets.len())
        };
        s.editor
            .mutate(&label, |p| {
                p.assets.extend(new_assets);
                Ok(())
            })
            .map_err(edit_err)?;
        autosave(&state, &s);
    }
    Ok(ImportResult {
        state: s.snapshot(),
        imported,
        failures,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResponse {
    pub state: ProjectState,
    pub outcome: EditOutcome,
}

#[tauri::command]
pub fn apply_edit(state: State<'_, AppState>, command: EditCommand) -> CmdResult<EditResponse> {
    let mut s = state.session.lock().unwrap();
    let outcome = s.editor.apply(&command).map_err(edit_err)?;
    autosave(&state, &s);
    Ok(EditResponse {
        state: s.snapshot(),
        outcome,
    })
}

#[tauri::command]
pub fn undo(state: State<'_, AppState>) -> ProjectState {
    let mut s = state.session.lock().unwrap();
    if s.editor.undo().is_some() {
        s.refresh_offline();
        autosave(&state, &s);
    }
    s.snapshot()
}

#[tauri::command]
pub fn redo(state: State<'_, AppState>) -> ProjectState {
    let mut s = state.session.lock().unwrap();
    if s.editor.redo().is_some() {
        s.refresh_offline();
        autosave(&state, &s);
    }
    s.snapshot()
}

#[tauri::command]
pub fn remove_asset(state: State<'_, AppState>, asset_id: AssetId) -> CmdResult<ProjectState> {
    let mut s = state.session.lock().unwrap();
    s.editor
        .mutate("Remove media", |p| {
            if p.sequence
                .tracks
                .iter()
                .any(|t| t.clips.iter().any(|c| c.asset_id == asset_id))
            {
                return Err(EditError::Invalid(
                    "this media is used in the timeline; delete its clips first".into(),
                ));
            }
            let before = p.assets.len();
            p.assets.retain(|a| a.id != asset_id);
            if p.assets.len() == before {
                return Err(EditError::UnknownAsset(asset_id));
            }
            Ok(())
        })
        .map_err(edit_err)?;
    s.refresh_offline();
    autosave(&state, &s);
    Ok(s.snapshot())
}

#[tauri::command]
pub async fn relink_asset(
    state: State<'_, AppState>,
    asset_id: AssetId,
    path: String,
) -> CmdResult<ProjectState> {
    let tools = state.tools()?;
    let p = user_path(&path, MEDIA_EXTENSIONS, true)?;
    let p = p.canonicalize().unwrap_or(p);
    let probe_path = p.clone();
    let media = tauri::async_runtime::spawn_blocking(move || engine::probe(&tools, &probe_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mut s = state.session.lock().unwrap();
    s.editor
        .mutate("Relink media", |proj| {
            let a = proj
                .assets
                .iter_mut()
                .find(|a| a.id == asset_id)
                .ok_or(EditError::UnknownAsset(asset_id))?;
            if (a.media.video.is_some() && media.video.is_none())
                || (a.media.audio.is_some() && media.audio.is_none())
            {
                return Err(EditError::Invalid(
                    "the replacement file is missing a video or audio stream the original had"
                        .into(),
                ));
            }
            a.path = p.to_string_lossy().into_owned();
            a.media = media;
            Ok(())
        })
        .map_err(|e| match e {
            EditError::Invalid(m) if m.contains("extends past") => {
                "the replacement file is shorter than the clips that use it".to_string()
            }
            e => e.to_string(),
        })?;
    s.refresh_offline();
    autosave(&state, &s);
    Ok(s.snapshot())
}

#[tauri::command]
pub fn save_project(state: State<'_, AppState>, path: Option<String>) -> CmdResult<ProjectState> {
    let mut s = state.session.lock().unwrap();
    let target = match path {
        Some(p) => user_path(&p, &[pio::EXTENSION], false)?,
        None => s.path.clone().ok_or("choose where to save the project")?,
    };
    pio::save(s.editor.project(), &target).map_err(|e| e.to_string())?;
    s.path = Some(target);
    s.editor.mark_saved();
    clear_autosave(&state);
    Ok(s.snapshot())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenResult {
    pub state: ProjectState,
    pub relinked: Vec<AssetId>,
}

#[tauri::command]
pub fn open_project(state: State<'_, AppState>, path: String) -> CmdResult<OpenResult> {
    let p = user_path(&path, &[pio::EXTENSION], true)?;
    let (project, report) = pio::load(&p).map_err(|e| e.to_string())?;
    stop_playback(&state);
    let mut s = state.session.lock().unwrap();
    *s = Session {
        editor: pulsecut_project_model::Editor::opened(project),
        path: Some(p),
        offline: report.offline.into_iter().collect(),
    };
    clear_autosave(&state);
    Ok(OpenResult {
        state: s.snapshot(),
        relinked: report.relinked,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInfo {
    pub project_name: String,
    pub original_path: Option<String>,
    pub modified: Option<u64>,
}

#[tauri::command]
pub fn check_recovery(state: State<'_, AppState>) -> Option<RecoveryInfo> {
    let dir = state.autosave_dir.as_ref()?;
    let file = dir.join("recovery.pulsecut");
    let text = std::fs::read_to_string(&file).ok()?;
    let project = pio::from_json(&text).ok()?;
    let meta: serde_json::Value = std::fs::read_to_string(dir.join("recovery.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let modified = std::fs::metadata(&file)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs());
    Some(RecoveryInfo {
        project_name: project.name,
        original_path: meta["originalPath"].as_str().map(str::to_string),
        modified,
    })
}

#[tauri::command]
pub fn recover_autosave(state: State<'_, AppState>) -> CmdResult<ProjectState> {
    let dir = state.autosave_dir.as_ref().ok_or("no recovery data")?;
    let (project, _) = pio::load(&dir.join("recovery.pulsecut")).map_err(|e| e.to_string())?;
    let meta: serde_json::Value = std::fs::read_to_string(dir.join("recovery.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut s = state.session.lock().unwrap();
    *s = Session::new(project);
    s.path = meta["originalPath"].as_str().map(PathBuf::from);
    s.refresh_offline();
    Ok(s.snapshot())
}

#[tauri::command]
pub fn discard_recovery(state: State<'_, AppState>) {
    clear_autosave(&state);
}

#[tauri::command]
pub async fn render_frame(
    state: State<'_, AppState>,
    frame: i64,
    scale: f64,
) -> CmdResult<Response> {
    let tools = state.tools()?;
    let (project, offline) = {
        let s = state.session.lock().unwrap();
        (s.editor.project().clone(), s.offline.clone())
    };
    let frame = frame.clamp(0, (project.sequence.duration_frames() - 1).max(0));
    let scale = scale.clamp(0.1, 1.0);
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        engine::render_still(&tools, &project, frame, scale, &offline, false)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn render_audio(
    state: State<'_, AppState>,
    start_frame: i64,
    end_frame: i64,
) -> CmdResult<Response> {
    let tools = state.tools()?;
    let (project, offline) = {
        let s = state.session.lock().unwrap();
        (s.editor.project().clone(), s.offline.clone())
    };
    let fps = project.sequence.frame_rate.as_f64();
    let max_frames = (fps * 60.0).ceil() as i64;
    let start = start_frame.max(0);
    let end = end_frame
        .min(project.sequence.duration_frames())
        .min(start + max_frames);
    if end <= start {
        return Ok(Response::new(Vec::new()));
    }
    let pcm = tauri::async_runtime::spawn_blocking(move || {
        engine::render_audio_pcm(&tools, &project, start, end, &offline)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(Response::new(pcm))
}

/// Playback messages: 8-byte little-endian frame index followed by JPEG bytes.
/// Index -1 marks the natural end of the stream; -2 is followed by a UTF-8 error.
#[tauri::command]
pub fn playback_start(
    state: State<'_, AppState>,
    frame: i64,
    scale: f64,
    on_frame: Channel<Response>,
) -> CmdResult<()> {
    let tools = state.tools()?;
    stop_playback(&state);
    let (project, offline) = {
        let s = state.session.lock().unwrap();
        (s.editor.project().clone(), s.offline.clone())
    };
    let end_channel = on_frame.clone();
    let stream = PlaybackStream::start(
        &tools,
        &project,
        frame.max(0),
        scale.clamp(0.1, 1.0),
        &offline,
        Duration::from_millis(250),
        move |index, jpeg| {
            let mut msg = Vec::with_capacity(jpeg.len() + 8);
            msg.extend_from_slice(&index.to_le_bytes());
            msg.extend_from_slice(&jpeg);
            on_frame.send(Response::new(msg)).is_ok()
        },
        move |error| {
            let mut msg = Vec::new();
            match error {
                None => msg.extend_from_slice(&(-1i64).to_le_bytes()),
                Some(e) => {
                    msg.extend_from_slice(&(-2i64).to_le_bytes());
                    msg.extend_from_slice(e.as_bytes());
                }
            }
            let _ = end_channel.send(Response::new(msg));
        },
    )
    .map_err(|e| e.to_string())?;
    *state.playback.lock().unwrap() = Some(stream);
    Ok(())
}

#[tauri::command]
pub fn playback_stop(state: State<'_, AppState>) {
    stop_playback(&state);
}

#[tauri::command]
pub async fn asset_thumbnail(state: State<'_, AppState>, asset_id: AssetId) -> CmdResult<Response> {
    if let Some(t) = state.thumbnails.lock().unwrap().get(&asset_id) {
        return Ok(Response::new(t.as_ref().clone()));
    }
    let tools = state.tools()?;
    let asset = state
        .session
        .lock()
        .unwrap()
        .editor
        .project()
        .asset(asset_id)
        .cloned()
        .ok_or("unknown asset")?;
    if asset.media.video.is_none() {
        return Err("audio-only media has no thumbnail".into());
    }
    let at = (asset.media.duration.as_f64() * 0.1).min(1.0);
    let path = PathBuf::from(&asset.path);
    let bytes =
        tauri::async_runtime::spawn_blocking(move || analysis::thumbnail(&tools, &path, at, 240))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    state
        .thumbnails
        .lock()
        .unwrap()
        .insert(asset_id, Arc::new(bytes.clone()));
    Ok(Response::new(bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Waveform {
    pub peaks_per_second: u32,
    pub peaks: Arc<Vec<f32>>,
}

#[tauri::command]
pub async fn asset_waveform(state: State<'_, AppState>, asset_id: AssetId) -> CmdResult<Waveform> {
    if let Some(w) = state.waveforms.lock().unwrap().get(&asset_id) {
        return Ok(Waveform {
            peaks_per_second: analysis::WAVEFORM_PEAKS_PER_SECOND,
            peaks: w.clone(),
        });
    }
    let tools = state.tools()?;
    let asset = state
        .session
        .lock()
        .unwrap()
        .editor
        .project()
        .asset(asset_id)
        .cloned()
        .ok_or("unknown asset")?;
    if asset.media.audio.is_none() {
        return Err("media has no audio".into());
    }
    let path = PathBuf::from(&asset.path);
    let peaks = tauri::async_runtime::spawn_blocking(move || analysis::waveform(&tools, &path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let peaks = Arc::new(peaks);
    state
        .waveforms
        .lock()
        .unwrap()
        .insert(asset_id, peaks.clone());
    Ok(Waveform {
        peaks_per_second: analysis::WAVEFORM_PEAKS_PER_SECOND,
        peaks,
    })
}

#[derive(Serialize, Clone)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ExportEvent {
    Progress {
        fraction: f64,
        out_seconds: f64,
        total_seconds: f64,
        speed: Option<String>,
    },
    Verifying,
    Done {
        path: String,
        encoder: String,
        elapsed_seconds: f64,
        report: VerifyReport,
    },
    Failed {
        message: String,
        log: String,
    },
    Cancelled,
}

#[tauri::command]
pub fn export_start(
    state: State<'_, AppState>,
    path: String,
    settings: ExportSettings,
    on_event: Channel<ExportEvent>,
) -> CmdResult<()> {
    let tools = state.tools()?;
    let dest = user_path(&path, &["mp4", "mov"], false)?;
    if !(settings.scale == 1.0 || settings.scale == 0.5) {
        return Err("export scale must be full or half resolution".into());
    }
    if !(64..=512).contains(&settings.audio_bitrate_kbps) {
        return Err("audio bitrate must be between 64 and 512 kbps".into());
    }
    let slot = state.export_cancel.clone();
    let cancel = {
        let mut guard = slot.lock().unwrap();
        if guard.is_some() {
            return Err("an export is already running".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *guard = Some(cancel.clone());
        cancel
    };
    let project = state.session.lock().unwrap().editor.project().clone();
    std::thread::spawn(move || {
        let result = engine::export(&tools, &project, &dest, &settings, &cancel, |p| {
            let _ = on_event.send(ExportEvent::Progress {
                fraction: p.fraction,
                out_seconds: p.out_seconds,
                total_seconds: p.total_seconds,
                speed: p.speed,
            });
        });
        let event = match result {
            Ok(res) => {
                let _ = on_event.send(ExportEvent::Verifying);
                match engine::verify_export(&tools, &project, &dest, settings.scale) {
                    Ok(report) => ExportEvent::Done {
                        path: res.path,
                        encoder: res.encoder,
                        elapsed_seconds: res.elapsed_seconds,
                        report,
                    },
                    Err(e) => ExportEvent::Failed {
                        message: format!("the export finished but could not be verified: {e}"),
                        log: String::new(),
                    },
                }
            }
            Err(MediaError::Cancelled) => ExportEvent::Cancelled,
            Err(MediaError::Render { message, log }) => ExportEvent::Failed { message, log },
            Err(e) => ExportEvent::Failed {
                message: e.to_string(),
                log: String::new(),
            },
        };
        slot.lock().unwrap().take();
        let _ = on_event.send(event);
    });
    Ok(())
}

#[tauri::command]
pub fn export_cancel(state: State<'_, AppState>) -> bool {
    match state.export_cancel.lock().unwrap().as_ref() {
        Some(c) => {
            c.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

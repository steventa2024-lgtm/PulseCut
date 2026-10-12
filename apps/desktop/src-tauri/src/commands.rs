use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use media_engine::{
    engine_status, preview_cache_key, probe_file, render_project, thumbnail_jpeg, waveform_peaks,
    RenderReport, RenderSettings,
};
use project_model::{
    from_json, prepare_for_save, resolve_after_load, to_json_pretty, Command, Project, Session,
    SessionSnapshot,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub session: Mutex<Session>,
    pub project_path: Mutex<Option<PathBuf>>,
    pub render_running: Arc<AtomicBool>,
    pub render_cancel: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: Mutex::new(Session::new_empty("Untitled")),
            project_path: Mutex::new(None),
            render_running: Arc::new(AtomicBool::new(false)),
            render_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn lock<'a, T>(mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

fn app_cache(app: &AppHandle) -> PathBuf {
    app.path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pulsecut"))
}

fn cache_root(app: &AppHandle) -> PathBuf {
    if let Some(path) = std::env::var_os("PULSECUT_CACHE") {
        return PathBuf::from(path);
    }
    app_cache(app)
}

fn recovery_path(app: &AppHandle) -> PathBuf {
    cache_root(app).join("recovery").join("autosave.pulsecut")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    project: Project,
    revision: u64,
    can_undo: bool,
    can_redo: bool,
    dirty: bool,
    path: Option<String>,
}

fn ui_from(state: &AppState, snapshot: SessionSnapshot) -> UiState {
    let path = lock(&state.project_path)
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    UiState {
        project: snapshot.project,
        revision: snapshot.revision,
        can_undo: snapshot.can_undo,
        can_redo: snapshot.can_redo,
        dirty: snapshot.dirty,
        path,
    }
}

fn current_ui(state: &AppState) -> UiState {
    let snapshot = lock(&state.session).snapshot();
    ui_from(state, snapshot)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryFile {
    saved_at_unix_ms: u64,
    project_path: Option<String>,
    project: Project,
}

fn write_recovery(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let project = lock(&state.session).project().clone();
    let project_path = lock(&state.project_path)
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let file = RecoveryFile {
        saved_at_unix_ms: unix_ms(),
        project_path,
        project,
    };
    let path = recovery_path(app);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let json = serde_json::to_string_pretty(&file).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("pulsecut.tmp");
    fs::write(&tmp, json).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    Ok(())
}

fn delete_recovery(app: &AppHandle) {
    let _ = fs::remove_file(recovery_path(app));
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineReport {
    ok: bool,
    message: String,
    ffmpeg_path: Option<String>,
    ffprobe_path: Option<String>,
    version: Option<String>,
    libx264: bool,
    aac: bool,
}

#[tauri::command]
pub fn engine_report() -> EngineReport {
    match engine_status() {
        Ok(status) => {
            let ok = status.libx264 && status.aac;
            let message = if ok {
                status.version.clone()
            } else if !status.libx264 {
                "FFmpeg is installed, but this build has no libx264 encoder. Preview and H.264 export need libx264.".into()
            } else {
                "FFmpeg is installed, but this build has no AAC encoder.".into()
            };
            EngineReport {
                ok,
                message,
                ffmpeg_path: Some(status.ffmpeg_path),
                ffprobe_path: Some(status.ffprobe_path),
                version: Some(status.version),
                libx264: status.libx264,
                aac: status.aac,
            }
        }
        Err(err) => EngineReport {
            ok: false,
            message: err.to_string(),
            ffmpeg_path: None,
            ffprobe_path: None,
            version: None,
            libx264: false,
            aac: false,
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    license: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "PulseCut",
        version: env!("CARGO_PKG_VERSION"),
        license: "GPL-3.0-or-later",
    }
}

#[tauri::command]
pub fn editor_state(state: State<AppState>) -> UiState {
    current_ui(&state)
}

#[tauri::command]
pub fn dispatch_command(
    app: AppHandle,
    state: State<AppState>,
    command: Command,
) -> Result<UiState, String> {
    let snapshot = {
        let mut session = lock(&state.session);
        session.dispatch(command).map_err(|err| err.to_string())?
    };
    if let Err(err) = write_recovery(&app, &state) {
        eprintln!("PulseCut recovery write failed: {err}");
    }
    Ok(ui_from(&state, snapshot))
}

#[tauri::command]
pub fn probe_media(path: String) -> Result<project_model::Asset, String> {
    let path = normalize_existing(Path::new(&path))?;
    probe_file(&path)
        .map(|info| info.into_asset())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn new_project(app: AppHandle, state: State<AppState>) -> UiState {
    *lock(&state.session) = Session::new_empty("Untitled");
    *lock(&state.project_path) = None;
    delete_recovery(&app);
    current_ui(&state)
}

#[tauri::command]
pub fn open_project(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<UiState, String> {
    let path = PathBuf::from(&path);
    if !path.is_file() {
        return Err(format!("Project file not found: {}", path.display()));
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read {}: {err}", path.display()))?;
    let mut project = from_json(&text).map_err(|err| err.to_string())?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let _missing = resolve_after_load(&mut project, parent);
    let mut session = Session::new(project).map_err(|err| err.to_string())?;
    session.mark_saved().map_err(|err| err.to_string())?;
    *lock(&state.session) = session;
    *lock(&state.project_path) = Some(path);
    delete_recovery(&app);
    Ok(current_ui(&state))
}

#[tauri::command]
pub fn save_project(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<UiState, String> {
    let path = ensure_extension(PathBuf::from(path), "pulsecut");
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(format!(
                "The folder for that project does not exist: {}",
                parent.display()
            ));
        }
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    {
        let mut session = lock(&state.session);
        let mut document = session.project().clone();
        prepare_for_save(&mut document, parent);
        let json = to_json_pretty(&document).map_err(|err| err.to_string())?;
        let tmp = path.with_extension("pulsecut.tmp");
        fs::write(&tmp, format!("{json}\n"))
            .map_err(|err| format!("Could not write {}: {err}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
        *session.project_mut() = document;
        session.mark_saved().map_err(|err| err.to_string())?;
    }
    *lock(&state.project_path) = Some(path);
    delete_recovery(&app);
    Ok(current_ui(&state))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    mode: String,
    output: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    crf: Option<u8>,
    preset: Option<String>,
    audio_bitrate_kbps: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    mode: String,
    ratio: f64,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    mode: String,
    revision: u64,
    output: String,
    duration_us: i64,
    width: u32,
    height: u32,
    bytes: u64,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    fps_num: Option<u32>,
    fps_den: Option<u32>,
    audio_sample_rate: Option<u32>,
    audio_channels: Option<u32>,
}

#[tauri::command]
pub fn start_render(
    app: AppHandle,
    state: State<AppState>,
    request: RenderRequest,
) -> Result<(), String> {
    if state.render_running.swap(true, Ordering::SeqCst) {
        return Err("A render is already running. Cancel it before starting another.".into());
    }
    let project = lock(&state.session).project().clone();
    let revision = lock(&state.session).snapshot().revision;
    let settings = match settings_for(&project, &request) {
        Ok(settings) => settings,
        Err(err) => {
            state.render_running.store(false, Ordering::SeqCst);
            return Err(err);
        }
    };
    let mode = request.mode.clone();
    let output = match output_for(&app, &project, &settings, &request) {
        Ok(path) => path,
        Err(err) => {
            state.render_running.store(false, Ordering::SeqCst);
            return Err(err);
        }
    };

    if mode == "preview"
        && output.is_file()
        && fs::metadata(&output)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
    {
        match media_engine::report_output(&output) {
            Ok(report) => {
                state.render_running.store(false, Ordering::SeqCst);
                emit_done(&app, &mode, revision, &report);
                return Ok(());
            }
            Err(err) => {
                eprintln!("Cached preview could not be read, rendering again: {err}");
            }
        }
    }

    let cancel = Arc::clone(&state.render_cancel);
    cancel.store(false, Ordering::SeqCst);
    let running = Arc::clone(&state.render_running);
    let app_thread = app.clone();
    let mode_thread = mode.clone();
    let cancel_thread = cancel;

    thread::spawn(move || {
        let partial = output.with_extension("partial.mp4");
        let result = render_project(
            &project,
            &settings,
            &partial,
            |ratio| {
                let _ = app_thread.emit(
                    "render-progress",
                    ProgressEvent {
                        mode: mode_thread.clone(),
                        ratio,
                        message: format!("{}%", (ratio * 100.0).round() as u32),
                    },
                );
            },
            Some(cancel_thread.clone()),
        );
        let cancelled = cancel_thread.load(Ordering::SeqCst);
        let finished = match result {
            Ok(_) if !cancelled => {
                if let Err(err) = fs::rename(&partial, &output) {
                    Err(format!(
                        "The render finished but the file could not be moved into place: {err}"
                    ))
                } else {
                    if mode_thread == "preview" {
                        if let Some(dir) = output.parent() {
                            if let Ok(entries) = fs::read_dir(dir) {
                                for entry in entries.flatten() {
                                    if entry.path() != output
                                        && entry.path().extension().and_then(|ext| ext.to_str())
                                            == Some("mp4")
                                    {
                                        let _ = fs::remove_file(entry.path());
                                    }
                                }
                            }
                        }
                    }
                    media_engine::report_output(&output).map_err(|err| err.to_string())
                }
            }
            Ok(_) => {
                let _ = fs::remove_file(&partial);
                Err("Render cancelled.".into())
            }
            Err(err) => {
                let _ = fs::remove_file(&partial);
                Err(err.to_string())
            }
        };
        running.store(false, Ordering::SeqCst);
        match finished {
            Ok(report) => emit_done(&app_thread, &mode_thread, revision, &report),
            Err(message) if message == "Render cancelled." => {
                let _ = app_thread.emit("render-cancelled", message);
            }
            Err(message) => {
                let _ = app_thread.emit("render-error", message);
            }
        }
    });

    Ok(())
}

fn emit_done(app: &AppHandle, mode: &str, revision: u64, report: &RenderReport) {
    let _ = app.emit(
        "render-done",
        DoneEvent {
            mode: mode.to_string(),
            revision,
            output: report.output.clone(),
            duration_us: report.duration_us,
            width: report.width,
            height: report.height,
            bytes: report.bytes,
            video_codec: report.video_codec.clone(),
            audio_codec: report.audio_codec.clone(),
            fps_num: report.fps_num,
            fps_den: report.fps_den,
            audio_sample_rate: report.audio_sample_rate,
            audio_channels: report.audio_channels,
        },
    );
}

fn settings_for(project: &Project, request: &RenderRequest) -> Result<RenderSettings, String> {
    match request.mode.as_str() {
        "preview" => Ok(RenderSettings::preview_from_sequence(project)),
        "export" => {
            let mut settings = RenderSettings::export_from_sequence(project);
            if let Some(width) = request.width {
                settings.width = width;
            }
            if let Some(height) = request.height {
                settings.height = height;
            }
            if let Some(crf) = request.crf {
                settings.crf = crf;
            }
            if let Some(preset) = &request.preset {
                settings.preset = preset.clone();
            }
            if let Some(rate) = request.audio_bitrate_kbps {
                settings.audio_bitrate_kbps = rate;
            }
            Ok(settings)
        }
        _ => Err("Render mode must be \"preview\" or \"export\".".into()),
    }
}

fn output_for(
    app: &AppHandle,
    project: &Project,
    settings: &RenderSettings,
    request: &RenderRequest,
) -> Result<PathBuf, String> {
    if request.mode == "preview" {
        let dir = app_cache(app).join("previews");
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let key = preview_cache_key(project, settings);
        return Ok(dir.join(format!("{key}.mp4")));
    }
    let raw = request
        .output
        .clone()
        .ok_or_else(|| "Choose a destination for the exported MP4.".to_string())?;
    Ok(ensure_extension(PathBuf::from(raw), "mp4"))
}

#[tauri::command]
pub fn cancel_render(state: State<AppState>) -> Result<(), String> {
    if !state.render_running.load(Ordering::SeqCst) {
        return Err("No render is running.".into());
    }
    state.render_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn thumbnail(path: String, time_us: i64) -> Result<String, String> {
    let path = normalize_existing(Path::new(&path))?;
    let bytes = thumbnail_jpeg(&path, time_us, 320).map_err(|err| err.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub fn waveform(
    path: String,
    source_in_us: i64,
    duration_us: i64,
    buckets: usize,
) -> Result<Vec<f32>, String> {
    let path = normalize_existing(Path::new(&path))?;
    waveform_peaks(&path, source_in_us, duration_us, buckets).map_err(|err| err.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryInfo {
    exists: bool,
    saved_at_unix_ms: Option<u64>,
    project_name: Option<String>,
    project_path: Option<String>,
}

#[tauri::command]
pub fn recovery_info(app: AppHandle) -> RecoveryInfo {
    let path = recovery_path(&app);
    let Ok(text) = fs::read_to_string(&path) else {
        return RecoveryInfo {
            exists: false,
            saved_at_unix_ms: None,
            project_name: None,
            project_path: None,
        };
    };
    match serde_json::from_str::<RecoveryFile>(&text) {
        Ok(file) => RecoveryInfo {
            exists: true,
            saved_at_unix_ms: Some(file.saved_at_unix_ms),
            project_name: Some(file.project.name),
            project_path: file.project_path,
        },
        Err(_) => RecoveryInfo {
            exists: false,
            saved_at_unix_ms: None,
            project_name: None,
            project_path: None,
        },
    }
}

#[tauri::command]
pub fn restore_recovery(app: AppHandle, state: State<AppState>) -> Result<UiState, String> {
    let text = fs::read_to_string(recovery_path(&app))
        .map_err(|_| "No recovery file is available.".to_string())?;
    let mut file: RecoveryFile = serde_json::from_str(&text)
        .map_err(|_| "The recovery file is damaged and cannot be opened.".to_string())?;
    if let Some(project_path) = &file.project_path {
        let parent = Path::new(project_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let _ = resolve_after_load(&mut file.project, parent);
    }
    let mut session = Session::new(file.project).map_err(|err| err.to_string())?;
    session.mark_dirty();
    *lock(&state.session) = session;
    *lock(&state.project_path) = file.project_path.map(PathBuf::from);
    Ok(current_ui(&state))
}

#[tauri::command]
pub fn discard_recovery(app: AppHandle) -> Result<(), String> {
    delete_recovery(&app);
    Ok(())
}

#[tauri::command]
pub fn load_sample(app: AppHandle, state: State<AppState>) -> Result<UiState, String> {
    let media = fixtures_media().ok_or_else(|| {
        "The sample media folder was not found. Generate it with scripts/generate_fixtures.sh from the repository root.".to_string()
    })?;
    let mut session = Session::new_empty("Sample");
    session
        .dispatch(project_model::Command::SetSequence {
            name: Some("Sample".into()),
            width: 1280,
            height: 720,
            fps_num: 30,
            fps_den: 1,
        })
        .map_err(|err| err.to_string())?;
    let red = probed_asset(&media.join("clip_red_720p30.mp4"))?;
    let blue = probed_asset(&media.join("clip_blue_1080p24.mp4"))?;
    let music = probed_asset(&media.join("music_stereo.m4a"))?;
    let red_id = red.id.clone();
    let blue_id = blue.id.clone();
    let music_id = music.id.clone();
    for asset in [red, blue, music] {
        session
            .dispatch(project_model::Command::ImportAsset { asset })
            .map_err(|err| err.to_string())?;
    }
    let v1 = session.project().sequence.tracks[0].id.clone();
    let v2 = session.project().sequence.tracks[1].id.clone();
    let a1 = session.project().sequence.tracks[2].id.clone();
    let a2 = session.project().sequence.tracks[3].id.clone();
    session
        .dispatch(project_model::Command::PlaceAsset {
            asset_id: red_id,
            video_track_id: Some(v1),
            audio_track_id: Some(a1),
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: None,
        })
        .map_err(|err| err.to_string())?;
    session
        .dispatch(project_model::Command::PlaceAsset {
            asset_id: blue_id,
            video_track_id: Some(v2),
            audio_track_id: None,
            timeline_start_us: 2_000_000,
            source_in_us: 500_000,
            duration_us: Some(2_000_000),
        })
        .map_err(|err| err.to_string())?;
    session
        .dispatch(project_model::Command::PlaceAsset {
            asset_id: music_id,
            video_track_id: None,
            audio_track_id: Some(a2),
            timeline_start_us: 0,
            source_in_us: 0,
            duration_us: Some(6_000_000),
        })
        .map_err(|err| err.to_string())?;
    session.mark_dirty();
    *lock(&state.session) = session;
    *lock(&state.project_path) = None;
    if let Err(err) = write_recovery(&app, &state) {
        eprintln!("PulseCut recovery write failed: {err}");
    }
    Ok(current_ui(&state))
}

fn probed_asset(path: &Path) -> Result<project_model::Asset, String> {
    if !path.is_file() {
        return Err(format!(
            "Sample file is missing: {}. Run scripts/generate_fixtures.sh.",
            path.display()
        ));
    }
    probe_file(path)
        .map(|info| info.into_asset())
        .map_err(|err| err.to_string())
}

fn fixtures_media() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    for start in starts {
        let mut dir = start;
        for _ in 0..8 {
            let candidate = dir.join("fixtures/media/clip_red_720p30.mp4");
            if candidate.is_file() {
                return Some(dir.join("fixtures/media"));
            }
            if !dir.pop() {
                break;
            }
        }
    }
    None
}

fn normalize_existing(path: &Path) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(strip_verbatim(canonical))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn ensure_extension(path: PathBuf, extension: &str) -> PathBuf {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case(extension) => path,
        _ => path.with_extension(extension),
    }
}

mod commands;
mod state;

use pulsecut_media_engine::FfmpegTools;
use state::{default_project, AppState, Session};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let autosave_dir = app
                .path()
                .app_local_data_dir()
                .ok()
                .map(|d| d.join("autosave"));
            app.manage(AppState {
                tools: FfmpegTools::locate(),
                session: Mutex::new(Session::new(default_project())),
                playback: Mutex::new(None),
                export_cancel: Arc::new(Mutex::new(None)),
                thumbnails: Mutex::new(HashMap::new()),
                waveforms: Mutex::new(HashMap::new()),
                autosave_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine_status,
            commands::get_state,
            commands::new_project,
            commands::import_media,
            commands::apply_edit,
            commands::undo,
            commands::redo,
            commands::remove_asset,
            commands::relink_asset,
            commands::save_project,
            commands::open_project,
            commands::check_recovery,
            commands::recover_autosave,
            commands::discard_recovery,
            commands::render_frame,
            commands::render_audio,
            commands::playback_start,
            commands::playback_stop,
            commands::asset_thumbnail,
            commands::asset_waveform,
            commands::export_start,
            commands::export_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PulseCut");
}

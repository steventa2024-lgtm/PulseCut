mod commands;
mod preview_http;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let dir = app
                .path()
                .app_cache_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("pulsecut"))
                .join("previews");
            std::fs::create_dir_all(&dir)?;
            preview_http::serve(dir)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine_report,
            commands::app_info,
            commands::editor_state,
            commands::dispatch_command,
            commands::probe_media,
            commands::new_project,
            commands::open_project,
            commands::save_project,
            commands::start_render,
            commands::cancel_render,
            commands::thumbnail,
            commands::waveform,
            commands::recovery_info,
            commands::restore_recovery,
            commands::discard_recovery,
            commands::load_sample,
        ])
        .run(tauri::generate_context!())
        .expect("PulseCut failed to start");
}

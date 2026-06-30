mod commands;
mod models;
mod ops;
mod remote;
mod sidecars;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::SummaryRequestState {
            current_request_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
        .setup(|app| {
            let _ = app.handle();
            let remote_state = remote::RemoteState::new(&app.handle())?;
            app.manage(remote_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_children,
            commands::inspect_path,
            commands::search_entries,
            commands::summarize_paths,
            commands::create_folder,
            commands::create_file,
            commands::rename_path,
            commands::start_delete_paths,
            commands::start_copy_or_move_paths,
            commands::open_path,
            commands::open_with_dialog,
            commands::build_extraction_preview,
            commands::start_extract_archives,
            commands::scan_selection,
            commands::start_convert_to_chd,
            commands::start_restore_from_chd,
            commands::get_app_metadata,
            commands::list_connection_profiles,
            commands::save_connection_profile,
            commands::delete_connection_profile,
            commands::list_active_connections,
            commands::connect_connection_profile,
            commands::test_connection_profile,
            commands::test_connection_profile_payload,
            commands::disconnect_connection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

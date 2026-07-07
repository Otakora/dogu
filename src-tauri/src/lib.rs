mod commands;
mod m3u;
mod models;
mod ops;
mod pause;
mod remote;
mod sidecars;
mod terminal;

use tauri::Manager;

pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::SummaryRequestState {
            current_request_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        })
        .manage(pause::PauseRegistry::new())
        .manage(terminal::TerminalManager::new())
        .setup(|app| {
            let _ = app.handle();
            let remote_state = remote::RemoteState::new(&app.handle())?;
            app.manage(remote_state);
            // Clean up temp dirs left by any previous session that crashed or was force-killed.
            ops::sweep_temp_dirs(&app.handle());
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
            commands::build_extraction_preview_deep,
            commands::start_extract_archives,
            commands::scan_selection,
            commands::start_convert_to_chd,
            commands::start_restore_from_chd,
            commands::detect_disc_kinds,
            commands::start_convert_to_cso,
            commands::start_restore_from_cso,
            commands::start_pack_to_xiso,
            commands::start_unpack_xiso,
            commands::check_tools,
            commands::start_convert_to_rvz,
            commands::start_restore_from_rvz,
            commands::resume_job,
            commands::preflight_remote_transfer,
            commands::get_app_metadata,
            commands::list_connection_profiles,
            commands::save_connection_profile,
            commands::delete_connection_profile,
            commands::list_active_connections,
            commands::connect_connection_profile,
            commands::test_connection_profile,
            commands::test_connection_profile_payload,
            commands::disconnect_connection,
            commands::list_volumes,
            commands::get_remote_disk_usage,
            commands::get_known_folders,
            commands::create_terminal,
            commands::create_terminal_for_remote_session,
            commands::terminal_input,
            commands::resize_terminal,
            commands::close_terminal,
            commands::get_available_shells,
            commands::scan_for_m3u_groups,
            commands::generate_m3u_files,
            commands::get_compression_capabilities,
            commands::start_compress
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

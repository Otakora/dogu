use std::path::PathBuf;

use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use tauri::{AppHandle, State};

use crate::{
    models::{
        ActiveConnectionDto, AppMetadataDto, ChdConversionOptionsPayload, ChdRestoreOptionsPayload,
        ConnectionOpenResultDto, ConnectionProfileDto, ConnectionProfilePayload, EntryDto,
        ExtractionOptionsPayload, ExtractionPreviewRow, PropertiesSummaryDto, SelectionAnalysisDto,
        SummaryOptionsPayload,
    },
    ops, remote,
};

pub struct SummaryRequestState {
    pub current_request_id: Arc<AtomicU64>,
}

fn parse_paths(paths: Vec<String>) -> Vec<PathBuf> {
    paths.into_iter().map(PathBuf::from).collect()
}

#[tauri::command]
pub fn list_children(remote_state: State<'_, remote::RemoteState>, path: String) -> Result<Vec<EntryDto>, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .list_children(&path)
            .map_err(|error| error.to_string());
    }
    ops::list_children(&PathBuf::from(path)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn inspect_path(remote_state: State<'_, remote::RemoteState>, path: String) -> Result<EntryDto, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .inspect_path(&path)
            .map_err(|error| error.to_string());
    }
    ops::entry_from_path(&PathBuf::from(path)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_entries(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
    query: String,
    recursive: bool,
) -> Result<Vec<EntryDto>, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .search_entries(&path, &query, recursive)
            .map_err(|error| error.to_string());
    }
    ops::search_entries(&PathBuf::from(path), &query, recursive).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn summarize_paths(
    state: State<'_, SummaryRequestState>,
    remote_state: State<'_, remote::RemoteState>,
    request_id: u64,
    paths: Vec<String>,
    options: SummaryOptionsPayload,
) -> Result<PropertiesSummaryDto, String> {
    state.current_request_id.store(request_id, Ordering::Relaxed);
    if paths
        .first()
        .map(|path| remote::RemoteManager::is_remote_path(path))
        .unwrap_or(false)
    {
        return remote_state
            .inner
            .summarize_paths(&paths, &options)
            .map_err(|error| error.to_string());
    }
    let parsed = parse_paths(paths);
    let state_ref = state.current_request_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ops::summarize_paths(&parsed, &options, Some((&state_ref, request_id))).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn create_folder(
    remote_state: State<'_, remote::RemoteState>,
    parent: String,
    name: String,
) -> Result<String, String> {
    if remote::RemoteManager::is_remote_path(&parent) {
        return remote_state
            .inner
            .create_folder(&parent, &name)
            .map_err(|error| error.to_string());
    }
    ops::create_folder(&PathBuf::from(parent), &name).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_file(
    remote_state: State<'_, remote::RemoteState>,
    parent: String,
    name: String,
) -> Result<String, String> {
    if remote::RemoteManager::is_remote_path(&parent) {
        return remote_state
            .inner
            .create_file(&parent, &name)
            .map_err(|error| error.to_string());
    }
    ops::create_file(&PathBuf::from(parent), &name).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_path(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
    new_name: String,
) -> Result<String, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .rename_path(&path, &new_name)
            .map_err(|error| error.to_string());
    }
    ops::rename_path(&PathBuf::from(path), &new_name).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_path(
    app: AppHandle,
    remote_state: State<'_, remote::RemoteState>,
    path: String,
) -> Result<(), String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .open_path(&app, &path, false)
            .map_err(|error| error.to_string());
    }
    ops::open_path(&PathBuf::from(path)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_with_dialog(
    app: AppHandle,
    remote_state: State<'_, remote::RemoteState>,
    path: String,
) -> Result<(), String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .open_path(&app, &path, true)
            .map_err(|error| error.to_string());
    }
    ops::open_with_dialog(&PathBuf::from(path)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn build_extraction_preview(
    archives: Vec<String>,
    options: ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>, String> {
    ops::build_extraction_preview(&parse_paths(archives), &options).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn scan_selection(paths: Vec<String>) -> Result<SelectionAnalysisDto, String> {
    ops::scan_selection(&parse_paths(paths)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_app_metadata(app: AppHandle) -> Result<AppMetadataDto, String> {
    Ok(AppMetadataDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        chdman: ops::load_chdman_metadata(&app),
    })
}

#[tauri::command]
pub fn start_delete_paths(
    app: AppHandle,
    remote_state: State<'_, remote::RemoteState>,
    job_id: String,
    paths: Vec<String>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let remote_manager = remote_state.inner.clone();
    let is_remote = paths
        .first()
        .map(|path| remote::RemoteManager::is_remote_path(path))
        .unwrap_or(false);
    let parsed_paths = parse_paths(paths);
    tauri::async_runtime::spawn_blocking(move || {
        let result = if is_remote {
            remote_manager.delete_paths(
                &app_handle,
                &job_id,
                parsed_paths
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
            )
        } else {
            ops::delete_paths(&app_handle, &job_id, parsed_paths)
        };
        let message = match &result {
            Ok(_) => "Operacion completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_copy_or_move_paths(
    app: AppHandle,
    remote_state: State<'_, remote::RemoteState>,
    job_id: String,
    paths: Vec<String>,
    destination: String,
    operation: String,
    overwrite: bool,
) -> Result<(), String> {
    let app_handle = app.clone();
    let remote_manager = remote_state.inner.clone();
    let parsed_paths = parse_paths(paths.clone());
    let destination_path = PathBuf::from(destination.clone());
    let is_remote = paths
        .first()
        .map(|path| remote::RemoteManager::is_remote_path(path) || remote::RemoteManager::is_remote_path(&destination))
        .unwrap_or_else(|| remote::RemoteManager::is_remote_path(&destination));
    tauri::async_runtime::spawn_blocking(move || {
        let result = if is_remote {
            remote_manager.copy_or_move_paths(
                &app_handle,
                &job_id,
                parsed_paths
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect(),
                destination_path.to_string_lossy().to_string(),
                &operation,
                overwrite,
            )
        } else {
            ops::copy_or_move_paths(
                &app_handle,
                &job_id,
                parsed_paths,
                destination_path,
                &operation,
                overwrite,
            )
        };
        let message = match &result {
            Ok(_) => "Operacion completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_extract_archives(
    app: AppHandle,
    job_id: String,
    archives: Vec<String>,
    options: ExtractionOptionsPayload,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(archives);
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::extract_archives(&app_handle, &job_id, parsed_paths, options);
        let message = match &result {
            Ok(_) => "Descompresion completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_convert_to_chd(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: ChdConversionOptionsPayload,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::convert_to_chd(&app_handle, &job_id, &parsed_paths, options);
        let message = match &result {
            Ok(_) => "Conversion CHD completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_restore_from_chd(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: ChdRestoreOptionsPayload,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::restore_from_chd(&app_handle, &job_id, &parsed_paths, options);
        let message = match &result {
            Ok(_) => "Recuperacion desde CHD completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn list_connection_profiles(
    remote_state: State<'_, remote::RemoteState>,
) -> Result<Vec<ConnectionProfileDto>, String> {
    remote_state.inner.list_profiles().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile: ConnectionProfilePayload,
) -> Result<ConnectionProfileDto, String> {
    remote_state
        .inner
        .save_profile(profile)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile_id: String,
) -> Result<(), String> {
    remote_state
        .inner
        .delete_profile(&profile_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_active_connections(
    remote_state: State<'_, remote::RemoteState>,
) -> Result<Vec<ActiveConnectionDto>, String> {
    remote_state
        .inner
        .list_active_connections()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn connect_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile_id: String,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    remote_state
        .inner
        .connect_profile(&profile_id, trust_current_fingerprint)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn test_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile_id: String,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    remote_state
        .inner
        .test_profile(&profile_id, trust_current_fingerprint)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn test_connection_profile_payload(
    remote_state: State<'_, remote::RemoteState>,
    profile: ConnectionProfilePayload,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    remote_state
        .inner
        .test_profile_payload(profile, trust_current_fingerprint)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn disconnect_connection(
    remote_state: State<'_, remote::RemoteState>,
    session_id: String,
) -> Result<(), String> {
    remote_state
        .inner
        .disconnect_connection(&session_id)
        .map_err(|error| error.to_string())
}

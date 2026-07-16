use std::path::PathBuf;

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use tauri::{AppHandle, State};

use crate::{
    m3u,
    models::{
        ActiveConnectionDto, AppMetadataDto, AvailableShell, ChdConversionOptionsPayload,
        ChdRestoreOptionsPayload, CompressionCapabilitiesDto, CompressionOptionsPayload,
        ConnectionOpenResultDto, ConnectionProfileDto, ConnectionProfilePayload,
        DiscImageOptionsPayload, EntryDto, ExtractionOptionsPayload, ExtractionPreviewEntry,
        ExtractionPreviewRow, KnownFoldersDto, M3uGeneratePayload, M3uGenerateResultDto,
        M3uGroupDto, M3uScanOptions, PreflightCheckResult, PropertiesSummaryDto,
        RemoteDiskUsageDto, SelectionAnalysisDto, SummaryOptionsPayload, VolumeDto,
    },
    ops, pause, remote, terminal,
};

pub struct SummaryRequestState {
    pub current_request_id: Arc<AtomicU64>,
}

fn parse_paths(paths: Vec<String>) -> Vec<PathBuf> {
    paths.into_iter().map(PathBuf::from).collect()
}

#[tauri::command]
pub async fn list_children(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
) -> Result<Vec<EntryDto>, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        let rm = remote_state.inner.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            rm.list_children(&path).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let path_buf = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        ops::list_children(&path_buf).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn inspect_path(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
) -> Result<EntryDto, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        let rm = remote_state.inner.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            rm.inspect_path(&path).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let path_buf = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        ops::entry_from_path(&path_buf).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn search_entries(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
    query: String,
    recursive: bool,
) -> Result<Vec<EntryDto>, String> {
    if remote::RemoteManager::is_remote_path(&path) {
        let rm = remote_state.inner.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            rm.search_entries(&path, &query, recursive)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let path_buf = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        ops::search_entries(&path_buf, &query, recursive).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn summarize_paths(
    state: State<'_, SummaryRequestState>,
    remote_state: State<'_, remote::RemoteState>,
    request_id: u64,
    paths: Vec<String>,
    options: SummaryOptionsPayload,
) -> Result<PropertiesSummaryDto, String> {
    state
        .current_request_id
        .store(request_id, Ordering::Relaxed);
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
        ops::summarize_paths(&parsed, &options, Some((&state_ref, request_id)))
            .map_err(|error| error.to_string())
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

/// Synchronous single-entry delete for the destination picker (file or folder,
/// recursive). Dual local/remote, mirroring `create_folder`/`rename_path`.
#[tauri::command]
pub fn delete_entry(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
) -> Result<(), String> {
    if remote::RemoteManager::is_remote_path(&path) {
        return remote_state
            .inner
            .remove_entry(&path)
            .map_err(|error| error.to_string());
    }
    ops::delete_entry(&PathBuf::from(path)).map_err(|error| error.to_string())
}

/// Renames files to ASCII-safe (de-accented) names so tools that can't handle
/// non-ASCII paths (chdman) can process them. `new_name` is computed by the
/// frontend; the backend only validates it is pure ASCII and applies it. Renames
/// deepest paths first so a parent rename can't invalidate a still-pending child.
#[tauri::command]
pub fn deaccent_rename(
    remote_state: State<'_, remote::RemoteState>,
    renames: Vec<crate::models::DeaccentRenameRequest>,
) -> Result<Vec<crate::models::DeaccentRenameResult>, String> {
    for r in &renames {
        if !r.new_name.is_ascii() || r.new_name.trim().is_empty() {
            return Err(format!("Nombre de destino no valido: '{}'", r.new_name));
        }
    }
    let mut ordered = renames;
    ordered.sort_by_key(|r| std::cmp::Reverse(r.path.matches(['/', '\\']).count()));

    let mut results = Vec::with_capacity(ordered.len());
    for r in &ordered {
        let new_path = if remote::RemoteManager::is_remote_path(&r.path) {
            remote_state
                .inner
                .rename_path(&r.path, &r.new_name)
                .map_err(|error| error.to_string())?
        } else {
            ops::rename_path(&PathBuf::from(&r.path), &r.new_name)
                .map_err(|error| error.to_string())?
        };
        results.push(crate::models::DeaccentRenameResult {
            old_path: r.path.clone(),
            new_path,
        });
    }
    Ok(results)
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

/// Deep variant of `build_extraction_preview`: enumerates every file/folder at
/// all levels of each (local) archive so the frontend can build navigable nested
/// ghost trees. Remote archives are skipped (contents aren't inspectable without
/// downloading them).
#[tauri::command]
pub fn build_extraction_preview_deep(
    app: AppHandle,
    archives: Vec<String>,
    options: ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>, String> {
    let local: Vec<String> = archives
        .into_iter()
        .filter(|p| !remote::RemoteManager::is_remote_path(p))
        .collect();
    if local.is_empty() {
        return Ok(Vec::new());
    }
    ops::build_extraction_preview_deep(&app, &parse_paths(local), &options)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn build_extraction_preview(
    app: AppHandle,
    archives: Vec<String>,
    options: ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>, String> {
    let (remote_archives, local_archives): (Vec<String>, Vec<String>) = archives
        .into_iter()
        .partition(|p| remote::RemoteManager::is_remote_path(p));

    let mut rows: Vec<ExtractionPreviewRow> = Vec::new();

    // Remote archives: generate a placeholder row (contents can't be previewed without downloading).
    for path in &remote_archives {
        let dest_root =
            if options.destination_mode != "custom" || options.destination_path.is_none() {
                "(carpeta del archivo remoto)".to_string()
            } else {
                options.destination_path.clone().unwrap_or_default()
            };
        rows.push(ExtractionPreviewRow {
            archive_path: path.clone(),
            destination_root: dest_root,
            entries: vec![ExtractionPreviewEntry {
                name: "(contenido del archivo)".to_string(),
                is_dir: true,
                destination_path: String::new(),
            }],
        });
    }

    // Local archives: use normal preview.
    if !local_archives.is_empty() {
        let local_rows =
            ops::build_extraction_preview(&app, &parse_paths(local_archives), &options)
                .map_err(|error| error.to_string())?;
        rows.extend(local_rows);
    }

    Ok(rows)
}

#[tauri::command]
pub fn scan_selection(
    remote_state: State<'_, remote::RemoteState>,
    paths: Vec<String>,
    max_depth: Option<u32>,
) -> Result<SelectionAnalysisDto, String> {
    let depth = max_depth.map(|d| d as usize).unwrap_or(3);
    let rm = remote_state.inner.clone();
    ops::scan_selection(&parse_paths(paths), depth, Some(rm)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app_metadata(app: AppHandle) -> Result<AppMetadataDto, String> {
    Ok(AppMetadataDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        chdman: ops::load_chdman_metadata(&app),
        chdman_runtime: ops::probe_chdman_runtime(&app),
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
    rename_on_conflict: Option<bool>,
) -> Result<(), String> {
    let rename_on_conflict = rename_on_conflict.unwrap_or(false);
    let app_handle = app.clone();
    let remote_manager = remote_state.inner.clone();
    let parsed_paths = parse_paths(paths.clone());
    let destination_path = PathBuf::from(destination.clone());
    let is_remote = paths
        .first()
        .map(|path| {
            remote::RemoteManager::is_remote_path(path)
                || remote::RemoteManager::is_remote_path(&destination)
        })
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
                rename_on_conflict,
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
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(archives);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::extract_archives(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Descompresion completada.".to_string(),
            Err(error) => {
                // Emit error as a log line so it appears in the progress panel.
                let _ = ops::emit_log(&app_handle, &job_id, format!("[ERROR] {}", error));
                error.to_string()
            }
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
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::convert_to_chd(
            &app_handle,
            &job_id,
            &parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
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
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::restore_from_chd(
            &app_handle,
            &job_id,
            &parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Recuperacion desde CHD completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_convert_to_cso(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::convert_to_cso(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Conversion CSO completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_restore_from_cso(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::restore_from_cso(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Restauracion CSO completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

/// Classify local `.iso` selections by content (trimmed XISO / Xbox redump /
/// GameCube-Wii / generic ISO) so the UI only offers disc treatments that are
/// actually valid. Cheap: a few small reads per file, remote/non-iso skipped.
#[tauri::command]
pub fn detect_disc_kinds(paths: Vec<String>) -> Vec<crate::models::DiscKindDto> {
    ops::detect_disc_kinds(parse_paths(paths))
}

/// Probes every external tool at startup (chdman, 7-Zip, DolphinTool) and reports
/// availability + the capabilities each unlocks, so the UI can prepare/guide and
/// disable features whose tool is missing.
#[tauri::command]
pub fn check_tools(app: AppHandle) -> Vec<crate::models::ToolStatusDto> {
    ops::check_tools(&app)
}

#[tauri::command]
pub fn start_convert_to_rvz(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::convert_to_rvz(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Conversion RVZ completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_restore_from_rvz(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::restore_from_rvz(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Restauracion RVZ completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_pack_to_xiso(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::pack_to_xiso(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Conversion XISO completada.".to_string(),
            Err(error) => error.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

#[tauri::command]
pub fn start_unpack_xiso(
    app: AppHandle,
    job_id: String,
    paths: Vec<String>,
    options: DiscImageOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(paths);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::unpack_xiso(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Extraccion XISO completada.".to_string(),
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
    remote_state
        .inner
        .list_profiles()
        .map_err(|error| error.to_string())
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
pub async fn connect_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile_id: String,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    let rm = remote_state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rm.connect_profile(&profile_id, trust_current_fingerprint)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn test_connection_profile(
    remote_state: State<'_, remote::RemoteState>,
    profile_id: String,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    let rm = remote_state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rm.test_profile(&profile_id, trust_current_fingerprint)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn test_connection_profile_payload(
    remote_state: State<'_, remote::RemoteState>,
    profile: ConnectionProfilePayload,
    trust_current_fingerprint: bool,
) -> Result<ConnectionOpenResultDto, String> {
    let rm = remote_state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rm.test_profile_payload(profile, trust_current_fingerprint)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn disconnect_connection(
    remote_state: State<'_, remote::RemoteState>,
    session_id: String,
) -> Result<(), String> {
    let rm = remote_state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rm.disconnect_connection(&session_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn list_volumes() -> Result<Vec<VolumeDto>, String> {
    tauri::async_runtime::spawn_blocking(|| ops::list_volumes().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_remote_disk_usage(
    remote_state: State<'_, remote::RemoteState>,
    path: String,
    force_refresh: Option<bool>,
) -> Result<RemoteDiskUsageDto, String> {
    if !remote::RemoteManager::is_remote_path(&path) {
        return Err("La ruta indicada no es remota.".to_string());
    }
    let rm = remote_state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        rm.get_disk_usage(&path, force_refresh.unwrap_or(false))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_known_folders() -> KnownFoldersDto {
    ops::get_known_folders()
}

// ── M3U commands ───────────────────────────────────────────

/// Scan one or more directories and return proposed M3U groups with conflict warnings.
/// Supports both local and remote paths.
#[tauri::command]
pub async fn scan_for_m3u_groups(
    remote_state: State<'_, remote::RemoteState>,
    dirs: Vec<String>,
    options: M3uScanOptions,
) -> Result<Vec<M3uGroupDto>, String> {
    let is_remote = dirs
        .first()
        .map(|p| remote::RemoteManager::is_remote_path(p))
        .unwrap_or(false);

    let scan_roots: Vec<String> = dirs.iter().map(|d| d.replace('\\', "/")).collect();

    if is_remote {
        let rm = remote_state.inner.clone();
        let recursive = options.recursive;
        return tauri::async_runtime::spawn_blocking(move || {
            let mut raw_files = Vec::new();
            for dir in &dirs {
                let virtual_paths = if recursive {
                    rm.scan_files_recursive(dir).map_err(|e| e.to_string())?
                } else {
                    // Non-recursive: use list_children, keep only files
                    rm.list_children(dir)
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .filter(|e| !e.is_dir)
                        .map(|e| e.path)
                        .collect()
                };
                for vp in virtual_paths {
                    if let Some(rf) = m3u::raw_file_from_remote_path(&vp) {
                        raw_files.push(rf);
                    }
                }
            }
            Ok(m3u::build_groups(raw_files, &scan_roots, &options))
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    let paths: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let raw_files = m3u::scan_local(&paths, options.recursive);
        Ok(m3u::build_groups(raw_files, &scan_roots, &options))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Write the confirmed M3U files to disk (local or remote).
#[tauri::command]
pub async fn generate_m3u_files(
    remote_state: State<'_, remote::RemoteState>,
    payload: M3uGeneratePayload,
) -> Result<M3uGenerateResultDto, String> {
    let is_remote = payload
        .groups
        .first()
        .map(|g| remote::RemoteManager::is_remote_path(&g.output_path))
        .unwrap_or(false);

    if is_remote {
        let rm = remote_state.inner.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let mut created = Vec::new();
            let mut skipped = Vec::new();
            let mut failed = Vec::new();

            for group in &payload.groups {
                // Split output_path into parent dir + filename
                let out = &group.output_path;
                let last_slash = out.rfind('/').unwrap_or(0);
                let parent = &out[..last_slash];
                let filename = &out[last_slash + 1..];

                // Check existence only for skip logic (best-effort)
                if !payload.overwrite {
                    let children = rm.list_children(parent);
                    if let Ok(list) = children {
                        if list.iter().any(|e| e.name.eq_ignore_ascii_case(filename)) {
                            skipped.push(out.clone());
                            continue;
                        }
                    }
                }

                let content = m3u::format_m3u_content(&group.base_name, &group.entries);
                match rm.write_text_file(parent, filename, &content) {
                    Ok(_) => created.push(out.clone()),
                    Err(e) => failed.push(crate::models::M3uFailureDto {
                        path: out.clone(),
                        error: e.to_string(),
                    }),
                }
            }
            Ok(M3uGenerateResultDto {
                created,
                skipped,
                failed,
            })
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    tauri::async_runtime::spawn_blocking(move || {
        Ok(m3u::generate_all_local(
            &payload.groups,
            payload.overwrite,
            payload.rename_on_conflict,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Terminal commands ──────────────────────────────────────

#[tauri::command]
pub fn create_terminal(
    tm: State<'_, terminal::TerminalManager>,
    app_handle: AppHandle,
    id: String,
    cwd: String,
    shell: Option<String>,
) -> Result<(), String> {
    terminal::create_terminal(&tm, app_handle, id, cwd, shell).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_input(
    tm: State<'_, terminal::TerminalManager>,
    id: String,
    data: String,
) -> Result<(), String> {
    terminal::send_input(&tm, &id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_terminal(
    tm: State<'_, terminal::TerminalManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    terminal::resize_terminal(&tm, &id, cols, rows).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_terminal(tm: State<'_, terminal::TerminalManager>, id: String) {
    terminal::close_terminal(&tm, &id);
}

#[tauri::command]
pub fn get_available_shells() -> Vec<AvailableShell> {
    terminal::get_available_shells()
}

#[tauri::command]
pub fn create_terminal_for_remote_session(
    remote_state: State<'_, remote::RemoteState>,
    tm: State<'_, terminal::TerminalManager>,
    app_handle: AppHandle,
    virtual_path: String,
    id: String,
) -> Result<(), String> {
    let params = remote_state
        .inner
        .get_ssh_terminal_params(&virtual_path)
        .map_err(|e| e.to_string())?;
    terminal::probe_ssh_terminal(
        &params.host,
        params.port,
        &params.username,
        &params.password,
    )
    .map_err(|e| e.to_string())?;
    terminal::create_ssh_terminal(
        &tm,
        app_handle,
        id,
        params.host,
        params.port,
        params.username,
        params.password,
        params.remote_path,
    )
    .map_err(|e| e.to_string())
}

// ── Pause / resume ─────────────────────────────────────────

/// Called by the frontend to unblock a paused job.
/// `decision` must be one of: "retry", "skip", "abort".
#[tauri::command]
pub fn resume_job(
    pause_registry: State<'_, pause::PauseRegistry>,
    job_id: String,
    decision: String,
) -> Result<(), String> {
    pause_registry.resume(&job_id, pause::PauseDecision::from_str(&decision));
    Ok(())
}

// ── Pre-flight remote transfer check ──────────────────────

/// Checks available local temp space against the estimated output size.
/// Accepts the source file paths and an operation kind: "extract" | "chd-convert" | "chd-restore".
#[tauri::command]
pub fn preflight_remote_transfer(
    app: AppHandle,
    sources: Vec<String>,
    op_kind: String,
) -> Result<PreflightCheckResult, String> {
    let paths: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    ops::preflight_remote_transfer(&app, &paths, &op_kind).map_err(|e| e.to_string())
}

// ── Compression ───────────────────────────────────────────

#[tauri::command]
pub fn get_compression_capabilities(app: AppHandle) -> CompressionCapabilitiesDto {
    ops::get_compression_capabilities(&app)
}

#[tauri::command]
pub fn start_compress(
    app: AppHandle,
    job_id: String,
    sources: Vec<String>,
    options: CompressionOptionsPayload,
    remote_state: State<'_, remote::RemoteState>,
    pause_registry: State<'_, pause::PauseRegistry>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let parsed_paths = parse_paths(sources);
    let rm = remote_state.inner.clone();
    let pr = (*pause_registry).clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = ops::compress_to_archive(
            &app_handle,
            &job_id,
            parsed_paths,
            options,
            Some(rm),
            Some(pr),
        );
        let message = match &result {
            Ok(_) => "Compresión completada.".to_string(),
            Err(e) => e.to_string(),
        };
        let _ = ops::emit_finished(&app_handle, &job_id, result.is_ok(), message);
    });
    Ok(())
}

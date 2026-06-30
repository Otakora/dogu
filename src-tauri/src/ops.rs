use std::{
    sync::atomic::{AtomicU64, Ordering},
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::{
    models::{
        ChdConversionOptionsPayload, ChdRestoreOptionsPayload, ChdSourceDto, EntryDto,
        ExtractionOptionsPayload, ExtractionPreviewRow, JobFinishedDto, JobLogDto,
        JobProgressDto, PropertiesSummaryDto, SelectionAnalysisDto, SummaryOptionsPayload,
    },
    sidecars::{chdman_path, runtime_root, seven_zip_path},
};

const ARCHIVE_EXTENSIONS: &[&str] = &[".zip", ".7z", ".rar"];
const CD_SOURCE_EXTENSIONS: &[&str] = &[".cue", ".gdi", ".toc"];
const DVD_SOURCE_EXTENSIONS: &[&str] = &[".iso"];
const PAIRABLE_BIN_EXTENSIONS: &[&str] = &[".bin"];

pub fn entry_from_path(path: &Path) -> Result<EntryDto> {
    let metadata = fs::metadata(path)?;
    let is_dir = metadata.is_dir();
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let modified_ts = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (size, size_label, has_children, has_directory_children) = if is_dir {
        let mut child_count = 0usize;
        let mut has_directory_children = false;
        if let Ok(items) = fs::read_dir(path) {
            for item in items.flatten() {
                child_count += 1;
                if !has_directory_children {
                    has_directory_children = item
                        .file_type()
                        .map(|file_type| file_type.is_dir())
                        .unwrap_or(false);
                }
            }
        }
        let label = match child_count {
            0 => String::from("Vacia"),
            1 => String::from("1 elemento"),
            n => format!("{n} elementos"),
        };
        (0, label, child_count > 0, has_directory_children)
    } else {
        (metadata.len(), format_size(metadata.len()), false, false)
    };
    Ok(EntryDto {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .unwrap_or_else(|| path.as_os_str())
            .to_string_lossy()
            .to_string(),
        is_dir,
        size,
        size_label,
        modified_ts,
        modified_label: format_timestamp(modified),
        extension: path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| format!(".{}", ext.to_ascii_lowercase()))
            .unwrap_or_default(),
        has_children,
        has_directory_children,
        location_kind: "local".to_string(),
        display_path: path.to_string_lossy().to_string(),
        root_label: None,
    })
}

pub fn list_children(path: &Path) -> Result<Vec<EntryDto>> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("No se pudo leer la carpeta: {}", path.display()))?
        .filter_map(|item| item.ok())
        .filter_map(|item| entry_from_path(&item.path()).ok())
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn search_entries(root: &Path, query: &str, recursive: bool) -> Result<Vec<EntryDto>> {
    let lowered = query.trim().to_lowercase();
    if lowered.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    if recursive {
        for item in WalkDir::new(root).into_iter().filter_map(|entry| entry.ok()) {
            let path = item.path();
            if path == root {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .to_lowercase();
            if name.contains(&lowered) {
                if let Ok(entry) = entry_from_path(path) {
                    results.push(entry);
                }
            }
        }
    } else {
        for entry in list_children(root)? {
            if entry.name.to_lowercase().contains(&lowered) {
                results.push(entry);
            }
        }
    }

    results.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .reverse()
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(results)
}

pub fn summarize_paths(
    paths: &[PathBuf],
    options: &SummaryOptionsPayload,
    cancel_state: Option<(&AtomicU64, u64)>,
) -> Result<PropertiesSummaryDto> {
    let mut lines = Vec::new();
    let mut total_size = 0_u64;
    let mut files = 0_usize;
    let mut directories = 0_usize;

    for path in paths {
        ensure_summary_not_cancelled(cancel_state)?;
        let metadata = fs::metadata(path)
            .with_context(|| format!("No se pudo leer metadata de {}", path.display()))?;
        if metadata.is_dir() {
            let (size, file_count, dir_count) =
                compute_directory_stats(path, options.max_depth, cancel_state)?;
            total_size += size;
            files += file_count;
            directories += dir_count;
            lines.push(format!(
                "[DIR] {} | {} | {} | {} ficheros, {} subcarpetas",
                path.file_name()
                    .unwrap_or_else(|| path.as_os_str())
                    .to_string_lossy(),
                path.display(),
                format_size(size),
                file_count,
                dir_count
            ));
        } else {
            files += 1;
            total_size += metadata.len();
            lines.push(format!(
                "[FILE] {} | {} | {}",
                path.file_name()
                    .unwrap_or_else(|| path.as_os_str())
                    .to_string_lossy(),
                path.display(),
                format_size(metadata.len())
            ));
        }
    }

    Ok(PropertiesSummaryDto {
        count: paths.len(),
        files,
        directories,
        total_size,
        total_size_label: format_size(total_size),
        lines,
    })
}

pub fn create_folder(parent: &Path, name: &str) -> Result<String> {
    let target = parent.join(name);
    fs::create_dir(&target)
        .with_context(|| format!("No se pudo crear la carpeta {}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

pub fn create_file(parent: &Path, name: &str) -> Result<String> {
    let file_name = if Path::new(name).extension().is_none() {
        format!("{name}.txt")
    } else {
        name.to_string()
    };
    let target = parent.join(file_name);
    fs::write(&target, b"")
        .with_context(|| format!("No se pudo crear el fichero {}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

pub fn rename_path(path: &Path, new_name: &str) -> Result<String> {
    let target = path.with_file_name(new_name);
    fs::rename(path, &target).with_context(|| {
        format!(
            "No se pudo renombrar {} a {}",
            path.display(),
            target.display()
        )
    })?;
    Ok(target.to_string_lossy().to_string())
}

pub fn delete_paths(app: &AppHandle, job_id: &str, paths: Vec<PathBuf>) -> Result<()> {
    let total = paths.len().max(1) as f64;
    for (index, path) in paths.iter().enumerate() {
        delete_path_with_log(app, job_id, path, 0)?;
        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Eliminando {}", path.display()),
        )?;
    }
    Ok(())
}

fn delete_path_with_log(app: &AppHandle, job_id: &str, path: &Path, depth: usize) -> Result<()> {
    let indent = "  ".repeat(depth);
    emit_log(app, job_id, format!("{indent}{}", path.display()))?;
    if path.is_dir() {
        let mut children = fs::read_dir(path)?
            .filter_map(|entry| entry.ok().map(|item| item.path()))
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            delete_path_with_log(app, job_id, &child, depth + 1)?;
        }
        fs::remove_dir(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn copy_or_move_paths(
    app: &AppHandle,
    job_id: &str,
    paths: Vec<PathBuf>,
    destination: PathBuf,
    operation: &str,
    overwrite: bool,
) -> Result<()> {
    let total = paths.len().max(1) as f64;
    for (index, source) in paths.iter().enumerate() {
        let target = destination.join(
            source
                .file_name()
                .ok_or_else(|| anyhow!("Ruta invalida: {}", source.display()))?,
        );
        if target.exists() {
            if !overwrite {
                return Err(anyhow!("El destino ya existe: {}", target.display()));
            }
            remove_single_path(&target)?;
        }
        emit_log(
            app,
            job_id,
            format!(
                "{} {} -> {}",
                if operation == "cut" { "Moviendo" } else { "Copiando" },
                source.display(),
                target.display()
            ),
        )?;

        if operation == "cut" {
            move_path(source, &target)?;
        } else {
            copy_path(source, &target)?;
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!(
                "{} {}",
                if operation == "cut" { "Moviendo" } else { "Copiando" },
                source.display()
            ),
        )?;
    }
    Ok(())
}

pub fn open_path(path: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()?;
        return Ok(());
    }

    Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

pub fn open_with_dialog(path: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        Command::new("rundll32.exe")
            .args(["shell32.dll,OpenAs_RunDLL", &path.to_string_lossy()])
            .spawn()?;
        return Ok(());
    }

    let status = Command::new("gio").arg("open").arg(path).status();
    if status.is_err() {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

pub fn build_extraction_preview(
    archives: &[PathBuf],
    options: &ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>> {
    let mut rows = Vec::new();
    for archive in archives {
        let destination_root = if options.destination_mode == "custom" {
            options
                .destination_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| archive.parent().unwrap_or(archive).to_path_buf())
        } else {
            archive.parent().unwrap_or(archive).to_path_buf()
        };
        let destination = if options.individual_folders {
            destination_root.join(
                archive
                    .file_stem()
                    .unwrap_or_else(|| archive.as_os_str())
                    .to_string_lossy()
                    .to_string(),
            )
        } else {
            destination_root
        };
        rows.push(ExtractionPreviewRow {
            archive_path: archive.to_string_lossy().to_string(),
            destination_path: destination.to_string_lossy().to_string(),
        });
    }
    Ok(rows)
}

pub fn extract_archives(
    app: &AppHandle,
    job_id: &str,
    archives: Vec<PathBuf>,
    options: ExtractionOptionsPayload,
) -> Result<()> {
    let preview = build_extraction_preview(&archives, &options)?;
    let seven_zip = seven_zip_path(app);
    let total = preview.len().max(1) as f64;

    for (index, row) in preview.iter().enumerate() {
        let archive = PathBuf::from(&row.archive_path);
        let destination = PathBuf::from(&row.destination_path);
        fs::create_dir_all(&destination)?;
        emit_log(
            app,
            job_id,
            format!("Descomprimiendo {} -> {}", archive.display(), destination.display()),
        )?;
        match archive
            .extension()
            .and_then(OsStr::to_str)
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default()
            .as_str()
        {
            "zip" => extract_zip(&archive, &destination, options.overwrite)?,
            "7z" | "rar" => {
                let tool = seven_zip
                    .clone()
                    .ok_or_else(|| anyhow!("No se encontro 7zz/7z integrado ni en PATH."))?;
                ensure_executable(&tool)?;
                let overwrite_flag = if options.overwrite { "-aoa" } else { "-aos" };
                let status = Command::new(tool)
                    .arg("x")
                    .arg(&archive)
                    .arg(format!("-o{}", destination.to_string_lossy()))
                    .arg(overwrite_flag)
                    .arg("-y")
                    .status()?;
                if !status.success() {
                    return Err(anyhow!(
                        "7zz devolvio un error al extraer {}",
                        archive.display()
                    ));
                }
            }
            _ => return Err(anyhow!("Formato no soportado: {}", archive.display())),
        }

        if options.delete_archives {
            fs::remove_file(&archive)?;
            emit_log(
                app,
                job_id,
                format!("Archivo comprimido eliminado: {}", archive.display()),
            )?;
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Descomprimiendo {}", archive.display()),
        )?;
    }
    Ok(())
}

pub fn scan_selection(paths: &[PathBuf]) -> Result<SelectionAnalysisDto> {
    let mut archives = Vec::new();
    let mut has_directories = false;
    let mut has_files = false;
    let mut chd_sources = Vec::new();
    let mut restorable_chds = Vec::new();
    let mut unique_extensions = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut seen_restorable = BTreeSet::new();

    for path in paths {
        if path.is_dir() {
            has_directories = true;
            for item in WalkDir::new(path).into_iter().filter_map(|entry| entry.ok()) {
                let candidate = item.path().to_path_buf();
                if is_chd_file(&candidate) {
                    let key = candidate.to_string_lossy().to_lowercase();
                    if seen_restorable.insert(key) {
                        restorable_chds.push(candidate.to_string_lossy().to_string());
                    }
                }
                if let Some(source) = detect_chd_source(&candidate)? {
                    if seen_sources.insert(source.source_path.clone()) {
                        for ext in &source.display_extensions {
                            unique_extensions.insert(ext.clone());
                        }
                        chd_sources.push(source);
                    }
                }
            }
            continue;
        }

        has_files = true;
        if is_archive(path) {
            archives.push(path.to_string_lossy().to_string());
        }
        if is_chd_file(path) {
            let key = path.to_string_lossy().to_lowercase();
            if seen_restorable.insert(key) {
                restorable_chds.push(path.to_string_lossy().to_string());
            }
        }
        if let Some(source) = detect_chd_source(path)? {
            if seen_sources.insert(source.source_path.clone()) {
                for ext in &source.display_extensions {
                    unique_extensions.insert(ext.clone());
                }
                chd_sources.push(source);
            }
        }
    }

    let unique_extensions = unique_extensions.into_iter().collect::<Vec<_>>();
    let chd_menu_label = if chd_sources.is_empty() {
        None
    } else if paths.iter().any(|path| path.is_dir()) {
        Some(format!(
            "Convertir carpeta/as {} a .chd",
            unique_extensions.join(" + ")
        ))
    } else if chd_sources.len() == 1 {
        Some(format!(
            "Convertir fichero {} a .chd",
            unique_extensions.join(" + ")
        ))
    } else {
        Some(format!(
            "Convertir {} ficheros {} a .chd",
            chd_sources.len(),
            unique_extensions.join(" + ")
        ))
    };
    let chd_restore_menu_label = if restorable_chds.is_empty() {
        None
    } else if paths.iter().any(|path| path.is_dir()) {
        Some("Recuperar contenido desde carpeta/as .chd".to_string())
    } else if restorable_chds.len() == 1 {
        Some("Recuperar contenido desde fichero .chd".to_string())
    } else {
        Some(format!(
            "Recuperar contenido desde {} ficheros .chd",
            restorable_chds.len()
        ))
    };

    Ok(SelectionAnalysisDto {
        archives,
        chd_sources,
        restorable_chds,
        has_directories,
        has_files,
        unique_extensions,
        chd_menu_label,
        chd_restore_menu_label,
    })
}

pub fn convert_to_chd(
    app: &AppHandle,
    job_id: &str,
    paths: &[PathBuf],
    options: ChdConversionOptionsPayload,
) -> Result<()> {
    let chdman = chdman_path(app).ok_or_else(|| anyhow!("No se encontro chdman integrado."))?;
    ensure_executable(&chdman)?;
    let analysis = scan_selection(paths)?;
    let selected_root = paths.iter().find(|path| path.is_dir()).cloned();
    let total = analysis.chd_sources.len().max(1) as f64;
    let mut touched_directories = BTreeSet::new();

    for (index, source) in analysis.chd_sources.iter().enumerate() {
        let source_path = PathBuf::from(&source.source_path);
        let container_dir = PathBuf::from(&source.container_dir);
        let output_path = build_chd_output_path(
            &source_path,
            &container_dir,
            selected_root.as_deref(),
            &options,
        );
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut command = Command::new(&chdman);
        command
            .arg(&source.command)
            .arg("-i")
            .arg(&source_path)
            .arg("-o")
            .arg(&output_path);
        if options.overwrite {
            command.arg("-f");
        }

        emit_log(
            app,
            job_id,
            format!(
                "Ejecutando: {} {} -i {} -o {}{}",
                chdman.display(),
                source.command,
                source_path.display(),
                output_path.display(),
                if options.overwrite { " -f" } else { "" }
            ),
        )?;

        let output = command.output()?;
        if !output.stdout.is_empty() {
            emit_log(
                app,
                job_id,
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            )?;
        }
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!(if error.is_empty() {
                "chdman devolvio un error".to_string()
            } else {
                error
            }));
        }

        if options.delete_originals {
            for original in &source.required_paths {
                let original_path = PathBuf::from(original);
                if original_path.exists() {
                    remove_single_path(&original_path)?;
                    emit_log(
                        app,
                        job_id,
                        format!("Original eliminado: {}", original_path.display()),
                    )?;
                }
            }
        }

        if options.deposit_to_parent {
            touched_directories.insert(container_dir);
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Convirtiendo {}", source_path.display()),
        )?;
    }

    if options.deposit_to_parent && options.delete_original_subfolders {
        if let Some(root) = selected_root {
            let mut directories = touched_directories.into_iter().collect::<Vec<_>>();
            directories.sort_by_key(|directory| std::cmp::Reverse(directory.components().count()));
            for directory in directories {
                if directory == root {
                    continue;
                }
                if directory.exists() && fs::read_dir(&directory)?.next().is_none() {
                    fs::remove_dir(&directory)?;
                    emit_log(
                        app,
                        job_id,
                        format!("Subcarpeta eliminada: {}", directory.display()),
                    )?;
                }
            }
        }
    }

    Ok(())
}

pub fn load_chdman_metadata(app: &AppHandle) -> serde_json::Value {
    let metadata_path = runtime_root(app).join("third_party").join("chdman").join("metadata.json");
    fs::read_to_string(metadata_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

pub fn restore_from_chd(
    app: &AppHandle,
    job_id: &str,
    paths: &[PathBuf],
    options: ChdRestoreOptionsPayload,
) -> Result<()> {
    let chdman = chdman_path(app).ok_or_else(|| anyhow!("No se encontro chdman integrado."))?;
    ensure_executable(&chdman)?;
    let analysis = scan_selection(paths)?;
    let total = analysis.restorable_chds.len().max(1) as f64;

    for (index, source) in analysis.restorable_chds.iter().enumerate() {
        let chd_path = PathBuf::from(source);
        let destination_root = if options.destination_mode == "custom" {
            options
                .destination_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| chd_path.parent().unwrap_or(&chd_path).to_path_buf())
        } else {
            chd_path.parent().unwrap_or(&chd_path).to_path_buf()
        };
        let destination = if options.individual_folders {
            destination_root.join(
                chd_path
                    .file_stem()
                    .unwrap_or_else(|| chd_path.as_os_str())
                    .to_string_lossy()
                    .to_string(),
            )
        } else {
            destination_root
        };
        fs::create_dir_all(&destination)?;

        emit_log(
            app,
            job_id,
            format!("Recuperando contenido desde {} -> {}", chd_path.display(), destination.display()),
        )?;

        let stem = chd_path
            .file_stem()
            .unwrap_or_else(|| chd_path.as_os_str())
            .to_string_lossy()
            .to_string();
        let cue_path = destination.join(format!("{stem}.cue"));
        let bin_path = destination.join(format!("{stem}.bin"));
        let iso_path = destination.join(format!("{stem}.iso"));

        let cd_result = run_extractcd(
            &chdman,
            &chd_path,
            &cue_path,
            &bin_path,
            options.overwrite,
            options.split_bin,
        );
        match cd_result {
            Ok(_) => {
                emit_log(
                    app,
                    job_id,
                    format!("Extraido como CD: {}", cue_path.display()),
                )?;
            }
            Err(error) => {
                let first_error = error.to_string();
                cleanup_restore_outputs(&cue_path, &bin_path, &destination, &stem, &chd_path)?;
                run_extractdvd(&chdman, &chd_path, &iso_path, options.overwrite).map_err(|dvd_error| {
                    anyhow!(
                        "No se pudo recuperar {} como CD ni como DVD.\nCD: {}\nDVD: {}",
                        chd_path.display(),
                        first_error,
                        dvd_error
                    )
                })?;
                emit_log(
                    app,
                    job_id,
                    format!("Extraido como DVD: {}", iso_path.display()),
                )?;
            }
        }

        if options.delete_chd {
            fs::remove_file(&chd_path)?;
            emit_log(
                app,
                job_id,
                format!("CHD eliminado tras recuperar contenido: {}", chd_path.display()),
            )?;
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Recuperando {}", chd_path.display()),
        )?;
    }

    Ok(())
}

fn extract_zip(archive_path: &Path, destination: &Path, overwrite: bool) -> Result<()> {
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index)?;
        let out_path = destination.join(item.mangled_name());
        if item.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if out_path.exists() && !overwrite {
            return Err(anyhow!(
                "Ya existe un fichero y sobrescritura no esta permitida: {}",
                out_path.display()
            ));
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&out_path)?;
        std::io::copy(&mut item, &mut output)?;
    }
    Ok(())
}

fn ensure_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path)?;
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0 {
            permissions.set_mode(mode | 0o755);
            fs::set_permissions(path, permissions)?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn ensure_summary_not_cancelled(cancel_state: Option<(&AtomicU64, u64)>) -> Result<()> {
    if let Some((state, request_id)) = cancel_state {
        if state.load(Ordering::Relaxed) != request_id {
            return Err(anyhow!("summary-cancelled"));
        }
    }
    Ok(())
}

fn compute_directory_stats(
    path: &Path,
    max_depth: Option<usize>,
    cancel_state: Option<(&AtomicU64, u64)>,
) -> Result<(u64, usize, usize)> {
    let mut total_size = 0_u64;
    let mut file_count = 0_usize;
    let mut dir_count = 0_usize;
    let mut walker = WalkDir::new(path);
    if let Some(limit) = max_depth {
        walker = walker.max_depth(limit + 1);
    }
    for (index, entry) in walker.into_iter().filter_map(|entry| entry.ok()).enumerate() {
        if index % 128 == 0 {
            ensure_summary_not_cancelled(cancel_state)?;
        }
        if entry.path() == path {
            continue;
        }
        if entry.file_type().is_dir() {
            dir_count += 1;
        } else {
            file_count += 1;
            total_size += entry.metadata()?.len();
        }
    }
    Ok((total_size, file_count, dir_count))
}

fn copy_path(source: &Path, target: &Path) -> Result<()> {
    if source.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_source = entry.path();
            let child_target = target.join(entry.file_name());
            copy_path(&child_source, &child_target)?;
        }
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)?;
    Ok(())
}

fn move_path(source: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(source, target) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_path(source, target)?;
            remove_single_path(source)
        }
    }
}

fn remove_single_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| format!(".{}", ext.to_ascii_lowercase()))
        .map(|ext| ARCHIVE_EXTENSIONS.contains(&ext.as_str()))
        .unwrap_or(false)
}

fn is_chd_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.eq_ignore_ascii_case("chd"))
            .unwrap_or(false)
}

fn detect_chd_source(path: &Path) -> Result<Option<ChdSourceDto>> {
    if !path.is_file() {
        return Ok(None);
    }
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| format!(".{}", ext.to_ascii_lowercase()))
        .unwrap_or_default();

    if CD_SOURCE_EXTENSIONS.contains(&extension.as_str()) {
        let required_paths = if extension == ".cue" {
            parse_cue_references(path)?
        } else if extension == ".gdi" {
            parse_gdi_references(path)?
        } else {
            vec![path.to_path_buf()]
        };
        return Ok(Some(ChdSourceDto {
            source_path: path.to_string_lossy().to_string(),
            container_dir: path
                .parent()
                .unwrap_or(path)
                .to_string_lossy()
                .to_string(),
            command: "createcd".to_string(),
            display_extensions: vec![extension],
            required_paths: required_paths
                .into_iter()
                .map(|item| item.to_string_lossy().to_string())
                .collect(),
        }));
    }

    if DVD_SOURCE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(Some(ChdSourceDto {
            source_path: path.to_string_lossy().to_string(),
            container_dir: path
                .parent()
                .unwrap_or(path)
                .to_string_lossy()
                .to_string(),
            command: "createdvd".to_string(),
            display_extensions: vec![extension],
            required_paths: vec![path.to_string_lossy().to_string()],
        }));
    }

    if PAIRABLE_BIN_EXTENSIONS.contains(&extension.as_str()) {
        let cue_candidate = path.with_extension("cue");
        if cue_candidate.exists() {
            let required_paths = parse_cue_references(&cue_candidate)?;
            return Ok(Some(ChdSourceDto {
                source_path: cue_candidate.to_string_lossy().to_string(),
                container_dir: cue_candidate
                    .parent()
                    .unwrap_or(&cue_candidate)
                    .to_string_lossy()
                    .to_string(),
                command: "createcd".to_string(),
                display_extensions: vec![".bin".to_string(), ".cue".to_string()],
                required_paths: required_paths
                    .into_iter()
                    .map(|item| item.to_string_lossy().to_string())
                    .collect(),
            }));
        }
        if let Some(referenced_cue) = find_cue_for_bin(path)? {
            let required_paths = parse_cue_references(&referenced_cue)?;
            return Ok(Some(ChdSourceDto {
                source_path: referenced_cue.to_string_lossy().to_string(),
                container_dir: referenced_cue
                    .parent()
                    .unwrap_or(&referenced_cue)
                    .to_string_lossy()
                    .to_string(),
                command: "createcd".to_string(),
                display_extensions: vec![".bin".to_string(), ".cue".to_string()],
                required_paths: required_paths
                    .into_iter()
                    .map(|item| item.to_string_lossy().to_string())
                    .collect(),
            }));
        }
    }

    Ok(None)
}

fn parse_cue_references(path: &Path) -> Result<Vec<PathBuf>> {
    let mut references = vec![path.to_path_buf()];
    let content = fs::read_to_string(path).unwrap_or_default();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_uppercase().starts_with("FILE ") {
            continue;
        }
        if let Some(start) = trimmed.find('"') {
            if let Some(end) = trimmed[start + 1..].find('"') {
                let filename = &trimmed[start + 1..start + 1 + end];
                references.push(path.parent().unwrap_or(path).join(filename));
            }
        }
    }
    Ok(dedupe_paths(references))
}

fn parse_gdi_references(path: &Path) -> Result<Vec<PathBuf>> {
    let mut references = vec![path.to_path_buf()];
    let content = fs::read_to_string(path).unwrap_or_default();
    for line in content.lines().skip(1) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() >= 5 {
            references.push(path.parent().unwrap_or(path).join(parts[4]));
        }
    }
    Ok(dedupe_paths(references))
}

fn find_cue_for_bin(bin_path: &Path) -> Result<Option<PathBuf>> {
    let parent = match bin_path.parent() {
        Some(parent) => parent,
        None => return Ok(None),
    };
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.eq_ignore_ascii_case("cue"))
            .unwrap_or(false)
        {
            let references = parse_cue_references(&path)?;
            if references.iter().any(|item| same_path(item, bin_path)) {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

fn build_chd_output_path(
    source_path: &Path,
    container_dir: &Path,
    selected_root: Option<&Path>,
    options: &ChdConversionOptionsPayload,
) -> PathBuf {
    let mut stem = source_path
        .file_stem()
        .unwrap_or_else(|| source_path.as_os_str())
        .to_string_lossy()
        .to_string();
    if options.name_as_container {
        stem = container_dir
            .file_name()
            .unwrap_or_else(|| container_dir.as_os_str())
            .to_string_lossy()
            .to_string();
    }

    let mut output_dir = source_path.parent().unwrap_or(source_path).to_path_buf();
    if options.deposit_to_parent {
        if let Some(root) = selected_root {
            if source_path.starts_with(root) && source_path.parent().unwrap_or(source_path) != root {
                output_dir = root.to_path_buf();
            }
        }
    }
    output_dir.join(format!("{stem}.chd"))
}

fn run_extractcd(
    chdman: &Path,
    input: &Path,
    cue_path: &Path,
    bin_path: &Path,
    overwrite: bool,
    split_bin: bool,
) -> Result<()> {
    let mut command = Command::new(chdman);
    command.arg("extractcd").arg("-i").arg(input).arg("-o").arg(cue_path);
    if !split_bin {
        command.arg("-ob").arg(bin_path);
    } else {
        command.arg("-sb");
    }
    if overwrite {
        command.arg("-f");
    }
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(anyhow!(if stderr.is_empty() {
        if stdout.is_empty() {
            "extractcd devolvio un error".to_string()
        } else {
            stdout
        }
    } else {
        stderr
    }))
}

fn run_extractdvd(chdman: &Path, input: &Path, iso_path: &Path, overwrite: bool) -> Result<()> {
    let mut command = Command::new(chdman);
    command
        .arg("extractdvd")
        .arg("-i")
        .arg(input)
        .arg("-o")
        .arg(iso_path);
    if overwrite {
        command.arg("-f");
    }
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(anyhow!(if stderr.is_empty() {
        if stdout.is_empty() {
            "extractdvd devolvio un error".to_string()
        } else {
            stdout
        }
    } else {
        stderr
    }))
}

fn cleanup_restore_outputs(
    cue_path: &Path,
    bin_path: &Path,
    destination: &Path,
    stem: &str,
    source_chd: &Path,
) -> Result<()> {
    if cue_path.exists() {
        let _ = fs::remove_file(cue_path);
    }
    if bin_path.exists() {
        let _ = fs::remove_file(bin_path);
    }
    if destination.exists() {
        for entry in fs::read_dir(destination)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_else(|| path.as_os_str())
                .to_string_lossy()
                .to_string();
            if same_path(&path, source_chd) {
                continue;
            }
            if name.to_lowercase().starts_with(&stem.to_lowercase()) && path.is_file() {
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(())
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
}

pub fn emit_progress(app: &AppHandle, job_id: &str, progress: f64, message: String) -> Result<()> {
    app.emit(
        "job-progress",
        JobProgressDto {
            job_id: job_id.to_string(),
            progress,
            message,
        },
    )?;
    Ok(())
}

pub fn emit_log(app: &AppHandle, job_id: &str, line: String) -> Result<()> {
    app.emit(
        "job-log",
        JobLogDto {
            job_id: job_id.to_string(),
            line,
        },
    )?;
    Ok(())
}

pub fn emit_finished(app: &AppHandle, job_id: &str, success: bool, message: String) -> Result<()> {
    app.emit(
        "job-finished",
        JobFinishedDto {
            job_id: job_id.to_string(),
            success,
            message,
        },
    )?;
    Ok(())
}

pub fn format_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    for unit in &units {
        if value < 1024.0 || *unit == "TB" {
            if *unit == "B" {
                return format!("{} {}", value as u64, unit);
            }
            return format!("{value:.2} {unit}");
        }
        value /= 1024.0;
    }
    format!("{size} B")
}

fn format_timestamp(timestamp: SystemTime) -> String {
    let date_time: DateTime<Local> = timestamp.into();
    date_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

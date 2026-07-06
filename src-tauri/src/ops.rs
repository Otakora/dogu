use std::{
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io::{self, Write, Seek},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use tauri::{AppHandle, Emitter, Manager};
use walkdir::WalkDir;
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// RAII wrapper that removes a directory tree when dropped.
/// Guarantees cleanup of temporary staging/download dirs even when the
/// enclosing function returns early via `?` or panics.
struct TempGuard(PathBuf);

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

use crate::{
    models::{
        ChdConversionOptionsPayload, ChdRestoreOptionsPayload, ChdSourceDto,
        CompressionCapabilitiesDto, CompressionOptionsPayload,
        EntryDto,
        ExtractionOptionsPayload, ExtractionPreviewEntry, ExtractionPreviewRow, JobFinishedDto,
        JobLogDto, JobProgressDto, KnownFoldersDto, PreflightCheckResult, PreflightWarning,
        RemoteTransferPolicy, PropertiesSummaryDto, SelectionAnalysisDto, SummaryOptionsPayload, VolumeDto,
    },
    pause,
    remote,
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

/// Returns `desired` if it doesn't exist, otherwise the first non-existing
/// "stem (N)" variant (N starting at 2), preserving any extension. Works for
/// both files and directories. Used to standardize conflict handling: when an
/// operation isn't overwriting, the output is renamed instead of failing.
pub fn unique_path(desired: &Path) -> PathBuf {
    if !desired.exists() {
        return desired.to_path_buf();
    }
    let parent = desired.parent().map(Path::to_path_buf).unwrap_or_default();
    let file_name = desired
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // Split "name.ext" into ("name", Some("ext")); directories/extension-less → (name, None).
    let (stem, ext) = match desired.extension().and_then(OsStr::to_str) {
        Some(e) if !e.is_empty() => {
            let s = &file_name[..file_name.len() - e.len() - 1];
            (s.to_string(), Some(e.to_string()))
        }
        _ => (file_name.clone(), None),
    };
    let mut n: u32 = 2;
    loop {
        let candidate_name = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = parent.join(&candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Finds a stem (with " (N)" suffix if needed) for which none of the CHD-restore
/// outputs (`stem.cue`, `stem.bin`, `stem.iso`) already exist in `dir`. Keeps
/// the .cue/.bin pair named consistently so the cue keeps referencing its bin.
fn unique_restore_stem(dir: &Path, stem: &str) -> String {
    let collides = |s: &str| {
        dir.join(format!("{s}.cue")).exists()
            || dir.join(format!("{s}.bin")).exists()
            || dir.join(format!("{s}.iso")).exists()
    };
    if !collides(stem) {
        return stem.to_string();
    }
    let mut n: u32 = 2;
    loop {
        let candidate = format!("{stem} ({n})");
        if !collides(&candidate) {
            return candidate;
        }
        n += 1;
    }
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
    rename_on_conflict: bool,
) -> Result<()> {
    let total = paths.len().max(1) as f64;
    for (index, source) in paths.iter().enumerate() {
        let mut target = destination.join(
            source
                .file_name()
                .ok_or_else(|| anyhow!("Ruta invalida: {}", source.display()))?,
        );
        if target.exists() {
            if overwrite {
                remove_single_path(&target)?;
            } else if rename_on_conflict {
                target = unique_path(&target);
            } else {
                return Err(anyhow!("El destino ya existe: {}", target.display()));
            }
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
        // OpenWith.exe is the native "Open With" dialog in Windows 8+.
        // rundll32 shell32.dll,OpenAs_RunDLL is deprecated and silently fails on Win10/11.
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let open_with = PathBuf::from(system_root).join("System32").join("OpenWith.exe");
        if open_with.exists() {
            Command::new(&open_with).arg(path).spawn()?;
        } else {
            Command::new("rundll32.exe")
                .args(["shell32.dll,OpenAs_RunDLL", &path.to_string_lossy()])
                .spawn()?;
        }
        return Ok(());
    }

    let status = Command::new("gio").arg("open").arg(path).status();
    if status.is_err() {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

fn extraction_destination_root(archive: &Path, options: &ExtractionOptionsPayload) -> PathBuf {
    let destination_root = if options.destination_mode == "custom" {
        options
            .destination_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| archive.parent().unwrap_or(archive).to_path_buf())
    } else {
        archive.parent().unwrap_or(archive).to_path_buf()
    };
    if options.individual_folders {
        destination_root.join(
            archive
                .file_stem()
                .unwrap_or_else(|| archive.as_os_str())
                .to_string_lossy()
                .to_string(),
        )
    } else {
        destination_root
    }
}

fn entry_stem_or_name(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| name.to_string())
}

/// Lists the top-level entries (files/folders) contained directly inside an archive,
/// without extracting it. Used both to build the extraction preview and to know which
/// entries to split into their own folders when `split_entries` is enabled.
fn list_top_level_entries(app: &AppHandle, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    match archive_path
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "zip" => list_top_level_zip_entries(archive_path),
        "7z" | "rar" => {
            let tool = seven_zip_path(app)
                .ok_or_else(|| anyhow!("No se encontro 7zz/7z integrado ni en PATH."))?;
            ensure_executable(&tool)?;
            list_top_level_7z_entries(&tool, archive_path)
        }
        _ => Err(anyhow!("Formato no soportado: {}", archive_path.display())),
    }
}

fn list_top_level_zip_entries(archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let item = archive.by_index(index)?;
        let mangled = item.mangled_name();
        let mut components = mangled.components();
        if let Some(first) = components.next() {
            let name = first.as_os_str().to_string_lossy().to_string();
            if name.is_empty() {
                continue;
            }
            let is_dir = components.next().is_some() || item.name().ends_with('/');
            seen.entry(name).and_modify(|d| *d = *d || is_dir).or_insert(is_dir);
        }
    }
    Ok(seen.into_iter().collect())
}

fn list_top_level_7z_entries(tool: &Path, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let output = Command::new(tool).arg("l").arg("-slt").arg(archive_path).output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "No se pudo listar el contenido de {}",
            archive_path.display()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    let mut current_path: Option<String> = None;
    let mut current_is_dir = false;

    let flush = |path: Option<String>, is_dir: bool, seen: &mut std::collections::BTreeMap<String, bool>| {
        if let Some(path) = path {
            let normalized = path.replace('\\', "/");
            let mut parts = normalized.split('/').filter(|p| !p.is_empty());
            if let Some(first) = parts.next() {
                let has_more = parts.next().is_some();
                let entry_is_dir = has_more || is_dir;
                seen
                    .entry(first.to_string())
                    .and_modify(|d| *d = *d || entry_is_dir)
                    .or_insert(entry_is_dir);
            }
        }
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Path = ") {
            flush(current_path.take(), current_is_dir, &mut seen);
            current_path = Some(rest.trim().to_string());
            current_is_dir = false;
        } else if let Some(rest) = line.strip_prefix("Folder = ") {
            current_is_dir = rest.trim() == "+";
        }
    }
    flush(current_path.take(), current_is_dir, &mut seen);

    Ok(seen.into_iter().collect())
}

/// Inserts `rel` and every ancestor directory into `seen`. Ancestors are always
/// directories; the leaf keeps its own kind. Paths are normalised to '/'.
fn insert_with_ancestors(
    seen: &mut std::collections::BTreeMap<String, bool>,
    rel: &str,
    is_dir: bool,
) {
    let norm = rel.replace('\\', "/");
    let trimmed = norm.trim_matches('/');
    if trimmed.is_empty() {
        return;
    }
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    for i in 0..parts.len() {
        let is_last = i + 1 == parts.len();
        let path = parts[..=i].join("/");
        let entry_is_dir = if is_last { is_dir } else { true };
        seen.entry(path)
            .and_modify(|d| *d = *d || entry_is_dir)
            .or_insert(entry_is_dir);
    }
}

/// Lists every entry (files and directories, full relative paths) inside an
/// archive, synthesising intermediate directories that aren't listed explicitly.
/// Used to build the deep extraction preview that powers nested ghost trees.
fn list_all_entries(app: &AppHandle, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    match archive_path
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "zip" => list_all_zip_entries(archive_path),
        "7z" | "rar" => {
            let tool = seven_zip_path(app)
                .ok_or_else(|| anyhow!("No se encontro 7zz/7z integrado ni en PATH."))?;
            ensure_executable(&tool)?;
            list_all_7z_entries(&tool, archive_path)
        }
        _ => Err(anyhow!("Formato no soportado: {}", archive_path.display())),
    }
}

fn list_all_zip_entries(archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let item = archive.by_index(index)?;
        let is_dir = item.name().ends_with('/');
        let rel = item.mangled_name().to_string_lossy().to_string();
        insert_with_ancestors(&mut seen, &rel, is_dir);
    }
    Ok(seen.into_iter().collect())
}

fn list_all_7z_entries(tool: &Path, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let output = Command::new(tool).arg("l").arg("-slt").arg(archive_path).output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "No se pudo listar el contenido de {}",
            archive_path.display()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    let mut current_path: Option<String> = None;
    let mut current_is_dir = false;
    // The entry listing begins after the "----------" separator; everything
    // before it is the archive's own header (including a "Path = <archive>"
    // line that must not be treated as an entry).
    let mut in_entries = false;

    for line in text.lines() {
        if !in_entries {
            if line.trim_start().starts_with("----------") {
                in_entries = true;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Path = ") {
            if let Some(p) = current_path.take() {
                insert_with_ancestors(&mut seen, &p, current_is_dir);
            }
            current_path = Some(rest.trim().to_string());
            current_is_dir = false;
        } else if let Some(rest) = line.strip_prefix("Folder = ") {
            if rest.trim() == "+" {
                current_is_dir = true;
            }
        } else if let Some(rest) = line.strip_prefix("Attributes = ") {
            // Directories carry the 'D' attribute flag (files never do).
            if rest.contains('D') {
                current_is_dir = true;
            }
        }
    }
    if let Some(p) = current_path.take() {
        insert_with_ancestors(&mut seen, &p, current_is_dir);
    }

    Ok(seen.into_iter().collect())
}

/// Builds a deep extraction preview: one entry per file/folder at every level of
/// the archive, with the destination path each will occupy. Powers navigable
/// nested ghost trees. `splitEntries` is intentionally not applied here.
pub fn build_extraction_preview_deep(
    app: &AppHandle,
    archives: &[PathBuf],
    options: &ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>> {
    let mut rows = Vec::new();
    for archive in archives {
        let destination_root = extraction_destination_root(archive, options);
        let raw = list_all_entries(app, archive)?;

        // Apply `splitEntries` (each top-level file is moved into a folder named
        // after its stem) and re-synthesise ancestor directories so every level
        // — including any folders split introduces — is represented.
        let mut final_map: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        for (rel, is_dir) in &raw {
            let final_rel = if options.split_entries && !is_dir && !rel.contains('/') {
                format!("{}/{}", entry_stem_or_name(rel), rel)
            } else {
                rel.clone()
            };
            insert_with_ancestors(&mut final_map, &final_rel, *is_dir);
        }

        let mut preview_entries: Vec<ExtractionPreviewEntry> = Vec::new();

        // "Extract to folder" wraps the contents in a new folder (destination_root
        // itself). Emit that wrapper so it shows as a ghost in the parent folder
        // and can be navigated into.
        if options.individual_folders {
            preview_entries.push(ExtractionPreviewEntry {
                name: destination_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                is_dir: true,
                destination_path: destination_root.to_string_lossy().to_string(),
            });
        }

        for (rel, is_dir) in &final_map {
            let dest = rel
                .split('/')
                .filter(|s| !s.is_empty())
                .fold(destination_root.clone(), |acc, seg| acc.join(seg));
            preview_entries.push(ExtractionPreviewEntry {
                name: rel.rsplit('/').next().unwrap_or(rel).to_string(),
                is_dir: *is_dir,
                destination_path: dest.to_string_lossy().to_string(),
            });
        }

        rows.push(ExtractionPreviewRow {
            archive_path: archive.to_string_lossy().to_string(),
            destination_root: destination_root.to_string_lossy().to_string(),
            entries: preview_entries,
        });
    }
    Ok(rows)
}

/// Moves each top-level file entry produced by an extraction into a folder named after
/// the file (stripping the extension), leaving top-level directories untouched since they
/// already act as their own folder.
fn split_top_level_entries(destination: &Path, entries: &[(String, bool)]) -> Result<()> {
    for (name, is_dir) in entries {
        if *is_dir {
            continue;
        }
        let entry_path = destination.join(name);
        if !entry_path.exists() {
            continue;
        }

        let stem = entry_stem_or_name(name);
        let mut folder_name = stem.clone();
        let mut counter = 2;
        loop {
            let candidate = destination.join(&folder_name);
            if !candidate.exists() || candidate == entry_path {
                break;
            }
            folder_name = format!("{stem} ({counter})");
            counter += 1;
        }
        let folder_path = destination.join(&folder_name);

        if folder_path == entry_path {
            // The folder name would collide with the file being moved (no extension).
            let staging = destination.join(format!("__dogu_split_{name}"));
            fs::rename(&entry_path, &staging)?;
            fs::create_dir_all(&folder_path)?;
            fs::rename(&staging, folder_path.join(name))?;
        } else {
            fs::create_dir_all(&folder_path)?;
            fs::rename(&entry_path, folder_path.join(name))?;
        }
    }
    Ok(())
}

pub fn build_extraction_preview(
    app: &AppHandle,
    archives: &[PathBuf],
    options: &ExtractionOptionsPayload,
) -> Result<Vec<ExtractionPreviewRow>> {
    let mut rows = Vec::new();
    for archive in archives {
        let destination_root = extraction_destination_root(archive, options);
        let entries = list_top_level_entries(app, archive)?;
        let preview_entries = entries
            .iter()
            .map(|(name, is_dir)| {
                let destination_path = if options.split_entries && !is_dir {
                    destination_root.join(entry_stem_or_name(name)).join(name)
                } else {
                    destination_root.join(name)
                };
                ExtractionPreviewEntry {
                    name: name.clone(),
                    is_dir: *is_dir,
                    destination_path: destination_path.to_string_lossy().to_string(),
                }
            })
            .collect();
        rows.push(ExtractionPreviewRow {
            archive_path: archive.to_string_lossy().to_string(),
            destination_root: destination_root.to_string_lossy().to_string(),
            entries: preview_entries,
        });
    }
    Ok(rows)
}

/// Returns the virtual remote parent of a `remote://session/path/to/file` path.
/// E.g. `remote://s/games/disc.cue` → `remote://s/games`
fn remote_parent_of(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    let rest = s.strip_prefix("remote://")?;
    let slash = rest.find('/')?;
    let session_id = &rest[..slash];
    let logical = &rest[slash..];
    let trimmed = logical.trim_end_matches('/');
    let parent_slash = trimmed.rfind('/')?;
    let parent = if parent_slash == 0 { "/" } else { &trimmed[..parent_slash] };
    Some(format!("remote://{session_id}{parent}"))
}

/// Deletes all orphaned temp directories left by a previous (crashed) session.
/// Safe to call at startup because no operations are running yet.
pub fn sweep_temp_dirs(app: &AppHandle) {
    // Wipe everything inside the two app-managed temp subtrees.
    if let Ok(app_data) = app.path().app_data_dir() {
        for subdir in ["remote-download", "temp"] {
            let dir = app_data.join(subdir);
            if dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    }
    // Wipe terminal init files/dirs left in the system temp dir.
    if let Ok(entries) = fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("dogu-init-") || name_str.starts_with("dogu-zsh-") {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Derives a temp download dir for an operation.
fn temp_download_dir(app: &AppHandle, job_id: &str) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow!("{e}"))?
        .join("remote-download")
        .join(job_id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn extract_archives(
    app: &AppHandle,
    job_id: &str,
    archives: Vec<PathBuf>,
    options: ExtractionOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let seven_zip = seven_zip_path(app);
    let total = archives.len().max(1) as f64;

    for (index, archive) in archives.iter().enumerate() {
        let prog_base = index as f64 / total;
        let prog_span = 1.0 / total;
        let source_name = archive.file_name().unwrap_or(archive.as_os_str()).to_string_lossy().to_string();
        let archive_is_remote = remote::RemoteManager::is_remote_path(&archive.to_string_lossy());

        // Download remote archive to a temp dir so tools can open it as a local file.
        // TempGuard ensures the dir is removed even if this iteration returns early via `?`.
        let (effective_archive, _guard) = if archive_is_remote {
            let rm = remote_manager.as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el archivo"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            emit_log(app, job_id, format!("Descargando {}...", source_name))?;
            let (local, bytes) = rm.download_file_to_dir(&archive.to_string_lossy(), &dl_dir)
                .map_err(|e| anyhow!("Fallo al descargar '{}': {}", source_name, e))?;
            emit_log(app, job_id, format!("{} descargado ({} bytes)", source_name, bytes))?;
            (local, Some(TempGuard(dl_dir)))
        } else {
            (archive.clone(), None)
        };

        // When source is remote and destination is "same", send output back to the remote parent.
        let effective_remote_dest: Option<String> = if options.remote_destination.is_some() {
            options.remote_destination.clone()
        } else if archive_is_remote && options.destination_mode != "custom" {
            remote_parent_of(archive)
        } else {
            None
        };

        let use_remote = remote_manager.is_some()
            && pause_registry.is_some()
            && effective_remote_dest.is_some();

        if use_remote {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            emit_log(app, job_id, format!("Extrayendo {}...", source_name))?;
            let policy = options.remote_transfer.clone()
                .unwrap_or_else(|| RemoteTransferPolicy { on_error: "abort".to_string() });
            let options_c = options.clone();
            let seven_zip_c = seven_zip.clone();
            let effective_archive_c = effective_archive.clone();
            run_remote_file_cycle(
                app, job_id, &rm, pr, remote_dest, &policy, &source_name,
                |temp_dir| {
                    let extract_target = if options_c.individual_folders {
                        let stem = effective_archive_c
                            .file_stem()
                            .unwrap_or(effective_archive_c.as_os_str())
                            .to_string_lossy()
                            .to_string();
                        let sub = temp_dir.join(&stem);
                        fs::create_dir_all(&sub)?;
                        sub
                    } else {
                        temp_dir.to_path_buf()
                    };
                    do_extract_archive(app, job_id, prog_base, prog_span, &effective_archive_c, &extract_target, &options_c, &seven_zip_c)?;
                    collect_dir_entries(temp_dir)
                },
            )?;
        } else {
            let destination = extraction_destination_root(&effective_archive, &options);
            fs::create_dir_all(&destination)?;
            emit_log(
                app,
                job_id,
                format!("Descomprimiendo {} -> {}", effective_archive.display(), destination.display()),
            )?;
            do_extract_archive(app, job_id, prog_base, prog_span, &effective_archive, &destination, &options, &seven_zip)?;
        }

        if options.delete_archives {
            if archive_is_remote {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, &archive.to_string_lossy())?;
                    emit_log(app, job_id, format!("Archivo remoto eliminado: {}", archive.display()))?;
                }
            } else {
                fs::remove_file(archive)?;
                emit_log(app, job_id, format!("Archivo comprimido eliminado: {}", archive.display()))?;
            }
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

fn do_extract_archive(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    archive: &Path,
    destination: &Path,
    options: &ExtractionOptionsPayload,
    seven_zip: &Option<PathBuf>,
) -> Result<()> {
    match archive
        .extension()
        .and_then(OsStr::to_str)
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "zip" => extract_zip(app, job_id, base, span, archive, destination, options.overwrite, options.rename_on_conflict)?,
        "7z" | "rar" => {
            let tool = seven_zip
                .clone()
                .ok_or_else(|| anyhow!("No se encontro 7zz/7z integrado ni en PATH."))?;
            ensure_executable(&tool)?;
            // -aoa overwrite, -aou auto-rename the extracted file, -aos skip existing.
            let overwrite_flag = if options.overwrite {
                "-aoa"
            } else if options.rename_on_conflict {
                "-aou"
            } else {
                "-aos"
            };
            let mut cmd = Command::new(&tool);
            cmd.arg("x")
                .arg(archive)
                .arg(format!("-o{}", destination.to_string_lossy()))
                .arg(overwrite_flag)
                .arg("-y")
                .arg("-bsp1"); // stream progress percentage to stdout
            run_command_streaming(app, job_id, base, span, cmd)
                .map_err(|e| anyhow!("7-zip fallo al extraer '{}': {}", archive.display(), e))?;
        }
        _ => return Err(anyhow!("Formato no soportado: {}", archive.display())),
    }
    if options.split_entries {
        let entries = list_top_level_entries(app, archive)?;
        split_top_level_entries(destination, &entries)?;
        emit_log(
            app,
            job_id,
            format!("Elementos de {} separados en carpetas individuales", archive.display()),
        )?;
    }
    Ok(())
}

fn collect_dir_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        result.push(entry?.path());
    }
    Ok(result)
}

pub fn scan_selection(
    paths: &[PathBuf],
    max_depth: usize,
    remote_manager: Option<Arc<remote::RemoteManager>>,
) -> Result<SelectionAnalysisDto> {
    let mut archives = Vec::new();
    let mut has_directories = false;
    let mut has_remote_directories = false;
    let mut has_files = false;
    let mut chd_sources = Vec::new();
    let mut restorable_chds = Vec::new();
    let mut orphan_bins: Vec<String> = Vec::new();
    let mut unique_extensions = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut seen_restorable = BTreeSet::new();

    for path in paths {
        let path_str = path.to_string_lossy();

        if path_str.starts_with("remote://") {
            let ext = path
                .extension()
                .and_then(OsStr::to_str)
                .map(|e| format!(".{}", e.to_ascii_lowercase()))
                .unwrap_or_default();

            let is_known_file = ext == ".chd"
                || ARCHIVE_EXTENSIONS.contains(&ext.as_str())
                || CD_SOURCE_EXTENSIONS.contains(&ext.as_str())
                || DVD_SOURCE_EXTENSIONS.contains(&ext.as_str())
                || PAIRABLE_BIN_EXTENSIONS.contains(&ext.as_str());

            if !is_known_file {
                // No recognised file extension — treat as a remote directory.
                if let Some(ref rm) = remote_manager {
                    has_directories = true;
                    let file_paths = rm.list_remote_files_recursive(&path_str, max_depth).unwrap_or_default();
                    for file_path in file_paths {
                        let file_pb = PathBuf::from(&file_path);
                        let fext = file_pb
                            .extension()
                            .and_then(OsStr::to_str)
                            .map(|e| format!(".{}", e.to_ascii_lowercase()))
                            .unwrap_or_default();
                        if ARCHIVE_EXTENSIONS.contains(&fext.as_str()) {
                            archives.push(file_path);
                        } else if fext == ".chd" {
                            let key = file_path.to_lowercase();
                            if seen_restorable.insert(key) {
                                restorable_chds.push(file_path);
                            }
                        } else if let Some(source) = detect_chd_source_remote(&file_pb) {
                            if seen_sources.insert(source.source_path.clone()) {
                                for e in &source.display_extensions {
                                    unique_extensions.insert(e.clone());
                                }
                                chd_sources.push(source);
                            }
                        }
                    }
                } else {
                    has_remote_directories = true;
                }
                continue;
            }

            has_files = true;
            if ARCHIVE_EXTENSIONS.contains(&ext.as_str()) {
                archives.push(path_str.to_string());
            }
            if ext == ".chd" {
                let key = path_str.to_lowercase();
                if seen_restorable.insert(key) {
                    restorable_chds.push(path_str.to_string());
                }
            }
            if let Some(source) = detect_chd_source_remote(path) {
                if seen_sources.insert(source.source_path.clone()) {
                    for e in &source.display_extensions {
                        unique_extensions.insert(e.clone());
                    }
                    chd_sources.push(source);
                }
            }
            continue;
        }

        if path.is_dir() {
            has_directories = true;
            let walk_depth = if max_depth == 0 { usize::MAX } else { max_depth };
            for item in WalkDir::new(path).max_depth(walk_depth).into_iter().filter_map(|entry| entry.ok()) {
                let candidate = item.path().to_path_buf();
                if is_chd_file(&candidate) {
                    let key = candidate.to_string_lossy().to_lowercase();
                    if seen_restorable.insert(key) {
                        restorable_chds.push(candidate.to_string_lossy().to_string());
                    }
                }
                match detect_chd_source(&candidate)? {
                    Some(source) => {
                        if seen_sources.insert(source.source_path.clone()) {
                            for ext in &source.display_extensions {
                                unique_extensions.insert(ext.clone());
                            }
                            chd_sources.push(source);
                        }
                    }
                    None => {
                        // No source found — check if this is an orphan .bin (no matching .cue)
                        let ext = candidate.extension().and_then(OsStr::to_str)
                            .map(|e| format!(".{}", e.to_ascii_lowercase()))
                            .unwrap_or_default();
                        if PAIRABLE_BIN_EXTENSIONS.contains(&ext.as_str()) {
                            let bin_str = candidate.to_string_lossy().to_string();
                            if !orphan_bins.contains(&bin_str) {
                                orphan_bins.push(bin_str);
                            }
                        }
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
        match detect_chd_source(path)? {
            Some(source) => {
                if seen_sources.insert(source.source_path.clone()) {
                    for ext in &source.display_extensions {
                        unique_extensions.insert(ext.clone());
                    }
                    chd_sources.push(source);
                }
            }
            None => {
                let ext = path.extension().and_then(OsStr::to_str)
                    .map(|e| format!(".{}", e.to_ascii_lowercase()))
                    .unwrap_or_default();
                if PAIRABLE_BIN_EXTENSIONS.contains(&ext.as_str()) {
                    let bin_str = path.to_string_lossy().to_string();
                    if !orphan_bins.contains(&bin_str) {
                        orphan_bins.push(bin_str);
                    }
                }
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
        has_remote_directories,
        orphan_bins,
    })
}

pub fn convert_to_chd(
    app: &AppHandle,
    job_id: &str,
    paths: &[PathBuf],
    options: ChdConversionOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let chdman = chdman_path(app).ok_or_else(|| anyhow!("No se encontro chdman integrado."))?;
    ensure_executable(&chdman)?;
    let analysis = scan_selection(paths, usize::MAX, None)?;
    let selected_root = paths.iter().find(|path| path.is_dir()).cloned();
    let mut touched_directories = BTreeSet::new();

    // Separate sources with missing files so they don't affect the progress denominator.
    for source in analysis.chd_sources.iter().filter(|s| !s.missing_files.is_empty()) {
        let name = PathBuf::from(&source.source_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        emit_log(app, job_id, format!(
            "Saltando '{}': faltan ficheros requeridos — {}",
            name,
            source.missing_files.join(", ")
        ))?;
    }
    let valid_sources: Vec<_> = analysis.chd_sources.iter()
        .filter(|s| s.missing_files.is_empty())
        .collect();
    let total = valid_sources.len().max(1) as f64;
    let is_single = valid_sources.len() == 1;

    for (index, source) in valid_sources.iter().enumerate() {
        let prog_base = index as f64 / total;
        let prog_span = 1.0 / total;
        let source_path = PathBuf::from(&source.source_path);
        let source_is_remote = remote::RemoteManager::is_remote_path(&source.source_path);

        // Download the container directory for remote sources so chdman can access all required files.
        // TempGuard ensures cleanup even on early return via `?`.
        let (effective_source_path, _guard) = if source_is_remote {
            let rm = remote_manager.as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el archivo"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            rm.download_remote_dir_to_local(&source.container_dir, &dl_dir)?;
            emit_log(app, job_id, format!("Directorio remoto descargado: {}", source.container_dir))?;
            let leaf = source_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            (dl_dir.join(&leaf), Some(TempGuard(dl_dir)))
        } else {
            (source_path.clone(), None)
        };

        // Re-detect from the local copy to get proper required_paths (CUE → BIN list).
        let local_source = detect_chd_source(&effective_source_path)
            .ok()
            .flatten()
            .unwrap_or_else(|| ChdSourceDto {
                source_path: effective_source_path.to_string_lossy().to_string(),
                container_dir: effective_source_path.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                command: source.command.clone(),
                display_extensions: source.display_extensions.clone(),
                required_paths: vec![effective_source_path.to_string_lossy().to_string()],
                missing_files: vec![],
            });
        let container_dir = effective_source_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| effective_source_path.clone());

        let source_name = effective_source_path
            .file_name()
            .unwrap_or(effective_source_path.as_os_str())
            .to_string_lossy()
            .to_string();

        // When source is remote, derive destination based on destination_mode.
        let effective_remote_dest: Option<String> = if options.remote_destination.is_some() {
            options.remote_destination.clone()
        } else if source_is_remote {
            match options.destination_mode.as_deref() {
                Some("custom") => None, // local custom path — no remote upload
                Some("parent") => {
                    // Place CHD in the parent of the container dir (grandparent of source file)
                    remote_parent_of(&source_path)
                        .and_then(|container| remote_parent_of(&PathBuf::from(container)))
                }
                _ => remote_parent_of(&source_path), // "same" or default
            }
        } else {
            None
        };

        let use_remote = remote_manager.is_some()
            && pause_registry.is_some()
            && effective_remote_dest.is_some();

        if use_remote {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            let policy = options.remote_transfer.clone()
                .unwrap_or_else(|| RemoteTransferPolicy { on_error: "abort".to_string() });
            let chd_name = build_chd_output_filename(&effective_source_path, &options, is_single);
            let chdman_c = chdman.clone();
            let source_cmd = local_source.command.clone();
            let effective_source_c = effective_source_path.clone();
            let overwrite = options.overwrite;

            run_remote_file_cycle(
                app, job_id, &rm, pr, remote_dest, &policy, &source_name,
                |temp_dir| {
                    let output_path = temp_dir.join(&chd_name);
                    run_chdman_convert(app, job_id, prog_base, prog_span, &chdman_c, &source_cmd, &effective_source_c, &output_path, overwrite)?;
                    Ok(vec![output_path])
                },
            )?;
        } else {
            let mut output_path = build_chd_output_path(
                &effective_source_path,
                &container_dir,
                selected_root.as_deref(),
                &options,
                is_single,
            );
            // Not overwriting + collision → rename so chdman writes a fresh file.
            if output_path.exists() && !options.overwrite && options.rename_on_conflict {
                output_path = unique_path(&output_path);
            }
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            run_chdman_convert(app, job_id, prog_base, prog_span, &chdman, &local_source.command, &effective_source_path, &output_path, options.overwrite)
                .map_err(|e| { let _ = emit_log(app, job_id, e.to_string()); e })?;
            emit_log(app, job_id, format!("CHD creado: {}", output_path.display()))?;
        }

        if options.delete_originals {
            for original in &source.required_paths {
                if source_is_remote {
                    if let Some(ref rm) = remote_manager {
                        rm.delete_entry(app, job_id, original)?;
                        emit_log(app, job_id, format!("Original remoto eliminado: {original}"))?;
                    }
                } else {
                    let original_path = PathBuf::from(original);
                    if original_path.exists() {
                        remove_single_path(&original_path)?;
                        emit_log(app, job_id, format!("Original eliminado: {}", original_path.display()))?;
                    }
                }
            }
        }

        // Track local container dirs so we can remove them if they're empty after cleanup.
        if options.delete_original_subfolders && !source_is_remote {
            touched_directories.insert(container_dir);
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Convirtiendo {}", effective_source_path.display()),
        )?;
    }

    // Remove each source folder that is now empty (deepest first so nested dirs are cleaned up).
    if options.delete_original_subfolders {
        let mut directories = touched_directories.into_iter().collect::<Vec<_>>();
        directories.sort_by_key(|directory| std::cmp::Reverse(directory.components().count()));
        for directory in directories {
            if directory.exists() && fs::read_dir(&directory)?.next().is_none() {
                fs::remove_dir(&directory)?;
                emit_log(app, job_id, format!("Carpeta original eliminada: {}", directory.display()))?;
            }
        }
    }

    Ok(())
}

fn run_chdman_convert(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    chdman: &Path,
    command: &str,
    source: &Path,
    output: &Path,
    overwrite: bool,
) -> Result<()> {
    let mut cmd = Command::new(chdman);
    cmd.arg(command).arg("-i").arg(source).arg("-o").arg(output);
    if overwrite {
        cmd.arg("-f");
    }
    // chdman prints "Compressing, NN.N% complete..." to stderr, updated with \r.
    run_command_streaming(app, job_id, base, span, cmd)
        .map_err(|e| anyhow!(if e.to_string().is_empty() { "chdman devolvio un error".to_string() } else { e.to_string() }))
}

/// Returns just the filename (not the full path) for a CHD output file.
fn build_chd_output_filename(
    source_path: &Path,
    options: &ChdConversionOptionsPayload,
    is_single: bool,
) -> String {
    if is_single {
        if let Some(ref name) = options.custom_name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                let stem = trimmed.strip_suffix(".chd").unwrap_or(trimmed);
                return format!("{stem}.chd");
            }
        }
    }
    let stem = source_path
        .file_stem()
        .unwrap_or(source_path.as_os_str())
        .to_string_lossy();
    format!("{stem}.chd")
}

pub fn load_chdman_metadata(app: &AppHandle) -> serde_json::Value {
    let metadata_path = runtime_root(app).join("third_party").join("chdman").join("metadata.json");
    fs::read_to_string(metadata_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Probes whether the bundled chdman binary can actually execute on this machine.
/// On Linux the binary may fail to launch if required shared libraries are missing.
/// Running with no args makes chdman print its command list and exit — a fast, safe probe.
pub fn probe_chdman_runtime(app: &AppHandle) -> crate::models::ToolRuntimeDto {
    use crate::models::ToolRuntimeDto;

    let Some(path) = chdman_path(app) else {
        return ToolRuntimeDto {
            available: false,
            path: None,
            version: None,
            error: Some("chdman no encontrado en los recursos de la aplicación".to_string()),
        };
    };

    let path_str = path.to_string_lossy().to_string();
    match Command::new(&path).output() {
        Ok(output) => {
            // The binary ran (even if exit code != 0 — chdman exits 1 with no args).
            // Grab the first non-empty line from stdout or stderr as the version hint.
            let version = [&output.stdout, &output.stderr]
                .iter()
                .find_map(|b| {
                    let s = String::from_utf8_lossy(b);
                    s.lines().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string())
                });
            ToolRuntimeDto { available: true, path: Some(path_str), version, error: None }
        }
        Err(e) => {
            // OS-level failure — missing shared lib, wrong architecture, etc.
            ToolRuntimeDto {
                available: false,
                path: Some(path_str),
                version: None,
                error: Some(e.to_string()),
            }
        }
    }
}

pub fn restore_from_chd(
    app: &AppHandle,
    job_id: &str,
    paths: &[PathBuf],
    options: ChdRestoreOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let chdman = chdman_path(app).ok_or_else(|| anyhow!("No se encontro chdman integrado."))?;
    ensure_executable(&chdman)?;
    let analysis = scan_selection(paths, usize::MAX, None)?;
    let total = analysis.restorable_chds.len().max(1) as f64;
    let is_single = analysis.restorable_chds.len() == 1;

    for (index, source) in analysis.restorable_chds.iter().enumerate() {
        let prog_base = index as f64 / total;
        let prog_span = 1.0 / total;
        let chd_path = PathBuf::from(source);
        let chd_is_remote = remote::RemoteManager::is_remote_path(source);

        // Download remote .chd to a temp dir so chdman can read it locally.
        // TempGuard ensures cleanup even on early return via `?`.
        let (effective_chd_path, _guard) = if chd_is_remote {
            let rm = remote_manager.as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el CHD"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            emit_log(app, job_id, format!("[Descarga] CHD remoto: {source}"))?;
            let (local, bytes) = rm.download_file_to_dir(source, &dl_dir)
                .map_err(|e| anyhow!("Fallo al descargar CHD remoto '{source}': {e}"))?;
            emit_log(app, job_id, format!("[Descarga] OK: {} bytes → {}", bytes, local.display()))?;
            (local, Some(TempGuard(dl_dir)))
        } else {
            (chd_path.clone(), None)
        };

        let chd_stem = effective_chd_path
            .file_stem()
            .unwrap_or_else(|| effective_chd_path.as_os_str())
            .to_string_lossy()
            .to_string();

        // When source is remote and destination is "same", send output back to the remote parent.
        let effective_remote_dest: Option<String> = if options.remote_destination.is_some() {
            options.remote_destination.clone()
        } else if chd_is_remote && options.destination_mode != "custom" {
            remote_parent_of(&chd_path)
        } else {
            None
        };

        let destination_root = if options.destination_mode == "custom" {
            options
                .destination_path
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| effective_chd_path.parent().unwrap_or(&effective_chd_path).to_path_buf())
        } else {
            effective_chd_path.parent().unwrap_or(&effective_chd_path).to_path_buf()
        };

        // Folder naming: custom only for single-CHD operations
        let folder_name = if options.individual_folders {
            let name = if is_single && options.folder_naming_mode == "custom" {
                options.custom_folder_name.as_deref()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .unwrap_or(&chd_stem)
                    .to_string()
            } else {
                chd_stem.clone()
            };
            Some(name)
        } else {
            None
        };

        // Output file stem: custom only for single-CHD operations
        let stem = if is_single && options.output_naming_mode == "custom" {
            options.custom_output_name.as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(&chd_stem)
                .to_string()
        } else {
            chd_stem.clone()
        };

        let use_remote_dest = remote_manager.is_some()
            && pause_registry.is_some()
            && effective_remote_dest.is_some();

        if use_remote_dest {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            let policy = options.remote_transfer.clone()
                .unwrap_or_else(|| RemoteTransferPolicy { on_error: "abort".to_string() });
            let chdman_c = chdman.clone();
            let effective_chd_c = effective_chd_path.clone();
            let stem_c = stem.clone();
            let overwrite = options.overwrite;
            let split_bin = options.split_bin;
            let folder_name_c = folder_name.clone();
            let source_name = effective_chd_path.file_name().unwrap_or(effective_chd_path.as_os_str()).to_string_lossy().to_string();

            run_remote_file_cycle(
                app, job_id, &rm, pr, remote_dest, &policy, &source_name,
                |temp_dir| {
                    let write_dir = if let Some(ref name) = folder_name_c {
                        let d = temp_dir.join(name);
                        fs::create_dir_all(&d)?;
                        d
                    } else {
                        temp_dir.to_path_buf()
                    };
                    let cue_path = write_dir.join(format!("{stem_c}.cue"));
                    let bin_path = write_dir.join(format!("{stem_c}.bin"));
                    let iso_path = write_dir.join(format!("{stem_c}.iso"));

                    match run_extractcd(app, job_id, prog_base, prog_span, &chdman_c, &effective_chd_c, &cue_path, &bin_path, overwrite, split_bin) {
                        Ok(_) => {},
                        Err(first_err) => {
                            cleanup_restore_outputs(&cue_path, &bin_path, &write_dir, &stem_c, &effective_chd_c)?;
                            run_extractdvd(app, job_id, prog_base, prog_span, &chdman_c, &effective_chd_c, &iso_path, overwrite).map_err(|dvd_err| {
                                anyhow!(
                                    "No se pudo recuperar como CD ni DVD.\nCD: {}\nDVD: {}",
                                    first_err, dvd_err
                                )
                            })?;
                        }
                    }

                    if folder_name_c.is_some() {
                        Ok(vec![write_dir])
                    } else {
                        collect_dir_entries(temp_dir)
                    }
                },
            )?;
        } else {
            let destination = if let Some(ref name) = folder_name {
                destination_root.join(name)
            } else {
                destination_root
            };
            fs::create_dir_all(&destination)?;

            // Not overwriting + collision → rename the output stem so the
            // .cue/.bin pair lands on fresh names instead of failing.
            let effective_stem = if !options.overwrite && options.rename_on_conflict {
                unique_restore_stem(&destination, &stem)
            } else {
                stem.clone()
            };

            emit_log(
                app,
                job_id,
                format!("Recuperando contenido desde {} -> {}", effective_chd_path.display(), destination.display()),
            )?;

            let cue_path = destination.join(format!("{effective_stem}.cue"));
            let bin_path = destination.join(format!("{effective_stem}.bin"));
            let iso_path = destination.join(format!("{effective_stem}.iso"));

            match run_extractcd(app, job_id, prog_base, prog_span, &chdman, &effective_chd_path, &cue_path, &bin_path, options.overwrite, options.split_bin) {
                Ok(_) => {
                    emit_log(app, job_id, format!("Extraido como CD: {}", cue_path.display()))?;
                }
                Err(error) => {
                    let first_error = error.to_string();
                    cleanup_restore_outputs(&cue_path, &bin_path, &destination, &effective_stem, &effective_chd_path)?;
                    run_extractdvd(app, job_id, prog_base, prog_span, &chdman, &effective_chd_path, &iso_path, options.overwrite).map_err(|dvd_error| {
                        anyhow!(
                            "No se pudo recuperar {} como CD ni como DVD.\nCD: {}\nDVD: {}",
                            effective_chd_path.display(), first_error, dvd_error
                        )
                    })?;
                    emit_log(app, job_id, format!("Extraido como DVD: {}", iso_path.display()))?;
                }
            }
        }

        if options.delete_chd {
            if chd_is_remote {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, source)?;
                    emit_log(app, job_id, format!("CHD remoto eliminado: {source}"))?;
                }
            } else {
                fs::remove_file(&chd_path)?;
                emit_log(app, job_id, format!("CHD eliminado tras recuperar contenido: {}", chd_path.display()))?;
            }
        }

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!("Recuperando {}", effective_chd_path.display()),
        )?;
    }

    Ok(())
}

fn extract_zip(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    archive_path: &Path,
    destination: &Path,
    overwrite: bool,
    rename_on_conflict: bool,
) -> Result<()> {
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let count = archive.len().max(1) as f64;
    for index in 0..archive.len() {
        let mut item = archive.by_index(index)?;
        let mut out_path = destination.join(item.mangled_name());
        if item.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if out_path.exists() && !overwrite {
            if rename_on_conflict {
                out_path = unique_path(&out_path);
            } else {
                return Err(anyhow!(
                    "Ya existe un fichero y sobrescritura no esta permitida: {}",
                    out_path.display()
                ));
            }
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&out_path)?;
        std::io::copy(&mut item, &mut output)?;
        let frac = (index + 1) as f64 / count;
        let _ = emit_progress(app, job_id, (base + span * frac).clamp(0.0, 1.0), format!("{}", out_path.display()));
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

/// Classify a remote:// path as a CHD source by extension only (no file reading).
/// required_paths will contain just the source itself; the backend resolves siblings after download.
/// Returns filenames (not full paths) of required files that do not exist on disk.
fn check_required_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter(|p| !p.exists())
        .map(|p| {
            p.file_name()
                .unwrap_or(p.as_os_str())
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

fn detect_chd_source_remote(path: &Path) -> Option<ChdSourceDto> {
    let path_str = path.to_string_lossy().to_string();
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default();
    // container = everything up to and including the last '/'
    let container_dir = {
        let s = path_str.as_str();
        s.rfind('/').map(|i| s[..=i].trim_end_matches('/').to_string())
            .unwrap_or_else(|| path_str.clone())
    };
    if CD_SOURCE_EXTENSIONS.contains(&ext.as_str()) {
        return Some(ChdSourceDto {
            source_path: path_str.clone(),
            container_dir,
            command: "createcd".to_string(),
            display_extensions: vec![ext],
            required_paths: vec![path_str],
            missing_files: vec![], // Cannot verify remote files at scan time
        });
    }
    if DVD_SOURCE_EXTENSIONS.contains(&ext.as_str()) {
        return Some(ChdSourceDto {
            source_path: path_str.clone(),
            container_dir,
            command: "createdvd".to_string(),
            display_extensions: vec![ext],
            required_paths: vec![path_str],
            missing_files: vec![],
        });
    }
    None
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
        let missing_files = check_required_paths(&required_paths);
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
            missing_files,
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
            missing_files: vec![], // .iso is self-contained
        }));
    }

    if PAIRABLE_BIN_EXTENSIONS.contains(&extension.as_str()) {
        let cue_candidate = path.with_extension("cue");
        if cue_candidate.exists() {
            let required_paths = parse_cue_references(&cue_candidate)?;
            let missing_files = check_required_paths(&required_paths);
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
                missing_files,
            }));
        }
        if let Some(referenced_cue) = find_cue_for_bin(path)? {
            let required_paths = parse_cue_references(&referenced_cue)?;
            let missing_files = check_required_paths(&required_paths);
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
                missing_files,
            }));
        }
        // .bin with no matching .cue anywhere in the folder — caller handles as orphan
    }

    Ok(None)
}

fn parse_cue_references(path: &Path) -> Result<Vec<PathBuf>> {
    let mut references = vec![path.to_path_buf()];
    // Use lossy conversion — many .cue files from older games use Windows-1252 / Latin-1, which
    // would silently return an empty string with read_to_string, leaving the bin list empty.
    let content = fs::read(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.to_ascii_uppercase().starts_with("FILE ") {
            continue;
        }
        // CUE sheets use either  FILE "name.bin" BINARY  or  FILE name.bin BINARY
        let after_file = trimmed[5..].trim_start();
        let filename: Option<&str> = if after_file.starts_with('"') {
            // Quoted: extract between the two double-quotes
            after_file[1..].find('"').map(|end| &after_file[1..1 + end])
        } else {
            // Unquoted: the last whitespace-separated token is the type keyword (BINARY, WAVE…),
            // everything before it is the filename.
            after_file.rfind(char::is_whitespace)
                .map(|pos| after_file[..pos].trim())
                .filter(|s| !s.is_empty())
        };
        if let Some(name) = filename {
            if !name.is_empty() {
                references.push(path.parent().unwrap_or(path).join(name));
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
    is_single: bool,
) -> PathBuf {
    // Priority: custom_name (single-source only) → nameAsContainer → source file stem
    let raw_stem = if is_single {
        if let Some(name) = options.custom_name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            // Strip .chd suffix if the user accidentally typed it
            name.trim_end_matches(".chd").trim_end_matches(".CHD").to_string()
        } else if options.name_as_container {
            container_dir.file_name().unwrap_or_else(|| container_dir.as_os_str()).to_string_lossy().to_string()
        } else {
            source_path.file_stem().unwrap_or_else(|| source_path.as_os_str()).to_string_lossy().to_string()
        }
    } else if options.name_as_container {
        container_dir.file_name().unwrap_or_else(|| container_dir.as_os_str()).to_string_lossy().to_string()
    } else {
        source_path.file_stem().unwrap_or_else(|| source_path.as_os_str()).to_string_lossy().to_string()
    };

    // Explicit local destination overrides all other placement logic
    if options.destination_mode.as_deref() == Some("custom") {
        if let Some(ref dest) = options.destination_path {
            return PathBuf::from(dest).join(format!("{raw_stem}.chd"));
        }
    }

    let mut output_dir = source_path.parent().unwrap_or(source_path).to_path_buf();
    if options.deposit_to_parent {
        if let Some(root) = selected_root {
            if source_path.starts_with(root) && source_path.parent().unwrap_or(source_path) != root {
                output_dir = root.to_path_buf();
            }
        }
    }
    output_dir.join(format!("{raw_stem}.chd"))
}

fn run_extractcd(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
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
    run_command_streaming(app, job_id, base, span, command)
        .map_err(|e| anyhow!(if e.to_string().is_empty() { "extractcd devolvio un error".to_string() } else { e.to_string() }))
}

fn run_extractdvd(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    chdman: &Path,
    input: &Path,
    iso_path: &Path,
    overwrite: bool,
) -> Result<()> {
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
    run_command_streaming(app, job_id, base, span, command)
        .map_err(|e| anyhow!(if e.to_string().is_empty() { "extractdvd devolvio un error".to_string() } else { e.to_string() }))
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

/// Parses the first "NN%" / "NN.N%" in a line into a 0..1 fraction.
fn first_percent_fraction(line: &str) -> Option<f64> {
    let pct = line.find('%')?;
    let num: String = line[..pct]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    let v: f64 = num.parse().ok()?;
    if (0.0..=100.0).contains(&v) {
        Some(v / 100.0)
    } else {
        None
    }
}

/// Reads `reader` byte-by-byte, splitting on '\n' and '\r' (so tools that redraw
/// a line with carriage returns are handled), emits progress for any line with a
/// percentage, and returns the full captured text.
fn stream_progress_lines<R: std::io::Read>(
    reader: R,
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
) -> String {
    use std::io::{BufReader, Read};
    let mut reader = BufReader::new(reader);
    let mut captured = String::new();
    let mut seg: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    let mut last_emitted = -1.0_f64; // throttle: tools redraw progress very frequently
    loop {
        match reader.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                let b = byte[0];
                if b == b'\n' || b == b'\r' {
                    if !seg.is_empty() {
                        let line = String::from_utf8_lossy(&seg).to_string();
                        captured.push_str(&line);
                        captured.push('\n');
                        if let Some(frac) = first_percent_fraction(&line) {
                            let p = (base + span * frac).clamp(0.0, 1.0);
                            if (p - last_emitted).abs() >= 0.005 || p >= 0.999 {
                                last_emitted = p;
                                let _ = emit_progress(app, job_id, p, line.trim().to_string());
                            }
                        }
                        seg.clear();
                    }
                } else {
                    seg.push(b);
                }
            }
            Err(_) => break,
        }
    }
    if !seg.is_empty() {
        captured.push_str(&String::from_utf8_lossy(&seg));
    }
    captured
}

/// Spawns `command` and streams live progress from BOTH of its output streams
/// (different tools print progress to stdout or stderr) into `emit_progress` as
/// `base + span * fraction`. Returns Ok on success, or Err with the captured
/// output on failure.
fn run_command_streaming(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    mut command: Command,
) -> Result<()> {
    use std::process::Stdio;

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;

    // Read stderr on a worker thread (needs owned clones), stdout on this thread.
    let stderr_handle = child.stderr.take().map(|pipe| {
        let app_c = app.clone();
        let job_c = job_id.to_string();
        std::thread::spawn(move || stream_progress_lines(pipe, &app_c, &job_c, base, span))
    });

    let out_captured = match child.stdout.take() {
        Some(pipe) => stream_progress_lines(pipe, app, job_id, base, span),
        None => String::new(),
    };
    let err_captured = stderr_handle.map(|h| h.join().unwrap_or_default()).unwrap_or_default();

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        let mut msg = err_captured.trim().to_string();
        let out = out_captured.trim();
        if !out.is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str(out);
        }
        if msg.is_empty() {
            msg = "El proceso devolvió un error".to_string();
        }
        Err(anyhow!(msg))
    }
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

pub fn list_volumes() -> Result<Vec<VolumeDto>> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let mut volumes: Vec<VolumeDto> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .map(|disk| {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let mount_trimmed = mount.trim_end_matches(['/', '\\']).to_string();
            let label = disk.name().to_string_lossy().to_string();
            let display_label = if label.is_empty() { "Local Disk".to_string() } else { label.clone() };
            let name = if mount_trimmed.is_empty() {
                display_label.clone()
            } else {
                format!("{} ({})", display_label, mount_trimmed)
            };
            VolumeDto {
                path: mount,
                name,
                label,
                total_bytes: disk.total_space(),
                free_bytes: disk.available_space(),
                is_removable: disk.is_removable(),
            }
        })
        .collect();
    volumes.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(volumes)
}

pub fn get_known_folders() -> KnownFoldersDto {
    fn to_str(p: Option<std::path::PathBuf>) -> Option<String> {
        p.map(|pb| pb.to_string_lossy().to_string())
    }
    KnownFoldersDto {
        home: to_str(dirs::home_dir()),
        desktop: to_str(dirs::desktop_dir()),
        documents: to_str(dirs::document_dir()),
        downloads: to_str(dirs::download_dir()),
        pictures: to_str(dirs::picture_dir()),
        music: to_str(dirs::audio_dir()),
        videos: to_str(dirs::video_dir()),
    }
}

// ── Remote transfer cycle ──────────────────────────────────────────

/// Generic helper for "convert locally to temp dir → upload → cleanup" cycle.
///
/// `produce` receives a fresh temp directory path and must write the output
/// files there, returning their paths. This function then uploads each file
/// to `remote_dest_dir` and removes the temp dir. If an upload fails the
/// `policy` governs behaviour: abort / skip / pause-and-ask.
pub fn run_remote_file_cycle<F>(
    app: &AppHandle,
    job_id: &str,
    remote_manager: &remote::RemoteManager,
    pause_registry: &pause::PauseRegistry,
    remote_dest_dir: &str,
    policy: &RemoteTransferPolicy,
    source_name: &str,
    produce: F,
) -> Result<()>
where
    F: FnOnce(&Path) -> Result<Vec<PathBuf>>,
{
    use tauri::Manager;
    let temp_root = app
        .path()
        .app_data_dir()
        .context("No se pudo obtener el directorio de datos de la aplicacion")?
        .join("temp");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let temp_dir = temp_root.join(format!("remote-cycle-{}-{}", job_id, nanos));
    fs::create_dir_all(&temp_dir)?;
    let _guard = TempGuard(temp_dir.clone()); // cleaned up on any exit path

    let output_files = produce(&temp_dir)?;

    // Ensure the remote destination directory exists
    remote_manager.ensure_remote_dir(remote_dest_dir)
        .map_err(|e| anyhow!("No se pudo verificar el directorio remoto '{}': {}", remote_dest_dir, e))?;

    for file in &output_files {
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "fichero".to_string());

        let _ = emit_log(app, job_id, format!("Subiendo {}...", file_name));
        'retry: loop {
            match remote_manager.upload_file_to_remote(app, job_id, file, remote_dest_dir) {
                Ok(_) => {
                    let _ = emit_log(app, job_id, format!("{} subido correctamente", file_name));
                    break 'retry;
                }
                Err(upload_err) => match policy.on_error() {
                    "skip" => {
                        let _ = emit_log(
                            app,
                            job_id,
                            format!("Subida omitida ({source_name} → {file_name}): {upload_err}"),
                        );
                        break 'retry;
                    }
                    "pause" => {
                        let decision = pause_registry.pause_for_decision(
                            app,
                            job_id,
                            &upload_err.to_string(),
                            &file_name,
                            true,
                        );
                        match decision {
                            pause::PauseDecision::Retry => continue 'retry,
                            pause::PauseDecision::Skip => {
                                let _ = emit_log(
                                    app,
                                    job_id,
                                    format!("Subida omitida por el usuario: {file_name}"),
                                );
                                break 'retry;
                            }
                            pause::PauseDecision::Abort => {
                                return Err(anyhow!("Operacion cancelada por el usuario."));
                            }
                        }
                    }
                    _ => {
                        return Err(upload_err);
                    }
                },
            }
        }
    }

    Ok(())
}

// ── Remote transfer pre-flight ─────────────────────────────────────

/// Returns the free bytes on the disk that contains `path`.
/// Uses sysinfo to enumerate mounted disks and picks the longest-prefix match.
pub fn free_space_at(path: &Path) -> Result<u64> {
    use sysinfo::Disks;
    let path_str = path.to_string_lossy().to_lowercase();
    let disks = Disks::new_with_refreshed_list();
    let best = disks
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point().to_string_lossy().to_lowercase();
            if path_str.starts_with(mount.as_str()) {
                Some((mount.len(), d.available_space()))
            } else {
                None
            }
        })
        .max_by_key(|(prefix_len, _)| *prefix_len);
    best.map(|(_, free)| free)
        .ok_or_else(|| anyhow!("No se pudo determinar el espacio libre en: {}", path.display()))
}

/// Rough estimate of the output size for a single source file given the operation kind.
/// chd-convert  → ×1.0 (CHDs are about the same size as the combined source)
/// chd-restore  → ×1.5 (raw/bin tends to be a bit larger than compressed CHD)
/// extract      → ×3.0 (archives may be highly compressed; conservative upper bound)
pub fn estimate_output_size(source: &Path, op_kind: &str) -> u64 {
    let size = fs::metadata(source).map(|m| m.len()).unwrap_or(0);
    let factor = match op_kind {
        "chd-convert" => 1.0_f64,
        "chd-restore" => 1.5,
        "compress" => 0.5,
        _ => 3.0, // extract
    };
    (size as f64 * factor) as u64
}

/// Pre-flight check: compare estimated output sizes against free space at the temp dir.
/// Returns warnings (lowSpace / criticalSpace / noSpaceCheck) without blocking the operation.
pub fn preflight_remote_transfer(
    app: &AppHandle,
    sources: &[PathBuf],
    op_kind: &str,
) -> Result<PreflightCheckResult> {
    use tauri::Manager;
    let temp_dir = app
        .path()
        .app_data_dir()
        .context("No se pudo obtener el directorio de datos de la aplicación")?
        .join("temp");

    let largest_estimate = sources
        .iter()
        .map(|s| estimate_output_size(s, op_kind))
        .max()
        .unwrap_or(0);

    let free = match free_space_at(&temp_dir) {
        Ok(f) => f,
        Err(_) => {
            return Ok(PreflightCheckResult {
                ok: true,
                warnings: vec![PreflightWarning {
                    kind: "noSpaceCheck".to_string(),
                    detail: format!(
                        "No se pudo determinar el espacio libre en {}",
                        temp_dir.display()
                    ),
                }],
            });
        }
    };

    let threshold_critical = largest_estimate; // < 1× → critical
    let threshold_low = (largest_estimate as f64 * 1.5) as u64; // < 1.5× → low

    if free < threshold_critical {
        Ok(PreflightCheckResult {
            ok: false,
            warnings: vec![PreflightWarning {
                kind: "criticalSpace".to_string(),
                detail: format!(
                    "Espacio libre insuficiente: {} MB disponibles, se estiman {} MB necesarios.",
                    free / 1_048_576,
                    largest_estimate / 1_048_576
                ),
            }],
        })
    } else if free < threshold_low {
        Ok(PreflightCheckResult {
            ok: true,
            warnings: vec![PreflightWarning {
                kind: "lowSpace".to_string(),
                detail: format!(
                    "Espacio ajustado: {} MB disponibles, se estiman {} MB necesarios.",
                    free / 1_048_576,
                    largest_estimate / 1_048_576
                ),
            }],
        })
    } else {
        Ok(PreflightCheckResult {
            ok: true,
            warnings: vec![],
        })
    }
}

// ── Compression ────────────────────────────────────────────────────────────────

/// Detects the `rar` binary: first checks PATH, then known WinRAR install dirs on Windows.
pub fn find_rar_binary() -> Option<PathBuf> {
    // `rar` with no args exits non-zero but that's fine — Ok means the binary was found.
    if Command::new("rar").output().is_ok() {
        return Some(PathBuf::from("rar"));
    }
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\WinRAR\rar.exe",
            r"C:\Program Files (x86)\WinRAR\rar.exe",
        ];
        for c in &candidates {
            if Path::new(c).exists() {
                return Some(PathBuf::from(c));
            }
        }
    }
    None
}

pub fn get_compression_capabilities(app: &AppHandle) -> CompressionCapabilitiesDto {
    CompressionCapabilitiesDto {
        can_compress_zip: true,
        can_compress_7z: seven_zip_path(app).is_some(),
        can_compress_rar: find_rar_binary().is_some(),
    }
}

pub fn compress_to_archive(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: CompressionOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let ext = match options.format.as_str() {
        "7z" => "7z",
        "rar" => "rar",
        _ => "zip",
    };
    let archive_filename = format!("{}.{}", options.archive_name.trim(), ext);

    let any_source_is_remote = sources.iter().any(|p| remote::RemoteManager::is_remote_path(&p.to_string_lossy()));

    // Download remote sources to a temp dir so compression tools can access them.
    // TempGuard ensures cleanup even on early return via `?`.
    let (effective_sources, _guard) = if any_source_is_remote {
        let rm = remote_manager.as_ref()
            .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar los archivos"))?;
        let dl_dir = temp_download_dir(app, job_id)?;
        let mut local_sources = Vec::with_capacity(sources.len());
        for src in &sources {
            if remote::RemoteManager::is_remote_path(&src.to_string_lossy()) {
                let src_str = src.to_string_lossy();
                emit_log(app, job_id, format!("[Descarga] {}", src_str))?;
                let (local, bytes) = rm.download_file_to_dir(&src_str, &dl_dir)
                    .map_err(|e| anyhow!("Fallo al descargar '{}': {e}", src_str))?;
                emit_log(app, job_id, format!("[Descarga] OK: {} bytes → {}", bytes, local.display()))?;
                local_sources.push(local);
            } else {
                local_sources.push(src.clone());
            }
        }
        (local_sources, Some(TempGuard(dl_dir)))
    } else {
        (sources.clone(), None)
    };

    // When sources are remote and destination is "same", send archive to the remote parent of sources[0].
    let effective_remote_dest: Option<String> = if options.remote_destination.is_some() {
        options.remote_destination.clone()
    } else if any_source_is_remote && options.destination_mode != "custom" {
        sources.first().and_then(|p| remote_parent_of(p))
    } else {
        None
    };

    let dest_folder = match options.destination_mode.as_str() {
        "custom" => options
            .destination_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Modo personalizado sin destination_path"))?,
        _ => effective_sources
            .first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow!("No se pudo determinar la carpeta de destino"))?,
    };

    let use_remote = remote_manager.is_some()
        && pause_registry.is_some()
        && effective_remote_dest.is_some();

    if use_remote {
        let rm = Arc::clone(remote_manager.as_ref().unwrap());
        let pr = pause_registry.as_ref().unwrap();
        let remote_dest = effective_remote_dest.as_deref().unwrap();
        let policy = options
            .remote_transfer
            .clone()
            .unwrap_or_else(|| RemoteTransferPolicy { on_error: "abort".to_string() });
        let effective_sources_c = effective_sources.clone();
        let options_c = options.clone();
        let archive_filename_c = archive_filename.clone();
        run_remote_file_cycle(
            app, job_id, &rm, pr, remote_dest, &policy, &archive_filename,
            |temp_dir| {
                let archive_path = temp_dir.join(&archive_filename_c);
                do_compress(app, job_id, 0.0, 0.9, &effective_sources_c, &archive_path, &options_c)?;
                Ok(vec![archive_path])
            },
        )?;
    } else {
        let mut archive_path = dest_folder.join(&archive_filename);
        if archive_path.exists() && !options.overwrite {
            if options.rename_on_conflict {
                archive_path = unique_path(&archive_path);
            } else {
                return Err(anyhow!(
                    "El archivo '{}' ya existe. Activa la opción de sobreescribir.",
                    archive_filename
                ));
            }
        }
        emit_log(
            app, job_id,
            format!("Comprimiendo {} elemento(s) → {}", effective_sources.len(), archive_path.display()),
        )?;
        do_compress(app, job_id, 0.0, 0.9, &effective_sources, &archive_path, &options)?;
        emit_log(app, job_id, format!("Archivo creado: {}", archive_path.display()))?;
    }

    emit_progress(app, job_id, 0.9, format!("Comprimiendo {}", archive_filename))?;

    if options.delete_originals {
        for source in &sources {
            if remote::RemoteManager::is_remote_path(&source.to_string_lossy()) {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, &source.to_string_lossy())?;
                    emit_log(app, job_id, format!("Original remoto eliminado: {}", source.display()))?;
                }
            } else if source.is_dir() {
                fs::remove_dir_all(source)?;
                emit_log(app, job_id, format!("Original eliminado: {}", source.display()))?;
            } else {
                fs::remove_file(source)?;
                emit_log(app, job_id, format!("Original eliminado: {}", source.display()))?;
            }
        }
    }

    emit_progress(app, job_id, 1.0, "Compresión completada.".to_string())?;
    Ok(())
}

fn do_compress(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    sources: &[PathBuf],
    archive_path: &Path,
    options: &CompressionOptionsPayload,
) -> Result<()> {
    match options.format.as_str() {
        "7z" => compress_via_7z(app, job_id, base, span, sources, archive_path, options.compression_level),
        "rar" => {
            let rar = find_rar_binary()
                .ok_or_else(|| anyhow!("RAR no está disponible en este sistema"))?;
            compress_via_rar(&rar, sources, archive_path, options.compression_level)
        }
        _ => compress_to_zip(app, job_id, base, span, sources, archive_path, options.compression_level),
    }
}

/// Counts regular files under `path` (1 if `path` is itself a file).
fn count_files_under(path: &Path) -> u64 {
    if path.is_file() {
        1
    } else {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count() as u64
    }
}

fn compress_to_zip(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    sources: &[PathBuf],
    archive_path: &Path,
    level: u8,
) -> Result<()> {
    let file = fs::File::create(archive_path)?;
    let mut zip = zip::ZipWriter::new(file);

    let (method, zip_level) = if level == 0 {
        (zip::CompressionMethod::Stored, None)
    } else {
        (zip::CompressionMethod::Deflated, Some(level as i64))
    };
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(zip_level);

    let total: u64 = sources.iter().map(|s| count_files_under(s)).sum::<u64>().max(1);
    let mut done: u64 = 0;

    for source in sources {
        let src_base = source.parent().unwrap_or(source);
        add_path_to_zip(app, job_id, base, span, total, &mut done, &mut zip, source, src_base, options)?;
    }
    zip.finish()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add_path_to_zip<W: Write + Seek>(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    total: u64,
    done: &mut u64,
    zip: &mut zip::ZipWriter<W>,
    path: &Path,
    base_dir: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    if path.is_file() {
        let name = path
            .strip_prefix(base_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(&name, options)?;
        let mut f = fs::File::open(path)?;
        io::copy(&mut f, zip)?;
        *done += 1;
        let _ = emit_progress(app, job_id, (base + span * (*done as f64 / total as f64)).clamp(0.0, 1.0), name);
    } else if path.is_dir() {
        for entry in WalkDir::new(path).min_depth(0).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            let name = entry_path
                .strip_prefix(base_dir)
                .unwrap_or(entry_path)
                .to_string_lossy()
                .replace('\\', "/");
            if name.is_empty() {
                continue;
            }
            if entry_path.is_dir() {
                zip.add_directory(format!("{name}/"), options)?;
            } else {
                zip.start_file(&name, options)?;
                let mut f = fs::File::open(entry_path)?;
                io::copy(&mut f, zip)?;
                *done += 1;
                let _ = emit_progress(app, job_id, (base + span * (*done as f64 / total as f64)).clamp(0.0, 1.0), name);
            }
        }
    }
    Ok(())
}

fn compress_via_7z(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    sources: &[PathBuf],
    archive_path: &Path,
    level: u8,
) -> Result<()> {
    let tool = seven_zip_path(app)
        .ok_or_else(|| anyhow!("No se encontró 7zz/7z integrado ni en PATH."))?;
    ensure_executable(&tool)?;
    let mut cmd = Command::new(&tool);
    cmd.arg("a")
        .arg(archive_path)
        .args(sources)
        .arg(format!("-mx={level}"))
        .arg("-y")
        .arg("-bsp1"); // stream progress percentage to stdout
    run_command_streaming(app, job_id, base, span, cmd)
        .map_err(|e| anyhow!("7zz devolvió un error al comprimir: {}", e))
}

fn compress_via_rar(rar_binary: &Path, sources: &[PathBuf], archive_path: &Path, level: u8) -> Result<()> {
    // RAR levels: 0=store, 1=fastest, 2=fast, 3=normal, 4=good, 5=best
    let rar_level = match level {
        0 => 0u8,
        1..=2 => 1,
        3..=4 => 2,
        5..=6 => 3,
        7..=8 => 4,
        _ => 5,
    };
    let status = Command::new(rar_binary)
        .arg("a")
        .arg(format!("-m{rar_level}"))
        .arg("-y")
        .arg(archive_path)
        .args(sources)
        .status()?;
    if !status.success() {
        return Err(anyhow!("rar devolvió un error al comprimir"));
    }
    Ok(())
}

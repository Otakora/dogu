use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
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
        CompressionCapabilitiesDto, CompressionOptionsPayload, DiscImageOptionsPayload, EntryDto,
        ExtractionOptionsPayload, ExtractionPreviewEntry, ExtractionPreviewRow, JobFinishedDto,
        JobLogDto, JobProgressDto, KnownFoldersDto, PreflightCheckResult, PreflightWarning,
        PropertiesSummaryDto, RemoteTransferPolicy, SelectionAnalysisDto, SummaryOptionsPayload,
        VolumeDto,
    },
    pause,
    process::hide_console_window,
    remote,
    sidecars::{
        chdman_path, dolphin_tool_path, rar_capable_seven_zip_path, runtime_root, seven_zip_path,
    },
};

const ARCHIVE_EXTENSIONS: &[&str] = &[".zip", ".7z", ".rar"];
const CD_SOURCE_EXTENSIONS: &[&str] = &[".cue", ".gdi", ".toc"];
const DVD_SOURCE_EXTENSIONS: &[&str] = &[".iso"];
const PAIRABLE_BIN_EXTENSIONS: &[&str] = &[".bin"];
const DISC_TRACK_EXTENSIONS: &[&str] = &[".bin", ".raw"];

fn path_identity(path: &Path) -> String {
    let value = path.to_string_lossy();
    if cfg!(windows) && !value.starts_with("remote://") { value.to_lowercase() } else { value.into_owned() }
}

fn track_identity(path: &Path) -> String {
    if path.to_string_lossy().starts_with("remote://") { return path_identity(path); }
    path_identity(&fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn remote_track_reference(parent: &str, name: &str) -> Option<String> {
    let rest = parent.strip_prefix("remote://")?;
    let (session, dir) = rest.split_once('/').unwrap_or((rest, ""));
    let mut components: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();
    let normalized = name.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {},
            ".." => { components.pop()?; },
            part => components.push(part),
        }
    }
    Some(format!("remote://{session}/{}", components.join("/")))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveFormat {
    Zip,
    SevenZip,
    Rar,
}

fn archive_format(path: &Path) -> Result<ArchiveFormat> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "zip" => Ok(ArchiveFormat::Zip),
        "7z" => Ok(ArchiveFormat::SevenZip),
        "rar" => Ok(ArchiveFormat::Rar),
        _ => Err(anyhow!("Formato no soportado: {}", path.display())),
    }
}

fn archive_tool(app: &AppHandle, format: ArchiveFormat) -> Result<PathBuf> {
    match format {
        ArchiveFormat::Zip => Err(anyhow!("ZIP no requiere herramienta externa")),
        ArchiveFormat::SevenZip => {
            seven_zip_path(app).ok_or_else(|| anyhow!("No se encontro 7-Zip integrado ni en PATH."))
        }
        ArchiveFormat::Rar => rar_capable_seven_zip_path(app).ok_or_else(|| {
            anyhow!(
                "No se encontro una herramienta compatible con archivos .rar. \
El bundle de Dogu debe incluir 7z.exe/7z.dll en Windows o 7zz en Linux; 7za.exe no soporta RAR."
            )
        }),
    }
}

fn command_output_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match (stderr.is_empty(), stdout.is_empty()) {
        (false, false) => format!("{stderr}\n{stdout}"),
        (false, true) => stderr,
        (true, false) => stdout,
        (true, true) => "El proceso devolvio un error".to_string(),
    }
}

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
        for item in WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
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
    fs::OpenOptions::new().write(true).create_new(true).open(&target)
        .with_context(|| format!("No se pudo crear el fichero {}", target.display()))?;
    Ok(target.to_string_lossy().to_string())
}

pub fn rename_path(path: &Path, new_name: &str) -> Result<String> {
    if new_name.is_empty() || new_name == "." || new_name == ".." || new_name.contains(['/', '\\']) {
        return Err(anyhow!("Nombre de destino invalido"));
    }
    let target = path.with_file_name(new_name);
    let same_windows_path = cfg!(windows) && path.to_string_lossy().eq_ignore_ascii_case(&target.to_string_lossy());
    if !same_windows_path {
        match fs::symlink_metadata(&target) {
            Ok(_) => return Err(anyhow!("El destino ya existe: {}", target.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
        }
    }
    fs::rename(path, &target).with_context(|| {
        format!(
            "No se pudo renombrar {} a {}",
            path.display(),
            target.display()
        )
    })?;
    Ok(target.to_string_lossy().to_string())
}

/// Permanently deletes a single file or (recursively) a directory. Used by the
/// destination picker's inline delete — a direct, synchronous counterpart to
/// `create_folder`/`rename_path` (the job-based `delete_paths` is for bulk ops).
pub fn delete_entry(path: &Path) -> Result<()> {
    remove_single_path(path).with_context(|| format!("No se pudo eliminar {}", path.display()))
}

/// Returns `desired` if it doesn't exist, otherwise the first non-existing
/// "stem (N)" variant (N starting at 1), preserving any extension. Works for
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
    let mut n: u32 = 1;
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
        dir.join(format!("{s}.gdi")).exists()
            || dir.join(format!("{s}.cue")).exists()
            || dir.join(format!("{s}.bin")).exists()
            || dir.join(format!("{s}.iso")).exists()
    };
    if !collides(stem) {
        return stem.to_string();
    }
    let mut n: u32 = 1;
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
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
        emit_log(app, job_id, format!("{indent}  omitido: ya no existe"))?;
        return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() {
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
    fixed_target_name: Option<&str>,
) -> Result<()> {
    let total = paths.len().max(1) as f64;
    for (index, source) in paths.iter().enumerate() {
        let mut target = destination.join(
            fixed_target_name.map(std::ffi::OsStr::new).unwrap_or(source
                .file_name()
                .ok_or_else(|| anyhow!("Ruta invalida: {}", source.display()))?),
        );
        if target.exists() {
            if overwrite {
                // Preserve the previous destination until the new entry is ready.
            } else if rename_on_conflict && fixed_target_name.is_none() {
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
                if operation == "cut" {
                    "Moviendo"
                } else {
                    "Copiando"
                },
                source.display(),
                target.display()
            ),
        )?;

        copy_or_move_one_staged(source, &target, operation == "cut", overwrite)?;

        emit_progress(
            app,
            job_id,
            (index + 1) as f64 / total,
            format!(
                "{} {}",
                if operation == "cut" {
                    "Moviendo"
                } else {
                    "Copiando"
                },
                source.display()
            ),
        )?;
    }
    Ok(())
}

pub fn open_path(path: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        hide_console_window(&mut command)
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
        let open_with = PathBuf::from(system_root)
            .join("System32")
            .join("OpenWith.exe");
        if open_with.exists() {
            let mut command = Command::new(&open_with);
            hide_console_window(&mut command).arg(path).spawn()?;
        } else {
            let mut command = Command::new("rundll32.exe");
            hide_console_window(&mut command)
                .args(["shell32.dll,OpenAs_RunDLL", &path.to_string_lossy()])
                .spawn()?;
        }
        return Ok(());
    }

    let status = Command::new("gio").arg("open").arg(path).status();
    if !matches!(status, Ok(status) if status.success()) {
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
    let destination_root = if options.destination_mode == "parent" {
        destination_root.parent().unwrap_or(&destination_root).to_path_buf()
    } else { destination_root };
    let destination_root = if let Some(name) = &options.shared_folder_name { destination_root.join(name) } else { destination_root };
    if options.individual_folders {
        destination_root.join(options.planned_folder_name.clone().unwrap_or_else(|| archive
                .file_stem()
                .unwrap_or_else(|| archive.as_os_str())
                .to_string_lossy()
                .to_string()))
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
    let format = archive_format(archive_path)?;
    match format {
        ArchiveFormat::Zip => list_top_level_zip_entries(archive_path),
        ArchiveFormat::SevenZip | ArchiveFormat::Rar => {
            let tool = archive_tool(app, format)?;
            ensure_executable(&tool)?;
            list_top_level_7z_entries(&tool, archive_path)
        }
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
            seen.entry(name)
                .and_modify(|d| *d = *d || is_dir)
                .or_insert(is_dir);
        }
    }
    Ok(seen.into_iter().collect())
}

fn list_top_level_7z_entries(tool: &Path, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let mut command = Command::new(tool);
    let output = hide_console_window(&mut command)
        .arg("l")
        .arg("-slt")
        // Force UTF-8 console output; otherwise 7-Zip emits non-ASCII names (accents)
        // in the OEM code page, which `from_utf8_lossy` turns into replacement chars —
        // corrupting ghost paths so they no longer match the real extracted files.
        .arg("-sccUTF-8")
        .arg(archive_path)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "No se pudo listar el contenido de {} con {}.\n{}",
            archive_path.display(),
            tool.display(),
            command_output_message(&output),
        ));
    }
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    for (normalized, is_dir) in parse_7z_slt_entries(&String::from_utf8_lossy(&output.stdout)) {
        let mut parts = normalized.split('/').filter(|p| !p.is_empty());
        if let Some(first) = parts.next() {
            let has_more = parts.next().is_some();
            let entry_is_dir = has_more || is_dir;
            seen.entry(first.to_string())
                .and_modify(|d| *d = *d || entry_is_dir)
                .or_insert(entry_is_dir);
        }
    }

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
pub fn list_all_entries(app: &AppHandle, archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let format = archive_format(archive_path)?;
    match format {
        ArchiveFormat::Zip => list_all_zip_entries(archive_path),
        ArchiveFormat::SevenZip | ArchiveFormat::Rar => {
            let tool = archive_tool(app, format)?;
            ensure_executable(&tool)?;
            list_all_7z_entries(&tool, archive_path)
        }
    }
}

fn list_all_zip_entries(archive_path: &Path) -> Result<Vec<(String, bool)>> {
    let file = fs::File::open(archive_path)?;
    list_all_zip_reader(file)
}

pub fn list_all_zip_reader(reader: impl Read + Seek) -> Result<Vec<(String, bool)>> {
    let mut archive = ZipArchive::new(reader)?;
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
    let mut command = Command::new(tool);
    let output = hide_console_window(&mut command)
        .arg("l")
        .arg("-slt")
        // See list_top_level_7z_entries: UTF-8 output so accented names survive.
        .arg("-sccUTF-8")
        .arg(archive_path)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "No se pudo listar el contenido de {} con {}.\n{}",
            archive_path.display(),
            tool.display(),
            command_output_message(&output),
        ));
    }
    let mut seen: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    for (path, is_dir) in parse_7z_slt_entries(&String::from_utf8_lossy(&output.stdout)) {
        insert_with_ancestors(&mut seen, &path, is_dir);
    }

    Ok(seen.into_iter().collect())
}

fn parse_7z_slt_entries(text: &str) -> Vec<(String, bool)> {
    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_is_dir = false;
    let mut in_entries = false;

    let flush =
        |path: &mut Option<String>, is_dir: &mut bool, entries: &mut Vec<(String, bool)>| {
            if let Some(raw) = path.take() {
                let normalized = raw.replace('\\', "/");
                let trimmed = normalized.trim_matches('/').to_string();
                if !trimmed.is_empty() {
                    entries.push((trimmed, *is_dir));
                }
            }
            *is_dir = false;
        };

    for line in text.lines() {
        if !in_entries {
            if line.trim_start().starts_with("----------") {
                in_entries = true;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("Path = ") {
            flush(&mut current_path, &mut current_is_dir, &mut entries);
            current_path = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Folder = ") {
            current_is_dir = rest.trim() == "+";
        } else if let Some(rest) = line.strip_prefix("Attributes = ") {
            if rest.contains('D') {
                current_is_dir = true;
            }
        }
    }
    flush(&mut current_path, &mut current_is_dir, &mut entries);

    entries
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
        let raw = list_all_entries(app, archive)?;
        rows.push(extraction_preview_from_entries(archive, options, &raw));
    }
    Ok(rows)
}

pub fn extraction_preview_from_entries(archive: &Path, options: &ExtractionOptionsPayload, raw: &[(String, bool)]) -> ExtractionPreviewRow {
        let destination_root = extraction_destination_root(archive, options);

        // Apply `splitEntries` (each top-level file is moved into a folder named
        // after its stem) and re-synthesise ancestor directories so every level
        // — including any folders split introduces — is represented.
        let mut final_map: std::collections::BTreeMap<String, bool> =
            std::collections::BTreeMap::new();
        for (rel, is_dir) in raw {
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

        ExtractionPreviewRow {
            archive_path: archive.to_string_lossy().to_string(),
            destination_root: destination_root.to_string_lossy().to_string(),
            entries: preview_entries,
        }
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
    let parent = if parent_slash == 0 {
        "/"
    } else {
        &trimmed[..parent_slash]
    };
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
    if let Some(name) = options.planned_folder_name.as_deref() {
        validated_planned_output_name(Some(name), name)?;
    }
    if let Some(name) = options.shared_folder_name.as_deref() {
        validated_planned_output_name(Some(name), name)?;
    }
    let total = archives.len().max(1) as f64;

    for (index, archive) in archives.iter().enumerate() {
        let prog_base = index as f64 / total;
        let prog_span = 1.0 / total;
        let source_name = archive
            .file_name()
            .unwrap_or(archive.as_os_str())
            .to_string_lossy()
            .to_string();
        let archive_is_remote = remote::RemoteManager::is_remote_path(&archive.to_string_lossy());

        // Check the real destination, not just the share root, before a
        // potentially large download and extraction.
        let mut effective_remote_dest: Option<String> = if options.remote_destination.is_some() {
            options.remote_destination.clone()
        } else if archive_is_remote && options.destination_mode != "custom" {
            let parent = remote_parent_of(archive);
            if options.destination_mode == "parent" { parent.as_deref().and_then(|p| remote_parent_of(Path::new(p))).or(parent) } else { parent }
        } else {
            None
        };
        if let Some(destination) = &effective_remote_dest {
            let rm = remote_manager.as_ref().ok_or_else(|| anyhow!("Se necesita el gestor remoto para comprobar el destino"))?;
            emit_log(app, job_id, format!("Comprobando escritura en {destination}..."))?;
            rm.check_remote_staging_writable(destination)?;
        }

        if let (Some(parent), Some(name)) = (&effective_remote_dest, &options.shared_folder_name) {
            let rm = remote_manager.as_ref().ok_or_else(|| anyhow!("Se necesita el gestor remoto para crear el destino"))?;
            let find_folder = || -> Result<Option<String>> {
                let entry = rm.list_children(parent)?.into_iter().find(|entry| entry.name == *name);
                match entry {
                    Some(entry) if entry.is_dir => Ok(Some(entry.path)),
                    Some(_) => Err(anyhow!("Ya existe un archivo con el nombre de la carpeta de destino")),
                    None => Ok(None),
                }
            };
            let folder = match find_folder()? {
                Some(path) => path,
                None => match rm.create_folder(parent, name) {
                    Ok(path) => path,
                    Err(error) => find_folder()?.ok_or(error)?,
                },
            };
            rm.check_remote_staging_writable(&folder)?;
            effective_remote_dest = Some(folder);
        }

        // Download remote archive to a temp dir so tools can open it as a local file.
        // TempGuard ensures the dir is removed even if this iteration returns early via `?`.
        let (effective_archive, _guard) = if archive_is_remote {
            let rm = remote_manager
                .as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el archivo"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            emit_log(app, job_id, format!("Descargando {}...", source_name))?;
            emit_progress(app, job_id, prog_base, format!("Descargando {source_name}"))?;
            let (local, bytes) = rm
                .download_file_to_dir_with_progress(&archive.to_string_lossy(), &dl_dir, Some((app, job_id, prog_base, prog_span * 0.30)))
                .map_err(|e| anyhow!("Fallo al descargar '{}': {}", source_name, e))?;
            emit_log(
                app,
                job_id,
                format!("{} descargado ({} bytes)", source_name, bytes),
            )?;
            (local, Some(TempGuard(dl_dir)))
        } else {
            (archive.clone(), None)
        };

        let use_remote =
            remote_manager.is_some() && pause_registry.is_some() && effective_remote_dest.is_some();

        if use_remote {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            emit_log(app, job_id, format!("Extrayendo {}...", source_name))?;
            let policy = options
                .remote_transfer
                .clone()
                .unwrap_or_else(|| RemoteTransferPolicy {
                    on_error: "abort".to_string(),
                });
            let options_c = options.clone();
            let effective_archive_c = effective_archive.clone();
            run_remote_file_cycle_with_policy(
                app,
                job_id,
                &rm,
                pr,
                remote_dest,
                &policy,
                &source_name,
                options.overwrite,
                Some((prog_base + prog_span * 0.65, prog_span * 0.34)),
                |temp_dir| {
                    let shared_root = temp_dir.to_path_buf();
                    let extract_target = if options_c.individual_folders {
                        let stem = effective_archive_c
                            .file_stem()
                            .unwrap_or(effective_archive_c.as_os_str())
                            .to_string_lossy()
                            .to_string();
                        let sub = shared_root.join(options_c.planned_folder_name.as_deref().unwrap_or(&stem));
                        fs::create_dir_all(&sub)?;
                        sub
                    } else {
                        shared_root
                    };
                    do_extract_archive(
                        app,
                        job_id,
                        prog_base + if archive_is_remote { prog_span * 0.30 } else { 0.0 },
                        prog_span * if archive_is_remote { 0.35 } else { 0.65 },
                        &effective_archive_c,
                        &extract_target,
                        &options_c,
                    )?;
                    if let Some(entries) = &options_c.planned_entries {
                        let remapped = temp_dir.join("dogu-planned-output");
                        remap_extraction_tree(temp_dir, &remapped, entries)?;
                        collect_dir_entries(&remapped)
                    } else { collect_dir_entries(temp_dir) }
                },
            )?;
        } else {
            let destination = extraction_destination_root(&effective_archive, &options);
            let reserved_new_folder = options.individual_folders && options.planned_folder_name.is_some() && !options.overwrite;
            if reserved_new_folder {
                if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }
                fs::create_dir(&destination).with_context(|| format!("No se pudo reservar la carpeta de salida '{}'", destination.display()))?;
            } else { fs::create_dir_all(&destination)?; }
            emit_log(
                app,
                job_id,
                format!(
                    "Descomprimiendo {} -> {}",
                    effective_archive.display(),
                    destination.display()
                ),
            )?;
            // Extraction failures are confined to a directory owned by this
            // attempt; only completed output is published into the destination.
            let publication = (|| -> Result<()> {
                let staging = retry_transient_stage(app, job_id, || {
                    let staging = tempfile::Builder::new().prefix("dogu-extract-").tempdir_in(&destination)?;
                    do_extract_archive(app, job_id, prog_base + if archive_is_remote { prog_span * 0.30 } else { 0.0 }, prog_span * if archive_is_remote { 0.69 } else { 0.99 }, &effective_archive, staging.path(), &options)?;
                    Ok(staging)
                })?;
                if let Some(entries) = &options.planned_entries {
                    let remapped = tempfile::Builder::new().prefix("dogu-mapped-").tempdir_in(&destination)?;
                    remap_extraction_tree(staging.path(), remapped.path(), entries)?;
                    publish_extracted_tree(remapped.path(), &destination, &effective_archive, &options)?;
                } else {
                    publish_extracted_tree(staging.path(), &destination, &effective_archive, &options)?;
                }
                Ok(())
            })();
            if publication.is_err() && reserved_new_folder {
                // Only remove the directory if it is still empty; preserve any
                // partial publication or files created independently by the user.
                let _ = fs::remove_dir(&destination);
            }
            publication?;
        }

        if options.delete_archives {
            if archive_is_remote {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, &archive.to_string_lossy())?;
                    emit_log(
                        app,
                        job_id,
                        format!("Archivo remoto eliminado: {}", archive.display()),
                    )?;
                }
            } else {
                fs::remove_file(archive)?;
                emit_log(
                    app,
                    job_id,
                    format!("Archivo comprimido eliminado: {}", archive.display()),
                )?;
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
) -> Result<()> {
    let format = archive_format(archive)?;
    match format {
        ArchiveFormat::Zip => extract_zip(
            app,
            job_id,
            base,
            span,
            archive,
            destination,
            options.overwrite,
            options.rename_on_conflict,
        )?,
        ArchiveFormat::SevenZip | ArchiveFormat::Rar => {
            let tool = archive_tool(app, format)?;
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
                .with_context(|| format!("7-zip fallo al extraer '{}'", archive.display()))?;
        }
    }
    if options.split_entries {
        let entries = list_top_level_entries(app, archive)?;
        split_top_level_entries(destination, &entries)?;
        emit_log(
            app,
            job_id,
            format!(
                "Elementos de {} separados en carpetas individuales",
                archive.display()
            ),
        )?;
    }
    Ok(())
}

/// Apply the queue's exact relative output names inside an isolated staging tree.
fn remap_extraction_tree(source: &Path, destination: &Path, plans: &[crate::models::PlannedExtractionEntry]) -> Result<()> {
    use std::path::Component;
    let validate = |value: &str| -> Result<PathBuf> {
        let path = Path::new(value);
        if value.is_empty() || path.components().any(|component| !matches!(component, Component::Normal(_))) {
            return Err(anyhow!("Ruta de extraccion planificada invalida: {value}"));
        }
        Ok(path.to_path_buf())
    };
    let relative_key = |path: &Path| path_identity(path).replace('\\', "/");
    let mut mapped = std::collections::HashMap::new();
    let mut targets = std::collections::HashSet::new();
    for plan in plans {
        let from = validate(&plan.from)?;
        let to = validate(&plan.to)?;
        if !targets.insert(relative_key(&to)) || mapped.insert(relative_key(&from), (to, plan.is_dir)).is_some() {
            return Err(anyhow!("La previsualizacion contiene rutas duplicadas"));
        }
    }
    let items = WalkDir::new(source).min_depth(1).into_iter().collect::<std::result::Result<Vec<_>, _>>()?;
    // Renaming a referenced track without rewriting its CUE would create a
    // broken disc image. Stop instead of publishing a misleading result.
    for item in &items {
        if !item.file_type().is_file() || !item.path().extension().and_then(OsStr::to_str).is_some_and(|ext| ext.eq_ignore_ascii_case("cue")) { continue; }
        let cue_relative = item.path().strip_prefix(source)?;
        let Some((cue_target, _)) = mapped.get(&relative_key(cue_relative)) else { continue; };
        for reference in parse_cue_references(item.path())?.into_iter().skip(1) {
            let Ok(source_relative) = reference.strip_prefix(source) else { return Err(anyhow!("El CUE contiene una referencia fuera del archivo extraido")); };
            let Some((target, _)) = mapped.get(&relative_key(source_relative)) else { continue; };
            let cue_dir = item.path().parent().unwrap_or(source);
            let Ok(reference_relative) = reference.strip_prefix(cue_dir) else { return Err(anyhow!("El CUE contiene una referencia fuera de su carpeta")); };
            let expected = cue_target.parent().unwrap_or(Path::new("")).join(reference_relative);
            if relative_key(target) != relative_key(&expected) {
                return Err(anyhow!("El nombre planificado de '{}' romperia las referencias de '{}'. Renombra el destino o usa una carpeta individual.", source_relative.display(), cue_relative.display()));
            }
        }
    }
    fs::create_dir_all(destination)?;
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let relative = item.path().strip_prefix(source)?;
        let key = relative_key(relative);
        let (to, is_dir) = mapped.get(&key).ok_or_else(|| anyhow!("El archivo contiene una entrada no prevista: {}", relative.display()))?;
        if item.file_type().is_dir() != *is_dir || (!item.file_type().is_dir() && !item.file_type().is_file()) {
            return Err(anyhow!("La entrada extraida no coincide con la previsualizacion: {}", relative.display()));
        }
        seen.insert(key);
        let target = destination.join(to);
        if *is_dir { fs::create_dir_all(&target)?; }
        else {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            fs::rename(item.path(), &target)?;
        }
    }
    if seen.len() != mapped.len() { return Err(anyhow!("El archivo no contiene todas las entradas previstas en la cola")); }
    Ok(())
}

/// Merge completed extraction output. Do not auto-retry publishing: a failed
/// publish can already have moved earlier files into the user's destination.
fn publish_extracted_tree(staging: &Path, destination: &Path, archive: &Path, options: &ExtractionOptionsPayload) -> Result<()> {
    let skip_existing = !options.overwrite && !options.rename_on_conflict
        && options.planned_entries.is_none() && options.planned_folder_name.is_none()
        && !matches!(archive_format(archive)?, ArchiveFormat::Zip);
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut reserved = std::collections::HashSet::new();
    for item in WalkDir::new(staging).min_depth(1) {
        let item = item?;
        let relative = item.path().strip_prefix(staging)?;
        let target = destination.join(relative);
        if item.file_type().is_dir() {
            directories.push(target);
        } else if item.file_type().is_file() {
            let mut target = target;
            if !options.overwrite && (target.exists() || reserved.contains(&path_identity(&target))) {
                if options.rename_on_conflict {
                    let parent = target.parent().ok_or_else(|| anyhow!("Destino invalido"))?;
                    let name = target.file_stem().unwrap_or_default().to_string_lossy();
                    let extension = target.extension().map(|ext| format!(".{}", ext.to_string_lossy())).unwrap_or_default();
                    let mut number = 1;
                    loop {
                        let candidate = parent.join(format!("{name} ({number}){extension}"));
                        if !candidate.exists() && !reserved.contains(&path_identity(&candidate)) {
                            target = candidate;
                            break;
                        }
                        number += 1;
                    }
                } else if skip_existing { continue; }
                else { return Err(anyhow!("Ya existe un fichero y sobrescritura no esta permitida: {}", target.display())); }
            }
            reserved.insert(path_identity(&target));
            files.push((item.path().to_path_buf(), target));
        } else {
            return Err(anyhow!("La extraccion contiene un tipo de entrada no admitido: {}", relative.display()));
        }
    }
    for directory in directories { fs::create_dir_all(directory)?; }
    for (source, target) in files {
        if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
        commit_staged_file(&source, &target, options.overwrite)?;
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
    let mut track_candidates: Vec<String> = Vec::new();
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
                || DISC_TRACK_EXTENSIONS.contains(&ext.as_str());

            if !is_known_file {
                // No recognised file extension — treat as a remote directory.
                if let Some(ref rm) = remote_manager {
                    has_directories = true;
                    let file_paths = rm
                        .list_remote_files_recursive(&path_str, max_depth)
                        .unwrap_or_default();
                    for file_path in file_paths {
                        let file_pb = PathBuf::from(&file_path);
                        let fext = file_pb
                            .extension()
                            .and_then(OsStr::to_str)
                            .map(|e| format!(".{}", e.to_ascii_lowercase()))
                            .unwrap_or_default();
                        if DISC_TRACK_EXTENSIONS.contains(&fext.as_str()) { track_candidates.push(file_path.clone()); }
                        if ARCHIVE_EXTENSIONS.contains(&fext.as_str()) {
                            archives.push(file_path);
                        } else if fext == ".chd" {
                            let key = file_path.clone();
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
            if DISC_TRACK_EXTENSIONS.contains(&ext.as_str()) { track_candidates.push(path_str.to_string()); }
            if ARCHIVE_EXTENSIONS.contains(&ext.as_str()) {
                archives.push(path_str.to_string());
            }
            if ext == ".chd" {
                let key = path_str.to_string();
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
            let walk_depth = if max_depth == 0 {
                usize::MAX
            } else {
                max_depth
            };
            for item in WalkDir::new(path)
                .max_depth(walk_depth)
                .into_iter()
                .filter_map(|entry| entry.ok())
            {
                let candidate = item.path().to_path_buf();
                if candidate.is_file() && candidate.extension().and_then(OsStr::to_str).is_some_and(|ext| ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("raw")) {
                    track_candidates.push(candidate.to_string_lossy().to_string());
                }
                if is_chd_file(&candidate) {
                    let key = path_identity(&candidate);
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
                    None => {}
                }
            }
            continue;
        }

        has_files = true;
        if path.extension().and_then(OsStr::to_str).is_some_and(|ext| ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("raw")) {
            track_candidates.push(path.to_string_lossy().to_string());
        }
        if is_archive(path) {
            archives.push(path.to_string_lossy().to_string());
        }
        if is_chd_file(path) {
            let key = path_identity(path);
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
            None => {}
        }
    }

    // Resolve after the complete scan, so selection order and repeated folders do not matter.
    let mut associated = BTreeSet::new();
    let mut gdi_tracks = BTreeSet::new();
    let mut unknown_remote_dirs = BTreeSet::new();
    let mut unknown_remote_gdi_dirs = BTreeSet::new();
    for source in &mut chd_sources {
        if source.source_path.starts_with("remote://") {
            let ext = source.source_path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            if ext != "cue" && ext != "gdi" { continue; }
            let parent = source.source_path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
            let content = remote_manager.as_ref().and_then(|manager| manager.read_disc_descriptor(&source.source_path).ok());
            if let Some(content) = content {
                let names = if ext == "cue" { cue_reference_names(&content) } else { gdi_reference_names(&content) };
                for name in names {
                    if let Some(reference) = remote_track_reference(parent, &name) {
                        if ext == "gdi" { gdi_tracks.insert(reference.clone()); }
                        associated.insert(reference.clone());
                        if !source.required_paths.contains(&reference) { source.required_paths.push(reference); }
                    }
                }
            } else {
                unknown_remote_dirs.insert(parent.to_string());
                if ext == "gdi" { unknown_remote_gdi_dirs.insert(parent.to_string()); }
            }
        } else {
            associated.extend(source.required_paths.iter().map(|p| track_identity(Path::new(p))));
            if source.source_path.to_ascii_lowercase().ends_with(".gdi") {
                gdi_tracks.extend(source.required_paths.iter().map(|p| track_identity(Path::new(p))));
            }
        }
    }
    // Do not guess whether a remote ISO belongs to an unreadable GDI: doing so
    // could silently create an unrelated DVD CHD or suppress a real source.
    if remote_manager.is_some() && chd_sources.iter().any(|source| {
        source.source_path.starts_with("remote://")
            && source.source_path.to_ascii_lowercase().ends_with(".iso")
            && source.source_path.rsplit_once('/').is_some_and(|(parent, _)| unknown_remote_gdi_dirs.contains(parent))
    }) {
        return Err(anyhow!("No se pudo leer un GDI remoto para distinguir sus pistas ISO. Comprueba la conexion y vuelve a intentarlo, o selecciona solo el GDI."));
    }
    chd_sources.retain(|source| {
        !source.source_path.to_ascii_lowercase().ends_with(".iso")
            || !gdi_tracks.contains(&track_identity(Path::new(&source.source_path)))
    });
    unique_extensions = chd_sources.iter().flat_map(|source| source.display_extensions.iter().cloned()).collect();
    let mut seen_tracks = BTreeSet::new();
    let orphan_bins = track_candidates.into_iter().filter(|track| {
        let key = track_identity(Path::new(track));
        if !seen_tracks.insert(key.clone()) || associated.contains(&key) { return false; }
        if track.starts_with("remote://") {
            let parent = track.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
            if unknown_remote_dirs.contains(parent) { return false; }
        }
        true
    }).collect();

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
    let scan_inputs = recover_missing_chd_scan_inputs(paths);
    let analysis = scan_selection(&scan_inputs, usize::MAX, None)?;
    let selected_root = paths.iter().find(|path| path.is_dir()).cloned();
    let mut touched_directories = BTreeSet::new();
    let mut touched_remote_directories = BTreeSet::new();

    // Separate sources with missing files so they don't affect the progress denominator.
    for source in analysis
        .chd_sources
        .iter()
        .filter(|s| !s.missing_files.is_empty())
    {
        let name = PathBuf::from(&source.source_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        emit_log(
            app,
            job_id,
            format!(
                "Saltando '{}': faltan ficheros requeridos — {}",
                name,
                source.missing_files.join(", ")
            ),
        )?;
    }
    let valid_sources: Vec<_> = analysis
        .chd_sources
        .iter()
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
            let rm = remote_manager
                .as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el archivo"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            rm.download_remote_dir_to_local(&source.container_dir, &dl_dir)?;
            emit_log(
                app,
                job_id,
                format!("Directorio remoto descargado: {}", source.container_dir),
            )?;
            let leaf = source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            (dl_dir.join(&leaf), Some(TempGuard(dl_dir)))
        } else {
            (source_path.clone(), None)
        };

        // Re-detect from the local copy to get proper required_paths (CUE → BIN list).
        let local_source = detect_chd_source(&effective_source_path)?
            .ok_or_else(|| anyhow!("No se pudo analizar la imagen de origen: {}", effective_source_path.display()))?;
        if !local_source.missing_files.is_empty() {
            return Err(anyhow!("Faltan ficheros requeridos por '{}': {}", source.source_path, local_source.missing_files.join(", ")));
        }
        let remote_originals = if source_is_remote && options.delete_originals {
            remote_original_paths(&source.container_dir, &effective_source_path, &local_source.required_paths)?
        } else { Vec::new() };
        let original_container_dir = source_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| source_path.clone());
        let mut staging_guard: Option<TempGuard> = None;
        let (run_source_path, run_source_command) =
            if !source_is_remote && should_stage_local_chd_source(source) {
                let (staged_source_path, guard) = stage_local_chd_source(app, job_id, source)?;
                staging_guard = Some(guard);
                (staged_source_path, source.command.clone())
            } else {
                (effective_source_path.clone(), local_source.command.clone())
            };

        let source_name = source_path
            .file_name()
            .unwrap_or(source_path.as_os_str())
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

        let use_remote =
            remote_manager.is_some() && pause_registry.is_some() && effective_remote_dest.is_some();

        if use_remote {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            let policy = options
                .remote_transfer
                .clone()
                .unwrap_or_else(|| RemoteTransferPolicy {
                    on_error: "abort".to_string(),
                });
            let chd_name = build_chd_output_filename(&source_path, &options, is_single);
            let chdman_c = chdman.clone();
            let source_cmd = run_source_command.clone();
            let run_source_c = run_source_path.clone();
            let overwrite = options.overwrite;

            run_remote_file_cycle_with_policy(
                app,
                job_id,
                &rm,
                pr,
                remote_dest,
                &policy,
                &source_name,
                overwrite,
                None,
                |temp_dir| {
                    let output_path = temp_dir.join(&chd_name);
                    run_chdman_convert(
                        app,
                        job_id,
                        prog_base,
                        prog_span,
                        &chdman_c,
                        &source_cmd,
                        &run_source_c,
                        &output_path,
                        overwrite,
                    )?;
                    Ok(vec![output_path])
                },
            )?;
        } else {
            let mut output_path = build_chd_output_path(
                &source_path,
                &original_container_dir,
                selected_root.as_deref(),
                &options,
                is_single,
            );
            // Not overwriting + collision → rename so chdman writes a fresh file.
            if output_path.exists() && !options.overwrite && options.rename_on_conflict {
                output_path = unique_path(&output_path);
            }
            let final_output_path = output_path.clone();
            let output_path = if staging_guard.is_some() {
                let staging_dir = staging_guard
                    .as_ref()
                    .map(|guard| guard.0.join("chd-out"))
                    .expect("staging_guard checked above");
                fs::create_dir_all(&staging_dir)?;
                let staged = staging_dir.join(
                    final_output_path
                        .file_name()
                        .unwrap_or_else(|| final_output_path.as_os_str()),
                );
                if staged.exists() {
                    remove_single_path(&staged)?;
                }
                staged
            } else {
                output_path
            };
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            run_chdman_convert(
                app,
                job_id,
                prog_base,
                prog_span,
                &chdman,
                &run_source_command,
                &run_source_path,
                &output_path,
                options.overwrite,
            )
            .map_err(|e| {
                let _ = emit_log(app, job_id, e.to_string());
                e
            })?;
            if output_path != final_output_path {
                if final_output_path.exists() && options.overwrite {
                    remove_single_path(&final_output_path)?;
                }
                move_path(&output_path, &final_output_path)?;
            }
            emit_log(
                app,
                job_id,
                format!("CHD creado: {}", final_output_path.display()),
            )?;
        }

        if options.delete_originals {
            let originals = if source_is_remote { &remote_originals } else { &source.required_paths };
            for original in originals {
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
            if options.delete_original_subfolders {
                if source_is_remote {
                    touched_remote_directories.insert(source.container_dir.clone());
                    let root = source.container_dir.trim_end_matches('/');
                    for original in &remote_originals {
                        let mut parent = original.rsplit_once('/').map(|(dir, _)| dir);
                        while let Some(dir) = parent {
                            if dir == root || !dir.starts_with(&format!("{root}/")) { break; }
                            touched_remote_directories.insert(dir.to_string());
                            parent = dir.rsplit_once('/').map(|(next, _)| next);
                        }
                    }
                } else { touched_directories.insert(original_container_dir.clone()); }
            }
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
                emit_log(
                    app,
                    job_id,
                    format!("Carpeta original eliminada: {}", directory.display()),
                )?;
            }
        }
    }

    if let Some(ref rm) = remote_manager {
        let mut directories = touched_remote_directories.into_iter().collect::<Vec<_>>();
        directories.sort_by_key(|directory| std::cmp::Reverse(directory.matches('/').count()));
        for directory in directories {
            if rm.remove_empty_directory(&directory)? {
                emit_log(app, job_id, format!("Carpeta remota vacia eliminada: {directory}"))?;
            }
        }
    }

    Ok(())
}

/// Map only references inside the downloaded container back to its remote path.
fn remote_original_paths(remote_container: &str, local_source: &Path, required: &[String]) -> Result<Vec<String>> {
    let local_container = local_source.parent().ok_or_else(|| anyhow!("No se pudo resolver el directorio descargado"))?;
    required.iter().map(|entry| {
        let path = Path::new(entry);
        let relative = path.strip_prefix(local_container).map_err(|_| anyhow!("La referencia '{}' queda fuera de la carpeta descargada", entry))?;
        let mut parts = Vec::new();
        for component in relative.components() {
            match component {
                std::path::Component::Normal(part) => parts.push(part.to_str().ok_or_else(|| anyhow!("Nombre de pista invalido"))?),
                _ => return Err(anyhow!("La referencia '{}' queda fuera de la carpeta descargada", entry)),
            }
        }
        if parts.is_empty() { return Err(anyhow!("Referencia de pista vacia")); }
        Ok(format!("{}/{}", remote_container.trim_end_matches('/'), parts.join("/")))
    }).collect()
}

fn should_stage_local_chd_source(source: &ChdSourceDto) -> bool {
    source.command == "createcd" && source.required_paths.iter().any(|path| path.len() >= 240)
}

fn stage_local_chd_source(
    app: &AppHandle,
    job_id: &str,
    source: &ChdSourceDto,
) -> Result<(PathBuf, TempGuard)> {
    let container_dir = PathBuf::from(&source.container_dir);
    let source_path = PathBuf::from(&source.source_path);
    let staging_root = temp_download_dir(app, job_id)?;
    let container_name = container_dir
        .file_name()
        .unwrap_or_else(|| container_dir.as_os_str());
    let staged_container = staging_root.join(container_name);
    copy_path(&container_dir, &staged_container)?;
    let staged_source = staged_container.join(
        source_path
            .file_name()
            .unwrap_or_else(|| source_path.as_os_str()),
    );
    Ok((staged_source, TempGuard(staging_root)))
}

fn recover_missing_chd_scan_inputs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();

    for path in paths {
        let recovered = if path.exists() || path.to_string_lossy().starts_with("remote://") {
            path.clone()
        } else {
            // The predicted path is gone. Fall back ONLY to its immediate parent —
            // never a distant shared ancestor — so a single-disc conversion can't
            // reach a common parent folder and over-scan sibling discs (which caused
            // cross-disc collisions and deletions). If the parent is gone too, keep
            // the missing path; scanning will simply skip it instead of over-reaching.
            match path.parent() {
                Some(parent) if parent.exists() && parent.is_dir() => parent.to_path_buf(),
                _ => path.clone(),
            }
        };

        let key = path_identity(&recovered);
        if seen.insert(key) {
            result.push(recovered);
        }
    }

    result
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
    // chdman (MAME) on Windows narrows its argv to the system code page, so any
    // non-ASCII character in a path passed as an argument (é, í, ñ, …) makes it
    // fail with "No such file or directory" even though the file exists. Work
    // around it by setting the working directory to the source's folder — Rust
    // sets the cwd as proper Unicode, which chdman inherits correctly — and passing
    // chdman only RELATIVE, ASCII-safe names: the input by its file name and the
    // output to a temp name in that same folder. The finished CHD is then moved to
    // its real destination with std::fs, which handles the Unicode path chdman can't.
    let source_dir = source
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let source_name = source
        .file_name()
        .ok_or_else(|| anyhow!("Ruta de origen invalida: {}", source.display()))?;

    let staging = tempfile::Builder::new().prefix("dogu-chd-").tempdir_in(&source_dir)?;
    let stage_name = staging.path().file_name().ok_or_else(|| anyhow!("Directorio temporal CHD invalido"))?;
    let tmp_out_name = Path::new(stage_name).join("output.chd");
    let tmp_out_path = staging.path().join("output.chd");

    retry_transient_stage(app, job_id, || {
        if tmp_out_path.exists() { fs::remove_file(&tmp_out_path)?; }
        let mut cmd = Command::new(chdman);
        cmd.arg(command).arg("-i").arg(source_name).arg("-o").arg(&tmp_out_name)
            .arg("-f").current_dir(&source_dir);
        run_command_streaming(app, job_id, base, span, cmd).context("chdman no pudo completar la conversion")
    })?;

    // Relocate the finished CHD to where the caller wants it.
    if output.exists() {
        if overwrite {
            remove_single_path(output)?;
        } else {
            let _ = fs::remove_file(&tmp_out_path);
            return Err(anyhow!("El destino ya existe: {}", output.display()));
        }
    }
    move_path(&tmp_out_path, output).map_err(|e| {
        let _ = fs::remove_file(&tmp_out_path);
        anyhow!("No se pudo mover el CHD generado a su destino: {e}")
    })?;
    Ok(())
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
    let metadata_path = runtime_root(app)
        .join("third_party")
        .join("chdman")
        .join("metadata.json");
    fs::read_to_string(metadata_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Probes whether the bundled chdman binary can actually execute on this machine.
/// On Linux the binary may fail to launch if required shared libraries are missing.
/// Running with no args makes chdman print its command list and exit — a fast, safe probe.
pub fn probe_chdman_runtime(app: &AppHandle) -> crate::models::ToolRuntimeDto {
    probe_tool_runtime(chdman_path(app), "chdman no encontrado en los recursos de la aplicación")
}

/// Generic runtime probe: resolves a bundled/PATH binary and confirms it can be
/// launched by the OS. `resolve` returns the binary path (mirrors chdman/7z/dolphin).
fn probe_tool_runtime(path: Option<PathBuf>, missing_error: &str) -> crate::models::ToolRuntimeDto {
    use crate::models::ToolRuntimeDto;

    let Some(path) = path else {
        return ToolRuntimeDto {
            available: false,
            path: None,
            version: None,
            error: Some(missing_error.to_string()),
        };
    };
    let path_str = path.to_string_lossy().to_string();
    if let Err(error) = ensure_executable(&path) {
        return ToolRuntimeDto { available: false, path: Some(path_str), version: None, error: Some(error.to_string()) };
    }
    let mut command = Command::new(&path);
    match hide_console_window(&mut command).output() {
        Ok(output) => {
            // Missing ELF dependencies terminate in the dynamic loader (127):
            // chdman and DolphinTool legitimately return 1 for their usage text.
            if cfg!(target_os = "linux") && (output.status.code() == Some(127) || output.status.code().is_none()) {
                return ToolRuntimeDto { available: false, path: Some(path_str), version: None, error: Some(command_output_message(&output)) };
            }
            let version = [&output.stdout, &output.stderr].iter().find_map(|b| {
                let s = String::from_utf8_lossy(b);
                s.lines()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string())
            });
            ToolRuntimeDto {
                available: true,
                path: Some(path_str),
                version,
                error: None,
            }
        }
        Err(e) => ToolRuntimeDto {
            available: false,
            path: Some(path_str),
            version: None,
            error: Some(e.to_string()),
        },
    }
}

/// Probes every external tool Dogu relies on and reports the capabilities each
/// one unlocks. Called at startup so the UI can prepare/guide and disable
/// features whose tool is unavailable.
pub fn check_tools(app: &AppHandle) -> Vec<crate::models::ToolStatusDto> {
    use crate::models::ToolStatusDto;

    vec![
        ToolStatusDto {
            id: "chdman".to_string(),
            display_name: "chdman (MAME)".to_string(),
            optional: false,
            provisioning: "bundled".to_string(),
            runtime: probe_chdman_runtime(app),
            enables: vec!["chd".to_string()],
            guidance_url: Some("https://docs.mamedev.org/tools/chdman.html".to_string()),
        },
        ToolStatusDto {
            id: "sevenZip".to_string(),
            display_name: "7-Zip".to_string(),
            optional: false,
            provisioning: "bundled".to_string(),
            runtime: probe_tool_runtime(
                seven_zip_path(app),
                "7-Zip no encontrado en los recursos de la aplicación",
            ),
            enables: vec!["archives".to_string()],
            guidance_url: Some("https://www.7-zip.org/".to_string()),
        },
        ToolStatusDto {
            id: "dolphinTool".to_string(),
            display_name: "DolphinTool".to_string(),
            // Optional: nod is the built-in RVZ engine, so RVZ still works without it.
            optional: true,
            provisioning: "bundled".to_string(),
            runtime: probe_tool_runtime(
                dolphin_tool_path(app),
                "DolphinTool no encontrado en los recursos (motor RVZ nativo 'nod' disponible igualmente)",
            ),
            enables: vec!["rvzDolphin".to_string()],
            guidance_url: Some("https://dolphin-emu.org/download/".to_string()),
        },
    ]
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
            let rm = remote_manager
                .as_ref()
                .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar el CHD"))?;
            let dl_dir = temp_download_dir(app, job_id)?;
            emit_log(app, job_id, format!("[Descarga] CHD remoto: {source}"))?;
            let (local, bytes) = rm
                .download_file_to_dir(source, &dl_dir)
                .map_err(|e| anyhow!("Fallo al descargar CHD remoto '{source}': {e}"))?;
            emit_log(
                app,
                job_id,
                format!("[Descarga] OK: {} bytes → {}", bytes, local.display()),
            )?;
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
                .unwrap_or_else(|| {
                    effective_chd_path
                        .parent()
                        .unwrap_or(&effective_chd_path)
                        .to_path_buf()
                })
        } else {
            effective_chd_path
                .parent()
                .unwrap_or(&effective_chd_path)
                .to_path_buf()
        };

        // Folder naming: custom only for single-CHD operations
        let folder_name = if options.individual_folders {
            let name = if is_single && options.folder_naming_mode == "custom" {
                options
                    .custom_folder_name
                    .as_deref()
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
            options
                .custom_output_name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(&chd_stem)
                .to_string()
        } else {
            chd_stem.clone()
        };

        let use_remote_dest =
            remote_manager.is_some() && pause_registry.is_some() && effective_remote_dest.is_some();

        if use_remote_dest {
            let rm = Arc::clone(remote_manager.as_ref().unwrap());
            let pr = pause_registry.as_ref().unwrap();
            let remote_dest = effective_remote_dest.as_deref().unwrap();
            let policy = options
                .remote_transfer
                .clone()
                .unwrap_or_else(|| RemoteTransferPolicy {
                    on_error: "abort".to_string(),
                });
            let chdman_c = chdman.clone();
            let effective_chd_c = effective_chd_path.clone();
            let stem_c = stem.clone();
            let overwrite = options.overwrite;
            let split_bin = options.split_bin;
            let folder_name_c = folder_name.clone();
            let source_name = effective_chd_path
                .file_name()
                .unwrap_or(effective_chd_path.as_os_str())
                .to_string_lossy()
                .to_string();

            run_remote_file_cycle_with_policy(
                app,
                job_id,
                &rm,
                pr,
                remote_dest,
                &policy,
                &source_name,
                overwrite,
                None,
                |temp_dir| {
                    let write_dir = if let Some(ref name) = folder_name_c {
                        let d = temp_dir.join(name);
                        fs::create_dir_all(&d)?;
                        d
                    } else {
                        temp_dir.to_path_buf()
                    };
                    restore_chd_content(
                        app,
                        job_id,
                        prog_base,
                        prog_span,
                        &chdman_c,
                        &effective_chd_c,
                        &write_dir,
                        &stem_c,
                        overwrite,
                        split_bin,
                    )?;

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
                format!(
                    "Recuperando contenido desde {} -> {}",
                    effective_chd_path.display(),
                    destination.display()
                ),
            )?;

            let restored = restore_chd_content(
                app,
                job_id,
                prog_base,
                prog_span,
                &chdman,
                &effective_chd_path,
                &destination,
                &effective_stem,
                options.overwrite,
                options.split_bin,
            )?;
            emit_log(
                app,
                job_id,
                format!(
                    "Extraido como {}: {}",
                    restored.label(),
                    destination.display()
                ),
            )?;
        }

        if options.delete_chd {
            if chd_is_remote {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, source)?;
                    emit_log(app, job_id, format!("CHD remoto eliminado: {source}"))?;
                }
            } else {
                fs::remove_file(&chd_path)?;
                emit_log(
                    app,
                    job_id,
                    format!(
                        "CHD eliminado tras recuperar contenido: {}",
                        chd_path.display()
                    ),
                )?;
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

// ---- Native disc-image treatments (CSO / XISO) ----

fn leaf_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_else(|| path.as_os_str())
        .to_string_lossy()
        .to_string()
}

fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to)?;
    let _ = fs::remove_file(from);
    Ok(())
}

/// Brings a source local for processing. Remote sources are downloaded into a
/// dedicated per-job temp dir (single file when possible — disc images are
/// self-contained — falling back to a whole-directory download for folder
/// sources like a remote xiso-pack). Returns the local path plus a guard that
/// cleans the download up on any exit path.
fn localize_remote_source(
    app: &AppHandle,
    job_id: &str,
    rm: &remote::RemoteManager,
    source: &Path,
    source_is_remote: bool,
) -> Result<(PathBuf, Option<TempGuard>)> {
    if !source_is_remote {
        return Ok((source.to_path_buf(), None));
    }
    let source_str = source.to_string_lossy().to_string();
    let dl_dir = temp_download_dir(app, job_id)?;
    match rm.download_remote_file(&source_str) {
        Ok(shared) => {
            let local = dl_dir.join(leaf_name(source));
            move_file(&shared, &local)?;
            emit_log(
                app,
                job_id,
                format!("Origen remoto descargado: {}", source.display()),
            )?;
            Ok((local, Some(TempGuard(dl_dir))))
        }
        Err(_) => {
            // Likely a directory source (e.g. packing a remote folder to XISO).
            rm.download_remote_dir_to_local(&source_str, &dl_dir)?;
            emit_log(
                app,
                job_id,
                format!("Carpeta remota descargada: {}", source.display()),
            )?;
            Ok((dl_dir.clone(), Some(TempGuard(dl_dir))))
        }
    }
}

/// Removes the original after a successful op — remote or local.
fn finalize_delete_original(
    app: &AppHandle,
    job_id: &str,
    remote_manager: &Option<Arc<remote::RemoteManager>>,
    source: &Path,
) -> Result<()> {
    let source_str = source.to_string_lossy().to_string();
    if remote::RemoteManager::is_remote_path(&source_str) {
        if let Some(rm) = remote_manager {
            rm.delete_paths(app, job_id, vec![source_str])?;
        }
    } else {
        remove_single_path(source)?;
    }
    emit_log(
        app,
        job_id,
        format!("Original eliminado: {}", source.display()),
    )?;
    Ok(())
}

/// Chooses the effective remote destination directory for a disc-image op:
/// an explicit `remote_destination`, or the remote source's own folder when the
/// source is remote and the destination mode is not a local "custom" path.
fn effective_disc_remote_dest(source: &Path, options: &DiscImageOptionsPayload) -> Option<String> {
    if options.remote_destination.is_some() {
        return options.remote_destination.clone();
    }
    let source_is_remote = remote::RemoteManager::is_remote_path(&source.to_string_lossy());
    if source_is_remote && options.destination_mode != "custom" {
        return remote_parent_of(source);
    }
    None
}

/// Driver for a single-file-output disc-image op on one source, transparently
/// handling remote sources (download → local) and remote destinations (produce
/// to temp → upload → cleanup). `produce(local_source, output_path)` writes the
/// single output file.
#[allow(clippy::too_many_arguments)]
fn unique_remote_output_name(manager: &remote::RemoteManager, destination: &str, desired: &str) -> Result<String> {
    let exists = |name: &str| manager.queue_path_exists(&format!("{}/{}", destination.trim_end_matches('/'), name));
    if !exists(desired)? { return Ok(desired.to_string()); }
    let path = Path::new(desired);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or(desired);
    let extension = path.extension().and_then(OsStr::to_str).map(|ext| format!(".{ext}")).unwrap_or_default();
    for number in 1u32.. {
        let name = format!("{stem} ({number}){extension}");
        if !exists(&name)? { return Ok(name); }
    }
    Err(anyhow!("No se encontro un nombre remoto disponible"))
}

fn validated_planned_output_name<'a>(planned: Option<&'a str>, fallback: &'a str) -> Result<&'a str> {
    let name = planned.unwrap_or(fallback);
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(anyhow!("Nombre de salida planificado invalido"));
    }
    Ok(name)
}

fn run_disc_file_op(
    app: &AppHandle,
    job_id: &str,
    remote_manager: &Option<Arc<remote::RemoteManager>>,
    pause_registry: &Option<pause::PauseRegistry>,
    source: &Path,
    options: &DiscImageOptionsPayload,
    output_name: &str,
    produce: impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let source_is_remote = remote::RemoteManager::is_remote_path(&source.to_string_lossy());
    let output_name = validated_planned_output_name(options.planned_output_name.as_deref(), output_name)?;

    // Fast path: nothing remote — write straight to the local destination.
    if !source_is_remote && options.remote_destination.is_none() {
        let dest_dir = disc_destination_dir(source, options)?;
        fs::create_dir_all(&dest_dir)?;
        let output = resolve_output_path(
            dest_dir.join(output_name),
            options.overwrite,
            options.rename_on_conflict,
        )?;
        emit_log(
            app,
            job_id,
            format!("{} -> {}", source.display(), output.display()),
        )?;
        return produce_staged_disc_file(app, job_id, source, &output, options.overwrite, &produce);
    }

    let rm = remote_manager
        .as_ref()
        .ok_or_else(|| anyhow!("Se necesita el gestor remoto para operaciones remotas."))?;
    let (local_source, _guard) = localize_remote_source(app, job_id, rm, source, source_is_remote)?;

    if let Some(dest) = effective_disc_remote_dest(source, options) {
        let pr = pause_registry
            .as_ref()
            .ok_or_else(|| anyhow!("Se necesita el registro de pausa para subir a remoto."))?;
        let policy = options
            .remote_transfer
            .clone()
            .unwrap_or_else(|| RemoteTransferPolicy {
                on_error: "abort".to_string(),
            });
        let source_name = leaf_name(source);
        let output_name = if !options.overwrite && options.rename_on_conflict {
            unique_remote_output_name(rm, &dest, output_name)?
        } else { output_name.to_string() };
        run_remote_file_cycle_with_policy(
            app,
            job_id,
            rm,
            pr,
            &dest,
            &policy,
            &source_name,
            options.overwrite,
            None,
            |temp_dir| {
                let output = temp_dir.join(&output_name);
                produce(&local_source, &output)?;
                Ok(vec![output])
            },
        )
    } else {
        // Remote source → local (custom) destination.
        let dest_dir = disc_destination_dir(source, options)?;
        fs::create_dir_all(&dest_dir)?;
        let output = resolve_output_path(
            dest_dir.join(output_name),
            options.overwrite,
            options.rename_on_conflict,
        )?;
        produce_staged_disc_file(app, job_id, &local_source, &output, options.overwrite, &produce)
    }
}

fn produce_staged_disc_file(
    app: &AppHandle,
    job_id: &str,
    source: &Path,
    output: &Path,
    overwrite: bool,
    produce: &impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let parent = output.parent().ok_or_else(|| anyhow!("Destino invalido"))?;
    let stage_parent = staging_parent_outside_sources(parent, &[source.to_path_buf()])?;
    let staging = tempfile::Builder::new().prefix("dogu-disc-").tempdir_in(stage_parent)?;
    let staged = staging.path().join(output.file_name().ok_or_else(|| anyhow!("Destino invalido"))?);
    retry_transient_stage(app, job_id, || {
        if staged.exists() { remove_single_path(&staged)?; }
        produce(source, &staged)
    })?;
    commit_staged_file(&staged, output, overwrite)
}

fn resolve_output_path(path: PathBuf, overwrite: bool, rename_on_conflict: bool) -> Result<PathBuf> {
    if !path.exists() || overwrite { return Ok(path); }
    if rename_on_conflict { return Ok(unique_path(&path)); }
    Err(anyhow!("El destino ya existe: {}. Activa sobreescribir o renombrar en conflicto.", path.display()))
}

fn staging_parent_outside_sources(destination: &Path, sources: &[PathBuf]) -> Result<PathBuf> {
    let directories: Vec<PathBuf> = sources.iter().filter(|source| source.is_dir())
        .map(fs::canonicalize).collect::<io::Result<_>>()?;
    let mut parent = fs::canonicalize(destination)?;
    while directories.iter().any(|source| parent.starts_with(source)) {
        parent = parent.parent().ok_or_else(|| anyhow!("No se puede crear una zona temporal fuera de las carpetas de origen"))?.to_path_buf();
    }
    Ok(parent)
}

/// Like `run_disc_file_op` but the op produces a whole output *folder* (e.g.
/// unpack-xiso). For remote destinations the folder is built in a temp dir and
/// uploaded recursively.
#[allow(clippy::too_many_arguments)]
fn run_disc_folder_op(
    app: &AppHandle,
    job_id: &str,
    remote_manager: &Option<Arc<remote::RemoteManager>>,
    _pause_registry: &Option<pause::PauseRegistry>,
    source: &Path,
    options: &DiscImageOptionsPayload,
    folder_name: &str,
    produce: impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let source_is_remote = remote::RemoteManager::is_remote_path(&source.to_string_lossy());
    let folder_name = validated_planned_output_name(options.planned_output_name.as_deref(), folder_name)?;

    if !source_is_remote && options.remote_destination.is_none() {
        let dest_dir = disc_destination_dir(source, options)?;
        fs::create_dir_all(&dest_dir)?;
        let output_dir = resolve_output_path(
            dest_dir.join(folder_name),
            options.overwrite,
            options.rename_on_conflict,
        )?;
        return produce_staged_disc_folder(app, job_id, source, &output_dir, options.overwrite, &produce);
    }

    let rm = remote_manager
        .as_ref()
        .ok_or_else(|| anyhow!("Se necesita el gestor remoto para operaciones remotas."))?;
    let (local_source, _guard) = localize_remote_source(app, job_id, rm, source, source_is_remote)?;

    if let Some(dest) = effective_disc_remote_dest(source, options) {
        let folder_name = if !options.overwrite && options.rename_on_conflict {
            unique_remote_output_name(rm, &dest, folder_name)?
        } else { folder_name.to_string() };
        let (staging, output) = retry_transient_stage(app, job_id, || {
            let staging = tempfile::Builder::new().prefix("dogu-xiso-").tempdir()?;
            let output = staging.path().join(&folder_name);
            fs::create_dir_all(&output)?;
            produce(&local_source, &output)?;
            Ok((staging, output))
        })?;
        rm.ensure_remote_dir(&dest)?;
        if options.overwrite { rm.upload_staged_to_remote(app, job_id, &output, &dest)?; }
        else { rm.upload_staged_no_overwrite(app, job_id, &output, &dest)?; }
        drop(staging);
        Ok(())
    } else {
        let dest_dir = disc_destination_dir(source, options)?;
        fs::create_dir_all(&dest_dir)?;
        let output_dir = resolve_output_path(
            dest_dir.join(folder_name),
            options.overwrite,
            options.rename_on_conflict,
        )?;
        produce_staged_disc_folder(app, job_id, &local_source, &output_dir, options.overwrite, &produce)
    }
}

fn produce_staged_disc_folder(
    app: &AppHandle, job_id: &str, source: &Path, output: &Path, overwrite: bool,
    produce: &impl Fn(&Path, &Path) -> Result<()>,
) -> Result<()> {
    let parent = output.parent().ok_or_else(|| anyhow!("Destino invalido"))?;
    let staging = retry_transient_stage(app, job_id, || {
        let staging = tempfile::Builder::new().prefix("dogu-folder-").tempdir_in(parent)?;
        produce(source, staging.path())?;
        Ok(staging)
    })?;
    publish_staged_directory(staging.path(), output, overwrite)
}

fn publish_staged_directory(staged: &Path, target: &Path, overwrite: bool) -> Result<()> {
    let parent = target.parent().ok_or_else(|| anyhow!("Destino invalido"))?;
    let backup = if target.exists() {
        if !overwrite { return Err(anyhow!("El destino ya existe: {}", target.display())); }
        let backup = tempfile::Builder::new().prefix("dogu-backup-").tempdir_in(parent)?;
        fs::rename(target, backup.path().join("original"))?;
        Some(backup)
    } else { None };
    if let Err(error) = fs::rename(staged, target) {
        if let Some(backup) = backup {
            if let Err(restore_error) = fs::rename(backup.path().join("original"), target) {
                let saved = backup.keep();
                return Err(anyhow!("No se pudo publicar {}: {}. No se pudo restaurar el destino anterior: {}. Copia conservada en {}", target.display(), error, restore_error, saved.join("original").display()));
            }
        }
        return Err(error.into());
    }
    Ok(())
}

fn disc_destination_dir(source: &Path, options: &DiscImageOptionsPayload) -> Result<PathBuf> {
    if options.destination_mode == "custom" {
        return options
            .destination_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Modo personalizado sin destination_path"));
    }
    source
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("No se pudo determinar la carpeta de destino"))
}

fn source_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_else(|| path.as_os_str())
        .to_string_lossy()
        .to_string()
}

fn xiso_output_stem(path: &Path) -> String {
    let stem = source_stem(path);
    stem.strip_suffix(".xiso").unwrap_or(&stem).to_string()
}

pub fn convert_to_cso(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        if ext_lower(source) != "iso" {
            return Err(anyhow!("CSO solo acepta origen .iso: {}", source.display()));
        }
        let base = index as f64 / total;
        let span = 1.0 / total;
        let level = options.compression_level;
        let output_name = format!("{}.cso", source_stem(source));
        run_disc_file_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &output_name,
            |local, output| write_cso_v1(app, job_id, base, span, local, output, level),
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Conversion CSO completada.".to_string())?;
    Ok(())
}

pub fn restore_from_cso(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        if ext_lower(source) != "cso" {
            return Err(anyhow!(
                "La restauracion CSO solo acepta .cso: {}",
                source.display()
            ));
        }
        let base = index as f64 / total;
        let span = 1.0 / total;
        let output_name = format!("{}.iso", source_stem(source));
        run_disc_file_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &output_name,
            |local, output| read_cso_v1(app, job_id, base, span, local, output),
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Restauracion CSO completada.".to_string())?;
    Ok(())
}

pub fn pack_to_xiso(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        let base = index as f64 / total;
        let span = 1.0 / total;
        let output_name = format!("{}.xiso.iso", xiso_output_stem(source));
        run_disc_file_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &output_name,
            |local, output| write_xiso(app, job_id, base, span, local, output),
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Conversion XISO completada.".to_string())?;
    Ok(())
}

pub fn unpack_xiso(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        if ext_lower(source) != "iso" {
            return Err(anyhow!("No es una imagen XISO: {}", source.display()));
        }
        let base = index as f64 / total;
        let span = 1.0 / total;
        let folder_name = xiso_output_stem(source);
        run_disc_folder_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &folder_name,
            |local, output_dir| read_xiso(app, job_id, base, span, local, output_dir),
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Extraccion XISO completada.".to_string())?;
    Ok(())
}

// ---- RVZ (GameCube / Wii): native `nod` engine + DolphinTool fallback ----

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RvzEngine {
    Nod,
    Dolphin,
}

impl RvzEngine {
    fn from_opt(value: &Option<String>, default: RvzEngine) -> RvzEngine {
        match value.as_deref() {
            Some("dolphin") => RvzEngine::Dolphin,
            Some("nod") => RvzEngine::Nod,
            _ => default,
        }
    }

    fn label(self) -> &'static str {
        match self {
            RvzEngine::Nod => "nod",
            RvzEngine::Dolphin => "DolphinTool",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RvzDirection {
    /// ISO -> RVZ
    Encode,
    /// RVZ -> ISO
    Decode,
}

/// Ordered list of engines to try: the configured primary, then the fallback
/// (when enabled and different). Defaults to nod primary, DolphinTool fallback.
fn rvz_engine_order(options: &DiscImageOptionsPayload) -> Vec<RvzEngine> {
    let primary = RvzEngine::from_opt(&options.rvz_primary_engine, RvzEngine::Nod);
    let mut order = vec![primary];
    if options.rvz_enable_fallback {
        let default_fallback = if primary == RvzEngine::Nod {
            RvzEngine::Dolphin
        } else {
            RvzEngine::Nod
        };
        let fallback = RvzEngine::from_opt(&options.rvz_fallback_engine, default_fallback);
        if fallback != primary {
            order.push(fallback);
        }
    }
    order
}

pub fn convert_to_rvz(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        if ext_lower(source) != "iso" {
            return Err(anyhow!("RVZ solo acepta origen .iso: {}", source.display()));
        }
        let base = index as f64 / total;
        let span = 1.0 / total;
        let output_name = format!("{}.rvz", source_stem(source));
        run_disc_file_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &output_name,
            |local, output| {
                run_rvz_operation(
                    app,
                    job_id,
                    base,
                    span,
                    local,
                    output,
                    &options,
                    RvzDirection::Encode,
                )
            },
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Conversion RVZ completada.".to_string())?;
    Ok(())
}

pub fn restore_from_rvz(
    app: &AppHandle,
    job_id: &str,
    sources: Vec<PathBuf>,
    options: DiscImageOptionsPayload,
    remote_manager: Option<Arc<remote::RemoteManager>>,
    pause_registry: Option<pause::PauseRegistry>,
) -> Result<()> {
    let total = sources.len().max(1) as f64;
    for (index, source) in sources.iter().enumerate() {
        if ext_lower(source) != "rvz" {
            return Err(anyhow!(
                "La restauracion RVZ solo acepta .rvz: {}",
                source.display()
            ));
        }
        let base = index as f64 / total;
        let span = 1.0 / total;
        let output_name = format!("{}.iso", source_stem(source));
        run_disc_file_op(
            app,
            job_id,
            &remote_manager,
            &pause_registry,
            source,
            &options,
            &output_name,
            |local, output| {
                run_rvz_operation(
                    app,
                    job_id,
                    base,
                    span,
                    local,
                    output,
                    &options,
                    RvzDirection::Decode,
                )
            },
        )?;
        if options.delete_originals {
            finalize_delete_original(app, job_id, &remote_manager, source)?;
        }
    }
    emit_progress(app, job_id, 1.0, "Restauracion RVZ completada.".to_string())?;
    Ok(())
}

/// Runs one RVZ conversion/restoration, trying each configured engine in order.
/// If an engine fails, its partial output is discarded and the next engine is
/// attempted, so a successful run always leaves a complete, valid file.
fn run_rvz_operation(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
    options: &DiscImageOptionsPayload,
    direction: RvzDirection,
) -> Result<()> {
    let engines = rvz_engine_order(options);
    let mut last_err: Option<anyhow::Error> = None;
    for (attempt, engine) in engines.iter().enumerate() {
        // Discard any partial output left by a previous failed attempt.
        if output.exists() {
            let _ = remove_single_path(output);
        }
        let result = match (engine, direction) {
            (RvzEngine::Nod, RvzDirection::Encode) => rvz_encode_nod(
                app,
                job_id,
                base,
                span,
                source,
                output,
                options.compression_level,
            ),
            (RvzEngine::Nod, RvzDirection::Decode) => {
                rvz_decode_nod(app, job_id, base, span, source, output)
            }
            (RvzEngine::Dolphin, _) => rvz_run_dolphin(
                app,
                job_id,
                base,
                span,
                source,
                output,
                options.compression_level,
                direction,
            ),
        };
        match result {
            Ok(()) => {
                if attempt > 0 {
                    emit_log(
                        app,
                        job_id,
                        format!("RVZ: completado con el motor de reserva {}", engine.label()),
                    )?;
                }
                return Ok(());
            }
            Err(error) => {
                emit_log(
                    app,
                    job_id,
                    format!("RVZ: el motor {} fallo: {error}", engine.label()),
                )?;
                last_err = Some(error);
            }
        }
    }
    if output.exists() {
        let _ = remove_single_path(output);
    }
    Err(last_err.unwrap_or_else(|| anyhow!("No hay ningun motor RVZ disponible.")))
}

fn rvz_encode_nod(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
    compression_level: u8,
) -> Result<()> {
    use nod::common::{Compression, Format};
    use nod::read::{DiscOptions, DiscReader};
    use nod::write::{DiscWriter, FormatOptions, ProcessOptions};

    let disc = DiscReader::new(source, &DiscOptions::default())
        .map_err(|e| anyhow!("nod no pudo abrir la ISO (¿es un disco GameCube/Wii?): {e}"))?;
    let zstd_level = compression_level.clamp(1, 22) as i8;
    let format_options = FormatOptions {
        format: Format::Rvz,
        compression: Compression::Zstandard(zstd_level),
        block_size: Format::Rvz.default_block_size(),
    };
    let writer = DiscWriter::new(disc, &format_options)
        .map_err(|e| anyhow!("nod no pudo iniciar la escritura RVZ: {e}"))?;
    let bound = writer.progress_bound().max(1);

    let mut out = std::io::BufWriter::with_capacity(1024 * 1024, fs::File::create(output)?);
    let finalization = writer
        .process(
            |data, pos, _| {
                out.write_all(data.as_ref())?;
                let progress = base + span * (pos as f64 / bound as f64);
                let _ = emit_progress(
                    app,
                    job_id,
                    progress.clamp(0.0, 0.99),
                    format!(
                        "Escribiendo RVZ (nod) {}%",
                        (pos.saturating_mul(100) / bound).min(100)
                    ),
                );
                Ok(())
            },
            &ProcessOptions::default(),
        )
        .map_err(|e| anyhow!("nod fallo al comprimir a RVZ: {e}"))?;

    // RVZ/WIA write their header last; flush it to the start of the file.
    if !finalization.header.is_empty() {
        out.seek(std::io::SeekFrom::Start(0))?;
        out.write_all(finalization.header.as_ref())?;
    }
    out.flush()?;
    Ok(())
}

fn rvz_decode_nod(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
) -> Result<()> {
    use nod::common::{Compression, Format};
    use nod::read::{DiscOptions, DiscReader};
    use nod::write::{DiscWriter, FormatOptions, ProcessOptions};

    let disc = DiscReader::new(source, &DiscOptions::default())
        .map_err(|e| anyhow!("nod no pudo abrir el RVZ: {e}"))?;
    let format_options = FormatOptions {
        format: Format::Iso,
        compression: Compression::None,
        block_size: Format::Iso.default_block_size(),
    };
    let writer = DiscWriter::new(disc, &format_options)
        .map_err(|e| anyhow!("nod no pudo iniciar la restauracion a ISO: {e}"))?;
    let bound = writer.progress_bound().max(1);

    let mut out = std::io::BufWriter::with_capacity(1024 * 1024, fs::File::create(output)?);
    let finalization = writer
        .process(
            |data, pos, _| {
                out.write_all(data.as_ref())?;
                let progress = base + span * (pos as f64 / bound as f64);
                let _ = emit_progress(
                    app,
                    job_id,
                    progress.clamp(0.0, 0.99),
                    format!(
                        "Restaurando ISO (nod) {}%",
                        (pos.saturating_mul(100) / bound).min(100)
                    ),
                );
                Ok(())
            },
            &ProcessOptions::default(),
        )
        .map_err(|e| anyhow!("nod fallo al restaurar el RVZ a ISO: {e}"))?;
    if !finalization.header.is_empty() {
        out.seek(std::io::SeekFrom::Start(0))?;
        out.write_all(finalization.header.as_ref())?;
    }
    out.flush()?;
    Ok(())
}

fn rvz_run_dolphin(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
    compression_level: u8,
    direction: RvzDirection,
) -> Result<()> {
    let tool = dolphin_tool_path(app).ok_or_else(|| {
        anyhow!("DolphinTool no esta disponible (no se encontro el binario en third_party/dolphin-tool ni en PATH).")
    })?;

    // DolphinTool needs a "user" folder for temporary processing files; point it
    // at a scratch dir so it doesn't create one in Dogu's working directory.
    let user_dir = std::env::temp_dir().join("dogu-dolphintool");
    let _ = fs::create_dir_all(&user_dir);

    let mut command = Command::new(&tool);
    command
        .arg("convert")
        .arg("-u")
        .arg(&user_dir)
        .arg("-i")
        .arg(source)
        .arg("-o")
        .arg(output);
    match direction {
        RvzDirection::Encode => {
            let level = compression_level.clamp(1, 22);
            command
                .arg("-f")
                .arg("rvz")
                .arg("-c")
                .arg("zstd")
                .arg("-l")
                .arg(level.to_string())
                .arg("-b")
                .arg("131072");
        }
        RvzDirection::Decode => {
            command.arg("-f").arg("iso");
        }
    }

    emit_progress(
        app,
        job_id,
        (base + span * 0.05).clamp(0.0, 0.99),
        "DolphinTool procesando...".to_string(),
    )?;
    let result = hide_console_window(&mut command)
        .output()
        .map_err(|e| anyhow!("No se pudo ejecutar DolphinTool: {e}"))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(anyhow!("DolphinTool fallo: {}", stderr.trim()));
    }
    emit_progress(
        app,
        job_id,
        (base + span).min(0.99),
        "DolphinTool completado".to_string(),
    )?;
    Ok(())
}

fn cso_alignment_for_size(size: u64) -> u8 {
    for align in 0..=8 {
        if (size >> align) <= 0x7fff_ffff {
            return align;
        }
    }
    8
}

fn write_cso_v1(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
    compression_level: u8,
) -> Result<()> {
    // CISO/CSO v1 stores each block as a *raw* DEFLATE stream (no zlib header or
    // Adler-32 trailer). This is what maxcso, PCSX2 and PPSSPP expect, so the
    // resulting .cso loads directly in emulators.
    use flate2::{write::DeflateEncoder, Compression};

    const HEADER_SIZE: u64 = 24;
    const BLOCK_SIZE: usize = 2048;
    const RAW_FLAG: u32 = 0x8000_0000;

    let mut input = fs::File::open(source)?;
    let input_len = input.metadata()?.len();
    let sector_count = ((input_len + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64) as usize;
    let index_len = sector_count + 1;
    let min_data_start = HEADER_SIZE + (index_len as u64 * 4);
    let align = cso_alignment_for_size(input_len.max(min_data_start));
    let align_mask = (1_u64 << align) - 1;
    let level = Compression::new(u32::from(compression_level.min(9)));

    let mut out = std::io::BufWriter::new(fs::File::create(output)?);
    out.write_all(&vec![0; min_data_start as usize])?;

    let mut indexes = vec![0_u32; index_len];
    let mut position = min_data_start;
    let mut buffer = vec![0_u8; BLOCK_SIZE];
    for sector in 0..sector_count {
        if align_mask > 0 {
            let padding = (1_u64 << align) - (position & align_mask);
            if padding < (1_u64 << align) {
                out.seek(std::io::SeekFrom::Start(position))?;
                out.write_all(&vec![0; padding as usize])?;
                position += padding;
            }
        }

        let remaining = input_len.saturating_sub((sector as u64) * BLOCK_SIZE as u64);
        let block_len = remaining.min(BLOCK_SIZE as u64) as usize;
        input.read_exact(&mut buffer[..block_len])?;
        // Every CSO block covers a full 2048-byte sector; zero-pad a trailing
        // partial sector so raw blocks are exactly one sector on disk (what
        // maxcso/PCSX2/PPSSPP assume). Real ISOs are sector-aligned, so this
        // only guards non-standard inputs. The reader drops the padding via
        // the true image size in the header.
        if block_len < BLOCK_SIZE {
            buffer[block_len..].fill(0);
        }

        let mut encoder = DeflateEncoder::new(Vec::new(), level);
        encoder.write_all(&buffer)?;
        let compressed = encoder.finish()?;
        let use_raw = compressed.len() + 12 >= BLOCK_SIZE;
        indexes[sector] = ((position >> align) as u32) | if use_raw { RAW_FLAG } else { 0 };

        out.seek(std::io::SeekFrom::Start(position))?;
        if use_raw {
            out.write_all(&buffer)?;
            position += BLOCK_SIZE as u64;
        } else {
            out.write_all(&compressed)?;
            position += compressed.len() as u64;
        }

        let progress = base + span * ((sector + 1) as f64 / sector_count.max(1) as f64);
        emit_progress(
            app,
            job_id,
            progress.clamp(0.0, 0.99),
            format!(
                "Comprimiendo CSO {}%",
                ((sector + 1) * 100 / sector_count.max(1))
            ),
        )?;
    }
    indexes[index_len - 1] = (position >> align) as u32;

    out.seek(std::io::SeekFrom::Start(0))?;
    out.write_all(b"CISO")?;
    out.write_all(&24_u32.to_le_bytes())?;
    out.write_all(&input_len.to_le_bytes())?;
    out.write_all(&(BLOCK_SIZE as u32).to_le_bytes())?;
    out.write_all(&[1, align, 0, 0])?;
    for entry in indexes {
        out.write_all(&entry.to_le_bytes())?;
    }
    out.flush()?;
    Ok(())
}

fn read_cso_v1(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
) -> Result<()> {
    use flate2::read::{DeflateDecoder, ZlibDecoder};

    const RAW_FLAG: u32 = 0x8000_0000;
    const OFFSET_MASK: u32 = 0x7fff_ffff;

    let mut input = fs::File::open(source)?;
    let mut header = [0_u8; 24];
    input.read_exact(&mut header)?;
    if &header[0..4] != b"CISO" {
        return Err(anyhow!(
            "No parece un fichero CSO valido: {}",
            source.display()
        ));
    }
    let total_size = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let block_size = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let version = header[20];
    let align = header[21];
    if version > 1 {
        return Err(anyhow!(
            "CSO v{} no esta soportado nativamente todavia. Dogu escribe y restaura CSO v1/DEFLATE.",
            version
        ));
    }
    if block_size == 0 {
        return Err(anyhow!("CSO con tamano de bloque invalido"));
    }
    let sector_count = ((total_size + block_size as u64 - 1) / block_size as u64) as usize;
    let mut indexes = vec![0_u32; sector_count + 1];
    for entry in &mut indexes {
        let mut raw = [0_u8; 4];
        input.read_exact(&mut raw)?;
        *entry = u32::from_le_bytes(raw);
    }

    let mut output_file = std::io::BufWriter::new(fs::File::create(output)?);
    for sector in 0..sector_count {
        let current = indexes[sector];
        let next = indexes[sector + 1];
        let offset = ((current & OFFSET_MASK) as u64) << align;
        let next_offset = ((next & OFFSET_MASK) as u64) << align;
        let data_len = next_offset
            .checked_sub(offset)
            .ok_or_else(|| anyhow!("Indice CSO corrupto"))? as usize;
        let remaining = total_size.saturating_sub((sector as u64) * block_size as u64);
        let expected_len = remaining.min(block_size as u64) as usize;

        input.seek(std::io::SeekFrom::Start(offset))?;
        let mut chunk = vec![0_u8; data_len];
        input.read_exact(&mut chunk)?;
        if (current & RAW_FLAG) != 0 {
            output_file.write_all(&chunk[..expected_len.min(chunk.len())])?;
        } else {
            // Standard CSO blocks are raw DEFLATE; try that first and fall back to
            // zlib-wrapped blocks for tolerance with any non-standard producer.
            let mut decoded = Vec::with_capacity(expected_len);
            let deflate_result = DeflateDecoder::new(chunk.as_slice()).read_to_end(&mut decoded);
            if deflate_result.is_err() || decoded.len() < expected_len {
                decoded.clear();
                ZlibDecoder::new(chunk.as_slice()).read_to_end(&mut decoded)?;
            }
            if decoded.len() < expected_len {
                return Err(anyhow!("Bloque CSO truncado al descomprimir"));
            }
            output_file.write_all(&decoded[..expected_len])?;
        }

        let progress = base + span * ((sector + 1) as f64 / sector_count.max(1) as f64);
        emit_progress(
            app,
            job_id,
            progress.clamp(0.0, 0.99),
            format!(
                "Restaurando CSO {}%",
                ((sector + 1) * 100 / sector_count.max(1))
            ),
        )?;
    }
    output_file.flush()?;
    Ok(())
}

fn write_xiso(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output: &Path,
) -> Result<()> {
    use xdvdfs::write::{self, img::ProgressInfo};

    let image = fs::File::options()
        .write(true)
        .truncate(true)
        .create(true)
        .open(output)?;
    let mut image = std::io::BufWriter::with_capacity(1024 * 1024, image);
    let mut total_entries: usize = 1;
    let mut done_entries: usize = 0;
    let mut progress = |info| match info {
        ProgressInfo::FileCount(count) => total_entries += count,
        ProgressInfo::DirCount(count) => total_entries += count,
        ProgressInfo::DirAdded(path, _) | ProgressInfo::FileAdded(path, _) => {
            done_entries += 1;
            let _ = emit_progress(
                app,
                job_id,
                (base + span * (done_entries as f64 / total_entries.max(1) as f64))
                    .clamp(0.0, 0.99),
                format!("Empaquetando XISO {path}"),
            );
        }
        ProgressInfo::FinishedPacking => {
            let _ = emit_progress(
                app,
                job_id,
                (base + span).min(0.99),
                "XISO escrito".to_string(),
            );
        }
        _ => {}
    };

    if source.is_dir() {
        let mut source_fs = write::fs::StdFilesystem::create(source);
        write::img::create_xdvdfs_image(&mut source_fs, &mut image, &mut progress)?;
    } else {
        let file = fs::File::options().read(true).open(source)?;
        let reader = std::io::BufReader::new(file);
        let source_image = xdvdfs::blockdev::OffsetWrapper::new(reader)
            .map_err(|_| anyhow!("La ISO no contiene un volumen XDVDFS/Xbox reconocible"))?;
        let mut source_fs = write::fs::XDVDFSFilesystem::new(source_image)
            .ok_or_else(|| anyhow!("No se pudo abrir el sistema de ficheros XDVDFS"))?;
        write::img::create_xdvdfs_image(&mut source_fs, &mut image, &mut progress)?;
    }
    image.flush()?;
    Ok(())
}

fn read_xiso(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    source: &Path,
    output_dir: &Path,
) -> Result<()> {
    let file = fs::File::options().read(true).open(source)?;
    let reader = std::io::BufReader::new(file);
    let mut image = xdvdfs::blockdev::OffsetWrapper::new(reader)
        .map_err(|_| anyhow!("La ISO no contiene un volumen XDVDFS/Xbox reconocible"))?;
    let volume = xdvdfs::read::read_volume(&mut image)?;
    let tree = volume.root_table.file_tree(&mut image)?;
    let total = tree.len().max(1);

    for (index, (dir, dirent)) in tree.iter().enumerate() {
        let dir = dir.trim_start_matches('/');
        let dirname = output_dir.join(dir);
        let file_name = dirent.name_str::<std::io::Error>()?;
        let file_path = dirname.join(&*file_name);
        fs::create_dir_all(&dirname)?;
        if dirent.node.dirent.is_directory() {
            fs::create_dir_all(&file_path)?;
        } else if !dirent.node.dirent.is_empty() {
            let mut out = fs::File::options()
                .write(true)
                .truncate(true)
                .create(true)
                .open(&file_path)?;
            dirent.node.dirent.seek_to(&mut image)?;
            let data = image.get_ref().get_ref().try_clone();
            match data {
                Ok(data) => {
                    let data = data.take(dirent.node.dirent.data.size as u64);
                    let mut data = std::io::BufReader::new(data);
                    io::copy(&mut data, &mut out)?;
                }
                Err(_) => {
                    let data = dirent.node.dirent.read_data_all(&mut image)?;
                    out.write_all(&data)?;
                }
            }
        } else {
            let _ = fs::File::create(&file_path)?;
        }

        emit_progress(
            app,
            job_id,
            (base + span * ((index + 1) as f64 / total as f64)).clamp(0.0, 0.99),
            format!("Extrayendo XISO {}", file_path.display()),
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
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        total_bytes = total_bytes.saturating_add(archive.by_index(index)?.size());
    }
    let mut done = 0u64;
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
        copy_with_byte_progress(&mut item, &mut output, app, job_id, base, span, total_bytes, &mut done, &format!("Extrayendo {}", out_path.display()))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn copy_with_byte_progress<R: io::Read, W: io::Write>(reader: &mut R, writer: &mut W, app: &AppHandle, job_id: &str, base: f64, span: f64, total: u64, done: &mut u64, message: &str) -> Result<()> {
    let mut buffer = vec![0u8; 256 * 1024];
    let mut last = std::time::Instant::now();
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 { break; }
        writer.write_all(&buffer[..n])?;
        *done = done.saturating_add(n as u64);
        if last.elapsed() >= std::time::Duration::from_millis(150) {
            let _ = emit_progress(app, job_id, (base + span * (*done as f64 / total.max(1) as f64)).clamp(0.0, 1.0), message.to_string());
            last = std::time::Instant::now();
        }
    }
    let _ = emit_progress(app, job_id, (base + span * (*done as f64 / total.max(1) as f64)).clamp(0.0, 1.0), message.to_string());
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
    for (index, entry) in walker
        .into_iter()
        .filter_map(|entry| entry.ok())
        .enumerate()
    {
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
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        #[cfg(unix)]
        {
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            std::os::unix::fs::symlink(fs::read_link(source)?, target)?;
            return Ok(());
        }
        #[cfg(not(unix))]
        return Err(anyhow!("No se puede copiar un enlace simbolico de forma segura: {}", source.display()));
    }
    if metadata.is_dir() {
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

fn copy_or_move_one_staged(source: &Path, target: &Path, is_move: bool, overwrite: bool) -> Result<()> {
    if source == target {
        return Err(anyhow!("El origen y el destino son el mismo elemento."));
    }
    let parent = target.parent().ok_or_else(|| anyhow!("Destino invalido: {}", target.display()))?;
    let stage = tempfile::Builder::new().prefix(".dogu-transfer-").tempdir_in(parent)?;
    let payload = stage.path().join("payload");
    let backup = stage.path().join("original");
    let exists = fs::symlink_metadata(target).is_ok();
    if exists && !overwrite { return Err(anyhow!("El destino ya existe: {}", target.display())); }
    if is_move {
        // A same-volume rename avoids copying large files. If it is unavailable,
        // restore the old destination and fall back to a staged copy.
        if exists { fs::rename(target, &backup)?; }
        if fs::rename(source, target).is_ok() { return Ok(()); }
        if exists {
            if let Err(error) = fs::rename(&backup, target) {
                let saved = stage.keep();
                return Err(anyhow!("[REQUIRES_REVIEW] No se pudo recuperar el destino '{}': {error}. Se conserva en '{}'.", target.display(), saved.join("original").display()));
            }
        }
    }
    // The fallback also covers cross-volume moves and leaves the original intact.
    copy_path(source, &payload)?;
    if exists {
        fs::rename(target, &backup)?;
    }
    if let Err(error) = fs::rename(&payload, target) {
        if exists && fs::rename(&backup, target).is_err() {
            let saved = stage.keep();
            return Err(anyhow!("[REQUIRES_REVIEW] No se pudo publicar '{}': {error}. El destino anterior se conserva en '{}'.", target.display(), saved.join("original").display()));
        }
        return Err(anyhow!("No se pudo publicar '{}': {error}", target.display()));
    }
    if is_move {
        remove_single_path(source).map_err(|error| anyhow!("[REQUIRES_REVIEW] El destino '{}' se publico, pero no se pudo eliminar el origen '{}': {error:#}. Revisa ambas rutas.", target.display(), source.display()))?;
    }
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
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
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

/// The XDVDFS volume descriptor signature, present at the start and end of the
/// 2048-byte volume sector (sector 32 of the game partition).
const XDVDFS_VOLUME_MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";
/// Game-partition offsets probed to locate the XDVDFS volume. The first entry
/// (0) corresponds to a trimmed XISO; the rest are the XGD1/XGD2/XGD3 offsets
/// used by full Xbox disc dumps (redump). The volume descriptor sits at
/// `offset + 0x10000` (sector 32 * 2048 bytes).
const XDVDFS_PARTITION_OFFSETS: [u64; 4] = [0, 0x1830_0000, 0x0FD9_0000, 0x0208_0000];
const XDVDFS_VOLUME_SECTOR: u64 = 0x1_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum XisoKind {
    /// Trimmed XISO — game partition at offset 0. Playable in xemu as-is and can
    /// be unpacked into a file tree.
    Trimmed,
    /// Full Xbox disc dump (redump / XGD1-3). Can be trimmed into a playable XISO.
    Redump,
}

/// Probe an image for the XDVDFS volume signature by content (authoritative).
///
/// `Ok(Some(Trimmed))` / `Ok(Some(Redump))` when an Xbox volume is found,
/// `Ok(None)` when the file opened cleanly but is not an Xbox disc, and `Err`
/// when the file could not be read (callers may then fall back to naming).
fn xdvdfs_kind(path: &Path) -> io::Result<Option<XisoKind>> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0_u8; XDVDFS_VOLUME_MAGIC.len()];
    for (index, offset) in XDVDFS_PARTITION_OFFSETS.iter().enumerate() {
        if file
            .seek(io::SeekFrom::Start(offset + XDVDFS_VOLUME_SECTOR))
            .is_err()
        {
            continue;
        }
        match file.read_exact(&mut magic) {
            Ok(()) if &magic == XDVDFS_VOLUME_MAGIC => {
                file.seek(io::SeekFrom::Start(offset + XDVDFS_VOLUME_SECTOR + 2048 - magic.len() as u64))?;
                match file.read_exact(&mut magic) {
                    Ok(()) if &magic == XDVDFS_VOLUME_MAGIC => return Ok(Some(if index == 0 {
                        XisoKind::Trimmed
                    } else {
                        XisoKind::Redump
                    })),
                    Ok(()) => {},
                    Err(ref error) if error.kind() == io::ErrorKind::UnexpectedEof => {},
                    Err(error) => return Err(error),
                }
            }
            // Wrong signature, or the probe offset is past EOF for this image:
            // keep probing the remaining offsets.
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {}
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// True when `path` is a GameCube or Wii disc image, detected by the disc magic
/// word (Wii `5D1C9EA3` at 0x18, GameCube `C2339F3D` at 0x1C).
fn is_gc_wii_iso(path: &Path) -> io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut word = [0_u8; 4];
    file.seek(io::SeekFrom::Start(0x18))?;
    match file.read_exact(&mut word) {
        Ok(()) if word == [0x5D, 0x1C, 0x9E, 0xA3] => return Ok(true),
        Ok(()) => {},
        Err(ref error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(error) => return Err(error),
    }
    file.seek(io::SeekFrom::Start(0x1C))?;
    match file.read_exact(&mut word) {
        Ok(()) => Ok(word == [0xC2, 0x33, 0x9F, 0x3D]),
        Err(ref error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

/// Classify local `.iso` selections by content so the UI only offers treatments
/// that are actually valid for each file. Remote paths and non-`.iso` files are
/// skipped (the frontend falls back to the `.xiso` naming convention for those).
pub fn detect_disc_kinds(paths: Vec<PathBuf>) -> Vec<crate::models::DiscKindDto> {
    let mut out = Vec::new();
    for path in paths {
        let path_str = path.to_string_lossy();
        if remote::RemoteManager::is_remote_path(&path_str) {
            continue;
        }
        let ext = path
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "iso" {
            continue;
        }
        let kind = match xdvdfs_kind(&path) {
            Ok(Some(XisoKind::Trimmed)) => "xiso",
            Ok(Some(XisoKind::Redump)) => "redump",
            // Not an Xbox disc — check for a GameCube/Wii image (RVZ candidate).
            Ok(None) => match is_gc_wii_iso(&path) {
                Ok(true) => "gc-wii",
                Ok(false) => "iso",
                Err(_) => continue,
            },
            // Unreadable: leave it out so the frontend uses its naming fallback.
            Err(_) => continue,
        };
        out.push(crate::models::DiscKindDto {
            path: path_str.to_string(),
            kind: kind.to_string(),
        });
    }
    out
}

/// True when `path` is a trimmed XISO. Uses content detection for local files
/// (authoritative) and falls back to the `.xiso` naming convention only for
/// remote paths or files that cannot be read.
fn is_likely_xiso_path(path: &Path) -> bool {
    if !remote::RemoteManager::is_remote_path(&path.to_string_lossy()) {
        match xdvdfs_kind(path) {
            Ok(Some(XisoKind::Trimmed)) => return true,
            Ok(Some(XisoKind::Redump)) | Ok(None) => return false,
            Err(_) => {} // Unreadable — fall through to the naming heuristic.
        }
    }
    let name = path
        .file_name()
        .unwrap_or_else(|| path.as_os_str())
        .to_string_lossy()
        .to_ascii_lowercase();
    name.ends_with(".xiso.iso") || name.ends_with(".xiso")
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
        s.rfind('/')
            .map(|i| s[..=i].trim_end_matches('/').to_string())
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
    if DVD_SOURCE_EXTENSIONS.contains(&ext.as_str()) && !is_likely_xiso_path(path) {
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
            container_dir: path.parent().unwrap_or(path).to_string_lossy().to_string(),
            command: "createcd".to_string(),
            display_extensions: vec![extension],
            required_paths: required_paths
                .into_iter()
                .map(|item| item.to_string_lossy().to_string())
                .collect(),
            missing_files,
        }));
    }

    if DVD_SOURCE_EXTENSIONS.contains(&extension.as_str()) && !is_likely_xiso_path(path) {
        return Ok(Some(ChdSourceDto {
            source_path: path.to_string_lossy().to_string(),
            container_dir: path.parent().unwrap_or(path).to_string_lossy().to_string(),
            command: "createdvd".to_string(),
            display_extensions: vec![extension],
            required_paths: vec![path.to_string_lossy().to_string()],
            missing_files: vec![], // .iso is self-contained
        }));
    }

    if PAIRABLE_BIN_EXTENSIONS.contains(&extension.as_str()) {
        let cue_candidate = path.with_extension("cue");
        if cue_candidate.exists() && parse_cue_references(&cue_candidate)?.iter().any(|reference| track_identity(reference) == track_identity(path)) {
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
    for name in cue_reference_names(&content) {
        references.push(path.parent().unwrap_or(path).join(name));
    }
    Ok(dedupe_paths(references))
}

fn cue_reference_names(content: &str) -> Vec<String> {
    let mut references = Vec::new();
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
            after_file
                .rfind(char::is_whitespace)
                .map(|pos| after_file[..pos].trim())
                .filter(|s| !s.is_empty())
        };
        if let Some(name) = filename {
            if !name.is_empty() {
                references.push(name.to_string());
            }
        }
    }
    references
}

fn parse_gdi_references(path: &Path) -> Result<Vec<PathBuf>> {
    let mut references = vec![path.to_path_buf()];
    let content = String::from_utf8_lossy(&fs::read(path)?).into_owned();
    for name in gdi_reference_names(&content) {
        references.push(path.parent().unwrap_or(path).join(name));
    }
    Ok(dedupe_paths(references))
}

fn gdi_reference_names(content: &str) -> Vec<String> {
    let mut references = Vec::new();
    for line in content.lines().skip(1) {
        let mut remainder = line.trim();
        for _ in 0..4 {
            let Some(end) = remainder.find(char::is_whitespace) else { remainder = ""; break; };
            remainder = remainder[end..].trim_start();
        }
        let Some(end) = remainder.rfind(char::is_whitespace) else { continue; };
        let name = remainder[..end].trim().trim_matches('"');
        if !name.is_empty() { references.push(name.to_string()); }
    }
    references
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
            if references.iter().any(|item| track_identity(item) == track_identity(bin_path)) {
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
        if let Some(name) = options
            .custom_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            // Strip .chd suffix if the user accidentally typed it
            name.trim_end_matches(".chd")
                .trim_end_matches(".CHD")
                .to_string()
        } else if options.name_as_container {
            container_dir
                .file_name()
                .unwrap_or_else(|| container_dir.as_os_str())
                .to_string_lossy()
                .to_string()
        } else {
            source_path
                .file_stem()
                .unwrap_or_else(|| source_path.as_os_str())
                .to_string_lossy()
                .to_string()
        }
    } else if options.name_as_container {
        container_dir
            .file_name()
            .unwrap_or_else(|| container_dir.as_os_str())
            .to_string_lossy()
            .to_string()
    } else {
        source_path
            .file_stem()
            .unwrap_or_else(|| source_path.as_os_str())
            .to_string_lossy()
            .to_string()
    };

    // Explicit local destination overrides all other placement logic
    if options.destination_mode.as_deref() == Some("custom") {
        if let Some(ref dest) = options.destination_path {
            return PathBuf::from(dest).join(format!("{raw_stem}.chd"));
        }
    }

    // Normally the CHD lands in the folder holding the source disc image
    // (its container). "Deposit to parent" places it one level up — the parent
    // of that container — which is deterministic and matches the ghost.
    let mut output_dir = source_path.parent().unwrap_or(source_path).to_path_buf();
    if options.deposit_to_parent {
        output_dir = output_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(output_dir);
    }
    let _ = selected_root; // no longer used for placement
    output_dir.join(format!("{raw_stem}.chd"))
}

/// Which disc geometry chdman recorded inside a CHD. Determines the correct
/// native output format when restoring, so the user never has to pick one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChdDiscKind {
    /// Dreamcast GD-ROM — metadata tag `CHGD` (or legacy `CHGT`). Restores to `.gdi`.
    GdRom,
    /// Standard CD — metadata tag `CHT2` (or legacy `CHTR`). Restores to `.cue`/`.bin`.
    Cd,
    /// No CD-track metadata (DVD and similar). Restores to `.iso`.
    Other,
}

/// Reads the metadata tags chdman stored in a CHD header to decide the source
/// disc format. `chdman info` only parses the header (no hunk decompression),
/// so this is fast and safe to run before every restore. chdman writes a
/// distinct tag per geometry, and it does NOT enforce the output format on
/// extraction (it will happily emit a semantically-wrong `.gdi` for a plain CD),
/// so probing the tag is the only reliable way to choose the right format.
fn probe_chd_disc_kind(chdman: &Path, chd: &Path) -> ChdDiscKind {
    let mut command = Command::new(chdman);
    let text = match hide_console_window(&mut command)
        .arg("info")
        .arg("-i")
        .arg(chd)
        .output()
    {
        Ok(out) => {
            let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&out.stderr));
            s
        }
        // If the probe can't run, fall back to the historical CD-first behaviour.
        Err(_) => return ChdDiscKind::Cd,
    };
    if text.contains("Tag='CHGD'") || text.contains("Tag='CHGT'") {
        ChdDiscKind::GdRom
    } else if text.contains("Tag='CHT2'") || text.contains("Tag='CHTR'") {
        ChdDiscKind::Cd
    } else {
        ChdDiscKind::Other
    }
}

/// The disc-image format a restore actually produced.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RestoreFormat {
    Gdi,
    Cd,
    Dvd,
}

impl RestoreFormat {
    fn label(self) -> &'static str {
        match self {
            RestoreFormat::Gdi => "GDI",
            RestoreFormat::Cd => "CD",
            RestoreFormat::Dvd => "DVD",
        }
    }
}

/// Restores a CHD into `dir` under `stem`, choosing the output format from the
/// disc geometry chdman recorded (GD-ROM → `.gdi`, CD → `.cue`/`.bin`, otherwise
/// `.iso`) so the user doesn't have to know or decide in advance. The remaining
/// formats stay as ordered fallbacks in case an unusual CHD fails its preferred
/// path; partial output is cleaned between attempts. Returns the format that won.
#[allow(clippy::too_many_arguments)]
fn restore_chd_content(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    chdman: &Path,
    chd: &Path,
    dir: &Path,
    stem: &str,
    overwrite: bool,
    split_bin: bool,
) -> Result<RestoreFormat> {
    // chdman can't take non-ASCII paths as arguments (see run_chdman_convert). Run
    // it in the CHD's own folder with a relative `-i` and relative, ASCII output
    // names, so an accented *folder* is handled by the (Unicode-correct) cwd. The
    // output lands next to the CHD and is moved to `dir` when that differs. chdman
    // writes track references relatively, so moving the set together keeps them
    // valid — no rewriting. Accented *file names* are handled by the de-accent guard.
    let work_dir = chd
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| dir.to_path_buf());
    let chd_name = chd
        .file_name()
        .ok_or_else(|| anyhow!("Ruta CHD invalida: {}", chd.display()))?
        .to_owned();
    let order: &[RestoreFormat] = match probe_chd_disc_kind(chdman, chd) {
        ChdDiscKind::GdRom => &[RestoreFormat::Gdi, RestoreFormat::Cd, RestoreFormat::Dvd],
        ChdDiscKind::Cd => &[RestoreFormat::Cd, RestoreFormat::Dvd],
        ChdDiscKind::Other => &[RestoreFormat::Dvd, RestoreFormat::Cd],
    };

    let mut errors: Vec<String> = Vec::new();
    for fmt in order {
        // Every format attempt owns an isolated directory. Never clean by
        // filename prefix in a user's folder: unrelated files may share it.
        let run = retry_transient_stage(app, job_id, || {
        let attempt_dir = tempfile::Builder::new().prefix("dogu-restore-").tempdir_in(&work_dir)?;
        let input_from_attempt = Path::new("..").join(&chd_name);
        let mut command = Command::new(chdman);
        match fmt {
            RestoreFormat::Gdi => {
                command
                    .arg("extractcd")
                    .arg("-i")
                    .arg(&input_from_attempt)
                    .arg("-o")
                    .arg(format!("{stem}.gdi"));
            }
            RestoreFormat::Cd => {
                command
                    .arg("extractcd")
                    .arg("-i")
                    .arg(&input_from_attempt)
                    .arg("-o")
                    .arg(format!("{stem}.cue"));
                if split_bin {
                    command.arg("-sb");
                } else {
                    command.arg("-ob").arg(format!("{stem}.bin"));
                }
            }
            RestoreFormat::Dvd => {
                command
                    .arg("extractdvd")
                    .arg("-i")
                    .arg(&input_from_attempt)
                    .arg("-o")
                    .arg(format!("{stem}.iso"));
            }
        }
        if overwrite {
            command.arg("-f");
        }
        command.current_dir(attempt_dir.path());

        run_command_streaming(app, job_id, base, span, command)
            .with_context(|| format!("chdman ({}) no pudo completar la restauracion", fmt.label()))?;
        Ok(attempt_dir)
        });

        match run {
            Ok(attempt_dir) => {
                let produced = collect_restore_outputs(attempt_dir.path(), stem, chd);
                if produced.is_empty() {
                    errors.push(format!("{}: no se produjeron archivos", fmt.label()));
                    continue;
                }
                fs::create_dir_all(dir)?;
                // Check all destinations before changing any of them.
                for source in &produced {
                    let name = source.file_name().ok_or_else(|| anyhow!("Salida CHD invalida"))?;
                    let target = dir.join(name);
                    if target.exists() && !overwrite {
                        return Err(anyhow!("El destino ya existe: {}", target.display()));
                    }
                }
                publish_file_set(&produced, dir, overwrite)?;
                return Ok(*fmt);
            }
            Err(e) => errors.push(format!("{}: {e}", fmt.label())),
        }
    }
    Err(anyhow!(
        "No se pudo recuperar {} en ningun formato.\n{}",
        chd.display(),
        errors.join("\n")
    ))
}

/// Prepare all files on the destination filesystem before replacing anything.
/// On failure, restore prior files; preserve the backup if rollback fails.
fn publish_file_set(sources: &[PathBuf], destination: &Path, overwrite: bool) -> Result<()> {
    let staging = tempfile::Builder::new().prefix("dogu-publish-").tempdir_in(destination)?;
    let backup = tempfile::Builder::new().prefix("dogu-backup-").tempdir_in(destination)?;
    let mut names = Vec::new();
    for source in sources {
        let name = source.file_name().ok_or_else(|| anyhow!("Nombre de salida invalido"))?.to_owned();
        let target = destination.join(&name);
        if target.exists() && !overwrite { return Err(anyhow!("El destino ya existe: {}", target.display())); }
        fs::copy(source, staging.path().join(&name))?;
        names.push(name);
    }
    let mut saved = Vec::new();
    let mut published = Vec::new();
    let result = (|| -> Result<()> {
        for name in &names {
            let target = destination.join(name);
            if target.exists() {
                if !overwrite { return Err(anyhow!("El destino ya existe: {}", target.display())); }
                fs::rename(&target, backup.path().join(name))?;
                saved.push(name.clone());
            }
            commit_staged_file(&staging.path().join(name), &target, false)?;
            published.push(name.clone());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for name in published.iter().rev() {
            if let Err(e) = fs::remove_file(destination.join(name)) { failures.push(e.to_string()); }
        }
        for name in saved.iter().rev() {
            if let Err(e) = fs::rename(backup.path().join(name), destination.join(name)) { failures.push(e.to_string()); }
        }
        if !failures.is_empty() {
            let saved_path = backup.keep();
            return Err(anyhow!("Error al publicar: {error:#}. No se pudo completar la recuperacion: {}. Copia conservada en {}", failures.join("; "), saved_path.display()));
        }
        return Err(error);
    }
    Ok(())
}

/// Lists the files a chdman restore produced under `stem` in `dir` (the `.gdi`/
/// `.cue`/`.iso` index plus its track binaries), excluding the source CHD. Used to
/// move the set to a different destination folder.
fn collect_restore_outputs(dir: &Path, stem: &str, chd: &Path) -> Vec<PathBuf> {
    const OUTPUT_EXTS: &[&str] = &["gdi", "cue", "bin", "raw", "iso", "toc"];
    let stem_lower = stem.to_lowercase();
    let mut produced = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || same_path(&path, chd) {
                continue;
            }
            let ext_ok = path
                .extension()
                .and_then(OsStr::to_str)
                .map(|e| OUTPUT_EXTS.contains(&e.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            let name_lower = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if ext_ok && name_lower.starts_with(&stem_lower) {
                produced.push(path);
            }
        }
    }
    produced
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        let key = path_identity(&path);
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

fn same_path(left: &Path, right: &Path) -> bool {
    path_identity(left) == path_identity(right)
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
    let mut child = hide_console_window(&mut command).spawn()?;

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
    let err_captured = stderr_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();

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

/// Preserve the error chain: filesystem errors are often wrapped with operation context.
pub fn format_operation_error(error: &anyhow::Error) -> String {
    let detail = format!("{error:#}");
    let too_long = error.chain().any(|cause| {
        cause.downcast_ref::<std::io::Error>().is_some_and(|io| matches!(io.raw_os_error(), Some(206) | Some(111)))
    });
    if cfg!(windows) && too_long { format!("[PATH_TOO_LONG] {detail}") } else { detail }
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
            let display_label = if label.is_empty() {
                "Local Disk".to_string()
            } else {
                label.clone()
            };
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
fn run_remote_file_cycle_with_policy<F>(
    app: &AppHandle,
    job_id: &str,
    remote_manager: &remote::RemoteManager,
    pause_registry: &pause::PauseRegistry,
    remote_dest_dir: &str,
    policy: &RemoteTransferPolicy,
    source_name: &str,
    overwrite: bool,
    progress_range: Option<(f64, f64)>,
    produce: F,
) -> Result<()>
where
    F: Fn(&Path) -> Result<Vec<PathBuf>>,
{
    use tauri::Manager;
    let temp_root = app
        .path()
        .app_data_dir()
        .context("No se pudo obtener el directorio de datos de la aplicacion")?
        .join("temp");
    fs::create_dir_all(&temp_root)?;
    let (_guard, output_files) = retry_transient_stage(app, job_id, || {
        let temp_dir = tempfile::Builder::new().prefix("remote-cycle-").tempdir_in(&temp_root)?;
        let files = produce(temp_dir.path())?;
        Ok((temp_dir, files))
    })?;

    // Ensure the remote destination directory exists
    remote_manager
        .ensure_remote_dir(remote_dest_dir)
        .map_err(|e| {
            anyhow!(
                "No se pudo verificar el directorio remoto '{}': {}",
                remote_dest_dir,
                e
            )
        })?;

    let upload_progress = progress_range.map(|(base, span)| {
        let _ = emit_progress(app, job_id, base, "Subiendo archivos al destino remoto".to_string());
        let bytes = output_files.iter().flat_map(|path| WalkDir::new(path).into_iter().filter_map(|entry| entry.ok()))
            .filter(|entry| entry.file_type().is_file()).filter_map(|entry| entry.metadata().ok().map(|m| m.len()))
            .fold(0u64, u64::saturating_add);
        remote::ByteTransferProgress::ranged(app, job_id, bytes, base, span)
    });
    for file in &output_files {
        let file_name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "fichero".to_string());

        let _ = emit_log(app, job_id, format!("Subiendo {}...", file_name));
        'retry: loop {
            let upload = if let Some(progress) = &upload_progress { remote_manager.upload_staged_with_progress(app, job_id, file, remote_dest_dir, overwrite, progress) }
                else if overwrite { remote_manager.upload_staged_to_remote(app, job_id, file, remote_dest_dir) }
                else { remote_manager.upload_staged_no_overwrite(app, job_id, file, remote_dest_dir) };
            match upload {
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
                        return Err(anyhow!("No se publico {file_name}: subida omitida ({upload_err})"));
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
                                return Err(anyhow!("No se publico {file_name}: subida omitida por el usuario ({upload_err})"));
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
    let existing = path.ancestors().find(|ancestor| ancestor.exists()).unwrap_or(path);
    let path = fs::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    let disks = Disks::new_with_refreshed_list();
    let best = disks
        .iter()
        .filter_map(|d| {
            let mount = d.mount_point();
            if path.starts_with(mount) {
                Some((mount.components().count(), d.available_space()))
            } else {
                None
            }
        })
        .max_by_key(|(prefix_len, _)| *prefix_len);
    best.map(|(_, free)| free).ok_or_else(|| {
        anyhow!(
            "No se pudo determinar el espacio libre en: {}",
            path.display()
        )
    })
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
    let mut command = Command::new("rar");
    if hide_console_window(&mut command).output().is_ok() {
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

    let any_source_is_remote = sources
        .iter()
        .any(|p| remote::RemoteManager::is_remote_path(&p.to_string_lossy()));

    // Download remote sources to a temp dir so compression tools can access them.
    // TempGuard ensures cleanup even on early return via `?`.
    let (effective_sources, _guard) = if any_source_is_remote {
        let rm = remote_manager
            .as_ref()
            .ok_or_else(|| anyhow!("Se necesita el gestor remoto para descargar los archivos"))?;
        let dl_dir = temp_download_dir(app, job_id)?;
        let mut local_sources = Vec::with_capacity(sources.len());
        for (index, src) in sources.iter().enumerate() {
            if remote::RemoteManager::is_remote_path(&src.to_string_lossy()) {
                let src_str = src.to_string_lossy();
                emit_log(app, job_id, format!("[Descarga] {}", src_str))?;
                emit_progress(app, job_id, 0.30 * index as f64 / sources.len().max(1) as f64, format!("Descargando {src_str}"))?;
                let (local, bytes) = rm
                    .download_file_to_dir_with_progress(&src_str, &dl_dir, Some((app, job_id, 0.30 * index as f64 / sources.len().max(1) as f64, 0.30 / sources.len().max(1) as f64)))
                    .map_err(|e| anyhow!("Fallo al descargar '{}': {e}", src_str))?;
                emit_log(
                    app,
                    job_id,
                    format!("[Descarga] OK: {} bytes → {}", bytes, local.display()),
                )?;
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

    let use_remote =
        remote_manager.is_some() && pause_registry.is_some() && effective_remote_dest.is_some();

    if use_remote {
        let rm = Arc::clone(remote_manager.as_ref().unwrap());
        let pr = pause_registry.as_ref().unwrap();
        let remote_dest = effective_remote_dest.as_deref().unwrap();
        let policy = options
            .remote_transfer
            .clone()
            .unwrap_or_else(|| RemoteTransferPolicy {
                on_error: "abort".to_string(),
            });
        let effective_sources_c = effective_sources.clone();
        let options_c = options.clone();
        let archive_filename_c = if !options.overwrite && options.rename_on_conflict {
            unique_remote_output_name(&rm, remote_dest, &archive_filename)?
        } else { archive_filename.clone() };
        run_remote_file_cycle_with_policy(
            app,
            job_id,
            &rm,
            pr,
            remote_dest,
            &policy,
            &archive_filename,
            options.overwrite,
            Some((0.65, 0.34)),
            |temp_dir| {
                let archive_path = temp_dir.join(&archive_filename_c);
                do_compress(
                    app,
                    job_id,
                    if any_source_is_remote { 0.30 } else { 0.0 },
                    if any_source_is_remote { 0.35 } else { 0.65 },
                    &effective_sources_c,
                    &archive_path,
                    &options_c,
                )?;
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
            app,
            job_id,
            format!(
                "Comprimiendo {} elemento(s) → {}",
                effective_sources.len(),
                archive_path.display()
            ),
        )?;
        fs::create_dir_all(&dest_folder)?;
        // Build alongside the destination and publish only a complete archive.
        // A failed compressor cannot truncate an existing user file.
        let stage_parent = staging_parent_outside_sources(&dest_folder, &effective_sources)?;
        let staging = tempfile::Builder::new().prefix("dogu-compress-").tempdir_in(stage_parent)?;
        let staged_path = staging.path().join(&archive_filename);
        retry_transient_stage(app, job_id, || {
            if staged_path.exists() { fs::remove_file(&staged_path)?; }
            do_compress(app, job_id, if any_source_is_remote { 0.30 } else { 0.0 }, if any_source_is_remote { 0.69 } else { 0.99 }, &effective_sources, &staged_path, &options)
        })?;
        commit_staged_file(&staged_path, &archive_path, options.overwrite)?;
        emit_log(
            app,
            job_id,
            format!("Archivo creado: {}", archive_path.display()),
        )?;
    }

    emit_progress(
        app,
        job_id,
        0.99,
        format!("Comprimiendo {}", archive_filename),
    )?;

    if options.delete_originals {
        for source in &sources {
            if remote::RemoteManager::is_remote_path(&source.to_string_lossy()) {
                if let Some(ref rm) = remote_manager {
                    rm.delete_entry(app, job_id, &source.to_string_lossy())?;
                    emit_log(
                        app,
                        job_id,
                        format!("Original remoto eliminado: {}", source.display()),
                    )?;
                }
            } else {
                remove_single_path(source)?;
                emit_log(
                    app,
                    job_id,
                    format!("Original eliminado: {}", source.display()),
                )?;
            }
        }
    }

    emit_progress(app, job_id, 1.0, "Compresión completada.".to_string())?;
    Ok(())
}

/// Only retry errors that are explicitly transient and still retain their IO
/// type. Permanent, unknown and process-exit errors require user review.
fn is_transient_io(error: &anyhow::Error) -> bool {
    error.chain().filter_map(|cause| cause.downcast_ref::<io::Error>()).any(|error| {
        matches!(error.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe)
    })
}

fn retry_transient<T>(mut attempt: impl FnMut() -> Result<T>, mut wait: impl FnMut(usize, &anyhow::Error) -> Result<()>) -> Result<T> {
    for retries in 0..=2 {
        match attempt() {
            Ok(value) => return Ok(value),
            Err(error) if retries < 2 && is_transient_io(&error) => {
                wait(retries + 1, &error)?;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

fn retry_transient_stage<T>(app: &AppHandle, job_id: &str, attempt: impl FnMut() -> Result<T>) -> Result<T> {
    retry_transient(attempt, |retry, error| {
        emit_log(app, job_id, format!("Error temporal; reintento automatico {} de 2: {error:#}", retry))?;
        std::thread::sleep(std::time::Duration::from_millis(500 * retry as u64));
        Ok(())
    })
}

/// Publish a staged file in the same filesystem without exposing partial output.
fn commit_staged_file(staged: &Path, target: &Path, overwrite: bool) -> Result<()> {
    let staged = tempfile::TempPath::try_from_path(staged.to_path_buf())?;
    if overwrite {
        staged.persist(target).map_err(|error| anyhow!("No se pudo publicar {}: {}", target.display(), error.error))?;
    } else {
        staged.persist_noclobber(target).map_err(|error| anyhow!("No se pudo publicar {}: {}", target.display(), error.error))?;
    }
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
        "7z" => compress_via_7z(
            app,
            job_id,
            base,
            span,
            sources,
            archive_path,
            options.compression_level,
        ),
        "rar" => {
            let rar = find_rar_binary()
                .ok_or_else(|| anyhow!("RAR no está disponible en este sistema"))?;
            compress_via_rar(app, job_id, base, span, &rar, sources, archive_path, options.compression_level)
        }
        _ => compress_to_zip(
            app,
            job_id,
            base,
            span,
            sources,
            archive_path,
            options.compression_level,
        ),
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

    let total: u64 = sources.iter().flat_map(|source| WalkDir::new(source).into_iter().filter_map(|e| e.ok()))
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok().map(|m| m.len()))
        .fold(0u64, u64::saturating_add).max(1);
    let mut done: u64 = 0;

    for source in sources {
        let src_base = source.parent().unwrap_or(source);
        add_path_to_zip(
            app, job_id, base, span, total, &mut done, &mut zip, source, src_base, options,
        )?;
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
        copy_with_byte_progress(&mut f, zip, app, job_id, base, span, total, done, &format!("Comprimiendo {name}"))?;
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .min_depth(0)
            .into_iter()
            .filter_map(|e| e.ok())
        {
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
                copy_with_byte_progress(&mut f, zip, app, job_id, base, span, total, done, &format!("Comprimiendo {name}"))?;
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
        .context("7zz devolvió un error al comprimir")
}

fn compress_via_rar(
    app: &AppHandle,
    job_id: &str,
    base: f64,
    span: f64,
    rar_binary: &Path,
    sources: &[PathBuf],
    archive_path: &Path,
    level: u8,
) -> Result<()> {
    // RAR levels: 0=store, 1=fastest, 2=fast, 3=normal, 4=good, 5=best
    let rar_level = match level {
        0 => 0u8,
        1..=2 => 1,
        3..=4 => 2,
        5..=6 => 3,
        7..=8 => 4,
        _ => 5,
    };
    let mut command = Command::new(rar_binary);
    command
        .arg("a")
        .arg(format!("-m{rar_level}"))
        .arg("-y")
        .arg(archive_path)
        .args(sources);
    run_command_streaming(app, job_id, base, span, command).context("rar devolvió un error al comprimir")
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_gdi_and_cue_tracks_are_not_reported_as_orphans() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let gdi = root.join("game.gdi");
        let cue = root.join("other.cue");
        let bin = root.join("track 01.bin");
        let raw = root.join("track02.raw");
        let cue_bin = root.join("other.bin");
        let orphan_bin = root.join("loose.bin");
        let orphan_raw = root.join("loose.raw");
        fs::write(&gdi, "2\n1 0 4 2352 \"track 01.bin\" 0\n2 45000 0 2352 track02.raw 0\n").unwrap();
        fs::write(&cue, "FILE \"other.bin\" BINARY\n").unwrap();
        for track in [&bin, &raw, &cue_bin, &orphan_bin, &orphan_raw] { fs::write(track, b"track").unwrap(); }
        for selection in [vec![bin.clone(), raw.clone(), cue_bin.clone(), orphan_bin.clone(), orphan_raw.clone(), gdi.clone(), cue.clone()], vec![root.to_path_buf(), gdi.clone(), bin.clone()]] {
            let analysis = super::scan_selection(&selection, usize::MAX, None).unwrap();
            assert_eq!(analysis.chd_sources.len(), 2);
            assert!(analysis.chd_sources.iter().find(|s| s.source_path.ends_with("game.gdi")).unwrap().missing_files.is_empty());
            let orphans: Vec<_> = analysis.orphan_bins.iter().map(|s| std::path::Path::new(s).file_name().unwrap().to_string_lossy().to_string()).collect();
            assert_eq!(orphans.len(), 2);
            assert!(orphans.contains(&"loose.bin".to_string()));
            assert!(orphans.contains(&"loose.raw".to_string()));
        }
    }

    #[test]
    fn gdi_track_names_support_quotes_and_spaces() {
        let names = super::gdi_reference_names("2\n1 0 4 2352 \"Track 01.bin\" 0\n2 45000 0 2352 Track 02.raw 0\n");
        assert_eq!(names, vec!["Track 01.bin", "Track 02.raw"]);
    }

    #[test]
    fn remote_track_references_stay_in_the_same_session() {
        assert_eq!(super::remote_track_reference("remote://nas/games/disc", "./tracks\\Track 01.bin").as_deref(), Some("remote://nas/games/disc/tracks/Track 01.bin"));
        assert_eq!(super::remote_track_reference("remote://nas/games/disc", "../track.raw").as_deref(), Some("remote://nas/games/track.raw"));
        assert_eq!(super::remote_track_reference("remote://nas", "../track.raw"), None);
    }

    #[test]
    fn matching_track_name_in_another_folder_is_not_associated() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let gdi = first.join("game.gdi");
        let referenced = first.join("track01.bin");
        let unreferenced = second.join("track01.bin");
        fs::write(&gdi, "1\n1 0 4 2352 track01.bin 0\n").unwrap();
        fs::write(&referenced, b"a").unwrap();
        fs::write(&unreferenced, b"b").unwrap();
        let analysis = super::scan_selection(&[gdi, referenced, unreferenced.clone()], usize::MAX, None).unwrap();
        assert_eq!(analysis.orphan_bins, vec![unreferenced.to_string_lossy().to_string()]);
    }

    #[test]
    fn gdi_iso_tracks_are_not_separate_sources_but_unrelated_isos_remain() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let gdi = root.join("game.gdi");
        let iso1 = root.join("track 01.iso");
        let raw = root.join("track02.raw");
        let iso3 = root.join("track03.iso");
        let independent = root.join("other.iso");
        fs::write(&gdi, "3\n1 0 4 2048 \"track 01.iso\" 0\n2 20 0 2352 track02.raw 0\n3 45000 4 2048 track03.iso 0\n").unwrap();
        for path in [&iso1, &raw, &iso3, &independent] { fs::write(path, b"track").unwrap(); }
        for selection in [vec![iso1.clone(), raw.clone(), independent.clone(), iso3.clone(), gdi.clone()], vec![root.to_path_buf()]] {
            let result = super::scan_selection(&selection, usize::MAX, None).unwrap();
            assert_eq!(result.chd_sources.len(), 2);
            let gdi_source = result.chd_sources.iter().find(|s| s.source_path.ends_with("game.gdi")).unwrap();
            assert_eq!(gdi_source.command, "createcd");
            assert_eq!(gdi_source.required_paths.len(), 4);
            assert!(gdi_source.missing_files.is_empty());
            assert!(result.chd_sources.iter().any(|s| s.source_path.ends_with("other.iso") && s.command == "createdvd"));
        }
        let just_gdi = super::scan_selection(&[gdi], usize::MAX, None).unwrap();
        assert_eq!(just_gdi.chd_sources.len(), 1);
        assert_eq!(just_gdi.chd_sources[0].required_paths.len(), 4);
    }

    #[test]
    fn local_transfer_preserves_existing_destination_when_source_is_missing() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = dir.path().join("missing");
        let target = dir.path().join("target");
        std::fs::write(&target, b"original").unwrap();
        assert!(super::copy_or_move_one_staged(&source, &target, true, true).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn local_transfer_copies_and_moves_without_losing_the_source_or_destination() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        std::fs::write(&source, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        assert!(super::copy_or_move_one_staged(&source, &target, false, false).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        super::copy_or_move_one_staged(&source, &target, false, true).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), b"new");
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(super::copy_or_move_one_staged(&source, &source, true, true).is_err());
        super::copy_or_move_one_staged(&source, &target, true, true).unwrap();
        assert!(!source.exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
    }
    use super::*;

    #[test]
    fn zip_preview_reads_index_without_streaming_archive_content() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct CountingReader { inner: io::Cursor<Vec<u8>>, bytes: Rc<Cell<usize>> }
        impl Read for CountingReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let n = self.inner.read(buf)?;
                self.bytes.set(self.bytes.get() + n);
                Ok(n)
            }
        }
        impl Seek for CountingReader {
            fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> { self.inner.seek(pos) }
        }
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        writer.start_file("carpeta/disco.bin", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
        writer.write_all(&vec![42; 4 * 1024 * 1024]).unwrap();
        let data = writer.finish().unwrap().into_inner();
        let count = Rc::new(Cell::new(0));
        let entries = list_all_zip_reader(CountingReader { inner: io::Cursor::new(data), bytes: count.clone() }).unwrap();
        assert!(entries.contains(&("carpeta".to_string(), true)));
        assert!(entries.contains(&("carpeta/disco.bin".to_string(), false)));
        assert!(count.get() < 256 * 1024, "preview read {} bytes", count.get());
    }

    #[cfg(unix)]
    #[test]
    fn copying_and_removing_symlinks_never_traverses_the_target() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let outside = root.path().join("outside");
        let source = root.path().join("source");
        fs::create_dir(&outside).unwrap();
        fs::create_dir(&source).unwrap();
        fs::write(outside.join("keep"), b"data").unwrap();
        std::os::unix::fs::symlink(&outside, source.join("link")).unwrap();
        std::os::unix::fs::symlink("missing", source.join("broken")).unwrap();
        let copied = root.path().join("copy");
        copy_path(&source, &copied).unwrap();
        assert!(fs::symlink_metadata(copied.join("link")).unwrap().file_type().is_symlink());
        assert!(fs::symlink_metadata(copied.join("broken")).unwrap().file_type().is_symlink());
        remove_single_path(&copied.join("link")).unwrap();
        remove_single_path(&copied.join("broken")).unwrap();
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"data");
    }

    #[cfg(unix)]
    #[test]
    fn unix_paths_with_different_case_are_distinct() {
        let a = PathBuf::from("/games/Game.chd");
        let b = PathBuf::from("/games/game.chd");
        assert!(!same_path(&a, &b));
        assert_eq!(dedupe_paths(vec![a, b]).len(), 2);
    }

    #[test]
    fn identifies_xiso_by_both_volume_signatures_not_the_filename() {
        use std::io::{Seek, Write};
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let xiso = root.path().join("game.iso");
        let mut file = fs::File::create(&xiso).unwrap();
        file.seek(io::SeekFrom::Start(XDVDFS_VOLUME_SECTOR)).unwrap();
        file.write_all(XDVDFS_VOLUME_MAGIC).unwrap();
        assert_eq!(xdvdfs_kind(&xiso).unwrap(), None);
        file.seek(io::SeekFrom::Start(XDVDFS_VOLUME_SECTOR + 2048 - 20)).unwrap();
        file.write_all(XDVDFS_VOLUME_MAGIC).unwrap();
        drop(file);
        assert_eq!(xdvdfs_kind(&xiso).unwrap(), Some(XisoKind::Trimmed));

        let dump = root.path().join("dump.iso");
        let mut file = fs::File::create(&dump).unwrap();
        let sector = XDVDFS_PARTITION_OFFSETS[3] + XDVDFS_VOLUME_SECTOR;
        file.seek(io::SeekFrom::Start(sector)).unwrap();
        file.write_all(XDVDFS_VOLUME_MAGIC).unwrap();
        file.seek(io::SeekFrom::Start(sector + 2048 - 20)).unwrap();
        file.write_all(XDVDFS_VOLUME_MAGIC).unwrap();
        drop(file);
        assert_eq!(xdvdfs_kind(&dump).unwrap(), Some(XisoKind::Redump));
        assert_eq!(xdvdfs_kind(&root.path().join("missing.iso")).unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn retries_only_transient_errors_and_caps_attempts() {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = retry_transient(|| { attempts += 1; if attempts < 3 { Err(io::Error::new(io::ErrorKind::TimedOut, "timeout").into()) } else { Ok(42) } }, |n, _| { waits.push(n); Ok(()) });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 3);
        assert_eq!(waits, vec![1, 2]);
        attempts = 0;
        let result: Result<()> = retry_transient(|| { attempts += 1; Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied").into()) }, |_, _| Ok(()));
        assert!(result.is_err());
        assert_eq!(attempts, 1);
        attempts = 0;
        let result: Result<()> = retry_transient(|| { attempts += 1; Err(io::Error::new(io::ErrorKind::TimedOut, "timeout").into()) }, |_, _| Ok(()));
        assert!(result.is_err());
        assert_eq!(attempts, 3);
    }

    #[test]
    fn staged_folder_preserves_existing_on_conflict_and_replaces_on_request() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let target = root.path().join("output");
        let staged = root.path().join("stage");
        fs::create_dir(&target).unwrap();
        fs::create_dir(&staged).unwrap();
        fs::write(target.join("old"), b"old").unwrap();
        fs::write(staged.join("new"), b"new").unwrap();
        assert!(publish_staged_directory(&staged, &target, false).is_err());
        assert!(target.join("old").exists());
        publish_staged_directory(&staged, &target, true).unwrap();
        assert_eq!(fs::read(target.join("new")).unwrap(), b"new");
        assert!(!target.join("old").exists());
    }

    #[test]
    fn publishing_multi_file_set_checks_all_collisions_first() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = root.path().join("source");
        let dest = root.path().join("dest");
        fs::create_dir(&source).unwrap(); fs::create_dir(&dest).unwrap();
        fs::write(source.join("disc.cue"), b"new cue").unwrap();
        fs::write(source.join("disc.bin"), b"new bin").unwrap();
        fs::write(dest.join("disc.bin"), b"old bin").unwrap();
        let files = vec![source.join("disc.cue"), source.join("disc.bin")];
        assert!(publish_file_set(&files, &dest, false).is_err());
        assert!(!dest.join("disc.cue").exists());
        assert_eq!(fs::read(dest.join("disc.bin")).unwrap(), b"old bin");
        publish_file_set(&files, &dest, true).unwrap();
        assert_eq!(fs::read(dest.join("disc.cue")).unwrap(), b"new cue");
        assert_eq!(fs::read(dest.join("disc.bin")).unwrap(), b"new bin");
    }

    fn extraction_options(rename: bool, overwrite: bool) -> ExtractionOptionsPayload {
        ExtractionOptionsPayload { planned_folder_name: None, planned_entries: None, shared_folder_name: None, individual_folders: false, split_entries: false, destination_mode: "custom".into(), destination_path: None, delete_archives: false, overwrite, rename_on_conflict: rename, remote_destination: None, remote_transfer: None }
    }

    #[test]
    fn transient_error_classification_is_conservative() {
        let temporary = anyhow::Error::new(io::Error::new(io::ErrorKind::TimedOut, "timeout")).context("read");
        assert!(is_transient_io(&temporary));
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::NotFound, io::ErrorKind::InvalidData, io::ErrorKind::WriteZero] {
            assert!(!is_transient_io(&anyhow::Error::new(io::Error::new(kind, "failure"))));
        }
        assert!(!is_transient_io(&anyhow!("archive corrupt")));
    }

    #[test]
    fn extraction_conflict_preserves_existing_files() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let dest = dir.path().join("dest"); let stage = dir.path().join("stage");
        fs::create_dir(&dest).unwrap(); fs::create_dir(&stage).unwrap();
        fs::write(dest.join("a.txt"), "original").unwrap();
        fs::write(stage.join("a.txt"), "new").unwrap();
        fs::write(stage.join("z.txt"), "other").unwrap();
        assert!(publish_extracted_tree(&stage, &dest, Path::new("input.zip"), &extraction_options(false, false)).is_err());
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "original");
        assert!(!dest.join("z.txt").exists());
    }

    #[test]
    fn extraction_rename_reserves_all_generated_names() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let dest = dir.path().join("dest"); let stage = dir.path().join("stage");
        fs::create_dir(&dest).unwrap(); fs::create_dir(&stage).unwrap();
        fs::write(dest.join("a.txt"), "original").unwrap();
        fs::write(stage.join("a.txt"), "first").unwrap();
        fs::write(stage.join("a (2).txt"), "second").unwrap();
        publish_extracted_tree(&stage, &dest, Path::new("input.zip"), &extraction_options(true, false)).unwrap();
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "original");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 3);
    }

    #[test]
    fn staged_publication_does_not_overwrite_without_permission() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let dest = dir.path().join("existing.zip"); let stage = dir.path().join("staged.zip");
        fs::write(&dest, "original").unwrap(); fs::write(&stage, "new").unwrap();
        assert!(commit_staged_file(&stage, &dest, false).is_err());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "original");
    }

    #[test]
    fn extraction_refuses_a_track_rename_that_would_break_its_cue() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("game.cue"), b"FILE \"game.bin\" BINARY\n").unwrap();
        fs::write(source.join("game.bin"), b"track").unwrap();
        let entries = vec![
            crate::models::PlannedExtractionEntry { from: "game.cue".into(), to: "game.cue".into(), is_dir: false },
            crate::models::PlannedExtractionEntry { from: "game.bin".into(), to: "game (1).bin".into(), is_dir: false },
        ];
        assert!(remap_extraction_tree(&source, &root.path().join("out"), &entries).is_err());
        assert!(source.join("game.cue").exists());
        assert!(source.join("game.bin").exists());
    }

    #[test]
    fn planned_7z_extraction_refuses_destination_that_appears_later() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = root.path().join("stage");
        let destination = root.path().join("dest");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("game.bin"), b"new").unwrap();
        fs::write(destination.join("game.bin"), b"existing").unwrap();
        let mut options = extraction_options(false, false);
        options.planned_entries = Some(vec![crate::models::PlannedExtractionEntry { from: "game.bin".into(), to: "game.bin".into(), is_dir: false }]);
        assert!(publish_extracted_tree(&source, &destination, Path::new("input.7z"), &options).is_err());
        assert_eq!(fs::read(destination.join("game.bin")).unwrap(), b"existing");
    }

    #[test]
    fn extraction_mapping_preserves_planned_names_and_rejects_unknown_entries() {
        let root = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = root.path().join("source");
        let output = root.path().join("output");
        fs::create_dir_all(source.join("game")).unwrap();
        fs::write(source.join("game/disc.bin"), b"data").unwrap();
        let entries = vec![
            crate::models::PlannedExtractionEntry { from: "game".into(), to: "game (1)".into(), is_dir: true },
            crate::models::PlannedExtractionEntry { from: "game/disc.bin".into(), to: "game (1)/disc.bin".into(), is_dir: false },
        ];
        remap_extraction_tree(&source, &output, &entries).unwrap();
        assert_eq!(fs::read(output.join("game (1)/disc.bin")).unwrap(), b"data");
        fs::write(source.join("game/unexpected.bin"), b"other").unwrap();
        assert!(remap_extraction_tree(&source, &root.path().join("second"), &entries).is_err());
    }

    #[test]
    fn planned_disc_names_cannot_escape_the_destination() {
        assert_eq!(validated_planned_output_name(Some("game (1).iso"), "game.iso").unwrap(), "game (1).iso");
        assert!(validated_planned_output_name(Some("../escape.iso"), "game.iso").is_err());
        assert!(validated_planned_output_name(Some("sub\\escape.iso"), "game.iso").is_err());
    }

    #[test]
    fn rename_refuses_to_overwrite_existing_destination() {
        let directory = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let source = directory.path().join("original.bin");
        let target = directory.path().join("target.bin");
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"existing").unwrap();
        assert!(rename_path(&source, "target.bin").is_err());
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&target).unwrap(), b"existing");
        assert!(rename_path(&source, "../target.bin").is_err());
    }

    #[test]
    fn remote_chd_cleanup_maps_all_tracks_and_rejects_escape() {
        let root = PathBuf::from("download");
        let cue = root.join("Tekken.cue");
        let tracks = [cue.clone(), root.join("Tekken (Track 1).bin"), root.join("Tekken (Track 2).bin"), root.join("Tekken (Track 3).bin")];
        let required = tracks.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>();
        let actual = remote_original_paths("remote://session-1/psx/Tekken", &cue, &required).unwrap();
        assert_eq!(actual.len(), 4);
        assert!(actual[3].ends_with("/Tekken (Track 3).bin"));
        let escape = vec![root.join("..").join("outside.bin").to_string_lossy().to_string()];
        assert!(remote_original_paths("remote://session-1/psx/Tekken", &cue, &escape).is_err());
    }

    #[test]
    fn parse_7z_slt_entries_skips_archive_header_and_keeps_entries() {
        let sample = r#"Path = C:\archives\game.rar
Type = Rar5
Physical Size = 12345
----------
Path = Disc 1
Folder = +

Path = Disc 1\game.cue
Folder = -
Attributes = A

Path = Disc 1\Track 01.bin
Folder = -
Attributes = A
"#;

        let parsed = parse_7z_slt_entries(sample);
        assert_eq!(
            parsed,
            vec![
                ("Disc 1".to_string(), true),
                ("Disc 1/game.cue".to_string(), false),
                ("Disc 1/Track 01.bin".to_string(), false),
            ]
        );
    }

    #[test]
    fn parse_7z_slt_entries_detects_directory_via_attributes() {
        let sample = r#"Path = archive.7z
Type = 7z
----------
Path = saves
Attributes = D_ drwxr-xr-x

Path = saves\slot1.srm
Attributes = A_ -rw-r--r--
"#;

        let parsed = parse_7z_slt_entries(sample);
        assert_eq!(
            parsed,
            vec![
                ("saves".to_string(), true),
                ("saves/slot1.srm".to_string(), false),
            ]
        );
    }
}

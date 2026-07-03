use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Result;
use regex::Regex;
use walkdir::WalkDir;

use crate::models::{
    M3uEntryDto, M3uFailureDto, M3uGenerateGroupPayload, M3uGenerateResultDto,
    M3uGroupDto, M3uScanOptions, M3uWarningDto,
};

// Extensions that are valid M3U entries (directly listed)
const ELIGIBLE_EXT: &[&str] = &["cue", "chd", "gdi", "iso", "img"];

// ------------------------------------------------------------------
// Disc pattern detection
// ------------------------------------------------------------------

struct DiscInfo {
    sort_key: u32,
    base_name: String,
}

fn detect_disc(stem: &str) -> Option<DiscInfo> {
    static PATTERNS: OnceLock<Vec<(Regex, bool)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // (Disc N) / (Disc N of M) — No-Intro / Redump standard
            (Regex::new(r"(?i)\s*\(Dis[ck]\s+(\d+)(?:\s+of\s+\d+)?\)").unwrap(), false),
            // (CD N) / (CD-N)
            (Regex::new(r"(?i)\s*\(CD[- ]?(\d+)\)").unwrap(), false),
            // (Side X) — letter
            (Regex::new(r"(?i)\s*\(Side\s+([A-Za-z])\)").unwrap(), true),
            // - Disc N / - Disk N (hyphen, outside parens)
            (Regex::new(r"(?i)\s*[-–]\s*Dis[ck]\s+(\d+)").unwrap(), false),
            // - CD N
            (Regex::new(r"(?i)\s*[-–]\s*CD\s*(\d+)").unwrap(), false),
            // DiscN / DiskN (no parens, least priority)
            (Regex::new(r"(?i)\s+Dis[ck](\d+)").unwrap(), false),
        ]
    });

    for (re, is_letter) in patterns {
        if let Some(caps) = re.captures(stem) {
            let raw_base = re.replace(stem, "").to_string();
            // Collapse any double-spaces produced by the removal
            let base_name = raw_base.split_whitespace().collect::<Vec<_>>().join(" ");
            let sort_key = caps.get(1).map(|m| {
                let s = m.as_str();
                if *is_letter {
                    s.chars().next()
                        .map(|c| c.to_ascii_uppercase() as u32 - b'A' as u32 + 1)
                        .unwrap_or(1)
                } else {
                    s.parse::<u32>().unwrap_or(1)
                }
            }).unwrap_or(1);
            return Some(DiscInfo { sort_key, base_name });
        }
    }
    None
}

// ------------------------------------------------------------------
// Internal file representation during scanning
// ------------------------------------------------------------------

pub struct RawFile {
    pub abs_path: String,
    /// Parent dir with '/' separators (needed to compute relative paths)
    pub parent_dir: String,
    pub stem: String,
    pub ext: String,
}

fn normalize_sep(s: &str) -> String {
    s.replace('\\', "/")
}

// ------------------------------------------------------------------
// Local scanning
// ------------------------------------------------------------------

pub fn scan_local(dirs: &[PathBuf], recursive: bool) -> Vec<RawFile> {
    let mut results = Vec::new();
    for dir in dirs {
        let walker = if recursive {
            WalkDir::new(dir).min_depth(1)
        } else {
            WalkDir::new(dir).min_depth(1).max_depth(1)
        };
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_lowercase(),
                None => continue,
            };
            if !ELIGIBLE_EXT.contains(&ext.as_str()) {
                continue;
            }
            // Skip .bin that are covered by a .cue in the same directory —
            // the cue already references the bin internally. We never list .bin
            // directly because ELIGIBLE_EXT doesn't include it; this comment
            // documents the intentional omission.
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let parent = match path.parent() {
                Some(p) => normalize_sep(&p.to_string_lossy()),
                None => continue,
            };
            results.push(RawFile {
                abs_path: normalize_sep(&path.to_string_lossy()),
                parent_dir: parent,
                stem,
                ext,
            });
        }
    }
    results
}

// ------------------------------------------------------------------
// Grouping (shared by local and remote paths)
// ------------------------------------------------------------------

pub fn build_groups(
    files: Vec<RawFile>,
    scan_roots: &[String],
    options: &M3uScanOptions,
) -> Vec<M3uGroupDto> {
    // Group key: (output_dir, lowercase_base_name)
    let mut map: HashMap<(String, String), Vec<(RawFile, DiscInfo)>> = HashMap::new();

    for file in files {
        let (sort_key, base_name) = match detect_disc(&file.stem) {
            Some(info) => (info.sort_key, info.base_name),
            None => (0, file.stem.clone()),
        };

        // Determine the directory the M3U will go into
        let output_dir = match options.output_location.as_str() {
            "scanRoot" => {
                // Find the scan root that is a prefix of this file's parent
                scan_roots
                    .iter()
                    .find(|r| {
                        let r = normalize_sep(r);
                        file.parent_dir == r || file.parent_dir.starts_with(&format!("{r}/"))
                    })
                    .map(|r| normalize_sep(r))
                    .unwrap_or_else(|| file.parent_dir.clone())
            }
            // "currentDir" uses the same custom_output_path as "custom";
            // the frontend sets it to the directory the user is currently browsing.
            "custom" | "currentDir" => options
                .custom_output_path
                .as_deref()
                .map(normalize_sep)
                .unwrap_or_else(|| file.parent_dir.clone()),
            _ => file.parent_dir.clone(), // "sameFolder"
        };

        let key = (output_dir, base_name.to_lowercase());
        let disc_info = DiscInfo { sort_key, base_name };
        map.entry(key).or_default().push((file, disc_info));
    }

    let mut groups: Vec<M3uGroupDto> = map
        .into_iter()
        .filter_map(|((output_dir, _), mut files_with_info)| {
            // Recover the display base_name from first entry
            let base_name = files_with_info
                .first()
                .map(|(_, info)| info.base_name.clone())
                .unwrap_or_default();

            if options.multi_disc_only && files_with_info.len() < 2 {
                return None;
            }

            // Sort by disc sort_key, then by abs_path for stable tie-breaking
            files_with_info.sort_by(|(fa, ia), (fb, ib)| {
                ia.sort_key.cmp(&ib.sort_key).then_with(|| fa.abs_path.cmp(&fb.abs_path))
            });

            // Build entries
            let mut entries = Vec::new();
            for (file, info) in &files_with_info {
                let m3u_path = if options.use_relative_paths {
                    relative_path(&output_dir, &file.abs_path)
                } else {
                    file.abs_path.clone()
                };
                entries.push(M3uEntryDto {
                    absolute_path: file.abs_path.clone(),
                    m3u_path,
                    disc_number: info.sort_key,
                    format: file.ext.clone(),
                });
            }

            // Build warnings
            let mut warnings = Vec::new();
            let disc_nums: Vec<u32> = files_with_info.iter().map(|(_, i)| i.sort_key).collect();

            // Duplicate disc numbers (excluding 0 = single-disc)
            let mut seen = std::collections::HashSet::new();
            for &n in &disc_nums {
                if n > 0 && !seen.insert(n) {
                    warnings.push(M3uWarningDto {
                        kind: "duplicateDisc".to_string(),
                        detail: Some(n.to_string()),
                    });
                    break;
                }
            }

            // Missing discs (gap in sequence, e.g. 1,3 but not 2)
            let mut sorted_nums = disc_nums.clone();
            sorted_nums.dedup();
            sorted_nums.sort_unstable();
            if sorted_nums.first().copied().unwrap_or(0) > 0 {
                for window in sorted_nums.windows(2) {
                    if window[1] > window[0] + 1 {
                        warnings.push(M3uWarningDto {
                            kind: "missingDisc".to_string(),
                            detail: Some(format!("{}-{}", window[0] + 1, window[1] - 1)),
                        });
                    }
                }
            }

            // Mixed formats
            let formats: Vec<String> = {
                let mut v: Vec<String> = files_with_info.iter().map(|(f, _)| f.ext.clone()).collect();
                v.sort_unstable();
                v.dedup();
                v
            };
            if formats.len() > 1 {
                warnings.push(M3uWarningDto {
                    kind: "mixedFormats".to_string(),
                    detail: Some(formats.join(", ")),
                });
            }

            // M3U already exists
            let m3u_file = format!("{}/{}.m3u", output_dir.trim_end_matches('/'), sanitize_filename(&base_name));
            let m3u_exists = Path::new(&m3u_file).exists();
            if m3u_exists {
                warnings.push(M3uWarningDto {
                    kind: "m3uExists".to_string(),
                    detail: None,
                });
            }

            let id = format!("{}:{}", output_dir, base_name.to_lowercase());

            Some(M3uGroupDto {
                id,
                base_name,
                output_path: m3u_file,
                m3u_exists,
                entries,
                warnings,
            })
        })
        .collect();

    // Sort groups alphabetically by base_name for a predictable preview order
    groups.sort_by(|a, b| a.base_name.to_lowercase().cmp(&b.base_name.to_lowercase()));
    groups
}

// ------------------------------------------------------------------
// Remote path support: build RawFile list from remote entry data
// (entries come from RemoteManager::scan_files_recursive)
// ------------------------------------------------------------------

/// Build a RawFile from a remote virtual path.
/// `abs_path` here is the virtual path (`remote://session-id/logical/path`).
/// Separators are already '/' in logical paths.
pub fn raw_file_from_remote_path(virtual_path: &str) -> Option<RawFile> {
    let last_slash = virtual_path.rfind('/')?;
    let parent_dir = virtual_path[..last_slash].to_string();
    let filename = &virtual_path[last_slash + 1..];
    let dot = filename.rfind('.')?;
    let stem = filename[..dot].to_string();
    let ext = filename[dot + 1..].to_lowercase();
    if !ELIGIBLE_EXT.contains(&ext.as_str()) {
        return None;
    }
    Some(RawFile {
        abs_path: virtual_path.to_string(),
        parent_dir,
        stem,
        ext,
    })
}

// ------------------------------------------------------------------
// M3U content generation
// ------------------------------------------------------------------

pub fn format_m3u_content(base_name: &str, entries: &[String]) -> String {
    let mut lines = vec![format!("# {base_name}")];
    for entry in entries {
        // Always use '/' in M3U, even on Windows
        lines.push(entry.replace('\\', "/"));
    }
    lines.join("\n") + "\n"
}

// ------------------------------------------------------------------
// Local file generation
// ------------------------------------------------------------------

pub fn generate_local(payload: &M3uGenerateGroupPayload, overwrite: bool) -> Result<()> {
    let out = Path::new(&payload.output_path);
    if out.exists() && !overwrite {
        return Err(anyhow::anyhow!("exists"));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format_m3u_content(&payload.base_name, &payload.entries);
    std::fs::write(out, content.as_bytes())?;
    Ok(())
}

pub fn generate_all_local(
    groups: &[M3uGenerateGroupPayload],
    overwrite: bool,
) -> M3uGenerateResultDto {
    let mut created = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for group in groups {
        match generate_local(group, overwrite) {
            Ok(()) => created.push(group.output_path.clone()),
            Err(e) if e.to_string() == "exists" => skipped.push(group.output_path.clone()),
            Err(e) => failed.push(M3uFailureDto {
                path: group.output_path.clone(),
                error: e.to_string(),
            }),
        }
    }

    M3uGenerateResultDto { created, skipped, failed }
}

// ------------------------------------------------------------------
// Path utilities
// ------------------------------------------------------------------

/// Compute a relative path from `from_dir` to `target_file` using '/' separators.
/// Falls back to `target_file` if the prefix is not shared.
pub fn relative_path(from_dir: &str, target_file: &str) -> String {
    let from = from_dir.trim_end_matches('/');
    let target = target_file.trim_end_matches('/');

    if let Some(rel) = target.strip_prefix(&format!("{from}/")) {
        return rel.to_string();
    }
    // Different roots — return the target as-is
    target_file.to_string()
}

/// Strip characters that are invalid in filenames on Windows and Linux.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

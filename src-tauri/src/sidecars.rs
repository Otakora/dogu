use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tauri::{AppHandle, Manager};

fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

fn exe_name(base: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

pub fn runtime_root(app: &AppHandle) -> PathBuf {
    if let Some(env_root) = std::env::var_os("DOGU_RESOURCES_DIR") {
        return normalize_resource_root(PathBuf::from(env_root));
    }

    if cfg!(debug_assertions) {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        return normalize_resource_root(project_root);
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        return normalize_resource_root(resource_dir);
    }

    normalize_resource_root(PathBuf::from("."))
}

fn normalize_resource_root(root: PathBuf) -> PathBuf {
    if has_bundled_resources(&root) {
        return root;
    }

    let tauri_parent_escape_root = root.join("_up_");
    if has_bundled_resources(&tauri_parent_escape_root) {
        return tauri_parent_escape_root;
    }

    root
}

fn has_bundled_resources(root: &Path) -> bool {
    root.join("third_party").exists() || root.join("THIRD_PARTY_NOTICES").exists()
}

pub fn chdman_path(app: &AppHandle) -> Option<PathBuf> {
    let root = runtime_root(app);
    let candidate = root
        .join("third_party")
        .join("chdman")
        .join(platform_name())
        .join(exe_name("chdman"));
    if candidate.exists() {
        return Some(candidate);
    }
    let fallback = root
        .join("third_party")
        .join("chdman")
        .join(exe_name("chdman"));
    if fallback.exists() {
        return Some(fallback);
    }
    std::env::var_os("PATH").and_then(|_| which_in_path(&exe_name("chdman")))
}

pub fn seven_zip_path(app: &AppHandle) -> Option<PathBuf> {
    generic_seven_zip_candidates(app).into_iter().next()
}

pub fn rar_capable_seven_zip_path(app: &AppHandle) -> Option<PathBuf> {
    rar_seven_zip_candidates(app)
        .into_iter()
        .find(|path| seven_zip_supports_rar(path))
}

fn generic_seven_zip_candidates(app: &AppHandle) -> Vec<PathBuf> {
    build_seven_zip_candidates(app, false)
}

fn rar_seven_zip_candidates(app: &AppHandle) -> Vec<PathBuf> {
    build_seven_zip_candidates(app, true)
}

fn build_seven_zip_candidates(
    app: &AppHandle,
    prefer_rar_capable_path_tools: bool,
) -> Vec<PathBuf> {
    let root = runtime_root(app);
    let bundled_7zz = root
        .join("third_party")
        .join("7zip")
        .join(platform_name())
        .join(exe_name("7zz"));
    let bundled_7z = root
        .join("third_party")
        .join("7zip")
        .join(platform_name())
        .join(exe_name("7z"));
    let bundled_7za = root
        .join("third_party")
        .join("7zip")
        .join(platform_name())
        .join(exe_name("7za"));

    let mut candidates = Vec::new();
    let ordered = if prefer_rar_capable_path_tools {
        vec![
            Some(bundled_7zz),
            Some(bundled_7z),
            which_in_path(&exe_name("7zz")),
            which_in_path(&exe_name("7z")),
            Some(bundled_7za),
            which_in_path(&exe_name("7za")),
        ]
    } else {
        vec![
            Some(bundled_7zz),
            Some(bundled_7za),
            Some(bundled_7z),
            which_in_path(&exe_name("7zz")),
            which_in_path(&exe_name("7z")),
            which_in_path(&exe_name("7za")),
        ]
    };

    for candidate in ordered.into_iter().flatten() {
        if candidate.exists() && !candidates.iter().any(|p| p == &candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

/// Locates the bundled (or PATH-provided) DolphinTool binary used as the RVZ
/// fallback engine. Mirrors `chdman_path`; accepts both the official
/// `DolphinTool` name and the lowercase `dolphin-tool` some builds ship.
pub fn dolphin_tool_path(app: &AppHandle) -> Option<PathBuf> {
    let root = runtime_root(app);
    for base in ["DolphinTool", "dolphin-tool"] {
        let candidate = root
            .join("third_party")
            .join("dolphin-tool")
            .join(platform_name())
            .join(exe_name(base));
        if candidate.exists() {
            return Some(candidate);
        }
        let fallback = root
            .join("third_party")
            .join("dolphin-tool")
            .join(exe_name(base));
        if fallback.exists() {
            return Some(fallback);
        }
    }
    which_in_path(&exe_name("DolphinTool")).or_else(|| which_in_path(&exe_name("dolphin-tool")))
}

fn seven_zip_supports_rar(path: &Path) -> bool {
    match tool_stem(path).as_deref() {
        Some("7z") | Some("7zz") => true,
        Some("7za") | Some("7zr") => false,
        _ => probe_seven_zip_for_rar_support(path),
    }
}

fn tool_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
}

fn probe_seven_zip_for_rar_support(path: &Path) -> bool {
    let Ok(output) = Command::new(path).arg("i").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    stdout.contains(" rar      rar ")
        || stdout.contains(" rar5     rar ")
        || stdout.contains(" rar1")
        || stdout.contains(" rar2")
        || stdout.contains(" rar3")
        || stdout.contains(" rar5")
}

fn which_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for path in std::env::split_paths(&path_var) {
        let candidate = path.join(binary_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::tool_stem;
    use std::path::Path;

    #[test]
    fn tool_stem_handles_common_executables() {
        assert_eq!(
            tool_stem(Path::new("C:\\tools\\7z.exe")).as_deref(),
            Some("7z")
        );
        assert_eq!(
            tool_stem(Path::new("C:\\tools\\7zz.exe")).as_deref(),
            Some("7zz")
        );
        assert_eq!(
            tool_stem(Path::new("C:\\tools\\7za.exe")).as_deref(),
            Some("7za")
        );
    }
}

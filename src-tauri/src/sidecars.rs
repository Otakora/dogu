use std::path::{Path, PathBuf};

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
        return PathBuf::from(env_root);
    }

    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        return resource_dir;
    }

    PathBuf::from(".")
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
    let root = runtime_root(app);
    let candidate = root
        .join("third_party")
        .join("7zip")
        .join(platform_name())
        .join(exe_name("7zz"));
    if candidate.exists() {
        return Some(candidate);
    }
    let windows_fallback = root
        .join("third_party")
        .join("7zip")
        .join(platform_name())
        .join(exe_name("7za"));
    if windows_fallback.exists() {
        return Some(windows_fallback);
    }
    which_in_path(&exe_name("7zz"))
        .or_else(|| which_in_path(&exe_name("7z")))
        .or_else(|| which_in_path(&exe_name("7za")))
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

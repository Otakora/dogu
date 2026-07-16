use serde::Serialize;
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const STABLE_ENDPOINT: &str = "https://github.com/Otakora/dogu/releases/latest/download/latest.json";
const BETA_ENDPOINT: &str = "https://github.com/Otakora/dogu/releases/download/beta/latest-beta.json";
const STABLE_RELEASE_URL: &str = "https://github.com/Otakora/dogu/releases/latest";
const BETA_RELEASE_URL: &str = "https://github.com/Otakora/dogu/releases/tag/beta";
const UPDATE_SIGNING_PUBLIC_KEY: Option<&str> = option_env!("DOGU_UPDATER_PUBLIC_KEY");

pub struct PendingUpdate {
    update: Mutex<Option<Update>>,
}

impl PendingUpdate {
    pub fn new() -> Self {
        Self {
            update: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadataDto {
    pub version: String,
    pub current_version: String,
    pub channel: String,
    pub body: Option<String>,
    pub date: Option<String>,
    pub is_downgrade: bool,
    pub can_install: bool,
    pub manual_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum UpdateDownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started { content_length: Option<u64> },
    #[serde(rename_all = "camelCase")]
    Progress {
        downloaded: u64,
        chunk_length: usize,
        content_length: Option<u64>,
    },
    Finished,
}

fn endpoint_for_channel(channel: &str) -> Result<&'static str, String> {
    match channel {
        "stable" => Ok(STABLE_ENDPOINT),
        "beta" => Ok(BETA_ENDPOINT),
        _ => Err(format!("Unknown update channel: {channel}")),
    }
}

fn release_url_for_channel(channel: &str) -> &'static str {
    match channel {
        "beta" => BETA_RELEASE_URL,
        _ => STABLE_RELEASE_URL,
    }
}

fn current_runtime_can_install_update() -> bool {
    if cfg!(target_os = "linux") {
        std::env::var_os("APPIMAGE").is_some()
    } else {
        cfg!(target_os = "windows")
    }
}

fn configured_public_key() -> Result<&'static str, String> {
    UPDATE_SIGNING_PUBLIC_KEY
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| "Dogu updater is not configured: missing DOGU_UPDATER_PUBLIC_KEY at build time.".to_string())
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
    channel: String,
    allow_downgrade: bool,
) -> Result<Option<UpdateMetadataDto>, String> {
    let endpoint = endpoint_for_channel(channel.as_str())?
        .parse()
        .map_err(|error| format!("Invalid update endpoint: {error}"))?;
    let public_key = configured_public_key()?;
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let mut builder = app
        .updater_builder()
        .pubkey(public_key)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?;

    if allow_downgrade {
        builder = builder.version_comparator(|current, update| update.version != current);
    }

    let update = builder
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;

    let Some(update) = update else {
        *pending_update
            .update
            .lock()
            .map_err(|_| "Could not lock pending update state.".to_string())? = None;
        return Ok(None);
    };

    let manual_url = release_url_for_channel(channel.as_str()).to_string();
    let metadata = UpdateMetadataDto {
        version: update.version.clone(),
        current_version,
        channel,
        body: update.body.clone(),
        date: update.date.map(|date| date.to_string()),
        is_downgrade: allow_downgrade,
        can_install: current_runtime_can_install_update(),
        manual_url,
    };

    *pending_update
        .update
        .lock()
        .map_err(|_| "Could not lock pending update state.".to_string())? = Some(update);

    Ok(Some(metadata))
}

#[tauri::command]
pub async fn install_pending_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
    on_event: Channel<UpdateDownloadEvent>,
) -> Result<(), String> {
    if !current_runtime_can_install_update() {
        return Err("This Dogu package can check for updates, but it cannot install them automatically. Open the release page and install the package manually.".to_string());
    }

    let update = pending_update
        .update
        .lock()
        .map_err(|_| "Could not lock pending update state.".to_string())?
        .take()
        .ok_or_else(|| "There is no pending update to install.".to_string())?;

    let mut downloaded = 0_u64;
    let _ = on_event.send(UpdateDownloadEvent::Started {
        content_length: None,
    });
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                let _ = on_event.send(UpdateDownloadEvent::Progress {
                    downloaded,
                    chunk_length,
                    content_length,
                });
            },
            || {
                let _ = on_event.send(UpdateDownloadEvent::Finished);
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

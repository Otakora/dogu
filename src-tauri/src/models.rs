use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub size_label: String,
    pub modified_ts: i64,
    pub modified_label: String,
    pub extension: String,
    pub has_children: bool,
    pub has_directory_children: bool,
    pub location_kind: String,
    pub display_path: String,
    pub root_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfileDto {
    pub id: String,
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub share: String,
    pub workgroup: String,
    pub start_path: String,
    pub ssh_mode: String,
    pub ftp_mode: String,
    pub ftp_secure_implicit: bool,
    pub ftp_accept_invalid_certificates: bool,
    pub ftp_accept_invalid_hostnames: bool,
    pub trusted_fingerprints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfilePayload {
    pub id: Option<String>,
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub share: String,
    pub workgroup: String,
    pub start_path: String,
    pub ssh_mode: String,
    pub ftp_mode: String,
    pub ftp_secure_implicit: bool,
    pub ftp_accept_invalid_certificates: bool,
    pub ftp_accept_invalid_hostnames: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveConnectionDto {
    pub session_id: String,
    pub profile_id: String,
    pub label: String,
    pub protocol: String,
    pub host: String,
    pub root_path: String,
    pub display_path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOpenResultDto {
    pub connected: bool,
    pub requires_trust: bool,
    pub fingerprint: Option<String>,
    pub message: Option<String>,
    pub connection: Option<ActiveConnectionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertiesSummaryDto {
    pub count: usize,
    pub files: usize,
    pub directories: usize,
    pub total_size: u64,
    pub total_size_label: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryOptionsPayload {
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionOptionsPayload {
    pub individual_folders: bool,
    pub destination_mode: String,
    pub destination_path: Option<String>,
    pub delete_archives: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionPreviewRow {
    pub archive_path: String,
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdSourceDto {
    pub source_path: String,
    pub container_dir: String,
    pub command: String,
    pub display_extensions: Vec<String>,
    pub required_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAnalysisDto {
    pub archives: Vec<String>,
    pub chd_sources: Vec<ChdSourceDto>,
    pub restorable_chds: Vec<String>,
    pub has_directories: bool,
    pub has_files: bool,
    pub unique_extensions: Vec<String>,
    pub chd_menu_label: Option<String>,
    pub chd_restore_menu_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdConversionOptionsPayload {
    pub delete_originals: bool,
    pub name_as_container: bool,
    pub deposit_to_parent: bool,
    pub delete_original_subfolders: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdRestoreOptionsPayload {
    pub individual_folders: bool,
    pub destination_mode: String,
    pub destination_path: Option<String>,
    pub delete_chd: bool,
    pub overwrite: bool,
    pub split_bin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadataDto {
    pub app_version: String,
    pub chdman: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressDto {
    pub job_id: String,
    pub progress: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobLogDto {
    pub job_id: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobFinishedDto {
    pub job_id: String,
    pub success: bool,
    pub message: String,
}

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
    pub write_access: Option<bool>,
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

/// How to handle errors when transferring output files to a remote destination.
/// "abort" → stop on first error; "skip" → log and continue; "pause" → block and ask the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransferPolicy {
    pub on_error: String,
}

impl RemoteTransferPolicy {
    pub fn on_error(&self) -> &str {
        self.on_error.as_str()
    }
}

/// Emitted as a Tauri event when a job pauses waiting for user input.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPausedDto {
    pub job_id: String,
    pub error: String,
    pub file_name: String,
    pub is_recoverable: bool,
}

/// A single pre-flight warning about the upcoming remote transfer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightWarning {
    /// "lowSpace" | "criticalSpace" | "noSpaceCheck"
    pub kind: String,
    pub detail: String,
}

/// Result of a pre-flight space check before a remote transfer operation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightCheckResult {
    /// false when the check found a critical problem (less space than the largest file)
    pub ok: bool,
    pub warnings: Vec<PreflightWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionOptionsPayload {
    pub individual_folders: bool,
    pub split_entries: bool,
    pub destination_mode: String,
    pub destination_path: Option<String>,
    pub delete_archives: bool,
    pub overwrite: bool,
    /// When not overwriting and a name collides, auto-rename instead of failing/skipping.
    #[serde(default)]
    pub rename_on_conflict: bool,
    /// Remote destination path (virtual). When set output is written to temp then uploaded.
    #[serde(default)]
    pub remote_destination: Option<String>,
    #[serde(default)]
    pub remote_transfer: Option<RemoteTransferPolicy>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionPreviewEntry {
    pub name: String,
    pub is_dir: bool,
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionPreviewRow {
    pub archive_path: String,
    pub destination_root: String,
    pub entries: Vec<ExtractionPreviewEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdSourceDto {
    pub source_path: String,
    pub container_dir: String,
    pub command: String,
    pub display_extensions: Vec<String>,
    pub required_paths: Vec<String>,
    /// Non-empty when one or more required files referenced by the source do not exist on disk.
    /// Sources with missing files must be skipped or surfaced as warnings.
    pub missing_files: Vec<String>,
}

/// Content-based classification of a disc image path, used to decide which
/// treatments are safe to offer for a `.iso` (see `ops::detect_disc_kinds`).
/// `kind` is one of: "xiso" (trimmed XDVDFS, ready for xemu / can be unpacked),
/// "redump" (full Xbox disc dump / XGD1-3, can be trimmed into a playable XISO),
/// or "iso" (not an Xbox disc — only generic CSO/CHD treatments apply).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscKindDto {
    pub path: String,
    pub kind: String,
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
    /// True when one or more selected remote paths are directories and no remote_manager was
    /// available to scan them. The frontend uses this to show the CHD option for remote folders.
    pub has_remote_directories: bool,
    /// .bin files found during scanning that have no matching .cue file anywhere in the same folder.
    pub orphan_bins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdConversionOptionsPayload {
    pub delete_originals: bool,
    pub name_as_container: bool,
    pub deposit_to_parent: bool,
    pub delete_original_subfolders: bool,
    pub overwrite: bool,
    /// When not overwriting and a name collides, auto-rename instead of failing.
    #[serde(default)]
    pub rename_on_conflict: bool,
    /// Custom output stem (no extension). Only applied when there is exactly 1 source.
    pub custom_name: Option<String>,
    /// "same" (default) = write alongside source; "custom" = write to destination_path.
    #[serde(default)]
    pub destination_mode: Option<String>,
    /// Local destination folder. Used when destination_mode == "custom" and remote_destination is None.
    #[serde(default)]
    pub destination_path: Option<String>,
    /// Remote destination path (virtual). When set the CHD is written to temp then uploaded.
    #[serde(default)]
    pub remote_destination: Option<String>,
    #[serde(default)]
    pub remote_transfer: Option<RemoteTransferPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChdRestoreOptionsPayload {
    pub individual_folders: bool,
    pub destination_mode: String,
    pub destination_path: Option<String>,
    /// "chdStem" | "custom" — how to name the extracted files (.cue/.bin/.iso)
    pub output_naming_mode: String,
    /// Used only when output_naming_mode == "custom" and there is exactly 1 CHD
    pub custom_output_name: Option<String>,
    /// "chdStem" | "custom" — how to name the individual subfolder (only when individual_folders)
    pub folder_naming_mode: String,
    /// Used only when folder_naming_mode == "custom" and there is exactly 1 CHD
    pub custom_folder_name: Option<String>,
    pub delete_chd: bool,
    pub overwrite: bool,
    pub split_bin: bool,
    /// When not overwriting and a name collides, auto-rename instead of failing.
    #[serde(default)]
    pub rename_on_conflict: bool,
    /// Remote destination path (virtual). When set the output is written to temp then uploaded.
    #[serde(default)]
    pub remote_destination: Option<String>,
    #[serde(default)]
    pub remote_transfer: Option<RemoteTransferPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscImageOptionsPayload {
    pub destination_mode: String,
    pub destination_path: Option<String>,
    pub delete_originals: bool,
    pub overwrite: bool,
    #[serde(default)]
    pub rename_on_conflict: bool,
    #[serde(default = "default_disc_image_compression_level")]
    pub compression_level: u8,
    #[serde(default)]
    pub remote_destination: Option<String>,
    #[serde(default)]
    pub remote_transfer: Option<RemoteTransferPolicy>,
    /// RVZ engine to try first ("nod" or "dolphin"). Only used by RVZ modes.
    #[serde(default)]
    pub rvz_primary_engine: Option<String>,
    /// RVZ engine to fall back to when the primary fails ("nod" or "dolphin").
    #[serde(default)]
    pub rvz_fallback_engine: Option<String>,
    /// Whether to try the fallback engine if the primary one fails.
    #[serde(default = "default_true")]
    pub rvz_enable_fallback: bool,
}

fn default_disc_image_compression_level() -> u8 {
    9
}

fn default_true() -> bool {
    true
}

/// Runtime availability check for an external tool binary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRuntimeDto {
    pub available: bool,
    /// Absolute path to the binary that was probed.
    pub path: Option<String>,
    /// First line of output from the binary, if it ran successfully.
    pub version: Option<String>,
    /// Human-readable reason why the tool is unavailable, if applicable.
    pub error: Option<String>,
}

/// Startup status of one external tool Dogu depends on, plus the capabilities it
/// unlocks. The frontend uses this to show the preparation screen and to disable
/// features whose tool is missing or not runnable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatusDto {
    /// Stable id: "chdman" | "sevenZip" | "dolphinTool".
    pub id: String,
    pub display_name: String,
    /// When true, Dogu still works if this tool is missing (only its features are
    /// disabled). When false, a core feature set is unavailable.
    pub optional: bool,
    /// How Dogu obtains this tool: "bundled" (shipped in repo) or "manual"
    /// (user installs / places it; Dogu detects it).
    pub provisioning: String,
    pub runtime: ToolRuntimeDto,
    /// Capability ids this tool unlocks when available (e.g. "chd", "archives").
    pub enables: Vec<String>,
    /// Optional page that helps the user obtain the tool when it is missing.
    pub guidance_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadataDto {
    pub app_version: String,
    /// `std::env::consts::OS` — "linux", "windows", etc.
    pub platform: String,
    pub chdman: serde_json::Value,
    pub chdman_runtime: ToolRuntimeDto,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDto {
    pub path: String,
    pub name: String,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDiskUsageDto {
    pub scope_path: String,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    /// "sftpStatvfs" | "sshDf" | "unsupported" | "unavailable"
    pub method: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableShell {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalDataPayload {
    pub id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExitPayload {
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCwdPayload {
    pub id: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFoldersDto {
    pub home: Option<String>,
    pub desktop: Option<String>,
    pub documents: Option<String>,
    pub downloads: Option<String>,
    pub pictures: Option<String>,
    pub music: Option<String>,
    pub videos: Option<String>,
}

// ── Compression ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionOptionsPayload {
    /// "zip" | "7z" | "rar"
    pub format: String,
    /// Archive filename stem (without extension)
    pub archive_name: String,
    /// "same" = alongside first source; "custom" = use destination_path
    pub destination_mode: String,
    pub destination_path: Option<String>,
    /// 0=store, 1=fastest … 9=maximum
    pub compression_level: u8,
    pub delete_originals: bool,
    pub overwrite: bool,
    /// When not overwriting and the archive name collides, auto-rename instead of failing.
    #[serde(default)]
    pub rename_on_conflict: bool,
    #[serde(default)]
    pub remote_destination: Option<String>,
    #[serde(default)]
    pub remote_transfer: Option<RemoteTransferPolicy>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionCapabilitiesDto {
    pub can_compress_zip: bool,
    pub can_compress_7z: bool,
    pub can_compress_rar: bool,
}

// ── M3U generation ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uScanOptions {
    /// Only produce M3U for groups with 2+ discs
    pub multi_disc_only: bool,
    /// Recurse into subdirectories
    pub recursive: bool,
    /// "sameFolder" | "scanRoot" | "currentDir" | "custom"
    pub output_location: String,
    /// Required when output_location == "custom" or "currentDir"
    pub custom_output_path: Option<String>,
    /// Emit relative paths in the M3U (recommended). False → absolute.
    pub use_relative_paths: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uEntryDto {
    /// Full absolute path (for display)
    pub absolute_path: String,
    /// Path written inside the M3U file (relative or absolute)
    pub m3u_path: String,
    /// Extracted disc number (0 = single/undiscovered)
    pub disc_number: u32,
    /// Lowercase file extension
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uWarningDto {
    /// "missingDisc" | "duplicateDisc" | "mixedFormats" | "m3uExists"
    pub kind: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uGroupDto {
    /// Stable string id (base_name + parent for deduplication)
    pub id: String,
    pub base_name: String,
    /// Where the M3U file will be written
    pub output_path: String,
    pub m3u_exists: bool,
    pub entries: Vec<M3uEntryDto>,
    pub warnings: Vec<M3uWarningDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uGenerateGroupPayload {
    pub output_path: String,
    pub base_name: String,
    /// Lines to write in the M3U (already resolved relative/absolute)
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uGeneratePayload {
    pub groups: Vec<M3uGenerateGroupPayload>,
    pub overwrite: bool,
    /// When not overwriting and the .m3u already exists, auto-rename instead of skipping.
    #[serde(default)]
    pub rename_on_conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uFailureDto {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct M3uGenerateResultDto {
    pub created: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<M3uFailureDto>,
}

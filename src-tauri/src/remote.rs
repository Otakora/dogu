use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read},
    net::TcpStream,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
    sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local};
use remotefs::{
    File as RemoteFile, RemoteFs,
    fs::{
        FileType, Metadata as RemoteMetadata, ReadStream, RemoteError, RemoteErrorType, UnixPex,
        UnixPexClass, Welcome,
    },
};
use remotefs_ftp::FtpFs;
use remotefs_smb::SmbFs;
#[cfg(target_family = "unix")]
use remotefs_smb::{SmbCredentials, SmbOptions};
#[cfg(target_family = "windows")]
use remotefs_smb::SmbCredentials;
use remotefs_ssh::{LibSsh2Session, ScpFs, SftpFs, SshOpts};
use rustls::{ClientConfig, RootCertStore};
use serde::{Deserialize, Serialize};
use ssh2::Session as Ssh2Session;
use suppaftp::{
    RustlsConnector, RustlsFtpStream,
    list::{File as FtpListFile, PosixPexQuery},
};
use tauri::{AppHandle, Manager};
use webpki_roots::TLS_SERVER_ROOTS;

use crate::{
    models::{
        ActiveConnectionDto, ConnectionOpenResultDto, ConnectionProfileDto, ConnectionProfilePayload,
        EntryDto, PropertiesSummaryDto, SummaryOptionsPayload,
    },
    ops,
};

const REMOTE_PREFIX: &str = "remote://";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileFile {
    profiles: Vec<ConnectionProfileDto>,
}

#[derive(Clone)]
pub struct RemoteState {
    pub inner: Arc<RemoteManager>,
}

pub struct RemoteManager {
    profiles_path: PathBuf,
    temp_root: PathBuf,
    profiles: Mutex<Vec<ConnectionProfileDto>>,
    sessions: Mutex<HashMap<String, RemoteSession>>,
    next_id: AtomicU64,
}

struct RemoteSession {
    connection: ActiveConnectionDto,
    profile: ConnectionProfileDto,
    root_provider_path: String,
    fs: Box<dyn RemoteFs + Send>,
}

struct SessionAccess<'a> {
    guard: std::sync::MutexGuard<'a, HashMap<String, RemoteSession>>,
    session_id: String,
}

impl<'a> Deref for SessionAccess<'a> {
    type Target = RemoteSession;

    fn deref(&self) -> &Self::Target {
        self.guard
            .get(&self.session_id)
            .expect("remote session disappeared while borrowed")
    }
}

impl<'a> DerefMut for SessionAccess<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .get_mut(&self.session_id)
            .expect("remote session disappeared while mutably borrowed")
    }
}

impl RemoteState {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .context("No se pudo resolver la carpeta de datos de la aplicacion")?;
        fs::create_dir_all(&app_data_dir)?;
        let temp_root = app_data_dir.join("remote-cache");
        fs::create_dir_all(&temp_root)?;
        let profiles_path = app_data_dir.join("connection_profiles.json");
        Ok(Self {
            inner: Arc::new(RemoteManager {
                profiles: Mutex::new(load_profiles(&profiles_path)?),
                sessions: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                profiles_path,
                temp_root,
            }),
        })
    }
}

impl RemoteManager {
    pub fn is_remote_path(path: &str) -> bool {
        path.starts_with(REMOTE_PREFIX)
    }

    pub fn list_profiles(&self) -> Result<Vec<ConnectionProfileDto>> {
        Ok(self.lock_profiles()?.clone())
    }

    pub fn save_profile(&self, payload: ConnectionProfilePayload) -> Result<ConnectionProfileDto> {
        validate_connection_profile_payload(&payload)?;
        let mut profiles = self.lock_profiles()?;
        let profile_id = payload
            .id
            .clone()
            .unwrap_or_else(|| format!("profile-{}", self.next_id.fetch_add(1, Ordering::Relaxed)));
        let trusted_fingerprints = profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| profile.trusted_fingerprints.clone())
            .unwrap_or_default();
        let profile = build_profile_from_payload(payload, profile_id.clone(), trusted_fingerprints)?;
        if let Some(existing) = profiles.iter_mut().find(|item| item.id == profile_id) {
            *existing = profile.clone();
        } else {
            profiles.push(profile.clone());
        }
        save_profiles(&self.profiles_path, &profiles)?;
        Ok(profile)
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<()> {
        let mut profiles = self.lock_profiles()?;
        profiles.retain(|profile| profile.id != profile_id);
        save_profiles(&self.profiles_path, &profiles)?;
        Ok(())
    }

    pub fn list_active_connections(&self) -> Result<Vec<ActiveConnectionDto>> {
        Ok(self
            .lock_sessions()?
            .values()
            .map(|session| session.connection.clone())
            .collect())
    }

    pub fn connect_profile(&self, profile_id: &str, trust_current_fingerprint: bool) -> Result<ConnectionOpenResultDto> {
        if let Some(existing) = self
            .lock_sessions()?
            .values()
            .find(|session| session.profile.id == profile_id)
            .map(|session| session.connection.clone())
        {
            return Ok(ConnectionOpenResultDto {
                connected: true,
                requires_trust: false,
                fingerprint: None,
                message: Some("La conexion ya estaba activa.".to_string()),
                connection: Some(existing),
            });
        }

        let profile = self
            .lock_profiles()?
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| anyhow!("No se encontro el perfil remoto solicitado."))?;

        if profile.protocol == "ssh" {
            if let Some(fingerprint) = probe_ssh_fingerprint(&profile)
                .map_err(|error| anyhow!(friendly_connection_error(&profile, "fingerprint", &error.to_string())))?
            {
                if !profile.trusted_fingerprints.iter().any(|item| item == &fingerprint) {
                    if !trust_current_fingerprint {
                        return Ok(ConnectionOpenResultDto {
                            connected: false,
                            requires_trust: true,
                            fingerprint: Some(fingerprint),
                            message: Some("La huella SSH del host no esta guardada como confiable.".to_string()),
                            connection: None,
                        });
                    }
                    self.persist_fingerprint(&profile.id, &fingerprint)?;
                }
            }
        }

        let session_id = format!("session-{}", self.next_id.fetch_add(1, Ordering::Relaxed));
        let (mut remote_fs, provider_root) = self
            .build_remote_fs(&profile)
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        remote_fs
            .connect()
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        validate_remote_root(&profile, &mut *remote_fs, &provider_root)?;
        let connection = ActiveConnectionDto {
            session_id: session_id.clone(),
            profile_id: profile.id.clone(),
            label: profile.label.clone(),
            protocol: profile.protocol.clone(),
            host: profile.host.clone(),
            root_path: build_remote_virtual_path(&session_id, "/"),
            display_path: profile.start_path.clone(),
            detail: describe_profile(&profile),
        };
        self.lock_sessions()?.insert(
            session_id,
            RemoteSession {
                connection: connection.clone(),
                profile,
                root_provider_path: provider_root,
                fs: remote_fs,
            },
        );
        Ok(ConnectionOpenResultDto {
            connected: true,
            requires_trust: false,
            fingerprint: None,
            message: Some("Conexion remota abierta.".to_string()),
            connection: Some(connection),
        })
    }

    pub fn test_profile_payload(
        &self,
        payload: ConnectionProfilePayload,
        trust_current_fingerprint: bool,
    ) -> Result<ConnectionOpenResultDto> {
        validate_connection_profile_payload(&payload)?;
        let profile = build_profile_from_payload(
            payload,
            "draft-profile".to_string(),
            Vec::new(),
        )?;

        if profile.protocol == "ssh" {
            if let Some(fingerprint) = probe_ssh_fingerprint(&profile)
                .map_err(|error| anyhow!(friendly_connection_error(&profile, "fingerprint", &error.to_string())))?
            {
                if !trust_current_fingerprint {
                    return Ok(ConnectionOpenResultDto {
                        connected: false,
                        requires_trust: true,
                        fingerprint: Some(fingerprint),
                        message: Some("La huella SSH del host no esta guardada como confiable.".to_string()),
                        connection: None,
                    });
                }
            }
        }

        let (mut remote_fs, provider_root) = self
            .build_remote_fs(&profile)
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        remote_fs
            .connect()
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        validate_remote_root(&profile, &mut *remote_fs, &provider_root)?;
        let _ = remote_fs.disconnect();
        Ok(ConnectionOpenResultDto {
            connected: true,
            requires_trust: false,
            fingerprint: None,
            message: Some("Conexion verificada correctamente.".to_string()),
            connection: None,
        })
    }

    pub fn test_profile(&self, profile_id: &str, trust_current_fingerprint: bool) -> Result<ConnectionOpenResultDto> {
        let profile = self
            .lock_profiles()?
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| anyhow!("No se encontro el perfil remoto solicitado."))?;

        if profile.protocol == "ssh" {
            if let Some(fingerprint) = probe_ssh_fingerprint(&profile)
                .map_err(|error| anyhow!(friendly_connection_error(&profile, "fingerprint", &error.to_string())))?
            {
                if !profile.trusted_fingerprints.iter().any(|item| item == &fingerprint) {
                    if !trust_current_fingerprint {
                        return Ok(ConnectionOpenResultDto {
                            connected: false,
                            requires_trust: true,
                            fingerprint: Some(fingerprint),
                            message: Some("La huella SSH del host no esta guardada como confiable.".to_string()),
                            connection: None,
                        });
                    }
                    self.persist_fingerprint(&profile.id, &fingerprint)?;
                }
            }
        }

        let (mut remote_fs, provider_root) = self
            .build_remote_fs(&profile)
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        remote_fs
            .connect()
            .map_err(|error| anyhow!(friendly_connection_error(&profile, "connect", &error.to_string())))?;
        validate_remote_root(&profile, &mut *remote_fs, &provider_root)?;
        let _ = remote_fs.disconnect();
        Ok(ConnectionOpenResultDto {
            connected: true,
            requires_trust: false,
            fingerprint: None,
            message: Some("Conexion verificada correctamente.".to_string()),
            connection: None,
        })
    }

    pub fn disconnect_connection(&self, session_id: &str) -> Result<()> {
        if let Some(mut session) = self.lock_sessions()?.remove(session_id) {
            let _ = session.fs.disconnect();
        }
        Ok(())
    }

    pub fn list_children(&self, path: &str) -> Result<Vec<EntryDto>> {
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let provider_path = session.resolve_provider_path(&logical_path);
        let children = session
            .fs
            .list_dir(Path::new(&provider_path))
            .map_err(|error| anyhow!(error.to_string()))?;
        let mut entries = Vec::new();
        for child in children {
            let child_logical = session.provider_child_to_logical(&provider_path, child.path())?;
            entries.push(session.entry_from_remote_file(child, &child_logical)?);
        }
        entries.sort_by(|left, right| {
            left.is_dir
                .cmp(&right.is_dir)
                .reverse()
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(entries)
    }

    pub fn inspect_path(&self, path: &str) -> Result<EntryDto> {
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        if logical_path == "/" {
            return Ok(EntryDto {
                path: build_remote_virtual_path(&session_id, "/"),
                name: session.connection.label.clone(),
                is_dir: true,
                size: 0,
                size_label: "Raiz remota".to_string(),
                modified_ts: 0,
                modified_label: "-".to_string(),
                extension: String::new(),
                has_children: true,
                has_directory_children: true,
                location_kind: "remote".to_string(),
                display_path: session.profile.start_path.clone(),
                root_label: Some(session.connection.label.clone()),
            });
        }
        let provider_path = session.resolve_provider_path(&logical_path);
        let file = session
            .fs
            .stat(Path::new(&provider_path))
            .map_err(|error| anyhow!(error.to_string()))?;
        session.entry_from_remote_file(file, &logical_path)
    }

    pub fn search_entries(&self, path: &str, query: &str, recursive: bool) -> Result<Vec<EntryDto>> {
        let lowered = query.trim().to_lowercase();
        if lowered.is_empty() {
            return Ok(Vec::new());
        }
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let mut results = Vec::new();
        collect_remote_matches(&mut session, &logical_path, &lowered, recursive, &mut results)?;
        results.sort_by(|left, right| {
            left.is_dir
                .cmp(&right.is_dir)
                .reverse()
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(results)
    }

    pub fn summarize_paths(&self, paths: &[String], options: &SummaryOptionsPayload) -> Result<PropertiesSummaryDto> {
        let first = paths.first().ok_or_else(|| anyhow!("No hay rutas para resumir."))?;
        let (session_id, _) = parse_remote_virtual_path(first).ok_or_else(|| anyhow!("Ruta remota invalida."))?;
        let mut session = self.get_session_mut(&session_id)?;

        let mut files = 0usize;
        let mut directories = 0usize;
        let mut total_size = 0u64;
        let mut lines = Vec::new();
        for path in paths {
            let (_, logical_path) = parse_remote_virtual_path(path).ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
            let provider_path = session.resolve_provider_path(&logical_path);
            let item = session
                .fs
                .stat(Path::new(&provider_path))
                .map_err(|error| anyhow!(error.to_string()))?;
            if item.is_dir() {
                let (dir_size, file_count, dir_count) =
                    summarize_remote_dir(&mut session, &logical_path, options.max_depth, 0)?;
                total_size += dir_size;
                files += file_count;
                directories += dir_count + 1;
                lines.push(format!(
                    "[DIR] {} | {} | {} | {} ficheros, {} subcarpetas",
                    logical_leaf_name(&logical_path),
                    logical_path,
                    ops::format_size(dir_size),
                    file_count,
                    dir_count
                ));
            } else {
                total_size += item.metadata().size;
                files += 1;
                lines.push(format!(
                    "[FILE] {} | {} | {}",
                    logical_leaf_name(&logical_path),
                    logical_path,
                    ops::format_size(item.metadata().size)
                ));
            }
        }

        Ok(PropertiesSummaryDto {
            count: paths.len(),
            files,
            directories,
            total_size,
            total_size_label: ops::format_size(total_size),
            lines,
        })
    }

    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        let (session_id, parent_logical) = parse_remote_virtual_path(parent)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {parent}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let logical_path = join_logical_path(&parent_logical, name);
        let provider_path = session.resolve_provider_path(&logical_path);
        session
            .fs
            .create_dir(Path::new(&provider_path), UnixPex::from(0o755))
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(build_remote_virtual_path(&session_id, &logical_path))
    }

    pub fn create_file(&self, parent: &str, name: &str) -> Result<String> {
        let (session_id, parent_logical) = parse_remote_virtual_path(parent)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {parent}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let final_name = if Path::new(name).extension().is_none() {
            format!("{name}.txt")
        } else {
            name.to_string()
        };
        let logical_path = join_logical_path(&parent_logical, &final_name);
        let provider_path = session.resolve_provider_path(&logical_path);
        let metadata = empty_file_metadata();
        session
            .fs
            .create_file(Path::new(&provider_path), &metadata, Box::new(Cursor::new(Vec::<u8>::new())))
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(build_remote_virtual_path(&session_id, &logical_path))
    }

    pub fn rename_path(&self, path: &str, new_name: &str) -> Result<String> {
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let parent = logical_parent_path(&logical_path).unwrap_or_else(|| "/".to_string());
        let target_logical = join_logical_path(&parent, new_name);
        let source_provider = session.resolve_provider_path(&logical_path);
        let target_provider = session.resolve_provider_path(&target_logical);
        session
            .fs
            .mov(Path::new(&source_provider), Path::new(&target_provider))
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(build_remote_virtual_path(&session_id, &target_logical))
    }

    pub fn open_path(&self, _app: &AppHandle, path: &str, with_dialog: bool) -> Result<()> {
        let local_temp = self.download_remote_file(path)?;
        if with_dialog {
            crate::ops::open_with_dialog(&local_temp)?;
        } else {
            crate::ops::open_path(&local_temp)?;
        }
        Ok(())
    }

    pub fn delete_paths(&self, app: &AppHandle, job_id: &str, paths: Vec<String>) -> Result<()> {
        let total = paths.len().max(1) as f64;
        for (index, path) in paths.iter().enumerate() {
            self.delete_one(app, job_id, path, 0)?;
            ops::emit_progress(
                app,
                job_id,
                (index + 1) as f64 / total,
                format!("Eliminando {path}"),
            )?;
        }
        Ok(())
    }

    pub fn copy_or_move_paths(
        &self,
        app: &AppHandle,
        job_id: &str,
        paths: Vec<String>,
        destination: String,
        operation: &str,
        overwrite: bool,
    ) -> Result<()> {
        let total = paths.len().max(1) as f64;
        for (index, source) in paths.iter().enumerate() {
            ops::emit_log(
                app,
                job_id,
                format!(
                    "{} {} -> {}",
                    if operation == "cut" { "Moviendo" } else { "Copiando" },
                    source,
                    destination
                ),
            )?;
            let source_is_remote = Self::is_remote_path(source);
            let destination_is_remote = Self::is_remote_path(&destination);
            match (source_is_remote, destination_is_remote) {
                (true, true) => self.transfer_remote_to_remote(source, &destination, operation == "cut", overwrite)?,
                (true, false) => self.transfer_remote_to_local(source, &destination, operation == "cut", overwrite)?,
                (false, true) => self.transfer_local_to_remote(source, &destination, operation == "cut", overwrite)?,
                (false, false) => return Err(anyhow!("La operacion no requiere el gestor remoto.")),
            }
            ops::emit_progress(
                app,
                job_id,
                (index + 1) as f64 / total,
                format!(
                    "{} {}",
                    if operation == "cut" { "Moviendo" } else { "Copiando" },
                    source
                ),
            )?;
        }
        Ok(())
    }

    fn transfer_remote_to_local(
        &self,
        source: &str,
        destination: &str,
        move_after: bool,
        overwrite: bool,
    ) -> Result<()> {
        let (session_id, source_logical) = parse_remote_virtual_path(source)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {source}"))?;
        let source_name = logical_leaf_name(&source_logical);
        let target_root = PathBuf::from(destination);
        let target_path = target_root.join(&source_name);
        self.copy_remote_entry_to_local_path(&session_id, &source_logical, &target_path, overwrite)?;
        if move_after {
            let mut session = self.get_session_mut(&session_id)?;
            let source_provider = session.resolve_provider_path(&source_logical);
            remove_remote_target(&mut *session.fs, Path::new(&source_provider))?;
        }
        Ok(())
    }

    fn transfer_local_to_remote(
        &self,
        source: &str,
        destination: &str,
        move_after: bool,
        overwrite: bool,
    ) -> Result<()> {
        let source_path = PathBuf::from(source);
        if !source_path.exists() {
            return Err(anyhow!("La ruta local no existe: {}", source_path.display()));
        }
        let (session_id, destination_logical) = parse_remote_virtual_path(destination)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {destination}"))?;
        let source_name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("No se pudo resolver el nombre del elemento local."))?;
        let target_logical = join_logical_path(&destination_logical, source_name);
        self.copy_local_entry_to_remote_path(&source_path, &session_id, &target_logical, overwrite)?;
        if move_after {
            remove_local_target(&source_path)?;
        }
        Ok(())
    }

    fn download_remote_file(&self, path: &str) -> Result<PathBuf> {
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let provider_path = session.resolve_provider_path(&logical_path);
        let buffer = read_remote_bytes(&mut *session.fs, Path::new(&provider_path))?;
        let target_dir = self.temp_root.join(&session.profile.id);
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(logical_leaf_name(&logical_path));
        fs::write(&target, buffer)?;
        Ok(target)
    }

    fn delete_one(&self, app: &AppHandle, job_id: &str, path: &str, depth: usize) -> Result<()> {
        let indent = "  ".repeat(depth);
        ops::emit_log(app, job_id, format!("{indent}{path}"))?;
        let (session_id, logical_path) = parse_remote_virtual_path(path)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {path}"))?;
        let mut session = self.get_session_mut(&session_id)?;
        let provider_path = session.resolve_provider_path(&logical_path);
        let item = session
            .fs
            .stat(Path::new(&provider_path))
            .map_err(|error| anyhow!(error.to_string()))?;
        if item.is_dir() {
            let children = session
                .fs
                .list_dir(Path::new(&provider_path))
                .map_err(|error| anyhow!(error.to_string()))?;
            let child_paths = children
                .into_iter()
                .map(|child| {
                    session
                        .provider_child_to_logical(&provider_path, child.path())
                        .map(|logical| build_remote_virtual_path(&session_id, &logical))
                })
                .collect::<Result<Vec<_>>>()?;
            for child_path in child_paths {
                self.delete_one(app, job_id, &child_path, depth + 1)?;
            }
            let mut session = self.get_session_mut(&session_id)?;
            let provider_path = session.resolve_provider_path(&logical_path);
            session
                .fs
                .remove_dir(Path::new(&provider_path))
                .map_err(|error| anyhow!(error.to_string()))?;
        } else {
            session
                .fs
                .remove_file(Path::new(&provider_path))
                .map_err(|error| anyhow!(error.to_string()))?;
        }
        Ok(())
    }

    fn transfer_remote_to_remote(&self, source: &str, destination: &str, move_after: bool, overwrite: bool) -> Result<()> {
        let (source_session_id, source_logical) = parse_remote_virtual_path(source)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {source}"))?;
        let (destination_session_id, destination_logical) = parse_remote_virtual_path(destination)
            .ok_or_else(|| anyhow!("Ruta remota invalida: {destination}"))?;
        let source_name = logical_leaf_name(&source_logical);
        let target_logical = join_logical_path(&destination_logical, &source_name);

        if source_session_id == destination_session_id {
            let mut session = self.get_session_mut(&source_session_id)?;
            let source_provider = session.resolve_provider_path(&source_logical);
            let target_provider = session.resolve_provider_path(&target_logical);
            if overwrite && session.fs.exists(Path::new(&target_provider)).unwrap_or(false) {
                remove_remote_target(&mut *session.fs, Path::new(&target_provider))?;
            }
            if move_after {
                session
                    .fs
                    .mov(Path::new(&source_provider), Path::new(&target_provider))
                    .map_err(|error| anyhow!(error.to_string()))?;
            } else {
                session
                    .fs
                    .copy(Path::new(&source_provider), Path::new(&target_provider))
                    .map_err(|error| anyhow!(error.to_string()))?;
            }
            return Ok(());
        }

        let is_directory = {
            let mut session = self.get_session_mut(&source_session_id)?;
            let source_provider = session.resolve_provider_path(&source_logical);
            session
                .fs
                .stat(Path::new(&source_provider))
                .map_err(|error| anyhow!(error.to_string()))?
                .is_dir()
        };
        if is_directory {
            self.copy_remote_dir_between_sessions(&source_session_id, &source_logical, &destination_session_id, &target_logical, overwrite)?;
        } else {
            self.copy_remote_file_between_sessions(&source_session_id, &source_logical, &destination_session_id, &target_logical, overwrite)?;
        }
        if move_after {
            let mut session = self.get_session_mut(&source_session_id)?;
            let source_provider = session.resolve_provider_path(&source_logical);
            remove_remote_target(&mut *session.fs, Path::new(&source_provider))?;
        }
        Ok(())
    }

    fn copy_remote_dir_between_sessions(
        &self,
        source_session_id: &str,
        source_logical: &str,
        destination_session_id: &str,
        destination_logical: &str,
        overwrite: bool,
    ) -> Result<()> {
        {
            let mut session = self.get_session_mut(destination_session_id)?;
            let target_provider = session.resolve_provider_path(destination_logical);
            if !session.fs.exists(Path::new(&target_provider)).unwrap_or(false) {
                session
                    .fs
                    .create_dir(Path::new(&target_provider), UnixPex::from(0o755))
                    .map_err(|error| anyhow!(error.to_string()))?;
            }
        }
        let children = self.list_children(&build_remote_virtual_path(source_session_id, source_logical))?;
        for child in children {
            let child_logical = parse_remote_virtual_path(&child.path)
                .map(|(_, logical)| logical)
                .ok_or_else(|| anyhow!("Ruta remota hija invalida."))?;
            let target_child = join_logical_path(destination_logical, &child.name);
            if child.is_dir {
                self.copy_remote_dir_between_sessions(
                    source_session_id,
                    &child_logical,
                    destination_session_id,
                    &target_child,
                    overwrite,
                )?;
            } else {
                self.copy_remote_file_between_sessions(
                    source_session_id,
                    &child_logical,
                    destination_session_id,
                    &target_child,
                    overwrite,
                )?;
            }
        }
        Ok(())
    }

    fn copy_remote_file_between_sessions(
        &self,
        source_session_id: &str,
        source_logical: &str,
        destination_session_id: &str,
        destination_logical: &str,
        overwrite: bool,
    ) -> Result<()> {
        let source_bytes = {
            let mut session = self.get_session_mut(source_session_id)?;
            let source_provider = session.resolve_provider_path(source_logical);
            read_remote_bytes(&mut *session.fs, Path::new(&source_provider))?
        };
        let mut session = self.get_session_mut(destination_session_id)?;
        let destination_provider = session.resolve_provider_path(destination_logical);
        if overwrite && session.fs.exists(Path::new(&destination_provider)).unwrap_or(false) {
            remove_remote_target(&mut *session.fs, Path::new(&destination_provider))?;
        }
        session
            .fs
            .create_file(
                Path::new(&destination_provider),
                &file_metadata(source_bytes.len() as u64),
                Box::new(Cursor::new(source_bytes)),
            )
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(())
    }

    fn copy_remote_entry_to_local_path(
        &self,
        session_id: &str,
        source_logical: &str,
        local_target: &Path,
        overwrite: bool,
    ) -> Result<()> {
        let source_name = logical_leaf_name(source_logical);
        let (is_directory, source_provider) = {
            let mut session = self.get_session_mut(session_id)?;
            let source_provider = session.resolve_provider_path(source_logical);
            let is_directory = session
                .fs
                .stat(Path::new(&source_provider))
                .map_err(|error| anyhow!(error.to_string()))?
                .is_dir();
            (is_directory, source_provider)
        };

        if local_target.exists() {
            if !overwrite {
                return Err(anyhow!("El destino local ya existe: {}", local_target.display()));
            }
            remove_local_target(local_target)?;
        }

        if is_directory {
            fs::create_dir_all(local_target)?;
            let children = self.list_children(&build_remote_virtual_path(session_id, source_logical))?;
            for child in children {
                let child_logical = parse_remote_virtual_path(&child.path)
                    .map(|(_, logical)| logical)
                    .ok_or_else(|| anyhow!("Ruta remota hija invalida."))?;
                self.copy_remote_entry_to_local_path(
                    session_id,
                    &child_logical,
                    &local_target.join(child.name),
                    overwrite,
                )?;
            }
            return Ok(());
        }

        let bytes = {
            let mut session = self.get_session_mut(session_id)?;
            read_remote_bytes(&mut *session.fs, Path::new(&source_provider))?
        };
        if let Some(parent) = local_target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(local_target, bytes)?;
        let _ = source_name;
        Ok(())
    }

    fn copy_local_entry_to_remote_path(
        &self,
        local_source: &Path,
        session_id: &str,
        destination_logical: &str,
        overwrite: bool,
    ) -> Result<()> {
        let mut session = self.get_session_mut(session_id)?;
        let destination_provider = session.resolve_provider_path(destination_logical);
        let destination_exists = session.fs.exists(Path::new(&destination_provider)).unwrap_or(false);
        if destination_exists {
            if !overwrite {
                return Err(anyhow!("El destino remoto ya existe: {destination_logical}"));
            }
            remove_remote_target(&mut *session.fs, Path::new(&destination_provider))?;
        }

        if local_source.is_dir() {
            session
                .fs
                .create_dir(Path::new(&destination_provider), UnixPex::from(0o755))
                .map_err(|error| anyhow!(error.to_string()))?;
            drop(session);
            for entry in fs::read_dir(local_source)? {
                let entry = entry?;
                let entry_path = entry.path();
                let name = entry_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| anyhow!("No se pudo leer el nombre de una entrada local."))?;
                let child_target = join_logical_path(destination_logical, name);
                self.copy_local_entry_to_remote_path(&entry_path, session_id, &child_target, overwrite)?;
            }
            return Ok(());
        }

        let bytes = fs::read(local_source)?;
        session
            .fs
            .create_file(
                Path::new(&destination_provider),
                &file_metadata(bytes.len() as u64),
                Box::new(Cursor::new(bytes)),
            )
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(())
    }

    fn build_remote_fs(&self, profile: &ConnectionProfileDto) -> Result<(Box<dyn RemoteFs + Send>, String)> {
        match profile.protocol.as_str() {
            "ssh" => {
                let opts = SshOpts::new(profile.host.clone())
                    .port(profile.port)
                    .username(profile.username.clone())
                    .password(profile.password.clone());
                let fs: Box<dyn RemoteFs + Send> = if profile.ssh_mode == "scp" {
                    Box::new(ScpFs::<LibSsh2Session>::libssh2(opts))
                } else {
                    Box::new(SftpFs::<LibSsh2Session>::libssh2(opts))
                };
                Ok((fs, profile.start_path.clone()))
            }
            "smb" => self.build_smb_fs(profile),
            "ftp" => {
                let mut fs = FtpFs::new(profile.host.clone(), profile.port)
                    .username(profile.username.clone())
                    .password(profile.password.clone());
                fs = if profile.ftp_mode == "active" { fs.active_mode() } else { fs.passive_mode() };
                Ok((Box::new(fs), profile.start_path.clone()))
            }
            "ftps" => {
                if profile.ftp_secure_implicit {
                    Ok((
                        Box::new(ImplicitFtpsFs::new(
                            profile.host.clone(),
                            profile.port,
                            profile.username.clone(),
                            profile.password.clone(),
                            profile.ftp_mode.clone(),
                            profile.ftp_accept_invalid_certificates,
                            profile.ftp_accept_invalid_hostnames,
                        )),
                        profile.start_path.clone(),
                    ))
                } else {
                    let mut fs = FtpFs::new(profile.host.clone(), profile.port)
                        .username(profile.username.clone())
                        .password(profile.password.clone());
                    fs = if profile.ftp_mode == "active" { fs.active_mode() } else { fs.passive_mode() };
                    fs = fs.secure();
                    Ok((Box::new(fs), profile.start_path.clone()))
                }
            }
            other => Err(anyhow!("Protocolo remoto no soportado: {other}")),
        }
    }

    fn build_smb_fs(&self, profile: &ConnectionProfileDto) -> Result<(Box<dyn RemoteFs + Send>, String)> {
        #[cfg(target_family = "unix")]
        {
            let credentials = SmbCredentials::default()
                .server(format!("smb://{}", profile.host))
                .share(format!("/{}", profile.share.trim_matches('/')))
                .username(profile.username.clone())
                .password(profile.password.clone())
                .workgroup(profile.workgroup.clone());
            let fs = SmbFs::try_new(
                credentials,
                SmbOptions::default().one_share_per_server(true).case_sensitive(true),
            )
            .map_err(|error| anyhow!(error.to_string()))?;
            return Ok((Box::new(fs), profile.start_path.clone()));
        }
        #[cfg(target_family = "windows")]
        {
            let mut credentials = SmbCredentials::new(profile.host.clone(), profile.share.clone());
            if !profile.username.is_empty() {
                credentials = credentials.username(profile.username.clone());
            }
            if !profile.password.is_empty() {
                credentials = credentials.password(profile.password.clone());
            }
            let fs = SmbFs::new(credentials);
            return Ok((Box::new(fs), normalize_windows_share_path(&profile.start_path)));
        }
        #[allow(unreachable_code)]
        Err(anyhow!("SMB no esta soportado en esta plataforma"))
    }

    fn persist_fingerprint(&self, profile_id: &str, fingerprint: &str) -> Result<()> {
        let mut profiles = self.lock_profiles()?;
        if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == profile_id) {
            if !profile.trusted_fingerprints.iter().any(|item| item == fingerprint) {
                profile.trusted_fingerprints.push(fingerprint.to_string());
                save_profiles(&self.profiles_path, &profiles)?;
            }
        }
        Ok(())
    }

    fn get_session_mut(&self, session_id: &str) -> Result<SessionAccess<'_>> {
        let guard = self.lock_sessions()?;
        if !guard.contains_key(session_id) {
            return Err(anyhow!("La sesion remota ya no esta activa."));
        }
        Ok(SessionAccess {
            guard,
            session_id: session_id.to_string(),
        })
    }

    fn lock_profiles(&self) -> Result<std::sync::MutexGuard<'_, Vec<ConnectionProfileDto>>> {
        self.profiles
            .lock()
            .map_err(|_| anyhow!("No se pudieron bloquear los perfiles remotos"))
    }

    fn lock_sessions(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, RemoteSession>>> {
        self.sessions
            .lock()
            .map_err(|_| anyhow!("No se pudieron bloquear las sesiones remotas"))
    }
}

impl RemoteSession {
    fn resolve_provider_path(&self, logical_path: &str) -> String {
        if logical_path == "/" {
            return self.root_provider_path.clone();
        }
        let root = self.root_provider_path.trim_end_matches(['/', '\\']);
        let suffix = logical_path.trim_start_matches('/').replace('\\', "/");
        if root.is_empty() || root == "/" || root == "\\" {
            if self.profile.protocol == "smb" && cfg!(target_family = "windows") {
                format!("\\{}", suffix.replace('/', "\\"))
            } else {
                format!("/{}", suffix)
            }
        } else if self.profile.protocol == "smb" && cfg!(target_family = "windows") {
            format!("{root}\\{}", suffix.replace('/', "\\"))
        } else {
            format!("{root}/{}", suffix)
        }
    }

    fn provider_child_to_logical(&self, parent_provider_path: &str, child_provider_path: &Path) -> Result<String> {
        provider_child_to_logical(parent_provider_path, child_provider_path)
    }

    fn entry_from_remote_file(&mut self, file: RemoteFile, logical_path: &str) -> Result<EntryDto> {
        let is_dir = file.is_dir();
        let provider_path = self.resolve_provider_path(logical_path);
        let (size, size_label, has_children, has_directory_children) = if is_dir {
            let children = self.fs.list_dir(Path::new(&provider_path)).unwrap_or_default();
            let count = children.len();
            let label = match count {
                0 => "Vacia".to_string(),
                1 => "1 elemento".to_string(),
                n => format!("{n} elementos"),
            };
            (0, label, count > 0, children.iter().any(|child| child.is_dir()))
        } else {
            let size = file.metadata().size;
            (size, ops::format_size(size), false, false)
        };
        let modified_ts = file
            .metadata()
            .modified
            .and_then(system_time_to_timestamp)
            .unwrap_or(0);
        let modified_label = file
            .metadata()
            .modified
            .map(format_system_time)
            .unwrap_or_else(|| "-".to_string());
        Ok(EntryDto {
            path: build_remote_virtual_path(&self.connection.session_id, logical_path),
            name: logical_leaf_name(logical_path),
            is_dir,
            size,
            size_label,
            modified_ts,
            modified_label,
            extension: if is_dir {
                String::new()
            } else {
                Path::new(logical_leaf_name(logical_path).as_str())
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| format!(".{}", value.to_ascii_lowercase()))
                    .unwrap_or_default()
            },
            has_children,
            has_directory_children,
            location_kind: "remote".to_string(),
            display_path: logical_path.to_string(),
            root_label: Some(self.connection.label.clone()),
        })
    }
}

struct ImplicitFtpsFs {
    stream: Option<RustlsFtpStream>,
    host: String,
    port: u16,
    username: String,
    password: String,
    mode: String,
    accept_invalid_certificates: bool,
    accept_invalid_hostnames: bool,
}

impl ImplicitFtpsFs {
    fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        mode: String,
        accept_invalid_certificates: bool,
        accept_invalid_hostnames: bool,
    ) -> Self {
        Self {
            stream: None,
            host,
            port,
            username,
            password,
            mode,
            accept_invalid_certificates,
            accept_invalid_hostnames,
        }
    }

    fn stream_mut(&mut self) -> Result<&mut RustlsFtpStream, RemoteError> {
        self.stream
            .as_mut()
            .ok_or_else(|| RemoteError::new(RemoteErrorType::NotConnected))
    }

    fn build_tls_connector(&self) -> Result<RustlsConnector, RemoteError> {
        if self.accept_invalid_certificates || self.accept_invalid_hostnames {
            return Err(RemoteError::new_ex(
                RemoteErrorType::UnsupportedFeature,
                "Las opciones de aceptar certificados/hostnames invalidos no estan disponibles con el backend FTPS actual.",
            ));
        }
        let mut root_store = RootCertStore::empty();
        root_store.extend(TLS_SERVER_ROOTS.iter().cloned());
        let config = std::sync::Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );
        Ok(config.into())
    }

    fn parse_list(&self, base: &Path, lines: Vec<String>) -> Vec<RemoteFile> {
        lines
            .into_iter()
            .filter_map(|line| FtpListFile::try_from(line).ok())
            .map(|entry| {
                let mut path = base.to_path_buf();
                path.push(entry.name());
                let file_type = if entry.is_symlink() {
                    FileType::Symlink
                } else if entry.is_directory() {
                    FileType::Directory
                } else {
                    FileType::File
                };
                RemoteFile {
                    path,
                    metadata: RemoteMetadata {
                        accessed: None,
                        created: None,
                        file_type,
                        gid: entry.gid(),
                        mode: Some(UnixPex::new(
                            ftp_pex_class(&entry, PosixPexQuery::Owner),
                            ftp_pex_class(&entry, PosixPexQuery::Group),
                            ftp_pex_class(&entry, PosixPexQuery::Others),
                        )),
                        modified: Some(entry.modified()),
                        size: entry.size() as u64,
                        symlink: None,
                        uid: None,
                    },
                }
            })
            .collect()
    }
}

impl RemoteFs for ImplicitFtpsFs {
    fn connect(&mut self) -> remotefs::RemoteResult<Welcome> {
        let connector = self.build_tls_connector()?;
        let mut stream = RustlsFtpStream::connect_secure_implicit(
            format!("{}:{}", self.host, self.port),
            connector,
            self.host.as_str(),
        )
        .map_err(|error| RemoteError::new_ex(RemoteErrorType::ConnectionError, error.to_string()))?;
        if self.mode == "active" {
            stream = stream.active_mode(Duration::from_secs(30));
        }
        stream
            .login(self.username.as_str(), self.password.as_str())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::AuthenticationFailed, error.to_string()))?;
        self.stream = Some(stream);
        Ok(Welcome::default())
    }

    fn disconnect(&mut self) -> remotefs::RemoteResult<()> {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.quit();
        }
        Ok(())
    }

    fn is_connected(&mut self) -> bool {
        self.stream.is_some()
    }

    fn pwd(&mut self) -> remotefs::RemoteResult<PathBuf> {
        self.stream_mut()?
            .pwd()
            .map(PathBuf::from)
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::ProtocolError, error.to_string()))
    }

    fn change_dir(&mut self, dir: &Path) -> remotefs::RemoteResult<PathBuf> {
        self.stream_mut()?
            .cwd(dir.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::ProtocolError, error.to_string()))?;
        self.pwd()
    }

    fn list_dir(&mut self, path: &Path) -> remotefs::RemoteResult<Vec<RemoteFile>> {
        let lines = self
            .stream_mut()?
            .list(Some(path.to_string_lossy().as_ref()))
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::ProtocolError, error.to_string()))?;
        Ok(self.parse_list(path, lines))
    }

    fn stat(&mut self, path: &Path) -> remotefs::RemoteResult<RemoteFile> {
        if path == Path::new("/") {
            return Ok(RemoteFile {
                path: PathBuf::from("/"),
                metadata: RemoteMetadata {
                    accessed: None,
                    created: None,
                    file_type: FileType::Directory,
                    gid: None,
                    mode: Some(UnixPex::from(0o755)),
                    modified: None,
                    size: 0,
                    symlink: None,
                    uid: None,
                },
            });
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("/"));
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        self.list_dir(parent)?
            .into_iter()
            .find(|entry| entry.path().file_name().and_then(|value| value.to_str()) == Some(name))
            .ok_or_else(|| RemoteError::new(RemoteErrorType::NoSuchFileOrDirectory))
    }

    fn setstat(&mut self, _path: &Path, _metadata: RemoteMetadata) -> remotefs::RemoteResult<()> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn exists(&mut self, path: &Path) -> remotefs::RemoteResult<bool> {
        Ok(self.stat(path).is_ok())
    }

    fn remove_file(&mut self, path: &Path) -> remotefs::RemoteResult<()> {
        self.stream_mut()?
            .rm(path.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))
    }

    fn remove_dir(&mut self, path: &Path) -> remotefs::RemoteResult<()> {
        self.stream_mut()?
            .rmdir(path.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))
    }

    fn create_dir(&mut self, path: &Path, _mode: UnixPex) -> remotefs::RemoteResult<()> {
        self.stream_mut()?
            .mkdir(path.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))
    }

    fn symlink(&mut self, _path: &Path, _target: &Path) -> remotefs::RemoteResult<()> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn copy(&mut self, src: &Path, dest: &Path) -> remotefs::RemoteResult<()> {
        let buffer = read_remote_bytes_remote(self, src)?;
        self.create_file(dest, &file_metadata(buffer.len() as u64), Box::new(Cursor::new(buffer)))?;
        Ok(())
    }

    fn mov(&mut self, src: &Path, dest: &Path) -> remotefs::RemoteResult<()> {
        self.stream_mut()?
            .rename(src.to_string_lossy().as_ref(), dest.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))
    }

    fn exec(&mut self, _cmd: &str) -> remotefs::RemoteResult<(u32, String)> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn append(&mut self, _path: &Path, _metadata: &RemoteMetadata) -> remotefs::RemoteResult<remotefs::fs::WriteStream> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn create(&mut self, _path: &Path, _metadata: &RemoteMetadata) -> remotefs::RemoteResult<remotefs::fs::WriteStream> {
        Err(RemoteError::new(RemoteErrorType::UnsupportedFeature))
    }

    fn open(&mut self, path: &Path) -> remotefs::RemoteResult<remotefs::fs::ReadStream> {
        let buffer = self
            .stream_mut()?
            .retr_as_buffer(path.to_string_lossy().as_ref())
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))?;
        Ok(ReadStream::from(Box::new(Cursor::new(buffer.into_inner())) as Box<dyn Read + Send>))
    }

    fn create_file(
        &mut self,
        path: &Path,
        _metadata: &RemoteMetadata,
        mut reader: Box<dyn Read + Send>,
    ) -> remotefs::RemoteResult<u64> {
        self.stream_mut()?
            .put_file(path.to_string_lossy().as_ref(), &mut reader)
            .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))
    }
}

fn required_trimmed(label: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("El campo {label} es obligatorio."));
    }
    Ok(trimmed.to_string())
}

fn build_profile_from_payload(
    payload: ConnectionProfilePayload,
    profile_id: String,
    trusted_fingerprints: Vec<String>,
) -> Result<ConnectionProfileDto> {
    Ok(ConnectionProfileDto {
        id: profile_id,
        label: required_trimmed("etiqueta", &payload.label)?,
        protocol: required_trimmed("protocolo", &payload.protocol)?,
        host: payload.host.trim().to_string(),
        port: payload.port.unwrap_or_else(|| default_port_for_protocol(&payload.protocol)),
        username: payload.username.trim().to_string(),
        password: payload.password,
        share: payload.share.trim().to_string(),
        workgroup: payload.workgroup.trim().to_string(),
        start_path: normalize_logical_path(&payload.start_path),
        ssh_mode: if payload.ssh_mode.trim().eq_ignore_ascii_case("scp") {
            "scp".to_string()
        } else {
            "sftp".to_string()
        },
        ftp_mode: if payload.ftp_mode.trim().eq_ignore_ascii_case("active") {
            "active".to_string()
        } else {
            "passive".to_string()
        },
        ftp_secure_implicit: payload.ftp_secure_implicit,
        ftp_accept_invalid_certificates: payload.ftp_accept_invalid_certificates,
        ftp_accept_invalid_hostnames: payload.ftp_accept_invalid_hostnames,
        trusted_fingerprints,
    })
}

fn load_profiles(path: &Path) -> Result<Vec<ConnectionProfileDto>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let parsed: ProfileFile = serde_json::from_str(&content).unwrap_or(ProfileFile { profiles: Vec::new() });
    Ok(parsed.profiles)
}

fn save_profiles(path: &Path, profiles: &[ConnectionProfileDto]) -> Result<()> {
    let payload = serde_json::to_string_pretty(&ProfileFile {
        profiles: profiles.to_vec(),
    })?;
    fs::write(path, payload)?;
    Ok(())
}

fn default_port_for_protocol(protocol: &str) -> u16 {
    match protocol {
        "ssh" => 22,
        "smb" => 445,
        "ftps" => 21,
        _ => 21,
    }
}

fn validate_connection_profile_payload(payload: &ConnectionProfilePayload) -> Result<()> {
    let protocol = payload.protocol.trim().to_ascii_lowercase();
    if payload.label.trim().is_empty() {
        return Err(anyhow!("Debes indicar una etiqueta para el perfil remoto."));
    }
    match protocol.as_str() {
        "smb" => {
            if payload.host.trim().is_empty() {
                return Err(anyhow!("Debes indicar el servidor o IP para la conexion SMB."));
            }
            if payload.share.trim().is_empty() {
                return Err(anyhow!("Debes indicar el recurso compartido SMB."));
            }
        }
        "ssh" => {
            if payload.host.trim().is_empty() {
                return Err(anyhow!("Debes indicar el servidor o IP para la conexion SSH."));
            }
            if payload.username.trim().is_empty() {
                return Err(anyhow!("Debes indicar el usuario para la conexion SSH."));
            }
        }
        "ftp" | "ftps" => {
            if payload.host.trim().is_empty() {
                return Err(anyhow!("Debes indicar el servidor o IP para la conexion FTP."));
            }
        }
        _ => return Err(anyhow!("El protocolo remoto indicado no es valido.")),
    }
    Ok(())
}

fn normalize_logical_path(path: &str) -> String {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() || normalized == "/" {
        "/".to_string()
    } else {
        format!("/{}", normalized.trim_matches('/'))
    }
}

fn normalize_windows_share_path(path: &str) -> String {
    let normalized = path.trim().replace('/', "\\");
    if normalized.is_empty() || normalized == "\\" {
        "\\".to_string()
    } else {
        format!("\\{}", normalized.trim_matches('\\'))
    }
}

fn build_remote_virtual_path(session_id: &str, logical_path: &str) -> String {
    if logical_path == "/" {
        format!("{REMOTE_PREFIX}{session_id}/")
    } else {
        format!("{REMOTE_PREFIX}{session_id}{}", normalize_logical_path(logical_path))
    }
}

pub fn parse_remote_virtual_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix(REMOTE_PREFIX)?;
    let split = rest.find('/')?;
    let session_id = rest[..split].to_string();
    let logical = normalize_logical_path(&rest[split..]);
    Some((session_id, logical))
}

fn logical_parent_path(logical_path: &str) -> Option<String> {
    let normalized = normalize_logical_path(logical_path);
    if normalized == "/" {
        return None;
    }
    let trimmed = normalized.trim_end_matches('/');
    let separator = trimmed.rfind('/')?;
    if separator == 0 {
        Some("/".to_string())
    } else {
        Some(trimmed[..separator].to_string())
    }
}

fn logical_leaf_name(logical_path: &str) -> String {
    let normalized = normalize_logical_path(logical_path);
    if normalized == "/" {
        "/".to_string()
    } else {
        normalized
            .rsplit('/')
            .next()
            .map(|item| item.to_string())
            .unwrap_or_else(|| normalized.to_string())
    }
}

fn join_logical_path(parent: &str, name: &str) -> String {
    let normalized_parent = normalize_logical_path(parent);
    let clean_name = name.trim_matches('/').trim_matches('\\');
    if normalized_parent == "/" {
        format!("/{}", clean_name)
    } else {
        format!("{normalized_parent}/{clean_name}")
    }
}

fn provider_child_to_logical(parent_provider_path: &str, child_provider_path: &Path) -> Result<String> {
    let parent = parent_provider_path.replace('\\', "/");
    let child = child_provider_path.to_string_lossy().replace('\\', "/");
    let suffix = child
        .strip_prefix(parent.trim_end_matches('/'))
        .ok_or_else(|| anyhow!("No se pudo reconstruir la ruta logica remota."))?;
    Ok(normalize_logical_path(suffix))
}

fn describe_profile(profile: &ConnectionProfileDto) -> String {
    match profile.protocol.as_str() {
        "ssh" => format!("SSH | {}:{} | {}", profile.host, profile.port, profile.username),
        "smb" => format!("SMB | {}\\{}", profile.host, profile.share),
        "ftps" => format!(
            "{} | {}:{} | {}",
            if profile.ftp_secure_implicit { "FTPS implicito" } else { "FTPS explicito" },
            profile.host,
            profile.port,
            profile.username
        ),
        _ => format!("FTP | {}:{} | {}", profile.host, profile.port, profile.username),
    }
}

fn protocol_label(profile: &ConnectionProfileDto) -> &'static str {
    match profile.protocol.as_str() {
        "smb" => "SMB",
        "ssh" => {
            if profile.ssh_mode == "scp" {
                "SSH/SCP"
            } else {
                "SSH/SFTP"
            }
        }
        "ftps" => {
            if profile.ftp_secure_implicit {
                "FTPS implicito"
            } else {
                "FTPS explicito"
            }
        }
        _ => "FTP",
    }
}

fn friendly_connection_error(profile: &ConnectionProfileDto, phase: &str, raw: &str) -> String {
    let message = raw.trim();
    let lowered = message.to_ascii_lowercase();
    let endpoint = format!("{}:{}", profile.host, profile.port);
    let protocol = protocol_label(profile);

    if lowered.contains("timed out") || lowered.contains("timeout") || lowered.contains("tiempo de espera") {
        return format!(
            "Tiempo de espera agotado al {} en {}. Revisa que el equipo este encendido, accesible y escuchando en el puerto correcto.",
            if phase == "fingerprint" { "contactar con el host" } else { "conectar" },
            endpoint
        );
    }
    if lowered.contains("connection refused") || lowered.contains("actively refused") {
        return format!(
            "El host {} respondio pero rechazo la conexion {}. Revisa que el servicio remoto este activo y que el puerto sea el correcto.",
            endpoint, protocol
        );
    }
    if lowered.contains("no route to host")
        || lowered.contains("host unreachable")
        || lowered.contains("network is unreachable")
    {
        return format!(
            "No se pudo alcanzar el host {} por red. Revisa IP, cableado, Wi-Fi, VPN o reglas de firewall.",
            endpoint
        );
    }
    if lowered.contains("resolve")
        || lowered.contains("getaddrinfo")
        || lowered.contains("name or service not known")
        || lowered.contains("failed to lookup address")
        || lowered.contains("nodename nor servname")
    {
        return format!(
            "No se pudo resolver el nombre del host '{}'. Revisa el DNS o usa una IP directa.",
            profile.host
        );
    }
    if lowered.contains("permission denied")
        || lowered.contains("authentication failed")
        || lowered.contains("authenticationfailed")
        || lowered.contains("login incorrect")
        || lowered.contains("530 ")
        || lowered.contains("auth fail")
        || lowered.contains("username or password")
    {
        return format!(
            "No se pudo iniciar sesion en {}. Revisa usuario, contrasena y permisos del servidor.",
            protocol
        );
    }
    if lowered.contains("certificate")
        || lowered.contains("tls")
        || lowered.contains("ssl")
        || lowered.contains("hostname")
        || lowered.contains("x509")
    {
        return format!(
            "No se pudo establecer la conexion segura {}. Revisa certificado, nombre del host y opciones de validacion TLS/SSL.",
            protocol
        );
    }
    if profile.protocol == "smb"
        && (lowered.contains("bad network name")
            || lowered.contains("object name not found")
            || lowered.contains("tree connect")
            || lowered.contains("resource name")
            || lowered.contains("share"))
    {
        return format!(
            "No se pudo acceder al recurso compartido SMB '{}'. Revisa servidor, nombre del recurso y permisos.",
            profile.share
        );
    }
    if lowered.contains("no such file or directory") || lowered.contains("not found") {
        return format!(
            "La ruta inicial configurada para {} no existe o no es accesible. Revisa la carpeta de inicio del perfil.",
            protocol
        );
    }
    if lowered.contains("unsupportedfeature") || lowered.contains("no estan disponibles con el backend ftps actual") {
        return "La combinacion de opciones de seguridad FTPS solicitada no esta soportada por el backend actual.".to_string();
    }

    format!(
        "No se pudo completar la operacion {} sobre {}. Detalle tecnico: {}",
        match phase {
            "fingerprint" => "de verificacion de huella",
            "root" => "de acceso a la ruta inicial",
            _ => "de conexion",
        },
        protocol,
        message
    )
}

fn validate_remote_root(profile: &ConnectionProfileDto, remote_fs: &mut dyn RemoteFs, provider_root: &str) -> Result<()> {
    let root_path = Path::new(provider_root);
    let root_entry = remote_fs
        .stat(root_path)
        .map_err(|error| anyhow!(friendly_connection_error(profile, "root", &error.to_string())))?;
    if !root_entry.is_dir() {
        return Err(anyhow!(
            "La ruta inicial configurada no es una carpeta navegable. Revisa la carpeta de inicio del perfil {}.",
            protocol_label(profile)
        ));
    }
    Ok(())
}

fn probe_ssh_fingerprint(profile: &ConnectionProfileDto) -> Result<Option<String>> {
    let tcp = TcpStream::connect((profile.host.as_str(), profile.port))
        .with_context(|| format!("No se pudo abrir la conexion TCP hacia {}:{}", profile.host, profile.port))?;
    let mut session = Ssh2Session::new().context("No se pudo crear la sesion SSH para leer la huella")?;
    session.set_tcp_stream(tcp);
    session.handshake().context("No se pudo negociar el handshake SSH para leer la huella")?;
    Ok(session
        .host_key_hash(ssh2::HashType::Sha256)
        .map(|bytes| format!("SHA256:{}", hex_bytes(bytes))))
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>()
}

fn empty_file_metadata() -> RemoteMetadata {
    file_metadata(0)
}

fn file_metadata(size: u64) -> RemoteMetadata {
    RemoteMetadata {
        accessed: None,
        created: None,
        file_type: FileType::File,
        gid: None,
        mode: Some(UnixPex::from(0o644)),
        modified: Some(SystemTime::now()),
        size,
        symlink: None,
        uid: None,
    }
}

fn system_time_to_timestamp(value: SystemTime) -> Option<i64> {
    let date_time = DateTime::<Local>::from(value);
    Some(date_time.timestamp())
}

fn format_system_time(value: SystemTime) -> String {
    DateTime::<Local>::from(value)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn read_remote_bytes(fs: &mut dyn RemoteFs, path: &Path) -> Result<Vec<u8>> {
    let mut stream = fs.open(path).map_err(|error| anyhow!(error.to_string()))?;
    let mut buffer = Vec::new();
    std::io::copy(&mut stream, &mut buffer).map_err(|error| anyhow!(error.to_string()))?;
    fs.on_read(stream).map_err(|error| anyhow!(error.to_string()))?;
    Ok(buffer)
}

fn read_remote_bytes_remote(fs: &mut dyn RemoteFs, path: &Path) -> remotefs::RemoteResult<Vec<u8>> {
    let mut stream = fs.open(path)?;
    let mut buffer = Vec::new();
    std::io::copy(&mut stream, &mut buffer)
        .map_err(|error| RemoteError::new_ex(RemoteErrorType::IoError, error.to_string()))?;
    fs.on_read(stream)?;
    Ok(buffer)
}

fn ftp_pex_class(file: &FtpListFile, query: PosixPexQuery) -> UnixPexClass {
    UnixPexClass::new(
        file.can_read(query),
        file.can_write(query),
        file.can_execute(query),
    )
}

fn remove_remote_target(fs: &mut dyn RemoteFs, path: &Path) -> Result<()> {
    let item = fs.stat(path).map_err(|error| anyhow!(error.to_string()))?;
    if item.is_dir() {
        fs.remove_dir_all(path).map_err(|error| anyhow!(error.to_string()))
    } else {
        fs.remove_file(path).map_err(|error| anyhow!(error.to_string()))
    }
}

fn remove_local_target(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn collect_remote_matches(
    session: &mut RemoteSession,
    logical_path: &str,
    lowered_query: &str,
    recursive: bool,
    results: &mut Vec<EntryDto>,
) -> Result<()> {
    let provider_path = session.resolve_provider_path(logical_path);
    let children = session
        .fs
        .list_dir(Path::new(&provider_path))
        .map_err(|error| anyhow!(error.to_string()))?;
    for child in children {
        let child_logical = session.provider_child_to_logical(&provider_path, child.path())?;
        let entry = session.entry_from_remote_file(child, &child_logical)?;
        if entry.name.to_lowercase().contains(lowered_query) {
            results.push(entry.clone());
        }
        if recursive && entry.is_dir {
            collect_remote_matches(session, &child_logical, lowered_query, recursive, results)?;
        }
    }
    Ok(())
}

fn summarize_remote_dir(
    session: &mut RemoteSession,
    logical_path: &str,
    max_depth: Option<usize>,
    depth: usize,
) -> Result<(u64, usize, usize)> {
    let provider_path = session.resolve_provider_path(logical_path);
    let children = session
        .fs
        .list_dir(Path::new(&provider_path))
        .map_err(|error| anyhow!(error.to_string()))?;
    let mut total_size = 0u64;
    let mut file_count = 0usize;
    let mut dir_count = 0usize;
    for child in children {
        let child_logical = session.provider_child_to_logical(&provider_path, child.path())?;
        if child.is_dir() {
            dir_count += 1;
            if max_depth.map(|limit| depth < limit).unwrap_or(true) {
                let (nested_size, nested_files, nested_dirs) =
                    summarize_remote_dir(session, &child_logical, max_depth, depth + 1)?;
                total_size += nested_size;
                file_count += nested_files;
                dir_count += nested_dirs;
            }
        } else {
            total_size += child.metadata().size;
            file_count += 1;
        }
    }
    Ok((total_size, file_count, dir_count))
}

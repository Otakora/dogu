use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::Mutex,
    time::Duration,
};

use anyhow::{anyhow, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tauri::{AppHandle, Emitter};

use crate::models::{AvailableShell, TerminalCwdPayload, TerminalDataPayload, TerminalExitPayload};

// ── Local PTY session ──────────────────────────────────────────────────────

struct TerminalSession {
    master: Box<dyn portable_pty::MasterPty>,
    writer: Mutex<Box<dyn Write + Send>>,
    /// Temp file or directory created for shell integration; cleaned up on close.
    temp_path: Option<std::path::PathBuf>,
}

// SAFETY: portable_pty MasterPty implementations are all Send.
unsafe impl Send for TerminalSession {}

// ── SSH terminal session ───────────────────────────────────────────────────

enum SshCmd {
    Input(String),
    Resize(u16, u16),
    Close,
}

struct SshTerminalSession {
    tx: std::sync::mpsc::Sender<SshCmd>,
}

// ── Unified session type ───────────────────────────────────────────────────

enum AnySession {
    Local(TerminalSession),
    Ssh(SshTerminalSession),
}

pub struct TerminalManager {
    sessions: Mutex<HashMap<String, AnySession>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

// ── Terminal creation ──────────────────────────────────────────────────────

pub fn create_terminal(
    manager: &TerminalManager,
    app_handle: AppHandle,
    id: String,
    cwd: String,
    shell: Option<String>,
) -> Result<()> {
    let pty_system = native_pty_system();

    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell_path = shell.unwrap_or_else(get_default_shell);
    let mut cmd = CommandBuilder::new(&shell_path);
    if !cwd.is_empty() {
        cmd.cwd(&cwd);
    }

    #[cfg(not(target_os = "windows"))]
    {
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
    }

    // Inject OSC 7 shell integration; returns temp path to clean up later.
    let temp_path = inject_shell_integration(&mut cmd, &shell_path, &id);

    // Emit the initial CWD immediately so color sync shows before the first prompt.
    if !cwd.is_empty() {
        let _ = app_handle.emit(
            "terminal-cwd",
            TerminalCwdPayload {
                id: id.clone(),
                cwd: cwd.clone(),
            },
        );
    }

    let _child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    {
        let mut sessions = manager.sessions.lock().unwrap();
        sessions.insert(
            id.clone(),
            AnySession::Local(TerminalSession {
                master: pair.master,
                writer: Mutex::new(writer),
                temp_path,
            }),
        );
    }

    std::thread::spawn(move || {
        reader_loop(reader, id, app_handle);
    });

    Ok(())
}

// ── OSC 7 parsing ──────────────────────────────────────────────────────────

/// Strips OSC 7 sequences from PTY output, returning clean bytes and any
/// CWD paths found. Carries incomplete sequences across read() boundaries
/// via the `pending` buffer.
fn process_terminal_data(input: &[u8], pending: &mut Vec<u8>) -> (Vec<u8>, Vec<String>) {
    let mut clean = Vec::with_capacity(input.len());
    let mut cwds: Vec<String> = Vec::new();

    let mut data = std::mem::take(pending);
    data.extend_from_slice(input);

    let mut i = 0;
    while i < data.len() {
        // ESC ] = start of OSC sequence
        if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b']' {
            let seq_start = i;
            i += 2;
            let content_start = i;

            loop {
                if i >= data.len() {
                    // Incomplete sequence — carry over to next call.
                    *pending = data[seq_start..].to_vec();
                    return (clean, cwds);
                }
                if data[i] == 0x07 {
                    // BEL terminator
                    let content = &data[content_start..i];
                    if let Some(cwd) = try_parse_osc7(content) {
                        cwds.push(cwd);
                    } else {
                        clean.extend_from_slice(&data[seq_start..=i]);
                    }
                    i += 1;
                    break;
                }
                if data[i] == 0x1b && i + 1 < data.len() && data[i + 1] == b'\\' {
                    // ST terminator (ESC \)
                    let content = &data[content_start..i];
                    if let Some(cwd) = try_parse_osc7(content) {
                        cwds.push(cwd);
                    } else {
                        clean.extend_from_slice(&data[seq_start..i + 2]);
                    }
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            clean.push(data[i]);
            i += 1;
        }
    }

    (clean, cwds)
}

fn try_parse_osc7(content: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(content).ok()?;
    let s = s.strip_prefix("7;")?;
    let path = if let Some(rest) = s.strip_prefix("file://") {
        // Strip hostname (everything before the first '/')
        let path_start = rest.find('/').unwrap_or(0);
        percent_decode(&rest[path_start..])
    } else {
        s.to_string()
    };
    if path.is_empty() {
        None
    } else {
        Some(normalize_osc7_path(&path))
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "windows")]
fn normalize_osc7_path(path: &str) -> String {
    // Convert /C:/Users/... → C:\Users\...
    let stripped = if path.starts_with('/') && path.len() > 2 {
        let rest = &path[1..];
        if rest.as_bytes().get(1).copied() == Some(b':') {
            rest
        } else {
            path
        }
    } else {
        path
    };
    stripped.replace('/', "\\")
}

#[cfg(not(target_os = "windows"))]
fn normalize_osc7_path(path: &str) -> String {
    path.to_string()
}

// ── Reader loop ────────────────────────────────────────────────────────────

fn reader_loop(mut reader: Box<dyn Read + Send>, id: String, app_handle: AppHandle) {
    let mut buf = [0u8; 8192];
    let mut osc_pending: Vec<u8> = Vec::new();
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let (clean, cwds) = process_terminal_data(&buf[..n], &mut osc_pending);

                for cwd in cwds {
                    let _ = app_handle.emit(
                        "terminal-cwd",
                        TerminalCwdPayload {
                            id: id.clone(),
                            cwd,
                        },
                    );
                }

                if !clean.is_empty() {
                    let data = String::from_utf8_lossy(&clean).into_owned();
                    let _ = app_handle.emit(
                        "terminal-data",
                        TerminalDataPayload {
                            id: id.clone(),
                            data,
                        },
                    );
                }
            }
        }
    }
    let _ = app_handle.emit("terminal-exit", TerminalExitPayload { id });
}

// ── Shell integration injection ────────────────────────────────────────────

/// Injects OSC 7 CWD tracking into the shell command.
/// Returns a temp path (file or dir) that must be cleaned up when the terminal closes.
fn inject_shell_integration(
    cmd: &mut CommandBuilder,
    shell_path: &str,
    terminal_id: &str,
) -> Option<std::path::PathBuf> {
    let shell_name = std::path::Path::new(shell_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match shell_name.as_str() {
        "bash" | "sh" => inject_bash(cmd, terminal_id),
        "zsh"         => inject_zsh(cmd, terminal_id),
        "fish"        => { inject_fish(cmd); None }
        "pwsh" | "powershell" => { inject_pwsh(cmd); None }
        _ => None,
    }
}

// ── bash / sh ──────────────────────────────────────────────────────────────

fn inject_bash(cmd: &mut CommandBuilder, terminal_id: &str) -> Option<std::path::PathBuf> {
    // Write a temporary --init-file that sources the user's .bashrc and then
    // appends our OSC 7 PROMPT_COMMAND hook, so we don't conflict with the
    // user's existing PROMPT_COMMAND.
    let content = r#"
[ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"
__dogu_osc7() { printf '\033]7;file://localhost%s\007' "$PWD"; }
case "${PROMPT_COMMAND:-}" in
  *__dogu_osc7*) ;;
  *) PROMPT_COMMAND="__dogu_osc7${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
esac
__dogu_osc7
"#;
    let path = write_temp_file(terminal_id, content, ".sh")?;
    cmd.arg("--init-file");
    cmd.arg(&path);
    Some(path)
}

// ── zsh ───────────────────────────────────────────────────────────────────

fn inject_zsh(cmd: &mut CommandBuilder, terminal_id: &str) -> Option<std::path::PathBuf> {
    // Point ZDOTDIR at a temp dir whose .zshrc sources the real one and adds
    // a chpwd hook. We save the original ZDOTDIR (if any) so the sourced
    // .zshrc can reference it correctly.
    let content = r#"
__dogu_orig="${_DOGU_ZDOT:-$HOME}"
[ -f "$__dogu_orig/.zshenv" ] && source "$__dogu_orig/.zshenv"
[ -f "$__dogu_orig/.zshrc"  ] && ZDOTDIR="$__dogu_orig" source "$__dogu_orig/.zshrc"
autoload -Uz add-zsh-hook
__dogu_osc7() { printf '\033]7;file://localhost%s\007' "$PWD"; }
add-zsh-hook chpwd __dogu_osc7
__dogu_osc7
"#;
    let dir = write_temp_dir(terminal_id, ".zshrc", content)?;

    // Preserve the original ZDOTDIR so the wrapper .zshrc can source it.
    if let Ok(orig) = std::env::var("ZDOTDIR") {
        cmd.env("_DOGU_ZDOT", orig);
    } else if let Ok(home) = std::env::var("HOME") {
        cmd.env("_DOGU_ZDOT", home);
    }
    cmd.env("ZDOTDIR", dir.to_str().unwrap_or(""));
    Some(dir)
}

// ── fish ──────────────────────────────────────────────────────────────────

fn inject_fish(cmd: &mut CommandBuilder) {
    // `--on-variable PWD` fires whenever the directory changes.
    cmd.arg("--init-command");
    cmd.arg(concat!(
        "function __dogu_osc7 --on-variable PWD;",
        " printf '\\033]7;file://localhost%s\\007' $PWD;",
        " end;",
        " __dogu_osc7"
    ));
}

// ── PowerShell (pwsh / powershell) ────────────────────────────────────────

fn inject_pwsh(cmd: &mut CommandBuilder) {
    // Override the prompt function to emit OSC 7 before each prompt.
    // We preserve the existing prompt output so the user's custom prompt is kept.
    // -NoExit keeps PowerShell interactive after running -Command.
    cmd.arg("-NoExit");
    cmd.arg("-Command");
    cmd.arg(
        r#"$__dOrig=$function:prompt; $function:prompt={ $__p=if($__dOrig){&$__dOrig}else{"PS $($pwd)> "}; [Console]::Write("`e]7;file://localhost/$(($pwd.path -replace '\\','/'))`a"); $__p }"#,
    );
}

// ── Temp file helpers ──────────────────────────────────────────────────────

fn write_temp_file(terminal_id: &str, content: &str, ext: &str) -> Option<std::path::PathBuf> {
    let safe: String = terminal_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    let path = std::env::temp_dir().join(format!("dogu-init-{}{}", safe, ext));
    std::fs::write(&path, content).ok()?;
    Some(path)
}

fn write_temp_dir(terminal_id: &str, filename: &str, content: &str) -> Option<std::path::PathBuf> {
    let safe: String = terminal_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    let dir = std::env::temp_dir().join(format!("dogu-zsh-{}", safe));
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::write(dir.join(filename), content).ok()?;
    Some(dir)
}

// ── PTY I/O operations ─────────────────────────────────────────────────────

pub fn send_input(manager: &TerminalManager, id: &str, data: &str) -> Result<()> {
    let sessions = manager.sessions.lock().unwrap();
    match sessions.get(id).ok_or_else(|| anyhow!("terminal '{}' not found", id))? {
        AnySession::Local(s) => {
            let mut writer = s.writer.lock().unwrap();
            writer.write_all(data.as_bytes())?;
            writer.flush().ok();
        }
        AnySession::Ssh(s) => {
            let _ = s.tx.send(SshCmd::Input(data.to_string()));
        }
    }
    Ok(())
}

pub fn resize_terminal(manager: &TerminalManager, id: &str, cols: u16, rows: u16) -> Result<()> {
    let sessions = manager.sessions.lock().unwrap();
    if let Some(session) = sessions.get(id) {
        match session {
            AnySession::Local(s) => {
                s.master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })?;
            }
            AnySession::Ssh(s) => {
                let _ = s.tx.send(SshCmd::Resize(cols, rows));
            }
        }
    }
    Ok(())
}

pub fn close_terminal(manager: &TerminalManager, id: &str) {
    let mut sessions = manager.sessions.lock().unwrap();
    if let Some(session) = sessions.remove(id) {
        match session {
            AnySession::Local(s) => {
                if let Some(path) = s.temp_path {
                    if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                    } else {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
            AnySession::Ssh(s) => {
                let _ = s.tx.send(SshCmd::Close);
            }
        }
    }
}

// ── Shell discovery ────────────────────────────────────────────────────────

fn get_default_shell() -> String {
    #[cfg(target_os = "windows")]
    return std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

    #[cfg(not(target_os = "windows"))]
    return std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
}

// ── SSH terminal ───────────────────────────────────────────────────────────

// ssh2::Session is not Send, but we move it into a single dedicated thread.
struct SendableSshSession(ssh2::Session);
unsafe impl Send for SendableSshSession {}

pub fn create_ssh_terminal(
    manager: &TerminalManager,
    app_handle: AppHandle,
    id: String,
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<SshCmd>();

    {
        let mut sessions = manager.sessions.lock().unwrap();
        sessions.insert(id.clone(), AnySession::Ssh(SshTerminalSession { tx }));
    }

    std::thread::spawn(move || {
        if let Err(e) = run_ssh_worker(app_handle.clone(), id.clone(), host, port, username, password, remote_path, rx) {
            let _ = app_handle.emit(
                "terminal-data",
                TerminalDataPayload {
                    id: id.clone(),
                    data: format!("\r\n[Error SSH: {}]\r\n", e),
                },
            );
        }
        let _ = app_handle.emit("terminal-exit", TerminalExitPayload { id });
    });

    Ok(())
}

fn run_ssh_worker(
    app_handle: AppHandle,
    id: String,
    host: String,
    port: u16,
    username: String,
    password: String,
    remote_path: String,
    rx: std::sync::mpsc::Receiver<SshCmd>,
) -> Result<()> {
    use ssh2::Session as Ssh2Session;

    let tcp = std::net::TcpStream::connect((host.as_str(), port))
        .map_err(|e| anyhow!("No se pudo conectar a {}:{}: {}", host, port, e))?;

    let mut session = Ssh2Session::new()
        .map_err(|e| anyhow!("No se pudo crear sesion SSH: {}", e))?;
    session.set_tcp_stream(tcp);
    session.handshake()
        .map_err(|e| anyhow!("Error en handshake SSH: {}", e))?;
    session
        .userauth_password(&username, &password)
        .map_err(|e| anyhow!("Autenticacion SSH fallida: {}", e))?;

    if !session.authenticated() {
        return Err(anyhow!("La autenticacion SSH fallo (credenciales incorrectas)"));
    }

    let mut channel = session
        .channel_session()
        .map_err(|e| anyhow!("No se pudo abrir el canal SSH: {}", e))?;
    channel
        .request_pty("xterm-256color", None, Some((80, 24, 0, 0)))
        .map_err(|e| anyhow!("No se pudo solicitar PTY: {}", e))?;

    // Try `shell` first. Some hardened servers (NAS, sftp-only) send FAILURE for
    // the "shell" channel request but still allow `exec` with an explicit shell.
    if channel.shell().is_err() {
        let started = ["bash", "/bin/bash", "sh", "/bin/sh"]
            .iter()
            .any(|sh| channel.exec(sh).is_ok());
        if !started {
            return Err(anyhow!(
                "El servidor rechazó la solicitud de shell. \
                 Comprueba que el usuario tiene acceso a sesiones interactivas \
                 (el servidor puede estar configurado como solo-SFTP)."
            ));
        }
    }

    // Send an initial CD to the requested remote path.
    if !remote_path.is_empty() && remote_path != "/" {
        let quoted = remote_path.replace('\'', "'\\''");
        let cd_cmd = format!("cd '{}'\n", quoted);
        let _ = channel.write_all(cd_cmd.as_bytes());
        let _ = channel.flush();
    }

    session.set_blocking(false);

    let wrap = SendableSshSession(session);
    let mut buf = [0u8; 8192];

    loop {
        // Read output from SSH channel.
        match channel.read(&mut buf) {
            Ok(0) => {
                if channel.eof() {
                    break;
                }
            }
            Ok(n) => {
                let data = String::from_utf8_lossy(&buf[..n]).into_owned();
                let _ = app_handle.emit(
                    "terminal-data",
                    TerminalDataPayload {
                        id: id.clone(),
                        data,
                    },
                );
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }

        // Drain incoming commands.
        loop {
            match rx.try_recv() {
                Ok(SshCmd::Input(data)) => {
                    wrap.0.set_blocking(true);
                    let _ = channel.write_all(data.as_bytes());
                    let _ = channel.flush();
                    wrap.0.set_blocking(false);
                }
                Ok(SshCmd::Resize(cols, rows)) => {
                    let _ = channel.request_pty_size(cols as u32, rows as u32, None, None);
                }
                Ok(SshCmd::Close) => return Ok(()),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}

pub fn get_available_shells() -> Vec<AvailableShell> {
    #[cfg(target_os = "windows")]
    return available_shells_windows();

    #[cfg(not(target_os = "windows"))]
    return available_shells_unix();
}

#[cfg(target_os = "windows")]
fn available_shells_windows() -> Vec<AvailableShell> {
    let mut shells = vec![];

    if let Ok(out) = std::process::Command::new("where").arg("pwsh.exe").output() {
        if out.status.success() {
            if let Some(path) = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                shells.push(AvailableShell {
                    name: "PowerShell".to_string(),
                    path,
                });
            }
        }
    }

    if let Ok(out) = std::process::Command::new("where")
        .arg("powershell.exe")
        .output()
    {
        if out.status.success() {
            if let Some(path) = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                shells.push(AvailableShell {
                    name: "Windows PowerShell".to_string(),
                    path,
                });
            }
        }
    }

    shells.push(AvailableShell {
        name: "Command Prompt".to_string(),
        path: std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
    });

    shells
}

#[cfg(not(target_os = "windows"))]
fn available_shells_unix() -> Vec<AvailableShell> {
    let mut shells = vec![];

    if let Ok(shell_path) = std::env::var("SHELL") {
        let raw = shell_path.split('/').last().unwrap_or("shell");
        let name = match raw {
            "bash" => "Bash",
            "zsh"  => "Zsh",
            "fish" => "Fish",
            "sh"   => "sh",
            other  => other,
        }
        .to_string();
        shells.push(AvailableShell {
            name,
            path: shell_path,
        });
    }

    if !shells.iter().any(|s| s.path.ends_with("/bash") || s.path == "bash") {
        if std::path::Path::new("/bin/bash").exists() {
            shells.push(AvailableShell {
                name: "Bash".to_string(),
                path: "/bin/bash".to_string(),
            });
        }
    }

    if shells.is_empty() {
        shells.push(AvailableShell {
            name: "sh".to_string(),
            path: "/bin/sh".to_string(),
        });
    }

    shells
}

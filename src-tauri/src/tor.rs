// Yonodex Desktop Client - Tor lifecycle management
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
//
// Spawns tor.exe as a child process on app startup, terminates it on app exit.
// The Tor daemon exposes:
//   - SOCKS5 proxy at 127.0.0.1:9050 (used by libp2p transport)
//   - Hidden service with a stable .onion address

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};

pub struct TorState {
    pub child: Mutex<Option<Child>>,
}

impl TorState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }
}

impl Default for TorState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TorPaths {
    pub tor_exe: PathBuf,
    pub torrc: PathBuf,
    pub working_dir: PathBuf,
    pub onion_hostname: PathBuf,
}

pub fn resolve_paths(app: &AppHandle) -> Result<TorPaths, String> {
    let resource_dir = app
        .path()
        .resolve("resources/tor", tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("resolve resource dir: {e}"))?;

    Ok(TorPaths {
        tor_exe: resource_dir.join("tor").join("tor.exe"),
        torrc: resource_dir.join("torrc-yonodex"),
        working_dir: resource_dir.clone(),
        onion_hostname: resource_dir
            .join("data")
            .join("onion-service")
            .join("hostname"),
    })
}

#[cfg(windows)]
fn make_command(tor_exe: &Path, torrc: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut cmd = Command::new(tor_exe);
    cmd.arg("-f").arg(torrc);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(windows))]
fn make_command(tor_exe: &Path, torrc: &Path) -> Command {
    let mut cmd = Command::new(tor_exe);
    cmd.arg("-f").arg(torrc);
    cmd
}

/// Spawn the Tor child process. Idempotent - no-op if already running.
pub fn start(app: &AppHandle, state: &State<'_, TorState>) -> Result<(), String> {
    {
        let guard = state.child.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Ok(());
        }
    }

    let paths = resolve_paths(app)?;
    if !paths.tor_exe.exists() {
        return Err(format!("tor.exe not found at {}", paths.tor_exe.display()));
    }

    let mut cmd = make_command(&paths.tor_exe, &paths.torrc);
    cmd.current_dir(&paths.working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().map_err(|e| format!("spawn tor: {e}"))?;

    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    *guard = Some(child);
    Ok(())
}

/// Terminate the Tor child process. Idempotent.
pub fn stop(state: &State<'_, TorState>) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

pub fn is_running(state: &State<'_, TorState>) -> bool {
    state.child.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Wait until Tor's SOCKS port accepts connections. This is the reliable
/// signal that Tor has finished bootstrapping, rather than parsing log lines.
pub fn wait_for_socks_port(timeout: Duration) -> Result<(), String> {
    let addr: std::net::SocketAddr = "127.0.0.1:9050"
        .parse()
        .map_err(|e| format!("parse addr: {e}"))?;

    let start = Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "tor SOCKS port did not open within {}s",
        timeout.as_secs()
    ))
}

/// Read the .onion address from the hostname file (Tor writes it on first launch).
pub fn read_onion_address(app: &AppHandle) -> Result<String, String> {
    let paths = resolve_paths(app)?;
    std::fs::read_to_string(&paths.onion_hostname)
        .map(|s| s.trim().to_string())
        .map_err(|e| format!("read hostname: {e}"))
}

// ---- Tauri commands ----
// Thin wrappers around the lifecycle functions, exposed to the frontend.

#[tauri::command]
pub async fn tor_start(app: AppHandle, state: State<'_, TorState>) -> Result<(), String> {
    // Spawn is fast - runs on the command's thread.
    start(&app, &state)?;

    // Waiting for bootstrap can take 1-3 min. Run it on a blocking
    // thread so the UI stays responsive.
    tauri::async_runtime::spawn_blocking(|| wait_for_socks_port(Duration::from_secs(180)))
        .await
        .map_err(|e| format!("join error: {e}"))??;

    Ok(())
}
#[tauri::command]
pub fn tor_stop(state: State<'_, TorState>) -> Result<(), String> {
    stop(&state)
}

#[tauri::command]
pub fn tor_is_running(state: State<'_, TorState>) -> bool {
    is_running(&state)
}

#[tauri::command]
pub fn tor_get_onion_address(app: AppHandle) -> Result<String, String> {
    read_onion_address(&app)
}

/// Non-blocking check: is Tor's SOCKS port accepting connections?
pub fn socks_port_open(timeout: Duration) -> bool {
    let addr: std::net::SocketAddr = match "127.0.0.1:9050".parse() {
        Ok(a) => a,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// Three-state query used by the UI:
///   "stopped"  - no child process
///   "starting" - child spawned, SOCKS not yet accepting
///   "running"  - child spawned, SOCKS accepting connections
#[tauri::command]
pub fn tor_get_state(state: State<'_, TorState>) -> String {
    if !is_running(&state) {
        return "stopped".to_string();
    }
    if socks_port_open(Duration::from_millis(200)) {
        "running".to_string()
    } else {
        "starting".to_string()
    }
}
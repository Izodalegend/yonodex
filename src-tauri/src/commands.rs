// Yonodex Desktop Client - Tauri Commands
// Whitepaper Layer 3, Section 5.2: encrypted DB access from the Svelte frontend
//
// All DB operations are exposed as Tauri commands. State is held in
// `AppState` and the encrypted handle stays in memory only while unlocked.

use crate::db::{self, DbHandle};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

pub struct AppState {
    pub db: Mutex<Option<DbHandle>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    Ok(dir.join("yonodex.db"))
}

#[derive(serde::Serialize)]
pub struct WalletConfigPayload {
    pub address: String,
    pub chain_id: String,
    pub wallet_uuid: String,
    pub wallet_name: String,
}

#[tauri::command]
pub fn db_is_unlocked(state: State<AppState>) -> bool {
    state.db.lock().map(|g| g.is_some()).unwrap_or(false)
}

#[tauri::command]
pub fn unlock_db(
    app: tauri::AppHandle,
    state: State<AppState>,
    password: String,
) -> Result<(), String> {
    let path = db_path(&app)?;
    let handle = db::open(&path, &password).map_err(|e| e.to_string())?;
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn lock_db(state: State<AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub fn save_wallet_config(
    state: State<AppState>,
    address: String,
    chain_id: String,
    wallet_uuid: String,
    wallet_name: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::save_wallet_config(db, &address, &chain_id, &wallet_uuid, &wallet_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_wallet_config(
    state: State<AppState>,
) -> Result<Option<WalletConfigPayload>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    let result = db::load_wallet_config(db).map_err(|e| e.to_string())?;
    Ok(result.map(|(address, chain_id, wallet_uuid, wallet_name)| {
        WalletConfigPayload {
            address,
            chain_id,
            wallet_uuid,
            wallet_name,
        }
    }))
}

#[tauri::command]
pub fn clear_wallet_config(state: State<AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::clear_wallet_config(db).map_err(|e| e.to_string())
}
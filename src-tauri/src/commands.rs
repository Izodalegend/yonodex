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
pub fn db_is_unlocked(state: State<'_, AppState>) -> bool {
    state.db.lock().map(|g| g.is_some()).unwrap_or(false)
}

#[tauri::command]
pub fn db_is_initialized(app: tauri::AppHandle) -> Result<bool, String> {
    let path = db_path(&app)?;
    Ok(db::is_initialized(&path))
}

#[tauri::command]
pub async fn unlock_db(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let path = db_path(&app)?;

    let handle = tauri::async_runtime::spawn_blocking(move || {
        db::open(&path, &password).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = Some(handle);
    Ok(())
}

#[tauri::command]
pub fn lock_db(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.db.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub fn save_wallet_config(
    state: State<'_, AppState>,
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
    state: State<'_, AppState>,
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
pub fn clear_wallet_config(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::clear_wallet_config(db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_password_hint(app: tauri::AppHandle, hint: String) -> Result<(), String> {
    let path = db_path(&app)?;
    db::save_hint(&path, &hint).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_password_hint(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = db_path(&app)?;
    Ok(db::load_hint(&path))
}

#[tauri::command]
pub fn reset_local_data(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut guard = state.db.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    let path = db_path(&app)?;
    db::reset_all(&path).map_err(|e| e.to_string())
}

// ---- Trade history (scoped per wallet) ----

#[derive(serde::Serialize)]
pub struct TradeRecordPayload {
    pub id: i64,
    pub wallet_address: String,
    pub tx_hash: Option<String>,
    pub chain_id: String,
    pub pair: String,
    pub side: String,
    pub amount_in: String,
    pub amount_out: String,
    pub token_in: String,
    pub token_out: String,
    pub status: String,
    pub timestamp: i64,
    pub notes: Option<String>,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn save_trade(
    state: State<'_, AppState>,
    wallet_address: String,
    tx_hash: Option<String>,
    chain_id: String,
    pair: String,
    side: String,
    amount_in: String,
    amount_out: String,
    token_in: String,
    token_out: String,
    status: String,
    notes: Option<String>,
) -> Result<i64, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::save_trade(
        db,
        &wallet_address,
        tx_hash.as_deref(),
        &chain_id,
        &pair,
        &side,
        &amount_in,
        &amount_out,
        &token_in,
        &token_out,
        &status,
        notes.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_trades(
    state: State<'_, AppState>,
    wallet_address: String,
    limit: Option<i64>,
) -> Result<Vec<TradeRecordPayload>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    let records = db::load_trades(db, &wallet_address, limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;
    Ok(records
        .into_iter()
        .map(|t| TradeRecordPayload {
            id: t.id,
            wallet_address: t.wallet_address,
            tx_hash: t.tx_hash,
            chain_id: t.chain_id,
            pair: t.pair,
            side: t.side,
            amount_in: t.amount_in,
            amount_out: t.amount_out,
            token_in: t.token_in,
            token_out: t.token_out,
            status: t.status,
            timestamp: t.timestamp,
            notes: t.notes,
        })
        .collect())
}

#[tauri::command]
pub fn delete_trade(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::delete_trade(db, id).map_err(|e| e.to_string())
}
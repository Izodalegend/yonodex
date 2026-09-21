// Yonodex Desktop Client - Tauri Commands
// Whitepaper Layer 3, Section 5.2: encrypted DB access from the Svelte frontend
// Whitepaper Layer 5, Section 7.1: order book operations

use crate::db::{self, DbHandle};
use crate::order::{Order, Side};
use crate::orderbook::OrderBook;
use ed25519_dalek::SigningKey;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

pub struct AppState {
    pub db: Mutex<Option<DbHandle>>,
    pub book: Mutex<Option<OrderBook>>,
    pub signing_key: Mutex<Option<SigningKey>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            db: Mutex::new(None),
            book: Mutex::new(None),
            signing_key: Mutex::new(None),
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OrderPayload {
    pub id: String,
    pub owner: String,
    pub side: String,
    pub pair: String,
    pub price: String,
    pub amount: String,
    pub expiry: i64,
    pub nonce: u64,
    pub signature: String,
    pub vector_clock: std::collections::HashMap<String, u64>,
}

impl From<&Order> for OrderPayload {
    fn from(o: &Order) -> Self {
        Self {
            id: o.id.clone(),
            owner: o.owner.clone(),
            side: o.side.as_str().to_string(),
            pair: o.pair.clone(),
            price: o.price.clone(),
            amount: o.amount.clone(),
            expiry: o.expiry,
            nonce: o.nonce,
            signature: o.signature.clone(),
            vector_clock: o.vector_clock.clone(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct BookStats {
    pub local_peer: String,
    pub live_count: usize,
    pub total_count: usize,
}

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

    let (owner_hex, private_hex) =
        db::ensure_signing_identity(&handle).map_err(|e| e.to_string())?;

    let mut key_bytes = [0u8; 32];
    let decoded = hex::decode(&private_hex).map_err(|e| e.to_string())?;
    if decoded.len() != 32 {
        return Err(format!("stored key has wrong length: {}", decoded.len()));
    }
    key_bytes.copy_from_slice(&decoded);
    let signing_key = SigningKey::from_bytes(&key_bytes);

    let mut book = OrderBook::new(owner_hex.clone());
    let persisted = db::load_all_orders(&handle).map_err(|e| e.to_string())?;
    for (order, tombstone, tombstoned_at) in persisted {
        book.restore_persisted(order, tombstone, tombstoned_at);
    }

    {
        let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
        *db_guard = Some(handle);
    }
    {
        let mut key_guard = state.signing_key.lock().map_err(|e| e.to_string())?;
        *key_guard = Some(signing_key);
    }
    {
        let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
        *book_guard = Some(book);
    }

    // Spawn the Node.js relay sidecar (transport for the order book).
    // Non-fatal: if the relay fails to start, the local order book still works.
    if let Err(e) = crate::relay::spawn(app.clone()).await {
        log::error!("relay spawn failed: {e}");
    }

    Ok(())
}

#[tauri::command]
pub async fn lock_db(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Stop the relay first so it doesn't try to write to a locked DB
    if let Err(e) = crate::relay::stop(app).await {
        log::error!("relay stop failed: {e}");
    }

    {
        let mut guard = state.db.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    {
        let mut guard = state.book.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    {
        let mut guard = state.signing_key.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
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
    {
        let mut guard = state.book.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    {
        let mut guard = state.signing_key.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }
    let path = db_path(&app)?;
    db::reset_all(&path).map_err(|e| e.to_string())
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
    let records =
        db::load_trades(db, &wallet_address, limit.unwrap_or(100)).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn save_node_identity(
    state: State<'_, AppState>,
    onion_address: String,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::save_node_identity(db, &onion_address).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_node_identity(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::load_node_identity(db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn debug_dump_node_identity(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = guard.as_ref().ok_or("database is locked")?;
    db::load_node_identity(db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn order_create(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    side: String,
    pair: String,
    price: String,
    amount: String,
    expiry: i64,
) -> Result<OrderPayload, String> {
    let side_enum = match side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => return Err(format!("invalid side: {side}")),
    };

    let created = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("database is locked")?;
        let key_guard = state.signing_key.lock().map_err(|e| e.to_string())?;
        let key = key_guard.as_ref().ok_or("signing key not loaded")?;
        let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
        let book = book_guard.as_mut().ok_or("order book not loaded")?;

        let owner = book.local_peer().to_string();
        let order = book
            .create_local(db, key, &owner, side_enum, &pair, &price, &amount, expiry)?;

        db::save_order(db, &order, None, None).map_err(|e| e.to_string())?;

        order
    };

    // Publish to the P2P network via relay (non-fatal if relay is down)
    let payload = serde_json::to_value(&created).map_err(|e| e.to_string())?;
    if let Err(e) = crate::relay::send(
        &app,
        serde_json::json!({ "type": "publish_order", "order": payload }),
    )
    .await
    {
        log::warn!("relay publish failed (non-fatal): {e}");
    }

    Ok(OrderPayload::from(&created))
}

#[tauri::command]
pub fn order_cancel(state: State<'_, AppState>, order_id: String) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("database is locked")?;
    let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_mut().ok_or("order book not loaded")?;

    book.cancel_local(&order_id)?;

    if let Some(entry) = book.get(&order_id) {
        let tombstone_str = entry.tombstone.map(|t| match t {
            crate::orderbook::TombstoneKind::Cancelled => "Cancelled",
            crate::orderbook::TombstoneKind::Filled => "Filled",
        });
        db::save_order(db, &entry.order, tombstone_str, entry.tombstoned_at)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn order_list(state: State<'_, AppState>) -> Result<Vec<OrderPayload>, String> {
    let book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_ref().ok_or("order book not loaded")?;
    Ok(book.live_orders().map(OrderPayload::from).collect())
}

#[tauri::command]
pub fn order_apply_remote(
    state: State<'_, AppState>,
    order: OrderPayload,
) -> Result<(), String> {
    let side_enum = match order.side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => return Err(format!("invalid side: {}", order.side)),
    };

    let incoming = Order {
        id: order.id,
        owner: order.owner,
        side: side_enum,
        pair: order.pair,
        price: order.price,
        amount: order.amount,
        expiry: order.expiry,
        nonce: order.nonce,
        signature: order.signature,
        vector_clock: order.vector_clock,
    };

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("database is locked")?;
    let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_mut().ok_or("order book not loaded")?;

    book.apply_remote(db, incoming.clone())?;

    db::save_order(db, &incoming, None, None).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn order_book_stats(state: State<'_, AppState>) -> Result<BookStats, String> {
    let book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_ref().ok_or("order book not loaded")?;
    Ok(BookStats {
        local_peer: book.local_peer().to_string(),
        live_count: book.live_count(),
        total_count: book.total_count(),
    })
}
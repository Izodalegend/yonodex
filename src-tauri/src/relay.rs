// Yonodex Desktop Client - IPC bridge to Node.js gossipsub relay
// Whitepaper Layer 5, Section 7.1: Rust owns the order book, Node.js is transport
// Whitepaper Layer 5, Section 7.3: signed trades also flow through this bridge
//
// Spawns `node p2p-tor-relay.mjs` as a child process:
//   - Rust -> Node via child stdin (JSON lines)
//   - Node -> Rust via child stdout (JSON lines)
//
// Incoming orders and trades are validated and applied to the Rust state.

use crate::commands::AppState;
use crate::order::Order;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

pub struct RelayState {
    pub child: Mutex<Option<Child>>,
    pub stdin: Mutex<Option<ChildStdin>>,
}

impl RelayState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
        }
    }
}

impl Default for RelayState {
    fn default() -> Self {
        Self::new()
    }
}

fn project_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    if cwd.ends_with("src-tauri") {
        Ok(cwd.parent().unwrap_or(&cwd).to_path_buf())
    } else {
        Ok(cwd)
    }
}

/// Spawn the relay. Idempotent - no-op if already running.
/// Reads YONODEX_DIAL env var for an optional peer multiaddr to dial on startup.
pub async fn spawn(app: AppHandle) -> Result<(), String> {
    let state = app.state::<RelayState>();
    {
        let guard = state.child.lock().await;
        if guard.is_some() {
            return Ok(());
        }
    }

    let root = project_root()?;
    let script = root.join("p2p-tor-relay.mjs");
    if !script.exists() {
        return Err(format!("relay script not found at {}", script.display()));
    }

    let mut cmd = Command::new("node");
    cmd.arg(&script);
    cmd.arg("--listen-port").arg("4001");

    if let Ok(dial) = std::env::var("YONODEX_DIAL") {
        if !dial.is_empty() {
            cmd.arg("--dial").arg(dial);
        }
    }

    cmd.current_dir(&root);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn().map_err(|e| format!("spawn relay: {e}"))?;
    let stdin = child.stdin.take().ok_or("relay: no stdin")?;
    let stdout = child.stdout.take().ok_or("relay: no stdout")?;

    *state.stdin.lock().await = Some(stdin);
    *state.child.lock().await = Some(child);

    // Spawn reader task — parses each JSON line from relay stdout
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if let Err(e) = handle_line(&handle, &line).await {
                log::error!("relay ipc: {e}");
            }
        }
        log::info!("relay stdout closed");
    });

    Ok(())
}

/// Stop the relay if running.
pub async fn stop(app: AppHandle) -> Result<(), String> {
    let state = app.state::<RelayState>();
    let mut guard = state.child.lock().await;
    if let Some(mut child) = guard.take() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    *state.stdin.lock().await = None;
    Ok(())
}

/// Send a JSON message to the relay's stdin.
pub async fn send(app: &AppHandle, msg: serde_json::Value) -> Result<(), String> {
    let state = app.state::<RelayState>();
    let mut guard = state.stdin.lock().await;
    let stdin = guard.as_mut().ok_or("relay not running")?;
    let mut buf = serde_json::to_vec(&msg).map_err(|e| e.to_string())?;
    buf.push(b'\n');
    stdin
        .write_all(&buf)
        .await
        .map_err(|e| format!("write stdin: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("flush stdin: {e}"))?;
    Ok(())
}

async fn handle_line(app: &AppHandle, line: &str) -> Result<(), String> {
    let msg: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("parse json: {e}"))?;
    let kind = msg
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("missing type")?;

    match kind {
        "status" => {
            let ready = msg.get("ready").and_then(|v| v.as_bool()).unwrap_or(false);
            let peers = msg.get("peer_count").and_then(|v| v.as_u64()).unwrap_or(0);
            log::info!("relay status: ready={ready} peers={peers}");
            Ok(())
        }
        "order_received" => {
            let order_val = msg.get("order").cloned().ok_or("missing order")?;
            let order: Order =
                serde_json::from_value(order_val).map_err(|e| format!("order parse: {e}"))?;
            apply_incoming_order(app, order)
        }
        "cancel_received" => {
            let order_id = msg
                .get("order_id")
                .and_then(|v| v.as_str())
                .ok_or("missing order_id")?
                .to_string();
            let remote_peer = msg
                .get("remote_peer")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let clock_val = msg
                .get("vector_clock")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let clock: std::collections::HashMap<String, u64> =
                serde_json::from_value(clock_val).map_err(|e| format!("clock parse: {e}"))?;
            apply_incoming_cancel(app, order_id, clock, remote_peer)
        }
        "trade_received" => {
            let trade_val = msg.get("trade").cloned().ok_or("missing trade")?;
            let trade: crate::trade::Trade =
                serde_json::from_value(trade_val).map_err(|e| format!("trade parse: {e}"))?;
            apply_incoming_trade(app, trade)
        }
        other => {
            log::warn!("relay ipc: unknown type {other}");
            Ok(())
        }
    }
}

fn apply_incoming_order(app: &AppHandle, order: Order) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("db locked")?;
    let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_mut().ok_or("book not loaded")?;

    // Loopback guard: gossipsub emitSelf echoes our own published orders back.
    // We already have them locally — skip rather than attempt a redundant apply
    // (which would correctly fail the nonce check and spam the log).
    if order.owner == book.local_peer() {
        log::debug!("skipping loopback of own order {}", order.id);
        return Ok(());
    }

    book.apply_remote(db, order.clone())?;
    crate::db::save_order(db, &order, None, None).map_err(|e| e.to_string())?;
    log::info!("applied remote order {}", order.id);
    Ok(())
}

fn apply_incoming_cancel(
    app: &AppHandle,
    order_id: String,
    clock: std::collections::HashMap<String, u64>,
    remote_peer: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("db locked")?;
    let mut book_guard = state.book.lock().map_err(|e| e.to_string())?;
    let book = book_guard.as_mut().ok_or("book not loaded")?;

    book.apply_remote_cancel(&order_id, &clock, &remote_peer)?;

    if let Some(entry) = book.get(&order_id) {
        let tombstone_str = entry.tombstone.map(|t| match t {
            crate::orderbook::TombstoneKind::Cancelled => "Cancelled",
            crate::orderbook::TombstoneKind::Filled => "Filled",
        });
        crate::db::save_order(db, &entry.order, tombstone_str, entry.tombstoned_at)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Apply a trade received from a peer via IPC.
/// Merges signatures with any local copy — verification happens here so the
/// relay path is self-contained.
fn apply_incoming_trade(app: &AppHandle, incoming: crate::trade::Trade) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("db locked")?;

    // Verify whichever signature came in
    if !incoming.buyer_signature.is_empty() {
        crate::trade::verify_buyer_signature(&incoming)?;
    }
    if !incoming.seller_signature.is_empty() {
        crate::trade::verify_seller_signature(&incoming)?;
    }

    // Merge with local copy if we have one
    let merged = match crate::db::load_trade(db, &incoming.id).map_err(|e| e.to_string())? {
        Some(mut local) => {
            if local.buyer_signature.is_empty() {
                local.buyer_signature = incoming.buyer_signature.clone();
            }
            if local.seller_signature.is_empty() {
                local.seller_signature = incoming.seller_signature.clone();
            }
            local
        }
        None => incoming,
    };

    crate::db::save_trade_agreement(db, &merged).map_err(|e| e.to_string())?;
    log::info!("applied remote trade {}", merged.id);
    Ok(())
}
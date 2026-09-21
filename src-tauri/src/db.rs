// Yonodex Desktop Client - Encrypted Database Module
// Whitepaper Layer 3, Section 5.2: Encrypted Local Database
//
// SQLCipher-backed SQLite with AES-256-GCM encryption.
// Key derived from master password via Argon2id (see crypto.rs).
// Salt stored in a sibling file (salt is not secret).

use crate::crypto::{derive_key, generate_salt, key_to_sqlcipher_hex, wipe_key};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_SQL: &str = include_str!("schema.sql");

pub struct DbHandle {
    pub conn: Connection,
    #[allow(dead_code)]
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum DbError {
    Io(String),
    Sqlite(String),
    Crypto(String),
    WrongPassword,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Io(s) => write!(f, "io: {s}"),
            DbError::Sqlite(s) => write!(f, "sqlite: {s}"),
            DbError::Crypto(s) => write!(f, "crypto: {s}"),
            DbError::WrongPassword => write!(f, "wrong password or corrupted database"),
        }
    }
}

impl std::error::Error for DbError {}

fn salt_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.to_path_buf();
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "yonodex.db".to_string());
    p.set_file_name(format!("{name}.salt"));
    p
}

fn hint_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.to_path_buf();
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "yonodex.db".to_string());
    p.set_file_name(format!("{name}.hint"));
    p
}

/// Returns true if the DB has been initialized (salt file exists).
pub fn is_initialized(db_path: &Path) -> bool {
    salt_path(db_path).exists()
}

/// Open (or create) the encrypted DB at `db_path`, unlocking it with `password`.
pub fn open(db_path: &Path, password: &str) -> Result<DbHandle, DbError> {
    let salt_file = salt_path(db_path);

    let salt = if salt_file.exists() {
        let bytes = fs::read(&salt_file).map_err(|e| DbError::Io(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(DbError::Io("salt file has wrong length".into()));
        }
        let mut s = [0u8; 32];
        s.copy_from_slice(&bytes);
        s
    } else {
        let s = generate_salt();
        if let Some(parent) = salt_file.parent() {
            fs::create_dir_all(parent).map_err(|e| DbError::Io(e.to_string()))?;
        }
        fs::write(&salt_file, s).map_err(|e| DbError::Io(e.to_string()))?;
        s
    };

    let mut key = derive_key(password, &salt).map_err(DbError::Crypto)?;
    let key_hex = key_to_sqlcipher_hex(&key);
    wipe_key(&mut key);

    let conn = Connection::open(db_path).map_err(|e| DbError::Sqlite(e.to_string()))?;

    conn.execute_batch(&format!("PRAGMA key = \"{key_hex}\";"))
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| DbError::WrongPassword)?;

    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(DbHandle {
        conn,
        path: db_path.to_path_buf(),
    })
}

// ---- Hint (plaintext memory aid) ----

pub fn save_hint(db_path: &Path, hint: &str) -> Result<(), DbError> {
    let path = hint_path(db_path);
    if hint.is_empty() {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| DbError::Io(e.to_string()))?;
        }
        return Ok(());
    }
    fs::write(&path, hint.as_bytes()).map_err(|e| DbError::Io(e.to_string()))?;
    Ok(())
}

pub fn load_hint(db_path: &Path) -> Option<String> {
    fs::read_to_string(hint_path(db_path)).ok()
}

// ---- Reset ----

pub fn reset_all(db_path: &Path) -> Result<(), DbError> {
    for path in [
        db_path.to_path_buf(),
        salt_path(db_path),
        hint_path(db_path),
    ] {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| DbError::Io(e.to_string()))?;
        }
    }
    Ok(())
}

// ---- Wallet config ----

pub fn save_wallet_config(
    db: &DbHandle,
    address: &str,
    chain_id: &str,
    wallet_uuid: &str,
    wallet_name: &str,
) -> Result<(), DbError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO wallet_config (id, address, chain_id, wallet_uuid, wallet_name, connected_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                address = excluded.address,
                chain_id = excluded.chain_id,
                wallet_uuid = excluded.wallet_uuid,
                wallet_name = excluded.wallet_name,
                connected_at = excluded.connected_at",
            rusqlite::params![address, chain_id, wallet_uuid, wallet_name, now],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

pub fn load_wallet_config(
    db: &DbHandle,
) -> Result<Option<(String, String, String, String)>, DbError> {
    let mut stmt = db
        .conn
        .prepare("SELECT address, chain_id, wallet_uuid, wallet_name FROM wallet_config WHERE id = 1")
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let row = stmt
        .query_row([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .ok();

    Ok(row)
}

pub fn clear_wallet_config(db: &DbHandle) -> Result<(), DbError> {
    db.conn
        .execute("DELETE FROM wallet_config WHERE id = 1", [])
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

// ---- Trade history (scoped per wallet) ----

#[derive(Debug, Clone)]
pub struct TradeRecord {
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

#[allow(clippy::too_many_arguments)]
pub fn save_trade(
    db: &DbHandle,
    wallet_address: &str,
    tx_hash: Option<&str>,
    chain_id: &str,
    pair: &str,
    side: &str,
    amount_in: &str,
    amount_out: &str,
    token_in: &str,
    token_out: &str,
    status: &str,
    notes: Option<&str>,
) -> Result<i64, DbError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO trade_history
             (wallet_address, tx_hash, chain_id, pair, side, amount_in, amount_out,
              token_in, token_out, status, timestamp, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                wallet_address, tx_hash, chain_id, pair, side, amount_in, amount_out,
                token_in, token_out, status, now, notes
            ],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(db.conn.last_insert_rowid())
}

/// Load trade history for a specific wallet address (newest first).
pub fn load_trades(
    db: &DbHandle,
    wallet_address: &str,
    limit: i64,
) -> Result<Vec<TradeRecord>, DbError> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, wallet_address, tx_hash, chain_id, pair, side, amount_in, amount_out,
                    token_in, token_out, status, timestamp, notes
             FROM trade_history
             WHERE wallet_address = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let rows = stmt
        .query_map(rusqlite::params![wallet_address, limit], |r| {
            Ok(TradeRecord {
                id: r.get(0)?,
                wallet_address: r.get(1)?,
                tx_hash: r.get(2)?,
                chain_id: r.get(3)?,
                pair: r.get(4)?,
                side: r.get(5)?,
                amount_in: r.get(6)?,
                amount_out: r.get(7)?,
                token_in: r.get(8)?,
                token_out: r.get(9)?,
                status: r.get(10)?,
                timestamp: r.get(11)?,
                notes: r.get(12)?,
            })
        })
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| DbError::Sqlite(e.to_string()))
}

pub fn delete_trade(db: &DbHandle, id: i64) -> Result<(), DbError> {
    db.conn
        .execute("DELETE FROM trade_history WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

// ---- Node identity (Tor .onion address) ----
// Whitepaper Layer 4, Section 6.1: every node has a stable Tor identity.
// The .onion address is derived from the Ed25519 keypair Tor generates on
// first launch. We store the address (not the key) so the app can display it
// and reason about node identity. The key stays in Tor's data directory.

/// Save or refresh the node identity. Idempotent - safe to call on every launch.
pub fn save_node_identity(db: &DbHandle, onion_address: &str) -> Result<(), DbError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO node_identity (id, onion_address, created_at, last_seen_at)
             VALUES (1, ?1, ?2, ?2)
             ON CONFLICT(id) DO UPDATE SET
                last_seen_at = excluded.last_seen_at",
            rusqlite::params![onion_address, now],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

/// Load the stored node identity (the .onion address), if present.
pub fn load_node_identity(db: &DbHandle) -> Result<Option<String>, DbError> {
    let mut stmt = db
        .conn
        .prepare("SELECT onion_address FROM node_identity WHERE id = 1")
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let row = stmt
        .query_row([], |r| r.get::<_, String>(0))
        .ok();

    Ok(row)
}

// ---- Order nonce tracking (replay protection) ----
// Whitepaper Layer 5, Section 7.1: nonce-based replay prevention.

/// Get the last seen nonce for an owner. Returns 0 if never seen.
pub fn get_last_nonce(db: &DbHandle, owner: &str) -> Result<u64, DbError> {
    let mut stmt = db
        .conn
        .prepare("SELECT last_nonce FROM order_nonces WHERE owner = ?1")
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let result = stmt
        .query_row(rusqlite::params![owner], |r| r.get::<_, i64>(0))
        .ok();

    Ok(result.unwrap_or(0) as u64)
}

/// Update the last seen nonce for an owner.
/// Only advances forward — will not accept a smaller value than what's stored.
pub fn set_last_nonce(db: &DbHandle, owner: &str, nonce: u64) -> Result<(), DbError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO order_nonces (owner, last_nonce, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(owner) DO UPDATE SET
                last_nonce = MAX(excluded.last_nonce, order_nonces.last_nonce),
                updated_at = excluded.updated_at",
            rusqlite::params![owner, nonce as i64, now],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(())
}

/// Check whether an incoming order's nonce is valid (strictly greater than last seen).
/// Returns Err if the nonce was already used (replay attack).
pub fn check_nonce(db: &DbHandle, owner: &str, nonce: u64) -> Result<(), DbError> {
    let last = get_last_nonce(db, owner)?;
    if nonce <= last {
        return Err(DbError::Sqlite(format!(
            "replay detected: nonce {} <= last seen {} for owner {}",
            nonce, last, owner
        )));
    }
    Ok(())
}

// ---- Order persistence ----
// Whitepaper Layer 5, Section 7.1: orders replicated via CRDT,
// persisted to encrypted DB so they survive restarts.

/// Save or update an order in the order_cache table.
/// Serializes the full Order struct to JSON for storage.
pub fn save_order(db: &DbHandle, order: &crate::order::Order, tombstone: Option<&str>, tombstoned_at: Option<i64>) -> Result<(), DbError> {
    let order_json = serde_json::to_string(order)
        .map_err(|e| DbError::Sqlite(format!("order serialize: {e}")))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO order_cache
                (order_id, pair, side, price, amount, owner, timestamp, order_json, tombstone, tombstoned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(order_id) DO UPDATE SET
                tombstone = excluded.tombstone,
                tombstoned_at = excluded.tombstoned_at,
                order_json = excluded.order_json",
            rusqlite::params![
                order.id,
                order.pair,
                order.side.as_str(),
                order.price,
                order.amount,
                order.owner,
                now,
                order_json,
                tombstone,
                tombstoned_at,
            ],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(())
}

/// Load all orders from the DB. Returns (Order, tombstone, tombstoned_at).
pub fn load_all_orders(
    db: &DbHandle,
) -> Result<Vec<(crate::order::Order, Option<String>, Option<i64>)>, DbError> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT order_json, tombstone, tombstoned_at
             FROM order_cache
             ORDER BY timestamp ASC",
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let rows = stmt
        .query_map([], |r| {
            let json: String = r.get(0)?;
            let tombstone: Option<String> = r.get(1)?;
            let tombstoned_at: Option<i64> = r.get(2)?;
            Ok((json, tombstone, tombstoned_at))
        })
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let mut out = Vec::new();
    for row in rows {
        let (json, tombstone, tombstoned_at) =
            row.map_err(|e| DbError::Sqlite(e.to_string()))?;
        let order: crate::order::Order = serde_json::from_str(&json)
            .map_err(|e| DbError::Sqlite(format!("order deserialize: {e}")))?;
        out.push((order, tombstone, tombstoned_at));
    }

    Ok(out)
}

/// Delete a single order by id (used by GC).
pub fn delete_order(db: &DbHandle, order_id: &str) -> Result<(), DbError> {
    db.conn
        .execute(
            "DELETE FROM order_cache WHERE order_id = ?1",
            rusqlite::params![order_id],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

/// Purge tombstones older than the retention window.
/// Returns the number of rows deleted.
pub fn purge_old_tombstones(db: &DbHandle, retention_secs: i64) -> Result<usize, DbError> {
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        - retention_secs;

    let affected = db
        .conn
        .execute(
            "DELETE FROM order_cache
             WHERE tombstone IS NOT NULL AND tombstoned_at IS NOT NULL AND tombstoned_at < ?1",
            rusqlite::params![cutoff],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(affected)
}

// ---- Signing identity (Ed25519 keypair for order signing) ----
// Whitepaper Layer 5, Section 7.1: orders signed by creator.
// Whitepaper Layer 3, Section 5.2: private keys stay in encrypted DB.

/// Generate a fresh random Ed25519 keypair and store it as the signing identity.
/// Idempotent — if an identity already exists, does nothing and returns it.
///
/// Returns (owner_hex, private_hex).
pub fn ensure_signing_identity(db: &DbHandle) -> Result<(String, String), DbError> {
    // If one already exists, return it
    if let Some(existing) = load_signing_identity(db)? {
        return Ok(existing);
    }

        // Generate 32 random bytes with our existing rand 0.9, then construct
    // the Ed25519 key from those bytes. Avoids rand_core version conflicts
    // between rand 0.9 and ed25519-dalek's internal rand_core 0.6.
    let mut secret_bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut secret_bytes);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_bytes);

    let owner_hex = hex::encode(signing_key.verifying_key().to_bytes());
    let private_hex = hex::encode(signing_key.to_bytes());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    db.conn
        .execute(
            "INSERT INTO signing_identity
                (id, scheme, owner_hex, private_hex, derived_from, created_at)
             VALUES (1, ?1, ?2, ?3, NULL, ?4)",
            rusqlite::params!["random_v1", owner_hex, private_hex, now],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok((owner_hex, private_hex))
}

/// Load the stored signing identity if present. Returns (owner_hex, private_hex).
pub fn load_signing_identity(db: &DbHandle) -> Result<Option<(String, String)>, DbError> {
    let mut stmt = db
        .conn
        .prepare("SELECT owner_hex, private_hex FROM signing_identity WHERE id = 1")
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let row = stmt
        .query_row([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .ok();

    Ok(row)
}
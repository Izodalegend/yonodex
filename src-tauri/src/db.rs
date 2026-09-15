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

/// Open (or create) the encrypted DB at `db_path`, unlocking it with `password`.
///
/// First run: generates a fresh salt, derives a key, creates the DB, applies schema.
/// Subsequent runs: loads the existing salt, derives the key, attempts to read from
/// the DB. If SQLCipher rejects the key (wrong password), returns `WrongPassword`.
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

    // Apply SQLCipher key. Must be the first statement after opening.
    conn.execute_batch(&format!("PRAGMA key = \"{key_hex}\";"))
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    // Force a real read so wrong passwords are detected immediately.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| DbError::WrongPassword)?;

    // First-run: create schema.
    conn.execute_batch(SCHEMA_SQL)
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(DbHandle {
        conn,
        path: db_path.to_path_buf(),
    })
}

/// Save the wallet connection config (replaces localStorage).
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

/// Load the wallet connection config, if present.
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

/// Clear the wallet connection config (disconnect).
pub fn clear_wallet_config(db: &DbHandle) -> Result<(), DbError> {
    db.conn
        .execute("DELETE FROM wallet_config WHERE id = 1", [])
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}
/// Returns true if the DB has been initialized (salt file exists).
/// Used by the frontend to distinguish first-run setup from unlock.
pub fn is_initialized(db_path: &Path) -> bool {
    salt_path(db_path).exists()
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub id: i64,
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

/// Save a trade record to history.
#[allow(clippy::too_many_arguments)]
pub fn save_trade(
    db: &DbHandle,
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
             (tx_hash, chain_id, pair, side, amount_in, amount_out, token_in, token_out, status, timestamp, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                tx_hash, chain_id, pair, side, amount_in, amount_out, token_in, token_out,
                status, now, notes
            ],
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    Ok(db.conn.last_insert_rowid())
}

/// Load recent trade history (newest first).
pub fn load_trades(db: &DbHandle, limit: i64) -> Result<Vec<TradeRecord>, DbError> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, tx_hash, chain_id, pair, side, amount_in, amount_out,
                    token_in, token_out, status, timestamp, notes
             FROM trade_history
             ORDER BY timestamp DESC
             LIMIT ?1",
        )
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    let rows = stmt
        .query_map(rusqlite::params![limit], |r| {
            Ok(TradeRecord {
                id: r.get(0)?,
                tx_hash: r.get(1)?,
                chain_id: r.get(2)?,
                pair: r.get(3)?,
                side: r.get(4)?,
                amount_in: r.get(5)?,
                amount_out: r.get(6)?,
                token_in: r.get(7)?,
                token_out: r.get(8)?,
                status: r.get(9)?,
                timestamp: r.get(10)?,
                notes: r.get(11)?,
            })
        })
        .map_err(|e| DbError::Sqlite(e.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| DbError::Sqlite(e.to_string()))
}

/// Delete a single trade by id.
pub fn delete_trade(db: &DbHandle, id: i64) -> Result<(), DbError> {
    db.conn
        .execute("DELETE FROM trade_history WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| DbError::Sqlite(e.to_string()))?;
    Ok(())
}

/// Path to the plaintext hint file (sibling to salt).
/// The hint is NOT secret - it exists purely as a memory aid for the user.
pub fn hint_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.to_path_buf();
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "yonodex.db".to_string());
    p.set_file_name(format!("{name}.hint"));
    p
}

/// Save the password hint (plaintext file).
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

/// Load the password hint, if any.
pub fn load_hint(db_path: &Path) -> Option<String> {
    let path = hint_path(db_path);
    fs::read_to_string(&path).ok()
}

/// Delete DB, salt, and hint. Used by "Reset Local Data".
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
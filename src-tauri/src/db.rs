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
// Yonodex Desktop Client - Cryptography
// Whitepaper Layer 3, Section 5.2: Argon2id key derivation
// Parameters from whitepaper: memory=64MB, iterations=3, parallelism=4
//
// This module derives the SQLCipher database encryption key from the user's
// master password. The key is held only in memory during app runtime - it is
// never written to disk. The salt is stored alongside the DB file (salt is not
// secret; the password is what protects the key).

use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

// Whitepaper-mandated parameters
const ARGON2_MEMORY_KIB: u32 = 64 * 1024; // 64 MB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32; // 256-bit key for AES-256-GCM

/// Generate a random 32-byte salt.
/// Called once when the DB is first created; salt is stored next to the DB.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::rng().fill_bytes(&mut salt);
    salt
}

/// Derive a 32-byte key from the master password and salt using Argon2id.
/// Returns the raw key bytes ready to pass to SQLCipher's PRAGMA key.
pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_LEN),
    )
    .map_err(|e| format!("argon2 params: {e}"))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("argon2 derive: {e}"))?;

    Ok(key)
}

/// Format the derived key as a SQLCipher-compatible hex string.
/// SQLCipher accepts: PRAGMA key = "x'<hex>'";
pub fn key_to_sqlcipher_hex(key: &[u8; KEY_LEN]) -> String {
    format!("x'{}'", hex::encode(key))
}

/// Wipe a key from memory. Best-effort (Rust cannot guarantee zeroing).
pub fn wipe_key(key: &mut [u8; KEY_LEN]) {
    for byte in key.iter_mut() {
        *byte = 0;
    }
}
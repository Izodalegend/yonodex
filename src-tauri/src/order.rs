// Yonodex Desktop Client - Order struct
// Whitepaper Layer 5, Section 7.1: Off-Chain Order Book
//
// Field definitions match the whitepaper exactly:
// id, owner, side, pair, price, amount, expiry, nonce, signature, vector_clock

use serde::{Deserialize, Serialize};

/// Which side of the book — buy or sell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
}

/// A signed order in the shared order book.
///
/// The `signature` field covers the canonical hash of all other fields
/// (excluding `signature` itself). See `signable_bytes()` for the exact
/// byte sequence that gets hashed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier (client-generated).
    pub id: String,

    /// Ed25519 public key of the creator, hex-encoded.
    pub owner: String,

    /// Buy or sell.
    pub side: Side,

    /// Trading pair, e.g. "TKA/TKB".
    pub pair: String,

    /// Limit price as a decimal string (to preserve precision).
    pub price: String,

    /// Order quantity as a decimal string.
    pub amount: String,

    /// Unix timestamp (seconds) when the order expires.
    pub expiry: i64,

    /// Per-owner monotonically increasing counter. Rejects replays.
    pub nonce: u64,

    /// Ed25519 signature of `signable_bytes()`, hex-encoded.
    pub signature: String,

    /// Vector clock — logical timestamp used for CRDT ordering.
    /// Map of peer_id -> counter.
    pub vector_clock: crate::clock::VectorClock,
}

impl Order {
    /// The canonical byte sequence that gets signed.
    ///
    /// Deterministic order of fields so all peers hash the same bytes.
    /// Format: pipe-delimited, no ambiguity.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut s = String::new();
        s.push_str(&self.id);
        s.push('|');
        s.push_str(&self.owner);
        s.push('|');
        s.push_str(self.side.as_str());
        s.push('|');
        s.push_str(&self.pair);
        s.push('|');
        s.push_str(&self.price);
        s.push('|');
        s.push_str(&self.amount);
        s.push('|');
        s.push_str(&self.expiry.to_string());
        s.push('|');
        s.push_str(&self.nonce.to_string());
        s.push('|');

        // Vector clock — sort keys for determinism
        let mut keys: Vec<_> = self.vector_clock.keys().cloned().collect();
        keys.sort();
        for k in keys {
            s.push_str(&k);
            s.push(':');
            s.push_str(&self.vector_clock[&k].to_string());
            s.push(',');
        }

        s.into_bytes()
    }
}

// ---- Signing + verification ----

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// Sign the order with the given Ed25519 signing key.
/// Returns the hex-encoded signature. Does NOT mutate the order —
/// the caller is responsible for setting `order.signature` on the result.
pub fn sign_order(order: &Order, signing_key: &SigningKey) -> String {
    let bytes = order.signable_bytes();
    let signature: Signature = signing_key.sign(&bytes);
    hex::encode(signature.to_bytes())
}

/// Verify the order's signature against the public key embedded in `order.owner`.
///
/// Returns Ok(()) if the signature is valid, Err with a description otherwise.
pub fn verify_order(order: &Order) -> Result<(), String> {
    // Decode owner public key
    let owner_bytes = hex::decode(&order.owner)
        .map_err(|e| format!("invalid owner hex: {e}"))?;
    if owner_bytes.len() != 32 {
        return Err(format!("owner must be 32 bytes, got {}", owner_bytes.len()));
    }
    let mut owner_arr = [0u8; 32];
    owner_arr.copy_from_slice(&owner_bytes);
    let verifying_key = VerifyingKey::from_bytes(&owner_arr)
        .map_err(|e| format!("invalid owner public key: {e}"))?;

    // Decode signature
    let sig_bytes = hex::decode(&order.signature)
        .map_err(|e| format!("invalid signature hex: {e}"))?;
    if sig_bytes.len() != 64 {
        return Err(format!("signature must be 64 bytes, got {}", sig_bytes.len()));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    // Verify against the canonical signable bytes
    let bytes = order.signable_bytes();
    verifying_key
        .verify(&bytes, &signature)
        .map_err(|e| format!("signature verification failed: {e}"))
}
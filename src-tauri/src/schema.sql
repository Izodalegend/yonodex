-- Yonodex Desktop Client - Encrypted DB Schema
-- Whitepaper Layer 3, Section 5.2: Encrypted Local Database
-- All data in this DB is encrypted at rest via SQLCipher (AES-256-GCM).
-- No private keys, no seed phrases, no signing material is ever stored here.

PRAGMA foreign_keys = ON;

-- Node identity - persistent .onion address for this client's Tor hidden service.
-- Whitepaper Layer 4, Section 6.1: every node has a stable Tor identity.
-- Single row (id=1) - the node has exactly one identity.
CREATE TABLE IF NOT EXISTS node_identity (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    onion_address   TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    last_seen_at    INTEGER NOT NULL
);

-- Schema version tracking (single row, id=1)
CREATE TABLE IF NOT EXISTS schema_meta (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    version         INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- Wallet connection config (replaces localStorage yonodex.wallet.connection)
-- Whitepaper Layer 3, Section 5.1: local-first, no cloud account required.
CREATE TABLE IF NOT EXISTS wallet_config (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    address         TEXT NOT NULL,
    chain_id        TEXT NOT NULL,
    wallet_uuid     TEXT NOT NULL,
    wallet_name     TEXT NOT NULL,
    connected_at    INTEGER NOT NULL
);

-- Trade history - scoped per wallet address.
-- Whitepaper Layer 3, Section 5.2: user's own trades only.
CREATE TABLE IF NOT EXISTS trade_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_address  TEXT NOT NULL,
    tx_hash         TEXT,
    chain_id        TEXT NOT NULL,
    pair            TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    amount_in       TEXT NOT NULL,
    amount_out      TEXT NOT NULL,
    token_in        TEXT NOT NULL,
    token_out       TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending', 'confirmed', 'failed')),
    timestamp       INTEGER NOT NULL,
    notes           TEXT
);

CREATE INDEX IF NOT EXISTS idx_trade_history_wallet_timestamp
    ON trade_history(wallet_address, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_trade_history_pair
    ON trade_history(pair);

-- Peer address book (P2P layer - used in 1.D and beyond)
-- Whitepaper Layer 3, Section 5.2: peer cache with optional expiration.
CREATE TABLE IF NOT EXISTS peer_cache (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id         TEXT UNIQUE NOT NULL,
    multiaddr       TEXT NOT NULL,
    last_seen       INTEGER NOT NULL,
    score           INTEGER DEFAULT 0,
    expires_at      INTEGER
);

CREATE INDEX IF NOT EXISTS idx_peer_cache_last_seen
    ON peer_cache(last_seen DESC);

-- Order cache — full CRDT order state
-- Whitepaper Layer 5, Section 7.1: orders persisted so they survive restarts.
-- The authoritative store is `order_json` (serialized Order struct).
-- The other columns are denormalized for querying/indexing.
CREATE TABLE IF NOT EXISTS order_cache (
    order_id        TEXT PRIMARY KEY,
    pair            TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price           TEXT NOT NULL,
    amount          TEXT NOT NULL,
    owner           TEXT NOT NULL,
    timestamp       INTEGER NOT NULL,
    order_json      TEXT NOT NULL,
    tombstone       TEXT,
    tombstoned_at   INTEGER
);

CREATE INDEX IF NOT EXISTS idx_order_cache_live
    ON order_cache(pair, side, timestamp DESC)
    WHERE tombstone IS NULL;

CREATE INDEX IF NOT EXISTS idx_order_cache_pair_timestamp
    ON order_cache(pair, timestamp DESC);

-- App settings (user preferences)
CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- Initialize schema version on first run
INSERT OR IGNORE INTO schema_meta (id, version, created_at, updated_at)
VALUES (1, 1, strftime('%s', 'now'), strftime('%s', 'now'));

-- Per-owner nonce tracking for replay protection.
-- Whitepaper Layer 5, Section 7.1: nonce-based replay prevention.
-- Each owner has a monotonically increasing nonce; any order with a
-- nonce <= the last seen nonce for that owner is rejected.
CREATE TABLE IF NOT EXISTS order_nonces (
    owner           TEXT PRIMARY KEY,
    last_nonce      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- Signing identity — Ed25519 keypair used to sign orders.
-- Whitepaper Layer 5, Section 7.1: orders are signed by their creator.
-- Whitepaper Layer 3, Section 5.2: private keys never leave the encrypted DB.
--
-- Single row (id=1) - the node has exactly one active signing identity.
--
-- `scheme` allows future migration without breaking existing orders:
--   - 'random_v1' : random keypair generated on first unlock (current)
--   - 'derived_v1': deterministically derived from password + wallet (future)
-- Old orders signed under a previous scheme remain valid forever -
-- verification only checks pubkey + signature, not generation method.
CREATE TABLE IF NOT EXISTS signing_identity (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    scheme          TEXT NOT NULL,
    owner_hex       TEXT NOT NULL,
    private_hex     TEXT NOT NULL,
    derived_from    TEXT,
    created_at      INTEGER NOT NULL
);
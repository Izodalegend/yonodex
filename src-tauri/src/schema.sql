-- Yonodex Desktop Client - Encrypted DB Schema
-- Whitepaper Layer 3, Section 5.2: Encrypted Local Database
-- All data in this DB is encrypted at rest via SQLCipher (AES-256-GCM).
-- No private keys, no seed phrases, no signing material is ever stored here.

PRAGMA foreign_keys = ON;

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

-- Trade history (user's own trades only - no counterparty data)
-- Whitepaper Layer 3, Section 5.2: trade history retained until user deletes.
CREATE TABLE IF NOT EXISTS trade_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tx_hash         TEXT UNIQUE,
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

CREATE INDEX IF NOT EXISTS idx_trade_history_timestamp
    ON trade_history(timestamp DESC);

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

-- Order book cache (populated once protocol layer is live)
-- Whitepaper Layer 3, Section 5.2: last 1000 orders per market, auto-pruned.
CREATE TABLE IF NOT EXISTS order_cache (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id        TEXT UNIQUE NOT NULL,
    pair            TEXT NOT NULL,
    side            TEXT NOT NULL CHECK (side IN ('buy', 'sell')),
    price           TEXT NOT NULL,
    amount          TEXT NOT NULL,
    owner           TEXT NOT NULL,
    timestamp       INTEGER NOT NULL
);

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
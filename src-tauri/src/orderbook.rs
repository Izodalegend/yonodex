// Yonodex Desktop Client - CRDT Order Book
// Whitepaper Layer 5, Section 7.1: Off-Chain Order Book
//
// Operation-based CRDT (CmRDT). Each order is an operation that can be
// applied in any order. Convergence is guaranteed by:
//   - Order IDs are unique (no two operations conflict on identity)
//   - Cancellations and fills are tombstoned with a vector clock
//   - Concurrent conflicting operations use LWW (Last-Write-Wins)
//   - Tombstones GC'd after retention window (default 24h)
//
// The local clock ticks on every local mutation and merges on every
// remote mutation, giving us a stable causal ordering.

use crate::clock::{self, ClockOrdering, VectorClock};
use crate::db::{self, DbHandle};
use crate::order::{self, Order, Side};
use ed25519_dalek::SigningKey;
use std::collections::HashMap;

/// Retention window for tombstones: 24 hours (in seconds).
pub const TOMBSTONE_RETENTION_SECS: i64 = 24 * 60 * 60;

/// Tombstone reason — why an order is no longer live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TombstoneKind {
    Cancelled,
    Filled,
}

/// An order plus its current state in the book.
#[derive(Debug, Clone)]
pub struct BookEntry {
    pub order: Order,
    pub tombstone: Option<TombstoneKind>,
    /// When this entry was marked as tombstoned (unix seconds). Used for GC.
    pub tombstoned_at: Option<i64>,
}

/// In-memory CRDT order book.
pub struct OrderBook {
    /// order_id -> entry
    entries: HashMap<String, BookEntry>,
    /// Local vector clock — ticks on every local operation, merges on remote.
    clock: VectorClock,
    /// This node's peer identifier (used as the key in the vector clock).
    local_peer: String,
}

impl OrderBook {
    pub fn new(local_peer: String) -> Self {
        Self {
            entries: HashMap::new(),
            clock: HashMap::new(),
            local_peer,
        }
    }

    pub fn local_peer(&self) -> &str {
        &self.local_peer
    }

    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }

    /// Number of live (non-tombstoned) orders.
    pub fn live_count(&self) -> usize {
        self.entries.values().filter(|e| e.tombstone.is_none()).count()
    }

    /// Total entries (live + tombstones).
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Get a live entry by order id.
    pub fn get(&self, order_id: &str) -> Option<&BookEntry> {
        self.entries.get(order_id)
    }

    /// Iterate over all live orders.
    pub fn live_orders(&self) -> impl Iterator<Item = &Order> {
        self.entries
            .values()
            .filter(|e| e.tombstone.is_none())
            .map(|e| &e.order)
    }

    /// Create a new order locally: sign, tick clock, add to book, persist nonce.
    pub fn create_local(
        &mut self,
        db: &DbHandle,
        signing_key: &SigningKey,
        owner: &str,
        side: Side,
        pair: &str,
        price: &str,
        amount: &str,
        expiry: i64,
    ) -> Result<Order, String> {
        // Generate a unique order id (timestamp + owner prefix for readability)
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("{}-{}", now_millis, &owner[..8.min(owner.len())]);

        // Get next nonce from DB
        let last_nonce = db::get_last_nonce(db, owner).map_err(|e| e.to_string())?;
        let nonce = last_nonce + 1;

        // Tick clock
        clock::tick(&mut self.clock, &self.local_peer);

        // Build order (signature empty for now)
        let mut order = Order {
            id: id.clone(),
            owner: owner.to_string(),
            side,
            pair: pair.to_string(),
            price: price.to_string(),
            amount: amount.to_string(),
            expiry,
            nonce,
            signature: String::new(),
            vector_clock: self.clock.clone(),
        };

        // Sign
        order.signature = order::sign_order(&order, signing_key);

        // Persist nonce BEFORE adding to book (so restart recovery is safe)
        db::set_last_nonce(db, owner, nonce).map_err(|e| e.to_string())?;

        // Add to book
        self.entries.insert(
            id.clone(),
            BookEntry {
                order: order.clone(),
                tombstone: None,
                tombstoned_at: None,
            },
        );

        Ok(order)
    }

    /// Apply a remote order received from the network.
    /// Validates signature, checks nonce, merges clock, adds to book.
    pub fn apply_remote(
        &mut self,
        db: &DbHandle,
        incoming: Order,
    ) -> Result<(), String> {
        // 1. Verify signature
        order::verify_order(&incoming)?;

        // 2. Check nonce (replay protection)
        db::check_nonce(db, &incoming.owner, incoming.nonce)
            .map_err(|e| e.to_string())?;

        // 3. Duplicate check — if we've already seen this order id, ignore
        if self.entries.contains_key(&incoming.id) {
            return Ok(());
        }

        // 4. Merge clock
        self.clock = clock::merge(&self.clock, &incoming.vector_clock);

        // 5. Advance nonce tracker
        db::set_last_nonce(db, &incoming.owner, incoming.nonce)
            .map_err(|e| e.to_string())?;

        // 6. Add to book
        self.entries.insert(
            incoming.id.clone(),
            BookEntry {
                order: incoming,
                tombstone: None,
                tombstoned_at: None,
            },
        );

        Ok(())
    }

    /// Cancel an order locally. Marks the entry as tombstoned, ticks clock.
    pub fn cancel_local(&mut self, order_id: &str) -> Result<(), String> {
        let entry = self
            .entries
            .get_mut(order_id)
            .ok_or_else(|| format!("order {order_id} not found"))?;

        if entry.tombstone.is_some() {
            return Err(format!("order {order_id} already tombstoned"));
        }

        clock::tick(&mut self.clock, &self.local_peer);
        entry.tombstone = Some(TombstoneKind::Cancelled);
        entry.tombstoned_at = Some(now_secs());
        Ok(())
    }

    /// Apply a remote cancellation. Uses LWW semantics via vector clocks.
    pub fn apply_remote_cancel(
        &mut self,
        order_id: &str,
        remote_clock: &VectorClock,
        remote_peer: &str,
    ) -> Result<(), String> {
        let entry = match self.entries.get_mut(order_id) {
            Some(e) => e,
            None => {
                // Order not seen yet — merge clock anyway so we're not blindsided later
                self.clock = clock::merge(&self.clock, remote_clock);
                return Ok(());
            }
        };

        // If already tombstoned, use vector clock comparison to decide winner
        if entry.tombstone.is_some() {
            let ordering = clock::compare(&entry.order.vector_clock, remote_clock);
            match ordering {
                ClockOrdering::Before | ClockOrdering::Equal => {
                    entry.tombstone = Some(TombstoneKind::Cancelled);
                    entry.tombstoned_at = Some(now_secs());
                }
                ClockOrdering::After => {
                    // Our state is newer — ignore
                }
                ClockOrdering::Concurrent => {
                    // Tie-break: higher peer_id wins (deterministic across all nodes)
                    if remote_peer > self.local_peer.as_str() {
                        entry.tombstone = Some(TombstoneKind::Cancelled);
                        entry.tombstoned_at = Some(now_secs());
                    }
                }
            }
        } else {
            entry.tombstone = Some(TombstoneKind::Cancelled);
            entry.tombstoned_at = Some(now_secs());
        }

        // Merge clock
        self.clock = clock::merge(&self.clock, remote_clock);
        Ok(())
    }

    /// Mark an order as filled (tombstone as Filled). Called by the matching engine.
    pub fn mark_filled(&mut self, order_id: &str) -> Result<(), String> {
        let entry = self
            .entries
            .get_mut(order_id)
            .ok_or_else(|| format!("order {order_id} not found"))?;

        clock::tick(&mut self.clock, &self.local_peer);
        entry.tombstone = Some(TombstoneKind::Filled);
        entry.tombstoned_at = Some(now_secs());
        Ok(())
    }

    /// Garbage collect tombstones older than the retention window.
    /// Returns the number of entries removed.
    pub fn gc(&mut self) -> usize {
        let cutoff = now_secs() - TOMBSTONE_RETENTION_SECS;
        let before = self.entries.len();

        self.entries.retain(|_, entry| match entry.tombstoned_at {
            Some(ts) => ts > cutoff, // keep recent tombstones
            None => true,            // always keep live orders
        });

        before - self.entries.len()
    }

    /// Merge another OrderBook's state into this one.
    /// Used for state reconciliation after a network partition.
    pub fn merge_from(&mut self, other: &OrderBook) {
        for (id, other_entry) in &other.entries {
            match self.entries.get_mut(id) {
                None => {
                    self.entries.insert(id.clone(), other_entry.clone());
                }
                Some(ours) => {
                    let ordering =
                        clock::compare(&ours.order.vector_clock, &other_entry.order.vector_clock);
                    if matches!(ordering, ClockOrdering::Before | ClockOrdering::Concurrent)
                        && other_entry.tombstone.is_some()
                    {
                        ours.tombstone = other_entry.tombstone;
                        ours.tombstoned_at = other_entry.tombstoned_at;
                    }
                }
            }
        }

        self.clock = clock::merge(&self.clock, &other.clock);
    }

    /// Restore a single order from the encrypted DB on app startup.
    /// Bypasses signature/nonce checks — the DB is the trusted local store.
    ///
    /// IMPORTANT: merges the persisted order's vector clock into the local
    /// clock so the clock resumes from where it left off, not from empty.
    /// Without this, local ops after a restart would collide with pre-restart
    /// clock values and break causal ordering.
    pub fn restore_persisted(
        &mut self,
        order: Order,
        tombstone: Option<String>,
        tombstoned_at: Option<i64>,
    ) {
        // Resume local clock from the highest value we've ever seen.
        self.clock = clock::merge(&self.clock, &order.vector_clock);

        let kind = tombstone.as_deref().and_then(|s| match s {
            "Cancelled" => Some(TombstoneKind::Cancelled),
            "Filled" => Some(TombstoneKind::Filled),
            _ => None,
        });

        self.entries.insert(
            order.id.clone(),
            BookEntry {
                order,
                tombstone: kind,
                tombstoned_at,
            },
        );
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
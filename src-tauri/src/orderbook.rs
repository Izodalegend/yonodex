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
// Fills are tracked separately (order_id -> cumulative filled amount)
// so the signed Order struct stays immutable.

use crate::clock::{self, ClockOrdering, VectorClock};
use crate::db::{self, DbHandle};
use crate::order::{self, Order, Side};
use ed25519_dalek::SigningKey;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

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
    /// order_id -> cumulative filled amount (across all matches)
    fills: HashMap<String, Decimal>,
    /// Local vector clock — ticks on every local operation, merges on remote.
    clock: VectorClock,
    /// This node's peer identifier (used as the key in the vector clock).
    local_peer: String,
}

impl OrderBook {
    pub fn new(local_peer: String) -> Self {
        Self {
            entries: HashMap::new(),
            fills: HashMap::new(),
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

    /// How much of an order is still fillable.
    /// Returns 0 if the order is unknown or already fully filled.
    pub fn remaining(&self, order_id: &str) -> Decimal {
        let entry = match self.entries.get(order_id) {
            Some(e) => e,
            None => return Decimal::ZERO,
        };

        let total = Decimal::from_str(&entry.order.amount).unwrap_or(Decimal::ZERO);
        let filled = self.fills.get(order_id).copied().unwrap_or(Decimal::ZERO);

        if filled >= total {
            Decimal::ZERO
        } else {
            total - filled
        }
    }

    /// Record a fill against an order. Fills accumulate.
    pub fn record_fill(&mut self, order_id: &str, amount: Decimal) -> Result<(), String> {
        if !self.entries.contains_key(order_id) {
            return Err(format!("unknown order {order_id}"));
        }
        let entry = self.fills.entry(order_id.to_string()).or_insert(Decimal::ZERO);
        *entry += amount;
        Ok(())
    }

    /// Iterate over live orders that still have remaining amount to fill.
    pub fn fillable_orders(&self) -> Vec<&Order> {
        self.entries
            .values()
            .filter(|e| e.tombstone.is_none())
            .filter(|e| self.remaining(&e.order.id) > Decimal::ZERO)
            .map(|e| &e.order)
            .collect()
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
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("{}-{}", now_millis, &owner[..8.min(owner.len())]);

        let last_nonce = db::get_last_nonce(db, owner).map_err(|e| e.to_string())?;
        let nonce = last_nonce + 1;

        clock::tick(&mut self.clock, &self.local_peer);

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

        order.signature = order::sign_order(&order, signing_key);

        db::set_last_nonce(db, owner, nonce).map_err(|e| e.to_string())?;

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
    pub fn apply_remote(&mut self, db: &DbHandle, incoming: Order) -> Result<(), String> {
        order::verify_order(&incoming)?;

        db::check_nonce(db, &incoming.owner, incoming.nonce).map_err(|e| e.to_string())?;

        if self.entries.contains_key(&incoming.id) {
            return Ok(());
        }

        self.clock = clock::merge(&self.clock, &incoming.vector_clock);

        db::set_last_nonce(db, &incoming.owner, incoming.nonce).map_err(|e| e.to_string())?;

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

    /// Cancel an order locally.
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

    /// Apply a remote cancellation. LWW semantics via vector clocks.
    pub fn apply_remote_cancel(
        &mut self,
        order_id: &str,
        remote_clock: &VectorClock,
        remote_peer: &str,
    ) -> Result<(), String> {
        let entry = match self.entries.get_mut(order_id) {
            Some(e) => e,
            None => {
                self.clock = clock::merge(&self.clock, remote_clock);
                return Ok(());
            }
        };

        if entry.tombstone.is_some() {
            let ordering = clock::compare(&entry.order.vector_clock, remote_clock);
            match ordering {
                ClockOrdering::Before | ClockOrdering::Equal => {
                    entry.tombstone = Some(TombstoneKind::Cancelled);
                    entry.tombstoned_at = Some(now_secs());
                }
                ClockOrdering::After => {}
                ClockOrdering::Concurrent => {
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

        self.clock = clock::merge(&self.clock, remote_clock);
        Ok(())
    }

    /// Mark an order as filled.
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
    pub fn gc(&mut self) -> usize {
        let cutoff = now_secs() - TOMBSTONE_RETENTION_SECS;
        let before = self.entries.len();

        self.entries.retain(|_, entry| match entry.tombstoned_at {
            Some(ts) => ts > cutoff,
            None => true,
        });

        before - self.entries.len()
    }

    /// Merge another OrderBook's state into this one.
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

        for (order_id, amount) in &other.fills {
            let entry = self.fills.entry(order_id.clone()).or_insert(Decimal::ZERO);
            if *amount > *entry {
                *entry = *amount;
            }
        }

        self.clock = clock::merge(&self.clock, &other.clock);
    }

    /// Restore a single order from the encrypted DB on app startup.
    /// Merges the persisted order's vector clock into the local clock so
    /// the clock resumes from where it left off, not from empty.
    pub fn restore_persisted(
        &mut self,
        order: Order,
        tombstone: Option<String>,
        tombstoned_at: Option<i64>,
    ) {
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

    /// Test-only: insert an order without signature or nonce checks.
    /// Used by unit tests to build a known book state quickly.
    #[cfg(test)]
    pub fn insert_for_test(&mut self, order: Order) {
        self.entries.insert(
            order.id.clone(),
            BookEntry {
                order,
                tombstone: None,
                tombstoned_at: None,
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
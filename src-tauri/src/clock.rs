// Yonodex Desktop Client - Vector Clock
// Whitepaper Layer 5, Section 7.1: logical timestamp for CRDT ordering
//
// A vector clock is a map of peer_id -> counter. It lets us determine
// whether two events are causally related, concurrent, or identical.
//
// Two operations:
//   - tick(local_peer): increment our own counter (on every local order create/cancel)
//   - merge(a, b): element-wise max (on every received order)
//
// Comparison rules (standard Lamport):
//   - a < b  if every entry in a <= b, and at least one is strictly less
//   - a > b  if every entry in a >= b, and at least one is strictly greater
//   - a == b if every entry matches
//   - a || b otherwise (concurrent — CRDT tie-break needed)

use std::collections::HashMap;

pub type VectorClock = HashMap<String, u64>;

/// Increment the local peer's counter in the given clock.
/// Called whenever this node produces a new event (create order, cancel, etc.).
pub fn tick(clock: &mut VectorClock, local_peer: &str) {
    let entry = clock.entry(local_peer.to_string()).or_insert(0);
    *entry += 1;
}

/// Merge two vector clocks into a new clock using element-wise max.
/// Called when we receive an event from a peer: our clock becomes
/// merge(our_clock, event_clock).
pub fn merge(a: &VectorClock, b: &VectorClock) -> VectorClock {
    let mut out = a.clone();
    for (k, &v) in b {
        let entry = out.entry(k.clone()).or_insert(0);
        if v > *entry {
            *entry = v;
        }
    }
    out
}

/// Comparison outcome between two vector clocks.
#[derive(Debug, PartialEq, Eq)]
pub enum ClockOrdering {
    /// a happened strictly before b (a < b)
    Before,
    /// a happened strictly after b (a > b)
    After,
    /// a and b are identical (a == b)
    Equal,
    /// a and b are concurrent (neither before nor after)
    Concurrent,
}

/// Compare two vector clocks.
/// This is the core operation used by the CRDT merge to detect conflicts.
pub fn compare(a: &VectorClock, b: &VectorClock) -> ClockOrdering {
    let mut a_less = false;
    let mut a_greater = false;

    let all_keys: std::collections::HashSet<&String> = a.keys().chain(b.keys()).collect();

    for k in all_keys {
        let av = a.get(k).copied().unwrap_or(0);
        let bv = b.get(k).copied().unwrap_or(0);

        if av < bv {
            a_less = true;
        } else if av > bv {
            a_greater = true;
        }

        // Short-circuit: if we've already seen both directions, it's concurrent
        if a_less && a_greater {
            return ClockOrdering::Concurrent;
        }
    }

    match (a_less, a_greater) {
        (false, false) => ClockOrdering::Equal,
        (true, false) => ClockOrdering::Before,
        (false, true) => ClockOrdering::After,
        (true, true) => ClockOrdering::Concurrent,
    }
}
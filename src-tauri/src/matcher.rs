// Yonodex Desktop Client - Matching Engine
// Whitepaper Layer 5, Section 7.3: Matching Engine
//
// Deterministic price-time priority matching.
//   - Buy orders sorted DESC by price, then ASC by id (earlier first)
//   - Sell orders sorted ASC by price, then ASC by id
//   - Match pairs where buy_price >= sell_price
//   - Execution price = resting order's price (the one with earlier id)
//   - Amount = min(buy_remaining, sell_remaining)
//
// Canonicalization (critical for cross-node agreement):
//   The Trade struct's id and created_at are derived from the two order
//   ids — never from wall-clock time. Both nodes produce BYTE-IDENTICAL
//   Trade structs given the same crossing pair, so both signatures are
//   over the same payload and validate against each other.
//
// Race resolution note:
//   The whitepaper prescribes a VDF for race resolution. A VDF enforces
//   a uniform delay but does not change the relative order of proposals —
//   it addresses a different problem than it claims. Instead we rely on
//   canonical Trade ids: if two nodes propose the same match, they produce
//   the same signed bytes and the tie is moot. If two DISTINCT matches
//   are proposed (different pairs), the lexicographically smaller trade_id
//   wins deterministically on every node. Front-running mitigation is
//   deferred to Phase 4 (zk-SNARK private orders).

use crate::order::Side;
use crate::orderbook::OrderBook;
use crate::trade::Trade;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Scan the book for crossing orders and produce trade proposals.
/// Mutates the book: records fills and tombstones fully filled orders.
///
/// Returns trades in the order they were matched (price-then-time).
pub fn find_matches(book: &mut OrderBook) -> Result<Vec<Trade>, String> {
    let mut trades: Vec<Trade> = Vec::new();

    // Snapshot of fillable orders (remaining > 0)
    let fillable: Vec<_> = book.fillable_orders().into_iter().cloned().collect();

    // Split by side
    let mut buys: Vec<_> = fillable
        .iter()
        .filter(|o| o.side == Side::Buy)
        .cloned()
        .collect();
    let mut sells: Vec<_> = fillable
        .iter()
        .filter(|o| o.side == Side::Sell)
        .cloned()
        .collect();

    // Sort buys: price DESC, then id ASC (time priority)
    buys.sort_by(|a, b| {
        let pa = Decimal::from_str(&a.price).unwrap_or(Decimal::ZERO);
        let pb = Decimal::from_str(&b.price).unwrap_or(Decimal::ZERO);
        pb.cmp(&pa).then_with(|| a.id.cmp(&b.id))
    });

    // Sort sells: price ASC, then id ASC (time priority)
    sells.sort_by(|a, b| {
        let pa = Decimal::from_str(&a.price).unwrap_or(Decimal::ZERO);
        let pb = Decimal::from_str(&b.price).unwrap_or(Decimal::ZERO);
        pa.cmp(&pb).then_with(|| a.id.cmp(&b.id))
    });

    // Do NOT use the local book clock here — it differs between peers.
    // Each trade's clock is derived from the two orders being matched,
    // so all peers compute identical bytes and signatures validate.

    let mut bi = 0;
    let mut si = 0;

    while bi < buys.len() && si < sells.len() {
        let buy = &buys[bi];
        let sell = &sells[si];

        let buy_price = Decimal::from_str(&buy.price).unwrap_or(Decimal::ZERO);
        let sell_price = Decimal::from_str(&sell.price).unwrap_or(Decimal::ZERO);

        // No cross possible — best bid below best ask
        if buy_price < sell_price {
            break;
        }

        let buy_rem = book.remaining(&buy.id);
        let sell_rem = book.remaining(&sell.id);

        if buy_rem <= Decimal::ZERO {
            bi += 1;
            continue;
        }
        if sell_rem <= Decimal::ZERO {
            si += 1;
            continue;
        }

        let trade_amount = buy_rem.min(sell_rem);

        // Execution price = resting order's price (earlier id = resting)
        let exec_price = if buy.id < sell.id { buy_price } else { sell_price };

        // ---- Canonical trade id + timestamp (deterministic across nodes) ----
        // Both nodes have both order ids. Lexicographic ordering is stable.
        let (earlier_id, later_id) = if buy.id < sell.id {
            (&buy.id, &sell.id)
        } else {
            (&sell.id, &buy.id)
        };

        let trade_id = format!("trade-{}-{}-{}", earlier_id, later_id, trade_amount);

        // Extract timestamp from the earlier order's id prefix.
        // Order ids are formatted as "{unix_millis}-{owner_prefix}".
        let created_at = extract_timestamp_from_order_id(earlier_id);

        let trade = Trade {
            id: trade_id,
            buy_order_id: buy.id.clone(),
            sell_order_id: sell.id.clone(),
            buyer_owner: buy.owner.clone(),
            seller_owner: sell.owner.clone(),
            price: exec_price.to_string(),
            amount: trade_amount.to_string(),
            pair: buy.pair.clone(),
            buyer_signature: String::new(),
            seller_signature: String::new(),
            vector_clock: crate::clock::merge(&buy.vector_clock, &sell.vector_clock),
            created_at,
        };

        // Record fills before tombstoning
        book.record_fill(&buy.id, trade_amount)?;
        book.record_fill(&sell.id, trade_amount)?;

        // Tombstone fully filled orders
        if book.remaining(&buy.id) <= Decimal::ZERO {
            book.mark_filled(&buy.id)?;
        }
        if book.remaining(&sell.id) <= Decimal::ZERO {
            book.mark_filled(&sell.id)?;
        }

        trades.push(trade);
    }

    Ok(trades)
}

/// Parse the millisecond timestamp prefix from an order id.
/// Order ids are `{unix_millis}-{owner_prefix}`. Returns unix seconds.
/// Returns 0 if the prefix can't be parsed — safe fallback; both nodes
/// produce the same 0 for the same malformed id, so canonicalization holds.
fn extract_timestamp_from_order_id(order_id: &str) -> i64 {
    order_id
        .split('-')
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|millis| millis / 1000)
        .unwrap_or(0)
}

// ---- Unit tests ----
// Prove the matcher is correct before we do full two-node integration.
// These run with `cargo test`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{Order, Side};
    use crate::orderbook::OrderBook;
    use std::collections::HashMap;

    /// Build a minimal unsigned order for testing.
    /// Signature verification is bypassed via `insert_for_test`.
    fn order(
        id: &str,
        owner: &str,
        side: Side,
        price: &str,
        amount: &str,
    ) -> Order {
        Order {
            id: id.to_string(),
            owner: owner.to_string(),
            side,
            pair: "TKA/TKB".to_string(),
            price: price.to_string(),
            amount: amount.to_string(),
            expiry: 9_999_999_999,
            nonce: 1,
            signature: String::new(),
            vector_clock: HashMap::new(),
        }
    }

    #[test]
    fn matches_simple_cross() {
        let mut book = OrderBook::new("local".to_string());
        book.insert_for_test(order("a-buy", "alice", Side::Buy, "100", "5"));
        book.insert_for_test(order("b-sell", "bob", Side::Sell, "90", "5"));

        let trades = find_matches(&mut book).expect("matcher ok");
        assert_eq!(trades.len(), 1, "one trade expected");

        let t = &trades[0];
        assert_eq!(t.amount, "5");
        assert_eq!(t.price, "100");
        assert_eq!(t.buyer_owner, "alice");
        assert_eq!(t.seller_owner, "bob");

        assert_eq!(book.remaining("a-buy"), Decimal::ZERO);
        assert_eq!(book.remaining("b-sell"), Decimal::ZERO);
    }

    #[test]
    fn no_match_when_prices_dont_cross() {
        let mut book = OrderBook::new("local".to_string());
        book.insert_for_test(order("a-buy", "alice", Side::Buy, "90", "5"));
        book.insert_for_test(order("b-sell", "bob", Side::Sell, "100", "5"));

        let trades = find_matches(&mut book).expect("matcher ok");
        assert_eq!(trades.len(), 0, "no trade — bid below ask");
    }

    #[test]
    fn price_time_priority_best_price_wins() {
        let mut book = OrderBook::new("local".to_string());
        book.insert_for_test(order("s1-95", "s1", Side::Sell, "95", "5"));
        book.insert_for_test(order("s2-90", "s2", Side::Sell, "90", "5"));
        book.insert_for_test(order("buyer", "alice", Side::Buy, "100", "5"));

        let trades = find_matches(&mut book).expect("matcher ok");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].sell_order_id, "s2-90");
    }

    #[test]
    fn partial_fill_leaves_remainder() {
        let mut book = OrderBook::new("local".to_string());
        book.insert_for_test(order("buy", "alice", Side::Buy, "100", "5"));
        book.insert_for_test(order("sell", "bob", Side::Sell, "100", "3"));

        let trades = find_matches(&mut book).expect("matcher ok");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].amount, "3");

        assert_eq!(book.remaining("buy").to_string(), "2");
        assert_eq!(book.remaining("sell"), Decimal::ZERO);
    }

    #[test]
    fn large_aggressor_consumes_multiple_levels() {
        let mut book = OrderBook::new("local".to_string());
        book.insert_for_test(order("s-90", "s1", Side::Sell, "90", "2"));
        book.insert_for_test(order("s-95", "s2", Side::Sell, "95", "2"));
        book.insert_for_test(order("s-100", "s3", Side::Sell, "100", "2"));
        book.insert_for_test(order("big-buy", "alice", Side::Buy, "100", "5"));

        let trades = find_matches(&mut book).expect("matcher ok");
        assert_eq!(trades.len(), 3, "three levels consumed");

        assert_eq!(trades[0].amount, "2");
        assert_eq!(trades[1].amount, "2");
        assert_eq!(trades[2].amount, "1");

        assert_eq!(book.remaining("big-buy"), Decimal::ZERO);
        assert_eq!(book.remaining("s-100").to_string(), "1");
    }

    #[test]
    fn deterministic_across_runs() {
        let build = || {
            let mut b = OrderBook::new("local".to_string());
            b.insert_for_test(order("b1", "a", Side::Buy, "105", "3"));
            b.insert_for_test(order("b2", "b", Side::Buy, "100", "4"));
            b.insert_for_test(order("s1", "c", Side::Sell, "98", "5"));
            b.insert_for_test(order("s2", "d", Side::Sell, "102", "3"));
            b
        };

        let mut book_a = build();
        let mut book_b = build();

        let trades_a = find_matches(&mut book_a).expect("ok");
        let trades_b = find_matches(&mut book_b).expect("ok");

        assert_eq!(trades_a.len(), trades_b.len());
        for (a, b) in trades_a.iter().zip(trades_b.iter()) {
            assert_eq!(a.id, b.id, "trade ids must match");
            assert_eq!(a.amount, b.amount, "amounts must match");
            assert_eq!(a.price, b.price, "prices must match");
            assert_eq!(a.buy_order_id, b.buy_order_id);
            assert_eq!(a.sell_order_id, b.sell_order_id);
        }
    }

    #[test]
    fn canonical_trade_id_is_stable_regardless_of_discovery_order() {
        let mut book1 = OrderBook::new("local".to_string());
        book1.insert_for_test(order("aaa-buy", "alice", Side::Buy, "100", "5"));
        book1.insert_for_test(order("bbb-sell", "bob", Side::Sell, "100", "5"));

        let mut book2 = OrderBook::new("local".to_string());
        book2.insert_for_test(order("bbb-sell", "bob", Side::Sell, "100", "5"));
        book2.insert_for_test(order("aaa-buy", "alice", Side::Buy, "100", "5"));

        let trades1 = find_matches(&mut book1).expect("ok");
        let trades2 = find_matches(&mut book2).expect("ok");

        assert_eq!(trades1.len(), 1);
        assert_eq!(trades2.len(), 1);
        assert_eq!(trades1[0].id, trades2[0].id, "canonical trade id must match");
    }
}

// ---- Integration test: two peers, two signing keys, one shared trade ----
// Proves that two independent nodes with different keys produce byte-identical
// Trade structs and cross-verify each other's signatures. This is the actual
// 2.B sub-task 9 goal — Tor/IPC were already proven in 2.A.
#[cfg(test)]
mod integration {
    use super::*;
    use crate::order::{Order, Side};
    use crate::orderbook::OrderBook;
    use crate::trade;
    use ed25519_dalek::SigningKey;
    use std::collections::HashMap;

    fn make_key(seed: u8) -> SigningKey {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        SigningKey::from_bytes(&bytes)
    }

    fn owner_of(key: &SigningKey) -> String {
        hex::encode(key.verifying_key().to_bytes())
    }

    fn signed_order(
        key: &SigningKey,
        id: &str,
        side: Side,
        price: &str,
        amount: &str,
        clock: HashMap<String, u64>,
    ) -> Order {
        let owner = owner_of(key);
        let mut order = Order {
            id: id.to_string(),
            owner,
            side,
            pair: "TKA/TKB".to_string(),
            price: price.to_string(),
            amount: amount.to_string(),
            expiry: 9_999_999_999,
            nonce: 1,
            signature: String::new(),
            vector_clock: clock,
        };
        order.signature = crate::order::sign_order(&order, key);
        order
    }

    #[test]
    fn two_peers_produce_identical_signed_trade() {
        // ---- Peer A and Peer B, different keys ----
        let key_a = make_key(1);
        let key_b = make_key(2);
        let owner_a = owner_of(&key_a);
        let owner_b = owner_of(&key_b);

        // ---- A creates a BUY order ----
        let mut clock_a = HashMap::new();
        clock_a.insert(owner_a.clone(), 1);
        let buy = signed_order(&key_a, "1000-aaaa", Side::Buy, "100", "5", clock_a.clone());

        // ---- B creates a SELL order that crosses A's buy ----
        let mut clock_b = HashMap::new();
        clock_b.insert(owner_b.clone(), 1);
        let sell = signed_order(&key_b, "2000-bbbb", Side::Sell, "95", "5", clock_b.clone());

        // ---- Both peers build the same book state (different insertion order) ----
        let mut book_a = OrderBook::new(owner_a.clone());
        let mut book_b = OrderBook::new(owner_b.clone());

        book_a.insert_for_test(buy.clone());
        book_a.insert_for_test(sell.clone());
        book_b.insert_for_test(sell.clone());
        book_b.insert_for_test(buy.clone());

        // ---- Both peers run the matcher ----
        let trades_a = find_matches(&mut book_a).expect("A matcher ok");
        let trades_b = find_matches(&mut book_b).expect("B matcher ok");

        assert_eq!(trades_a.len(), 1);
        assert_eq!(trades_b.len(), 1);

        let ta = &trades_a[0];
        let tb = &trades_b[0];

        // ---- Same trade id, same price, same amount, same signable bytes ----
        assert_eq!(ta.id, tb.id, "trade ids must match");
        assert_eq!(ta.price, tb.price);
        assert_eq!(ta.amount, tb.amount);
        assert_eq!(ta.buy_order_id, tb.buy_order_id);
        assert_eq!(ta.sell_order_id, tb.sell_order_id);
        assert_eq!(
            ta.signable_bytes(),
            tb.signable_bytes(),
            "signable bytes must be byte-identical"
        );

        // ---- Each peer signs their own side ----
        let signed_by_a = trade::sign_as(ta, &key_a, &owner_a).expect("A signs");
        let signed_by_b = trade::sign_as(tb, &key_b, &owner_b).expect("B signs");

        assert!(!signed_by_a.buyer_signature.is_empty());
        assert!(!signed_by_b.seller_signature.is_empty());

        // ---- Merge: A takes B's seller sig, B takes A's buyer sig ----
        let mut merged_a = signed_by_a.clone();
        merged_a.seller_signature = signed_by_b.seller_signature.clone();
        let mut merged_b = signed_by_b.clone();
        merged_b.buyer_signature = signed_by_a.buyer_signature.clone();

        // ---- Both peers verify both signatures on the merged trade ----
        assert!(
            trade::verify_buyer_signature(&merged_a).is_ok(),
            "A verifies buyer sig"
        );
        assert!(
            trade::verify_seller_signature(&merged_a).is_ok(),
            "A verifies seller sig"
        );
        assert!(
            trade::verify_buyer_signature(&merged_b).is_ok(),
            "B verifies buyer sig"
        );
        assert!(
            trade::verify_seller_signature(&merged_b).is_ok(),
            "B verifies seller sig"
        );

        // ---- Fully signed ----
        assert!(trade::is_fully_signed(&merged_a));
        assert!(trade::is_fully_signed(&merged_b));

        // ---- Both orders tombstoned as Filled on both peers ----
        assert!(book_a.get(&buy.id).unwrap().tombstone.is_some());
        assert!(book_a.get(&sell.id).unwrap().tombstone.is_some());
        assert!(book_b.get(&buy.id).unwrap().tombstone.is_some());
        assert!(book_b.get(&sell.id).unwrap().tombstone.is_some());
    }
}
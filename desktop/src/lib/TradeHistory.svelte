<script lang="ts">
  import { onMount } from "svelte";
  import { loadTrades, saveTrade, deleteTrade, type TradeRecord } from "../db";
  import { CHAINS, DEFAULT_CHAIN, CONTRACTS } from "../config";

  let { address }: { address: string } = $props();

  let trades = $state<TradeRecord[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const chain = CHAINS[DEFAULT_CHAIN];
  const contracts = CONTRACTS[DEFAULT_CHAIN];

  function formatTimestamp(ts: number): string {
    const d = new Date(ts * 1000);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function shortenHash(hash: string | null): string {
    if (!hash) return "—";
    return hash.slice(0, 6) + "..." + hash.slice(-4);
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      trades = await loadTrades(address, 100);
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to load trade history";
    } finally {
      loading = false;
    }
  }

  async function handleDelete(id: number) {
    try {
      await deleteTrade(id);
      trades = trades.filter((t) => t.id !== id);
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to delete trade";
    }
  }

  async function addTestTrade() {
    const isBuy = Math.random() > 0.5;
    const amount = (0.1 + Math.random() * 1.5).toFixed(4);
    const out = (amount * 1850).toFixed(2);

    try {
      await saveTrade({
        wallet_address: address,
        tx_hash: null,
        chain_id: chain.chainIdHex,
        pair: "TKA/TKB",
        side: isBuy ? "buy" : "sell",
        amount_in: isBuy ? out : amount,
        amount_out: isBuy ? amount : out,
        token_in: isBuy ? contracts.tokenB : contracts.tokenA,
        token_out: isBuy ? contracts.tokenA : contracts.tokenB,
        status: "confirmed",
        notes: "test trade (dev)",
      });
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to save test trade";
    }
  }

  $effect(() => {
    if (address) {
      refresh();
    }
  });
</script>

<div class="trade-history">
  <div class="header-row">
    <button class="action-btn" onclick={addTestTrade}>+ Test Trade</button>
    <button class="action-btn secondary" onclick={refresh} disabled={loading}>
      {loading ? "..." : "Refresh"}
    </button>
  </div>

  {#if loading && trades.length === 0}
    <p class="hint">Loading trade history...</p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if trades.length === 0}
    <p class="hint">
      No trades yet for this wallet.
      <br />
      <span class="small">Your trades will appear here once trading is live.</span>
    </p>
  {:else}
    <div class="list">
      {#each trades as trade (trade.id)}
        <div class="row">
          <div class="row-top">
            <span class="pair">{trade.pair}</span>
            <span class="side {trade.side}">{trade.side.toUpperCase()}</span>
            <span class="time">{formatTimestamp(trade.timestamp)}</span>
          </div>
          <div class="row-bottom">
            <span class="amount">
              {trade.amount_in} → {trade.amount_out}
            </span>
            <span class="hash">{shortenHash(trade.tx_hash)}</span>
            <button
              class="del-btn"
              onclick={() => handleDelete(trade.id)}
              title="Delete"
            >
              ×
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .trade-history {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-width: 320px;
    width: 100%;
  }

  .header-row {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
  }

  .action-btn {
    background: transparent;
    color: #a78bfa;
    border: 1px solid #a78bfa;
    border-radius: 6px;
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .action-btn.secondary {
    color: #8b8b94;
    border-color: #4a4a55;
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    max-height: 320px;
    overflow-y: auto;
  }

  .row {
    background: #131318;
    border: 1px solid #26262e;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .row-top {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .pair {
    color: #e8e8ec;
    font-weight: 600;
    font-size: 0.85rem;
  }

  .side {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
  }

  .side.buy {
    color: #22c55e;
    background: rgba(34, 197, 94, 0.12);
  }

  .side.sell {
    color: #f87171;
    background: rgba(248, 113, 113, 0.12);
  }

  .time {
    margin-left: auto;
    color: #6b6b74;
    font-size: 0.7rem;
  }

  .row-bottom {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-family: monospace;
    font-size: 0.75rem;
    color: #b8b8c0;
  }

  .amount {
    flex: 1;
  }

  .hash {
    color: #6b6b74;
  }

  .del-btn {
    background: transparent;
    color: #6b6b74;
    border: none;
    font-size: 1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.25rem;
  }

  .del-btn:hover {
    color: #f87171;
  }

  .hint {
    color: #8b8b94;
    font-size: 0.9rem;
    text-align: center;
    line-height: 1.5;
  }

  .hint .small {
    font-size: 0.75rem;
    color: #6b6b74;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    text-align: center;
  }
</style>
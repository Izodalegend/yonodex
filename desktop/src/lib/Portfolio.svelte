<script lang="ts">
  import { loadPortfolio, type PortfolioSnapshot } from "../portfolio";

  let { address }: { address: string } = $props();

  let snapshot = $state<PortfolioSnapshot | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  function formatBalance(v: string): string {
    const n = Number(v);
    if (!isFinite(n)) return v;
    if (n === 0) return "0";
    if (n < 0.0001) return "<0.0001";
    return n.toLocaleString(undefined, { maximumFractionDigits: 4 });
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      snapshot = await loadPortfolio(address);
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to load portfolio";
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (address) {
      refresh();
    }
  });
</script>

<div class="portfolio">
  {#if loading}
    <p class="hint">Loading balances...</p>
  {:else if error}
    <p class="error">{error}</p>
    <button class="refresh" onclick={refresh}>Retry</button>
  {:else if snapshot}
    <div class="chain-tag">{snapshot.chainName}</div>

    <div class="row native">
      <span class="label">{snapshot.native.symbol}</span>
      <span class="balance">{formatBalance(snapshot.native.balance)}</span>
    </div>

    {#each snapshot.tokens as token (token.address)}
      <div class="row">
        <span class="label">{token.symbol}</span>
        <span class="balance">{formatBalance(token.balance)}</span>
      </div>
    {/each}

    {#if snapshot.lp}
      <div class="row lp">
        <span class="label">LP</span>
        <span class="balance">
          {formatBalance(snapshot.lp.shares)}
          <span class="share">{snapshot.lp.sharePercent}%</span>
        </span>
      </div>
    {/if}

    <button class="refresh" onclick={refresh} disabled={loading}>Refresh</button>
  {/if}
</div>

<style>
  .portfolio {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-width: 260px;
    width: 100%;
  }

  .chain-tag {
    font-size: 0.75rem;
    color: #a78bfa;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    text-align: center;
    margin-bottom: 0.5rem;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.6rem 0.9rem;
    background: #131318;
    border: 1px solid #26262e;
    border-radius: 8px;
  }

  .row.native {
    border-color: #3a3a48;
  }

  .row.lp {
    border-color: #7c3aed;
  }

  .label {
    color: #b8b8c0;
    font-weight: 600;
    font-size: 0.9rem;
  }

  .balance {
    font-family: monospace;
    color: #e8e8ec;
    font-size: 0.95rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .share {
    color: #a78bfa;
    font-size: 0.8rem;
    font-weight: 600;
  }

  .hint {
    color: #8b8b94;
    font-size: 0.9rem;
    text-align: center;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    text-align: center;
  }

  .refresh {
    background: transparent;
    color: #a78bfa;
    border: 1px solid #a78bfa;
    border-radius: 8px;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 0.85rem;
    margin-top: 0.5rem;
  }

  .refresh:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
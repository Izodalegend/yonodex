<script lang="ts">
  import { onMount } from "svelte";
  import WalletConnect from "./lib/WalletConnect.svelte";
  import Portfolio from "./lib/Portfolio.svelte";
  import TradeHistory from "./lib/TradeHistory.svelte";
  import Unlock from "./lib/Unlock.svelte";
  import { dbIsUnlocked } from "./db";
  import type { EIP1193Provider } from "./wallet";

  const version = "0.1.0";

  let unlocked = $state(false);
  let checkingUnlock = $state(true);
  let wallet = $state<{ address: string; provider: EIP1193Provider } | null>(null);

  onMount(async () => {
    try {
      unlocked = await dbIsUnlocked();
    } catch {
      unlocked = false;
    } finally {
      checkingUnlock = false;
    }
  });

  function handleUnlock() {
    unlocked = true;
  }

  function handleConnect(data: { address: string; provider: EIP1193Provider }) {
    wallet = data;
  }

  function handleDisconnect() {
    wallet = null;
  }
</script>

<main>
  <header>
    <h1>Yonodex</h1>
    <p class="tagline">Decentralized Exchange</p>
  </header>

  {#if checkingUnlock}
    <div class="card">
      <p class="muted">Loading...</p>
    </div>
  {:else if !unlocked}
    <div class="card">
      <Unlock onunlock={handleUnlock} />
    </div>
  {:else}
    <section class="status">
      <div class="card">
        <h2>Wallet</h2>
        <WalletConnect onconnect={handleConnect} ondisconnect={handleDisconnect} />
      </div>

      <div class="card">
        <h2>Portfolio</h2>
        {#if wallet}
          <Portfolio address={wallet.address} />
        {:else}
          <p class="muted">Connect a wallet to view balances</p>
        {/if}
      </div>
    </section>

    <section class="full-width">
      <div class="card wide">
        <h2>Trade History</h2>
        <TradeHistory />
      </div>
    </section>
  {/if}

  <footer>
    <p>v{version} - AGPL-3.0 - Yonodex Team</p>
  </footer>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #0f0f12;
    color: #e8e8ec;
  }

  main {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-start;
    padding: 2rem;
    gap: 2rem;
  }

  header {
    text-align: center;
    margin-top: 1rem;
  }

  h1 {
    font-size: 3rem;
    margin: 0;
    line-height: 1.4;
    padding: 0.2em 0;
    background: linear-gradient(135deg, #a78bfa, #7c3aed);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .tagline {
    color: #8b8b94;
    margin-top: 0.5rem;
    font-size: 1.1rem;
  }

  .status {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
    justify-content: center;
  }

  .full-width {
    display: flex;
    justify-content: center;
    width: 100%;
  }

  .card {
    background: #1a1a1f;
    border: 1px solid #2a2a32;
    border-radius: 12px;
    padding: 2rem;
    min-width: 280px;
    text-align: center;
  }

  .card.wide {
    max-width: 720px;
    width: 100%;
  }

  .card h2 {
    margin: 0 0 1.5rem 0;
    color: #e8e8ec;
    font-size: 1.1rem;
  }

  .card p {
    margin: 0.5rem 0;
    color: #b8b8c0;
  }

  .muted {
    color: #6b6b74 !important;
    font-size: 0.9rem;
  }

  footer {
    color: #6b6b74;
    font-size: 0.85rem;
    margin-bottom: 1rem;
  }
</style>
<script lang="ts">
  import { onMount } from "svelte";
  import {
    discoverWallets,
    detectFrame,
    switchChain,
    isOnChain,
    type DetectedWallet,
    type EIP1193Provider,
  } from "../wallet";
  import { saveWalletConfig, loadWalletConfig, clearWalletConfig } from "../db";
  import { CHAINS, DEFAULT_CHAIN } from "../config";

  let {
    onconnect,
    ondisconnect,
  }: {
    onconnect?: (data: { address: string; provider: EIP1193Provider }) => void;
    ondisconnect?: () => void;
  } = $props();

  let wallets = $state<DetectedWallet[]>([]);
  let scanning = $state(true);
  let connecting = $state(false);
  let error = $state<string | null>(null);
  let wrongChain = $state(false);
  let switching = $state(false);
  let connected = $state<{
    address: string;
    chainId: string;
    wallet: DetectedWallet;
  } | null>(null);

  const expectedChain = CHAINS[DEFAULT_CHAIN];

  async function scan() {
    scanning = true;
    error = null;
    try {
      const eip6963 = await discoverWallets();
      const frame = await detectFrame();
      wallets = frame ? [...eip6963, frame] : eip6963;
    } finally {
      scanning = false;
    }
  }

  async function checkChain(wallet: DetectedWallet) {
    const ok = await isOnChain(wallet.provider, expectedChain.chainIdHex);
    wrongChain = !ok;
  }

  async function restore() {
    let saved;
    try {
      saved = await loadWalletConfig();
    } catch {
      return;
    }
    if (!saved) return;

    const frame = await detectFrame();
    if (!frame) return;

    try {
      const accounts = (await frame.provider.request({
        method: "eth_accounts",
      })) as string[];

      if (
        accounts &&
        accounts.length > 0 &&
        accounts[0].toLowerCase() === saved.address.toLowerCase()
      ) {
        const chainId = (await frame.provider.request({
          method: "eth_chainId",
        })) as string;
        connected = { address: accounts[0], chainId, wallet: frame };
        await checkChain(frame);
        onconnect?.({ address: accounts[0], provider: frame.provider });
      }
    } catch {
      // Silent restore failed - user will just connect manually
    }
  }

  onMount(async () => {
    await scan();
    await restore();
  });

  async function connect(wallet: DetectedWallet) {
    connecting = true;
    error = null;
    try {
      const accounts = (await wallet.provider.request({
        method: "eth_requestAccounts",
      })) as string[];
      if (!accounts || accounts.length === 0) {
        throw new Error("No accounts returned");
      }
      const chainId = (await wallet.provider.request({
        method: "eth_chainId",
      })) as string;
      connected = { address: accounts[0], chainId, wallet };

      try {
        await saveWalletConfig({
          address: accounts[0],
          chain_id: chainId,
          wallet_uuid: wallet.info.uuid,
          wallet_name: wallet.info.name,
        });
      } catch (e) {
        // Non-fatal - connection still works this session
        console.error("Failed to persist wallet config:", e);
      }

      await checkChain(wallet);
      onconnect?.({ address: accounts[0], provider: wallet.provider });
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to connect";
    } finally {
      connecting = false;
    }
  }

  async function handleSwitchChain() {
    if (!connected) return;
    switching = true;
    error = null;
    try {
      await switchChain(connected.wallet.provider, expectedChain.chainIdHex);
      await checkChain(connected.wallet);
      if (!wrongChain) {
        const chainId = (await connected.wallet.provider.request({
          method: "eth_chainId",
        })) as string;
        connected = { ...connected, chainId };
      }
    } catch (e) {
      error =
        e instanceof Error
          ? e.message
          : "Failed to switch chain. Please switch manually in Frame.";
    } finally {
      switching = false;
    }
  }

  async function handleDisconnect() {
    try {
      await clearWalletConfig();
    } catch (e) {
      console.error("Failed to clear wallet config:", e);
    }
    connected = null;
    wrongChain = false;
    error = null;
    ondisconnect?.();
  }

  function shorten(addr: string) {
    return addr.slice(0, 6) + "..." + addr.slice(-4);
  }
</script>

<div class="wallet-connect">
  {#if connected}
    {#if wrongChain}
      <div class="warning">
        Wrong network. Please switch to {expectedChain.shortName}.
      </div>
      <button class="switch-btn" onclick={handleSwitchChain} disabled={switching}>
        {switching ? "Switching..." : "Switch to " + expectedChain.shortName}
      </button>
    {/if}

    <div class="connected-card">
      <span class="dot"></span>
      <span class="address">{shorten(connected.address)}</span>
      <span class="wallet-name">via {connected.wallet.info.name}</span>
    </div>

    <button class="disconnect-btn" onclick={handleDisconnect}>Disconnect</button>
  {:else if scanning}
    <div class="hint">Scanning for wallets...</div>
  {:else if wallets.length === 0}
    <div class="hint">
      No wallet detected.
      <br />
      <span class="small">Install Frame or a browser wallet, then retry.</span>
    </div>
    <button class="retry" onclick={scan}>Retry</button>
  {:else}
    <div class="wallet-list">
      {#each wallets as wallet (wallet.info.uuid)}
        <button
          class="wallet-btn"
          onclick={() => connect(wallet)}
          disabled={connecting}
        >
          {connecting ? "Connecting..." : "Connect " + wallet.info.name}
        </button>
      {/each}
    </div>
  {/if}

  {#if error}
    <div class="error">{error}</div>
  {/if}
</div>

<style>
  .wallet-connect {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    min-width: 260px;
  }

  .hint {
    color: #8b8b94;
    font-size: 0.95rem;
    text-align: center;
    line-height: 1.5;
  }

  .hint .small {
    font-size: 0.8rem;
    color: #6b6b74;
  }

  .wallet-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
  }

  .wallet-btn {
    background: linear-gradient(135deg, #a78bfa, #7c3aed);
    color: white;
    border: none;
    border-radius: 8px;
    padding: 0.75rem 1.5rem;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: opacity 0.2s;
  }

  .wallet-btn:hover:not(:disabled) {
    opacity: 0.9;
  }

  .wallet-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .retry {
    background: transparent;
    color: #a78bfa;
    border: 1px solid #a78bfa;
    border-radius: 8px;
    padding: 0.5rem 1rem;
    cursor: pointer;
  }

  .connected-card {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: #1a1a1f;
    border: 1px solid #2a2a32;
    border-radius: 8px;
    padding: 0.75rem 1rem;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #22c55e;
  }

  .address {
    font-family: monospace;
    color: #e8e8ec;
  }

  .wallet-name {
    color: #6b6b74;
    font-size: 0.85rem;
  }

  .disconnect-btn {
    background: transparent;
    color: #6b6b74;
    border: none;
    font-size: 0.8rem;
    cursor: pointer;
    text-decoration: underline;
    padding: 0;
  }

  .disconnect-btn:hover {
    color: #b8b8c0;
  }

  .switch-btn {
    background: #f59e0b;
    color: #0f0f12;
    border: none;
    border-radius: 8px;
    padding: 0.5rem 1rem;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    width: 100%;
  }

  .switch-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .warning {
    color: #f59e0b;
    font-size: 0.8rem;
    text-align: center;
    line-height: 1.4;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    text-align: center;
  }
</style>
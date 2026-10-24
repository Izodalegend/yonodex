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
  let showDropdown = $state(false);
  
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
      if (wallets.length === 1) {
        // Auto-connect if only one wallet is found
        connect(wallets[0]);
      }
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
    
    // Close dropdown on click outside
    const close = () => (showDropdown = false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  });

  async function connect(wallet: DetectedWallet) {
    connecting = true;
    error = null;
    showDropdown = false;
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

<div class="relative flex items-center gap-3">
  {#if connected}
    {#if wrongChain}
      <button 
        class="bg-yellow-500 hover:bg-yellow-600 text-black text-xs font-bold py-1.5 px-3 rounded-md transition-colors"
        onclick={handleSwitchChain} 
        disabled={switching}
      >
        {switching ? "Switching..." : "Switch to " + expectedChain.shortName}
      </button>
    {/if}

    <div class="flex items-center gap-2 bg-[#1a1a1f] border border-[#2a2a32] rounded-full px-3 py-1.5">
      <span class="w-2 h-2 rounded-full bg-green-500 shadow-[0_0_5px_rgba(34,197,94,0.5)]"></span>
      <span class="text-sm font-mono text-white">{shorten(connected.address)}</span>
      <span class="text-xs text-gray-500 hidden md:inline">via {connected.wallet.info.name}</span>
    </div>

    <button 
      class="text-xs text-gray-500 hover:text-red-400 transition-colors" 
      onclick={handleDisconnect}
    >
      Disconnect
    </button>

  {:else if scanning}
    <div class="text-xs text-gray-500 animate-pulse">Scanning for wallets...</div>
    
  {:else if wallets.length === 0}
    <div class="flex items-center gap-3">
      <span class="text-xs text-gray-500">No wallet detected</span>
      <button 
        class="bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium py-1.5 px-4 rounded-lg transition-colors"
        onclick={scan}
      >
        Retry
      </button>
    </div>
    
  {:else}
    <button 
      class="bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium py-1.5 px-4 rounded-lg transition-colors shadow-lg shadow-purple-900/20"
      onclick={(e) => { e.stopPropagation(); showDropdown = !showDropdown; }}
      disabled={connecting}
    >
      {connecting ? "Connecting..." : "Connect Wallet"}
    </button>

    {#if showDropdown}
      <div class="absolute right-0 top-full mt-2 w-48 bg-[#1a1a1f] border border-[#2a2a32] rounded-lg shadow-2xl overflow-hidden z-50 flex flex-col">
        {#each wallets as wallet (wallet.info.uuid)}
          <button
            class="w-full text-left px-4 py-2.5 text-sm text-gray-300 hover:bg-[#2a2a32] hover:text-white transition-colors"
            onclick={() => connect(wallet)}
          >
            Connect {wallet.info.name}
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if error}
    <div class="absolute right-0 top-full mt-2 bg-red-900/90 border border-red-700 text-red-200 text-xs px-3 py-2 rounded-md z-50 whitespace-nowrap">
      {error}
    </div>
  {/if}
</div>
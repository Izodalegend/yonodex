<script lang="ts">
  import { onMount } from "svelte";
  import WalletConnect from "./lib/WalletConnect.svelte";
  import Portfolio from "./lib/Portfolio.svelte";
  import TradeHistory from "./lib/TradeHistory.svelte";
  import TorStatus from "./lib/TorStatus.svelte";
  import Unlock from "./lib/Unlock.svelte";
  import { dbIsUnlocked } from "./db";
  import type { EIP1193Provider } from "./wallet";

  const version = "0.1.2";
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  let unlocked = $state(false);
  let checkingUnlock = $state(true);
  let wallet = $state<{ address: string; provider: EIP1193Provider } | null>(null);

  onMount(async () => {
    if (!isTauri) {
      checkingUnlock = false;
      return;
    }
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

<div class="h-screen w-screen bg-[#0f0f12] text-gray-200 font-sans flex flex-col overflow-hidden selection:bg-purple-900 selection:text-white">
  
  {#if !isTauri}
    <!-- WEB LANDING PAGE (IPFS / Tor) -->
    <div class="flex-1 flex flex-col items-center justify-center p-4">
      <div class="text-center mb-8">
        <h1 class="text-5xl font-bold tracking-tight mb-2 bg-gradient-to-r from-purple-400 to-purple-600 bg-clip-text text-transparent">
          Yonodex
        </h1>
        <p class="text-gray-400 text-lg">Decentralized Exchange</p>
      </div>

      <div class="bg-[#1a1a1f] border border-[#2a2a32] rounded-2xl p-8 max-w-2xl w-full shadow-2xl text-center">
        <h2 class="text-2xl font-semibold mb-2">Desktop Client Required</h2>
        <p class="text-gray-400 mb-6">You are viewing the Yonodex web gateway.</p>
        
        <p class="text-sm text-gray-500 mb-8 max-w-md mx-auto">
          To unlock encrypted local storage, connect to the P2P network via Tor, and execute trades, 
          you must download the sovereign desktop client.
        </p>
        
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-10">
          <div class="flex flex-col items-center p-4 bg-[#131318] rounded-xl border border-[#2a2a32]">
            <span class="text-3xl mb-3">🔒</span>
            <strong class="text-white mb-1">Local Encryption</strong>
            <span class="text-xs text-gray-500">Argon2id + SQLCipher</span>
          </div>
          <div class="flex flex-col items-center p-4 bg-[#131318] rounded-xl border border-[#2a2a32]">
            <span class="text-3xl mb-3">🧅</span>
            <strong class="text-white mb-1">Tor Network</strong>
            <span class="text-xs text-gray-500">Zero-knowledge P2P</span>
          </div>
          <div class="flex flex-col items-center p-4 bg-[#131318] rounded-xl border border-[#2a2a32]">
            <span class="text-3xl mb-3">🤝</span>
            <strong class="text-white mb-1">Sovereign</strong>
            <span class="text-xs text-gray-500">No central servers</span>
          </div>
        </div>

        <a href="https://github.com/Izodalegend/yonodex/releases" target="_blank" rel="noreferrer" class="inline-block w-full">
          <button class="w-full bg-purple-600 hover:bg-purple-700 text-white font-bold py-3 px-4 rounded-lg transition-colors duration-200">
            Download v{version} for Windows
          </button>
        </a>
      </div>
    </div>

  {:else}
    <!-- DESKTOP TRADING TERMINAL -->
    
    <!-- Top Navbar -->
    <header class="h-14 border-b border-[#2a2a32] bg-[#131318] flex items-center justify-between px-4 shrink-0">
      <div class="flex items-center gap-6">
        <div class="flex items-center gap-2">
          <span class="text-xl font-bold text-white tracking-wide">YONODEX</span>
          <span class="text-xs bg-[#2a2a32] text-gray-400 px-2 py-0.5 rounded">v{version}</span>
        </div>
        <nav class="hidden md:flex gap-4 text-sm font-medium text-gray-400">
          <a href="#" class="text-white">Trade</a>
          <a href="#" class="hover:text-white transition-colors">Portfolio</a>
          <a href="#" class="hover:text-white transition-colors">Governance</a>
        </nav>
      </div>
      
      <div class="flex items-center gap-4">
        {#if isTauri}
          <div class="scale-90 origin-right">
            <TorStatus />
          </div>
        {/if}
        <WalletConnect onconnect={handleConnect} ondisconnect={handleDisconnect} />
      </div>
    </header>

    {#if checkingUnlock}
      <div class="flex-1 flex items-center justify-center">
        <p class="text-gray-500 animate-pulse">Initializing secure environment...</p>
      </div>
    {:else if !unlocked}
      <div class="flex-1 flex items-center justify-center p-4">
        <div class="bg-[#1a1a1f] border border-[#2a2a32] rounded-2xl p-8 max-w-md w-full shadow-2xl">
          <Unlock onunlock={handleUnlock} />
        </div>
      </div>
    {:else}
      <!-- Main Terminal Layout -->
      <main class="flex-1 flex overflow-hidden">
        
        <!-- Left Sidebar -->
        <aside class="w-16 border-r border-[#2a2a32] bg-[#131318] flex flex-col items-center py-4 gap-6 shrink-0">
          <button class="text-purple-500 p-2 rounded-lg bg-[#2a2a32]">
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"></path></svg>
          </button>
          <button class="text-gray-500 hover:text-gray-300 p-2 transition-colors">
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
          </button>
        </aside>

        <!-- Center Panel (Chart & Order Entry) -->
        <div class="flex-1 flex flex-col min-w-0 bg-[#0f0f12]">
          <!-- Chart Placeholder -->
          <div class="flex-1 border-b border-[#2a2a32] p-4 flex flex-col relative">
            <div class="flex justify-between items-center mb-4">
              <div class="flex items-center gap-2">
                <h2 class="text-lg font-bold text-white">ETH / USDC</h2>
                <span class="text-green-500 text-sm">+2.45%</span>
              </div>
              <div class="flex gap-2">
                <span class="text-xs text-gray-500 border border-[#2a2a32] px-2 py-1 rounded">1m</span>
                <span class="text-xs text-white bg-[#2a2a32] px-2 py-1 rounded">15m</span>
                <span class="text-xs text-gray-500 border border-[#2a2a32] px-2 py-1 rounded">1H</span>
              </div>
            </div>
            
            <div class="flex-1 flex items-center justify-center border border-dashed border-[#2a2a32] rounded-xl bg-[#131318]/50 relative overflow-hidden">
              <!-- Fake chart lines for visual effect -->
              <svg class="absolute inset-0 w-full h-full opacity-20" preserveAspectRatio="none" viewBox="0 0 100 100">
                <path d="M0,80 Q20,70 30,40 T60,50 T100,10" fill="none" stroke="#22c55e" stroke-width="1" />
                <path d="M0,90 Q25,85 40,60 T70,65 T100,30" fill="none" stroke="#3b82f6" stroke-width="1" />
              </svg>
              <span class="text-gray-600 font-mono text-sm z-10">Chart integration pending Phase 4 (Oracles)</span>
            </div>
          </div>

          <!-- Bottom Section: Order Book & Entry -->
          <div class="h-72 flex shrink-0">
            
            <!-- Order Book (Mock) -->
            <div class="flex-1 border-r border-[#2a2a32] flex flex-col p-2">
              <div class="grid grid-cols-3 text-xs text-gray-500 mb-2 px-2">
                <span>Price (USDC)</span>
                <span class="text-right">Amount (ETH)</span>
                <span class="text-right">Total</span>
              </div>
              
              <!-- Asks (Sells) -->
              <div class="flex-1 flex flex-col justify-end text-xs font-mono gap-1">
                <div class="grid grid-cols-3 text-red-500 px-2 py-0.5 relative">
                  <div class="absolute right-0 top-0 h-full bg-red-500/10 w-1/3 z-0"></div>
                  <span class="z-10">3,450.20</span><span class="text-right z-10">1.240</span><span class="text-gray-400 text-right z-10">4,278.2</span>
                </div>
                <div class="grid grid-cols-3 text-red-500 px-2 py-0.5 relative">
                  <div class="absolute right-0 top-0 h-full bg-red-500/10 w-1/2 z-0"></div>
                  <span class="z-10">3,448.10</span><span class="text-right z-10">2.500</span><span class="text-gray-400 text-right z-10">8,620.2</span>
                </div>
              </div>

              <div class="py-2 text-center text-lg font-bold text-white border-y border-[#2a2a32] my-1">
                3,445.80
              </div>

              <!-- Bids (Buys) -->
              <div class="flex-1 flex flex-col justify-start text-xs font-mono gap-1">
                <div class="grid grid-cols-3 text-green-500 px-2 py-0.5 relative">
                  <div class="absolute right-0 top-0 h-full bg-green-500/10 w-1/4 z-0"></div>
                  <span class="z-10">3,445.50</span><span class="text-right z-10">0.850</span><span class="text-gray-400 text-right z-10">2,928.6</span>
                </div>
                <div class="grid grid-cols-3 text-green-500 px-2 py-0.5 relative">
                  <div class="absolute right-0 top-0 h-full bg-green-500/10 w-3/4 z-0"></div>
                  <span class="z-10">3,442.00</span><span class="text-right z-10">3.120</span><span class="text-gray-400 text-right z-10">10,739.0</span>
                </div>
              </div>
            </div>

            <!-- Order Entry -->
            <div class="w-80 p-4 border-r border-[#2a2a32] flex flex-col">
              <div class="flex bg-[#131318] p-1 rounded-lg mb-4">
                <button class="flex-1 py-1.5 text-sm font-medium text-green-500 bg-[#1a1a1f] rounded shadow-sm">Buy</button>
                <button class="flex-1 py-1.5 text-sm font-medium text-gray-500 hover:text-white transition-colors">Sell</button>
              </div>
              
              <div class="flex-1 flex flex-col gap-3">
                <div class="bg-[#131318] border border-[#2a2a32] rounded-lg p-2 flex justify-between items-center">
                  <span class="text-xs text-gray-500">Price</span>
                  <span class="font-mono text-sm text-white">Market</span>
                </div>
                <div class="bg-[#131318] border border-[#2a2a32] rounded-lg p-2 flex justify-between items-center">
                  <span class="text-xs text-gray-500">Amount</span>
                  <span class="font-mono text-sm text-white">0.00 ETH</span>
                </div>
                
                <div class="mt-auto">
                  <button class="w-full bg-green-500 hover:bg-green-600 text-white font-bold py-2.5 rounded-lg transition-colors">
                    Connect Wallet to Trade
                  </button>
                </div>
              </div>
            </div>

            <!-- Right Panel (Trade History & Portfolio) -->
            <div class="w-72 bg-[#131318] p-4 flex flex-col">
              <div class="mb-6">
                <h3 class="text-sm font-semibold text-white mb-3">Portfolio</h3>
                <div class="bg-[#1a1a1f] border border-[#2a2a32] rounded-lg p-3">
                  {#if wallet}
                    <Portfolio address={wallet.address} />
                  {:else}
                    <p class="text-xs text-gray-500 text-center">Connect wallet to view</p>
                  {/if}
                </div>
              </div>
              
              <div class="flex-1 flex flex-col min-h-0">
                <h3 class="text-sm font-semibold text-white mb-3">Recent Trades</h3>
                <div class="flex-1 overflow-auto bg-[#1a1a1f] border border-[#2a2a32] rounded-lg p-3">
                  {#if wallet}
                    <TradeHistory address={wallet.address} />
                  {:else}
                    <p class="text-xs text-gray-500 text-center mt-4">Connect wallet to view history</p>
                  {/if}
                </div>
              </div>
            </div>

          </div>
        </div>
      </main>
    {/if}
  {/if}

  <!-- Footer -->
  <footer class="h-8 bg-[#131318] border-t border-[#2a2a32] flex items-center justify-between px-4 text-[11px] text-gray-500 shrink-0">
    <div class="flex items-center gap-2">
      {#if isTauri}
        <span class="w-2 h-2 rounded-full bg-green-500"></span>
        <span>Network Connected</span>
      {:else}
        <span>Web Gateway Mode</span>
      {/if}
    </div>
    <div class="flex gap-4">
      <span>AGPL-3.0</span>
      <span>v{version}</span>
    </div>
  </footer>
</div>
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { torGetState, torGetOnionAddress, type TorState } from "../tor";
  import { saveNodeIdentity, loadNodeIdentity } from "../db";

  let state = $state<TorState>("stopped");
  let onionAddress = $state<string | null>(null);

  let intervalId: ReturnType<typeof setInterval> | null = null;
  let lastPersisted = $state<string | null>(null);

  async function refresh() {
    try {
      // Fast path: load cached identity from encrypted DB first.
      // This ensures the .onion shows even while Tor is bootstrapping.
      if (!onionAddress) {
        try {
          const cached = await loadNodeIdentity();
          if (cached) {
            onionAddress = cached;
            lastPersisted = cached;
          }
        } catch {
          // DB may not be unlocked yet - silent
        }
      }

      const s = await torGetState();
      state = s;

      if (s === "running") {
        try {
          const fresh = await torGetOnionAddress();
          onionAddress = fresh;

          // Persist to encrypted DB when we get a fresh address
          // and it differs from what we last saved.
          if (fresh && fresh !== lastPersisted) {
            try {
              await saveNodeIdentity(fresh);
              lastPersisted = fresh;
            } catch {
              // Non-fatal - try again on next poll
            }
          }
        } catch {
          // Tor not ready - keep cached value if any
        }
      }
    } catch {
      // Silent - footer indicator should never throw
    }
  }

  function shortenOnion(addr: string): string {
    if (addr.length <= 16) return addr;
    return addr.slice(0, 8) + "…" + addr.slice(-6);
  }

  onMount(() => {
    refresh();
    intervalId = setInterval(refresh, 3000);
  });

  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });
</script>

<div class="tor-footer">
  <span class="dot {state}"></span>
  <span class="status">
    {#if state === "running"}
      Tor connected
    {:else if state === "starting"}
      Tor bootstrapping…
    {:else}
      Tor offline
    {/if}
  </span>
  {#if onionAddress}
    <span class="sep">·</span>
    <span class="onion" title={onionAddress}>{shortenOnion(onionAddress)}</span>
  {/if}
</div>

<style>
  .tor-footer {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: #6b6b74;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot.running {
    background: #22c55e;
    box-shadow: 0 0 5px rgba(34, 197, 94, 0.5);
  }

  .dot.starting {
    background: #f59e0b;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .dot.stopped {
    background: #6b6b74;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .status {
    color: #8b8b94;
  }

  .sep {
    color: #3a3a48;
  }

  .onion {
    color: #a78bfa;
    font-family: monospace;
  }
</style>
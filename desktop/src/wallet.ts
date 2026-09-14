// Yonodex Desktop Client - Wallet Integration
// Whitepaper Layer 3 (Client Layer), Section 5.4: EIP-6963 multi-wallet discovery

import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

export interface EIP6963ProviderInfo {
  uuid: string;
  name: string;
  icon: string;
  rdns: string;
}

export interface EIP1193Provider {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
  on?: (event: string, handler: (...args: unknown[]) => void) => void;
  removeListener?: (event: string, handler: (...args: unknown[]) => void) => void;
}

export interface DetectedWallet {
  info: EIP6963ProviderInfo;
  provider: EIP1193Provider;
}

// EIP-6963 wallet discovery
export function discoverWallets(timeoutMs = 300): Promise<DetectedWallet[]> {
  return new Promise((resolve) => {
    const wallets: DetectedWallet[] = [];
    const seen = new Set<string>();

    const handler = (event: Event) => {
      const custom = event as CustomEvent<DetectedWallet>;
      const detail = custom.detail;
      if (!detail || !detail.info || seen.has(detail.info.uuid)) return;
      seen.add(detail.info.uuid);
      wallets.push(detail);
    };

    window.addEventListener("eip6963:announceProvider", handler);
    window.dispatchEvent(new Event("eip6963:requestProvider"));

    setTimeout(() => {
      window.removeEventListener("eip6963:announceProvider", handler);
      resolve(wallets);
    }, timeoutMs);
  });
}

// Frame wallet - desktop-native wallet, exposes JSON-RPC on localhost:1248
const FRAME_RPC_URL = "http://127.0.0.1:1248";

export interface FrameProvider extends EIP1193Provider {
  __isFrame: true;
}

export async function detectFrame(timeoutMs = 500): Promise<DetectedWallet | null> {
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);

    const response = await tauriFetch(FRAME_RPC_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "eth_chainId",
        params: [],
      }),
      signal: controller.signal,
    });

    clearTimeout(timeout);

    if (!response.ok) return null;
    const data = await response.json();
    if (!data || !data.result) return null;

    const provider: FrameProvider = {
      __isFrame: true,
      request: async ({ method, params }) => {
        const res = await tauriFetch(FRAME_RPC_URL, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            jsonrpc: "2.0",
            id: Date.now(),
            method,
            params: params ?? [],
          }),
        });
        const json = await res.json();
        if (json.error) {
          throw new Error(json.error.message || "Frame RPC error");
        }
        return json.result;
      },
    };

    return {
      info: {
        uuid: "frame-desktop-native",
        name: "Frame",
        icon: "",
        rdns: "sh.frame",
      },
      provider,
    };
  } catch {
    // Frame not running - expected
    return null;
  }
}

// Whitepaper Layer 3, Section 5.1: local-first architecture
// Persist connection metadata so the app can restore state on relaunch.

const STORAGE_KEY = "yonodex.wallet.connection";

export interface PersistedConnection {
  address: string;
  chainId: string;
  walletUuid: string;
  walletName: string;
}

export function saveConnection(conn: PersistedConnection): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conn));
}

export function loadConnection(): PersistedConnection | null {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as PersistedConnection;
  } catch {
    return null;
  }
}

export function clearConnection(): void {
  localStorage.removeItem(STORAGE_KEY);
}

// Whitepaper Layer 3, Section 5.4: chain switching via EIP-3326
export async function switchChain(
  provider: EIP1193Provider,
  chainIdHex: string
): Promise<void> {
  await provider.request({
    method: "wallet_switchEthereumChain",
    params: [{ chainId: chainIdHex }],
  });
}

export async function isOnChain(
  provider: EIP1193Provider,
  expectedChainIdHex: string
): Promise<boolean> {
  try {
    const current = (await provider.request({
      method: "eth_chainId",
    })) as string;
    return current.toLowerCase() === expectedChainIdHex.toLowerCase();
  } catch {
    return false;
  }
}
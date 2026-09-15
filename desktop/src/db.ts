// Yonodex Desktop Client - Encrypted DB client wrapper
// Whitepaper Layer 3, Section 5.2: encrypted DB access from Svelte
//
// Thin TypeScript layer over the Rust Tauri commands.
// The DB stays locked until the user provides their master password.

import { invoke } from "@tauri-apps/api/core";

export interface WalletConfig {
  address: string;
  chain_id: string;
  wallet_uuid: string;
  wallet_name: string;
}

export async function dbIsUnlocked(): Promise<boolean> {
  return invoke<boolean>("db_is_unlocked");
}
export async function dbIsInitialized(): Promise<boolean> {
  return invoke<boolean>("db_is_initialized");
}

export async function unlockDb(password: string): Promise<void> {
  return invoke("unlock_db", { password });
}

export async function lockDb(): Promise<void> {
  return invoke("lock_db");
}

export async function saveWalletConfig(config: WalletConfig): Promise<void> {
  return invoke("save_wallet_config", {
    address: config.address,
    chainId: config.chain_id,
    walletUuid: config.wallet_uuid,
    walletName: config.wallet_name,
  });
}

export async function loadWalletConfig(): Promise<WalletConfig | null> {
  return invoke<WalletConfig | null>("load_wallet_config");
}

export async function clearWalletConfig(): Promise<void> {
  return invoke("clear_wallet_config");
}

export interface TradeRecord {
  id: number;
  tx_hash: string | null;
  chain_id: string;
  pair: string;
  side: string;
  amount_in: string;
  amount_out: string;
  token_in: string;
  token_out: string;
  status: string;
  timestamp: number;
  notes: string | null;
}

export async function saveTrade(trade: Omit<TradeRecord, "id" | "timestamp">): Promise<number> {
  return invoke<number>("save_trade", {
    txHash: trade.tx_hash,
    chainId: trade.chain_id,
    pair: trade.pair,
    side: trade.side,
    amountIn: trade.amount_in,
    amountOut: trade.amount_out,
    tokenIn: trade.token_in,
    tokenOut: trade.token_out,
    status: trade.status,
    notes: trade.notes,
  });
}

export async function loadTrades(limit = 100): Promise<TradeRecord[]> {
  return invoke<TradeRecord[]>("load_trades", { limit });
}

export async function deleteTrade(id: number): Promise<void> {
  return invoke("delete_trade", { id });
}

export async function savePasswordHint(hint: string): Promise<void> {
  return invoke("save_password_hint", { hint });
}

export async function loadPasswordHint(): Promise<string | null> {
  return invoke<string | null>("load_password_hint");
}

export async function resetLocalData(): Promise<void> {
  return invoke("reset_local_data");
}
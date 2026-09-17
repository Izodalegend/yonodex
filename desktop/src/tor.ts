// Yonodex Desktop Client - Tor client wrapper
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
//
// Tor auto-starts with the app. The UI only reads state - no user toggle.

import { invoke } from "@tauri-apps/api/core";

export type TorState = "stopped" | "starting" | "running";

export async function torGetState(): Promise<TorState> {
  return invoke<TorState>("tor_get_state");
}

export async function torGetOnionAddress(): Promise<string> {
  return invoke<string>("tor_get_onion_address");
}

export async function torIsRunning(): Promise<boolean> {
  return invoke<boolean>("tor_is_running");
}

// Debug/admin only - not exposed in normal UI.
export async function torStart(): Promise<void> {
  return invoke("tor_start");
}

export async function torStop(): Promise<void> {
  return invoke("tor_stop");
}
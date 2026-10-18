# Yonodex (TefaDex)

**A censorship-resistant, privacy-preserving AMM & P2P DEX.**

Yonodex is a next-generation decentralized exchange designed from first principles around security, privacy, and censorship resistance. Unlike conventional DEXs that rely on centralized frontend infrastructure, proprietary matching engines, or custodial intermediaries, Yonodex operates as a fully peer-to-peer network.

## Architecture (10 Layers)
1. **Public Interface:** Disposable, centralized (IPFS, Tor, Clearnet).
2. **Software Distribution:** Verifiable, trust-minimized (GPG, reproducible builds).
3. **Client Layer:** User Sovereignty (Tauri + Svelte + SQLCipher).
4. **P2P Network:** Decentralized (Tor hidden services, libp2p, Noise_XX).
5. **Protocol Layer:** Off-chain order book (CRDT) & deterministic matching engine.
6. **Data & Oracle:** Trust-minimized (Chainlink, Pyth, The Graph).
7. **Settlement:** On-chain 2-of-2 multisig escrow (YonodexEscrow).
8. **Governance:** DAO-controlled (TEFA token, Snapshot X).
9. **Security:** Active defense (Forta, Tenderly).
10. **Community:** Censorship-resistant coordination (Matrix, Discourse).

## Repository Structure
This is a monorepo containing the following components:
*   `/src-tauri` — The Rust backend (Tauri v2 core, P2P networking, SQLCipher DB).
*   `/desktop` — The Svelte + Vite frontend user interface.
*   `/contracts-foundry` — The Solidity smart contracts (Escrow, HTLC, Smart Router) built with Foundry.
*   `/contracts` — Legacy Hardhat contracts (deprecated/for reference).

## Project Status: Phase 2 (Protocol & Settlement)
- **Phase 1 (Client & P2P):** ✅ Complete (Encrypted DB, Tor transport, Frame wallet).
- **Phase 2 (Protocol & Settlement):** 🔄 In Progress (CRDT order book, Matching Engine done. Escrow deployed to local Anvil).
- **Phase 3-5 (Distribution, Oracles, Governance):** 📋 Planned.

## Quick Start (Development)

### Prerequisites
- Node.js (v18+)
- Rust (stable) & Cargo
- Tauri CLI (`cargo install tauri-cli --version "^2"`)
- **Windows Only:** Microsoft Visual C++ Build Tools and Strawberry Perl (Required for compiling OpenSSL/SQLCipher from source).

### Build & Run
```bash
# 1. Install frontend dependencies
cd desktop && npm install

# 2. Run the app in development mode
npm run tauri dev
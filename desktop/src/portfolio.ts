// Yonodex Desktop Client - Portfolio reader
// Whitepaper Layer 3 (Client Layer), Section 5.1: local-first architecture
// Reads use a direct RPC provider (not the wallet) - wallet is for signing only.

import { JsonRpcProvider, Contract, formatEther } from "ethers";
import { CHAINS, CONTRACTS, DEFAULT_CHAIN } from "./config";

const ERC20_ABI = [
  "function balanceOf(address owner) view returns (uint256)",
  "function decimals() view returns (uint8)",
  "function symbol() view returns (string)",
];

const POOL_ABI = [
  "function getLpInfo(address user) view returns (uint256 shares, uint256 total, uint256 sharePercent)",
];

export interface TokenBalance {
  symbol: string;
  address: string;
  balance: string;
  raw: bigint;
}

export interface LpPosition {
  shares: string;
  total: string;
  sharePercent: number;
}

export interface PortfolioSnapshot {
  address: string;
  chainId: string;
  chainName: string;
  native: {
    symbol: string;
    balance: string;
  };
  tokens: TokenBalance[];
  lp: LpPosition | null;
  fetchedAt: number;
}

const ACTIVE_CHAIN = DEFAULT_CHAIN;

export async function loadPortfolio(address: string): Promise<PortfolioSnapshot> {
  const chain = CHAINS[ACTIVE_CHAIN];
  const contracts = CONTRACTS[ACTIVE_CHAIN];

  // Direct RPC read - not routed through Frame
  const provider = new JsonRpcProvider(chain.rpcUrl);

  // 1. Native balance
  const nativeWei = await provider.getBalance(address);
  const nativeBalance = formatEther(nativeWei);

  // 2. ERC-20 balances in parallel
  const tokenSpecs = [
    { symbol: "TKA", address: contracts.tokenA },
    { symbol: "TKB", address: contracts.tokenB },
  ];

  const tokenResults = await Promise.all(
    tokenSpecs.map(async (spec) => {
      try {
        if (!spec.address) return null;
        const c = new Contract(spec.address, ERC20_ABI, provider);
        const raw = (await c.balanceOf(address)) as bigint;
        const decimals = (await c.decimals()) as bigint;
        const balance = formatUnitsBigInt(raw, Number(decimals));
        return {
          symbol: spec.symbol,
          address: spec.address,
          balance,
          raw,
        } as TokenBalance;
      } catch {
        return null;
      }
    })
  );

  const tokens = tokenResults.filter((t): t is TokenBalance => t !== null);

  // 3. LP pool position
  let lp: LpPosition | null = null;
  try {
    if (contracts.pool) {
      const pool = new Contract(contracts.pool, POOL_ABI, provider);
      const [shares, total, sharePercent] = (await pool.getLpInfo(address)) as [
        bigint,
        bigint,
        bigint
      ];
      lp = {
        shares: formatUnitsBigInt(shares, 18),
        total: formatUnitsBigInt(total, 18),
        sharePercent: Number(sharePercent),
      };
    }
  } catch {
    lp = null;
  }

  return {
    address,
    chainId: chain.chainIdHex,
    chainName: chain.name,
    native: {
      symbol: chain.nativeCurrency.symbol,
      balance: nativeBalance,
    },
    tokens,
    lp,
    fetchedAt: Date.now(),
  };
}

function formatUnitsBigInt(value: bigint, decimals: number): string {
  const s = value.toString().padStart(decimals + 1, "0");
  const intPart = s.slice(0, s.length - decimals);
  const fracPart = s.slice(s.length - decimals).replace(/0+$/, "");
  return fracPart ? `${intPart}.${fracPart}` : intPart;
}
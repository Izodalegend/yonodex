// Yonodex Desktop Client — Configuration
// Whitepaper Layer 3 (Client Layer): chain + contract configuration

export interface ChainConfig {
  chainId: number;
  chainIdHex: string;
  name: string;
  shortName: string;
  rpcUrl: string;
  explorerUrl: string;
  nativeCurrency: {
    name: string;
    symbol: string;
    decimals: number;
  };
}

export interface ContractAddresses {
  router: string;
  tokenA: string;
  tokenB: string;
  pool: string;
  forwarder: string;
}

export const CHAINS: Record<string, ChainConfig> = {
  sepolia: {
    chainId: 11155111,
    chainIdHex: "0xaa36a7",
    name: "Sepolia Testnet",
    shortName: "Sepolia",
    rpcUrl: "https://sepolia.infura.io/v3/7cc54e6c6a2146b1963a922ab3ce5b0c",
    explorerUrl: "https://sepolia.etherscan.io",
    nativeCurrency: {
      name: "Sepolia Ether",
      symbol: "ETH",
      decimals: 18,
    },
  },
  arbitrumSepolia: {
    chainId: 421614,
    chainIdHex: "0x66eee",
    name: "Arbitrum Sepolia",
    shortName: "Arbitrum",
    rpcUrl: "https://sepolia-rollup.arbitrum.io/rpc",
    explorerUrl: "https://sepolia.arbiscan.io",
    nativeCurrency: {
      name: "Arbitrum Ether",
      symbol: "ETH",
      decimals: 18,
    },
  },
};

// Deployed contract addresses (Sepolia — from deployment-sepolia.json)
export const CONTRACTS: Record<string, ContractAddresses> = {
  sepolia: {
    router: "0x1cf4Fa805917Dde8E5D7c7afa5bb52130541DA73",
    tokenA: "0x1c047809679F026b3f4c2619d0F89BB319922422",
    tokenB: "0x61dCbEDbf407812E9f3ad89973246692087D6f42",
    pool: "0xC5F01fedCD2b484B946b6b9c0fb46b82a7FD726B",
    forwarder: "0x269630E8457166efA355038d31eaC355c8569b40",
  },
  arbitrumSepolia: {
    router: "",
    tokenA: "",
    tokenB: "",
    pool: "",
    forwarder: "",
  },
};

export const DEFAULT_CHAIN = "sepolia";
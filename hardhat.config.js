import hardhatEthers from "@nomicfoundation/hardhat-ethers";

export default {
  solidity: "0.8.24",
  networks: {
    sepolia: {
      type: "http",
      url: "https://sepolia.infura.io/v3/7cc54e6c6a2146b1963a922ab3ce5b0c",
      accounts: ["0xc4fa308df2fe8baf409ac497723f15d4f94605acb162e979664bd7154a35965f"],
      chainId: 11155111,
    },
  },
  plugins: [hardhatEthers],
};

	import { network } from "hardhat";

async function main() {
  const { ethers } = await network.connect();

  const [deployer] = await ethers.getSigners();
  console.log("Deployer:", deployer.address);
  const balance = await ethers.provider.getBalance(deployer.address);
  console.log("Sepolia ETH balance:", ethers.formatEther(balance));

  if (balance === 0n) {
    console.log("❌ No Sepolia ETH! Get testnet ETH first.");
    return;
  }

  console.log("\n📦 Deploying to Sepolia...\n");

  const feeData = await ethers.provider.getFeeData();
  const gasPrice = feeData.gasPrice;
  console.log("Current gas price:", ethers.formatUnits(gasPrice, "gwei"), "gwei");

  const opts = { gasPrice: gasPrice * 2n, gasLimit: 5000000 };

  // 1. Deploy TrustedForwarder
  console.log("Deploying TrustedForwarder...");
  const Forwarder = await ethers.getContractFactory("TrustedForwarder");
  const forwarder = await Forwarder.deploy(opts);
  await forwarder.waitForDeployment();
  const forwarderAddr = await forwarder.getAddress();
  console.log(`TrustedForwarder: ${forwarderAddr}`);

  // 2. Deploy Router (needs forwarder address)
  console.log("Deploying Router...");
  const Router = await ethers.getContractFactory("Router");
  const router = await Router.deploy(forwarderAddr, opts);
  await router.waitForDeployment();
  const routerAddr = await router.getAddress();
  console.log(`Router: ${routerAddr}`);

  // 3. Deploy tokens
  console.log("Deploying TokenA...");
  const Token = await ethers.getContractFactory("MockERC20");
  const tokenA = await Token.deploy("TEFA Token A", "TKA", 18, opts);
  await tokenA.waitForDeployment();
  const tokenAAddr = await tokenA.getAddress();
  console.log(`TokenA (TKA): ${tokenAAddr}`);

  console.log("Deploying TokenB...");
  const tokenB = await Token.deploy("TEFA Token B", "TKB", 18, opts);
  await tokenB.waitForDeployment();
  const tokenBAddr = await tokenB.getAddress();
  console.log(`TokenB (TKB): ${tokenBAddr}`);

  // 4. Create Pool
  console.log("Creating Pool...");
  const createTx = await router.createPool(tokenAAddr, tokenBAddr, opts);
  await createTx.wait();
  const poolAddr = await router.getPool(tokenAAddr, tokenBAddr);
  console.log(`Pool: ${poolAddr}`);

  console.log("\n🎉 DEX DEPLOYED TO SEPOLIA!");
  console.log("\n📋 Save these addresses for frontend:");
  console.log(`FORWARDER: ${forwarderAddr}`);
  console.log(`ROUTER: ${routerAddr}`);
  console.log(`TOKEN_A: ${tokenAAddr}`);
  console.log(`TOKEN_B: ${tokenBAddr}`);
  console.log(`POOL: ${poolAddr}`);
}

main().catch(console.error);

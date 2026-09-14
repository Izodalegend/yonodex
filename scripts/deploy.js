cat > scripts/deploy-sepolia-fixed.js << 'EOF'
import hre from "hardhat";

async function main() {
  console.log("Deploying TEFA DEX to Sepolia...");
  const { ethers } = await hre.network.create({});

  const [deployer] = await ethers.getSigners();
  console.log("Deployer:", deployer.address);
  const balance = await ethers.provider.getBalance(deployer.address);
  console.log("Sepolia ETH balance:", ethers.formatEther(balance));

  if (balance === 0n) {
    console.log("No Sepolia ETH!");
    return;
  }

  // Deploy tokens
  console.log("Deploying TokenA...");
  const Token = await ethers.getContractFactory("MockERC20");
  const tokenA = await Token.deploy("TEFA Token A", "TKA", ethers.parseEther("1000000"));
  await tokenA.waitForDeployment();
  const tokenAAddr = await tokenA.getAddress();
  console.log(`TokenA: ${tokenAAddr}`);

  console.log("Deploying TokenB...");
  const tokenB = await Token.deploy("TEFA Token B", "TKB", ethers.parseEther("1000000"));
  await tokenB.waitForDeployment();
  const tokenBAddr = await tokenB.getAddress();
  console.log(`TokenB: ${tokenBAddr}`);

  // Deploy Router (assuming no constructor args)
  console.log("Deploying Router...");
  const Router = await ethers.getContractFactory("Router");
  const router = await Router.deploy();
  await router.waitForDeployment();
  const routerAddr = await router.getAddress();
  console.log(`Router: ${routerAddr}`);

  // Create Pool
  console.log("Creating Pool...");
  const createTx = await router.createPool(tokenAAddr, tokenBAddr);
  await createTx.wait();
  const poolAddr = await router.getPool(tokenAAddr, tokenBAddr);
  console.log(`Pool: ${poolAddr}`);

  // Approve and add liquidity
  console.log("Approving Router...");
  const approveA = await tokenA.approve(routerAddr, ethers.parseEther("10000"));
  await approveA.wait();
  const approveB = await tokenB.approve(routerAddr, ethers.parseEther("10000"));
  await approveB.wait();

  console.log("Adding liquidity...");
  const addLiqTx = await router.addLiquidity(tokenAAddr, tokenBAddr, ethers.parseEther("10000"), ethers.parseEther("10000"));
  await addLiqTx.wait();
  console.log("Liquidity added: 10,000 TKA + 10,000 TKB");

  // Test swap
  console.log("Testing swap...");
  const approveSwap = await tokenA.approve(routerAddr, ethers.parseEther("10"));
  await approveSwap.wait();
  const swapTx = await router.swap(tokenAAddr, tokenBAddr, ethers.parseEther("10"));
  await swapTx.wait();
  console.log("Test swap successful!");

  console.log("\nDEPLOYMENT COMPLETE!");
  console.log(`ROUTER: ${routerAddr}`);
  console.log(`TOKEN_A: ${tokenAAddr}`);
  console.log(`TOKEN_B: ${tokenBAddr}`);
  console.log(`POOL: ${poolAddr}`);
}

main().catch(console.error);
EOF

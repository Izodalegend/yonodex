// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Script.sol";
import {Config} from "./Config.sol";
import {AddressBook} from "./helpers/AddressBook.sol";
import {YonodexGuardian} from "../src/YonodexGuardian.sol";
import {YonodexEscrowFactory} from "../src/YonodexEscrowFactory.sol";
import {YonodexRouter} from "../src/YonodexRouter.sol";
import {YonodexHTLC} from "../src/YonodexHTLC.sol";
import {YonodexPaymaster} from "../src/YonodexPaymaster.sol";
import {IEntryPoint} from "account-abstraction/interfaces/IEntryPoint.sol";

contract Deploy is Script, AddressBook {
    function run() external {
        // 1. Safety Check: Verify we are on a supported testnet
        require(
            block.chainid == Config.SEPOLIA_CHAIN_ID || block.chainid == Config.ARB_SEPOLIA_CHAIN_ID,
            "Deploy: Unsupported chain ID. Aborting to prevent mainnet accidents."
        );

        // 2. Safety Check: Verify the canonical EntryPoint exists on this chain
        require(
            Config.ENTRY_POINT.code.length > 0,
            "Deploy: Canonical EntryPoint not found on this chain."
        );

        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        console.log("Starting Yonodex Protocol Deployment...");
        console.log("Deployer Address:", deployer);
        console.log("Chain ID:", block.chainid);

        vm.startBroadcast(deployerPrivateKey);

        // 3. Deploy in dependency order
        // Guardian (Owner is the deployer)
        YonodexGuardian guardian = new YonodexGuardian(deployer);
        console.log("Guardian deployed at:", address(guardian));

        // Factory (Requires Guardian)
        YonodexEscrowFactory factory = new YonodexEscrowFactory(address(guardian));
        console.log("Factory deployed at:", address(factory));

        // Router (No dependencies)
        YonodexRouter router = new YonodexRouter();
        console.log("Router deployed at:", address(router));

        // HTLC (No dependencies)
        YonodexHTLC htlc = new YonodexHTLC();
        console.log("HTLC deployed at:", address(htlc));

        // Paymaster (Requires EntryPoint, Signer is the deployer for now)
        YonodexPaymaster paymaster = new YonodexPaymaster(
            IEntryPoint(Config.ENTRY_POINT),
            deployer
        );
        console.log("Paymaster deployed at:", address(paymaster));

        vm.stopBroadcast();

        // 4. Save to Address Book
        saveDeployment(
            block.chainid,
            address(guardian),
            address(factory),
            address(router),
            address(htlc),
            address(paymaster),
            Config.ENTRY_POINT
        );

        console.log("Deployment complete. Address book saved to deployments/", vm.toString(block.chainid), ".json");
    }
}
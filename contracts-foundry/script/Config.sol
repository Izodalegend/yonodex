// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

library Config {
    // Official EIP-4337 v0.7.0 EntryPoint (deployed via CREATE2, same address on all chains)
    address constant ENTRY_POINT = 0x0000000071727De22E5E9d8BAf0edAc6f37da032;

    // Chain IDs
    uint256 constant SEPOLIA_CHAIN_ID = 11155111;
    uint256 constant ARB_SEPOLIA_CHAIN_ID = 421614;

    // Defaults
    uint256 constant DEFAULT_ESCROW_DEADLINE = 1 days;
    uint256 constant DEFAULT_ARBITRATION_FEE_BPS = 100; // 1%
}
// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexPaymaster} from "../src/YonodexPaymaster.sol";
import {EntryPoint} from "account-abstraction/core/EntryPoint.sol";
import {IEntryPoint} from "account-abstraction/interfaces/IEntryPoint.sol";

contract YonodexPaymasterTest is Test {
    YonodexPaymaster paymaster;
    EntryPoint entryPoint;

    address owner = address(0xABCD);
    address signer = address(0x1111);
    address newSigner = address(0x2222);
    address stranger = address(0xDEAD);

    function setUp() public {
        // Deploy the official EIP-4337 EntryPoint
        entryPoint = new EntryPoint();

        // Deploy the Paymaster
        vm.prank(owner);
        paymaster = new YonodexPaymaster(IEntryPoint(address(entryPoint)), signer);
    }

    // ---- Constructor ----

    function test_constructor_sets_signer() public view {
        assertEq(paymaster.verifyingSigner(), signer);
    }

    function test_constructor_sets_owner() public view {
        assertEq(paymaster.owner(), owner);
    }

    // ---- Verifying Signer ----

    function test_setVerifyingSigner_works_for_owner() public {
        vm.prank(owner);
        paymaster.setVerifyingSigner(newSigner);
        assertEq(paymaster.verifyingSigner(), newSigner);
    }

    function test_setVerifyingSigner_reverts_for_stranger() public {
        vm.prank(stranger);
        // OpenZeppelin v5 Ownable custom error
        vm.expectRevert(abi.encodeWithSignature("OwnableUnauthorizedAccount(address)", stranger));
        paymaster.setVerifyingSigner(newSigner);
    }

    // ---- Deposits (Gas Sponsorship) ----

    function test_deposit_funds_entryPoint() public {
        vm.deal(owner, 10 ether);
        
        vm.prank(owner);
        paymaster.deposit{value: 5 ether}();

        // The EntryPoint should now hold the Paymaster's deposit
        uint256 deposit = entryPoint.balanceOf(address(paymaster));
        assertEq(deposit, 5 ether);
    }
}
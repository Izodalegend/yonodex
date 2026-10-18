// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexGuardian} from "../src/YonodexGuardian.sol";

contract YonodexGuardianTest is Test {
    YonodexGuardian guardian;

    address owner = address(0xABCD);
    address stranger = address(0x1234);
    address newOwner = address(0x5678);

    event EscrowCreationPaused(address indexed by);
    event EscrowCreationUnpaused(address indexed by);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    function setUp() public {
        guardian = new YonodexGuardian(owner);
    }

    // ---- Constructor ----

    function test_constructor_sets_owner() public view {
        assertEq(guardian.owner(), owner);
    }

    function test_constructor_reverts_zero_owner() public {
        vm.expectRevert(YonodexGuardian.InvalidOwner.selector);
        new YonodexGuardian(address(0));
    }

    // ---- Pause ----

    function test_pause_works_for_owner() public {
        assertFalse(guardian.escrowCreationPaused());

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit EscrowCreationPaused(owner);
        
        guardian.pauseEscrowCreation();
        assertTrue(guardian.escrowCreationPaused());
    }

    function test_pause_reverts_for_stranger() public {
        vm.prank(stranger);
        vm.expectRevert(YonodexGuardian.Unauthorized.selector);
        guardian.pauseEscrowCreation();
    }

    // ---- Unpause ----

    function test_unpause_works_for_owner() public {
        vm.prank(owner);
        guardian.pauseEscrowCreation();
        assertTrue(guardian.escrowCreationPaused());

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit EscrowCreationUnpaused(owner);
        
        guardian.unpauseEscrowCreation();
        assertFalse(guardian.escrowCreationPaused());
    }

    function test_unpause_reverts_for_stranger() public {
        vm.prank(stranger);
        vm.expectRevert(YonodexGuardian.Unauthorized.selector);
        guardian.unpauseEscrowCreation();
    }

    // ---- Transfer Ownership ----

    function test_transfer_ownership_works() public {
        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit OwnershipTransferred(owner, newOwner);
        
        guardian.transferOwnership(newOwner);
        assertEq(guardian.owner(), newOwner);
    }

    function test_transfer_ownership_reverts_for_stranger() public {
        vm.prank(stranger);
        vm.expectRevert(YonodexGuardian.Unauthorized.selector);
        guardian.transferOwnership(newOwner);
    }

    function test_transfer_ownership_reverts_zero_address() public {
        vm.prank(owner);
        vm.expectRevert(YonodexGuardian.InvalidOwner.selector);
        guardian.transferOwnership(address(0));
    }
}
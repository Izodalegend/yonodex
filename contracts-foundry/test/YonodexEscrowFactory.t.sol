// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexEscrowFactory} from "../src/YonodexEscrowFactory.sol";
import {YonodexEscrow} from "../src/YonodexEscrow.sol";

/// Mock Guardian — just enough to test the pause gate.
contract MockGuardian {
    bool public paused;

    function setPaused(bool value) external {
        paused = value;
    }

    function escrowCreationPaused() external view returns (bool) {
        return paused;
    }
}

contract YonodexEscrowFactoryTest is Test {
    // ---- Test actors ----
    address buyer = address(0xB0B);
    address seller = address(0xA11CE);
    address stranger = address(0xDEAD);

    address tokenBuyer = address(0x1111);
    address tokenSeller = address(0x2222);

    uint256 amountBuyer = 1000e18;
    uint256 amountSeller = 2000e18;
    uint256 deadline;

    YonodexEscrowFactory factory;

    function setUp() public {
        deadline = block.timestamp + 1 days;
        factory = new YonodexEscrowFactory(address(0));
    }

    // ---- Constructor ----

    function test_constructor_sets_guardian_zero() public view {
        assertEq(factory.guardian(), address(0));
    }

    function test_constructor_sets_guardian_nonzero() public {
        MockGuardian g = new MockGuardian();
        YonodexEscrowFactory f = new YonodexEscrowFactory(address(g));
        assertEq(f.guardian(), address(g));
    }

    // ---- createEscrow — basic ----

    function test_createEscrow_deploys() public {
        vm.prank(stranger);
        address escrowAddr = factory.createEscrow(
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            address(0)
        );

        // Escrow exists at the returned address
        YonodexEscrow escrow = YonodexEscrow(escrowAddr);
        assertEq(escrow.buyer(), buyer);
        assertEq(escrow.seller(), seller);
        assertEq(escrow.amountBuyer(), amountBuyer);
        assertEq(escrow.amountSeller(), amountSeller);
        assertEq(escrow.deadline(), deadline);
    }

    function test_createEscrow_increments_nonce() public {
        assertEq(factory.nonce(), 0);
        vm.prank(stranger);
        factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        assertEq(factory.nonce(), 1);
    }

    function test_createEscrow_emits_event() public {
        // We can't easily predict the salt, so just check the event topic is emitted
        vm.prank(stranger);
        vm.recordLogs();
        factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        Vm.Log[] memory logs = vm.getRecordedLogs();

        // Event signature: EscrowCreated(address,address,address,bytes32)
        bytes32 expectedTopic = keccak256("EscrowCreated(address,address,address,bytes32)");
        bool found = false;
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].topics[0] == expectedTopic) {
                found = true;
                break;
            }
        }
        assertTrue(found, "EscrowCreated event not emitted");
    }

    // ---- Same params, different nonce → different address ----

    function test_createEscrow_same_params_different_address() public {
        vm.prank(stranger);
        address a = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        vm.prank(stranger);
        address b = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        assertTrue(a != b, "nonce must produce unique addresses");
    }

    // ---- Precomputation ----

    function test_computeNextEscrowAddress_matches_actual() public {
        // Precompute with caller = stranger, using current nonce (0)
        address predicted = factory.computeNextEscrowAddress(
            stranger,
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            address(0)
        );

        // Now actually deploy from stranger
        vm.prank(stranger);
        address actual = factory.createEscrow(
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            address(0)
        );

        assertEq(predicted, actual, "precomputed address must match deployment");
    }

    function test_computeEscrowAddress_matches_actual_with_explicit_nonce() public {
        uint256 explicitNonce = 42;
        address predicted = factory.computeEscrowAddress(
            stranger,
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            address(0),
            explicitNonce
        );

        // Advance nonce to 42 by calling createEscrow 42 times
        // (fast — each call is a fresh deploy but gas isn't our concern in tests)
        for (uint256 i = 0; i < 42; i++) {
            vm.prank(stranger);
            factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        }

        // Next call uses nonce 42
        vm.prank(stranger);
        address actual = factory.createEscrow(
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            address(0)
        );

        assertEq(predicted, actual, "explicit-nonce precompute must match");
    }

    function test_computeNextEscrowAddress_changes_with_nonce() public {
        address a = factory.computeNextEscrowAddress(stranger, buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        vm.prank(stranger);
        factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        address b = factory.computeNextEscrowAddress(stranger, buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        assertTrue(a != b, "next-address must change after a deployment");
    }

    // ---- Guardian pause gate ----

    function test_createEscrow_reverts_when_guardian_paused() public {
        MockGuardian g = new MockGuardian();
        YonodexEscrowFactory f = new YonodexEscrowFactory(address(g));

        g.setPaused(true);

        vm.prank(stranger);
        vm.expectRevert("factory: paused");
        f.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
    }

    function test_createEscrow_works_when_guardian_unpaused() public {
        MockGuardian g = new MockGuardian();
        YonodexEscrowFactory f = new YonodexEscrowFactory(address(g));

        g.setPaused(false);

        vm.prank(stranger);
        address escrowAddr = f.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        assertTrue(escrowAddr != address(0));
    }

    function test_createEscrow_works_when_guardian_zero() public {
        // Our default factory has guardian = address(0); pause check skipped
        vm.prank(stranger);
        address escrowAddr = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        assertTrue(escrowAddr != address(0));
    }

    // ---- Anyone can create ----

    function test_createEscrow_callable_by_buyer() public {
        vm.prank(buyer);
        address addr = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        assertTrue(addr != address(0));
    }

    function test_createEscrow_callable_by_seller() public {
        vm.prank(seller);
        address addr = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        assertTrue(addr != address(0));
    }

    function test_createEscrow_callable_by_stranger() public {
        vm.prank(stranger);
        address addr = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));
        assertTrue(addr != address(0));
    }

    // ---- Different caller produces different address (caller in salt) ----

    function test_createEscrow_caller_in_salt() public {
        vm.prank(buyer);
        address fromBuyer = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        vm.prank(seller);
        address fromSeller = factory.createEscrow(buyer, seller, tokenBuyer, tokenSeller, amountBuyer, amountSeller, deadline, address(0));

        assertTrue(fromBuyer != fromSeller, "caller must affect salt");
    }
}
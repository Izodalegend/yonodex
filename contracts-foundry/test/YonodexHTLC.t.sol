// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexHTLC} from "../src/YonodexHTLC.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract MockToken is ERC20 {
    constructor() ERC20("Mock Token", "MOCK") {}
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract YonodexHTLCTest is Test {
    YonodexHTLC htlc;
    MockToken token;

    address sender = address(0xA11CE);
    address receiver = address(0xB0B);
    address stranger = address(0xDEAD);

    uint256 constant AMOUNT = 1 ether;
    bytes32 constant PREIMAGE = keccak256("super-secret-preimage");
    bytes32 constant HASHLOCK = keccak256(abi.encodePacked(PREIMAGE));
    
    uint256 timelock;
    bytes32 swapIdETH = keccak256("swap-eth");
    bytes32 swapIdToken = keccak256("swap-token");

    function setUp() public {
        htlc = new YonodexHTLC();
        token = new MockToken();
        
        timelock = block.timestamp + 1 days;
        
        vm.deal(sender, 10 ether);
        token.mint(sender, 10 ether);
    }

    // ---- Creation ----

    function test_createSwap_ETH() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);
        
        (
            address _sender,
            address _receiver,
            address _token,
            uint256 _amount,
            bytes32 _hashlock,
            uint256 _timelock,
            bool _redeemed,
            bool _refunded
        ) = htlc.swaps(swapIdETH);
        
        assertEq(_sender, sender);
        assertEq(_receiver, receiver);
        assertEq(_token, address(0));
        assertEq(_amount, AMOUNT);
        assertEq(_hashlock, HASHLOCK);
        assertEq(_timelock, timelock);
        assertFalse(_redeemed);
        assertFalse(_refunded);
    }

    function test_createSwap_ERC20() public {
        vm.startPrank(sender);
        token.approve(address(htlc), AMOUNT);
        htlc.createSwap(swapIdToken, receiver, address(token), AMOUNT, HASHLOCK, timelock);
        vm.stopPrank();
        
        assertEq(token.balanceOf(address(htlc)), AMOUNT);
    }

    // ---- Redeem ----

    function test_redeem_ETH() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        uint256 receiverBalanceBefore = receiver.balance;

        // Anyone can call redeem (usually the receiver), but they MUST have the preimage
        vm.prank(stranger);
        htlc.redeem(swapIdETH, PREIMAGE);

        assertEq(receiver.balance, receiverBalanceBefore + AMOUNT);
        (,,,,,, bool redeemed,) = htlc.swaps(swapIdETH);
        assertTrue(redeemed);
    }

    function test_redeem_ERC20() public {
        vm.startPrank(sender);
        token.approve(address(htlc), AMOUNT);
        htlc.createSwap(swapIdToken, receiver, address(token), AMOUNT, HASHLOCK, timelock);
        vm.stopPrank();

        vm.prank(receiver);
        htlc.redeem(swapIdToken, PREIMAGE);

        assertEq(token.balanceOf(receiver), AMOUNT);
    }

    // ---- Refund ----

    function test_refund_ETH_after_timelock() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        // Fast forward past timelock
        vm.warp(timelock + 1);

        uint256 senderBalanceBefore = sender.balance;

        // Anyone can trigger the refund (e.g., a relayer), funds go back to sender
        vm.prank(stranger);
        htlc.refund(swapIdETH);

        assertEq(sender.balance, senderBalanceBefore + AMOUNT);
        (,,,,,,, bool refunded) = htlc.swaps(swapIdETH);
        assertTrue(refunded);
    }

    // ---- Failure Cases ----

    function test_revert_invalid_preimage() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        vm.prank(receiver);
        vm.expectRevert(YonodexHTLC.InvalidPreimage.selector);
        htlc.redeem(swapIdETH, keccak256("wrong-preimage"));
    }

    function test_revert_refund_before_timelock() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        vm.prank(sender);
        vm.expectRevert(YonodexHTLC.SwapNotExpired.selector);
        htlc.refund(swapIdETH);
    }

    function test_revert_double_redeem() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        vm.prank(receiver);
        htlc.redeem(swapIdETH, PREIMAGE);

        vm.prank(receiver);
        vm.expectRevert(YonodexHTLC.AlreadyRedeemedOrRefunded.selector);
        htlc.redeem(swapIdETH, PREIMAGE);
    }

    function test_revert_redeem_after_refund() public {
        vm.prank(sender);
        htlc.createSwap{value: AMOUNT}(swapIdETH, receiver, address(0), AMOUNT, HASHLOCK, timelock);

        vm.warp(timelock + 1);
        htlc.refund(swapIdETH);

        vm.prank(receiver);
        vm.expectRevert(YonodexHTLC.AlreadyRedeemedOrRefunded.selector);
        htlc.redeem(swapIdETH, PREIMAGE);
    }
}
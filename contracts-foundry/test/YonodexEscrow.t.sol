// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexEscrow} from "../src/YonodexEscrow.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// Minimal ERC20 for testing — standard OZ ERC20 with an open mint.
contract MockERC20 is ERC20 {
    constructor(string memory name_, string memory symbol_) ERC20(name_, symbol_) {}

    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

contract YonodexEscrowTest is Test {
    // ---- Test actors (real private keys so we can sign) ----
    uint256 constant BUYER_PK = 0xB0B;
    uint256 constant SELLER_PK = 0xA11CE;
    uint256 constant STRANGER_PK = 0xDEAD;

    address buyer;
    address seller;
    address stranger;

    MockERC20 tokenBuyer;
    MockERC20 tokenSeller;

    uint256 amountBuyer = 1000e18;
    uint256 amountSeller = 2000e18;
    uint256 deadline;

    YonodexEscrow escrow;

    // Must match the typehashes in YonodexEscrow.sol
    bytes32 constant SETTLE_TYPEHASH = keccak256(
        "Settle(address buyer,address seller,address tokenBuyer,address tokenSeller,uint256 amountBuyer,uint256 amountSeller,uint256 deadline,uint256 nonce)"
    );
    bytes32 constant CANCEL_TYPEHASH = keccak256(
        "Cancel(address buyer,address seller,address tokenBuyer,address tokenSeller,uint256 amountBuyer,uint256 amountSeller,uint256 deadline,uint256 nonce)"
    );

    function setUp() public {
        buyer = vm.addr(BUYER_PK);
        seller = vm.addr(SELLER_PK);
        stranger = vm.addr(STRANGER_PK);

        tokenBuyer = new MockERC20("Token A", "TKNA");
        tokenSeller = new MockERC20("Token B", "TKNB");

        // Give both parties enough tokens
        tokenBuyer.mint(buyer, amountBuyer * 10);
        tokenSeller.mint(seller, amountSeller * 10);

        deadline = block.timestamp + 1 days;

        escrow = new YonodexEscrow(
            buyer,
            seller,
            address(tokenBuyer),
            address(tokenSeller),
            amountBuyer,
            amountSeller,
            deadline,
            address(0)
        );

        // Approve escrow to pull tokens
        vm.prank(buyer);
        tokenBuyer.approve(address(escrow), type(uint256).max);

        vm.prank(seller);
        tokenSeller.approve(address(escrow), type(uint256).max);
    }

    // ---- Signature helpers ----

    function _signDigest(uint256 pk, bytes32 typehash) internal view returns (bytes memory) {
        bytes32 structHash = keccak256(
            abi.encode(
                typehash,
                buyer,
                seller,
                address(tokenBuyer),
                address(tokenSeller),
                amountBuyer,
                amountSeller,
                deadline,
                uint256(0)
            )
        );
        bytes32 digest = keccak256(
            abi.encodePacked("\x19\x01", escrow.DOMAIN_SEPARATOR(), structHash)
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk, digest);
        return abi.encodePacked(r, s, v);
    }

    function _settleSigs() internal view returns (bytes memory buyerSig, bytes memory sellerSig) {
        buyerSig = _signDigest(BUYER_PK, SETTLE_TYPEHASH);
        sellerSig = _signDigest(SELLER_PK, SETTLE_TYPEHASH);
    }

    function _cancelSigs() internal view returns (bytes memory buyerSig, bytes memory sellerSig) {
        buyerSig = _signDigest(BUYER_PK, CANCEL_TYPEHASH);
        sellerSig = _signDigest(SELLER_PK, CANCEL_TYPEHASH);
    }

    function _fund() internal {
        vm.prank(buyer);
        escrow.deposit();
        vm.prank(seller);
        escrow.deposit();
    }

    // ---- Constructor tests ----

    function test_constructor_sets_immutables() public view {
        assertEq(escrow.buyer(), buyer);
        assertEq(escrow.seller(), seller);
        assertEq(escrow.tokenBuyer(), address(tokenBuyer));
        assertEq(escrow.tokenSeller(), address(tokenSeller));
        assertEq(escrow.amountBuyer(), amountBuyer);
        assertEq(escrow.amountSeller(), amountSeller);
        assertEq(escrow.deadline(), deadline);
        assertEq(escrow.arbitrationAddress(), address(0));
    }

    function test_constructor_reverts_zero_buyer() public {
        vm.expectRevert("escrow: buyer zero");
        new YonodexEscrow(address(0), seller, address(tokenBuyer), address(tokenSeller), amountBuyer, amountSeller, deadline, address(0));
    }

    function test_constructor_reverts_same_party() public {
        vm.expectRevert("escrow: same party");
        new YonodexEscrow(buyer, buyer, address(tokenBuyer), address(tokenSeller), amountBuyer, amountSeller, deadline, address(0));
    }

    function test_constructor_reverts_deadline_too_soon() public {
        vm.expectRevert("escrow: deadline too soon");
        new YonodexEscrow(buyer, seller, address(tokenBuyer), address(tokenSeller), amountBuyer, amountSeller, block.timestamp + 30 minutes, address(0));
    }

    function test_constructor_reverts_deadline_too_far() public {
        vm.expectRevert("escrow: deadline too far");
        new YonodexEscrow(buyer, seller, address(tokenBuyer), address(tokenSeller), amountBuyer, amountSeller, block.timestamp + 30 days, address(0));
    }

    // ---- Deposit tests ----

    function test_deposit_buyer_works() public {
        vm.prank(buyer);
        escrow.deposit();
        assertEq(escrow.depositedByBuyer(), amountBuyer);
        assertEq(tokenBuyer.balanceOf(address(escrow)), amountBuyer);
    }

    function test_deposit_seller_works() public {
        vm.prank(seller);
        escrow.deposit();
        assertEq(escrow.depositedBySeller(), amountSeller);
        assertEq(tokenSeller.balanceOf(address(escrow)), amountSeller);
    }

    function test_deposit_both_funded() public {
        _fund();
        assertEq(escrow.depositedByBuyer(), amountBuyer);
        assertEq(escrow.depositedBySeller(), amountSeller);
    }

    function test_deposit_double_reverts() public {
        vm.prank(buyer);
        escrow.deposit();
        vm.prank(buyer);
        vm.expectRevert("escrow: buyer already deposited");
        escrow.deposit();
    }

    function test_deposit_nonparty_reverts() public {
        vm.prank(stranger);
        vm.expectRevert("escrow: not a party");
        escrow.deposit();
    }

    // ---- Settle tests ----

    function test_settle_works() public {
        _fund();
        (bytes memory buyerSig, bytes memory sellerSig) = _settleSigs();
        escrow.settle(buyerSig, sellerSig);

        assertTrue(escrow.settled());
        // Buyer deposited tokenBuyer, seller deposited tokenSeller — on settle,
        // buyer receives tokenSeller, seller receives tokenBuyer.
        assertEq(tokenSeller.balanceOf(buyer), amountSeller);
        assertEq(tokenBuyer.balanceOf(seller), amountBuyer);
    }

    function test_settle_before_funded_reverts() public {
        vm.prank(buyer);
        escrow.deposit();
        (bytes memory buyerSig, bytes memory sellerSig) = _settleSigs();
        vm.expectRevert("escrow: not funded");
        escrow.settle(buyerSig, sellerSig);
    }

    function test_settle_bad_buyer_sig_reverts() public {
        _fund();
        (, bytes memory sellerSig) = _settleSigs();
        // Sign with wrong key
        bytes memory badBuyerSig = _signDigest(STRANGER_PK, SETTLE_TYPEHASH);
        vm.expectRevert("escrow: bad signature");
        escrow.settle(badBuyerSig, sellerSig);
    }

    function test_settle_bad_seller_sig_reverts() public {
        _fund();
        (bytes memory buyerSig, ) = _settleSigs();
        bytes memory badSellerSig = _signDigest(STRANGER_PK, SETTLE_TYPEHASH);
        vm.expectRevert("escrow: bad signature");
        escrow.settle(buyerSig, badSellerSig);
    }

    function test_settle_twice_reverts() public {
        _fund();
        (bytes memory buyerSig, bytes memory sellerSig) = _settleSigs();
        escrow.settle(buyerSig, sellerSig);
        vm.expectRevert("escrow: finalized");
        escrow.settle(buyerSig, sellerSig);
    }

    // ---- Cancel tests ----

    function test_cancel_returns_deposits() public {
        _fund();
        uint256 buyerBalBefore = tokenBuyer.balanceOf(buyer);
        uint256 sellerBalBefore = tokenSeller.balanceOf(seller);

        (bytes memory buyerSig, bytes memory sellerSig) = _cancelSigs();
        escrow.cancel(buyerSig, sellerSig);

        assertTrue(escrow.cancelled());
        assertEq(tokenBuyer.balanceOf(buyer), buyerBalBefore + amountBuyer);
        assertEq(tokenSeller.balanceOf(seller), sellerBalBefore + amountSeller);
    }

    function test_cancel_then_settle_reverts() public {
        _fund();
        (bytes memory buyerSig, bytes memory sellerSig) = _cancelSigs();
        escrow.cancel(buyerSig, sellerSig);

        (bytes memory settleBuyerSig, bytes memory settleSellerSig) = _settleSigs();
        vm.expectRevert("escrow: finalized");
        escrow.settle(settleBuyerSig, settleSellerSig);
    }

    // ---- Partial refund tests ----

    function test_refundPartial_buyer() public {
        vm.prank(buyer);
        escrow.deposit();
        uint256 balBefore = tokenBuyer.balanceOf(buyer);

        vm.prank(buyer);
        escrow.refundPartial();

        assertEq(tokenBuyer.balanceOf(buyer), balBefore + amountBuyer);
        assertEq(escrow.depositedByBuyer(), 0);
    }

    function test_refundPartial_seller() public {
        vm.prank(seller);
        escrow.deposit();
        uint256 balBefore = tokenSeller.balanceOf(seller);

        vm.prank(seller);
        escrow.refundPartial();

        assertEq(tokenSeller.balanceOf(seller), balBefore + amountSeller);
        assertEq(escrow.depositedBySeller(), 0);
    }

    function test_refundPartial_when_funded_reverts() public {
        _fund();
        vm.prank(buyer);
        vm.expectRevert("escrow: both funded, use cancel/settle/refund");
        escrow.refundPartial();
    }

    function test_refundPartial_nonparty_reverts() public {
        vm.prank(stranger);
        vm.expectRevert("escrow: not a party");
        escrow.refundPartial();
    }

    // ---- Post-deadline refund tests ----

    function test_refund_after_deadline() public {
        _fund();
        uint256 buyerBalBefore = tokenBuyer.balanceOf(buyer);
        uint256 sellerBalBefore = tokenSeller.balanceOf(seller);

        vm.warp(deadline + 1);
        escrow.refund();

        assertEq(tokenBuyer.balanceOf(buyer), buyerBalBefore + amountBuyer);
        assertEq(tokenSeller.balanceOf(seller), sellerBalBefore + amountSeller);
    }

    function test_refund_before_deadline_reverts() public {
        _fund();
        vm.expectRevert("escrow: not expired");
        escrow.refund();
    }

    function test_refund_callable_by_anyone() public {
        _fund();
        vm.warp(deadline + 1);
        // Stranger triggers the refund — assets still go to depositors
        vm.prank(stranger);
        escrow.refund();

        assertEq(escrow.depositedByBuyer(), 0);
        assertEq(escrow.depositedBySeller(), 0);
    }

    // ---- Cross-instance signature binding ----

    function test_signature_from_other_escrow_rejected() public {
        _fund();

        // Deploy a second escrow with different params
        YonodexEscrow escrow2 = new YonodexEscrow(
            buyer,
            seller,
            address(tokenBuyer),
            address(tokenSeller),
            amountBuyer,
            amountSeller + 1,   // different amount → different address → different domain
            deadline,
            address(0)
        );

        // Sign for escrow2
        bytes32 structHash = keccak256(
            abi.encode(
                SETTLE_TYPEHASH,
                buyer,
                seller,
                address(tokenBuyer),
                address(tokenSeller),
                amountBuyer,
                amountSeller + 1,
                deadline,
                uint256(0)
            )
        );
        bytes32 digest = keccak256(
            abi.encodePacked("\x19\x01", escrow2.DOMAIN_SEPARATOR(), structHash)
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(BUYER_PK, digest);
        bytes memory buyerSig = abi.encodePacked(r, s, v);

        // Same signature, wrong escrow → rejected
        (, bytes memory sellerSig) = _settleSigs();
        vm.expectRevert("escrow: bad signature");
        escrow.settle(buyerSig, sellerSig);
    }
}
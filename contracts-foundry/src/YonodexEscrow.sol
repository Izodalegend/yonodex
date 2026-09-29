// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

// Yonodex Escrow — Whitepaper Layer 7, Section 9.3
//
// One escrow per trade. 2-of-2 multisig. Non-custodial.
// Assets only flow to:
//   - Counterparty on successful settle (both signatures)
//   - Original depositor on refund
//   - Original depositor on cooperative cancel
//
// No admin. No owner. No upgrade. No pause. No rescue.
// All parameters immutable at deploy time.

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

contract YonodexEscrow is ReentrancyGuard {
    using SafeERC20 for IERC20;

    // ---- Immutable trade parameters ----
    address public immutable buyer;
    address public immutable seller;

    address public immutable tokenBuyer;   // token buyer deposits
    address public immutable tokenSeller;  // token seller deposits

    uint256 public immutable amountBuyer;
    uint256 public immutable amountSeller;

    uint256 public immutable deadline;

    // Immutable arbitration hook. address(0) = no arbitration (v1 default).
    // A future DAO can deploy escrows with this set to the DAO address.
    // Immutable by design — a mutable arbitration address is a backdoor.
    address public immutable arbitrationAddress;

    // EIP-712 domain separator — bound to this contract + chain.
    // Signatures only valid for this specific escrow instance.
    bytes32 public immutable DOMAIN_SEPARATOR;

    bytes32 private constant SETTLE_TYPEHASH = keccak256(
        "Settle(address buyer,address seller,address tokenBuyer,address tokenSeller,uint256 amountBuyer,uint256 amountSeller,uint256 deadline,uint256 nonce)"
    );
    bytes32 private constant CANCEL_TYPEHASH = keccak256(
        "Cancel(address buyer,address seller,address tokenBuyer,address tokenSeller,uint256 amountBuyer,uint256 amountSeller,uint256 deadline,uint256 nonce)"
    );

    // ---- Mutable state ----
    uint256 public depositedByBuyer;
    uint256 public depositedBySeller;

    bool public settled;
    bool public cancelled;

    // ---- Events (for indexer / Phase 4 subgraph) ----
    event Deposited(address indexed depositor, uint256 amount);
    event Funded();
    event Settled();
    event Cancelled();
    event Refunded(address indexed caller, uint256 buyerAmount, uint256 sellerAmount);

    // ---- Modifiers ----
    modifier notFinalized() {
        require(!settled && !cancelled, "escrow: finalized");
        _;
    }

    modifier onlyParties() {
        require(msg.sender == buyer || msg.sender == seller, "escrow: not a party");
        _;
    }

    // ---- Constructor ----
    constructor(
        address _buyer,
        address _seller,
        address _tokenBuyer,
        address _tokenSeller,
        uint256 _amountBuyer,
        uint256 _amountSeller,
        uint256 _deadline,
        address _arbitrationAddress
    ) {
        require(_buyer != address(0), "escrow: buyer zero");
        require(_seller != address(0), "escrow: seller zero");
        require(_buyer != _seller, "escrow: same party");
        require(_tokenBuyer != address(0), "escrow: tokenBuyer zero");
        require(_tokenSeller != address(0), "escrow: tokenSeller zero");
        require(_amountBuyer > 0, "escrow: amountBuyer zero");
        require(_amountSeller > 0, "escrow: amountSeller zero");
        require(_deadline >= block.timestamp + 1 hours, "escrow: deadline too soon");
        require(_deadline <= block.timestamp + 14 days, "escrow: deadline too far");

        // Note: _arbitrationAddress == address(0) is intentionally allowed.
        // Zero means "no arbitration" — the v1 default for escrows deployed
        // before the DAO exists. A zero-check here would break that design.
        // forge-lint: disable-next-line(missing-zero-check)

        buyer = _buyer;
        seller = _seller;
        tokenBuyer = _tokenBuyer;
        tokenSeller = _tokenSeller;
        amountBuyer = _amountBuyer;
        amountSeller = _amountSeller;
        deadline = _deadline;
        arbitrationAddress = _arbitrationAddress;

        DOMAIN_SEPARATOR = keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256("YonodexEscrow"),
                keccak256("1"),
                block.chainid,
                address(this)
            )
        );
    }

    // ---- Deposit ----
    // Each party deposits exactly once. Exact-amount enforcement.
    function deposit() external nonReentrant notFinalized {
        if (msg.sender == buyer) {
            require(depositedByBuyer == 0, "escrow: buyer already deposited");
            _pullExact(tokenBuyer, msg.sender, amountBuyer);
            depositedByBuyer = amountBuyer;
            emit Deposited(buyer, amountBuyer);
        } else if (msg.sender == seller) {
            require(depositedBySeller == 0, "escrow: seller already deposited");
            _pullExact(tokenSeller, msg.sender, amountSeller);
            depositedBySeller = amountSeller;
            emit Deposited(seller, amountSeller);
        } else {
            revert("escrow: not a party");
        }

        if (depositedByBuyer > 0 && depositedBySeller > 0) {
            emit Funded();
        }
    }

    // ---- Settlement ----
    // Both parties sign a Settle digest. Anyone can submit.
    function settle(bytes calldata buyerSig, bytes calldata sellerSig) external nonReentrant notFinalized {
        require(depositedByBuyer > 0 && depositedBySeller > 0, "escrow: not funded");

        bytes32 digest = _hashTyped(SETTLE_TYPEHASH);
        _verify(digest, buyer, buyerSig);
        _verify(digest, seller, sellerSig);

        settled = true;
		// forge-lint: disable-next-line(reentrancy-events)
        emit Settled();

        // CEI: state updated + event emitted before transfers
        IERC20(tokenBuyer).safeTransfer(seller, amountBuyer);
        IERC20(tokenSeller).safeTransfer(buyer, amountSeller);
    }

    // ---- Cooperative cancel ----
    // Both parties sign a Cancel digest. Anyone can submit.
    // Returns each deposit to its depositor.
    function cancel(bytes calldata buyerSig, bytes calldata sellerSig) external nonReentrant notFinalized {
        require(depositedByBuyer > 0 || depositedBySeller > 0, "escrow: nothing deposited");

        bytes32 digest = _hashTyped(CANCEL_TYPEHASH);
        _verify(digest, buyer, buyerSig);
        _verify(digest, seller, sellerSig);

        cancelled = true;
		// forge-lint: disable-next-line(reentrancy-events)
        emit Cancelled();

        _refundBoth();
    }

    // ---- Partial-funding refund ----
    // While only one side has deposited, that depositor can pull their own deposit.
    // Symmetry preserved: you can only ever get back what you put in.
    function refundPartial() external nonReentrant notFinalized {
        if (msg.sender == buyer) {
            require(depositedByBuyer > 0, "escrow: buyer nothing deposited");
            require(depositedBySeller == 0, "escrow: both funded, use cancel/settle/refund");
            uint256 amount = depositedByBuyer;
            depositedByBuyer = 0;
            IERC20(tokenBuyer).safeTransfer(buyer, amount);
            emit Refunded(buyer, amount, 0);
        } else if (msg.sender == seller) {
            require(depositedBySeller > 0, "escrow: seller nothing deposited");
            require(depositedByBuyer == 0, "escrow: both funded, use cancel/settle/refund");
            uint256 amount = depositedBySeller;
            depositedBySeller = 0;
            IERC20(tokenSeller).safeTransfer(seller, amount);
            emit Refunded(seller, 0, amount);
        } else {
            revert("escrow: not a party");
        }
    }

    // ---- Post-deadline refund ----
    // Anyone can call after deadline. Funds go to depositors (deterministic).
    function refund() external nonReentrant notFinalized {
        require(block.timestamp >= deadline, "escrow: not expired");
        require(depositedByBuyer > 0 || depositedBySeller > 0, "escrow: nothing deposited");

        uint256 buyerAmount = depositedByBuyer;
        uint256 sellerAmount = depositedBySeller;

        // Anyone can trigger, but assets only go to original depositors.
        _refundBoth();
        emit Refunded(msg.sender, buyerAmount, sellerAmount);
    }

    // ---- Internal helpers ----

    function _refundBoth() private {
        uint256 buyerAmount = depositedByBuyer;
        uint256 sellerAmount = depositedBySeller;

        depositedByBuyer = 0;
        depositedBySeller = 0;

        if (buyerAmount > 0) {
            IERC20(tokenBuyer).safeTransfer(buyer, buyerAmount);
        }
        if (sellerAmount > 0) {
            IERC20(tokenSeller).safeTransfer(seller, sellerAmount);
        }
    }

    function _pullExact(address token, address from, uint256 amount) private {
        IERC20 t = IERC20(token);
        uint256 before = t.balanceOf(address(this));
        t.safeTransferFrom(from, address(this), amount);
        uint256 afterBal = t.balanceOf(address(this));
        // Rejects fee-on-transfer and rebasing tokens — exact-amount enforcement.
        require(afterBal - before == amount, "escrow: amount mismatch");
    }

    function _hashTyped(bytes32 typehash) private view returns (bytes32) {
        return keccak256(
            abi.encode(
                typehash,
                buyer,
                seller,
                tokenBuyer,
                tokenSeller,
                amountBuyer,
                amountSeller,
                deadline,
                uint256(0) // nonce — placeholder for v2, always 0 in v1
            )
        );
    }

    function _verify(bytes32 digest, address expected, bytes calldata sig) private view {
        // EIP-712 prefix + domain separator
        bytes32 ethSigned = keccak256(abi.encodePacked("\x19\x01", DOMAIN_SEPARATOR, digest));
        // ECDSA.recover reverts on malleable signatures and address(0) recovery
        address recovered = ECDSA.recover(ethSigned, sig);
        require(recovered == expected, "escrow: bad signature");
    }
}
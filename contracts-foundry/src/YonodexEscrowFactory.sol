// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

// Yonodex Escrow Factory — Whitepaper Layer 7, Section 9.3
//
// Deploys YonodexEscrow instances via CREATE2 so both parties can
// precompute the escrow address off-chain before either deposits.
//
// The pause check lives here (not in the escrow itself) — one place
// for the Guardian to gate new trade creation. Existing escrows are
// unaffected and continue to settle/refund.
//
// No admin. No upgrade. The only privileged action is the Guardian's
// pause flag, and that's a separate contract.

import {YonodexEscrow} from "./YonodexEscrow.sol";

interface IYonodexGuardian {
    function escrowCreationPaused() external view returns (bool);
}

contract YonodexEscrowFactory {
    // Immutable Guardian reference. address(0) means "no Guardian yet"
    // (v1 default) — pause check skipped.
    address public immutable guardian;

    // Monotonic counter for salt uniqueness. Two identical trade
    // proposals must resolve to different addresses.
    uint256 public nonce;

    event EscrowCreated(
        address indexed escrow,
        address indexed buyer,
        address indexed seller,
        bytes32 salt
    );

    // _guardian == address(0) is intentionally allowed (no Guardian in v1).
    // forge-lint: disable-next-line(missing-zero-check)
    constructor(address _guardian) {
        guardian = _guardian;
    }

    /// Deploy a new escrow. Callable by anyone (both parties can create).
    function createEscrow(
        address buyer,
        address seller,
        address tokenBuyer,
        address tokenSeller,
        uint256 amountBuyer,
        uint256 amountSeller,
        uint256 deadline,
        address arbitrationAddress
    ) external returns (address escrow) {
        // Guardian pause gate — one place, checked at creation only
        if (guardian != address(0)) {
            require(
                !IYonodexGuardian(guardian).escrowCreationPaused(),
                "factory: paused"
            );
        }

        bytes32 salt = _computeSalt(
            msg.sender,
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            arbitrationAddress,
            nonce
        );
        nonce++;

        escrow = address(
            new YonodexEscrow{salt: salt}(
                buyer,
                seller,
                tokenBuyer,
                tokenSeller,
                amountBuyer,
                amountSeller,
                deadline,
                arbitrationAddress
            )
        );

        // forge-lint: disable-next-line(reentrancy-events)
        emit EscrowCreated(escrow, buyer, seller, salt);
    }

    /// Precompute the address the next createEscrow call will deploy to.
    /// Both parties can call this off-chain to verify the address before
    /// depositing. If they compute different addresses, something is wrong.
    function computeEscrowAddress(
        address caller,
        address buyer,
        address seller,
        address tokenBuyer,
        address tokenSeller,
        uint256 amountBuyer,
        uint256 amountSeller,
        uint256 deadline,
        address arbitrationAddress,
        uint256 nonceOverride
    ) external view returns (address) {
        bytes32 salt = _computeSalt(
            caller,
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            arbitrationAddress,
            nonceOverride
        );
        bytes32 initCodeHash = _initCodeHash(
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            arbitrationAddress
        );
        return _computeCreate2Address(salt, initCodeHash);
    }

    /// Convenience overload — uses the current nonce.
    function computeNextEscrowAddress(
        address caller,
        address buyer,
        address seller,
        address tokenBuyer,
        address tokenSeller,
        uint256 amountBuyer,
        uint256 amountSeller,
        uint256 deadline,
        address arbitrationAddress
    ) external view returns (address) {
        bytes32 salt = _computeSalt(
            caller,
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            arbitrationAddress,
            nonce
        );
        bytes32 initCodeHash = _initCodeHash(
            buyer,
            seller,
            tokenBuyer,
            tokenSeller,
            amountBuyer,
            amountSeller,
            deadline,
            arbitrationAddress
        );
        return _computeCreate2Address(salt, initCodeHash);
    }

    // ---- Internal ----

    function _initCodeHash(
        address buyer,
        address seller,
        address tokenBuyer,
        address tokenSeller,
        uint256 amountBuyer,
        uint256 amountSeller,
        uint256 deadline,
        address arbitrationAddress
    ) internal pure returns (bytes32) {
        // bytes.concat instead of abi.encodePacked — no packed-collision risk
        bytes memory code = bytes.concat(
            type(YonodexEscrow).creationCode,
            abi.encode(
                buyer,
                seller,
                tokenBuyer,
                tokenSeller,
                amountBuyer,
                amountSeller,
                deadline,
                arbitrationAddress
            )
        );
        return keccak256(code);
    }

    function _computeCreate2Address(
        bytes32 salt,
        bytes32 initCodeHash
    ) internal view returns (address) {
        bytes32 hash = keccak256(
            abi.encodePacked(
                bytes1(0xff),
                address(this),
                salt,
                initCodeHash
            )
        );
        return address(uint160(uint256(hash)));
    }

    function _computeSalt(
        address caller,
        address buyer,
        address seller,
        address tokenBuyer,
        address tokenSeller,
        uint256 amountBuyer,
        uint256 amountSeller,
        uint256 deadline,
        address arbitrationAddress,
        uint256 nonceValue
    ) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    caller,
                    buyer,
                    seller,
                    tokenBuyer,
                    tokenSeller,
                    amountBuyer,
                    amountSeller,
                    deadline,
                    arbitrationAddress,
                    nonceValue
                )
            );
    }
}
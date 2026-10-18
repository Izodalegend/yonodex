// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

// Yonodex Emergency Guardian — Whitepaper Layer 9, Section 9.5
//
// A safety valve to pause new escrow creation in case of a critical
// vulnerability discovery. Existing escrows are NOT affected and
// continue to settle/refund normally.
//
// In production, the `owner` will be a 4-of-7 Gnosis Safe multisig
// consisting of publicly known security experts.

contract YonodexGuardian {
    address public owner;
    bool public escrowCreationPaused;

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event EscrowCreationPaused(address indexed by);
    event EscrowCreationUnpaused(address indexed by);

    error Unauthorized();
    error InvalidOwner();

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    constructor(address _owner) {
        if (_owner == address(0)) revert InvalidOwner();
        owner = _owner;
        emit OwnershipTransferred(address(0), _owner);
    }

    /// @notice Pause the creation of new escrows (Emergency only)
    function pauseEscrowCreation() external onlyOwner {
        escrowCreationPaused = true;
        emit EscrowCreationPaused(msg.sender);
    }

    /// @notice Unpause the creation of new escrows
    function unpauseEscrowCreation() external onlyOwner {
        escrowCreationPaused = false;
        emit EscrowCreationUnpaused(msg.sender);
    }

    /// @notice Transfer ownership to a new multisig or address
    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert InvalidOwner();
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }
}
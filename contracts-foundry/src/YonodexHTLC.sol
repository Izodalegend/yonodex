// SPDX-License-Identifier: AGPL-3.0
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract YonodexHTLC is ReentrancyGuard {
    using SafeERC20 for IERC20;

    struct Swap {
        address sender;
        address receiver;
        address token;       // address(0) for native ETH
        uint256 amount;
        bytes32 hashlock;    // keccak256(preimage)
        uint256 timelock;    // Timestamp when refund is allowed
        bool redeemed;
        bool refunded;
    }

    mapping(bytes32 => Swap) public swaps;

    event SwapCreated(bytes32 indexed swapId, address indexed sender, address indexed receiver, address token, uint256 amount, bytes32 hashlock, uint256 timelock);
    event SwapRedeemed(bytes32 indexed swapId, bytes32 preimage);
    event SwapRefunded(bytes32 indexed swapId);

    error InvalidTimelock();
    error InvalidAmount();
    error SwapExists();
    error SwapNotFound();
    error SwapExpired();
    error SwapNotExpired();
    error InvalidPreimage();
    error AlreadyRedeemedOrRefunded();
    error Unauthorized();

    modifier swapExists(bytes32 swapId) {
        if (swaps[swapId].sender == address(0)) revert SwapNotFound();
        _;
    }

    function createSwap(
        bytes32 swapId,
        address receiver,
        address token,
        uint256 amount,
        bytes32 hashlock,
        uint256 timelock
    ) external payable nonReentrant {
        if (timelock <= block.timestamp) revert InvalidTimelock();
        if (amount == 0) revert InvalidAmount();
        if (swaps[swapId].sender != address(0)) revert SwapExists();

        if (token == address(0)) {
            if (msg.value != amount) revert InvalidAmount();
        } else {
            if (msg.value != 0) revert InvalidAmount();
            IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        }

        swaps[swapId] = Swap({
            sender: msg.sender,
            receiver: receiver,
            token: token,
            amount: amount,
            hashlock: hashlock,
            timelock: timelock,
            redeemed: false,
            refunded: false
        });

        emit SwapCreated(swapId, msg.sender, receiver, token, amount, hashlock, timelock);
    }

    function redeem(bytes32 swapId, bytes32 preimage) external nonReentrant swapExists(swapId) {
        Swap storage s = swaps[swapId];
        if (s.redeemed || s.refunded) revert AlreadyRedeemedOrRefunded();
        if (keccak256(abi.encodePacked(preimage)) != s.hashlock) revert InvalidPreimage();

        s.redeemed = true;

        if (s.token == address(0)) {
            (bool success, ) = payable(s.receiver).call{value: s.amount}("");
            require(success, "ETH transfer failed");
        } else {
            IERC20(s.token).safeTransfer(s.receiver, s.amount);
        }

        emit SwapRedeemed(swapId, preimage);
    }

    function refund(bytes32 swapId) external nonReentrant swapExists(swapId) {
        Swap storage s = swaps[swapId];
        if (s.redeemed || s.refunded) revert AlreadyRedeemedOrRefunded();
        if (block.timestamp < s.timelock) revert SwapNotExpired();

        s.refunded = true;

        if (s.token == address(0)) {
            (bool success, ) = payable(s.sender).call{value: s.amount}("");
            require(success, "ETH transfer failed");
        } else {
            IERC20(s.token).safeTransfer(s.sender, s.amount);
        }

        emit SwapRefunded(swapId);
    }
}
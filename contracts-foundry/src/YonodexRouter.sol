// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

interface IYonodexPool {
    function swap(address tokenIn, address tokenOut, uint256 amountIn, address to) external returns (uint256 amountOut);
}

// Yonodex Smart Router — Whitepaper Layer 7, Section 9.2
// Handles multi-hop and split routing to minimize slippage.
contract YonodexRouter is ReentrancyGuard {
    using SafeERC20 for IERC20;

    struct Hop {
        address pool;
        address tokenIn;
        address tokenOut;
    }

    struct SplitRoute {
        uint256 amountIn;
        Hop[] hops;
    }

    error DeadlineExpired();
    error SlippageExceeded();
    error InvalidRoute();

    /// @notice Executes a split and/or multi-hop swap
    /// @param routes The array of split routes (each containing its own hops)
    /// @param minAmountOut The minimum acceptable output amount (slippage protection)
    /// @param deadline The timestamp by which the swap must execute
    /// @param recipient The address receiving the final tokens
    /// @param tokenIn The initial token being swapped (must match routes[0].hops[0].tokenIn)
    /// @param tokenOut The final token expected (must match routes[i].hops[last].tokenOut)
    function swap(
        SplitRoute[] calldata routes,
        address tokenIn,
        address tokenOut,
        uint256 totalAmountIn,
        uint256 minAmountOut,
        uint256 deadline,
        address recipient
    ) external nonReentrant returns (uint256 finalAmountOut) {
        if (block.timestamp > deadline) revert DeadlineExpired();
        if (routes.length == 0) revert InvalidRoute();

        // 1. Pull initial funds from user
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), totalAmountIn);

        // 2. Execute each split route
        for (uint256 i = 0; i < routes.length; i++) {
            SplitRoute calldata route = routes[i];
            if (route.hops.length == 0) revert InvalidRoute();
            
            uint256 currentAmount = route.amountIn;

            for (uint256 j = 0; j < route.hops.length; j++) {
                Hop calldata hop = route.hops[j];
                
                // Approve the pool to spend the tokens
                IERC20(hop.tokenIn).forceApprove(hop.pool, currentAmount);

                // Execute the swap on the pool
                currentAmount = IYonodexPool(hop.pool).swap(
                    hop.tokenIn,
                    hop.tokenOut,
                    currentAmount,
                    (j == route.hops.length - 1) ? address(this) : address(this) // Keep funds in router until the end
                );
            }

            finalAmountOut += currentAmount;
        }

        // 3. Slippage check
        if (finalAmountOut < minAmountOut) revert SlippageExceeded();

        // 4. Send final tokens to recipient
        IERC20(tokenOut).safeTransfer(recipient, finalAmountOut);
    }
}
// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexRouter, IYonodexPool} from "../src/YonodexRouter.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

contract MockToken is ERC20 {
    constructor(string memory name, string memory symbol) ERC20(name, symbol) {}
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }
}

// A simple 1:1 swap pool for testing the router
contract MockPool is IYonodexPool {
    using SafeERC20 for IERC20;

    function swap(address tokenIn, address tokenOut, uint256 amountIn, address to) external returns (uint256 amountOut) {
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);
        amountOut = amountIn; // 1:1 ratio
        IERC20(tokenOut).safeTransfer(to, amountOut);
        return amountOut;
    }
}

contract YonodexRouterTest is Test {
    YonodexRouter router;
    MockToken tokenA;
    MockToken tokenB;
    MockToken tokenC;

    MockPool poolAB1;
    MockPool poolAB2;
    MockPool poolBC;

    address user = address(0xBEEF);
    address recipient = address(0xCAFE);

    uint256 constant AMOUNT = 100 ether;
    uint256 deadline;

    function setUp() public {
        router = new YonodexRouter();
        tokenA = new MockToken("Token A", "TKA");
        tokenB = new MockToken("Token B", "TKB");
        tokenC = new MockToken("Token C", "TKC");

        poolAB1 = new MockPool();
        poolAB2 = new MockPool();
        poolBC = new MockPool();

        // Fund pools with liquidity
        tokenB.mint(address(poolAB1), 1000 ether);
        tokenB.mint(address(poolAB2), 1000 ether);
        tokenC.mint(address(poolBC), 1000 ether);

        // Fund user and approve router
        tokenA.mint(user, 1000 ether);
        vm.prank(user);
        tokenA.approve(address(router), type(uint256).max);

        deadline = block.timestamp + 1 hours;
    }

    // ---- Single Hop (A -> B) ----

    function test_swap_single_hop() public {
        YonodexRouter.Hop[] memory hops = new YonodexRouter.Hop[](1);
        hops[0] = YonodexRouter.Hop(address(poolAB1), address(tokenA), address(tokenB));

        YonodexRouter.SplitRoute[] memory routes = new YonodexRouter.SplitRoute[](1);
        routes[0] = YonodexRouter.SplitRoute(AMOUNT, hops);

        vm.prank(user);
        router.swap(routes, address(tokenA), address(tokenB), AMOUNT, AMOUNT, deadline, recipient);

        assertEq(tokenB.balanceOf(recipient), AMOUNT);
    }

    // ---- Multi-Hop (A -> B -> C) ----

    function test_swap_multi_hop() public {
        YonodexRouter.Hop[] memory hops = new YonodexRouter.Hop[](2);
        hops[0] = YonodexRouter.Hop(address(poolAB1), address(tokenA), address(tokenB));
        hops[1] = YonodexRouter.Hop(address(poolBC), address(tokenB), address(tokenC));

        YonodexRouter.SplitRoute[] memory routes = new YonodexRouter.SplitRoute[](1);
        routes[0] = YonodexRouter.SplitRoute(AMOUNT, hops);

        vm.prank(user);
        router.swap(routes, address(tokenA), address(tokenC), AMOUNT, AMOUNT, deadline, recipient);

        assertEq(tokenC.balanceOf(recipient), AMOUNT);
    }

    // ---- Split Route (50% via pool 1, 50% via pool 2) ----

    function test_swap_split_route() public {
        YonodexRouter.Hop[] memory hops1 = new YonodexRouter.Hop[](1);
        hops1[0] = YonodexRouter.Hop(address(poolAB1), address(tokenA), address(tokenB));

        YonodexRouter.Hop[] memory hops2 = new YonodexRouter.Hop[](1);
        hops2[0] = YonodexRouter.Hop(address(poolAB2), address(tokenA), address(tokenB));

        YonodexRouter.SplitRoute[] memory routes = new YonodexRouter.SplitRoute[](2);
        routes[0] = YonodexRouter.SplitRoute(AMOUNT / 2, hops1);
        routes[1] = YonodexRouter.SplitRoute(AMOUNT / 2, hops2);

        vm.prank(user);
        router.swap(routes, address(tokenA), address(tokenB), AMOUNT, AMOUNT, deadline, recipient);

        assertEq(tokenB.balanceOf(recipient), AMOUNT);
    }

    // ---- Failure Cases ----

    function test_revert_deadline() public {
        YonodexRouter.Hop[] memory hops = new YonodexRouter.Hop[](1);
        hops[0] = YonodexRouter.Hop(address(poolAB1), address(tokenA), address(tokenB));
        YonodexRouter.SplitRoute[] memory routes = new YonodexRouter.SplitRoute[](1);
        routes[0] = YonodexRouter.SplitRoute(AMOUNT, hops);

        vm.warp(deadline + 1);

        vm.prank(user);
        vm.expectRevert(YonodexRouter.DeadlineExpired.selector);
        router.swap(routes, address(tokenA), address(tokenB), AMOUNT, AMOUNT, deadline, recipient);
    }

    function test_revert_slippage() public {
        YonodexRouter.Hop[] memory hops = new YonodexRouter.Hop[](1);
        hops[0] = YonodexRouter.Hop(address(poolAB1), address(tokenA), address(tokenB));
        YonodexRouter.SplitRoute[] memory routes = new YonodexRouter.SplitRoute[](1);
        routes[0] = YonodexRouter.SplitRoute(AMOUNT, hops);

        // Demand more than the 1:1 pool can provide
        uint256 impossibleMinOut = AMOUNT + 1 ether;

        vm.prank(user);
        vm.expectRevert(YonodexRouter.SlippageExceeded.selector);
        router.swap(routes, address(tokenA), address(tokenB), AMOUNT, impossibleMinOut, deadline, recipient);
    }
}
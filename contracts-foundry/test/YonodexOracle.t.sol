// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "forge-std/Test.sol";
import {YonodexOracle, IPyth} from "../src/YonodexOracle.sol";
import {AggregatorV3Interface} from "../src/interfaces/AggregatorV3Interface.sol";

contract MockChainlink is AggregatorV3Interface {
    int256 public price;
    uint256 public updatedAt;
    uint8 public dec;

    constructor(uint8 _dec) { dec = _dec; }
    function setPrice(int256 _price, uint256 _updatedAt) external { price = _price; updatedAt = _updatedAt; }
    function decimals() external view returns (uint8) { return dec; }
    function description() external pure returns (string memory) { return "Mock"; }
    function version() external pure returns (uint256) { return 1; }
    function getRoundData(uint80) external view returns (uint80, int256, uint256, uint256, uint80) {
        return (1, price, updatedAt, updatedAt, 1);
    }
    function latestRoundData() external view returns (uint80, int256, uint256, uint256, uint80) {
        return (1, price, updatedAt, updatedAt, 1);
    }
}

contract MockPyth is IPyth {
    Price public mockPrice;
    function setPrice(int64 _price, int32 _expo, uint256 _publishTime) external {
        mockPrice = Price(_price, 0, _expo, _publishTime);
    }
    function getPriceUnsafe(bytes32) external view returns (Price memory) {
        return mockPrice;
    }
}

contract YonodexOracleTest is Test {
    YonodexOracle oracle;
    MockChainlink clFeed;
    MockPyth pythFeed;

    function setUp() public {
        clFeed = new MockChainlink(8); // Chainlink ETH/USD uses 8 decimals
        pythFeed = new MockPyth();
        oracle = new YonodexOracle(address(clFeed), address(pythFeed), bytes32("ETH_USD"));
        
        vm.warp(1000000); // Set a standard timestamp
    }

    function test_happyPath_normalizesAndAverages() public {
        // Chainlink: $2000.00000000 (8 decimals)
        clFeed.setPrice(2000 * 1e8, block.timestamp);
        
        // Pyth: $2000.000000 (expo -6)
        pythFeed.setPrice(2000 * 1e6, -6, block.timestamp);

        uint256 price = oracle.getLatestPrice();
        assertEq(price, 2000 * 1e18); // Both normalized to 18 decimals
    }

    function test_revert_staleChainlink() public {
        clFeed.setPrice(2000 * 1e8, block.timestamp - 2 hours); // Stale
        pythFeed.setPrice(2000 * 1e6, -6, block.timestamp);

        vm.expectRevert(YonodexOracle.StalePrice.selector);
        oracle.getLatestPrice();
    }

    function test_revert_divergence() public {
        clFeed.setPrice(2000 * 1e8, block.timestamp); // $2000
        pythFeed.setPrice(2050 * 1e6, -6, block.timestamp); // $2050 (2.5% difference)

        vm.expectRevert(abi.encodeWithSelector(YonodexOracle.PriceDivergenceTooHigh.selector, 2000e18, 2050e18));
        oracle.getLatestPrice();
    }
}
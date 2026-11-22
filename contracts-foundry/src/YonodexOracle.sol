// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.24;

import "./interfaces/AggregatorV3Interface.sol";

// Minimal interface for Pyth Network
interface IPyth {
    struct Price {
        int64 price;
        uint64 conf;
        int32 expo;
        uint256 publishTime;
    }
    function getPriceUnsafe(bytes32 id) external view returns (Price memory price);
}

contract YonodexOracle {
    AggregatorV3Interface public immutable chainlinkFeed;
    IPyth public immutable pythContract;
    bytes32 public immutable pythPriceId;

    uint256 public constant MAX_DIVERGENCE_BPS = 100; // 1% divergence threshold
    uint256 public constant MAX_STALENESS = 1 hours;

    error StalePrice();
    error PriceDivergenceTooHigh(uint256 clPrice, uint256 pythPrice);

    constructor(address _chainlinkFeed, address _pythContract, bytes32 _pythPriceId) {
        chainlinkFeed = AggregatorV3Interface(_chainlinkFeed);
        pythContract = IPyth(_pythContract);
        pythPriceId = _pythPriceId;
    }

    /// @notice Fetches the latest price from Chainlink and Pyth, verifies they align, and returns the median.
    /// @return price The price normalized to 18 decimals.
    function getLatestPrice() external view returns (uint256 price) {
        // 1. Fetch Chainlink Price
        (
            /* uint80 roundID */,
            int256 clPrice,
            /* uint256 startedAt */,
            uint256 clUpdatedAt,
            /* uint80 answeredInRound */
        ) = chainlinkFeed.latestRoundData();

        if (block.timestamp - clUpdatedAt > MAX_STALENESS) revert StalePrice();
        require(clPrice > 0, "Invalid Chainlink price");

        // Normalize Chainlink to 18 decimals
        uint8 clDecimals = chainlinkFeed.decimals();
        uint256 clPrice18 = uint256(clPrice) * (10 ** (18 - clDecimals));

        // 2. Fetch Pyth Price
        IPyth.Price memory pyPrice = pythContract.getPriceUnsafe(pythPriceId);
        
        if (block.timestamp - pyPrice.publishTime > MAX_STALENESS) revert StalePrice();
        require(pyPrice.price > 0, "Invalid Pyth price");

        // Normalize Pyth to 18 decimals
        // Pyth returns price as int64 and expo as int32.
        // Value = price * 10^expo. We want value * 10^18 = price * 10^(expo + 18).
        uint256 pythPrice18;
        int256 expo = int256(pyPrice.expo);
        int256 shift = 18 + expo;

        if (shift > 0) {
            pythPrice18 = uint256(uint64(pyPrice.price)) * (10 ** uint256(shift));
        } else if (shift < 0) {
            pythPrice18 = uint256(uint64(pyPrice.price)) / (10 ** uint256(-shift));
        } else {
            pythPrice18 = uint256(uint64(pyPrice.price));
        }

        // 3. Check Divergence
        uint256 diff = clPrice18 > pythPrice18 ? clPrice18 - pythPrice18 : pythPrice18 - clPrice18;
        uint256 maxPrice = clPrice18 > pythPrice18 ? clPrice18 : pythPrice18;
        
        uint256 divergenceBps = (diff * 10000) / maxPrice;
        if (divergenceBps > MAX_DIVERGENCE_BPS) {
            revert PriceDivergenceTooHigh(clPrice18, pythPrice18);
        }

        // 4. Return the median (or average) of the two
        return (clPrice18 + pythPrice18) / 2;
    }
}
use alloy_sol_types::sol;

sol! {
    interface IMulticall {
        function multicall(bytes[] calldata data) external payable returns (bytes[] memory results);
    }

    interface INonfungiblePositionManager {
        function createAndInitializePoolIfNecessary(
            address token0,
            address token1,
            uint24 fee,
            uint160 sqrtPriceX96
        ) external payable returns (address pool);

        struct MintParams {
            address token0;
            address token1;
            uint24 fee;
            int24 tickLower;
            int24 tickUpper;
            uint256 amount0Desired;
            uint256 amount1Desired;
            uint256 amount0Min;
            uint256 amount1Min;
            address recipient;
            uint256 deadline;
        }

        function mint(MintParams calldata params)
            external
            payable
            returns (
                uint256 tokenId,
                uint128 liquidity,
                uint256 amount0,
                uint256 amount1
            );

        struct IncreaseLiquidityParams {
            uint256 tokenId;
            uint256 amount0Desired;
            uint256 amount1Desired;
            uint256 amount0Min;
            uint256 amount1Min;
            uint256 deadline;
        }

        function increaseLiquidity(IncreaseLiquidityParams calldata params)
            external
            payable
            returns (
                uint128 liquidity,
                uint256 amount0,
                uint256 amount1
            );

        struct DecreaseLiquidityParams {
            uint256 tokenId;
            uint128 liquidity;
            uint256 amount0Min;
            uint256 amount1Min;
            uint256 deadline;
        }

        function decreaseLiquidity(DecreaseLiquidityParams calldata params)
            external
            payable
            returns (uint256 amount0, uint256 amount1);

        struct CollectParams {
            uint256 tokenId;
            address recipient;
            uint128 amount0Max;
            uint128 amount1Max;
        }

        function collect(CollectParams calldata params) external payable returns (uint256 amount0, uint256 amount1);

        function burn(uint256 tokenId) external payable;

        function permit(
            address spender,
            uint256 tokenId,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s
        ) external payable;

        function safeTransferFrom(address from, address to, uint256 tokenId) external;

        function safeTransferFrom(address from, address to, uint256 tokenId, bytes calldata data) external;
    }

    interface ISelfPermit {
        function selfPermit(address token, uint256 value, uint256 deadline, uint8 v, bytes32 r, bytes32 s) external payable;
        function selfPermitAllowed(address token, uint256 nonce, uint256 expiry, uint8 v, bytes32 r, bytes32 s) external payable;
    }

    interface IPeripheryPaymentsWithFee {
        function unwrapWETH9(uint256 amountMinimum, address recipient) external payable;

        function refundETH() external payable;

        function sweepToken(
            address token,
            uint256 amountMinimum,
            address recipient
        ) external payable;

        function unwrapWETH9WithFee(
            uint256 amountMinimum,
            address recipient,
            uint256 feeBips,
            address feeRecipient
        ) external payable;

        function sweepTokenWithFee(
            address token,
            uint256 amountMinimum,
            address recipient,
            uint256 feeBips,
            address feeRecipient
        ) external payable;
    }

    interface IUniswapV3Staker {
        struct IncentiveKey {
            address rewardToken;
            address pool;
            uint256 startTime;
            uint256 endTime;
            address refundee;
        }

        function withdrawToken(
            uint256 tokenId,
            address to,
            bytes memory data
        ) external;

        function stakeToken(IncentiveKey memory key, uint256 tokenId) external;

        function unstakeToken(IncentiveKey memory key, uint256 tokenId) external;

        function claimReward(
            address rewardToken,
            address to,
            uint256 amountRequested
        ) external returns (uint256 reward);
    }

    interface IQuoter {
        function quoteExactInput(bytes memory path, uint256 amountIn) external returns (uint256 amountOut);

        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountIn,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountOut);

        function quoteExactOutput(bytes memory path, uint256 amountOut) external returns (uint256 amountIn);

        function quoteExactOutputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountOut,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountIn);
    }
}

sol! {
    interface IQuoterV2 {
        function quoteExactInput(bytes memory path, uint256 amountIn)
            external
            returns (
                uint256 amountOut,
                uint160[] memory sqrtPriceX96AfterList,
                uint32[] memory initializedTicksCrossedList,
                uint256 gasEstimate
            );

        struct QuoteExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amountIn;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }

        function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
            external
            returns (
                uint256 amountOut,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );

        function quoteExactOutput(bytes memory path, uint256 amountOut)
            external
            returns (
                uint256 amountIn,
                uint160[] memory sqrtPriceX96AfterList,
                uint32[] memory initializedTicksCrossedList,
                uint256 gasEstimate
            );

        struct QuoteExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint256 amount;
            uint24 fee;
            uint160 sqrtPriceLimitX96;
        }

        function quoteExactOutputSingle(QuoteExactOutputSingleParams memory params)
            external
            returns (
                uint256 amountIn,
                uint160 sqrtPriceX96After,
                uint32 initializedTicksCrossed,
                uint256 gasEstimate
            );
    }

    interface ISwapRouter {
        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);

        struct ExactInputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
        }

        function exactInput(ExactInputParams calldata params) external payable returns (uint256 amountOut);

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactOutputSingle(ExactOutputSingleParams calldata params) external payable returns (uint256 amountIn);

        struct ExactOutputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
        }

        function exactOutput(ExactOutputParams calldata params) external payable returns (uint256 amountIn);
    }
}

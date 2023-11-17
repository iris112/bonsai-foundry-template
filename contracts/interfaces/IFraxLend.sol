// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.21;

interface IFraxLend {
    function deposit(
        uint256 _amount,
        address _receiver
    ) external returns (uint256 _sharesReceived);

    function redeem(
        uint256 _shares,
        address _receiver,
        address _owner
    ) external returns (uint256 _amountToReturn);

    function balanceOf(address) external view returns (uint256);

    function addInterest() external;

    /// @notice The ```toAssetAmount``` function converts a given number of shares to an asset amount
    /// @param _shares Shares of asset (fToken)
    /// @param _roundUp Whether to round up after division
    /// @return The amount of asset
    function toAssetAmount(
        uint256 _shares,
        bool _roundUp
    ) external view returns (uint256);

    /// @notice The ```toAssetShares``` function converts a given asset amount to a number of asset shares (fTokens)
    /// @param _amount The amount of asset
    /// @param _roundUp Whether to round up after division
    /// @return The number of shares (fTokens)
    function toAssetShares(
        uint256 _amount,
        bool _roundUp
    ) external view returns (uint256);

    function currentRateInfo()
        external
        view
        returns (
            uint64 lastBlock,
            uint64 feeToProtocolRate,
            uint64 lastTimestamp,
            uint64 ratePerSec
        );

    function totalAsset()
        external
        view
        returns (uint128 amount, uint128 shares);

    function totalBorrow()
        external
        view
        returns (uint128 amount, uint128 shares);

    function paused() external view returns (bool);

    function maturityDate() external view returns (uint256);

    function penaltyRate() external view returns (uint256);

    function rateContract() external view returns (address);

    function rateInitCallData() external view returns (bytes calldata);

    function getConstants()
        external
        pure
        returns (
            uint256 _LTV_PRECISION,
            uint256 _LIQ_PRECISION,
            uint256 _UTIL_PREC,
            uint256 _FEE_PRECISION,
            uint256 _EXCHANGE_PRECISION,
            uint64 _DEFAULT_INT,
            uint16 _DEFAULT_PROTOCOL_FEE,
            uint256 _MAX_PROTOCOL_FEE
        );
}
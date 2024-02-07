// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.21;

interface IDebtManager {
    struct StrategyAllocation {
        address strategy;
        uint256 debt;
    }

    function vault() external view returns (address);

    function aprOracle() external view returns (address);

    function siloToStrategy(address _silo) external view returns (address);

    function utilizationTargets(address _strategy) external view returns (uint256);

    function requestLiquidity(uint256 _amount, address _silo, uint256 _slippage) external;
}

// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.21;

import {Ownable} from "../lib/openzeppelin-contracts/contracts/access/Ownable.sol";
import {IFraxLendStrategy} from "./interfaces/IFraxLendStrategy.sol";
import {IFraxLend} from "./interfaces/IFraxLend.sol";
import {IFraxLendV2} from "./interfaces/IFraxLendV2.sol";
import {IFraxLendV3} from "./interfaces/IFraxLendV3.sol";
import {IDebtManager} from "./interfaces/IDebtManager.sol";
import {IVault} from "./interfaces/IVault.sol";
import {IBonsaiRelay} from "bonsai/IBonsaiRelay.sol";
import {BonsaiCallbackReceiver} from "bonsai/BonsaiCallbackReceiver.sol";

interface ISturdyStrategyRate {
    function MIN_TARGET_UTIL() external view returns (uint256);

    function MAX_TARGET_UTIL() external view returns (uint256);

    function VERTEX_UTILIZATION() external view returns (uint256);

    function MIN_FULL_UTIL_RATE() external view returns (uint256);

    function MAX_FULL_UTIL_RATE() external view returns (uint256);

    function ZERO_UTIL_RATE() external view returns (uint256);

    function RATE_HALF_LIFE() external view returns (uint256);

    function VERTEX_RATE_PERCENT() external view returns (uint256);

    function RATE_PREC() external view returns (uint256);
}

contract ZKOptimalAllocation is Ownable, BonsaiCallbackReceiver {
    error AG_INVALID_CONFIGURATION();

    struct SturdyStrategyDataParams {
        uint256 curTimestamp;
        uint256 lastTimestamp;
        uint256 ratePerSec;
        uint256 fullUtilizationRate;
        uint256 totalAsset;
        uint256 totalBorrow;
        uint256 UTIL_PREC;
        uint256 MIN_TARGET_UTIL;
        uint256 MAX_TARGET_UTIL;
        uint256 VERTEX_UTILIZATION;
        uint256 MIN_FULL_UTIL_RATE;
        uint256 MAX_FULL_UTIL_RATE;
        uint256 ZERO_UTIL_RATE;
        uint256 RATE_HALF_LIFE;
        uint256 VERTEX_RATE_PERCENT;
        uint256 RATE_PREC;
        bool isInterestPaused;
    }

    /// @notice Image ID of the only zkVM binary to accept callbacks from.
    bytes32 public immutable fibImageId;

    /// @notice Gas limit set on the callback from Bonsai.
    /// @dev Should be set to the maximum amount of gas your callback might reasonably consume.
    uint64 private constant BONSAI_CALLBACK_GAS_LIMIT = 1000000;


    IDebtManager.StrategyAllocation[] private _allocationDatas;
    uint256 private _newAPR;
    uint256 private _curAPR;
    bool private _isSuccess;

    /// @notice Initialize the contract, binding it to a specified Bonsai relay and RISC Zero guest image.
    constructor(IBonsaiRelay bonsaiRelay, bytes32 _fibImageId) BonsaiCallbackReceiver(bonsaiRelay) {
        fibImageId = _fibImageId;
    }

    function startOptimalAllocation(
        IVault vault,
        uint256 chunkCount,
        uint256 totalInitialAmount,
        IDebtManager.StrategyAllocation[] calldata initialDatas
    ) external {
        uint256 strategyCount = initialDatas.length;
        IVault.StrategyParams[] memory strategyDatas = new IVault.StrategyParams[](strategyCount);
        SturdyStrategyDataParams[] memory sturdyDatas = new SturdyStrategyDataParams[](strategyCount);
        uint256 totalAvailable;

        if (address(vault) != address(0)) {
            for (uint256 i; i < strategyCount; ++i) {
                strategyDatas[i] = vault.strategies(initialDatas[i].strategy);
                sturdyDatas[i] = _getSturdyStrategyData(initialDatas[i].strategy);
            }
            totalAvailable = vault.totalAssets() - vault.minimum_total_idle();
        }

        bonsaiRelay.requestCallback(
            fibImageId, 
            abi.encode(
                chunkCount,
                totalInitialAmount,
                totalAvailable,
                initialDatas,
                strategyDatas,
                sturdyDatas
            ),
            address(this), 
            this.onResult.selector, 
            BONSAI_CALLBACK_GAS_LIMIT
        );
    }

    /// @notice Callback function logic for processing verified journals from Bonsai.
    function onResult(
        IDebtManager.StrategyAllocation[] calldata allocationDatas, 
        uint256 newAPR,
        uint256 curAPR,
        bool isSuccess
    ) external onlyBonsaiCallback(fibImageId) {
        uint256 length = allocationDatas.length;
        for (uint256 i; i < length; ++i) {
            _allocationDatas.push(allocationDatas[i]);
        }
        _newAPR = newAPR;
        _curAPR = curAPR;
        _isSuccess = isSuccess;

        // isSuccess = true then, Perform allocation via debt manager
    }

    function getResult() external view returns (IDebtManager.StrategyAllocation[] memory, uint256, uint256, bool) {
        return (
            _allocationDatas,
            _newAPR,
            _curAPR,
            _isSuccess
        );
    }

    function _getSturdyStrategyData(
        address strategy
    ) internal view returns (SturdyStrategyDataParams memory) {
        SturdyStrategyDataParams memory result;

        address pair = IFraxLendStrategy(strategy).pair();
        result = _getSturdyStrategyPairData(pair, result);

        address rate = IFraxLend(pair).rateContract();
        result = _getSturdyStrategyRateData(rate, result);

        return result;
    }

    function _getSturdyStrategyPairData(
        address pair,
        SturdyStrategyDataParams memory data
    ) internal view returns (SturdyStrategyDataParams memory) {
        (
            ,
            ,
            data.lastTimestamp,
            data.ratePerSec,
            data.fullUtilizationRate
        ) = IFraxLendV2(pair).currentRateInfo();
        (, , data.UTIL_PREC, , , , , ) = IFraxLendV3(pair).getConstants();
        data.isInterestPaused = IFraxLendV3(pair).isInterestPaused();
        data.curTimestamp = block.timestamp;

        return data;
    }

    function _getSturdyStrategyRateData(
        address rate,
        SturdyStrategyDataParams memory data
    ) internal view returns (SturdyStrategyDataParams memory) {

        data.MIN_TARGET_UTIL = ISturdyStrategyRate(rate).MIN_TARGET_UTIL();
        data.MAX_TARGET_UTIL = ISturdyStrategyRate(rate).MAX_TARGET_UTIL();
        data.VERTEX_UTILIZATION = ISturdyStrategyRate(rate).VERTEX_UTILIZATION();
        data.MIN_FULL_UTIL_RATE = ISturdyStrategyRate(rate).MIN_FULL_UTIL_RATE();
        data.MAX_FULL_UTIL_RATE = ISturdyStrategyRate(rate).MAX_FULL_UTIL_RATE();
        data.ZERO_UTIL_RATE = ISturdyStrategyRate(rate).ZERO_UTIL_RATE();
        data.RATE_HALF_LIFE = ISturdyStrategyRate(rate).RATE_HALF_LIFE();
        data.VERTEX_RATE_PERCENT = ISturdyStrategyRate(rate).VERTEX_RATE_PERCENT();
        data.RATE_PREC = ISturdyStrategyRate(rate).RATE_PREC();

        return data;
    }
}

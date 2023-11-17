// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.21;

interface IFraxLendV2 {
    function currentRateInfo()
        external
        view
        returns (
            uint32 lastBlock,
            uint32 feeToProtocolRate,
            uint64 lastTimestamp,
            uint64 ratePerSec,
            uint64 fullUtilizationRate
        );
}

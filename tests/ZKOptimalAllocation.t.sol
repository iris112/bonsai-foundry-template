// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

pragma solidity 0.8.21;

import {BonsaiTest} from "bonsai/BonsaiTest.sol";
import {IBonsaiRelay} from "bonsai/IBonsaiRelay.sol";
import {ZKOptimalAllocation} from "contracts/ZKOptimalAllocation.sol";
import {IDebtManager} from "../contracts/interfaces/IDebtManager.sol";
import {IVault} from "../contracts/interfaces/IVault.sol";

contract ZKOptimalAllocationTest is BonsaiTest {
    string MAINNET_RPC_URL = vm.envString("MAINNET_RPC_URL");

    function setUp() public withRelay {
        uint256 mainnetFork = vm.createSelectFork(MAINNET_RPC_URL);
        vm.rollFork(18_386_071);

        assertEq(vm.activeFork(), mainnetFork);
        assertEq(block.number, 18_386_071);
    }

    // Test the ZKOptimalAllocation contract by mocking an off-chain callback request
    function testOffChainMock() public {
        bytes32 imageId = queryImageId("OPTIMAL_ALLOCATION");
        // Deploy a new starter instance
        ZKOptimalAllocation starter = new ZKOptimalAllocation(
            IBonsaiRelay(bonsaiRelay),
            imageId
        );

        // Anticipate a callback invocation on the starter contract
        vm.expectCall(address(starter), abi.encodeWithSelector(ZKOptimalAllocation.onResult.selector));
        // Relay the solution as a callback using simulated data
        uint64 BONSAI_CALLBACK_GAS_LIMIT = 100000;
        uint256 chunkCount = 100;
        uint256 totalInitialAmount;
        uint256 totalAvailable = 14000 * 10 ** 18;
        IDebtManager.StrategyAllocation[] memory initialDatas = new IDebtManager.StrategyAllocation[](4);
        for (uint256 i; i < 4; ++i) {
            initialDatas[0].strategy = address(uint160(i + 1));
        }
        IVault.StrategyParams[] memory strategyDatas = new IVault.StrategyParams[](4);
        strategyDatas[0] = IVault.StrategyParams(
            1697739068,
            1697739068,
            0,
            6000 * 10 ** 18
        );
        strategyDatas[1] = IVault.StrategyParams(
            1697739068,
            1697739068,
            0,
            5000 * 10 ** 18
        );
        strategyDatas[2] = IVault.StrategyParams(
            1697739091,
            1697739091,
            0,
            3000 * 10 ** 18
        );
        strategyDatas[3] = IVault.StrategyParams(
            1697739092,
            1697739092,
            0,
            3000 * 10 ** 18
        );
        ZKOptimalAllocation.SturdyStrategyDataParams[] memory sturdyDatas = new ZKOptimalAllocation.SturdyStrategyDataParams[](4);
        sturdyDatas[0] = ZKOptimalAllocation.SturdyStrategyDataParams(
            1697739119,
            1694820803,
            162996627,
            1582470460,
            0,
            0,
            100000,
            75000,
            85000,
            87500,
            1582470460,
            3164940920000,
            158247046,
            172800,
            200000000000000000,
            1000000000000000000,
            false
        );
        sturdyDatas[1] = ZKOptimalAllocation.SturdyStrategyDataParams(
            1697739119,
            1697698655,
            918533958,
            5894455579,
            0,
            0,
            100000,
            75000,
            85000,
            87500,
            1582470460,
            3164940920000,
            158247046,
            172800,
            200000000000000000,
            1000000000000000000,
            false
        );
        sturdyDatas[2] = ZKOptimalAllocation.SturdyStrategyDataParams(
            1697739119,
            1697564051,
            176565000,
            1582470460,
            0,
            0,
            100000,
            75000,
            85000,
            87500,
            1582470460,
            3164940920000,
            158247046,
            172800,
            200000000000000000,
            1000000000000000000,
            false
        );
        sturdyDatas[3] = ZKOptimalAllocation.SturdyStrategyDataParams(
            1697739119,
            1697096315,
            334268038,
            1582470460,
            0,
            0,
            100000,
            75000,
            85000,
            87500,
            1582470460,
            3164940920000,
            158247046,
            172800,
            200000000000000000,
            1000000000000000000,
            false
        );

        runCallbackRequest(
            imageId,
            abi.encode(
                chunkCount,
                totalInitialAmount,
                totalAvailable,
                initialDatas,
                strategyDatas,
                sturdyDatas
            ), 
            address(starter), 
            starter.onResult.selector, 
            BONSAI_CALLBACK_GAS_LIMIT
        );

        // Validate the optimal allocation value
        (
            IDebtManager.StrategyAllocation[] memory allocations, 
            uint256 newAPR, 
            uint256 curAPR, 
            bool isSuccess
        ) = starter.getResult();

        assertEq(allocations[0].strategy, address(4));
        assertEq(allocations[0].debt, uint256(2940000000000000000000));
        assertEq(allocations[1].strategy, address(3));
        assertEq(allocations[1].debt, uint256(2940000000000000000000));
        assertEq(allocations[2].strategy, address(2));
        assertEq(allocations[2].debt, uint256(4900000000000000000000));
        assertEq(allocations[3].strategy, address(1));
        assertEq(allocations[3].debt, uint256(3220000000000000000000));
        assertEq(newAPR > curAPR, true);
        assertEq(isSuccess, true);
    }

    // Test the ZKOptimalAllocation contract by mocking an on-chain callback request
    function testOnChainMock() public {
        // Deploy a new starter instance
        ZKOptimalAllocation starter = new ZKOptimalAllocation(
            IBonsaiRelay(bonsaiRelay),
            queryImageId("OPTIMAL_ALLOCATION")
        );

        // Anticipate an on-chain callback request to the relay
        vm.expectCall(address(bonsaiRelay), abi.encodeWithSelector(IBonsaiRelay.requestCallback.selector));
        // Request the on-chain callback
        IDebtManager.StrategyAllocation[] memory initialDatas;
        starter.startOptimalAllocation(
            IVault(address(0)),
            100,
            0,
            initialDatas
        );

        // Anticipate a callback invocation on the starter contract
        vm.expectCall(address(starter), abi.encodeWithSelector(ZKOptimalAllocation.onResult.selector));
        // Relay the solution as a callback
        runPendingCallbackRequest();

        // Validate the optimal allocation value
        (
            IDebtManager.StrategyAllocation[] memory allocations, 
            uint256 newAPR, 
            uint256 curAPR, 
            bool isSuccess
        ) = starter.getResult();
        assertEq(allocations.length, 0);
        assertEq(newAPR, 0);
        assertEq(curAPR, 0);
        assertEq(isSuccess, false);
    }
}

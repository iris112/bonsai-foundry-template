// SPDX-License-Identifier: AGPL-3.0
pragma solidity 0.8.21;

import {IStrategy} from "./IStrategy.sol";

interface ISturdySiloStrategy is IStrategy {
    function pair() external view returns (address);
}

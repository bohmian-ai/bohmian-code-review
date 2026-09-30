// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract ValidContract {
    uint256 public total;

    function add(uint256 amount) public {
        total += amount;
    }
}

contract BrokenContract {
    uint256 public value = 10;

    function explode(uint256 x) public {
        if (x > 5 {
            revert("boom");
        }
    }
}

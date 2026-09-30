pragma solidity ^0.8.20;

contract Broken {
    uint256 public value = 10;

    function set(uint256 newValue) public {
        value = newValue;
        if (newValue > 100 {
            revert("too big");
        }
    }

    function
}

// SPDX-License-Identifier: MIT
// Ünïcode contract — héllo wörld 😀
pragma solidity ^0.8.20;

contract Unicode {
    string public message = "héllo wörld 😀";

    function setMessage(string memory newMessage) public {
        message = newMessage;
    }

    function getMessage() public view returns (string memory) {
        return message;
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./interfaces/IERC20.sol";
import {Ownable} from "./Ownable.sol";

interface IERC721 {
    function ownerOf(uint256 tokenId) external view returns (address);
}

contract Token is IERC20, Ownable {
    mapping(address => uint256) private _balances;
    uint256 public totalSupply;

    event Transfer(address indexed from, address indexed to, uint256 value);
    error InsufficientBalance(uint256 available, uint256 required);

    modifier onlyAdmin() {
        require(msg.sender == admin, "not admin");
        _;
    }

    constructor(uint256 initialSupply) {
        totalSupply = initialSupply;
    }

    fallback() external payable {}

    receive() external payable {}

    function balanceOf(address account) public view returns (uint256) {
        return _balances[account];
    }

    function mint(address to, uint256 amount) public onlyAdmin {
        _balances[to] += amount;
        totalSupply += amount;
        emit Transfer(address(0), to, amount);
    }

    function burn(uint256 amount) public {
        uint256 balance = _balances[msg.sender];
        if (balance < amount) {
            revert InsufficientBalance(balance, amount);
        }
        _balances[msg.sender] = balance - amount;
        totalSupply -= amount;
        emit Transfer(msg.sender, address(0), amount);
    }
}

library MathLib {
    function max(uint256 a, uint256 b) internal pure returns (uint256) {
        return a >= b ? a : b;
    }
}

struct Point {
    uint256 x;
    uint256 y;
}

enum Color { RED, GREEN, BLUE }

contract Factory {
    Token private _token;
    address public admin = msg.sender;

    function deploy() public returns (Token) {
        _token = new Token(1000);
        return _token;
    }
}

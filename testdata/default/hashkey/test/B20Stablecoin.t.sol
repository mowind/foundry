// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {B20TestBase} from "../src/B20TestBase.sol";
import {B20Caller, IB20Factory, IB20} from "../src/B20.sol";

/// Exercises B20 Stablecoin creation and a state-changing lifecycle through the
/// native Factory, plus its deterministic address derivation.
contract B20StablecoinTest is B20TestBase {
    B20Caller caller;

    function setUp() public {
        caller = new B20Caller();
    }

    function testCreateStablecoin() public returns (address token) {
        token = caller.createStablecoin(
            bytes32(uint256(0xC0FFEE)),
            "TestStable",
            "TST",
            address(this),
            "USD"
        );

        // Stablecoin variant discriminant is 0x01 at address byte [10].
        assertEq(uint8(uint160(token) >> 72), 0x01, "stablecoin must encode variant 0x01");
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token), "stablecoin must be initialized");

        IB20 b20 = IB20(token);
        assertEq(b20.name(), "TestStable", "name mismatch");
        assertEq(b20.symbol(), "TST", "symbol mismatch");
        assertEq(b20.currency(), "USD", "currency mismatch");
    }

    function testStablecoinDeterministicAddress() public {
        address predicted =
            caller.predictAddress(IB20Factory.B20Variant.STABLECOIN, address(caller), keccak256("det"));
        address actual = caller.createStablecoin(keccak256("det"), "DetStable", "DET", address(this), "USD");
        assertEq(predicted, actual, "deterministic address mismatch");
    }

    function testStablecoinStateChangingOperation() public {
        address token =
            caller.createStablecoin(keccak256("stable-op"), "StateStable", "STA", address(this), "USD");
        IB20 b20 = IB20(token);

        bytes32 mintRole = b20.MINT_ROLE();
        b20.grantRole(mintRole, address(this));

        uint256 mintAmount = 500e18;
        b20.mint(address(0xBEEF), mintAmount);
        assertEq(b20.balanceOf(address(0xBEEF)), mintAmount, "mint balance mismatch");
        assertEq(b20.totalSupply(), mintAmount, "totalSupply mismatch");

        vm.prank(address(0xBEEF));
        b20.transfer(address(0xCAFE), 200e18);
        assertEq(b20.balanceOf(address(0xBEEF)), 300e18, "sender balance after transfer");
        assertEq(b20.balanceOf(address(0xCAFE)), 200e18, "receiver balance after transfer");
    }

    function testStablecoinRejectsInvalidCurrency() public {
        // Non-A-Z currency code is rejected by the native initializer and rolls back creation.
        vm.expectRevert();
        caller.createStablecoin(keccak256("bad-cur"), "BadStable", "BAD", address(this), "usd");
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {B20Caller, IB20Factory, IB20} from "../src/B20.sol";

contract B20AssetTest is Test {
    B20Caller caller;
    address constant FACTORY = 0xB20f000000000000000000000000000000000000;
    bytes32 constant SALT = bytes32(uint256(0xC0FFEE));

    function setUp() public {
        caller = new B20Caller();
    }

    /// The three singleton precompiles must have code (the 0xef marker) so that
    /// Solidity's EXTCODESIZE check passes before the typed external call.
    function testSingletonMarkersPresent() public view {
        assertGt(FACTORY.code.length, 0, "B20Factory marker missing");
        assertGt(
            address(0x8453000000000000000000000000000000000001).code.length, 0,
            "ActivationRegistry marker missing"
        );
        assertGt(
            address(0x8453000000000000000000000000000000000002).code.length, 0,
            "PolicyRegistry marker missing"
        );
    }

    /// Creating a B20 Asset through the native Factory must succeed and return a
    /// structurally-valid B20 address.
    function testCreateB20Asset() public returns (address token) {
        token = caller.createAsset(SALT, "TestAsset", "TST", address(this));

        // The returned address must have the B20 prefix (byte 0 == 0xb2).
        assertEq(uint8(uint160(token) >> 152), 0xb2, "token address must have B20 prefix");
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token), "token must be initialized");
    }

    /// The deterministic address computed by getB20Address must match the actual creation.
    function testDeterministicAddress() public {
        address predicted =
            caller.predictAddress(IB20Factory.B20Variant.ASSET, address(caller), SALT);
        address actual = caller.createAsset(SALT, "DetAsset", "DET", address(this));
        assertEq(predicted, actual, "deterministic address mismatch");
    }

    /// After creation, a state-changing token operation (mint + transfer) must succeed.
    function testStateChangingTokenOperation() public {
        address token = caller.createAsset(
            keccak256("state-op"),
            "StateAsset",
            "STA",
            address(this)
        );

        IB20 b20 = IB20(token);

        // The creator (this contract) should be the admin and can grant mint role.
        bytes32 mintRole = b20.MINT_ROLE();
        b20.grantRole(mintRole, address(this));

        // Mint tokens — a state-changing operation.
        uint256 mintAmount = 1000e18;
        b20.mint(address(0xBEEF), mintAmount);
        assertEq(b20.balanceOf(address(0xBEEF)), mintAmount, "mint balance mismatch");
        assertEq(b20.totalSupply(), mintAmount, "totalSupply mismatch");

        // Transfer — another state-changing operation.
        vm.prank(address(0xBEEF));
        b20.transfer(address(0xCAFE), 400e18);
        assertEq(b20.balanceOf(address(0xBEEF)), 600e18, "sender balance after transfer");
        assertEq(b20.balanceOf(address(0xCAFE)), 400e18, "receiver balance after transfer");
    }
}

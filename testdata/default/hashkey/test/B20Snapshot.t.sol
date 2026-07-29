// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {B20TestBase} from "../src/B20TestBase.sol";
import {B20Caller, IB20Factory, IB20, IActivationRegistry} from "../src/B20.sol";
import {IPolicyRegistry} from "../src/B20.sol";

/// Verifies that Forge snapshot/revert restores Activation flags, token storage
/// and dynamic markers through the ordinary journal/database mechanism, and that
/// a failed Factory creation atomically rolls back marker and storage.
contract B20SnapshotTest is B20TestBase {
    B20Caller caller;
    IActivationRegistry registry = IActivationRegistry(ACTIVATION_REGISTRY);

    IPolicyRegistry policy = IPolicyRegistry(POLICY_REGISTRY);

    function setUp() public {
        caller = new B20Caller();
    }

    /// Reverting a journaled snapshot restores token balances and supply.
    function testSnapshotRevertsTokenStorage() public {
        address token = caller.createAsset(keccak256("snap-token"), "SnapAsset", "SNP", address(this));
        IB20 b20 = IB20(token);
        b20.grantRole(b20.MINT_ROLE(), address(this));
        b20.mint(address(0xBEEF), 1000e18);

        uint256 snap = vm.snapshot();

        b20.mint(address(0xCAFE), 500e18);
        assertEq(b20.balanceOf(address(0xCAFE)), 500e18);
        assertEq(b20.totalSupply(), 1500e18);

        assertTrue(vm.revertTo(snap));

        assertEq(b20.balanceOf(address(0xCAFE)), 0, "revert must clear post-snapshot mint");
        assertEq(b20.balanceOf(address(0xBEEF)), 1000e18, "revert must restore pre-snapshot balance");
        assertEq(b20.totalSupply(), 1000e18, "revert must restore totalSupply");
    }

    /// Reverting a journaled snapshot restores an Activation flag flipped after the snapshot.
    function testSnapshotRevertsActivationFlag() public {
        assertTrue(registry.isActivated(FEATURE_ASSET));
        uint256 snap = vm.snapshot();

        vm.prank(ADMIN);
        registry.deactivate(FEATURE_ASSET);
        assertFalse(registry.isActivated(FEATURE_ASSET));

        assertTrue(vm.revertTo(snap));
        assertTrue(registry.isActivated(FEATURE_ASSET), "revert must restore the seeded activation");
    }

    /// A failed Factory creation (a reverting init call) must atomically roll back the
    /// dynamic marker and token storage: the derived address is left uninitialized.
    function testFactoryRollbackClearsDynamicMarker() public {
        bytes32 salt = keccak256("rollback");
        address predicted =
            caller.predictAddress(IB20Factory.B20Variant.ASSET, address(caller), salt);

        // The factory has no balance, so a transfer init call reverts mid-creation,
        // exercising the Factory's atomic checkpoint rollback.
        bytes[] memory initCalls = new bytes[](1);
        initCalls[0] = abi.encodeWithSelector(IB20.transfer.selector, address(0xCAFE), 1e18);

        vm.expectRevert();
        caller.createAssetWithInitCalls(salt, "RollAsset", "ROL", address(this), initCalls);

        assertFalse(
            IB20Factory(FACTORY).isB20Initialized(predicted), "rollback must clear the dynamic marker"
        );

        // The rolled-back address is now a plain uninitialized account: a fresh creation
        // with the same derivation key must succeed.
        address token = caller.createAsset(salt, "RollAsset", "ROL", address(this));
        assertEq(token, predicted, "rolled-back address must be reusable");
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token));
    }

    /// Reverting a journaled snapshot taken before a *successful* Factory creation
    /// clears the dynamic marker and token storage, not just the in-creation checkpoint.
    function testSnapshotRevertClearsDynamicMarker() public {
        bytes32 salt = keccak256("snap-marker");
        address predicted =
            caller.predictAddress(IB20Factory.B20Variant.ASSET, address(caller), salt);

        uint256 snap = vm.snapshot();
        address token = caller.createAsset(salt, "SnapMarker", "SNM", address(this));
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token), "token must be created");

        assertTrue(vm.revertTo(snap));
        assertFalse(
            IB20Factory(FACTORY).isB20Initialized(predicted),
            "revert must clear the successfully-created dynamic marker"
        );
    }

    /// Reverting a journaled snapshot restores PolicyRegistry membership storage.
    function testSnapshotRevertsPolicyStorage() public {
        uint64 id = policy.createPolicy(address(this), IPolicyRegistry.PolicyType.BLOCKLIST);
        address[] memory blocked = new address[](1);
        blocked[0] = address(0xCAFE);
        policy.updateBlocklist(id, true, blocked);
        assertFalse(policy.isAuthorized(id, address(0xCAFE)));

        uint256 snap = vm.snapshot();

        // Remove the blocklist entry, then restore the snapshot.
        policy.updateBlocklist(id, false, blocked);
        assertTrue(policy.isAuthorized(id, address(0xCAFE)));

        assertTrue(vm.revertTo(snap));
        assertFalse(
            policy.isAuthorized(id, address(0xCAFE)), "revert must restore the policy blocklist"
        );
    }
}

// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {B20TestBase} from "../src/B20TestBase.sol";
import {B20Caller, IB20Factory, IB20} from "../src/B20.sol";

/// Wraps mutation cheatcodes behind an external call so a B20-protected rejection
/// surfaces as a lower-depth revert that `vm.expectRevert` can observe.
contract CheatcodeCaller is Test {
    function doStore(address target, bytes32 slot, bytes32 value) external {
        vm.store(target, slot, value);
    }

    function doEtch(address target, bytes memory code) external {
        vm.etch(target, code);
    }
}

/// Verifies the profile-aware cheatcode guard for B20 standalone local state:
/// vm.load may inspect any B20 storage, while vm.store and vm.etch reject the
/// fixed singletons and Factory-initialized dynamic tokens but permit
/// uninitialized 0xb2... addresses. Also covers the empty-revert behavior of an
/// uninitialized dynamic token call.
contract B20ProtectionTest is B20TestBase {
    B20Caller caller;
    CheatcodeCaller cc;

    function setUp() public {
        caller = new B20Caller();
        cc = new CheatcodeCaller();
    }

    // ── vm.load: read-only inspection stays open ────────────────────────────

    function testLoadReadsSeededActivationSlot() public view {
        bytes32 value = vm.load(ACTIVATION_REGISTRY, SLOT_ASSET);
        assertEq(value, bytes32(uint256(1)), "seeded feature slot must read active (1)");
    }

    function testLoadReadsFactoryMarker() public view {
        // The factory singleton marker code hash is observable through ordinary account reads.
        assertGt(FACTORY.code.length, 0, "factory marker must be present");
    }

    // ── vm.store / vm.etch: fixed singletons are protected ──────────────────

    function testStoreRejectsSingleton() public {
        vm.expectRevert();
        cc.doStore(ACTIVATION_REGISTRY, SLOT_ASSET, bytes32(uint256(0)));
    }

    function testStoreRejectsFactory() public {
        vm.expectRevert();
        cc.doStore(FACTORY, 0, bytes32(uint256(1)));
    }

    function testEtchRejectsSingleton() public {
        vm.expectRevert();
        cc.doEtch(POLICY_REGISTRY, hex"00");
    }

    // ── vm.store: initialized dynamic tokens are protected ──────────────────

    function testStoreRejectsInitializedDynamicToken() public {
        address token = caller.createAsset(keccak256("protected"), "ProtAsset", "PRT", address(this));
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token));

        vm.expectRevert();
        cc.doStore(token, 0, bytes32(uint256(1)));
    }

    function testEtchRejectsInitializedDynamicToken() public {
        address token = caller.createAsset(keccak256("protected-etch"), "ProtAsset2", "PR2", address(this));

        vm.expectRevert();
        cc.doEtch(token, hex"00");
    }

    // ── uninitialized 0xb2 addresses keep ordinary account semantics ────────

    function testStorePermitsUninitializedB20Address() public {
        // Canonical Asset-variant structure (0xb2 prefix, zero run, variant 0x00) with no
        // Factory marker: an ordinary account, writable via cheatcodes.
        address uninit = 0xB200000000000000000000000000000000000000;
        bytes32 slot = keccak256("uninit-slot");
        vm.store(uninit, slot, bytes32(uint256(0xABCDEF)));
        assertEq(vm.load(uninit, slot), bytes32(uint256(0xABCDEF)));
    }

    function testEtchPermitsUninitializedB20Address() public {
        address uninit = 0xb200000000000000000000000000000000000001;
        vm.etch(uninit, hex"dead");
        assertEq(uninit.code, hex"dead");
    }

    /// A typed call to a structurally-valid but uninitialized dynamic token must
    /// empty-revert (the native dispatcher rejects accounts without the marker).
    function testUninitializedDynamicCallReverts() public {
        address uninit = 0xB200000000000000000000000000000000000002;
        vm.expectRevert();
        IB20(uninit).name();
    }
}

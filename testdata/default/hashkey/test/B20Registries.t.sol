// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {B20TestBase} from "../src/B20TestBase.sol";
import {B20Caller, IB20Factory, IActivationRegistry, IPolicyRegistry} from "../src/B20.sol";

/// Exercises the ActivationRegistry and PolicyRegistry lifecycle under the
/// confirmed development-admin workflow: feature seed, admin-gated
/// deactivate/reactivate, typed-revert paths, and the feature gate on Factory
/// creation.
contract B20RegistriesTest is B20TestBase {
    B20Caller caller;
    IActivationRegistry registry = IActivationRegistry(ACTIVATION_REGISTRY);
    IPolicyRegistry policy = IPolicyRegistry(POLICY_REGISTRY);

    function setUp() public {
        caller = new B20Caller();
    }

    // ── ActivationRegistry: genesis baseline ────────────────────────────────

    function testActivationAdminIsDevelopmentAdmin() public view {
        assertEq(registry.admin(), ADMIN, "activation admin must be the local dev admin");
    }

    function testAllThreeFeaturesSeededActive() public view {
        assertTrue(registry.isActivated(FEATURE_POLICY), "PolicyRegistry feature not seeded");
        assertTrue(registry.isActivated(FEATURE_STABLECOIN), "Stablecoin feature not seeded");
        assertTrue(registry.isActivated(FEATURE_ASSET), "Asset feature not seeded");
    }

    function testCheckActivatedSucceedsForSeededFeature() public view {
        // Should not revert for a seeded feature.
        registry.checkActivated(FEATURE_ASSET);
    }

    function testCheckActivatedRevertsForUnknownFeature() public {
        bytes32 unknown = keccak256("not.a.real.feature");
        vm.expectRevert(abi.encodeWithSelector(IActivationRegistry.FeatureNotActivated.selector, unknown));
        registry.checkActivated(unknown);
    }

    // ── ActivationRegistry: admin-gated transitions ─────────────────────────

    function testDeactivateReactivateFeature() public {
        // Deactivate requires the development admin.
        vm.prank(ADMIN);
        registry.deactivate(FEATURE_ASSET);
        assertFalse(registry.isActivated(FEATURE_ASSET), "feature should be deactivated");

        // The deactivation must be effective: Factory creation for this variant reverts.
        vm.expectRevert();
        caller.createAsset(bytes32(uint256(0x1)), "NoAsset", "NOA", address(this));

        // Reactivate restores the feature.
        vm.prank(ADMIN);
        registry.activate(FEATURE_ASSET);
        assertTrue(registry.isActivated(FEATURE_ASSET), "feature should be reactivated");

        // Factory creation works again after reactivation.
        address token = caller.createAsset(bytes32(uint256(0x2)), "YesAsset", "YES", address(this));
        assertTrue(IB20Factory(FACTORY).isB20Initialized(token), "token must be initialized");
    }

    function testDeactivateRejectsNonAdmin() public {
        vm.expectRevert(abi.encodeWithSelector(IActivationRegistry.Unauthorized.selector, address(this)));
        registry.deactivate(FEATURE_ASSET);
    }

    function testActivateAlreadyActiveReverts() public {
        vm.prank(ADMIN);
        vm.expectRevert(abi.encodeWithSelector(IActivationRegistry.AlreadyActivated.selector, FEATURE_ASSET));
        registry.activate(FEATURE_ASSET);
    }

    function testDeactivateAlreadyInactiveReverts() public {
        vm.prank(ADMIN);
        registry.deactivate(FEATURE_STABLECOIN);
        assertFalse(registry.isActivated(FEATURE_STABLECOIN));

        vm.prank(ADMIN);
        vm.expectRevert(
            abi.encodeWithSelector(IActivationRegistry.FeatureNotActivated.selector, FEATURE_STABLECOIN)
        );
        registry.deactivate(FEATURE_STABLECOIN);

        // Restore the baseline so other tests are unaffected.
        vm.prank(ADMIN);
        registry.activate(FEATURE_STABLECOIN);
    }

    // ── PolicyRegistry: success and typed-revert paths ──────────────────────

    function testCreatePolicyAndView() public {
        uint64 id = policy.createPolicy(address(this), IPolicyRegistry.PolicyType.BLOCKLIST);
        assertTrue(policy.policyExists(id), "policy must exist");
        assertEq(policy.policyAdmin(id), address(this), "policy admin mismatch");

        // An empty blocklist authorizes everyone.
        assertTrue(policy.isAuthorized(id, address(0xBEEF)), "empty blocklist should authorize");
    }

    function testBlocklistRejectsAccount() public {
        uint64 id = policy.createPolicy(address(this), IPolicyRegistry.PolicyType.BLOCKLIST);
        address[] memory blocked = new address[](1);
        blocked[0] = address(0xCAFE);
        policy.updateBlocklist(id, true, blocked);
        assertFalse(policy.isAuthorized(id, address(0xCAFE)), "blocked account should be unauthorized");
        assertTrue(policy.isAuthorized(id, address(0xBEEF)), "non-blocked account should pass");
    }

    function testUpdateBlocklistRejectsNonAdmin() public {
        uint64 id = policy.createPolicy(address(this), IPolicyRegistry.PolicyType.BLOCKLIST);
        address[] memory blocked = new address[](1);
        blocked[0] = address(0xCAFE);
        vm.prank(address(0xDEAD));
        vm.expectRevert(IPolicyRegistry.Unauthorized.selector);
        policy.updateBlocklist(id, true, blocked);
    }

    function testCreatePolicyRejectsZeroAdmin() public {
        vm.expectRevert(IPolicyRegistry.ZeroAddress.selector);
        policy.createPolicy(address(0), IPolicyRegistry.PolicyType.BLOCKLIST);
    }

    function testPolicyAdminReturnsZeroForUnknownPolicy() public view {
        // Unknown custom policies are not protected; policyAdmin returns the zero admin
        // rather than reverting.
        assertEq(policy.policyAdmin(type(uint64).max), address(0));
    }

    function testCreatePolicyRevertsWhenFeatureDeactivated() public {
        vm.prank(ADMIN);
        registry.deactivate(FEATURE_POLICY);
        assertFalse(registry.isActivated(FEATURE_POLICY));

        vm.expectRevert();
        policy.createPolicy(address(this), IPolicyRegistry.PolicyType.BLOCKLIST);

        // Restore baseline.
        vm.prank(ADMIN);
        registry.activate(FEATURE_POLICY);
    }
}

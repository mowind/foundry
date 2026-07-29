// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";

/// Shared constants for the HashKey B20 standalone local state contract.
contract B20TestBase is Test {
    address constant FACTORY = 0xB20f000000000000000000000000000000000000;
    address constant ACTIVATION_REGISTRY = 0x8453000000000000000000000000000000000001;
    address constant POLICY_REGISTRY = 0x8453000000000000000000000000000000000002;

    /// Confirmed development activation admin (not a production parameter).
    address constant ADMIN = 0xCB00000000000000000000000000000000000000;

    /// Canonical feature ids: keccak256 of the canonical feature name.
    bytes32 constant FEATURE_POLICY = 0xb582ebae03f16fee49a6763f78df482fb11ae73f103ed0d330bbe556aa90a43f;
    bytes32 constant FEATURE_STABLECOIN = 0xecfa0def2c10020caaf65e6155aa69c84b24892aaef76eeac52e0e2b3a0b8601;
    bytes32 constant FEATURE_ASSET = 0xcdcc772fe4cbdb1029f822861176d09e646db96723d4c1e82ddfdeb8163ef54c;

    /// Canonical ActivationRegistry feature mapping slots (value 1 = active).
    bytes32 constant SLOT_POLICY = 0x8c5327ddcca092db72284503162323c6e8d392394b1d5c71991227bbc26f7c07;
    bytes32 constant SLOT_STABLECOIN = 0xca7c276524c5aeaac4d56c8a3d36eb5f9a64f60841fb65b539c99c21ca7df109;
    bytes32 constant SLOT_ASSET = 0x819420403a306232adb8ee78d9f35b5090371155b34376cf9b020e30029278e5;
}

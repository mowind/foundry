//! HashKey B20 integration fixture.
//!
//! Verifies that a Forge project with `network = "hashkey"` can compile a Solidity
//! caller, create a B20 Asset through the native Factory, verify its deterministic
//! address and marker, and complete a state-changing token operation.

use foundry_test_utils::forgetest_init;

forgetest_init!(hashkey_b20_asset_lifecycle, |prj, cmd| {
    // Add the B20 interface and caller.
    prj.add_source("B20.sol", include_str!("../../../../testdata/default/hashkey/src/B20.sol"));

    // Add the B20 test contract.
    prj.add_test("B20.t.sol", include_str!("../../../../testdata/default/hashkey/test/B20.t.sol"));

    // Enable the HashKey network profile.
    prj.create_file(
        "foundry.toml",
        r#"
[default]
src = "src"
out = "out"
libs = ["lib"]
network = "hashkey"
"#,
    );

    // Run only the B20 tests to avoid Counter test noise.
    cmd.arg("test").arg("--match-test").arg("B20").assert_success();
});

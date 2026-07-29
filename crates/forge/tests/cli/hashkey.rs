//! HashKey B20 integration fixtures.
//!
//! Verifies that a Forge project with `network = "hashkey"` can compile Solidity
//! callers, exercise the full B20 standalone local state lifecycle — Asset and
//! Stablecoin creation, ActivationRegistry/PolicyRegistry transitions, snapshot/revert,
//! cheatcode protection and Factory atomic rollback — through the native precompiles.

use foundry_test_utils::forgetest_init;

forgetest_init!(hashkey_b20_state_lifecycle, |prj, cmd| {
    // Add the B20 interfaces, caller helper and shared test base.
    prj.add_source("B20.sol", include_str!("../../../../testdata/default/hashkey/src/B20.sol"));
    prj.add_source(
        "B20TestBase.sol",
        include_str!("../../../../testdata/default/hashkey/src/B20TestBase.sol"),
    );

    // Add the B20 test contracts.
    prj.add_test("B20.t.sol", include_str!("../../../../testdata/default/hashkey/test/B20.t.sol"));
    prj.add_test(
        "B20Registries.t.sol",
        include_str!("../../../../testdata/default/hashkey/test/B20Registries.t.sol"),
    );
    prj.add_test(
        "B20Stablecoin.t.sol",
        include_str!("../../../../testdata/default/hashkey/test/B20Stablecoin.t.sol"),
    );
    prj.add_test(
        "B20Snapshot.t.sol",
        include_str!("../../../../testdata/default/hashkey/test/B20Snapshot.t.sol"),
    );
    prj.add_test(
        "B20Protection.t.sol",
        include_str!("../../../../testdata/default/hashkey/test/B20Protection.t.sol"),
    );

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

    // Run only the B20 fixtures to avoid Counter test noise.
    cmd.arg("test").arg("--match-path").arg("B20").assert_success();
});

#[cfg(all(unix, feature = "optimism", not(feature = "hashkey")))]
use foundry_test_utils::{snapbox::cmd::Command, str};

#[cfg(unix)]
mod repl;

#[cfg(all(unix, feature = "optimism", not(feature = "hashkey")))]
#[test]
fn hashkey_network_requires_build_capability() {
    Command::new(env!("CARGO_BIN_EXE_chisel"))
        .args(["--network", "hashkey", "eval", "1"])
        .assert()
        .failure()
        .stdout_eq("")
        .stderr_eq(str![[r#"
error: invalid value 'hashkey' for '--network <NETWORK>'
  [possible values: ethereum, optimism, tempo]

For more information, try '--help'.

"#]]);
}

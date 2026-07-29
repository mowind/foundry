//! Tests for the HashKey B20 network profile in Anvil.

#[cfg(feature = "cli")]
use std::{
    io::Read,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

#[cfg(feature = "cli")]
use crate::utils::http_provider;
use alloy_eips::eip7910::EthConfig;
use alloy_network::{AnyNetwork, TransactionBuilder};
use alloy_primitives::{Address, Bytes, U256, address, b256};
use alloy_provider::Provider;
#[cfg(feature = "cli")]
use alloy_rpc_types::TransactionRequest;
#[cfg(feature = "cli")]
use alloy_sol_types::{SolCall, sol};
use anvil::{NodeConfig, spawn};
use foundry_evm_networks::NetworkConfigs;

const B20_FACTORY: Address = address!("0xB20f000000000000000000000000000000000000");
const B20_ACTIVATION_REGISTRY: Address = address!("0x8453000000000000000000000000000000000001");
const B20_POLICY_REGISTRY: Address = address!("0x8453000000000000000000000000000000000002");
const FEATURE_POLICY_SLOT: U256 = U256::from_be_bytes(
    b256!("8c5327ddcca092db72284503162323c6e8d392394b1d5c71991227bbc26f7c07").0,
);
const FEATURE_STABLECOIN_SLOT: U256 = U256::from_be_bytes(
    b256!("ca7c276524c5aeaac4d56c8a3d36eb5f9a64f60841fb65b539c99c21ca7df109").0,
);
const FEATURE_ASSET_SLOT: U256 = U256::from_be_bytes(
    b256!("819420403a306232adb8ee78d9f35b5090371155b34376cf9b020e30029278e5").0,
);

#[cfg(feature = "cli")]
sol! {
    interface IB20Factory {
        function isB20(address token) external view returns (bool);
    }
}

#[cfg(feature = "cli")]
struct ChildGuard(Child);

#[cfg(feature = "cli")]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(feature = "cli")]
fn anvil_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_anvil") {
        return PathBuf::from(path);
    }

    std::env::current_exe()
        .expect("test executable path")
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target/debug directory")
        .join("anvil")
}

async fn assert_hashkey_genesis_baseline(provider: &impl Provider<AnyNetwork>) {
    for address in [B20_FACTORY, B20_ACTIVATION_REGISTRY, B20_POLICY_REGISTRY] {
        assert_eq!(provider.get_code_at(address).await.unwrap(), Bytes::from_static(&[0xef]));
    }
    for slot in [FEATURE_POLICY_SLOT, FEATURE_STABLECOIN_SLOT, FEATURE_ASSET_SLOT] {
        assert_eq!(
            provider.get_storage_at(B20_ACTIVATION_REGISTRY, slot).await.unwrap(),
            U256::ONE,
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hashkey_genesis_reset_and_precompile_inventory() {
    let (api, handle) =
        spawn(NodeConfig::test().with_networks(NetworkConfigs::with_hashkey())).await;
    let provider = handle.http_provider();

    assert_hashkey_genesis_baseline(&provider).await;

    let config: EthConfig = provider.client().request("eth_config", ()).await.unwrap();
    assert_eq!(config.current.precompiles.get("B20Factory"), Some(&B20_FACTORY));
    assert_eq!(
        config.current.precompiles.get("B20ActivationRegistry"),
        Some(&B20_ACTIVATION_REGISTRY),
    );
    assert_eq!(config.current.precompiles.get("B20PolicyRegistry"), Some(&B20_POLICY_REGISTRY),);
    assert_eq!(
        config.current.precompiles.values().filter(|address| address.as_slice()[0] == 0xb2).count(),
        1,
        "the static inventory must contain only the B20 Factory in the 0xb2 address domain",
    );

    api.anvil_set_code(B20_FACTORY, Bytes::from_static(&[0xde, 0xad])).await.unwrap();
    api.anvil_set_storage_at(B20_ACTIVATION_REGISTRY, FEATURE_ASSET_SLOT, U256::ZERO.into())
        .await
        .unwrap();
    api.anvil_reset(None).await.unwrap();

    assert_hashkey_genesis_baseline(&provider).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn hashkey_fork_preserves_remote_b20_state() {
    let (source_api, source_handle) = spawn(NodeConfig::test()).await;
    let remote_code = Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]);
    let remote_feature_value = U256::from(7);
    source_api.anvil_set_code(B20_FACTORY, remote_code.clone()).await.unwrap();
    source_api
        .anvil_set_storage_at(
            B20_ACTIVATION_REGISTRY,
            FEATURE_ASSET_SLOT,
            remote_feature_value.into(),
        )
        .await
        .unwrap();

    let (_api, handle) = spawn(
        NodeConfig::test()
            .with_networks(NetworkConfigs::with_hashkey())
            .with_eth_rpc_url(Some(source_handle.http_endpoint())),
    )
    .await;
    let provider = handle.http_provider();

    assert_eq!(provider.get_code_at(B20_FACTORY).await.unwrap(), remote_code);
    assert_eq!(
        provider.get_storage_at(B20_ACTIVATION_REGISTRY, FEATURE_ASSET_SLOT).await.unwrap(),
        remote_feature_value,
    );
    assert!(provider.get_code_at(B20_POLICY_REGISTRY).await.unwrap().is_empty());
}

#[cfg(feature = "cli")]
#[tokio::test(flavor = "multi_thread")]
async fn hashkey_cli_starts_with_b20_baseline() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let port_arg = port.to_string();

    let mut child = ChildGuard(
        Command::new(anvil_binary())
            .args(["--network", "hashkey", "--host", "127.0.0.1", "--port", &port_arg, "-q"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn anvil --network hashkey"),
    );

    let provider = http_provider(&format!("http://127.0.0.1:{port}"));
    let mut ready = false;
    for _ in 0..100 {
        if provider.get_chain_id().await.is_ok() {
            ready = true;
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("anvil exited before serving RPC: {status}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "anvil --network hashkey should start serving RPC");

    assert_hashkey_genesis_baseline(&provider).await;
}

#[cfg(feature = "cli")]
#[tokio::test(flavor = "multi_thread")]
async fn hashkey_cli_prints_profile_aware_b20_traces() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let port_arg = port.to_string();

    let mut child = ChildGuard(
        Command::new(anvil_binary())
            .args([
                "--network",
                "hashkey",
                "--host",
                "127.0.0.1",
                "--port",
                &port_arg,
                "--print-traces",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn anvil --network hashkey --print-traces"),
    );

    let provider = http_provider(&format!("http://127.0.0.1:{port}"));
    let mut ready = false;
    for _ in 0..100 {
        if provider.get_chain_id().await.is_ok() {
            ready = true;
            break;
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            panic!("anvil exited before serving RPC: {status}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "anvil --network hashkey should start serving RPC");

    let tx = TransactionRequest::default()
        .to(B20_FACTORY)
        .with_input(IB20Factory::isB20Call { token: Address::repeat_byte(0x11) }.abi_encode());
    provider.call(tx.into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let mut output = String::new();
    child.0.stdout.take().unwrap().read_to_string(&mut output).unwrap();
    child.0.stderr.take().unwrap().read_to_string(&mut output).unwrap();
    let trace = output[output.find("Traces=\n").expect("Anvil printed the trace")..].trim_end();
    snapbox::assert_data_eq!(
        trace,
        foundry_test_utils::str![[r#"
Traces=
  [12] B20Factory::isB20(0x1111111111111111111111111111111111111111)
    └─ ← [Return] false
"#]]
    );
}

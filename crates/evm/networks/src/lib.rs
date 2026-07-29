//! # foundry-evm-networks
//!
//! Foundry EVM network configuration.

use crate::celo::transfer::{
    CELO_TRANSFER_ADDRESS, CELO_TRANSFER_LABEL, PRECOMPILE_ID_CELO_TRANSFER,
};
use alloy_chains::{
    Chain, NamedChain,
    NamedChain::{Chiado, Gnosis, Moonbase, Moonbeam, MoonbeamDev, Moonriver, Rsk, RskTestnet},
};
use alloy_eips::eip1559::BaseFeeParams;
use alloy_evm::precompiles::{DynPrecompile, Precompile, PrecompilesMap};
use alloy_primitives::{Address, B256, ChainId, address, map::AddressHashMap};

#[cfg(feature = "hashkey")]
use alloy_primitives::{U256, b256};
use clap::Parser;
use foundry_evm_hardforks::{FoundryHardfork, TempoHardfork};
use revm::precompile::{
    Precompile as RevmPrecompile, PrecompileId,
    secp256r1::{P256VERIFY, P256VERIFY_OSAKA},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tempo_contracts::precompiles::{
    ACCOUNT_KEYCHAIN_ADDRESS, ADDRESS_REGISTRY_ADDRESS, CURRENT_COMMITTEE_ADDRESS,
    NONCE_PRECOMPILE_ADDRESS, RECEIVE_POLICY_GUARD_ADDRESS, SIGNATURE_VERIFIER_ADDRESS,
    STABLECOIN_DEX_ADDRESS, STORAGE_CREDITS_ADDRESS, TIP_FEE_MANAGER_ADDRESS,
    TIP20_CHANNEL_RESERVE_ADDRESS, TIP20_FACTORY_ADDRESS, TIP403_REGISTRY_ADDRESS,
    VALIDATOR_CONFIG_ADDRESS, VALIDATOR_CONFIG_V2_ADDRESS,
};

pub mod arbitrum;
pub mod celo;

#[cfg(feature = "optimism")]
mod optimism;

/// HashKey B20 standalone local development activation admin.
///
/// This is a deterministic non-zero address used only for standalone local simulation;
/// it is not a production HashKey parameter.
#[cfg(feature = "hashkey")]
pub const HSK_B20_LOCAL_ADMIN: Address = address!("CB00000000000000000000000000000000000000");

/// B20 singleton addresses.
#[cfg(feature = "hashkey")]
mod b20_addresses {
    use alloy_primitives::{Address, B256, address, b256};

    /// `B20Factory` singleton precompile address.
    pub const B20_FACTORY: Address = address!("B20F000000000000000000000000000000000000");
    /// `ActivationRegistry` singleton precompile address.
    pub const B20_ACTIVATION_REGISTRY: Address =
        address!("8453000000000000000000000000000000000001");
    /// `PolicyRegistry` singleton precompile address.
    pub const B20_POLICY_REGISTRY: Address = address!("8453000000000000000000000000000000000002");

    /// Canonical `keccak256([0xef])` code marker hash used by the B20 Factory.
    pub const B20_MARKER_CODE_HASH: B256 =
        b256!("309b8896ee4c1ff7ec1966155373dee42663b6b40c3fedc70ba501684848d2a3");

    /// ERC-7201 namespace root for the ActivationRegistry.
    ///
    /// Computed as `keccak256("base.activation.registry.storage") - 1` per the upstream
    /// derivation. See `optimism@149bcbfc:rust/b20/precompiles/src/activation/storage.rs`.
    #[cfg(test)]
    pub const ACTIVATION_REGISTRY_NS_ROOT: B256 =
        b256!("43ee1bbe25e988521cccd8b2c8fbd38c8287ebff8e074e825a70dfd3885cce00");

    /// Canonical feature IDs seeded to active in standalone local genesis.
    #[cfg(test)]
    pub const FEATURE_POLICY_REGISTRY: B256 =
        b256!("b582ebae03f16fee49a6763f78df482fb11ae73f103ed0d330bbe556aa90a43f");
    #[cfg(test)]
    pub const FEATURE_B20_STABLECOIN: B256 =
        b256!("ecfa0def2c10020caaf65e6155aa69c84b24892aaef76eeac52e0e2b3a0b8601");
    #[cfg(test)]
    pub const FEATURE_B20_ASSET: B256 =
        b256!("cdcc772fe4cbdb1029f822861176d09e646db96723d4c1e82ddfdeb8163ef54c");

    /// Computes the storage slot for a feature flag in the ActivationRegistry mapping.
    ///
    /// `keccak256(feature_id || ns_root)` where both are 32-byte big-endian.
    #[cfg(test)]
    pub fn feature_slot(feature_id: B256) -> B256 {
        use alloy_primitives::keccak256;
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(feature_id.as_slice());
        buf[32..].copy_from_slice(ACTIVATION_REGISTRY_NS_ROOT.as_slice());
        keccak256(buf)
    }
}

const TEMPO_PRECOMPILES: &[(&str, Address)] = &[
    ("Nonce", NONCE_PRECOMPILE_ADDRESS),
    ("StablecoinDex", STABLECOIN_DEX_ADDRESS),
    ("TIP20Factory", TIP20_FACTORY_ADDRESS),
    ("TIP403Registry", TIP403_REGISTRY_ADDRESS),
    ("FeeManager", TIP_FEE_MANAGER_ADDRESS),
    ("ValidatorConfig", VALIDATOR_CONFIG_ADDRESS),
    ("ValidatorConfigV2", VALIDATOR_CONFIG_V2_ADDRESS),
    ("AccountKeychain", ACCOUNT_KEYCHAIN_ADDRESS),
    ("SignatureVerifier", SIGNATURE_VERIFIER_ADDRESS),
    ("AddressRegistry", ADDRESS_REGISTRY_ADDRESS),
    ("TIP20ChannelReserve", TIP20_CHANNEL_RESERVE_ADDRESS),
    ("ReceivePolicyGuard", RECEIVE_POLICY_GUARD_ADDRESS),
    ("StorageCredits", STORAGE_CREDITS_ADDRESS),
    ("CurrentCommittee", CURRENT_COMMITTEE_ADDRESS),
];

/// BSC secp256r1 precompile address introduced by the Haber hardfork.
const BSC_P256_ADDRESS: Address = address!("0000000000000000000000000000000000000100");

const BSC_MAINNET_CHAIN_ID: u64 = 56;
const BSC_TESTNET_CHAIN_ID: u64 = 97;
const BSC_MAINNET_HABER_TIMESTAMP: u64 = 1_718_863_500;
const BSC_TESTNET_HABER_TIMESTAMP: u64 = 1_716_962_820;
const BSC_MAINNET_OSAKA_TIMESTAMP: u64 = 1_777_343_400;
const BSC_TESTNET_OSAKA_TIMESTAMP: u64 = 1_774_319_400;

/// Returns the BSC P256 precompile for the given timestamp. The outer option distinguishes BSC
/// chains from unrelated chains, while the inner option disables P256 before Haber.
const fn bsc_p256_precompile(chain_id: ChainId, timestamp: u64) -> Option<Option<RevmPrecompile>> {
    let (haber_timestamp, osaka_timestamp) = match chain_id {
        BSC_MAINNET_CHAIN_ID => (BSC_MAINNET_HABER_TIMESTAMP, BSC_MAINNET_OSAKA_TIMESTAMP),
        BSC_TESTNET_CHAIN_ID => (BSC_TESTNET_HABER_TIMESTAMP, BSC_TESTNET_OSAKA_TIMESTAMP),
        _ => return None,
    };

    if timestamp < haber_timestamp {
        Some(None)
    } else if timestamp < osaka_timestamp {
        Some(Some(P256VERIFY))
    } else {
        Some(Some(P256VERIFY_OSAKA))
    }
}

/// All well-known Tempo precompile addresses.
pub const TEMPO_PRECOMPILE_ADDRESSES: &[Address] = &[
    NONCE_PRECOMPILE_ADDRESS,
    STABLECOIN_DEX_ADDRESS,
    TIP20_FACTORY_ADDRESS,
    TIP403_REGISTRY_ADDRESS,
    TIP_FEE_MANAGER_ADDRESS,
    VALIDATOR_CONFIG_ADDRESS,
    VALIDATOR_CONFIG_V2_ADDRESS,
    ACCOUNT_KEYCHAIN_ADDRESS,
    SIGNATURE_VERIFIER_ADDRESS,
    ADDRESS_REGISTRY_ADDRESS,
    TIP20_CHANNEL_RESERVE_ADDRESS,
    RECEIVE_POLICY_GUARD_ADDRESS,
    STORAGE_CREDITS_ADDRESS,
    CURRENT_COMMITTEE_ADDRESS,
];

/// Returns whether a well-known Tempo precompile address is active at `hardfork`.
pub fn is_tempo_precompile_active_at(address: Address, hardfork: TempoHardfork) -> bool {
    if address == CURRENT_COMMITTEE_ADDRESS {
        hardfork.is_t8()
    } else if address == TIP20_CHANNEL_RESERVE_ADDRESS {
        hardfork.is_t5()
    } else if address == RECEIVE_POLICY_GUARD_ADDRESS {
        hardfork.is_t6()
    } else if address == STORAGE_CREDITS_ADDRESS {
        hardfork.is_t7()
    } else if address == ADDRESS_REGISTRY_ADDRESS || address == SIGNATURE_VERIFIER_ADDRESS {
        hardfork.is_t3()
    } else {
        true
    }
}

/// Returns the well-known Tempo precompile addresses active at `hardfork`.
pub fn active_tempo_precompile_addresses(hardfork: TempoHardfork) -> impl Iterator<Item = Address> {
    TEMPO_PRECOMPILE_ADDRESSES
        .iter()
        .copied()
        .filter(move |&address| is_tempo_precompile_active_at(address, hardfork))
}

fn active_tempo_precompiles(
    hardfork: Option<TempoHardfork>,
) -> impl Iterator<Item = (&'static str, Address)> {
    TEMPO_PRECOMPILES.iter().copied().filter(move |(_, address)| {
        hardfork.is_none_or(|hardfork| is_tempo_precompile_active_at(*address, hardfork))
    })
}

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum NetworkVariant {
    #[default]
    Ethereum,
    #[cfg(feature = "optimism")]
    Optimism,
    Tempo,
    #[cfg(feature = "hashkey")]
    HashKey,
}

impl std::str::FromStr for NetworkVariant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ethereum" => Ok(Self::Ethereum),
            #[cfg(feature = "optimism")]
            "optimism" => Ok(Self::Optimism),
            "tempo" => Ok(Self::Tempo),
            #[cfg(feature = "hashkey")]
            "hashkey" => Ok(Self::HashKey),
            _ => Err(format!("unknown network variant: {s}")),
        }
    }
}

impl NetworkVariant {
    /// Returns `true` if this is the Ethereum network variant.
    pub const fn is_ethereum(&self) -> bool {
        matches!(self, Self::Ethereum)
    }

    /// Returns `true` if this is the Optimism network variant.
    #[cfg(feature = "optimism")]
    pub const fn is_optimism(&self) -> bool {
        matches!(self, Self::Optimism)
    }

    /// Returns `true` if this is the Tempo network variant.
    pub const fn is_tempo(&self) -> bool {
        matches!(self, Self::Tempo)
    }

    /// Returns `true` if this is the HashKey network variant.
    #[cfg(feature = "hashkey")]
    pub const fn is_hashkey(&self) -> bool {
        matches!(self, Self::HashKey)
    }

    /// Returns the network variant name.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            #[cfg(feature = "optimism")]
            Self::Optimism => "optimism",
            Self::Tempo => "tempo",
            #[cfg(feature = "hashkey")]
            Self::HashKey => "hashkey",
        }
    }
}

impl std::fmt::Display for NetworkVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl From<ChainId> for NetworkVariant {
    fn from(chain_id: ChainId) -> Self {
        let chain = Chain::from_id(chain_id);
        if chain.is_tempo() {
            return Self::Tempo;
        }
        #[cfg(feature = "optimism")]
        if chain.is_optimism() {
            return Self::Optimism;
        }
        Self::Ethereum
    }
}

/// The base EVM semantics selected for a resolved network profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvmFamily {
    /// Canonical Ethereum execution semantics.
    #[default]
    Ethereum,
    /// OP Stack execution semantics.
    #[cfg(feature = "optimism")]
    Optimism,
    /// Tempo execution semantics.
    Tempo,
}

impl EvmFamily {
    /// Returns the family name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            #[cfg(feature = "optimism")]
            Self::Optimism => "optimism",
            Self::Tempo => "tempo",
        }
    }
}

/// The minimum runtime facts needed to project network-specific EVM semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkExecutionContext {
    /// Chain ID of the executing EVM.
    pub chain_id: ChainId,
    /// Timestamp fixed when the EVM is created.
    pub timestamp: u64,
}

impl NetworkExecutionContext {
    /// Creates a new execution context.
    pub const fn new(chain_id: ChainId, timestamp: u64) -> Self {
        Self { chain_id, timestamp }
    }
}

/// Canonical network-owned contract identity used by trace and debugger projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkTraceIdentity {
    /// HashKey B20 factory singleton.
    #[cfg(feature = "hashkey")]
    B20Factory,
    /// HashKey B20 activation registry singleton.
    #[cfg(feature = "hashkey")]
    B20ActivationRegistry,
    /// HashKey B20 policy registry singleton.
    #[cfg(feature = "hashkey")]
    B20PolicyRegistry,
    /// HashKey B20 Asset dynamic token.
    #[cfg(feature = "hashkey")]
    B20Asset,
    /// HashKey B20 Stablecoin dynamic token.
    #[cfg(feature = "hashkey")]
    B20Stablecoin,
}

impl NetworkTraceIdentity {
    /// Returns the stable user-facing trace label.
    pub const fn label(self) -> &'static str {
        match self {
            #[cfg(feature = "hashkey")]
            Self::B20Factory => "B20Factory",
            #[cfg(feature = "hashkey")]
            Self::B20ActivationRegistry => "B20ActivationRegistry",
            #[cfg(feature = "hashkey")]
            Self::B20PolicyRegistry => "B20PolicyRegistry",
            #[cfg(feature = "hashkey")]
            Self::B20Asset => "B20Asset",
            #[cfg(feature = "hashkey")]
            Self::B20Stablecoin => "B20Stablecoin",
        }
    }

    /// Returns the singleton address for fixed identities.
    pub const fn fixed_address(self) -> Option<Address> {
        #[cfg(feature = "hashkey")]
        {
            use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};

            return match self {
                Self::B20Factory => Some(B20_FACTORY),
                Self::B20ActivationRegistry => Some(B20_ACTIVATION_REGISTRY),
                Self::B20PolicyRegistry => Some(B20_POLICY_REGISTRY),
                Self::B20Asset | Self::B20Stablecoin => None,
            };
        }
        #[cfg(not(feature = "hashkey"))]
        match self {}
    }

    /// Returns all fixed network-owned trace identities.
    pub const fn fixed_identities() -> &'static [Self] {
        #[cfg(feature = "hashkey")]
        {
            const IDENTITIES: &[NetworkTraceIdentity] = &[
                NetworkTraceIdentity::B20Factory,
                NetworkTraceIdentity::B20ActivationRegistry,
                NetworkTraceIdentity::B20PolicyRegistry,
            ];
            return IDENTITIES;
        }
        #[cfg(not(feature = "hashkey"))]
        &[]
    }
}

/// State preparation selected by a resolved network profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetworkStatePlan {
    /// No profile-owned state preparation is required.
    #[default]
    None,
    /// Apply the existing Tempo state preparation path.
    Tempo,
    /// Apply the HashKey B20 development genesis state.
    #[cfg(feature = "hashkey")]
    HashKey,
}

/// A singleton marker account and its storage seeds for the B20 standalone local genesis.
#[cfg(feature = "hashkey")]
#[derive(Clone, Debug)]
pub struct B20GenesisAlloc {
    /// Singleton marker accounts: `(address, code_hash, nonce)`.
    pub markers: &'static [(Address, B256, u64)],
    /// ActivationRegistry feature flag storage: `(address, slot, value)`.
    pub feature_seeds: &'static [(Address, B256, U256)],
}

#[cfg(feature = "hashkey")]
impl B20GenesisAlloc {
    /// Returns the deterministic standalone local genesis alloc.
    ///
    /// B20 is active from timestamp `0`; the development admin is `HSK_B20_LOCAL_ADMIN`.
    /// Three singletons get `0xef` marker code; three canonical feature flags are seeded active.
    pub fn standalone_local() -> Self {
        use b20_addresses::{
            B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_MARKER_CODE_HASH, B20_POLICY_REGISTRY,
        };

        static MARKERS: [(Address, B256, u64); 3] = [
            (B20_FACTORY, B20_MARKER_CODE_HASH, 1),
            (B20_ACTIVATION_REGISTRY, B20_MARKER_CODE_HASH, 1),
            (B20_POLICY_REGISTRY, B20_MARKER_CODE_HASH, 1),
        ];

        // The feature mapping slots are keccak256(feature_id || ns_root).
        // These values are verified in the unit test
        // `feature_slot_derivation_matches_canonical_values`.
        static FEATURE_SEEDS: [(Address, B256, U256); 3] = [
            (
                B20_ACTIVATION_REGISTRY,
                b256!("8c5327ddcca092db72284503162323c6e8d392394b1d5c71991227bbc26f7c07"),
                U256::from_limbs([1, 0, 0, 0]),
            ),
            (
                B20_ACTIVATION_REGISTRY,
                b256!("ca7c276524c5aeaac4d56c8a3d36eb5f9a64f60841fb65b539c99c21ca7df109"),
                U256::from_limbs([1, 0, 0, 0]),
            ),
            (
                B20_ACTIVATION_REGISTRY,
                b256!("819420403a306232adb8ee78d9f35b5090371155b34376cf9b020e30029278e5"),
                U256::from_limbs([1, 0, 0, 0]),
            ),
        ];

        Self { markers: &MARKERS, feature_seeds: &FEATURE_SEEDS }
    }
}

/// Error returned when two network extensions claim the same singleton precompile address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecompileCompositionError {
    profile: &'static str,
    address: Address,
    existing: PrecompileId,
    requested: Option<PrecompileId>,
}

impl PrecompileCompositionError {
    /// Returns the profile that failed composition.
    pub const fn profile(&self) -> &'static str {
        self.profile
    }

    /// Returns the conflicting singleton address.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Returns the precompile already installed at the address.
    pub const fn existing(&self) -> &PrecompileId {
        &self.existing
    }

    /// Returns the precompile the profile requested, or `None` for removal.
    pub const fn requested(&self) -> Option<&PrecompileId> {
        self.requested.as_ref()
    }
}

impl std::fmt::Display for PrecompileCompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "network profile `{}` cannot compose precompile at {}: existing `{}` conflicts with ",
            self.profile,
            self.address,
            self.existing.name(),
        )?;
        if let Some(requested) = &self.requested {
            write!(f, "`{}`", requested.name())
        } else {
            f.write_str("removal")
        }
    }
}

impl std::error::Error for PrecompileCompositionError {}

/// Immutable runtime network semantics resolved from user configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResolvedNetworkProfile {
    family: EvmFamily,
    celo: bool,
    bypass_prevrandao: bool,
    #[cfg(feature = "hashkey")]
    hashkey: bool,
    #[cfg(feature = "hashkey")]
    b20_activation_time: Option<u64>,
    #[cfg(feature = "hashkey")]
    b20_activation_admin: Option<Address>,
}

impl ResolvedNetworkProfile {
    /// Returns the selected EVM family.
    pub const fn evm_family(self) -> EvmFamily {
        self.family
    }

    /// Returns the resolved profile name.
    pub const fn name(self) -> &'static str {
        #[cfg(feature = "hashkey")]
        if self.hashkey {
            return "hashkey";
        }
        if self.celo {
            return "celo";
        }
        self.family.name()
    }

    /// Returns whether the Celo extension is enabled.
    pub const fn is_celo(self) -> bool {
        self.celo
    }

    /// Returns whether Tempo semantics are selected.
    pub const fn is_tempo(self) -> bool {
        matches!(self.family, EvmFamily::Tempo)
    }

    /// Returns whether Optimism semantics are selected.
    #[cfg(feature = "optimism")]
    pub const fn is_optimism(self) -> bool {
        matches!(self.family, EvmFamily::Optimism)
    }

    /// Returns whether the HashKey B20 extension is enabled.
    #[cfg(feature = "hashkey")]
    pub const fn is_hashkey(self) -> bool {
        self.hashkey
    }

    /// Resolves an address to a network-owned trace identity for one activation snapshot.
    ///
    /// Dynamic B20 tokens are identified only from the canonical address variant. This does not
    /// read mutable token metadata or imply that an uninitialized structural address has code.
    pub fn trace_identity(
        self,
        address: Address,
        context: NetworkExecutionContext,
    ) -> Option<NetworkTraceIdentity> {
        #[cfg(feature = "hashkey")]
        if self.hashkey && self.b20_config().is_active_at(context.timestamp) {
            use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};
            use hsk_b20_precompiles::B20Variant;

            return match address {
                B20_FACTORY => Some(NetworkTraceIdentity::B20Factory),
                B20_ACTIVATION_REGISTRY => Some(NetworkTraceIdentity::B20ActivationRegistry),
                B20_POLICY_REGISTRY => Some(NetworkTraceIdentity::B20PolicyRegistry),
                address => match B20Variant::from_address(address) {
                    Some(B20Variant::Asset) => Some(NetworkTraceIdentity::B20Asset),
                    Some(B20Variant::Stablecoin) => Some(NetworkTraceIdentity::B20Stablecoin),
                    None => None,
                },
            };
        }
        let _ = (address, context);
        None
    }

    #[cfg(all(test, feature = "hashkey"))]
    fn with_b20_config(mut self, config: hsk_b20_config::B20Config) -> Self {
        self.b20_activation_time = config.activation_time();
        self.b20_activation_admin = config.activation_admin();
        self
    }

    /// Returns the B20 consensus configuration for standalone local development.
    #[cfg(feature = "hashkey")]
    pub fn b20_config(self) -> hsk_b20_config::B20Config {
        hsk_b20_config::B20Config::new(self.b20_activation_time, self.b20_activation_admin)
            .expect("resolved HashKey B20 config is valid")
    }

    /// Returns the B20 standalone genesis alloc for non-fork execution.
    ///
    /// Contains three singleton `0xef` marker accounts and three ActivationRegistry
    /// feature flag storage slots seeded to active. Returns `None` when the B20
    /// extension is not enabled.
    #[cfg(feature = "hashkey")]
    pub fn b20_genesis_alloc(self) -> Option<B20GenesisAlloc> {
        if !self.hashkey {
            return None;
        }
        Some(B20GenesisAlloc::standalone_local())
    }

    /// Returns the state preparation plan for this profile.
    pub const fn state_plan(self) -> NetworkStatePlan {
        #[cfg(feature = "hashkey")]
        if self.hashkey {
            return NetworkStatePlan::HashKey;
        }
        if self.is_tempo() { NetworkStatePlan::Tempo } else { NetworkStatePlan::None }
    }

    /// Returns whether `address` is protected from direct `vm.store` / `vm.etch`
    /// mutation under this profile's B20 standalone local semantics.
    ///
    /// Protected targets:
    /// - The three fixed B20 singleton precompiles (`B20Factory`, `ActivationRegistry`,
    ///   `PolicyRegistry`).
    /// - A canonical B20 dynamic token whose current account code hash equals the canonical `0xef`
    ///   marker, i.e. initialized by the Factory.
    ///
    /// Uninitialized `0xb2...` structural addresses are *not* protected: they behave
    /// as ordinary accounts until the Factory atomically initializes them, and revert
    /// of an initialized token's marker automatically restores that address to the
    /// unprotected state.
    ///
    /// `code_hash` is the current account's code hash (`KECCAK_EMPTY` for a non-existent
    /// account). Returns `false` for every address when the B20 extension is not enabled.
    #[cfg_attr(not(feature = "hashkey"), expect(clippy::missing_const_for_fn))]
    pub fn is_b20_protected(self, address: Address, code_hash: B256) -> bool {
        #[cfg(feature = "hashkey")]
        if self.hashkey {
            return self.is_b20_protected_inner(address, code_hash);
        }
        let _ = (address, code_hash);
        false
    }

    #[cfg(feature = "hashkey")]
    fn is_b20_protected_inner(self, address: Address, code_hash: B256) -> bool {
        use b20_addresses::{
            B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_MARKER_CODE_HASH, B20_POLICY_REGISTRY,
        };

        if matches!(address, B20_FACTORY | B20_ACTIVATION_REGISTRY | B20_POLICY_REGISTRY) {
            return true;
        }
        // An initialized dynamic token carries the canonical marker code hash at a
        // canonical variant address. Uninitialized `0xb2` addresses carry a different
        // (empty) code hash and remain unprotected.
        code_hash == B20_MARKER_CODE_HASH
            && hsk_b20_precompiles::B20Variant::from_address(address).is_some()
    }

    /// Returns the base fee parameters for this profile.
    #[cfg(feature = "optimism")]
    pub fn base_fee_params(self, timestamp: u64) -> BaseFeeParams {
        if self.is_optimism() {
            let op_hardforks = alloy_op_hardforks::OpChainHardforks::op_mainnet();
            if alloy_op_hardforks::OpHardforks::is_canyon_active_at_timestamp(
                &op_hardforks,
                timestamp,
            ) {
                return BaseFeeParams::optimism_canyon();
            }
            return BaseFeeParams::optimism();
        }
        BaseFeeParams::ethereum()
    }

    /// Returns the base fee parameters for this profile.
    #[cfg(not(feature = "optimism"))]
    pub const fn base_fee_params(self, timestamp: u64) -> BaseFeeParams {
        let _ = (self, timestamp);
        BaseFeeParams::ethereum()
    }

    /// Returns whether prevrandao should be bypassed for the executing chain.
    pub fn bypass_prevrandao(self, chain_id: u64) -> bool {
        if let Ok(
            Moonbeam | Moonbase | Moonriver | MoonbeamDev | Rsk | RskTestnet | Gnosis | Chiado,
        ) = NamedChain::try_from(chain_id)
        {
            return true;
        }
        self.bypass_prevrandao
    }

    /// Composes all profile and chain-specific precompiles for one EVM creation.
    pub fn inject_precompiles(
        self,
        precompiles: &mut PrecompilesMap,
        context: NetworkExecutionContext,
    ) -> Result<(), PrecompileCompositionError> {
        let p256verify = bsc_p256_precompile(context.chain_id, context.timestamp);

        if self.celo {
            self.ensure_compatible_precompile(
                precompiles,
                CELO_TRANSFER_ADDRESS,
                &PRECOMPILE_ID_CELO_TRANSFER,
                Some(&PRECOMPILE_ID_CELO_TRANSFER),
            )?;
        }
        if p256verify.is_some() {
            let requested = p256verify
                .as_ref()
                .and_then(|precompile| precompile.as_ref())
                .map(RevmPrecompile::id);
            self.ensure_compatible_precompile(
                precompiles,
                BSC_P256_ADDRESS,
                P256VERIFY.id(),
                requested,
            )?;
        }

        #[cfg(feature = "hashkey")]
        if self.hashkey {
            self.inject_b20_precompiles(precompiles, context)?;
        }

        if self.celo {
            precompiles.apply_precompile(&CELO_TRANSFER_ADDRESS, move |_| {
                Some(celo::transfer::precompile())
            });
        }
        if let Some(p256verify) = p256verify {
            precompiles.apply_precompile(&BSC_P256_ADDRESS, move |_| {
                p256verify.map(|p256verify| {
                    DynPrecompile::new(p256verify.id().clone(), move |input| {
                        p256verify.execute(input.data, input.gas, input.reservoir)
                    })
                })
            });
        }

        Ok(())
    }

    /// Installs B20 singletons and dynamic lookup when the activation snapshot is active.
    #[cfg(feature = "hashkey")]
    fn inject_b20_precompiles(
        self,
        precompiles: &mut PrecompilesMap,
        context: NetworkExecutionContext,
    ) -> Result<(), PrecompileCompositionError> {
        use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};

        let config = self.b20_config();
        if !config.is_active_at(context.timestamp) {
            return Ok(());
        }

        // Fail-closed singleton collision check.
        self.ensure_b20_singleton_free(precompiles, B20_FACTORY)?;
        self.ensure_b20_singleton_free(precompiles, B20_ACTIVATION_REGISTRY)?;
        self.ensure_b20_singleton_free(precompiles, B20_POLICY_REGISTRY)?;

        use hsk_b20_precompiles::{
            ActivationRegistry, B20Factory, B20Spec, BerylLookup, NoopPrecompileCallObserver,
            PolicyRegistryPrecompile,
        };

        B20Factory::install_with_observer(precompiles, B20Spec::Beryl, NoopPrecompileCallObserver);
        PolicyRegistryPrecompile::install(precompiles, B20Spec::Beryl);
        ActivationRegistry::install(precompiles, config.activation_admin());
        precompiles.map_precompile_lookup(|address, previous| {
            BerylLookup::lookup(address)
                .or_else(|| previous.and_then(|lookup| lookup.lookup(address)))
        });

        Ok(())
    }

    #[cfg(feature = "hashkey")]
    fn ensure_b20_singleton_free(
        self,
        precompiles: &PrecompilesMap,
        address: Address,
    ) -> Result<(), PrecompileCompositionError> {
        if let Some(existing) = precompiles.get(&address) {
            return Err(PrecompileCompositionError {
                profile: self.name(),
                address,
                existing: existing.precompile_id().clone(),
                requested: None,
            });
        }
        Ok(())
    }

    fn ensure_compatible_precompile(
        self,
        precompiles: &PrecompilesMap,
        address: Address,
        compatible: &PrecompileId,
        requested: Option<&PrecompileId>,
    ) -> Result<(), PrecompileCompositionError> {
        let Some(existing) = precompiles.get(&address) else { return Ok(()) };
        if existing.precompile_id() == compatible {
            return Ok(());
        }
        Err(PrecompileCompositionError {
            profile: self.name(),
            address,
            existing: existing.precompile_id().clone(),
            requested: requested.cloned(),
        })
    }

    /// Returns trace labels projected by this profile.
    pub fn precompile_labels(
        self,
        tempo_hardfork: Option<TempoHardfork>,
    ) -> AddressHashMap<String> {
        let mut labels = AddressHashMap::default();
        if self.celo {
            labels.insert(CELO_TRANSFER_ADDRESS, CELO_TRANSFER_LABEL.to_string());
        }
        #[cfg(feature = "hashkey")]
        if self.hashkey {
            use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};
            labels.insert(B20_FACTORY, "B20Factory".to_string());
            labels.insert(B20_ACTIVATION_REGISTRY, "B20ActivationRegistry".to_string());
            labels.insert(B20_POLICY_REGISTRY, "B20PolicyRegistry".to_string());
        }
        if self.is_tempo() {
            labels.extend(
                active_tempo_precompiles(tempo_hardfork)
                    .map(|(label, address)| (address, label.to_string())),
            );
        }
        labels
    }

    /// Returns the static precompile inventory projected by this profile.
    pub fn precompile_inventory(
        self,
        tempo_hardfork: Option<TempoHardfork>,
    ) -> BTreeMap<String, Address> {
        let mut precompiles = BTreeMap::new();
        if self.celo {
            precompiles
                .insert(PRECOMPILE_ID_CELO_TRANSFER.name().to_string(), CELO_TRANSFER_ADDRESS);
        }
        #[cfg(feature = "hashkey")]
        if self.hashkey {
            use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};
            precompiles.insert("B20Factory".to_string(), B20_FACTORY);
            precompiles.insert("B20ActivationRegistry".to_string(), B20_ACTIVATION_REGISTRY);
            precompiles.insert("B20PolicyRegistry".to_string(), B20_POLICY_REGISTRY);
        }
        if self.is_tempo() {
            precompiles.extend(
                active_tempo_precompiles(tempo_hardfork)
                    .map(|(label, address)| (label.to_string(), address)),
            );
        }
        precompiles
    }
}

#[derive(Clone, Debug, Default, Parser, Deserialize, Copy, PartialEq, Eq)]
pub struct NetworkConfigs {
    /// Enable a specific network family.
    #[arg(help_heading = "Networks", long, short, num_args = 1, value_name = "NETWORK", value_enum, conflicts_with_all = ["celo", "tempo"])]
    #[cfg_attr(feature = "optimism", arg(conflicts_with = "optimism"))]
    #[serde(default)]
    pub(crate) network: Option<NetworkVariant>,
    /// Enable Celo network features.
    #[arg(help_heading = "Networks", long, conflicts_with_all = ["network", "tempo"])]
    #[cfg_attr(feature = "optimism", arg(conflicts_with = "optimism"))]
    celo: bool,
    /// Enable Optimism network features (deprecated: use --network optimism).
    #[cfg(feature = "optimism")]
    #[arg(long, hide = true, conflicts_with_all = ["network", "celo", "tempo"])]
    // Deserialize-only legacy alias: accepted in foundry.toml but never serialized — the
    // canonical form is `network = "optimism"`.
    #[serde(default)]
    pub(crate) optimism: bool,
    /// Enable Tempo network features (deprecated: use --network tempo).
    #[arg(long, hide = true, conflicts_with_all = ["network", "celo"])]
    #[cfg_attr(feature = "optimism", arg(conflicts_with = "optimism"))]
    // Deserialize-only legacy alias: accepted in foundry.toml but never serialized — the
    // canonical form is `network = "tempo"`.
    #[serde(default)]
    tempo: bool,
    /// Whether to bypass prevrandao.
    #[arg(skip)]
    #[serde(default)]
    bypass_prevrandao: bool,
}

// Custom `Serialize` impl: always emits the *resolved* network as the canonical
// `network = "..."` field, and never emits the legacy `tempo` / `optimism` aliases. This avoids
// confusing output like `network = "tempo"` next to `tempo = false`, and ensures `tempo = true`
// in foundry.toml round-trips as `network = "tempo"`.
impl Serialize for NetworkConfigs {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("NetworkConfigs", 3)?;
        s.serialize_field("network", &self.resolved_network())?;
        s.serialize_field("celo", &self.celo)?;
        s.serialize_field("bypass_prevrandao", &self.bypass_prevrandao)?;
        s.end()
    }
}

impl NetworkConfigs {
    pub fn with_celo() -> Self {
        Self { celo: true, ..Default::default() }
    }

    pub fn with_tempo() -> Self {
        Self { network: Some(NetworkVariant::Tempo), tempo: true, ..Default::default() }
    }

    #[cfg(feature = "hashkey")]
    pub fn with_hashkey() -> Self {
        Self { network: Some(NetworkVariant::HashKey), ..Default::default() }
    }

    pub const fn is_tempo(&self) -> bool {
        if let Some(network) = self.resolved_network() { network.is_tempo() } else { false }
    }

    pub const fn is_celo(&self) -> bool {
        self.celo
    }

    /// Resolves user configuration into immutable runtime network semantics.
    pub const fn resolve(self) -> ResolvedNetworkProfile {
        let family = match self.resolved_network() {
            None | Some(NetworkVariant::Ethereum) => EvmFamily::Ethereum,
            #[cfg(feature = "optimism")]
            Some(NetworkVariant::Optimism) => EvmFamily::Optimism,
            #[cfg(feature = "hashkey")]
            Some(NetworkVariant::HashKey) => EvmFamily::Optimism,
            Some(NetworkVariant::Tempo) => EvmFamily::Tempo,
        };
        #[cfg(feature = "hashkey")]
        let hashkey = matches!(self.resolved_network(), Some(NetworkVariant::HashKey));
        ResolvedNetworkProfile {
            family,
            celo: self.celo,
            bypass_prevrandao: self.bypass_prevrandao,
            #[cfg(feature = "hashkey")]
            hashkey,
            #[cfg(feature = "hashkey")]
            b20_activation_time: if hashkey { Some(0) } else { None },
            #[cfg(feature = "hashkey")]
            b20_activation_admin: if hashkey { Some(HSK_B20_LOCAL_ADMIN) } else { None },
        }
    }

    /// Returns the resolved network variant, folding legacy flags.
    pub const fn resolved_network(&self) -> Option<NetworkVariant> {
        if let Some(n) = self.network {
            return Some(n);
        }
        #[cfg(feature = "optimism")]
        if self.optimism {
            return Some(NetworkVariant::Optimism);
        }
        if self.tempo {
            return Some(NetworkVariant::Tempo);
        }
        None
    }

    /// Returns the name of the currently active non-Ethereum network, or `None` for plain Ethereum.
    pub fn active_network_name(&self) -> Option<&'static str> {
        self.resolved_network().and_then(|n| match n {
            NetworkVariant::Ethereum => None,
            _ => Some(n.name()),
        })
    }

    pub fn with_chain_id(self, chain_id: u64) -> Self {
        let chain = Chain::from_id(chain_id);
        if self.resolved_network().is_some() {
            return if !self.celo
                && matches!(chain.named(), Some(NamedChain::Celo | NamedChain::CeloSepolia))
            {
                Self::with_celo()
            } else {
                self
            };
        }
        if chain.is_tempo() {
            return Self::with_tempo();
        }
        #[cfg(feature = "optimism")]
        if chain.is_optimism() {
            return Self::with_optimism();
        }
        self
    }

    /// Validates `hardfork` against the current `NetworkConfigs` and, if consistent, returns an
    /// updated instance with the network implied by the enabled hardfork.
    ///
    /// Returns `Err` when the hardfork's network family conflicts with the configured one.
    pub fn normalize_for_hardfork(self, hardfork: FoundryHardfork) -> Result<Self, String> {
        if let Some(configured) =
            self.active_network_name().filter(|&n| Some(n) != hardfork.namespace())
        {
            return Err(format!(
                "hardfork `{}` conflicts with network config `{configured}`",
                String::from(hardfork),
            ));
        }

        let network = match hardfork {
            FoundryHardfork::Ethereum(_) => self,
            FoundryHardfork::Tempo(_) => Self::with_tempo(),
            #[cfg(feature = "optimism")]
            FoundryHardfork::Optimism(_) => Self::with_optimism(),
        };

        Ok(network)
    }
}

impl From<NetworkVariant> for NetworkConfigs {
    fn from(network: NetworkVariant) -> Self {
        match network {
            NetworkVariant::Ethereum => Self::default(),
            NetworkVariant::Tempo => {
                Self { network: Some(network), tempo: true, ..Default::default() }
            }
            #[cfg(feature = "optimism")]
            NetworkVariant::Optimism => {
                Self { network: Some(network), optimism: true, ..Default::default() }
            }
            #[cfg(feature = "hashkey")]
            NetworkVariant::HashKey => Self { network: Some(network), ..Default::default() },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::precompile::{
        Precompiles,
        secp256r1::{P256VERIFY_BASE_GAS_FEE, P256VERIFY_BASE_GAS_FEE_OSAKA},
    };
    use std::borrow::Cow;

    const DYNAMIC_PRECOMPILE_ADDRESS: Address =
        address!("0000000000000000000000000000000000000bad");

    // --- Equivalence: new flag == legacy flag ---

    #[test]
    fn network_variant_predicates() {
        assert!(NetworkVariant::Ethereum.is_ethereum());
        assert!(!NetworkVariant::Ethereum.is_tempo());
        assert!(NetworkVariant::Tempo.is_tempo());
        assert!(!NetworkVariant::Tempo.is_ethereum());

        #[cfg(feature = "optimism")]
        {
            assert!(NetworkVariant::Optimism.is_optimism());
            assert!(!NetworkVariant::Optimism.is_ethereum());
            assert!(!NetworkVariant::Optimism.is_tempo());
        }
    }

    #[test]
    fn new_tempo_flag_equivalent_to_legacy() {
        let via_new = NetworkConfigs { network: Some(NetworkVariant::Tempo), ..Default::default() };
        let via_old = NetworkConfigs { tempo: true, ..Default::default() };
        assert_eq!(via_new.is_tempo(), via_old.is_tempo());
        assert_eq!(via_new.active_network_name(), via_old.active_network_name());
        assert_eq!(
            via_new.resolve().precompile_inventory(None),
            via_old.resolve().precompile_inventory(None)
        );
        assert_eq!(
            via_new.resolve().precompile_labels(None),
            via_old.resolve().precompile_labels(None)
        );
    }

    #[test]
    fn resolves_configuration_into_runtime_profile() {
        let ethereum = NetworkConfigs::default().resolve();
        assert_eq!(ethereum.evm_family(), EvmFamily::Ethereum);
        assert_eq!(ethereum.state_plan(), NetworkStatePlan::None);

        let celo = NetworkConfigs::with_celo().resolve();
        assert_eq!(celo.evm_family(), EvmFamily::Ethereum);
        assert!(celo.is_celo());
        assert_eq!(
            celo.precompile_inventory(None).get(PRECOMPILE_ID_CELO_TRANSFER.name()),
            Some(&CELO_TRANSFER_ADDRESS)
        );
        assert_eq!(
            celo.precompile_labels(None).get(&CELO_TRANSFER_ADDRESS),
            Some(&CELO_TRANSFER_LABEL.to_string())
        );

        let tempo = NetworkConfigs::with_tempo().resolve();
        assert_eq!(tempo.evm_family(), EvmFamily::Tempo);
        assert_eq!(tempo.state_plan(), NetworkStatePlan::Tempo);

        #[cfg(feature = "optimism")]
        assert_eq!(NetworkConfigs::with_optimism().resolve().evm_family(), EvmFamily::Optimism);
    }

    #[test]
    fn profile_precompile_composition_preserves_dynamic_lookup() {
        let dynamic_id = PrecompileId::Custom(Cow::Borrowed("dynamic-test"));
        let mut precompiles = PrecompilesMap::from_static(Precompiles::prague());
        precompiles.set_precompile_lookup({
            let dynamic_id = dynamic_id.clone();
            move |address: &Address| {
                (*address == DYNAMIC_PRECOMPILE_ADDRESS).then(|| {
                    DynPrecompile::new(dynamic_id.clone(), |_| unreachable!("not executed"))
                })
            }
        });

        NetworkConfigs::with_celo()
            .resolve()
            .inject_precompiles(&mut precompiles, NetworkExecutionContext::new(1, 0))
            .unwrap();

        assert_eq!(
            precompiles.get(&DYNAMIC_PRECOMPILE_ADDRESS).unwrap().precompile_id(),
            &dynamic_id
        );
        assert_eq!(
            precompiles.get(&CELO_TRANSFER_ADDRESS).unwrap().precompile_id(),
            &PRECOMPILE_ID_CELO_TRANSFER
        );
    }

    #[test]
    fn profile_precompile_composition_rejects_singleton_conflict() {
        let conflicting_id = PrecompileId::Custom(Cow::Borrowed("conflicting-test"));
        let mut precompiles = PrecompilesMap::from_static(Precompiles::prague());
        precompiles.apply_precompile(&CELO_TRANSFER_ADDRESS, {
            let conflicting_id = conflicting_id.clone();
            move |_| Some(DynPrecompile::new(conflicting_id, |_| unreachable!("not executed")))
        });

        let err = NetworkConfigs::with_celo()
            .resolve()
            .inject_precompiles(&mut precompiles, NetworkExecutionContext::new(1, 0))
            .unwrap_err();

        assert_eq!(err.address(), CELO_TRANSFER_ADDRESS);
        assert_eq!(err.existing(), &conflicting_id);
        assert_eq!(err.requested(), Some(&PRECOMPILE_ID_CELO_TRANSFER));
        assert_eq!(
            precompiles.get(&CELO_TRANSFER_ADDRESS).unwrap().precompile_id(),
            &conflicting_id
        );
    }

    fn bsc_p256_gas_used(chain_id: ChainId, timestamp: u64) -> Option<u64> {
        bsc_p256_precompile(chain_id, timestamp)
            .flatten()
            .map(|precompile| precompile.execute(&[], u64::MAX, 0).unwrap().gas_used)
    }

    fn assert_bsc_p256_boundaries(chain_id: ChainId, haber_timestamp: u64, osaka_timestamp: u64) {
        assert!(matches!(bsc_p256_precompile(chain_id, haber_timestamp - 1), Some(None)));
        assert_eq!(bsc_p256_gas_used(chain_id, haber_timestamp), Some(P256VERIFY_BASE_GAS_FEE));
        assert_eq!(bsc_p256_gas_used(chain_id, osaka_timestamp - 1), Some(P256VERIFY_BASE_GAS_FEE));
        assert_eq!(
            bsc_p256_gas_used(chain_id, osaka_timestamp),
            Some(P256VERIFY_BASE_GAS_FEE_OSAKA)
        );
    }

    #[test]
    fn selects_bsc_p256_at_mainnet_boundaries() {
        assert_bsc_p256_boundaries(
            BSC_MAINNET_CHAIN_ID,
            BSC_MAINNET_HABER_TIMESTAMP,
            BSC_MAINNET_OSAKA_TIMESTAMP,
        );
    }

    #[test]
    fn selects_bsc_p256_at_testnet_boundaries() {
        assert_bsc_p256_boundaries(
            BSC_TESTNET_CHAIN_ID,
            BSC_TESTNET_HABER_TIMESTAMP,
            BSC_TESTNET_OSAKA_TIMESTAMP,
        );
    }

    #[test]
    fn removes_bsc_p256_before_haber() {
        let mut precompiles = PrecompilesMap::from_static(Precompiles::osaka());
        assert!(precompiles.get(&BSC_P256_ADDRESS).is_some());
        NetworkConfigs::default()
            .resolve()
            .inject_precompiles(
                &mut precompiles,
                NetworkExecutionContext::new(BSC_MAINNET_CHAIN_ID, BSC_MAINNET_HABER_TIMESTAMP - 1),
            )
            .unwrap();
        assert!(precompiles.get(&BSC_P256_ADDRESS).is_none());
    }

    #[test]
    fn canonical_tempo_network_reports_precompiles() {
        let profile =
            NetworkConfigs { network: Some(NetworkVariant::Tempo), ..Default::default() }.resolve();

        assert_eq!(
            profile.precompile_inventory(None).get("TIP20ChannelReserve"),
            Some(&TIP20_CHANNEL_RESERVE_ADDRESS)
        );
        assert!(
            !profile
                .precompile_inventory(Some(TempoHardfork::T4))
                .contains_key("TIP20ChannelReserve")
        );
        assert!(
            !profile
                .precompile_inventory(Some(TempoHardfork::T4))
                .contains_key("ReceivePolicyGuard")
        );
        assert!(
            !profile.precompile_inventory(Some(TempoHardfork::T2)).contains_key("AddressRegistry")
        );
        assert!(
            !profile
                .precompile_inventory(Some(TempoHardfork::T2))
                .contains_key("SignatureVerifier")
        );
        assert_eq!(
            profile.precompile_inventory(Some(TempoHardfork::T3)).get("AddressRegistry"),
            Some(&ADDRESS_REGISTRY_ADDRESS)
        );
        assert_eq!(
            profile.precompile_inventory(Some(TempoHardfork::T3)).get("SignatureVerifier"),
            Some(&SIGNATURE_VERIFIER_ADDRESS)
        );
        assert_eq!(
            profile.precompile_labels(Some(TempoHardfork::T5)).get(&TIP20_CHANNEL_RESERVE_ADDRESS),
            Some(&"TIP20ChannelReserve".to_string())
        );
        assert!(profile.precompile_labels(None).contains_key(&TIP20_CHANNEL_RESERVE_ADDRESS));
        assert!(
            !profile
                .precompile_labels(Some(TempoHardfork::T5))
                .contains_key(&RECEIVE_POLICY_GUARD_ADDRESS)
        );
        assert!(
            profile
                .precompile_labels(Some(TempoHardfork::T6))
                .contains_key(&RECEIVE_POLICY_GUARD_ADDRESS)
        );
    }

    #[test]
    fn storage_credits_precompile_activates_at_t7() {
        assert!(!is_tempo_precompile_active_at(STORAGE_CREDITS_ADDRESS, TempoHardfork::T6));
        assert!(is_tempo_precompile_active_at(STORAGE_CREDITS_ADDRESS, TempoHardfork::T7));
        assert!(TEMPO_PRECOMPILE_ADDRESSES.contains(&STORAGE_CREDITS_ADDRESS));

        // The hardfork-filtered precompile map must honor the same T7 activation.
        let profile =
            NetworkConfigs { network: Some(NetworkVariant::Tempo), ..Default::default() }.resolve();
        assert!(
            !profile.precompile_inventory(Some(TempoHardfork::T6)).contains_key("StorageCredits")
        );
        assert!(
            profile.precompile_inventory(Some(TempoHardfork::T7)).contains_key("StorageCredits")
        );
    }

    #[test]
    fn current_committee_precompile_activates_at_t8() {
        assert!(!is_tempo_precompile_active_at(CURRENT_COMMITTEE_ADDRESS, TempoHardfork::T7));
        assert!(is_tempo_precompile_active_at(CURRENT_COMMITTEE_ADDRESS, TempoHardfork::T8));
        assert!(TEMPO_PRECOMPILE_ADDRESSES.contains(&CURRENT_COMMITTEE_ADDRESS));

        let profile =
            NetworkConfigs { network: Some(NetworkVariant::Tempo), ..Default::default() }.resolve();
        assert!(
            !profile.precompile_inventory(Some(TempoHardfork::T7)).contains_key("CurrentCommittee")
        );
        assert!(
            profile.precompile_inventory(Some(TempoHardfork::T8)).contains_key("CurrentCommittee")
        );
    }

    // --- resolved() / active_network_name ---

    #[test]
    fn active_network_name_tempo() {
        let cfg = NetworkConfigs::with_tempo();
        assert_eq!(cfg.active_network_name(), Some("tempo"));
    }

    #[test]
    fn active_network_name_default_is_none() {
        assert_eq!(NetworkConfigs::default().active_network_name(), None);
    }

    // --- Serde round-trip ---

    #[test]
    fn serde_roundtrip_tempo() {
        let original = NetworkConfigs::with_tempo();
        let json = serde_json::to_string(&original).unwrap();
        let restored: NetworkConfigs = serde_json::from_str(&json).unwrap();
        assert!(restored.is_tempo());
    }

    #[test]
    fn serde_legacy_tempo_bool_deserialized() {
        // Old foundry.toml format: `tempo = true`
        let json = r#"{"tempo": true, "celo": false, "bypass_prevrandao": false}"#;
        let cfg: NetworkConfigs = serde_json::from_str(json).unwrap();
        assert!(cfg.is_tempo());
    }

    #[test]
    fn serde_serializes_legacy_alias_as_canonical_network() {
        // Legacy `tempo = true` should serialize as the canonical `network = "tempo"`,
        // and the legacy `tempo` / `optimism` keys must not appear in the output.
        let cfg = NetworkConfigs { tempo: true, ..Default::default() };
        let json = serde_json::to_value(cfg).unwrap();
        assert_eq!(json["network"], serde_json::json!("tempo"));
        assert!(json.get("tempo").is_none(), "legacy `tempo` key should not be serialized");
        assert!(json.get("optimism").is_none(), "legacy `optimism` key should not be serialized");
    }

    #[test]
    fn serde_new_network_field_deserialized() {
        let json_tempo = r#"{"network": "tempo", "celo": false, "bypass_prevrandao": false}"#;
        let cfg_tempo: NetworkConfigs = serde_json::from_str(json_tempo).unwrap();
        assert!(cfg_tempo.is_tempo());
    }

    #[cfg(feature = "optimism")]
    mod optimism {
        use super::*;

        #[test]
        fn new_optimism_flag_equivalent_to_legacy() {
            let via_new =
                NetworkConfigs { network: Some(NetworkVariant::Optimism), ..Default::default() };
            let via_old = NetworkConfigs { optimism: true, ..Default::default() };
            assert_eq!(via_new.is_optimism(), via_old.is_optimism());
            assert_eq!(via_new.is_tempo(), via_old.is_tempo());
            assert_eq!(via_new.active_network_name(), via_old.active_network_name());
        }

        #[test]
        fn active_network_name_optimism() {
            let cfg = NetworkConfigs::with_optimism();
            assert_eq!(cfg.active_network_name(), Some("optimism"));
        }

        #[test]
        fn new_flag_wins_over_legacy_when_both_set() {
            // --network optimism --tempo: network field wins
            let cfg = NetworkConfigs {
                network: Some(NetworkVariant::Optimism),
                tempo: true,
                ..Default::default()
            };
            assert!(cfg.is_optimism());
            assert!(!cfg.is_tempo());
        }

        #[test]
        fn serde_roundtrip_optimism() {
            let original = NetworkConfigs::with_optimism();
            let json = serde_json::to_string(&original).unwrap();
            let restored: NetworkConfigs = serde_json::from_str(&json).unwrap();
            assert!(restored.is_optimism());
            assert!(!restored.is_tempo());
        }

        #[test]
        fn serde_optimism_field_deserialized() {
            let json_optimism =
                r#"{"network": "optimism", "celo": false, "bypass_prevrandao": false}"#;
            let cfg_optimism: NetworkConfigs = serde_json::from_str(json_optimism).unwrap();
            assert!(cfg_optimism.is_optimism());
        }
    }

    #[cfg(feature = "hashkey")]
    mod hashkey {
        use super::*;
        use alloy_primitives::{b256, keccak256};
        use b20_addresses::{B20_ACTIVATION_REGISTRY, B20_FACTORY, B20_POLICY_REGISTRY};

        #[test]
        fn resolves_to_optimism_family_with_b20_extension() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            assert!(profile.is_hashkey());
            assert!(profile.is_optimism());
            assert_eq!(profile.evm_family(), EvmFamily::Optimism);
            assert_eq!(profile.name(), "hashkey");
            assert_eq!(profile.state_plan(), NetworkStatePlan::HashKey);
        }

        #[test]
        fn b20_config_is_always_active_at_genesis() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let config = profile.b20_config();
            assert!(config.is_enabled());
            assert!(config.is_active_at(0));
            assert!(config.is_active_at(1));
            assert_eq!(config.activation_admin(), Some(HSK_B20_LOCAL_ADMIN));
        }

        #[test]
        fn trace_identity_follows_the_activation_snapshot() {
            let config =
                hsk_b20_config::B20Config::new(Some(100), Some(Address::repeat_byte(0x11)))
                    .unwrap();
            let profile = NetworkConfigs::with_hashkey().resolve().with_b20_config(config);
            let asset = hsk_b20_precompiles::B20Variant::Asset
                .compute_address(Address::repeat_byte(0x22), B256::repeat_byte(0x33))
                .0;

            assert_eq!(profile.trace_identity(asset, NetworkExecutionContext::new(177, 99)), None);
            assert_eq!(
                profile.trace_identity(asset, NetworkExecutionContext::new(177, 100)),
                Some(NetworkTraceIdentity::B20Asset)
            );
            assert_eq!(
                profile.trace_identity(asset, NetworkExecutionContext::new(177, 101)),
                Some(NetworkTraceIdentity::B20Asset)
            );
        }

        #[test]
        fn injects_b20_singletons_and_lookup() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let mut precompiles =
                PrecompilesMap::from_static(revm::precompile::Precompiles::prague());

            profile
                .inject_precompiles(&mut precompiles, NetworkExecutionContext::new(177, 0))
                .unwrap();

            assert!(precompiles.get(&B20_FACTORY).is_some());
            assert!(precompiles.get(&B20_ACTIVATION_REGISTRY).is_some());
            assert!(precompiles.get(&B20_POLICY_REGISTRY).is_some());
        }

        #[test]
        fn b20_lookup_composes_with_an_existing_dynamic_resolver() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let mut precompiles =
                PrecompilesMap::from_static(revm::precompile::Precompiles::prague());
            let custom_address = Address::repeat_byte(0x44);
            precompiles.set_precompile_lookup(move |address: &Address| {
                (*address == custom_address).then(|| {
                    DynPrecompile::new(
                        PrecompileId::Custom(std::borrow::Cow::Borrowed("custom-lookup")),
                        |_| unreachable!(),
                    )
                })
            });

            profile
                .inject_precompiles(&mut precompiles, NetworkExecutionContext::new(177, 0))
                .unwrap();
            let asset = hsk_b20_precompiles::B20Variant::Asset
                .compute_address(Address::repeat_byte(0x22), B256::repeat_byte(0x33))
                .0;

            assert_eq!(
                precompiles.get(&custom_address).unwrap().precompile_id().name(),
                "custom-lookup"
            );
            assert!(precompiles.get(&asset).is_some());
        }

        #[test]
        fn b20_injection_rejects_singleton_conflict() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let mut precompiles =
                PrecompilesMap::from_static(revm::precompile::Precompiles::prague());
            precompiles.apply_precompile(&B20_FACTORY, {
                move |_| {
                    Some(DynPrecompile::new(
                        PrecompileId::Custom(std::borrow::Cow::Borrowed("conflict-test")),
                        |_| unreachable!(),
                    ))
                }
            });

            let err = profile
                .inject_precompiles(&mut precompiles, NetworkExecutionContext::new(177, 0))
                .unwrap_err();
            assert_eq!(err.address(), B20_FACTORY);
            assert_eq!(err.requested(), None);
        }

        #[test]
        fn reports_b20_labels_and_inventory() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let labels = profile.precompile_labels(None);
            assert_eq!(labels.get(&B20_FACTORY), Some(&"B20Factory".to_string()));
            assert_eq!(
                labels.get(&B20_ACTIVATION_REGISTRY),
                Some(&"B20ActivationRegistry".to_string())
            );
            assert_eq!(labels.get(&B20_POLICY_REGISTRY), Some(&"B20PolicyRegistry".to_string()));

            let inventory = profile.precompile_inventory(None);
            assert_eq!(inventory.get("B20Factory"), Some(&B20_FACTORY));
            assert_eq!(inventory.get("B20ActivationRegistry"), Some(&B20_ACTIVATION_REGISTRY));
            assert_eq!(inventory.get("B20PolicyRegistry"), Some(&B20_POLICY_REGISTRY));
        }

        #[test]
        fn active_network_name_hashkey() {
            let cfg = NetworkConfigs::with_hashkey();
            assert_eq!(cfg.active_network_name(), Some("hashkey"));
        }

        #[test]
        fn serde_roundtrip_hashkey() {
            let original = NetworkConfigs::with_hashkey();
            let json = serde_json::to_string(&original).unwrap();
            let restored: NetworkConfigs = serde_json::from_str(&json).unwrap();
            let profile = restored.resolve();
            assert!(profile.is_hashkey());
            assert!(profile.is_optimism());
        }

        #[test]
        fn serde_hashkey_field_deserialized() {
            let json = r#"{"network": "hashkey", "celo": false, "bypass_prevrandao": false}"#;
            let cfg: NetworkConfigs = serde_json::from_str(json).unwrap();
            assert!(cfg.resolve().is_hashkey());
        }

        #[test]
        fn feature_slot_derivation_matches_canonical_values() {
            // The three canonical mapping slots verified against the upstream derivation
            // (see docs/research/hashkey-b20-local-state-contract.md §2).
            assert_eq!(
                b20_addresses::feature_slot(b20_addresses::FEATURE_POLICY_REGISTRY),
                b256!("8c5327ddcca092db72284503162323c6e8d392394b1d5c71991227bbc26f7c07")
            );
            assert_eq!(
                b20_addresses::feature_slot(b20_addresses::FEATURE_B20_STABLECOIN),
                b256!("ca7c276524c5aeaac4d56c8a3d36eb5f9a64f60841fb65b539c99c21ca7df109")
            );
            assert_eq!(
                b20_addresses::feature_slot(b20_addresses::FEATURE_B20_ASSET),
                b256!("819420403a306232adb8ee78d9f35b5090371155b34376cf9b020e30029278e5")
            );
        }

        #[test]
        fn genesis_alloc_has_three_markers_and_three_seeds() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            let alloc = profile.b20_genesis_alloc().unwrap();
            assert_eq!(alloc.markers.len(), 3);
            assert_eq!(alloc.feature_seeds.len(), 3);
            // All feature seeds must be active (value = 1).
            for &(_, _, value) in alloc.feature_seeds {
                assert_eq!(value, U256::from(1));
            }
            // All markers must have the canonical code hash and nonce 1.
            for &(_, code_hash, nonce) in alloc.markers {
                assert_eq!(code_hash, b20_addresses::B20_MARKER_CODE_HASH);
                assert_eq!(nonce, 1);
            }
        }

        #[test]
        fn genesis_alloc_is_none_without_hashkey() {
            let profile = ResolvedNetworkProfile::default();
            assert!(profile.b20_genesis_alloc().is_none());
        }

        #[test]
        fn normal_and_inspected_injection_produce_identical_precompile_ids() {
            // Normal and inspected execution both call inject_precompiles with the same
            // immutable profile and execution context. The resulting precompile identity at
            // each singleton address must be identical, which is the structural basis for
            // normal/inspected equivalence of B20 execution.
            let profile = NetworkConfigs::with_hashkey().resolve();
            let context = NetworkExecutionContext::new(177, 0);

            let mut normal = PrecompilesMap::from_static(revm::precompile::Precompiles::prague());
            profile.inject_precompiles(&mut normal, context).unwrap();

            let mut inspected =
                PrecompilesMap::from_static(revm::precompile::Precompiles::prague());
            profile.inject_precompiles(&mut inspected, context).unwrap();

            for &address in &[
                b20_addresses::B20_FACTORY,
                b20_addresses::B20_ACTIVATION_REGISTRY,
                b20_addresses::B20_POLICY_REGISTRY,
            ] {
                let normal_id = normal.get(&address).unwrap().precompile_id().clone();
                let inspected_id = inspected.get(&address).unwrap().precompile_id().clone();
                assert_eq!(normal_id, inspected_id, "precompile id mismatch at {address}");
            }
        }

        #[test]
        fn protection_is_noop_without_hashkey_profile() {
            // Non-hashkey profiles never protect any address, regardless of code hash.
            let profile = ResolvedNetworkProfile::default();
            assert!(
                !profile.is_b20_protected(
                    b20_addresses::B20_FACTORY,
                    b20_addresses::B20_MARKER_CODE_HASH
                )
            );
            assert!(!profile.is_b20_protected(Address::ZERO, B256::ZERO));
        }

        #[test]
        fn protection_blocks_fixed_singletons() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            // Singletons are protected regardless of code hash.
            for &address in &[
                b20_addresses::B20_FACTORY,
                b20_addresses::B20_ACTIVATION_REGISTRY,
                b20_addresses::B20_POLICY_REGISTRY,
            ] {
                assert!(
                    profile.is_b20_protected(address, B256::ZERO),
                    "singleton {address} should be protected"
                );
            }
        }

        #[test]
        fn protection_blocks_initialized_dynamic_token() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            // A canonical variant address carrying the marker code hash is initialized
            // and must be protected. Built from the same structural prefix the Factory uses.
            // b2 | 00..00 (9) | variant | tail(9 from keccak). Use Asset variant = 0x00.
            let mut bytes = [0u8; 20];
            bytes[0] = 0xb2;
            bytes[10] = 0x00; // Asset discriminant
            let tail = keccak256([0u8; 32]);
            bytes[11..].copy_from_slice(&tail.as_slice()[..9]);
            let token = Address::from(bytes);
            assert!(profile.is_b20_protected(token, b20_addresses::B20_MARKER_CODE_HASH));
        }

        #[test]
        fn protection_allows_uninitialized_dynamic_address() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            // A canonical variant address WITHOUT the marker is uninitialized and must
            // remain writable as an ordinary account.
            let mut bytes = [0u8; 20];
            bytes[0] = 0xb2;
            bytes[10] = 0x01; // Stablecoin discriminant
            let tail = keccak256([1u8; 32]);
            bytes[11..].copy_from_slice(&tail.as_slice()[..9]);
            let uninitialized = Address::from(bytes);
            assert!(!profile.is_b20_protected(uninitialized, B256::ZERO));
            // Non-canonical prefix (first byte != 0xb2) is never protected by B20.
            assert!(!profile.is_b20_protected(Address::ZERO, b20_addresses::B20_MARKER_CODE_HASH));
        }

        #[test]
        fn protection_allows_b2_prefix_with_unsupported_variant() {
            let profile = NetworkConfigs::with_hashkey().resolve();
            // Prefix byte 0xb2 with the required zero run but an unsupported variant
            // discriminant is not a canonical token address and stays unprotected even
            // if it somehow carried the marker hash.
            let mut bytes = [0u8; 20];
            bytes[0] = 0xb2;
            bytes[10] = 0x09; // unsupported variant
            let tail = keccak256([2u8; 32]);
            bytes[11..].copy_from_slice(&tail.as_slice()[..9]);
            let noncanonical = Address::from(bytes);
            assert!(!profile.is_b20_protected(noncanonical, b20_addresses::B20_MARKER_CODE_HASH));
        }
    }
}

//! Optimism-specific configuration helpers.

use crate::{NetworkConfigs, NetworkVariant};

impl NetworkConfigs {
    pub fn with_optimism() -> Self {
        Self { network: Some(NetworkVariant::Optimism), optimism: true, ..Default::default() }
    }

    pub const fn is_optimism(&self) -> bool {
        if let Some(network) = self.resolved_network() { network.is_optimism() } else { false }
    }
}

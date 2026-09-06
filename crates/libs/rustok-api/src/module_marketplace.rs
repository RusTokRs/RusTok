/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use serde::{Deserialize, Serialize};

/// Current operator-visible state for one configured federated module registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceRegistryStatus {
    #[serde(alias = "UNKNOWN")]
    Unknown,
    #[serde(alias = "READY")]
    Ready,
    #[serde(alias = "DEGRADED")]
    Degraded,
}

/// Bounded freshness evidence for one stable logical registry identity.
///
/// Endpoint URLs and provider errors are deliberately excluded so this DTO can
/// cross native, GraphQL, and headless transports without disclosing deployment
/// topology or untrusted remote response content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceRegistryFreshness {
    #[serde(alias = "registryId")]
    pub registry_id: String,
    pub status: MarketplaceRegistryStatus,
    #[serde(alias = "lastSuccessUnixMs")]
    pub last_success_unix_ms: Option<u64>,
    #[serde(alias = "consecutiveFailures")]
    pub consecutive_failures: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_serializes_without_endpoint_details() {
        let freshness = MarketplaceRegistryFreshness {
            registry_id: "community.eu".to_string(),
            status: MarketplaceRegistryStatus::Degraded,
            last_success_unix_ms: Some(1_725_000_000_000),
            consecutive_failures: 2,
        };

        let encoded = serde_json::to_value(&freshness).expect("registry freshness");
        assert_eq!(encoded["registry_id"], "community.eu");
        assert_eq!(encoded["status"], "degraded");
        assert!(encoded.get("url").is_none());
        assert!(encoded.get("error").is_none());
    }
}

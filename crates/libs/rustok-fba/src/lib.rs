//! # RusToK Fluid Backend Architecture (FBA)
//!
//! Shared Fluid Backend Architecture contracts for RusToK module boundaries.
//!
//! Provides the neutral descriptor types for backend topologies (embedded, remote, hybrid,
//! async companion), transport profiles (in-process, gRPC, HTTP, event), provider
//! descriptors, and consumer dependencies without coupling to concrete transport engines.
//!
//! ## Entry points
//!
//! - [`BackendTopology`]: Execution topologies for modular capabilities.
//! - [`TransportProfile`]: Transport profiles supported by a provider.
//! - [`CapabilityId`]: Identifies a capability by owning module, name, and semantic version.
//! - [`FbaProviderDescriptor`]: Provider capability registration descriptor.
//! - [`FbaConsumerDependency`]: Consumer dependency declaration with fallback modes.

use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod contract_tests;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendTopology {
    Embedded,
    Remote,
    Hybrid,
    AsyncCompanion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportProfile {
    InProcess,
    Grpc,
    Http,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityId {
    pub module: String,
    pub capability: String,
    pub version: String,
}

impl CapabilityId {
    pub fn new(
        module: impl Into<String>,
        capability: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            module: module.into(),
            capability: capability.into(),
            version: version.into(),
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.module.trim().is_empty() {
            return Err(PortError::validation(
                "FBA_INVALID_CAPABILITY",
                "capability module cannot be empty or whitespace",
            ));
        }
        if self.capability.trim().is_empty() {
            return Err(PortError::validation(
                "FBA_INVALID_CAPABILITY",
                "capability name cannot be empty or whitespace",
            ));
        }
        if self.version.trim().is_empty() {
            return Err(PortError::validation(
                "FBA_INVALID_CAPABILITY",
                "capability version cannot be empty or whitespace",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FbaCallContext {
    pub port: PortContext,
    pub policy: PortCallPolicy,
}

impl FbaCallContext {
    pub fn new(port: PortContext, policy: PortCallPolicy) -> Self {
        Self { port, policy }
    }
}

pub type FbaResult<T> = Result<T, PortError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FbaProviderDescriptor {
    pub id: CapabilityId,
    pub topology: BackendTopology,
    pub transports: Vec<TransportProfile>,
    pub degraded_modes: Vec<String>,
}

impl FbaProviderDescriptor {
    pub fn embedded(id: CapabilityId) -> Self {
        Self {
            id,
            topology: BackendTopology::Embedded,
            transports: vec![TransportProfile::InProcess],
            degraded_modes: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), PortError> {
        self.id.validate()?;
        if self.transports.is_empty() {
            return Err(PortError::validation(
                "FBA_INVALID_PROVIDER",
                "provider descriptor must declare at least one transport profile",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FbaConsumerDependency {
    pub consumer: String,
    pub provider: CapabilityId,
    pub required: bool,
    pub fallback_modes: Vec<String>,
}

impl FbaConsumerDependency {
    pub fn validate(&self) -> Result<(), PortError> {
        if self.consumer.trim().is_empty() {
            return Err(PortError::validation(
                "FBA_INVALID_CONSUMER",
                "consumer name cannot be empty or whitespace",
            ));
        }
        self.provider.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_id_roundtrips_through_json() {
        let cap = CapabilityId::new("rustok-commerce", "pricing", "1.0.0");
        assert_eq!(cap.module, "rustok-commerce");
        assert_eq!(cap.capability, "pricing");
        assert_eq!(cap.version, "1.0.0");

        let serialized = serde_json::to_string(&cap).expect("serialize capability id");
        let deserialized: CapabilityId =
            serde_json::from_str(&serialized).expect("deserialize capability id");
        assert_eq!(cap, deserialized);
    }

    #[test]
    fn backend_topology_and_transport_profile_use_snake_case() {
        assert_eq!(
            serde_json::to_string(&BackendTopology::AsyncCompanion).unwrap(),
            "\"async_companion\""
        );
        assert_eq!(
            serde_json::to_string(&BackendTopology::Embedded).unwrap(),
            "\"embedded\""
        );
        assert_eq!(
            serde_json::to_string(&TransportProfile::InProcess).unwrap(),
            "\"in_process\""
        );
        assert_eq!(
            serde_json::to_string(&TransportProfile::Grpc).unwrap(),
            "\"grpc\""
        );
    }

    #[test]
    fn fba_provider_descriptor_embedded_sets_in_process_transport() {
        let cap = CapabilityId::new("rustok-blog", "comments", "1.0.0");
        let descriptor = FbaProviderDescriptor::embedded(cap.clone());

        assert_eq!(descriptor.topology, BackendTopology::Embedded);
        assert_eq!(descriptor.transports, vec![TransportProfile::InProcess]);
        assert!(descriptor.degraded_modes.is_empty());
        assert_eq!(descriptor.id, cap);

        let json = serde_json::to_string(&descriptor).expect("serialize provider descriptor");
        let parsed: FbaProviderDescriptor =
            serde_json::from_str(&json).expect("deserialize provider descriptor");
        assert_eq!(descriptor, parsed);
    }

    #[test]
    fn fba_consumer_dependency_roundtrips() {
        let dep = FbaConsumerDependency {
            consumer: "rustok-storefront".to_string(),
            provider: CapabilityId::new("rustok-cart", "cart_service", "2.0.0"),
            required: true,
            fallback_modes: vec!["cached_read_only".to_string()],
        };

        let json = serde_json::to_string(&dep).expect("serialize consumer dependency");
        let parsed: FbaConsumerDependency =
            serde_json::from_str(&json).expect("deserialize consumer dependency");
        assert_eq!(dep, parsed);
    }

    #[test]
    fn validation_rejects_empty_identifiers_and_empty_transports() {
        let empty_module = CapabilityId::new("  ", "pricing", "1.0.0");
        assert!(empty_module.validate().is_err());

        let valid_cap = CapabilityId::new("rustok-commerce", "pricing", "1.0.0");
        assert!(valid_cap.validate().is_ok());

        let valid_desc = FbaProviderDescriptor::embedded(valid_cap.clone());
        assert!(valid_desc.validate().is_ok());

        let invalid_desc = FbaProviderDescriptor {
            id: valid_cap.clone(),
            topology: BackendTopology::Embedded,
            transports: Vec::new(),
            degraded_modes: Vec::new(),
        };
        assert!(invalid_desc.validate().is_err());

        let valid_dep = FbaConsumerDependency {
            consumer: "rustok-storefront".to_string(),
            provider: valid_cap.clone(),
            required: true,
            fallback_modes: vec![],
        };
        assert!(valid_dep.validate().is_ok());

        let invalid_dep = FbaConsumerDependency {
            consumer: "  ".to_string(),
            provider: valid_cap,
            required: true,
            fallback_modes: vec![],
        };
        assert!(invalid_dep.validate().is_err());
    }
}

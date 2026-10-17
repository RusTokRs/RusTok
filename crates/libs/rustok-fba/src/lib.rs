use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FbaConsumerDependency {
    pub consumer: String,
    pub provider: CapabilityId,
    pub required: bool,
    pub fallback_modes: Vec<String>,
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
}


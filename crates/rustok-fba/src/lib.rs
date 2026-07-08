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

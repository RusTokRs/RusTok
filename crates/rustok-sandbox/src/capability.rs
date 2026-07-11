use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{SandboxError, SandboxPolicy, SandboxResult, SandboxSubject};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityName(String);

impl CapabilityName {
    pub fn new(value: impl Into<String>) -> SandboxResult<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 96
            && value
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '_' | '.' | ':'));
        if !valid {
            return Err(SandboxError::InvalidRequest(format!(
                "invalid capability name `{value}`"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub name: CapabilityName,
    #[serde(default)]
    pub constraints: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityCall {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub capability: CapabilityName,
    pub operation: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    #[serde(default)]
    pub output: Value,
}

#[async_trait]
pub trait CapabilityBroker: Send + Sync {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse>;
}

#[derive(Clone)]
pub struct SandboxHost {
    policy: Arc<SandboxPolicy>,
    broker: Arc<dyn CapabilityBroker>,
}

impl SandboxHost {
    pub(crate) fn new(policy: Arc<SandboxPolicy>, broker: Arc<dyn CapabilityBroker>) -> Self {
        Self { policy, broker }
    }

    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    pub async fn invoke(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        let grant = self
            .policy
            .grant(&call.capability)
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        self.broker.invoke(call, grant).await
    }
}

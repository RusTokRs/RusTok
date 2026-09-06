use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IggyConnectorConfigurationSnapshot {
    pub active_mode: String,
    pub desired_mode: String,
    pub bundled_available: bool,
    pub external_addresses: Vec<String>,
    pub external_username: String,
    pub password_resolver: String,
    pub password_key: String,
    pub password_configured: bool,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
    pub configured: bool,
    pub configuration_error: Option<String>,
    pub restart_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IggyConnectorSettingsInput {
    pub mode: String,
    pub external_addresses: Vec<String>,
    pub external_username: String,
    pub password_resolver: String,
    pub password_key: String,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IggyConnectorUpdateOutcome {
    pub desired_mode: String,
    pub configured: bool,
    pub restart_required: bool,
}

#[async_trait]
pub trait IggyConnectorControl: Send + Sync {
    async fn configuration(&self) -> Result<IggyConnectorConfigurationSnapshot, String>;

    async fn update_configuration(
        &self,
        input: IggyConnectorSettingsInput,
        actor_id: Uuid,
        actor_tenant_id: Uuid,
    ) -> Result<IggyConnectorUpdateOutcome, String>;
}

#[derive(Clone)]
pub struct SharedIggyConnectorControl(pub Arc<dyn IggyConnectorControl>);

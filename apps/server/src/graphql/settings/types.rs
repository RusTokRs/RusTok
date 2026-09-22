use async_graphql::{InputObject, SimpleObject};
use rustok_iggy_connector::{IggyConnectorConfigurationSnapshot, IggyConnectorSettingsInput};

/// A single platform settings category and its JSON payload.
#[derive(Debug, Clone, SimpleObject)]
pub struct PlatformSettingsPayload {
    pub category: String,
    /// Settings serialised as a JSON string so clients can parse it dynamically.
    pub settings: String,
}

/// Input for updating a single category.
#[derive(Debug, Clone, InputObject)]
pub struct UpdatePlatformSettingsInput {
    pub category: String,
    /// Full replacement JSON string for the category settings.
    pub settings: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpdatePlatformSettingsPayload {
    pub success: bool,
    pub category: String,
    pub settings: String,
}

/// Global event delivery control plane. It is intentionally not tenant-scoped
/// and contains no Iggy credentials.
#[derive(Debug, Clone, SimpleObject)]
pub struct EventDeliveryConfigurationPayload {
    pub active_profile: String,
    pub desired_profile: String,
    pub iggy_mode: String,
    pub iggy_configured: bool,
    pub restart_required: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateEventDeliveryConfigurationInput {
    pub profile: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpdateEventDeliveryConfigurationPayload {
    pub desired_profile: String,
    pub restart_required: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct IggyConnectorConfigurationPayload {
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

impl From<IggyConnectorConfigurationSnapshot> for IggyConnectorConfigurationPayload {
    fn from(value: IggyConnectorConfigurationSnapshot) -> Self {
        Self {
            active_mode: value.active_mode,
            desired_mode: value.desired_mode,
            bundled_available: value.bundled_available,
            external_addresses: value.external_addresses,
            external_username: value.external_username,
            password_resolver: value.password_resolver,
            password_key: value.password_key,
            password_configured: value.password_configured,
            tls_enabled: value.tls_enabled,
            tls_domain: value.tls_domain,
            configured: value.configured,
            configuration_error: value.configuration_error,
            restart_required: value.restart_required,
        }
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateIggyConnectorConfigurationInput {
    pub mode: String,
    pub external_addresses: Vec<String>,
    pub external_username: String,
    pub password_resolver: String,
    pub password_key: String,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
}

impl From<UpdateIggyConnectorConfigurationInput> for IggyConnectorSettingsInput {
    fn from(value: UpdateIggyConnectorConfigurationInput) -> Self {
        Self {
            mode: value.mode,
            external_addresses: value.external_addresses,
            external_username: value.external_username,
            password_resolver: value.password_resolver,
            password_key: value.password_key,
            tls_enabled: value.tls_enabled,
            tls_domain: value.tls_domain,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpdateIggyConnectorConfigurationPayload {
    pub desired_mode: String,
    pub configured: bool,
    pub restart_required: bool,
}

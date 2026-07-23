use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IggyConnectorConfiguration {
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
#[serde(rename_all = "camelCase")]
pub struct IggyConnectorUpdate {
    pub desired_mode: String,
    pub configured: bool,
    pub restart_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IggyConnectorForm {
    pub mode: String,
    pub external_addresses: Vec<String>,
    pub external_username: String,
    pub password_resolver: String,
    pub password_key: String,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
}

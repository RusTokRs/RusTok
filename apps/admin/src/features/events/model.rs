use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventsStatusResponse {
    #[serde(rename = "eventsStatus")]
    pub events_status: EventsStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventsStatus {
    #[serde(rename = "configuredTransport")]
    pub configured_transport: String,
    #[serde(rename = "iggyMode")]
    pub iggy_mode: String,
    #[serde(rename = "relayIntervalMs")]
    pub relay_interval_ms: u64,
    #[serde(rename = "dlqEnabled")]
    pub dlq_enabled: bool,
    #[serde(rename = "maxAttempts")]
    pub max_attempts: i32,
    #[serde(rename = "pendingEvents")]
    pub pending_events: i64,
    #[serde(rename = "dlqEvents")]
    pub dlq_events: i64,
    #[serde(rename = "availableTransports")]
    pub available_transports: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformSettingsResponse {
    #[serde(rename = "platformSettings")]
    pub platform_settings: PlatformSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformSettingsPayload {
    pub settings: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateSettingsInput {
    pub category: String,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    pub update_platform_settings: UpdateSettingsPayload,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateSettingsPayload {
    pub success: bool,
}

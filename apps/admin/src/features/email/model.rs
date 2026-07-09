use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    pub update_platform_settings: UpdateSettingsPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSettingsPayload {
    pub success: bool,
}

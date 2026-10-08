use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CacheHealthPayload {
    #[serde(rename = "redisConfigured")]
    pub redis_configured: bool,
    #[serde(rename = "redisHealthy")]
    pub redis_healthy: bool,
    #[serde(rename = "redisError")]
    pub redis_error: Option<String>,
    pub backend: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GraphqlCacheHealthResponse {
    #[serde(rename = "cacheHealth")]
    pub cache_health: CacheHealthPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlatformSettingsData {
    pub category: String,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GraphqlPlatformSettingsResponse {
    #[serde(rename = "platformSettings")]
    pub platform_settings: PlatformSettingsData,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdatePlatformSettingsPayload {
    pub success: bool,
    pub category: String,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GraphqlUpdatePlatformSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    pub update_platform_settings: UpdatePlatformSettingsPayload,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CacheHealthResponse {
    #[serde(rename = "cacheHealth")]
    pub cache_health: CacheHealthPayload,
}

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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CacheSettings {
    #[serde(default = "default_cache_mode")]
    pub mode: String,
    #[serde(default)]
    pub redis_url: Option<String>,
    #[serde(default = "default_redis_host")]
    pub redis_host: String,
    #[serde(default = "default_redis_port")]
    pub redis_port: u16,
    #[serde(default)]
    pub redis_password: Option<String>,
    #[serde(default)]
    pub redis_db: u32,
}

fn default_cache_mode() -> String {
    "in-memory".to_string()
}

fn default_redis_host() -> String {
    "127.0.0.1".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            mode: default_cache_mode(),
            redis_url: None,
            redis_host: default_redis_host(),
            redis_port: default_redis_port(),
            redis_password: None,
            redis_db: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlatformSettingsPayload {
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlatformSettingsResponse {
    #[serde(rename = "platformSettings")]
    pub platform_settings: PlatformSettingsPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateSettingsInput {
    pub category: String,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateSettingsPayload {
    pub success: bool,
    pub category: String,
    pub settings: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdateSettingsResponse {
    #[serde(rename = "updatePlatformSettings")]
    pub update_platform_settings: UpdateSettingsPayload,
}

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

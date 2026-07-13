#[cfg(feature = "redis-cache")]
use std::time::Duration;

use crate::CacheService;

#[cfg(feature = "redis-cache")]
const REDIS_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_REDIS_STATUS_ERROR_BYTES: usize = 512;

/// Precise Redis lifecycle status. Configuration, client construction and live connectivity are
/// deliberately separate so an invalid URL or failed client initialization cannot appear as
/// "Redis not configured" or healthy memory-only operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedisCacheStatus {
    pub url_present: bool,
    pub client_initialized: bool,
    pub connectivity_healthy: bool,
    pub last_error: Option<String>,
}

impl RedisCacheStatus {
    pub fn is_healthy(&self) -> bool {
        !self.url_present || (self.client_initialized && self.connectivity_healthy)
    }

    pub fn is_degraded(&self) -> bool {
        self.url_present && !self.is_healthy()
    }
}

/// Render lifecycle metrics without URL, host or error labels.
pub fn format_redis_cache_status_prometheus_metrics(status: &RedisCacheStatus) -> String {
    format!(
        "rustok_cache_redis_url_present {url_present}\n\
         rustok_cache_redis_client_initialized {client_initialized}\n\
         rustok_cache_redis_connectivity_healthy {connectivity_healthy}\n\
         rustok_cache_redis_degraded {degraded}\n",
        url_present = u8::from(status.url_present),
        client_initialized = u8::from(status.client_initialized),
        connectivity_healthy = u8::from(status.connectivity_healthy),
        degraded = u8::from(status.is_degraded()),
    )
}

impl CacheService {
    pub fn redis_configuration_present(&self) -> bool {
        self.redis_url().is_some()
    }

    pub fn redis_client_initialized(&self) -> bool {
        self.has_redis()
    }

    pub async fn redis_status(&self) -> RedisCacheStatus {
        let url_present = self.redis_configuration_present();
        let client_initialized = self.redis_client_initialized();
        if !url_present {
            return RedisCacheStatus {
                url_present: false,
                client_initialized: false,
                connectivity_healthy: false,
                last_error: None,
            };
        }
        if !client_initialized {
            return RedisCacheStatus {
                url_present: true,
                client_initialized: false,
                connectivity_healthy: false,
                last_error: Some(
                    "Redis URL is configured but the client could not be initialized".to_string(),
                ),
            };
        }

        #[cfg(feature = "redis-cache")]
        {
            let Some(client) = self.redis_client() else {
                return RedisCacheStatus {
                    url_present: true,
                    client_initialized: false,
                    connectivity_healthy: false,
                    last_error: Some(
                        "Redis client disappeared after initialization".to_string(),
                    ),
                };
            };
            let connection = tokio::time::timeout(
                REDIS_STATUS_TIMEOUT,
                client.get_multiplexed_async_connection(),
            )
            .await;
            let mut connection = match connection {
                Ok(Ok(connection)) => connection,
                Ok(Err(error)) => {
                    return degraded_status(format!("Redis connection failed: {error}"));
                }
                Err(_) => {
                    return degraded_status(format!(
                        "Redis connection timed out after {} ms",
                        REDIS_STATUS_TIMEOUT.as_millis()
                    ));
                }
            };
            match tokio::time::timeout(
                REDIS_STATUS_TIMEOUT,
                redis::cmd("PING").query_async::<String>(&mut connection),
            )
            .await
            {
                Ok(Ok(response)) if response == "PONG" => RedisCacheStatus {
                    url_present: true,
                    client_initialized: true,
                    connectivity_healthy: true,
                    last_error: None,
                },
                Ok(Ok(response)) => degraded_status(format!(
                    "Redis PING returned an unexpected response: {}",
                    bounded_error(response)
                )),
                Ok(Err(error)) => degraded_status(format!("Redis PING failed: {error}")),
                Err(_) => degraded_status(format!(
                    "Redis PING timed out after {} ms",
                    REDIS_STATUS_TIMEOUT.as_millis()
                )),
            }
        }

        #[cfg(not(feature = "redis-cache"))]
        {
            RedisCacheStatus {
                url_present,
                client_initialized,
                connectivity_healthy: false,
                last_error: Some(
                    "Redis URL is configured but rustok-cache was built without redis-cache"
                        .to_string(),
                ),
            }
        }
    }

    pub async fn redis_status_prometheus_metrics(&self) -> String {
        format_redis_cache_status_prometheus_metrics(&self.redis_status().await)
    }
}

#[cfg(feature = "redis-cache")]
fn degraded_status(error: String) -> RedisCacheStatus {
    RedisCacheStatus {
        url_present: true,
        client_initialized: true,
        connectivity_healthy: false,
        last_error: Some(bounded_error(error)),
    }
}

fn bounded_error(error: String) -> String {
    if error.len() <= MAX_REDIS_STATUS_ERROR_BYTES {
        return error;
    }
    let mut boundary = MAX_REDIS_STATUS_ERROR_BYTES;
    while !error.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}…", &error[..boundary])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_redis_configuration_is_healthy_memory_only_status() {
        let status = RedisCacheStatus::default();
        assert!(!status.url_present);
        assert!(!status.client_initialized);
        assert!(!status.connectivity_healthy);
        assert!(status.last_error.is_none());
        assert!(status.is_healthy());
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn invalid_redis_url_is_configured_but_not_initialized() {
        let service = CacheService::from_url(Some("://invalid-redis-url"));
        let status = service.redis_status().await;
        assert!(status.url_present);
        assert!(!status.client_initialized);
        assert!(!status.connectivity_healthy);
        assert!(status.is_degraded());
        assert!(status
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("could not be initialized"));
    }

    #[test]
    fn redis_metrics_are_label_free_and_separate_lifecycle_phases() {
        let metrics = format_redis_cache_status_prometheus_metrics(&RedisCacheStatus {
            url_present: true,
            client_initialized: false,
            connectivity_healthy: false,
            last_error: Some("redacted".to_string()),
        });
        assert!(metrics.contains("rustok_cache_redis_url_present 1"));
        assert!(metrics.contains("rustok_cache_redis_client_initialized 0"));
        assert!(metrics.contains("rustok_cache_redis_connectivity_healthy 0"));
        assert!(metrics.contains("rustok_cache_redis_degraded 1"));
        assert!(!metrics.contains("redacted"));
        assert!(!metrics.contains('{'));
    }

    #[test]
    fn redis_error_text_is_bounded_on_utf8_boundary() {
        let bounded = bounded_error("é".repeat(MAX_REDIS_STATUS_ERROR_BYTES));
        assert!(bounded.len() <= MAX_REDIS_STATUS_ERROR_BYTES + '…'.len_utf8());
        assert!(bounded.is_char_boundary(bounded.len()));
    }
}

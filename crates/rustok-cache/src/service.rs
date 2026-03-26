use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "redis-cache")]
use rustok_core::RedisCacheBackend;
use rustok_core::{CacheBackend, FallbackCacheBackend, InMemoryCacheBackend};

/// Shared cache service providing backend creation from a centralized Redis connection.
///
/// Other modules (tenant, RBAC, rate-limit) call `CacheService::backend()` instead of
/// resolving Redis URLs themselves. This keeps Redis lifecycle in one place.
#[derive(Clone)]
pub struct CacheService {
    #[cfg(feature = "redis-cache")]
    redis_url: Option<String>,
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
}

impl CacheService {
    /// Build from environment variables (`RUSTOK_REDIS_URL` / `REDIS_URL`).
    pub fn from_env() -> Self {
        Self::from_url(None)
    }

    /// Build from an explicit URL, falling back to env vars when `url` is `None`.
    ///
    /// Priority: `url` argument → `RUSTOK_REDIS_URL` → `REDIS_URL`.
    /// Pass `Some(url)` to override env vars (e.g. from `settings.rustok.cache.redis_url`).
    #[cfg(feature = "redis-cache")]
    pub fn from_url(url: Option<&str>) -> Self {
        let redis_url = url
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(resolve_redis_url);
        let redis_client = redis_url
            .as_ref()
            .and_then(|u| redis::Client::open(u.as_str()).ok());
        Self {
            redis_url,
            redis_client,
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url(_url: Option<&str>) -> Self {
        Self {}
    }

    /// Returns `true` if a Redis connection is available.
    pub fn has_redis(&self) -> bool {
        #[cfg(feature = "redis-cache")]
        {
            self.redis_client.is_some()
        }
        #[cfg(not(feature = "redis-cache"))]
        {
            false
        }
    }

    /// Returns the resolved Redis URL, if any.
    pub fn redis_url(&self) -> Option<&str> {
        #[cfg(feature = "redis-cache")]
        {
            self.redis_url.as_deref()
        }
        #[cfg(not(feature = "redis-cache"))]
        {
            None
        }
    }

    /// Returns a reference to the underlying Redis client, if available.
    #[cfg(feature = "redis-cache")]
    pub fn redis_client(&self) -> Option<&redis::Client> {
        self.redis_client.as_ref()
    }

    /// Create a cache backend with the given prefix, TTL, and capacity.
    ///
    /// If Redis is available, returns a `FallbackCacheBackend` (Redis primary + in-memory fallback).
    /// Otherwise returns a pure in-memory backend.
    pub async fn backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
    ) -> Arc<dyn CacheBackend> {
        #[cfg(feature = "redis-cache")]
        if let Some(url) = &self.redis_url {
            if let Ok(redis_backend) = RedisCacheBackend::new(url, prefix, ttl).await {
                let memory = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
                return Arc::new(FallbackCacheBackend::new(Arc::new(redis_backend), memory));
            }
        }

        Arc::new(InMemoryCacheBackend::new(ttl, max_capacity))
    }

    /// Create a pure in-memory backend (no Redis).
    pub fn memory_backend(&self, ttl: Duration, max_capacity: u64) -> Arc<dyn CacheBackend> {
        Arc::new(InMemoryCacheBackend::new(ttl, max_capacity))
    }

    /// Health check: verify Redis connectivity (if configured).
    pub async fn health(&self) -> CacheHealthReport {
        let mut report = CacheHealthReport {
            redis_configured: self.has_redis(),
            redis_healthy: false,
            redis_error: None,
        };

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    let pong: redis::RedisResult<String> =
                        redis::cmd("PING").query_async(&mut conn).await;
                    match pong {
                        Ok(ref s) if s == "PONG" => {
                            report.redis_healthy = true;
                        }
                        Ok(s) => {
                            report.redis_error = Some(format!("unexpected PING response: {s}"));
                        }
                        Err(e) => {
                            report.redis_error = Some(e.to_string());
                        }
                    }
                }
                Err(e) => {
                    report.redis_error = Some(e.to_string());
                }
            }
        }

        report
    }
}

#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub redis_configured: bool,
    pub redis_healthy: bool,
    pub redis_error: Option<String>,
}

impl CacheHealthReport {
    pub fn is_healthy(&self) -> bool {
        !self.redis_configured || self.redis_healthy
    }
}

#[cfg(feature = "redis-cache")]
fn resolve_redis_url() -> Option<String> {
    std::env::var("RUSTOK_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .filter(|url| !url.trim().is_empty())
}

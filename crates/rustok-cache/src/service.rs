use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_core::{CacheBackend, CacheStats, FallbackCacheBackend, InMemoryCacheBackend};
#[cfg(feature = "redis-cache")]
use rustok_core::{CircuitBreakerConfig, RedisCacheBackend};

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
    default_backend_options: CacheBackendOptions,
}

/// Backend construction options used by `CacheService`.
///
/// Defaults preserve the historical contract: Redis primary with in-memory fallback when
/// Redis is configured, pure in-memory otherwise, and lightweight in-process statistics
/// enabled for every returned backend.
#[derive(Debug, Clone)]
pub struct CacheBackendOptions {
    pub metrics_enabled: bool,
    #[cfg(feature = "redis-cache")]
    pub redis_circuit_breaker: CircuitBreakerConfig,
}

impl Default for CacheBackendOptions {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            #[cfg(feature = "redis-cache")]
            redis_circuit_breaker: CircuitBreakerConfig::default(),
        }
    }
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
        Self::from_url_with_options(url, CacheBackendOptions::default())
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url(_url: Option<&str>) -> Self {
        Self::from_url_with_options(_url, CacheBackendOptions::default())
    }

    /// Build from an explicit URL and service-wide backend defaults.
    #[cfg(feature = "redis-cache")]
    pub fn from_url_with_options(url: Option<&str>, options: CacheBackendOptions) -> Self {
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
            default_backend_options: options,
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url_with_options(_url: Option<&str>, options: CacheBackendOptions) -> Self {
        Self {
            default_backend_options: options,
        }
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

    /// Returns the default backend options used by `backend()`.
    pub fn default_backend_options(&self) -> &CacheBackendOptions {
        &self.default_backend_options
    }

    /// Returns a reference to the underlying Redis client, if available.
    #[cfg(feature = "redis-cache")]
    pub fn redis_client(&self) -> Option<&redis::Client> {
        self.redis_client.as_ref()
    }

    /// Create a cache backend with the given prefix, TTL, and capacity.
    ///
    /// If Redis is available, returns a `FallbackCacheBackend` (Redis primary + in-memory fallback).
    /// Otherwise returns a pure in-memory backend. All returned backends are instrumented by default
    /// so `CacheBackend::stats()` exposes hits, misses, invalidations, and current entries.
    pub async fn backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
    ) -> Arc<dyn CacheBackend> {
        self.backend_with_options(
            prefix,
            ttl,
            max_capacity,
            self.default_backend_options.clone(),
        )
        .await
    }

    /// Create a cache backend with per-call construction options.
    pub async fn backend_with_options(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        let backend = self.raw_backend(prefix, ttl, max_capacity, &options).await;
        if options.metrics_enabled {
            Arc::new(InstrumentedCacheBackend::new(prefix, backend))
        } else {
            backend
        }
    }

    async fn raw_backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: &CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        #[cfg(feature = "redis-cache")]
        if let Some(url) = &self.redis_url {
            if let Ok(redis_backend) = RedisCacheBackend::with_circuit_breaker(
                url,
                prefix,
                ttl,
                options.redis_circuit_breaker.clone(),
            )
            .await
            {
                let memory = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
                return Arc::new(FallbackCacheBackend::new(Arc::new(redis_backend), memory));
            }
        }

        Arc::new(InMemoryCacheBackend::new(ttl, max_capacity))
    }

    /// Create a pure in-memory backend (no Redis).
    pub fn memory_backend(&self, ttl: Duration, max_capacity: u64) -> Arc<dyn CacheBackend> {
        let backend = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
        if self.default_backend_options.metrics_enabled {
            Arc::new(InstrumentedCacheBackend::new("memory", backend))
        } else {
            backend
        }
    }

    /// Health check: verify Redis connectivity (if configured).
    pub async fn health(&self) -> CacheHealthReport {
        let mut report = CacheHealthReport {
            redis_configured: self.has_redis(),
            redis_healthy: false,
            redis_error: None,
            metrics_enabled: self.default_backend_options.metrics_enabled,
            #[cfg(feature = "redis-cache")]
            redis_circuit_breaker_failure_threshold: self
                .default_backend_options
                .redis_circuit_breaker
                .failure_threshold,
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
    pub metrics_enabled: bool,
    #[cfg(feature = "redis-cache")]
    pub redis_circuit_breaker_failure_threshold: u32,
}

impl CacheHealthReport {
    pub fn is_healthy(&self) -> bool {
        !self.redis_configured || self.redis_healthy
    }
}

struct InstrumentedCacheBackend {
    name: String,
    inner: Arc<dyn CacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl InstrumentedCacheBackend {
    fn new(name: impl Into<String>, inner: Arc<dyn CacheBackend>) -> Self {
        Self {
            name: name.into(),
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl CacheBackend for InstrumentedCacheBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.inner.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        let value = self.inner.get(key).await?;
        if value.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        Ok(value)
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.inner.set(key, value).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        self.inner.set_with_ttl(key, value, ttl).await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.inner.invalidate(key).await
    }

    fn stats(&self) -> CacheStats {
        let inner = self.inner.stats();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed).saturating_add(inner.hits),
            misses: self
                .misses
                .load(Ordering::Relaxed)
                .saturating_add(inner.misses),
            evictions: self
                .evictions
                .load(Ordering::Relaxed)
                .saturating_add(inner.evictions),
            entries: inner.entries,
        }
    }
}

impl Drop for InstrumentedCacheBackend {
    fn drop(&mut self) {
        tracing::debug!(cache = %self.name, stats = ?self.stats(), "cache backend dropped");
    }
}

#[cfg(feature = "redis-cache")]
fn resolve_redis_url() -> Option<String> {
    std::env::var("RUSTOK_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .filter(|url| !url.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn instrumented_backend_tracks_hits_misses_and_invalidations() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);

        assert!(backend.get("missing").await.unwrap().is_none());
        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );
        backend.invalidate("present").await.unwrap();

        let stats = backend.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 0);
    }

    #[tokio::test]
    async fn backend_options_can_disable_instrumentation() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_with_options(
                "test-uninstrumented",
                Duration::from_secs(60),
                16,
                CacheBackendOptions {
                    metrics_enabled: false,
                    #[cfg(feature = "redis-cache")]
                    redis_circuit_breaker: CircuitBreakerConfig::default(),
                },
            )
            .await;

        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );

        let stats = backend.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.entries, 1);
    }
}

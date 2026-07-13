use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use moka::future::Cache;
use moka::Expiry;

use crate::context::CacheBackend;
#[cfg(feature = "redis-cache")]
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
use crate::Result;

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: u64,
}

pub struct InMemoryCacheBackend {
    cache: Cache<String, InMemoryCacheValue>,
    default_ttl: Duration,
    max_capacity: u64,
}

#[derive(Clone)]
struct InMemoryCacheValue {
    payload: Vec<u8>,
    ttl: Duration,
}

struct InMemoryCacheExpiry;

impl Expiry<String, InMemoryCacheValue> for InMemoryCacheExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &InMemoryCacheValue,
        _created_at: Instant,
    ) -> Option<Duration> {
        Some(value.ttl)
    }

    fn expire_after_update(
        &self,
        _key: &String,
        value: &InMemoryCacheValue,
        _updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        Some(value.ttl)
    }
}

impl InMemoryCacheBackend {
    pub fn new(ttl: Duration, max_capacity: u64) -> Self {
        let cache = Cache::builder()
            .expire_after(InMemoryCacheExpiry)
            .max_capacity(max_capacity)
            .build();

        Self {
            cache,
            default_ttl: ttl,
            max_capacity,
        }
    }
}

#[cfg(feature = "redis-cache")]
pub struct RedisCacheBackend {
    manager: redis::aio::ConnectionManager,
    prefix: String,
    ttl: Duration,
    circuit_breaker: Arc<CircuitBreaker>,
}

#[cfg(feature = "redis-cache")]
fn redis_ttl_millis(ttl: Duration) -> Option<u64> {
    if ttl.is_zero() {
        None
    } else {
        Some(ttl.as_millis().min(u64::MAX as u128) as u64)
    }
}

#[cfg(feature = "redis-cache")]
impl RedisCacheBackend {
    pub async fn new(url: &str, prefix: impl Into<String>, ttl: Duration) -> Result<Self> {
        Self::with_circuit_breaker(url, prefix, ttl, CircuitBreakerConfig::default()).await
    }

    pub async fn with_circuit_breaker(
        url: &str,
        prefix: impl Into<String>,
        ttl: Duration,
        breaker_config: CircuitBreakerConfig,
    ) -> Result<Self> {
        let client =
            redis::Client::open(url).map_err(|err| crate::Error::Cache(err.to_string()))?;
        let manager = client
            .get_connection_manager()
            .await
            .map_err(|err| crate::Error::Cache(err.to_string()))?;

        Ok(Self {
            manager,
            prefix: prefix.into(),
            ttl,
            circuit_breaker: Arc::new(CircuitBreaker::new(breaker_config)),
        })
    }

    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    fn key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{key}", self.prefix)
        }
    }
}

#[async_trait]
impl CacheBackend for InMemoryCacheBackend {
    async fn health(&self) -> Result<()> {
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.cache.get(key).await.map(|entry| entry.payload))
    }

    async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
        self.set_with_ttl(key, value, self.default_ttl).await
    }

    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()> {
        self.cache
            .insert(
                key,
                InMemoryCacheValue {
                    payload: value,
                    ttl,
                },
            )
            .await;
        Ok(())
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.iter().count() as u64,
            ..CacheStats::default()
        }
    }
}

#[cfg(feature = "redis-cache")]
#[async_trait]
impl CacheBackend for RedisCacheBackend {
    async fn health(&self) -> Result<()> {
        let mut manager = self.manager.clone();

        self.circuit_breaker
            .call(|| async move {
                let pong: String = redis::cmd("PING")
                    .query_async(&mut manager)
                    .await
                    .map_err(|err| crate::Error::Cache(err.to_string()))?;
                if pong == "PONG" {
                    Ok::<(), crate::Error>(())
                } else {
                    Err(crate::Error::Cache(format!(
                        "unexpected Redis PING response: {pong}"
                    )))
                }
            })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::Open => {
                    tracing::warn!("Redis cache circuit breaker is OPEN");
                    crate::Error::Cache("Redis unavailable (circuit breaker open)".to_string())
                }
                CircuitBreakerError::Upstream(err) => err,
            })
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut manager = self.manager.clone();
        let redis_key = self.key(key);

        self.circuit_breaker
            .call(|| async move {
                let value: Option<Vec<u8>> = redis::cmd("GET")
                    .arg(redis_key)
                    .query_async(&mut manager)
                    .await
                    .map_err(|err| crate::Error::Cache(err.to_string()))?;
                Ok::<Option<Vec<u8>>, crate::Error>(value)
            })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::Open => {
                    tracing::debug!("Redis cache GET failed: circuit breaker open");
                    crate::Error::Cache("Redis unavailable (circuit breaker open)".to_string())
                }
                CircuitBreakerError::Upstream(err) => err,
            })
    }

    async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
        self.set_with_ttl(key, value, self.ttl).await
    }

    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()> {
        let Some(ttl_millis) = redis_ttl_millis(ttl) else {
            return self.invalidate(&key).await;
        };

        let mut manager = self.manager.clone();
        let redis_key = self.key(&key);

        self.circuit_breaker
            .call(|| async move {
                redis::cmd("SET")
                    .arg(redis_key)
                    .arg(value)
                    .arg("PX")
                    .arg(ttl_millis)
                    .query_async::<()>(&mut manager)
                    .await
                    .map_err(|err| crate::Error::Cache(err.to_string()))?;
                Ok::<(), crate::Error>(())
            })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::Open => {
                    tracing::debug!("Redis cache SET failed: circuit breaker open");
                    crate::Error::Cache("Redis unavailable (circuit breaker open)".to_string())
                }
                CircuitBreakerError::Upstream(err) => err,
            })
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        let mut manager = self.manager.clone();
        let redis_key = self.key(key);

        self.circuit_breaker
            .call(|| async move {
                redis::cmd("DEL")
                    .arg(redis_key)
                    .query_async::<()>(&mut manager)
                    .await
                    .map_err(|err| crate::Error::Cache(err.to_string()))?;
                Ok::<(), crate::Error>(())
            })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::Open => {
                    tracing::debug!("Redis cache DEL failed: circuit breaker open");
                    crate::Error::Cache("Redis unavailable (circuit breaker open)".to_string())
                }
                CircuitBreakerError::Upstream(err) => err,
            })
    }

    fn stats(&self) -> CacheStats {
        CacheStats::default()
    }
}

/// `FallbackCacheBackend` wraps a primary `CacheBackend` (e.g. Redis) with an in-memory
/// fallback. When the primary backend is unavailable, reads are served from the in-memory
/// cache and writes are retained locally for the same bounded TTL. Keys whose primary write
/// failed are tracked separately so they remain readable after Redis reconnects without
/// making ordinary Redis misses return potentially stale local values.
///
/// # Example
/// ```rust,ignore
/// let redis = Arc::new(RedisCacheBackend::new(url, "prefix", ttl).await?);
/// let memory = Arc::new(InMemoryCacheBackend::new(ttl, 1000));
/// let cache: Arc<dyn CacheBackend> = Arc::new(FallbackCacheBackend::new(redis, memory));
/// ```
pub struct FallbackCacheBackend {
    primary: Arc<dyn CacheBackend>,
    fallback: Arc<InMemoryCacheBackend>,
    degraded_writes: InMemoryCacheBackend,
}

impl FallbackCacheBackend {
    pub fn new(primary: Arc<dyn CacheBackend>, fallback: Arc<InMemoryCacheBackend>) -> Self {
        let degraded_writes =
            InMemoryCacheBackend::new(fallback.default_ttl, fallback.max_capacity);
        Self {
            primary,
            fallback,
            degraded_writes,
        }
    }

    async fn mark_degraded_write(&self, key: String, ttl: Duration) {
        let _ = self
            .degraded_writes
            .set_with_ttl(key, Vec::new(), ttl)
            .await;
    }

    async fn clear_degraded_write(&self, key: &str) {
        let _ = self.degraded_writes.invalidate(key).await;
    }

    async fn has_degraded_write(&self, key: &str) -> bool {
        self.degraded_writes
            .get(key)
            .await
            .ok()
            .flatten()
            .is_some()
    }
}

#[async_trait]
impl CacheBackend for FallbackCacheBackend {
    async fn health(&self) -> Result<()> {
        // Report healthy as long as the fallback is available; primary degraded is OK.
        match self.primary.health().await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::warn!(error = %e, "Primary cache unhealthy, using in-memory fallback");
                self.fallback.health().await
            }
        }
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.primary.get(key).await {
            Ok(Some(value)) => {
                self.clear_degraded_write(key).await;
                Ok(Some(value))
            }
            Ok(None) if self.has_degraded_write(key).await => self.fallback.get(key).await,
            Ok(None) => Ok(None),
            Err(e) => {
                tracing::debug!(error = %e, key, "Primary cache GET failed, falling back to in-memory");
                self.fallback.get(key).await
            }
        }
    }

    async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
        let _ = self.fallback.set(key.clone(), value.clone()).await;

        match self.primary.set(key.clone(), value).await {
            Ok(()) => {
                self.clear_degraded_write(&key).await;
                Ok(())
            }
            Err(e) => {
                self.mark_degraded_write(key, self.fallback.default_ttl)
                    .await;
                tracing::debug!(error = %e, "Primary cache SET failed, retained bounded in-memory value");
                Ok(())
            }
        }
    }

    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()> {
        let _ = self
            .fallback
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await;

        match self.primary.set_with_ttl(key.clone(), value, ttl).await {
            Ok(()) => {
                self.clear_degraded_write(&key).await;
                Ok(())
            }
            Err(e) => {
                self.mark_degraded_write(key, ttl).await;
                tracing::debug!(error = %e, "Primary cache SET_TTL failed, retained bounded in-memory value");
                Ok(())
            }
        }
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        let _ = self.fallback.invalidate(key).await;
        self.clear_degraded_write(key).await;

        self.primary.invalidate(key).await.map_err(|e| {
            tracing::warn!(error = %e, key, "Primary cache invalidation failed; stale shared data may remain");
            e
        })
    }

    fn stats(&self) -> CacheStats {
        let primary = self.primary.stats();
        let fallback = self.fallback.stats();
        CacheStats {
            hits: primary.hits.saturating_add(fallback.hits),
            misses: primary.misses.saturating_add(fallback.misses),
            evictions: primary.evictions.saturating_add(fallback.evictions),
            entries: primary.entries.max(fallback.entries),
        }
    }
}

#[cfg(all(test, feature = "redis-cache"))]
mod redis_ttl_unit_tests {
    use super::redis_ttl_millis;
    use std::time::Duration;

    #[test]
    fn preserves_sub_second_ttl_precision() {
        assert_eq!(redis_ttl_millis(Duration::from_millis(250)), Some(250));
        assert_eq!(redis_ttl_millis(Duration::from_millis(1_500)), Some(1_500));
    }

    #[test]
    fn treats_zero_ttl_as_immediate_invalidation() {
        assert_eq!(redis_ttl_millis(Duration::ZERO), None);
    }
}

#[cfg(test)]
mod fallback_consistency_tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    #[derive(Default)]
    struct StubBackend {
        values: Mutex<HashMap<String, Vec<u8>>>,
        fail_get: AtomicBool,
        fail_set: AtomicBool,
        fail_invalidate: AtomicBool,
    }

    impl StubBackend {
        fn clear_value(&self, key: &str) {
            self.values.lock().unwrap().remove(key);
        }
    }

    #[async_trait]
    impl CacheBackend for StubBackend {
        async fn health(&self) -> Result<()> {
            Ok(())
        }

        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            if self.fail_get.load(Ordering::SeqCst) {
                return Err(crate::Error::Cache("get failed".to_string()));
            }
            Ok(self.values.lock().unwrap().get(key).cloned())
        }

        async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
            if self.fail_set.load(Ordering::SeqCst) {
                return Err(crate::Error::Cache("set failed".to_string()));
            }
            self.values.lock().unwrap().insert(key, value);
            Ok(())
        }

        async fn set_with_ttl(&self, key: String, value: Vec<u8>, _ttl: Duration) -> Result<()> {
            self.set(key, value).await
        }

        async fn invalidate(&self, key: &str) -> Result<()> {
            if self.fail_invalidate.load(Ordering::SeqCst) {
                return Err(crate::Error::Cache("invalidate failed".to_string()));
            }
            self.values.lock().unwrap().remove(key);
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats {
                entries: self.values.lock().unwrap().len() as u64,
                ..CacheStats::default()
            }
        }
    }

    #[tokio::test]
    async fn degraded_write_survives_primary_reconnect_but_not_ordinary_primary_miss() {
        let primary = Arc::new(StubBackend::default());
        primary.fail_set.store(true, Ordering::SeqCst);
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 32));
        let cache = FallbackCacheBackend::new(primary.clone(), fallback);

        cache
            .set("key".to_string(), b"local".to_vec())
            .await
            .unwrap();
        primary.fail_set.store(false, Ordering::SeqCst);

        assert_eq!(cache.get("key").await.unwrap(), Some(b"local".to_vec()));

        cache
            .set("key".to_string(), b"shared".to_vec())
            .await
            .unwrap();
        primary.clear_value("key");

        assert_eq!(cache.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn degraded_markers_expire_with_the_cached_value() {
        let primary = Arc::new(StubBackend::default());
        primary.fail_set.store(true, Ordering::SeqCst);
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 32));
        let cache = FallbackCacheBackend::new(primary, fallback);

        cache
            .set_with_ttl(
                "short".to_string(),
                b"value".to_vec(),
                Duration::from_millis(30),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(70)).await;

        assert_eq!(cache.get("short").await.unwrap(), None);
    }

    #[tokio::test]
    async fn invalidation_failure_is_not_silently_acknowledged() {
        let primary = Arc::new(StubBackend::default());
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 32));
        let cache = FallbackCacheBackend::new(primary.clone(), fallback);

        cache
            .set("key".to_string(), b"value".to_vec())
            .await
            .unwrap();
        primary.fail_invalidate.store(true, Ordering::SeqCst);

        assert!(cache.invalidate("key").await.is_err());
        primary.fail_get.store(true, Ordering::SeqCst);
        assert_eq!(cache.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn fallback_stats_include_local_entries() {
        let primary = Arc::new(StubBackend::default());
        primary.fail_set.store(true, Ordering::SeqCst);
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 32));
        let cache = FallbackCacheBackend::new(primary, fallback);

        cache
            .set("key".to_string(), b"value".to_vec())
            .await
            .unwrap();

        assert_eq!(cache.stats().entries, 1);
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;

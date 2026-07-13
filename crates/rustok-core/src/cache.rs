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
/// fallback. When the primary backend returns a `Cache` error (e.g. circuit breaker open),
/// reads are served from the in-memory cache and writes go to both backends so the in-memory
/// layer stays warm. This provides transparent degraded-mode operation without surfacing
/// cache errors to callers.
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
}

impl FallbackCacheBackend {
    pub fn new(primary: Arc<dyn CacheBackend>, fallback: Arc<InMemoryCacheBackend>) -> Self {
        Self { primary, fallback }
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
            Ok(value) => Ok(value),
            Err(e) => {
                tracing::debug!(error = %e, key, "Primary cache GET failed, falling back to in-memory");
                self.fallback.get(key).await
            }
        }
    }

    async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
        // Write to fallback unconditionally to keep it warm.
        let _ = self.fallback.set(key.clone(), value.clone()).await;

        match self.primary.set(key, value).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::debug!(error = %e, "Primary cache SET failed, wrote to in-memory fallback only");
                Ok(()) // Degrade gracefully — value is in memory
            }
        }
    }

    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> Result<()> {
        let _ = self
            .fallback
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await;

        match self.primary.set_with_ttl(key, value, ttl).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::debug!(error = %e, "Primary cache SET_TTL failed, wrote to in-memory fallback only");
                Ok(())
            }
        }
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        let _ = self.fallback.invalidate(key).await;

        match self.primary.invalidate(key).await {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::debug!(error = %e, key, "Primary cache INVALIDATE failed, in-memory entry removed");
                Ok(())
            }
        }
    }

    fn stats(&self) -> CacheStats {
        self.primary.stats()
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
#[path = "cache_tests.rs"]
mod tests;

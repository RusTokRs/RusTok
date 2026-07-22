use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats, InMemoryCacheBackend};

#[cfg(feature = "redis-cache")]
use rustok_core::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
#[cfg(feature = "redis-cache")]
use tokio::sync::Mutex as AsyncMutex;

#[cfg(feature = "redis-cache")]
use crate::fallback::DegradationAwareFallbackBackend;
use crate::{CacheBackendOptions, CacheService};

#[cfg(feature = "redis-cache")]
const SHARED_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "redis-cache")]
const SHARED_REDIS_COMPARE_AND_SET_SCRIPT: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current or current ~= ARGV[1] then
    return 0
end
local ttl = tonumber(ARGV[3])
if ttl <= 0 then
    redis.call('DEL', KEYS[1])
else
    redis.call('PSETEX', KEYS[1], ttl, ARGV[2])
end
return 1
"#;

/// Redis backend constructed from the client already owned by `CacheService`.
///
/// The connection manager is initialized lazily inside the same operation timeout and circuit
/// breaker that protect commands. A Redis outage during server startup therefore remains visible
/// through health while bounded local fallback stays available, and the existing backend can
/// connect automatically after Redis recovers instead of becoming a permanent memory-only cache.
#[cfg(feature = "redis-cache")]
pub(crate) struct SharedClientRedisCacheBackend {
    client: redis::Client,
    manager: AsyncMutex<Option<redis::aio::ConnectionManager>>,
    prefix: String,
    ttl: Duration,
    operation_timeout: Duration,
    circuit_breaker: Arc<CircuitBreaker>,
}

#[cfg(feature = "redis-cache")]
impl SharedClientRedisCacheBackend {
    pub(crate) async fn new(
        client: redis::Client,
        prefix: impl Into<String>,
        ttl: Duration,
        circuit_breaker: CircuitBreakerConfig,
    ) -> rustok_core::Result<Self> {
        Self::with_timeout(
            client,
            prefix,
            ttl,
            circuit_breaker,
            SHARED_REDIS_OPERATION_TIMEOUT,
        )
    }

    fn with_timeout(
        client: redis::Client,
        prefix: impl Into<String>,
        ttl: Duration,
        circuit_breaker: CircuitBreakerConfig,
        operation_timeout: Duration,
    ) -> rustok_core::Result<Self> {
        if operation_timeout.is_zero() {
            return Err(rustok_core::Error::Cache(
                "shared Redis operation timeout must be greater than zero".to_string(),
            ));
        }

        Ok(Self {
            client,
            manager: AsyncMutex::new(None),
            prefix: prefix.into(),
            ttl,
            operation_timeout,
            circuit_breaker: Arc::new(CircuitBreaker::new(circuit_breaker)),
        })
    }

    async fn connection_manager(&self) -> rustok_core::Result<redis::aio::ConnectionManager> {
        let mut state = self.manager.lock().await;
        if let Some(manager) = state.as_ref() {
            return Ok(manager.clone());
        }

        let manager = self
            .client
            .get_connection_manager()
            .await
            .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
        *state = Some(manager.clone());
        Ok(manager)
    }

    fn redis_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}:{key}", self.prefix)
        }
    }
}

#[cfg(feature = "redis-cache")]
#[async_trait]
impl CacheBackend for SharedClientRedisCacheBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        let timeout = self.operation_timeout;
        self.circuit_breaker
            .call(|| async move {
                shared_redis_timeout(timeout, async move {
                    let mut manager = self.connection_manager().await?;
                    let pong = redis::cmd("PING")
                        .query_async::<String>(&mut manager)
                        .await
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
                    if pong == "PONG" {
                        Ok(())
                    } else {
                        Err(rustok_core::Error::Cache(format!(
                            "unexpected Redis PING response: {pong}"
                        )))
                    }
                })
                .await
            })
            .await
            .map_err(shared_circuit_error)
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        let redis_key = self.redis_key(key);
        let timeout = self.operation_timeout;
        self.circuit_breaker
            .call(|| async move {
                shared_redis_timeout(timeout, async move {
                    let mut manager = self.connection_manager().await?;
                    redis::cmd("GET")
                        .arg(redis_key)
                        .query_async::<Option<Vec<u8>>>(&mut manager)
                        .await
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                })
                .await
            })
            .await
            .map_err(shared_circuit_error)
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.set_with_ttl(key, value, self.ttl).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        let Some(ttl_millis) = ttl_millis(ttl) else {
            return self.invalidate(&key).await;
        };
        let redis_key = self.redis_key(&key);
        let timeout = self.operation_timeout;
        self.circuit_breaker
            .call(|| async move {
                shared_redis_timeout(timeout, async move {
                    let mut manager = self.connection_manager().await?;
                    redis::cmd("SET")
                        .arg(redis_key)
                        .arg(value)
                        .arg("PX")
                        .arg(ttl_millis)
                        .query_async::<()>(&mut manager)
                        .await
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                })
                .await
            })
            .await
            .map_err(shared_circuit_error)
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        let redis_key = self.redis_key(key);
        let expected = expected.to_vec();
        let ttl_millis = ttl_millis(ttl.unwrap_or(self.ttl)).unwrap_or(0);
        let timeout = self.operation_timeout;
        self.circuit_breaker
            .call(|| async move {
                shared_redis_timeout(timeout, async move {
                    let mut manager = self.connection_manager().await?;
                    let applied = redis::cmd("EVAL")
                        .arg(SHARED_REDIS_COMPARE_AND_SET_SCRIPT)
                        .arg(1)
                        .arg(redis_key)
                        .arg(expected)
                        .arg(value)
                        .arg(ttl_millis)
                        .query_async::<i64>(&mut manager)
                        .await
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))?;
                    match applied {
                        1 => Ok(CacheCompareAndSetOutcome::Applied),
                        0 => Ok(CacheCompareAndSetOutcome::Mismatch),
                        other => Err(rustok_core::Error::Cache(format!(
                            "unexpected shared Redis compare-and-set response: {other}"
                        ))),
                    }
                })
                .await
            })
            .await
            .map_err(shared_circuit_error)
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        let redis_key = self.redis_key(key);
        let timeout = self.operation_timeout;
        self.circuit_breaker
            .call(|| async move {
                shared_redis_timeout(timeout, async move {
                    let mut manager = self.connection_manager().await?;
                    redis::cmd("DEL")
                        .arg(redis_key)
                        .query_async::<()>(&mut manager)
                        .await
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                })
                .await
            })
            .await
            .map_err(shared_circuit_error)
    }

    fn stats(&self) -> CacheStats {
        CacheStats::default()
    }
}

impl CacheService {
    /// Create an entry-count backend from the Redis client owned by this service.
    ///
    /// This is the migration-safe replacement for factories that reconstruct a Redis client
    /// from `redis_url`. It preserves fallback and instrumentation behavior. Logical keys are
    /// transparently scoped by a monotonic backend generation, so namespace invalidation does not
    /// require scanning Redis or enumerating process-local entries.
    pub async fn backend_shared_client(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
    ) -> Arc<dyn CacheBackend> {
        self.backend_shared_client_with_options(
            prefix,
            ttl,
            max_capacity,
            self.default_backend_options().clone(),
        )
        .await
    }

    pub async fn backend_shared_client_with_options(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        let backend = self
            .raw_shared_client_backend(prefix, ttl, max_capacity, &options)
            .await;
        let backend = self.wrap_generation_aware_backend(prefix, backend).await;
        let backend = self.wrap_generation_recovery_health(prefix, backend);
        if options.metrics_enabled {
            Arc::new(SharedInstrumentedCacheBackend::new(prefix, backend))
        } else {
            backend
        }
    }

    pub(crate) async fn raw_shared_client_backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: &CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        #[cfg(not(feature = "redis-cache"))]
        let _ = (prefix, options);

        #[cfg(feature = "redis-cache")]
        if let Some(client) = self.redis_client().cloned() {
            match SharedClientRedisCacheBackend::new(
                client,
                prefix,
                ttl,
                options.redis_circuit_breaker.clone(),
            )
            .await
            {
                Ok(redis) => {
                    let memory = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
                    return Arc::new(DegradationAwareFallbackBackend::new(
                        Arc::new(redis),
                        memory,
                    ));
                }
                Err(error) => {
                    tracing::warn!(%error, prefix, "Redis cache backend initialization failed; using memory backend");
                }
            }
        }

        Arc::new(InMemoryCacheBackend::new(ttl, max_capacity))
    }
}

struct SharedInstrumentedCacheBackend {
    name: String,
    inner: Arc<dyn CacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
    invalidations: AtomicU64,
}

impl SharedInstrumentedCacheBackend {
    fn new(name: impl Into<String>, inner: Arc<dyn CacheBackend>) -> Self {
        Self {
            name: name.into(),
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl CacheBackend for SharedInstrumentedCacheBackend {
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

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        self.inner.compare_and_set(key, expected, value, ttl).await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.inner.invalidate(key).await?;
        self.invalidations.fetch_add(1, Ordering::Relaxed);
        Ok(())
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
                .invalidations
                .load(Ordering::Relaxed)
                .saturating_add(inner.evictions),
            entries: inner.entries,
        }
    }
}

impl Drop for SharedInstrumentedCacheBackend {
    fn drop(&mut self) {
        tracing::debug!(cache = %self.name, stats = ?self.stats(), "shared-client cache backend dropped");
    }
}

#[cfg(feature = "redis-cache")]
fn ttl_millis(ttl: Duration) -> Option<u64> {
    if ttl.is_zero() {
        return None;
    }
    let millis = ttl.as_millis();
    Some(if millis == 0 {
        1
    } else {
        millis.min(i64::MAX as u128) as u64
    })
}

#[cfg(feature = "redis-cache")]
async fn shared_redis_timeout<T, F>(timeout: Duration, future: F) -> rustok_core::Result<T>
where
    F: std::future::Future<Output = rustok_core::Result<T>>,
{
    tokio::time::timeout(timeout, future).await.map_err(|_| {
        rustok_core::Error::Cache(format!(
            "shared Redis cache operation timed out after {} ms",
            timeout.as_millis()
        ))
    })?
}

#[cfg(feature = "redis-cache")]
fn shared_circuit_error(error: CircuitBreakerError<rustok_core::Error>) -> rustok_core::Error {
    match error {
        CircuitBreakerError::Open => {
            rustok_core::Error::Cache("Redis unavailable (circuit breaker open)".to_string())
        }
        CircuitBreakerError::Upstream(error) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct FailingInvalidationBackend;

    #[async_trait]
    impl CacheBackend for FailingInvalidationBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            Ok(None)
        }

        async fn set(&self, _key: String, _value: Vec<u8>) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn set_with_ttl(
            &self,
            _key: String,
            _value: Vec<u8>,
            _ttl: Duration,
        ) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            Err(rustok_core::Error::Cache(
                "simulated invalidation failure".to_string(),
            ))
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    #[tokio::test]
    async fn shared_client_factory_preserves_memory_contract_without_redis() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("shared-test", Duration::from_secs(60), 16)
            .await;

        assert!(backend.get("missing").await.unwrap().is_none());
        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );
        assert_eq!(backend.stats().entries, 1);
    }

    #[tokio::test]
    async fn shared_client_factory_exposes_atomic_cas_without_redis() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("shared-cas", Duration::from_secs(60), 16)
            .await;
        backend
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();

        assert_eq!(
            backend
                .compare_and_set("key", b"old", b"new".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Applied
        );
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn shared_instrumentation_counts_only_successful_invalidations() {
        let backend = SharedInstrumentedCacheBackend::new(
            "shared-failing-invalidation",
            Arc::new(FailingInvalidationBackend),
        );

        assert!(backend.invalidate("key").await.is_err());
        assert_eq!(backend.stats().evictions, 0);
    }

    #[tokio::test]
    async fn standard_factory_switches_namespace_on_generation_change() {
        let service = CacheService::from_url(None);
        let prefix = format!("shared-generation:{}", Uuid::new_v4().simple());
        let backend = service
            .backend_shared_client(&prefix, Duration::from_secs(60), 16)
            .await;
        backend
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"old".to_vec()));

        crate::observe_cache_backend_generation(&prefix, 1).unwrap();
        assert_eq!(backend.get("key").await.unwrap(), None);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn configured_redis_outage_remains_visible_and_local_writes_stay_bounded() {
        let service = CacheService::from_url(Some("redis://127.0.0.1:1/"));
        let options = service.default_backend_options().clone();
        let backend = service
            .raw_shared_client_backend("startup-outage", Duration::from_secs(30), 16, &options)
            .await;

        assert!(backend.health().await.is_err());
        backend
            .set("key".to_string(), b"local".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"local".to_vec()));
    }

    #[cfg(feature = "redis-cache")]
    #[test]
    fn shared_backend_ttl_preserves_positive_sub_millisecond_values() {
        assert_eq!(ttl_millis(Duration::from_nanos(1)), Some(1));
        assert_eq!(ttl_millis(Duration::from_micros(999)), Some(1));
    }

    #[cfg(feature = "redis-cache")]
    #[test]
    fn shared_backend_ttl_clamps_to_redis_signed_range() {
        assert_eq!(
            ttl_millis(Duration::new(u64::MAX, 999_999_999)),
            Some(i64::MAX as u64)
        );
    }

    #[cfg(feature = "redis-cache")]
    #[test]
    fn shared_redis_cas_script_is_conditional_and_binary_safe() {
        assert!(SHARED_REDIS_COMPARE_AND_SET_SCRIPT.contains("current ~= ARGV[1]"));
        assert!(SHARED_REDIS_COMPARE_AND_SET_SCRIPT.contains("PSETEX"));
        assert!(SHARED_REDIS_COMPARE_AND_SET_SCRIPT.contains("DEL"));
    }
}

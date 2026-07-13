#[cfg(feature = "redis-cache")]
use std::future::Future;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use moka::future::Cache;
use moka::Expiry;
use tokio::sync::Mutex as AsyncMutex;

use crate::context::{CacheBackend, CacheCompareAndSetOutcome};
#[cfg(feature = "redis-cache")]
use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
use crate::Result;

const IN_MEMORY_WRITE_LOCK_STRIPES: usize = 64;

#[cfg(feature = "redis-cache")]
const REDIS_COMPARE_AND_SET_SCRIPT: &str = r#"
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

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: u64,
}

#[derive(Debug, Clone, Copy)]
enum InMemoryCacheCapacity {
    Entries(u64),
    WeightBytes(u64),
}

pub struct InMemoryCacheBackend {
    cache: Cache<String, InMemoryCacheValue>,
    default_ttl: Duration,
    capacity: InMemoryCacheCapacity,
    write_locks: [AsyncMutex<()>; IN_MEMORY_WRITE_LOCK_STRIPES],
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

fn in_memory_entry_weight(key: &String, value: &InMemoryCacheValue) -> u32 {
    let weight = key
        .len()
        .saturating_add(value.payload.len())
        .saturating_add(std::mem::size_of::<InMemoryCacheValue>());
    weight.clamp(1, u32::MAX as usize) as u32
}

impl InMemoryCacheBackend {
    /// Construct a cache whose capacity is measured in entry count.
    pub fn new(ttl: Duration, max_capacity: u64) -> Self {
        Self::with_capacity(ttl, InMemoryCacheCapacity::Entries(max_capacity))
    }

    /// Construct a cache whose capacity is measured by key and payload bytes.
    ///
    /// This should be preferred for caches containing serialized documents or other
    /// variable-size payloads, where a count-only limit allows a small number of very
    /// large values to exhaust process memory.
    pub fn new_weighted(ttl: Duration, max_weight_bytes: u64) -> Self {
        Self::with_capacity(ttl, InMemoryCacheCapacity::WeightBytes(max_weight_bytes))
    }

    fn with_capacity(ttl: Duration, capacity: InMemoryCacheCapacity) -> Self {
        let cache = match capacity {
            InMemoryCacheCapacity::Entries(max_capacity) => Cache::builder()
                .expire_after(InMemoryCacheExpiry)
                .max_capacity(max_capacity)
                .build(),
            InMemoryCacheCapacity::WeightBytes(max_weight_bytes) => Cache::builder()
                .expire_after(InMemoryCacheExpiry)
                .weigher(in_memory_entry_weight)
                .max_capacity(max_weight_bytes)
                .build(),
        };

        Self {
            cache,
            default_ttl: ttl,
            capacity,
            write_locks: std::array::from_fn(|_| AsyncMutex::new(())),
        }
    }

    fn write_lock(&self, key: &str) -> &AsyncMutex<()> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        &self.write_locks[(hasher.finish() as usize) % IN_MEMORY_WRITE_LOCK_STRIPES]
    }

    async fn write_value_unlocked(&self, key: String, value: Vec<u8>, ttl: Duration) {
        if ttl.is_zero() {
            self.cache.invalidate(&key).await;
        } else {
            self.cache
                .insert(
                    key,
                    InMemoryCacheValue {
                        payload: value,
                        ttl,
                    },
                )
                .await;
        }
    }
}

#[cfg(feature = "redis-cache")]
const DEFAULT_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "redis-cache")]
pub struct RedisCacheBackend {
    manager: redis::aio::ConnectionManager,
    prefix: String,
    ttl: Duration,
    operation_timeout: Duration,
    circuit_breaker: Arc<CircuitBreaker>,
}

#[cfg(feature = "redis-cache")]
fn redis_ttl_millis(ttl: Duration) -> Option<u64> {
    if ttl.is_zero() {
        return None;
    }

    let millis = ttl.as_millis();
    Some(if millis == 0 {
        1
    } else {
        millis.min(u64::MAX as u128) as u64
    })
}

#[cfg(feature = "redis-cache")]
async fn redis_operation_with_timeout<T, F>(timeout: Duration, future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    tokio::time::timeout(timeout, future).await.map_err(|_| {
        crate::Error::Cache(format!(
            "Redis cache operation timed out after {} ms",
            timeout.as_millis()
        ))
    })?
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
        Self::with_circuit_breaker_and_timeout(
            url,
            prefix,
            ttl,
            breaker_config,
            DEFAULT_REDIS_OPERATION_TIMEOUT,
        )
        .await
    }

    pub async fn with_circuit_breaker_and_timeout(
        url: &str,
        prefix: impl Into<String>,
        ttl: Duration,
        breaker_config: CircuitBreakerConfig,
        operation_timeout: Duration,
    ) -> Result<Self> {
        if operation_timeout.is_zero() {
            return Err(crate::Error::Cache(
                "Redis cache operation timeout must be greater than zero".to_string(),
            ));
        }

        let client =
            redis::Client::open(url).map_err(|err| crate::Error::Cache(err.to_string()))?;
        let manager = redis_operation_with_timeout(operation_timeout, async {
            client
                .get_connection_manager()
                .await
                .map_err(|err| crate::Error::Cache(err.to_string()))
        })
        .await?;

        Ok(Self {
            manager,
            prefix: prefix.into(),
            ttl,
            operation_timeout,
            circuit_breaker: Arc::new(CircuitBreaker::new(breaker_config)),
        })
    }

    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    pub fn operation_timeout(&self) -> Duration {
        self.operation_timeout
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
        let _guard = self.write_lock(&key).lock().await;
        self.write_value_unlocked(key, value, ttl).await;
        Ok(())
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<CacheCompareAndSetOutcome> {
        let _guard = self.write_lock(key).lock().await;
        let current = self.cache.get(key).await;
        if current.as_ref().map(|entry| entry.payload.as_slice()) != Some(expected) {
            return Ok(CacheCompareAndSetOutcome::Mismatch);
        }

        self.write_value_unlocked(key.to_string(), value, ttl.unwrap_or(self.default_ttl))
            .await;
        Ok(CacheCompareAndSetOutcome::Applied)
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        let _guard = self.write_lock(key).lock().await;
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
        let operation_timeout = self.operation_timeout;

        self.circuit_breaker
            .call(|| async move {
                redis_operation_with_timeout(operation_timeout, async move {
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
        let operation_timeout = self.operation_timeout;

        self.circuit_breaker
            .call(|| async move {
                redis_operation_with_timeout(operation_timeout, async move {
                    let value: Option<Vec<u8>> = redis::cmd("GET")
                        .arg(redis_key)
                        .query_async(&mut manager)
                        .await
                        .map_err(|err| crate::Error::Cache(err.to_string()))?;
                    Ok::<Option<Vec<u8>>, crate::Error>(value)
                })
                .await
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
        let operation_timeout = self.operation_timeout;

        self.circuit_breaker
            .call(|| async move {
                redis_operation_with_timeout(operation_timeout, async move {
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

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<CacheCompareAndSetOutcome> {
        let mut manager = self.manager.clone();
        let redis_key = self.key(key);
        let expected = expected.to_vec();
        let ttl_millis = redis_ttl_millis(ttl.unwrap_or(self.ttl)).unwrap_or(0);
        let operation_timeout = self.operation_timeout;

        self.circuit_breaker
            .call(|| async move {
                redis_operation_with_timeout(operation_timeout, async move {
                    let applied = redis::cmd("EVAL")
                        .arg(REDIS_COMPARE_AND_SET_SCRIPT)
                        .arg(1)
                        .arg(redis_key)
                        .arg(expected)
                        .arg(value)
                        .arg(ttl_millis)
                        .query_async::<i64>(&mut manager)
                        .await
                        .map_err(|err| crate::Error::Cache(err.to_string()))?;
                    match applied {
                        1 => Ok(CacheCompareAndSetOutcome::Applied),
                        0 => Ok(CacheCompareAndSetOutcome::Mismatch),
                        other => Err(crate::Error::Cache(format!(
                            "unexpected Redis compare-and-set response: {other}"
                        ))),
                    }
                })
                .await
            })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::Open => crate::Error::Cache(
                    "Redis unavailable (circuit breaker open)".to_string(),
                ),
                CircuitBreakerError::Upstream(err) => err,
            })
    }

    async fn invalidate(&self, key: &str) -> Result<()> {
        let mut manager = self.manager.clone();
        let redis_key = self.key(key);
        let operation_timeout = self.operation_timeout;

        self.circuit_breaker
            .call(|| async move {
                redis_operation_with_timeout(operation_timeout, async move {
                    redis::cmd("DEL")
                        .arg(redis_key)
                        .query_async::<()>(&mut manager)
                        .await
                        .map_err(|err| crate::Error::Cache(err.to_string()))?;
                    Ok::<(), crate::Error>(())
                })
                .await
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
            InMemoryCacheBackend::with_capacity(fallback.default_ttl, fallback.capacity);
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

    async fn mirror_primary_cas(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) {
        let result = match ttl {
            Some(ttl) => self
                .fallback
                .set_with_ttl(key.to_string(), value, ttl)
                .await,
            None => self.fallback.set(key.to_string(), value).await,
        };
        if let Err(error) = result {
            tracing::warn!(%error, key, "Primary cache CAS applied but local mirror update failed");
        }
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

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<CacheCompareAndSetOutcome> {
        let outcome = self
            .primary
            .compare_and_set(key, expected, value.clone(), ttl)
            .await?;
        match outcome {
            CacheCompareAndSetOutcome::Applied => {
                self.clear_degraded_write(key).await;
                self.mirror_primary_cas(key, value, ttl).await;
            }
            CacheCompareAndSetOutcome::Mismatch => {
                self.clear_degraded_write(key).await;
                let _ = self.fallback.invalidate(key).await;
            }
        }
        Ok(outcome)
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

#[cfg(test)]
mod in_memory_capacity_tests {
    use super::*;

    #[test]
    fn entry_weight_accounts_for_key_payload_and_value_metadata() {
        let key = "tenant:key".to_string();
        let value = InMemoryCacheValue {
            payload: vec![0; 128],
            ttl: Duration::from_secs(1),
        };

        assert!(in_memory_entry_weight(&key, &value) >= (key.len() + 128) as u32);
    }

    #[tokio::test]
    async fn weighted_cache_does_not_retain_entry_larger_than_its_budget() {
        let cache = InMemoryCacheBackend::new_weighted(Duration::from_secs(60), 64);
        cache
            .set("large".to_string(), vec![0; 256])
            .await
            .unwrap();
        cache.cache.run_pending_tasks().await;

        assert_eq!(cache.get("large").await.unwrap(), None);
    }

    #[tokio::test]
    async fn in_memory_compare_and_set_applies_only_to_matching_bytes() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(60), 16);
        cache
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();

        assert_eq!(
            cache
                .compare_and_set("key", b"wrong", b"bad".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Mismatch
        );
        assert_eq!(cache.get("key").await.unwrap(), Some(b"old".to_vec()));

        assert_eq!(
            cache
                .compare_and_set("key", b"old", b"new".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Applied
        );
        assert_eq!(cache.get("key").await.unwrap(), Some(b"new".to_vec()));

        assert_eq!(
            cache
                .compare_and_set("key", b"new", Vec::new(), Some(Duration::ZERO))
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Applied
        );
        assert_eq!(cache.get("key").await.unwrap(), None);
    }
}

#[cfg(all(test, feature = "redis-cache"))]
mod redis_backend_unit_tests {
    use super::{
        redis_operation_with_timeout, redis_ttl_millis, CacheCompareAndSetOutcome,
        RedisCacheBackend, REDIS_COMPARE_AND_SET_SCRIPT,
    };
    use crate::CacheBackend;
    use std::time::Duration;

    #[test]
    fn preserves_sub_second_ttl_precision() {
        assert_eq!(redis_ttl_millis(Duration::from_millis(250)), Some(250));
        assert_eq!(redis_ttl_millis(Duration::from_millis(1_500)), Some(1_500));
    }

    #[test]
    fn rounds_positive_sub_millisecond_ttl_up_to_one_millisecond() {
        assert_eq!(redis_ttl_millis(Duration::from_nanos(1)), Some(1));
        assert_eq!(redis_ttl_millis(Duration::from_micros(999)), Some(1));
    }

    #[test]
    fn treats_zero_ttl_as_immediate_invalidation() {
        assert_eq!(redis_ttl_millis(Duration::ZERO), None);
    }

    #[test]
    fn compare_and_set_script_is_binary_safe_and_conditional() {
        assert!(REDIS_COMPARE_AND_SET_SCRIPT.contains("current ~= ARGV[1]"));
        assert!(REDIS_COMPARE_AND_SET_SCRIPT.contains("PSETEX"));
        assert!(REDIS_COMPARE_AND_SET_SCRIPT.contains("DEL"));
    }

    #[tokio::test]
    async fn operation_timeout_bounds_a_stalled_redis_future() {
        let result: crate::Result<()> = redis_operation_with_timeout(
            Duration::from_millis(5),
            std::future::pending::<crate::Result<()>>(),
        )
        .await;

        match result {
            Err(crate::Error::Cache(message)) => assert!(message.contains("timed out")),
            other => panic!("expected Redis timeout cache error, got {other:?}"),
        }
    }

    #[tokio::test]
    #[ignore = "requires a live Redis instance; set RUSTOK_CACHE_REAL_REDIS_URL"]
    async fn real_redis_compare_and_set_is_atomic_and_preserves_mismatch() {
        let Ok(redis_url) = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL") else {
            eprintln!("skipping real Redis CAS test: RUSTOK_CACHE_REAL_REDIS_URL is not set");
            return;
        };
        let prefix = format!(
            "cache-cas-test:{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let cache = RedisCacheBackend::new(&redis_url, prefix, Duration::from_secs(60))
            .await
            .unwrap();
        cache.set("key".to_string(), b"old".to_vec()).await.unwrap();

        assert_eq!(
            cache
                .compare_and_set("key", b"wrong", b"bad".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Mismatch
        );
        assert_eq!(cache.get("key").await.unwrap(), Some(b"old".to_vec()));
        assert_eq!(
            cache
                .compare_and_set("key", b"old", b"new".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Applied
        );
        assert_eq!(cache.get("key").await.unwrap(), Some(b"new".to_vec()));
        cache.invalidate("key").await.unwrap();
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

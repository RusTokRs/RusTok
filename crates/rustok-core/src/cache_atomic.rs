use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use moka::future::Cache;
use moka::ops::compute::{CompResult, Op};
use moka::Expiry;
use tokio::sync::Mutex as AsyncMutex;

use crate::cache::CacheStats;
use crate::context::{CacheBackend, CacheCompareAndSetOutcome};
use crate::Result;

const IN_MEMORY_WRITE_LOCK_STRIPES: usize = 64;

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

fn in_memory_entry_weight(key: &str, value: &InMemoryCacheValue) -> u32 {
    let weight = key
        .len()
        .saturating_add(value.payload.len())
        .saturating_add(std::mem::size_of::<InMemoryCacheValue>());
    weight.clamp(1, u32::MAX as usize) as u32
}

impl InMemoryCacheBackend {
    pub fn new(ttl: Duration, max_capacity: u64) -> Self {
        Self::with_capacity(ttl, InMemoryCacheCapacity::Entries(max_capacity))
    }

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
                .weigher(|key, value| in_memory_entry_weight(key, value))
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
        let ttl = ttl.unwrap_or(self.default_ttl);
        let expected = expected.to_vec();
        let replacement = InMemoryCacheValue {
            payload: value,
            ttl,
        };
        let result = self
            .cache
            .entry(key.to_string())
            .and_compute_with(move |current| {
                let operation = match current {
                    Some(entry) if entry.value().payload.as_slice() == expected.as_slice() => {
                        if ttl.is_zero() {
                            Op::Remove
                        } else {
                            Op::Put(replacement)
                        }
                    }
                    _ => Op::Nop,
                };
                std::future::ready(operation)
            })
            .await;

        Ok(match result {
            CompResult::ReplacedWith(_) | CompResult::Removed(_) => {
                CacheCompareAndSetOutcome::Applied
            }
            CompResult::Unchanged(_) | CompResult::StillNone(_) => {
                CacheCompareAndSetOutcome::Mismatch
            }
            CompResult::Inserted(_) => {
                debug_assert!(false, "compare-and-set must never insert a missing entry");
                CacheCompareAndSetOutcome::Mismatch
            }
        })
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
        self.degraded_writes.get(key).await.ok().flatten().is_some()
    }

    async fn mirror_primary_cas(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) {
        let result = match ttl {
            Some(ttl) => {
                self.fallback
                    .set_with_ttl(key.to_string(), value, ttl)
                    .await
            }
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
        match self.primary.health().await {
            Ok(()) => Ok(()),
            Err(error) => {
                tracing::warn!(%error, "Primary cache unhealthy, using in-memory fallback");
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
            Err(error) => {
                tracing::debug!(%error, key, "Primary cache GET failed, falling back to in-memory");
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
            Err(error) => {
                self.mark_degraded_write(key, self.fallback.default_ttl)
                    .await;
                tracing::debug!(%error, "Primary cache SET failed, retained bounded in-memory value");
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
            Err(error) => {
                self.mark_degraded_write(key, ttl).await;
                tracing::debug!(%error, "Primary cache SET_TTL failed, retained bounded in-memory value");
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
        self.primary.invalidate(key).await.map_err(|error| {
            tracing::warn!(%error, key, "Primary cache invalidation failed; stale shared data may remain");
            error
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn compare_and_set_does_not_insert_a_missing_or_expired_entry() {
        let cache = InMemoryCacheBackend::new(Duration::from_millis(10), 16);
        cache
            .set_with_ttl(
                "expired".to_string(),
                b"old".to_vec(),
                Duration::from_millis(1),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;

        assert_eq!(
            cache
                .compare_and_set("expired", b"old", b"revived".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Mismatch
        );
        assert_eq!(cache.get("expired").await.unwrap(), None);

        assert_eq!(
            cache
                .compare_and_set("missing", b"old", b"inserted".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Mismatch
        );
        assert_eq!(cache.get("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn compare_and_set_replaces_or_removes_only_a_matching_entry() {
        let cache = InMemoryCacheBackend::new(Duration::from_secs(60), 16);
        cache.set("key".to_string(), b"old".to_vec()).await.unwrap();

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

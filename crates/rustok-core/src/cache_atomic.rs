use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use moka::Expiry;
use moka::future::Cache;
use moka::ops::compute::{CompResult, Op};
use tokio::sync::Mutex as AsyncMutex;

use crate::Result;
use crate::cache::CacheStats;
use crate::context::{CacheBackend, CacheCompareAndSetOutcome};

const IN_MEMORY_WRITE_LOCK_STRIPES: usize = 64;

#[derive(Debug, Clone, Copy)]
enum InMemoryCacheCapacity {
    Entries(u64),
    WeightBytes(u64),
}

pub struct InMemoryCacheBackend {
    cache: Cache<String, InMemoryCacheValue>,
    default_ttl: Duration,
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

#[cfg(test)]
mod tests {
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
        cache.set("large".to_string(), vec![0; 256]).await.unwrap();
        cache.cache.run_pending_tasks().await;

        assert_eq!(cache.get("large").await.unwrap(), None);
    }

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

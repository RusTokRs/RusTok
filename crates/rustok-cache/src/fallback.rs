use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats, InMemoryCacheBackend};
use tokio::sync::Mutex;

const MAX_DEGRADED_WRITE_KEYS: usize = 4_096;
const MAX_DEGRADED_WRITE_KEY_BYTES: usize = 4 * 1_024;
const DEGRADED_WRITE_PRUNE_SCAN: usize = 16;

#[derive(Debug)]
struct DegradedWriteTracker {
    state: Mutex<DegradedWriteTrackerState>,
    maximum_keys: usize,
}

#[derive(Debug, Default)]
struct DegradedWriteTrackerState {
    keys: HashSet<String>,
    insertion_order: VecDeque<String>,
}

impl DegradedWriteTracker {
    fn new(maximum_keys: usize) -> Self {
        Self {
            state: Mutex::new(DegradedWriteTrackerState::default()),
            maximum_keys,
        }
    }

    async fn contains(&self, key: &str) -> bool {
        self.state.lock().await.keys.contains(key)
    }

    async fn remove(&self, key: &str) {
        let mut state = self.state.lock().await;
        state.keys.remove(key);
        state.insertion_order.retain(|candidate| candidate != key);
    }

    async fn insert(
        &self,
        key: &str,
        expected: &[u8],
        fallback: &InMemoryCacheBackend,
    ) -> rustok_core::Result<()> {
        if key.len() > MAX_DEGRADED_WRITE_KEY_BYTES {
            return Err(rustok_core::Error::Cache(format!(
                "degraded cache write key is {} bytes; maximum is {MAX_DEGRADED_WRITE_KEY_BYTES}",
                key.len()
            )));
        }
        if fallback.get(key).await?.as_deref() != Some(expected) {
            return Err(rustok_core::Error::Cache(
                "degraded cache write bytes were not retained by the local fallback".to_string(),
            ));
        }

        let mut state = self.state.lock().await;
        if state.keys.contains(key) {
            return Ok(());
        }

        let scan = state
            .insertion_order
            .iter()
            .take(DEGRADED_WRITE_PRUNE_SCAN)
            .cloned()
            .collect::<Vec<_>>();
        for candidate in scan {
            if fallback.get(&candidate).await?.is_none() {
                state.keys.remove(&candidate);
            }
        }
        while state
            .insertion_order
            .front()
            .is_some_and(|candidate| !state.keys.contains(candidate))
        {
            state.insertion_order.pop_front();
        }

        if state.keys.len() >= self.maximum_keys {
            return Err(rustok_core::Error::Cache(format!(
                "degraded cache write tracker reached capacity {}",
                self.maximum_keys
            )));
        }

        let key = key.to_string();
        state.keys.insert(key.clone());
        state.insertion_order.push_back(key);
        Ok(())
    }
}

/// Availability-preserving fallback whose health and atomic mutations remain strict about the
/// shared primary.
///
/// Ordinary writes are mirrored locally before they reach Redis. A bounded fail-closed tracker
/// records only writes whose primary operation failed, so those newer local bytes remain
/// authoritative during reconnects instead of being overwritten by stale shared data. Tracker
/// capacity is never recovered by evicting a live degraded write; new writes surface the primary
/// error when they cannot be tracked safely.
pub(crate) struct DegradationAwareFallbackBackend {
    primary: Arc<dyn CacheBackend>,
    fallback: Arc<InMemoryCacheBackend>,
    degraded_writes: DegradedWriteTracker,
}

impl DegradationAwareFallbackBackend {
    pub(crate) fn new(primary: Arc<dyn CacheBackend>, fallback: Arc<InMemoryCacheBackend>) -> Self {
        Self {
            primary,
            fallback,
            degraded_writes: DegradedWriteTracker::new(MAX_DEGRADED_WRITE_KEYS),
        }
    }

    async fn mark_degraded_write(&self, key: &str, expected: &[u8]) -> rustok_core::Result<()> {
        self.degraded_writes
            .insert(key, expected, &self.fallback)
            .await
    }

    async fn clear_degraded_write(&self, key: &str) {
        self.degraded_writes.remove(key).await;
    }

    async fn read_degraded_write(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        if !self.degraded_writes.contains(key).await {
            return Ok(None);
        }

        match self.fallback.get(key).await? {
            Some(value) => Ok(Some(value)),
            None => {
                // The payload expired or was evicted. Drop its tracker entry and resume normal
                // primary-first reads rather than turning the marker into a sticky miss.
                self.clear_degraded_write(key).await;
                Ok(None)
            }
        }
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

    async fn retain_degraded_write_or_fail(
        &self,
        key: &str,
        expected: &[u8],
        primary_error: rustok_core::Error,
    ) -> rustok_core::Result<()> {
        if let Err(tracker_error) = self.mark_degraded_write(key, expected).await {
            let _ = self.fallback.invalidate(key).await;
            tracing::warn!(
                error = %primary_error,
                %tracker_error,
                key,
                "Primary cache write failed and bounded local retention was unavailable"
            );
            return Err(primary_error);
        }

        tracing::debug!(
            error = %primary_error,
            key,
            "Primary cache write failed; retained bounded local value"
        );
        Ok(())
    }
}

#[async_trait]
impl CacheBackend for DegradationAwareFallbackBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.primary.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        if let Some(value) = self.read_degraded_write(key).await? {
            return Ok(Some(value));
        }

        match self.primary.get(key).await {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => {
                // A successful primary miss is authoritative. Remove any older local mirror so a
                // later Redis outage cannot resurrect stale bytes.
                self.clear_degraded_write(key).await;
                if let Err(error) = self.fallback.invalidate(key).await {
                    tracing::warn!(%error, key, "Failed to discard stale local cache mirror");
                }
                Ok(None)
            }
            Err(error) => {
                tracing::debug!(%error, key, "Primary cache GET failed, using local fallback");
                self.fallback.get(key).await
            }
        }
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        if let Err(error) = self.fallback.set(key.clone(), value.clone()).await {
            tracing::warn!(%error, key, "Failed to update local cache fallback");
        }

        match self.primary.set(key.clone(), value.clone()).await {
            Ok(()) => {
                self.clear_degraded_write(&key).await;
                Ok(())
            }
            Err(error) => {
                self.retain_degraded_write_or_fail(&key, &value, error)
                    .await
            }
        }
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        if ttl.is_zero() {
            // Zero TTL is a deletion, not an availability-preserving write. A failed shared delete
            // must remain visible to callers or stale Redis bytes could be reported as removed.
            return self.invalidate(&key).await;
        }

        if let Err(error) = self
            .fallback
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await
        {
            tracing::warn!(%error, key, "Failed to update local cache fallback");
        }

        match self
            .primary
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await
        {
            Ok(()) => {
                self.clear_degraded_write(&key).await;
                Ok(())
            }
            Err(error) => {
                self.retain_degraded_write_or_fail(&key, &value, error)
                    .await
            }
        }
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
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
                if let Err(error) = self.fallback.invalidate(key).await {
                    tracing::warn!(
                        %error,
                        key,
                        "Failed to discard local mirror after CAS mismatch"
                    );
                }
            }
        }
        Ok(outcome)
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        if let Err(error) = self.fallback.invalidate(key).await {
            tracing::warn!(%error, key, "Failed to invalidate local cache fallback");
        }
        self.clear_degraded_write(key).await;

        self.primary.invalidate(key).await.map_err(|error| {
            tracing::warn!(
                %error,
                key,
                "Primary cache invalidation failed; stale shared data may remain"
            );
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    struct HealthControlledBackend {
        healthy: AtomicBool,
    }

    #[async_trait]
    impl CacheBackend for HealthControlledBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            if self.healthy.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(rustok_core::Error::Cache(
                    "shared primary unavailable".to_string(),
                ))
            }
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            Err(rustok_core::Error::Cache(
                "shared primary unavailable".to_string(),
            ))
        }

        async fn set(&self, _key: String, _value: Vec<u8>) -> rustok_core::Result<()> {
            Err(rustok_core::Error::Cache(
                "shared primary unavailable".to_string(),
            ))
        }

        async fn set_with_ttl(
            &self,
            key: String,
            value: Vec<u8>,
            _ttl: Duration,
        ) -> rustok_core::Result<()> {
            self.set(key, value).await
        }

        async fn compare_and_set(
            &self,
            _key: &str,
            _expected: &[u8],
            _value: Vec<u8>,
            _ttl: Option<Duration>,
        ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
            Err(rustok_core::Error::Cache(
                "shared primary unavailable".to_string(),
            ))
        }

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            Err(rustok_core::Error::Cache(
                "shared primary unavailable".to_string(),
            ))
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    struct MissThenFailBackend {
        failing: AtomicBool,
    }

    #[async_trait]
    impl CacheBackend for MissThenFailBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            if self.failing.load(Ordering::SeqCst) {
                Err(rustok_core::Error::Cache(
                    "shared primary unavailable".to_string(),
                ))
            } else {
                Ok(None)
            }
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
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    struct RecoveringStaleBackend {
        fail_writes: AtomicBool,
        value: Mutex<Option<Vec<u8>>>,
    }

    #[async_trait]
    impl CacheBackend for RecoveringStaleBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            Ok(self
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone())
        }

        async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
            self.set_with_ttl(key, value, Duration::from_secs(30)).await
        }

        async fn set_with_ttl(
            &self,
            _key: String,
            value: Vec<u8>,
            _ttl: Duration,
        ) -> rustok_core::Result<()> {
            if self.fail_writes.load(Ordering::SeqCst) {
                return Err(rustok_core::Error::Cache(
                    "shared primary unavailable".to_string(),
                ));
            }
            *self
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value);
            Ok(())
        }

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            if self.fail_writes.load(Ordering::SeqCst) {
                return Err(rustok_core::Error::Cache(
                    "shared primary unavailable".to_string(),
                ));
            }
            *self
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    fn backend(
        primary: Arc<dyn CacheBackend>,
        fallback: Arc<InMemoryCacheBackend>,
    ) -> DegradationAwareFallbackBackend {
        DegradationAwareFallbackBackend::new(primary, fallback)
    }

    #[tokio::test]
    async fn degraded_tracker_rejects_missing_or_stale_local_payload() {
        let tracker = DegradedWriteTracker::new(1);
        let fallback = InMemoryCacheBackend::new(Duration::from_secs(30), 4);

        assert!(tracker
            .insert("missing", b"new", &fallback)
            .await
            .is_err());
        fallback
            .set("stale".to_string(), b"old".to_vec())
            .await
            .unwrap();
        assert!(tracker.insert("stale", b"new", &fallback).await.is_err());
        assert!(!tracker.contains("missing").await);
        assert!(!tracker.contains("stale").await);
    }

    #[tokio::test]
    async fn degraded_tracker_never_evicts_a_live_key_at_capacity() {
        let tracker = DegradedWriteTracker::new(1);
        let fallback = InMemoryCacheBackend::new(Duration::from_secs(30), 4);
        fallback
            .set("first".to_string(), b"one".to_vec())
            .await
            .unwrap();
        fallback
            .set("second".to_string(), b"two".to_vec())
            .await
            .unwrap();

        tracker.insert("first", b"one", &fallback).await.unwrap();
        assert!(tracker.insert("second", b"two", &fallback).await.is_err());
        assert!(tracker.contains("first").await);

        tracker.remove("first").await;
        tracker.insert("second", b"two", &fallback).await.unwrap();
        assert!(!tracker.contains("first").await);
        assert!(tracker.contains("second").await);
        assert_eq!(tracker.state.lock().await.insertion_order.len(), 1);
    }

    #[tokio::test]
    async fn reports_primary_degradation_while_local_fallback_still_serves_writes() {
        let primary = Arc::new(HealthControlledBackend {
            healthy: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(primary, fallback);

        backend
            .set("key".to_string(), b"local".to_vec())
            .await
            .unwrap();

        assert!(backend.health().await.is_err());
        assert_eq!(backend.get("key").await.unwrap(), Some(b"local".to_vec()));
    }

    #[tokio::test]
    async fn compare_and_set_fails_closed_when_shared_primary_is_unavailable() {
        let primary = Arc::new(HealthControlledBackend {
            healthy: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        fallback
            .set("key".to_string(), b"local".to_vec())
            .await
            .unwrap();
        let backend = backend(primary, Arc::clone(&fallback));

        assert!(backend
            .compare_and_set("key", b"local", b"new".to_vec(), None)
            .await
            .is_err());
        assert_eq!(fallback.get("key").await.unwrap(), Some(b"local".to_vec()));
    }

    #[tokio::test]
    async fn primary_miss_prevents_stale_fallback_resurrection_during_later_outage() {
        let primary = Arc::new(MissThenFailBackend {
            failing: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        fallback
            .set("key".to_string(), b"stale".to_vec())
            .await
            .unwrap();
        let primary_backend: Arc<dyn CacheBackend> = primary.clone();
        let backend = backend(primary_backend, Arc::clone(&fallback));

        assert_eq!(backend.get("key").await.unwrap(), None);
        assert_eq!(fallback.get("key").await.unwrap(), None);

        primary.failing.store(true, Ordering::SeqCst);
        assert_eq!(backend.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn recovered_stale_primary_does_not_override_a_degraded_write() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: Mutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let primary_backend: Arc<dyn CacheBackend> = primary.clone();
        let backend = backend(primary_backend, fallback);

        backend
            .set("key".to_string(), b"new".to_vec())
            .await
            .unwrap();
        primary.fail_writes.store(false, Ordering::SeqCst);

        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn zero_ttl_delete_fails_closed_when_shared_primary_is_unavailable() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: Mutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(primary, fallback);

        assert!(backend
            .set_with_ttl("key".to_string(), Vec::new(), Duration::ZERO)
            .await
            .is_err());
    }
}

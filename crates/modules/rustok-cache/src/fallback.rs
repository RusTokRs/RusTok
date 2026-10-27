use std::collections::{HashSet, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats, InMemoryCacheBackend};
use tokio::sync::Mutex;

const MAX_DEGRADED_WRITE_KEYS: usize = 4_096;
const MAX_DEGRADED_WRITE_KEY_BYTES: usize = 4 * 1_024;
const DEGRADED_WRITE_PRUNE_SCAN: usize = 16;
const MAX_PENDING_INVALIDATION_KEYS: usize = 4_096;
const FALLBACK_KEY_LOCK_STRIPES: usize = 256;

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

    async fn clear(&self) {
        let mut state = self.state.lock().await;
        state.keys.clear();
        state.insertion_order.clear();
    }

    async fn insert(
        &self,
        key: &str,
        expected: &[u8],
        fallback: &InMemoryCacheBackend,
    ) -> rustok_core::Result<()> {
        validate_degraded_key(key, "write")?;
        if fallback.get(key).await?.as_deref() != Some(expected) {
            return Err(rustok_core::Error::Cache(
                "degraded cache write bytes were not retained by the local fallback".to_string(),
            ));
        }

        let mut state = self.state.lock().await;
        if state.keys.contains(key) {
            return Ok(());
        }

        let scan = DEGRADED_WRITE_PRUNE_SCAN.min(state.insertion_order.len());
        for _ in 0..scan {
            let Some(candidate) = state.insertion_order.pop_front() else {
                break;
            };
            if !state.keys.contains(&candidate) {
                continue;
            }
            if fallback.get(&candidate).await?.is_none() {
                state.keys.remove(&candidate);
            } else {
                state.insertion_order.push_back(candidate);
            }
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

#[derive(Debug)]
struct PendingInvalidationTracker {
    keys: Mutex<HashSet<String>>,
    maximum_keys: usize,
}

impl PendingInvalidationTracker {
    fn new(maximum_keys: usize) -> Self {
        Self {
            keys: Mutex::new(HashSet::new()),
            maximum_keys,
        }
    }

    async fn contains(&self, key: &str) -> bool {
        self.keys.lock().await.contains(key)
    }

    async fn remove(&self, key: &str) {
        self.keys.lock().await.remove(key);
    }

    async fn clear(&self) {
        self.keys.lock().await.clear();
    }

    async fn snapshot(&self) -> Vec<String> {
        self.keys.lock().await.iter().cloned().collect()
    }

    async fn is_empty(&self) -> bool {
        self.keys.lock().await.is_empty()
    }

    async fn insert(&self, key: &str) -> rustok_core::Result<()> {
        validate_degraded_key(key, "invalidation")?;
        let mut keys = self.keys.lock().await;
        if keys.contains(key) {
            return Ok(());
        }
        if keys.len() >= self.maximum_keys {
            return Err(rustok_core::Error::Cache(format!(
                "pending cache invalidation tracker reached capacity {}",
                self.maximum_keys
            )));
        }
        keys.insert(key.to_string());
        Ok(())
    }
}

fn validate_degraded_key(key: &str, operation: &str) -> rustok_core::Result<()> {
    if key.len() > MAX_DEGRADED_WRITE_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "degraded cache {operation} key is {} bytes; maximum is {MAX_DEGRADED_WRITE_KEY_BYTES}",
            key.len()
        )));
    }
    Ok(())
}

/// Availability-preserving fallback whose health and atomic mutations remain strict about the
/// shared primary.
///
/// Every write is retained locally before it reaches Redis, but only writes whose primary
/// operation **failed** are marked as authoritative local state. Reads therefore serve local bytes
/// exclusively for those marked writes: an ordinary primary miss never resurrects a value that only
/// exists as an unmarked local mirror. Failed invalidations retain a separate bounded process-local
/// tombstone: reads never resurrect stale shared bytes, but they also never replay a delete because
/// a newer cross-instance write may have happened after the failed invalidation. Only an explicit
/// invalidation retry may clear shared state. Tracker capacity is never recovered by evicting a live
/// mutation; when a new unsynchronized mutation cannot be retained safely the backend enters a
/// fail-closed state in which reads, writes and CAS are rejected until the owner confirms a healthy
/// shared primary through [`CacheBackend::reconcile_local_state`]. A fixed set of striped locks
/// serializes local bookkeeping for the same key without introducing an unbounded per-key registry;
/// network calls are made outside those stripes so a slow primary cannot serialize unrelated keys.
pub struct DegradationAwareFallbackBackend {
    primary: Arc<dyn CacheBackend>,
    fallback: Arc<InMemoryCacheBackend>,
    degraded_writes: DegradedWriteTracker,
    pending_invalidations: PendingInvalidationTracker,
    key_locks: Vec<Mutex<()>>,
    tombstone_saturation: AtomicBool,
}

impl DegradationAwareFallbackBackend {
    pub(crate) fn new(primary: Arc<dyn CacheBackend>, fallback: Arc<InMemoryCacheBackend>) -> Self {
        Self {
            primary,
            fallback,
            degraded_writes: DegradedWriteTracker::new(MAX_DEGRADED_WRITE_KEYS),
            pending_invalidations: PendingInvalidationTracker::new(MAX_PENDING_INVALIDATION_KEYS),
            key_locks: (0..FALLBACK_KEY_LOCK_STRIPES)
                .map(|_| Mutex::new(()))
                .collect(),
            tombstone_saturation: AtomicBool::new(false),
        }
    }

    fn saturation_error() -> rustok_core::Error {
        rustok_core::Error::Cache(
            "cache invalidation tombstone tracker saturated; cache reads and writes are suspended until \
             CacheBackend::reconcile_local_state() confirms a healthy shared primary"
                .to_string(),
        )
    }

    fn ensure_operation_allowed(&self) -> rustok_core::Result<()> {
        if self.tombstone_saturation.load(Ordering::Acquire) {
            return Err(Self::saturation_error());
        }
        Ok(())
    }

    fn key_lock(&self, key: &str) -> &Mutex<()> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        &self.key_locks[(hasher.finish() as usize) % self.key_locks.len()]
    }


    async fn mark_degraded_write(&self, key: &str, expected: &[u8]) -> rustok_core::Result<()> {
        self.degraded_writes
            .insert(key, expected, &self.fallback)
            .await
    }

    async fn clear_degraded_write(&self, key: &str) {
        self.degraded_writes.remove(key).await;
    }

    async fn clear_pending_invalidation(&self, key: &str) {
        self.pending_invalidations.remove(key).await;
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

    async fn pending_invalidation_active(&self, key: &str) -> bool {
        if !self.pending_invalidations.contains(key).await {
            return false;
        }

        if let Err(error) = self.fallback.invalidate(key).await {
            tracing::warn!(%error, key, "Failed to preserve local cache invalidation tombstone");
        }
        tracing::debug!(
            key,
            "Suppressing shared cache value until an explicit invalidation retry or newer write"
        );
        true
    }

    async fn discard_local_write_if_unchanged(&self, key: &str, expected: &[u8]) {
        match self
            .fallback
            .compare_and_set(key, expected, Vec::new(), Some(Duration::ZERO))
            .await
        {
            Ok(CacheCompareAndSetOutcome::Applied | CacheCompareAndSetOutcome::Mismatch) => {}
            Err(error) => {
                tracing::warn!(
                    %error,
                    key,
                    "Failed to conditionally discard an untracked degraded cache write"
                );
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
            self.discard_local_write_if_unchanged(key, expected).await;
            tracing::warn!(
                error = %primary_error,
                %tracker_error,
                key,
                "Primary cache write failed and bounded local retention was unavailable"
            );
            return Err(primary_error);
        }

        self.clear_pending_invalidation(key).await;
        tracing::debug!(
            error = %primary_error,
            key,
            "Primary cache write failed; retained bounded local value"
        );
        Ok(())
    }

    /// Retain a write locally, publish it to the shared primary outside the key stripe, then
    /// record or clear the degraded marker under the stripe.
    ///
    /// The order is deliberate: local bytes exist before the network attempt, so a primary failure
    /// can be retained with [`DegradedWriteTracker`]; the marker is committed only after the primary
    /// outcome is known, and the byte comparison in `insert` rejects a marker whose payload a
    /// concurrent writer already replaced.
    async fn write_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<()> {
        {
            let _guard = self.key_lock(&key).lock().await;
            self.ensure_operation_allowed()?;
            let local = match ttl {
                Some(ttl) => {
                    self.fallback
                        .set_with_ttl(key.clone(), value.clone(), ttl)
                        .await
                }
                None => self.fallback.set(key.clone(), value.clone()).await,
            };
            if let Err(error) = local {
                tracing::warn!(%error, key, "Failed to retain local cache fallback write");
            }
        }

        let primary_result = match ttl {
            Some(ttl) => {
                self.primary
                    .set_with_ttl(key.clone(), value.clone(), ttl)
                    .await
            }
            None => self.primary.set(key.clone(), value.clone()).await,
        };

        let _guard = self.key_lock(&key).lock().await;
        match primary_result {
            Ok(()) => {
                self.clear_degraded_write(&key).await;
                self.clear_pending_invalidation(&key).await;
                Ok(())
            }
            Err(error) => self.retain_degraded_write_or_fail(&key, &value, error).await,
        }
    }

    async fn invalidate_locked(&self, key: &str) -> rustok_core::Result<()> {
        if let Err(error) = self.fallback.invalidate(key).await {
            tracing::warn!(%error, key, "Failed to invalidate local cache fallback");
        }
        self.clear_degraded_write(key).await;

        match self.primary.invalidate(key).await {
            Ok(()) => {
                self.clear_pending_invalidation(key).await;
                Ok(())
            }
            Err(error) => {
                if let Err(tracker_error) = self.pending_invalidations.insert(key).await {
                    self.tombstone_saturation.store(true, Ordering::Release);
                    tracing::warn!(
                        %error,
                        %tracker_error,
                        key,
                        "Primary cache invalidation failed and local tombstone retention was unavailable; entered fail-closed saturation state"
                    );
                } else {
                    tracing::warn!(
                        %error,
                        key,
                        "Primary cache invalidation failed; retained bounded local tombstone"
                    );
                }
                Err(error)
            }
        }
    }

    async fn has_unsynchronized_mutation(&self, key: &str) -> bool {
        self.degraded_writes.contains(key).await || self.pending_invalidations.contains(key).await
    }
}

#[async_trait]
impl CacheBackend for DegradationAwareFallbackBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.ensure_operation_allowed()?;
        self.primary.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        // A saturated backend must not serve anything: unmarked local bytes could be older than the
        // shared value, and shared bytes may be stale because an invalidation was never retained.
        self.ensure_operation_allowed()?;

        {
            let _guard = self.key_lock(key).lock().await;
            if self.pending_invalidation_active(key).await {
                return Ok(None);
            }
            if let Some(value) = self.read_degraded_write(key).await? {
                return Ok(Some(value));
            }
        }

        let primary_result = self.primary.get(key).await;

        let _guard = self.key_lock(key).lock().await;
        // Re-check after async read to catch a saturation or invalidation that raced with the network call.
        self.ensure_operation_allowed()?;
        if self.pending_invalidation_active(key).await {
            return Ok(None);
        }
        if let Some(value) = self.read_degraded_write(key).await? {
            return Ok(Some(value));
        }

        match primary_result {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => {
                // A successful primary miss is authoritative. Drop any older marked local write so a
                // later outage cannot resurrect bytes the shared primary has already discarded.
                self.clear_degraded_write(key).await;
                self.clear_pending_invalidation(key).await;
                if let Err(error) = self.fallback.invalidate(key).await {
                    tracing::warn!(%error, key, "Failed to discard stale local cache state");
                }
                Ok(None)
            }
            Err(error) => {
                // Degraded reads serve only locally confirmed degraded writes. An unmarked local
                // mirror is deliberately ignored: it may predate the current shared value and its
                // remaining lifetime is not observable through this backend.
                tracing::debug!(
                    %error,
                    key,
                    "Primary cache GET failed; serving only locally confirmed degraded writes"
                );
                Ok(None)
            }
        }
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.write_with_ttl(key, value, None).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        if ttl.is_zero() {
            // Zero TTL is a deletion, not an availability-preserving write. A failed shared delete
            // remains visible to the caller while a bounded local tombstone prevents resurrection.
            let _guard = self.key_lock(&key).lock().await;
            self.ensure_operation_allowed()?;
            return self.invalidate_locked(&key).await;
        }
        self.write_with_ttl(key, value, Some(ttl)).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        {
            let _guard = self.key_lock(key).lock().await;
            self.ensure_operation_allowed()?;
            if self.has_unsynchronized_mutation(key).await {
                return Err(rustok_core::Error::Cache(
                    "cache compare-and-set rejected while local and shared state are unsynchronized"
                        .to_string(),
                ));
            }
        }

        // The shared primary is authoritative for CAS: no stripe is held across the network call so
        // an unavailable primary cannot serialize unrelated keys.
        let outcome = self
            .primary
            .compare_and_set(key, expected, value.clone(), ttl)
            .await?;

        let _guard = self.key_lock(key).lock().await;
        match outcome {
            CacheCompareAndSetOutcome::Applied => {
                self.clear_degraded_write(key).await;
                self.clear_pending_invalidation(key).await;
                self.mirror_primary_cas(key, value, ttl).await;
            }
            CacheCompareAndSetOutcome::Mismatch => {
                self.clear_degraded_write(key).await;
                self.clear_pending_invalidation(key).await;
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
        let _guard = self.key_lock(key).lock().await;
        self.ensure_operation_allowed()?;
        self.invalidate_locked(key).await
    }

    async fn reconcile_local_state(&self) -> rustok_core::Result<()> {
        // Clearing fail-closed state on a bare health signal would hide an invalidation that never
        // happened, so reconciliation both confirms the shared primary and *replays* every retained
        // invalidation. Saturation is lifted only when the whole replay succeeded.
        self.primary.health().await?;

        let mut replay_failures = 0usize;
        for key in self.pending_invalidations.snapshot().await {
            let _guard = self.key_lock(&key).lock().await;
            match self.primary.invalidate(&key).await {
                Ok(()) => {
                    if let Err(error) = self.fallback.invalidate(&key).await {
                        tracing::warn!(
                            %error,
                            key,
                            "Failed to discard local state during cache reconciliation"
                        );
                    }
                    self.degraded_writes.remove(&key).await;
                    self.pending_invalidations.remove(&key).await;
                }
                Err(error) => {
                    replay_failures = replay_failures.saturating_add(1);
                    tracing::warn!(
                        %error,
                        key,
                        "Retained cache invalidation replay failed during reconciliation"
                    );
                }
            }
        }

        // Degraded writes are newer local bytes that were never acknowledged to the shared primary.
        // They cannot be replayed safely (a newer cross-instance write may exist), so reconciliation
        // drops the markers: those entries become misses instead of stale hits.
        self.degraded_writes.clear().await;

        if replay_failures > 0 {
            return Err(rustok_core::Error::Cache(format!(
                "cache fallback reconciliation replayed {} retained invalidation(s) unsuccessfully; \
                 fail-closed state is retained",
                replay_failures
            )));
        }

        self.tombstone_saturation.store(false, Ordering::Release);
        tracing::info!(
            "Reconciled degradation-aware cache fallback after primary health confirmation and invalidation replay"
        );
        Ok(())
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
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::Notify;

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

    struct BlockingMissBackend {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[async_trait]
    impl CacheBackend for BlockingMissBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            self.started.notify_one();
            self.release.notified().await;
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
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    struct RecoveringStaleBackend {
        fail_writes: AtomicBool,
        value: StdMutex<Option<Vec<u8>>>,
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

    #[test]
    fn same_key_uses_one_bounded_lock_stripe() {
        let primary = Arc::new(HealthControlledBackend {
            healthy: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(primary, fallback);

        assert_eq!(backend.key_locks.len(), FALLBACK_KEY_LOCK_STRIPES);
        assert!(std::ptr::eq(
            backend.key_lock("same-key"),
            backend.key_lock("same-key")
        ));
    }

    #[tokio::test]
    async fn failed_write_cleanup_does_not_delete_newer_local_bytes() {
        let primary = Arc::new(HealthControlledBackend {
            healthy: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(primary, Arc::clone(&fallback));
        fallback
            .set("key".to_string(), b"newer".to_vec())
            .await
            .unwrap();

        backend
            .discard_local_write_if_unchanged("key", b"older")
            .await;
        assert_eq!(fallback.get("key").await.unwrap(), Some(b"newer".to_vec()));

        backend
            .discard_local_write_if_unchanged("key", b"newer")
            .await;
        assert_eq!(fallback.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn primary_miss_rechecks_degraded_write_completed_during_read() {
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let primary = Arc::new(BlockingMissBackend {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = Arc::new(backend(primary, Arc::clone(&fallback)));
        let started_wait = started.notified();
        let get_backend = Arc::clone(&backend);
        let read = tokio::spawn(async move { get_backend.get("key").await.unwrap() });

        started_wait.await;
        fallback
            .set("key".to_string(), b"local".to_vec())
            .await
            .unwrap();
        backend
            .degraded_writes
            .insert("key", b"local", &fallback)
            .await
            .unwrap();
        release.notify_one();

        assert_eq!(read.await.unwrap(), Some(b"local".to_vec()));
        assert_eq!(fallback.get("key").await.unwrap(), Some(b"local".to_vec()));
    }

    #[tokio::test]
    async fn degraded_tracker_rejects_missing_or_stale_local_payload() {
        let tracker = DegradedWriteTracker::new(1);
        let fallback = InMemoryCacheBackend::new(Duration::from_secs(30), 4);

        assert!(tracker.insert("missing", b"new", &fallback).await.is_err());
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

        assert!(
            backend
                .compare_and_set("key", b"local", b"new".to_vec(), None)
                .await
                .is_err()
        );
        assert_eq!(fallback.get("key").await.unwrap(), Some(b"local".to_vec()));
    }

    #[tokio::test]
    async fn compare_and_set_rejects_a_tracked_degraded_write_after_primary_recovers() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let primary_backend: Arc<dyn CacheBackend> = primary.clone();
        let backend = backend(primary_backend, Arc::clone(&fallback));

        backend
            .set("key".to_string(), b"local-new".to_vec())
            .await
            .unwrap();
        primary.fail_writes.store(false, Ordering::SeqCst);

        let error = backend
            .compare_and_set("key", b"local-new", b"cas-value".to_vec(), None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unsynchronized"));
        assert_eq!(
            backend.get("key").await.unwrap(),
            Some(b"local-new".to_vec())
        );
        assert_eq!(
            primary
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_deref(),
            Some(b"old".as_slice())
        );
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
            value: StdMutex::new(Some(b"old".to_vec())),
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
    async fn unmarked_local_mirror_is_never_served_during_a_primary_outage() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(false),
            value: StdMutex::new(Some(b"fresh".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        fallback
            .set("key".to_string(), b"stale".to_vec())
            .await
            .unwrap();
        let backend = backend(Arc::clone(&primary) as Arc<dyn CacheBackend>, Arc::clone(&fallback));

        // The local layer may hold bytes the shared primary never acknowledged; a read must use the
        // primary value and must not treat the local copy as authoritative.
        assert_eq!(backend.get("key").await.unwrap(), Some(b"fresh".to_vec()));
        assert!(!backend.degraded_writes.contains("key").await);

        // With the primary unavailable those older local bytes stay unreachable: the entry becomes a
        // miss instead of a stale hit whose remaining lifetime this backend cannot observe.
        primary.fail_writes.store(true, Ordering::SeqCst);
        assert_eq!(backend.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn degraded_read_prefers_the_marked_write_over_the_shared_value() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(Arc::clone(&primary) as Arc<dyn CacheBackend>, fallback);

        backend
            .set("key".to_string(), b"new".to_vec())
            .await
            .unwrap();

        // Read while the primary is still failing: the marked local write is authoritative.
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
        primary.fail_writes.store(false, Ordering::SeqCst);
        // Once the primary answers, the marked local write still wins until it is republished or the
        // marker expires; it is never silently replaced by older shared bytes.
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn failed_invalidation_tombstone_requires_explicit_retry() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        fallback
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();
        let primary_backend: Arc<dyn CacheBackend> = primary.clone();
        let backend = backend(primary_backend, Arc::clone(&fallback));

        assert!(backend.invalidate("key").await.is_err());
        assert!(backend.pending_invalidations.contains("key").await);
        assert_eq!(backend.get("key").await.unwrap(), None);
        assert_eq!(fallback.get("key").await.unwrap(), None);

        primary.fail_writes.store(false, Ordering::SeqCst);
        *primary
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(b"newer".to_vec());

        assert_eq!(backend.get("key").await.unwrap(), None);
        assert!(backend.pending_invalidations.contains("key").await);
        assert_eq!(
            primary
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_deref(),
            Some(b"newer".as_slice())
        );

        backend.invalidate("key").await.unwrap();
        assert!(!backend.pending_invalidations.contains("key").await);
        assert!(
            primary
                .value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
        );
    }

    #[tokio::test]
    async fn newer_degraded_write_supersedes_a_pending_invalidation() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let primary_backend: Arc<dyn CacheBackend> = primary.clone();
        let backend = backend(primary_backend, Arc::clone(&fallback));

        assert!(backend.invalidate("key").await.is_err());
        assert!(backend.pending_invalidations.contains("key").await);
        backend
            .set("key".to_string(), b"new".to_vec())
            .await
            .unwrap();

        assert!(!backend.pending_invalidations.contains("key").await);
        assert!(backend.degraded_writes.contains("key").await);
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn degraded_tracker_rotates_prune_candidates_without_tombstone_growth() {
        let tracker = DegradedWriteTracker::new(3);
        let fallback = InMemoryCacheBackend::new(Duration::from_secs(30), 64);
        fallback
            .set("live".to_string(), b"live".to_vec())
            .await
            .unwrap();
        tracker.insert("live", b"live", &fallback).await.unwrap();

        for index in 0..64 {
            let key = format!("expired-{index}");
            fallback
                .set(key.clone(), b"temporary".to_vec())
                .await
                .unwrap();
            tracker.insert(&key, b"temporary", &fallback).await.unwrap();
            fallback.invalidate(&key).await.unwrap();
        }

        fallback
            .set("fresh".to_string(), b"fresh".to_vec())
            .await
            .unwrap();
        tracker.insert("fresh", b"fresh", &fallback).await.unwrap();

        let state = tracker.state.lock().await;
        assert!(state.keys.contains("live"));
        assert!(state.keys.contains("fresh"));
        assert!(state.insertion_order.len() <= tracker.maximum_keys);
        assert_eq!(state.insertion_order.len(), state.keys.len());
    }

    #[tokio::test]
    async fn pending_invalidation_tracker_is_bounded_and_never_evicts_live_tombstones() {
        let tracker = PendingInvalidationTracker::new(1);
        tracker.insert("first").await.unwrap();
        assert!(tracker.insert("second").await.is_err());
        assert!(tracker.contains("first").await);
        assert!(!tracker.contains("second").await);

        tracker.remove("first").await;
        tracker.insert("second").await.unwrap();
        assert!(!tracker.contains("first").await);
        assert!(tracker.contains("second").await);
    }

    #[tokio::test]
    async fn zero_ttl_delete_fails_closed_when_shared_primary_is_unavailable() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"old".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(primary, fallback);

        assert!(
            backend
                .set_with_ttl("key".to_string(), Vec::new(), Duration::ZERO)
                .await
                .is_err()
        );
        assert!(backend.pending_invalidations.contains("key").await);
        assert_eq!(backend.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn tombstone_tracker_saturation_fails_closed_and_prevents_stale_primary_hits_until_reconciled()
     {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"stale".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let mut backend = backend(primary.clone(), fallback);
        backend.pending_invalidations = PendingInvalidationTracker::new(1);

        // First invalidation fails on primary, successfully tracked as tombstone
        assert!(backend.invalidate("key-1").await.is_err());
        assert_eq!(backend.get("key-1").await.unwrap(), None);
        assert!(backend.health().await.is_ok());

        // Second invalidation fails on primary and exceeds capacity (1)
        assert!(backend.invalidate("key-2").await.is_err());

        // Health/readiness now fails closed
        assert!(backend.health().await.is_err());

        // Fail-closed applies to *every* read path, including a primary that still fails: an
        // untracked invalidation means both shared bytes and local bytes may be stale.
        assert!(backend.get("key-2").await.is_err());
        assert!(backend.get("key-1").await.is_err());
        assert!(backend.set("key-3".to_string(), b"v".to_vec()).await.is_err());

        // Reconciliation while the primary is still unhealthy must be rejected...
        assert!(backend.reconcile_local_state().await.is_err());
        assert!(backend.health().await.is_err());

        // ...and must replay retained invalidations before lifting the fail-closed state.
        primary.fail_writes.store(false, Ordering::SeqCst);
        backend.reconcile_local_state().await.unwrap();
        assert!(backend.health().await.is_ok());

        // The replayed tombstone is gone and the confirmed primary is authoritative again.
        assert_eq!(backend.get("key-2").await.unwrap(), None);
    }

    #[tokio::test]
    async fn reconciliation_stays_fail_closed_while_a_retained_invalidation_cannot_be_replayed() {
        let primary = Arc::new(RecoveringStaleBackend {
            fail_writes: AtomicBool::new(true),
            value: StdMutex::new(Some(b"stale".to_vec())),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = backend(Arc::clone(&primary) as Arc<dyn CacheBackend>, fallback);

        assert!(backend.invalidate("key").await.is_err());
        assert!(!backend.pending_invalidations.is_empty().await);

        let error = backend.reconcile_local_state().await.unwrap_err().to_string();
        assert!(error.contains("retained invalidation"));
        assert!(backend.get("key").await.is_err());

        primary.fail_writes.store(false, Ordering::SeqCst);
        backend.reconcile_local_state().await.unwrap();
        assert!(backend.health().await.is_ok());
    }
}

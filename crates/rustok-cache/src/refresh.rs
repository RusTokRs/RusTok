use std::collections::HashSet;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use rustok_core::CacheBackend;

use crate::{
    CacheEnvelope, CacheEnvelopeFreshness, CacheLoadPolicy, CacheLoadSource, CacheService,
    TypedCacheLoadResult, DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
};

pub const MAX_CACHE_REFRESH_KEY_BYTES: usize = crate::service::MAX_CACHE_LOAD_KEY_BYTES;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheRefreshKey {
    backend_id: usize,
    key: String,
}

impl CacheRefreshKey {
    fn new(backend: &Arc<dyn CacheBackend>, key: &str) -> Self {
        Self {
            backend_id: Arc::as_ptr(backend) as *const () as usize,
            key: key.to_string(),
        }
    }
}

#[derive(Default)]
struct CacheRefreshMetrics {
    started: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    deduplicated: AtomicU64,
    saturated: AtomicU64,
    rejected: AtomicU64,
    runtime_unavailable: AtomicU64,
}

struct CacheRefreshInner {
    in_flight: Arc<StdMutex<HashSet<CacheRefreshKey>>>,
    permits: Arc<Semaphore>,
    metrics: CacheRefreshMetrics,
}

/// Bounded process-local coordinator for stale-while-revalidate refresh work.
///
/// Refresh identity is scoped by both backend instance and key. A global semaphore limits
/// refresh concurrency, while a per-key lease prevents duplicate refresh tasks for the same
/// cache entry. Failed refreshes leave the stale value untouched until its hard expiry.
#[derive(Clone)]
pub struct CacheRefreshCoordinator {
    inner: Arc<CacheRefreshInner>,
}

impl CacheRefreshCoordinator {
    pub fn new(max_concurrent_refreshes: usize) -> Result<Self, CacheRefreshCoordinatorError> {
        if max_concurrent_refreshes == 0 {
            return Err(CacheRefreshCoordinatorError::ZeroConcurrency);
        }
        Ok(Self {
            inner: Arc::new(CacheRefreshInner {
                in_flight: Arc::new(StdMutex::new(HashSet::new())),
                permits: Arc::new(Semaphore::new(max_concurrent_refreshes)),
                metrics: CacheRefreshMetrics::default(),
            }),
        })
    }

    /// Schedule refresh work without blocking the request serving stale data.
    pub fn schedule<F, Fut>(
        &self,
        backend: &Arc<dyn CacheBackend>,
        key: impl Into<String>,
        refresh: F,
    ) -> CacheRefreshSchedule
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = rustok_core::Result<()>> + Send + 'static,
    {
        let key = key.into();
        if validate_refresh_key(&key).is_err() {
            self.inner
                .metrics
                .rejected
                .fetch_add(1, Ordering::Relaxed);
            return CacheRefreshSchedule::InvalidKey;
        }

        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            self.inner
                .metrics
                .runtime_unavailable
                .fetch_add(1, Ordering::Relaxed);
            return CacheRefreshSchedule::RuntimeUnavailable;
        };

        let refresh_key = CacheRefreshKey::new(backend, &key);
        {
            let mut in_flight = self
                .inner
                .in_flight
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !in_flight.insert(refresh_key.clone()) {
                self.inner
                    .metrics
                    .deduplicated
                    .fetch_add(1, Ordering::Relaxed);
                return CacheRefreshSchedule::AlreadyRunning;
            }
        }

        let permit = match Arc::clone(&self.inner.permits).try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                self.remove_in_flight(&refresh_key);
                self.inner
                    .metrics
                    .saturated
                    .fetch_add(1, Ordering::Relaxed);
                return CacheRefreshSchedule::AtCapacity;
            }
        };

        self.inner.metrics.started.fetch_add(1, Ordering::Relaxed);
        let inner = Arc::clone(&self.inner);
        runtime.spawn(async move {
            let _lease = CacheRefreshLease {
                key: refresh_key,
                in_flight: Arc::clone(&inner.in_flight),
                _permit: permit,
            };
            match refresh().await {
                Ok(()) => {
                    inner.metrics.completed.fetch_add(1, Ordering::Relaxed);
                }
                Err(error) => {
                    inner.metrics.failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(%error, key, "Stale cache background refresh failed");
                }
            }
        });

        CacheRefreshSchedule::Spawned
    }

    pub fn in_flight(&self) -> usize {
        self.inner
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    pub fn stats(&self) -> CacheRefreshStats {
        CacheRefreshStats {
            started: self.inner.metrics.started.load(Ordering::Relaxed),
            completed: self.inner.metrics.completed.load(Ordering::Relaxed),
            failed: self.inner.metrics.failed.load(Ordering::Relaxed),
            deduplicated: self.inner.metrics.deduplicated.load(Ordering::Relaxed),
            saturated: self.inner.metrics.saturated.load(Ordering::Relaxed),
            rejected: self.inner.metrics.rejected.load(Ordering::Relaxed),
            runtime_unavailable: self
                .inner
                .metrics
                .runtime_unavailable
                .load(Ordering::Relaxed),
            in_flight: self.in_flight() as u64,
        }
    }

    fn remove_in_flight(&self, key: &CacheRefreshKey) {
        self.inner
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
    }
}

struct CacheRefreshLease {
    key: CacheRefreshKey,
    in_flight: Arc<StdMutex<HashSet<CacheRefreshKey>>>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for CacheRefreshLease {
    fn drop(&mut self) {
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.key);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheRefreshSchedule {
    NotNeeded,
    Spawned,
    AlreadyRunning,
    AtCapacity,
    InvalidKey,
    RuntimeUnavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheRefreshStats {
    pub started: u64,
    pub completed: u64,
    pub failed: u64,
    pub deduplicated: u64,
    pub saturated: u64,
    pub rejected: u64,
    pub runtime_unavailable: u64,
    pub in_flight: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheRefreshCoordinatorError {
    ZeroConcurrency,
}

impl std::fmt::Display for CacheRefreshCoordinatorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroConcurrency => {
                write!(formatter, "cache refresh concurrency must be greater than zero")
            }
        }
    }
}

impl std::error::Error for CacheRefreshCoordinatorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleWhileRevalidateResult<T> {
    pub cache: TypedCacheLoadResult<T>,
    pub refresh: CacheRefreshSchedule,
}

impl CacheService {
    /// Serve a typed envelope and asynchronously refresh stale cache hits.
    pub async fn load_enveloped_stale_while_revalidate<T, F, Fut>(
        &self,
        coordinator: &CacheRefreshCoordinator,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        expected_schema_version: u32,
        policy: CacheLoadPolicy,
        loader: F,
    ) -> rustok_core::Result<StaleWhileRevalidateResult<T>>
    where
        T: Serialize + DeserializeOwned + Send + 'static,
        F: Fn() -> Fut + Clone + Send + 'static,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>> + Send + 'static,
    {
        self.load_enveloped_stale_while_revalidate_with_limit_at(
            coordinator,
            backend,
            key,
            expected_schema_version,
            policy,
            DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
            current_unix_ms(),
            loader,
        )
        .await
    }

    /// Deterministic-clock SWR variant for tests and hosts with an injected clock.
    ///
    /// A stale hit carries the exact encoded bytes observed by the request. Before the
    /// background loader writes, it verifies that the key still contains those bytes. A
    /// concurrent replacement or invalidation therefore wins and the refresh becomes a no-op.
    /// This is an optimistic lost-update guard; a backend-level atomic compare-and-set remains
    /// necessary to eliminate the final read/write race across processes.
    pub async fn load_enveloped_stale_while_revalidate_with_limit_at<T, F, Fut>(
        &self,
        coordinator: &CacheRefreshCoordinator,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        expected_schema_version: u32,
        policy: CacheLoadPolicy,
        max_encoded_bytes: usize,
        now_unix_ms: u64,
        loader: F,
    ) -> rustok_core::Result<StaleWhileRevalidateResult<T>>
    where
        T: Serialize + DeserializeOwned + Send + 'static,
        F: Fn() -> Fut + Clone + Send + 'static,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>> + Send + 'static,
    {
        let key = key.into();
        validate_refresh_key(&key)?;

        // Probe the typed hit directly so the background refresh can retain the exact bytes
        // observed by this request. Passing only decoded metadata cannot distinguish two values
        // with identical timestamps or source revisions.
        if let Some(observed_bytes) = backend.get(&key).await? {
            match CacheEnvelope::<T>::decode_with_limit(
                &observed_bytes,
                expected_schema_version,
                max_encoded_bytes,
            ) {
                Ok(envelope) if !envelope.is_hard_expired(now_unix_ms) => {
                    let freshness = envelope.freshness(now_unix_ms);
                    let cache = typed_hit_result(envelope, freshness);
                    let refresh = if freshness == CacheEnvelopeFreshness::Stale {
                        let refresh_backend = Arc::clone(&backend);
                        let refresh_key = key.clone();
                        let refresh_policy = policy.clone();
                        let refresh_loader = loader.clone();
                        coordinator.schedule(&backend, key, move || async move {
                            refresh_envelope(
                                refresh_backend,
                                refresh_key,
                                observed_bytes,
                                expected_schema_version,
                                refresh_policy,
                                max_encoded_bytes,
                                refresh_loader,
                            )
                            .await
                        })
                    } else {
                        CacheRefreshSchedule::NotNeeded
                    };
                    return Ok(StaleWhileRevalidateResult { cache, refresh });
                }
                Ok(_) => {
                    tracing::debug!(key, "Invalidating hard-expired cache envelope");
                    backend.invalidate(&key).await?;
                }
                Err(error) => {
                    tracing::warn!(%error, key, "Invalidating incompatible cache envelope");
                    backend.invalidate(&key).await?;
                }
            }
        }

        let foreground_loader = loader.clone();
        let cache = self
            .load_enveloped_or_fill_with_limit_at(
                backend,
                key,
                expected_schema_version,
                policy,
                max_encoded_bytes,
                now_unix_ms,
                move || foreground_loader(),
            )
            .await?;

        // A foreground fill or a value that appeared after the exact-byte probe has already
        // resolved the request. Do not immediately run the source loader a second time.
        Ok(StaleWhileRevalidateResult {
            cache,
            refresh: CacheRefreshSchedule::NotNeeded,
        })
    }
}

fn typed_hit_result<T>(
    envelope: CacheEnvelope<T>,
    freshness: CacheEnvelopeFreshness,
) -> TypedCacheLoadResult<T> {
    let generated_at_unix_ms = envelope.generated_at_unix_ms();
    let source_revision = envelope.source_revision().map(ToOwned::to_owned);
    TypedCacheLoadResult {
        value: envelope.into_payload(),
        source: CacheLoadSource::Hit,
        freshness,
        generated_at_unix_ms,
        source_revision,
    }
}

async fn refresh_envelope<T, F, Fut>(
    backend: Arc<dyn CacheBackend>,
    key: String,
    observed_bytes: Vec<u8>,
    expected_schema_version: u32,
    policy: CacheLoadPolicy,
    max_encoded_bytes: usize,
    loader: F,
) -> rustok_core::Result<()>
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>> + Send + 'static,
{
    let ttl = match policy.ttl.ttl_for(&key) {
        Some(ttl) if ttl.is_zero() => {
            return Err(rustok_core::Error::Cache(
                "cache refresh TTL must be greater than zero".to_string(),
            ));
        }
        ttl => ttl,
    };

    let envelope = match policy.loader_timeout {
        Some(timeout) => tokio::time::timeout(timeout, loader())
            .await
            .map_err(|_| {
                rustok_core::Error::Cache(format!(
                    "cache refresh loader timed out after {} ms",
                    timeout.as_millis()
                ))
            })??,
        None => loader().await?,
    };

    if envelope.schema_version() != expected_schema_version {
        return Err(rustok_core::Error::Cache(format!(
            "cache refresh produced schema version {}; expected {}",
            envelope.schema_version(),
            expected_schema_version
        )));
    }
    if envelope.is_hard_expired(current_unix_ms()) {
        return Err(rustok_core::Error::Cache(
            "cache refresh produced an already hard-expired envelope".to_string(),
        ));
    }

    let bytes = envelope.encode_with_limit(max_encoded_bytes).map_err(|error| {
        rustok_core::Error::Cache(format!("cache refresh envelope error: {error}"))
    })?;

    if backend.get(&key).await?.as_deref() != Some(observed_bytes.as_slice()) {
        tracing::debug!(key, "Skipping stale cache refresh because the entry changed");
        return Ok(());
    }

    match ttl {
        Some(ttl) => backend.set_with_ttl(key, bytes, ttl).await,
        None => backend.set(key, bytes).await,
    }
}

fn validate_refresh_key(key: &str) -> rustok_core::Result<()> {
    if key.trim().is_empty() {
        return Err(rustok_core::Error::Cache(
            "cache refresh key must not be empty".to_string(),
        ));
    }
    if key.len() > MAX_CACHE_REFRESH_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "cache refresh key is {} bytes; maximum is {}",
            key.len(),
            MAX_CACHE_REFRESH_KEY_BYTES
        )));
    }
    Ok(())
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheTtlPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn coordinator_deduplicates_and_releases_refresh_keys() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(2).unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();

        assert_eq!(
            coordinator.schedule(&backend, "shared", move || async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
                Ok(())
            }),
            CacheRefreshSchedule::Spawned
        );
        started_rx.await.unwrap();
        assert_eq!(
            coordinator.schedule(&backend, "shared", || async { Ok(()) }),
            CacheRefreshSchedule::AlreadyRunning
        );

        let _ = release_tx.send(());
        for _ in 0..50 {
            if coordinator.in_flight() == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }

        assert_eq!(coordinator.in_flight(), 0);
        let stats = coordinator.stats();
        assert_eq!(stats.started, 1);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.deduplicated, 1);
    }

    #[tokio::test]
    async fn coordinator_rejects_new_key_when_global_capacity_is_full() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (_release_tx, release_rx) = oneshot::channel::<()>();

        assert_eq!(
            coordinator.schedule(&backend, "first", move || async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
                Ok(())
            }),
            CacheRefreshSchedule::Spawned
        );
        started_rx.await.unwrap();
        assert_eq!(
            coordinator.schedule(&backend, "second", || async { Ok(()) }),
            CacheRefreshSchedule::AtCapacity
        );
        assert_eq!(coordinator.stats().saturated, 1);
    }

    #[tokio::test]
    async fn coordinator_rejects_invalid_keys_without_running_refresh() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));

        for key in [
            "".to_string(),
            "x".repeat(MAX_CACHE_REFRESH_KEY_BYTES + 1),
        ] {
            let calls = Arc::clone(&calls);
            assert_eq!(
                coordinator.schedule(&backend, key, move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
                CacheRefreshSchedule::InvalidKey
            );
        }

        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(coordinator.in_flight(), 0);
        assert_eq!(coordinator.stats().rejected, 2);
    }

    #[tokio::test]
    async fn swr_rejects_invalid_key_before_backend_or_loader_work() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);

        let error = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend,
                "x".repeat(MAX_CACHE_REFRESH_KEY_BYTES + 1),
                1,
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                1024,
                2_000,
                move || {
                    let loader_calls = Arc::clone(&loader_calls);
                    async move {
                        loader_calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, 2_000, "unexpected".to_string())
                            .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                    }
                },
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("maximum"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(coordinator.in_flight(), 0);
    }

    #[tokio::test]
    async fn stale_value_is_served_while_one_background_refresh_replaces_it() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let stale = CacheEnvelope::new(1, 1_000, "stale".to_string())
            .unwrap()
            .with_expirations(Some(1_500), Some(10_000))
            .unwrap()
            .encode()
            .unwrap();
        backend.set("document".to_string(), stale).await.unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);
        let result = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend.clone(),
                "document",
                1,
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                1024,
                2_000,
                move || {
                    let loader_calls = Arc::clone(&loader_calls);
                    async move {
                        loader_calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, current_unix_ms(), "fresh".to_string())
                            .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(result.cache.value, "stale");
        assert_eq!(result.cache.source, CacheLoadSource::Hit);
        assert_eq!(result.refresh, CacheRefreshSchedule::Spawned);

        for _ in 0..100 {
            if let Some(bytes) = backend.get("document").await.unwrap() {
                if CacheEnvelope::<String>::decode_with_limit(&bytes, 1, 1024)
                    .is_ok_and(|envelope| envelope.payload() == "fresh")
                {
                    break;
                }
            }
            tokio::task::yield_now().await;
        }

        let bytes = backend.get("document").await.unwrap().unwrap();
        let refreshed = CacheEnvelope::<String>::decode_with_limit(&bytes, 1, 1024).unwrap();
        assert_eq!(refreshed.payload(), "fresh");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn concurrent_replacement_wins_over_slow_stale_refresh() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let stale = CacheEnvelope::new(1, 1_000, "stale".to_string())
            .unwrap()
            .with_expirations(Some(1_500), Some(10_000))
            .unwrap()
            .encode()
            .unwrap();
        backend.set("document".to_string(), stale).await.unwrap();

        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let loader_started = Arc::clone(&started);
        let loader_release = Arc::clone(&release);
        let result = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend.clone(),
                "document",
                1,
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                1024,
                2_000,
                move || {
                    let loader_started = Arc::clone(&loader_started);
                    let loader_release = Arc::clone(&loader_release);
                    async move {
                        loader_started.notify_one();
                        loader_release.notified().await;
                        CacheEnvelope::new(1, current_unix_ms(), "refresh".to_string())
                            .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(result.refresh, CacheRefreshSchedule::Spawned);
        tokio::time::timeout(Duration::from_secs(1), started.notified())
            .await
            .expect("refresh loader did not start");

        let replacement = CacheEnvelope::new(1, current_unix_ms(), "replacement".to_string())
            .unwrap()
            .with_source_revision("external-2")
            .unwrap()
            .encode()
            .unwrap();
        backend
            .set("document".to_string(), replacement)
            .await
            .unwrap();
        release.notify_one();

        tokio::time::timeout(Duration::from_secs(1), async {
            while coordinator.in_flight() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refresh task did not finish");

        let bytes = backend.get("document").await.unwrap().unwrap();
        let current = CacheEnvelope::<String>::decode_with_limit(&bytes, 1, 1024).unwrap();
        assert_eq!(current.payload(), "replacement");
        assert_eq!(current.source_revision(), Some("external-2"));
        assert_eq!(coordinator.stats().failed, 0);
    }

    #[tokio::test]
    async fn foreground_stale_fill_does_not_run_loader_twice() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);

        let result = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend,
                "foreground-stale",
                1,
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                1024,
                2_000,
                move || {
                    let loader_calls = Arc::clone(&loader_calls);
                    async move {
                        loader_calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, 1_000, "stale".to_string())
                            .unwrap()
                            .with_expirations(Some(1_500), Some(10_000))
                            .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(result.cache.source, CacheLoadSource::Filled);
        assert_eq!(result.cache.freshness, CacheEnvelopeFreshness::Stale);
        assert_eq!(result.refresh, CacheRefreshSchedule::NotNeeded);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(coordinator.stats().started, 0);
    }
}

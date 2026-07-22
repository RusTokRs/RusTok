use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use rustok_core::{CacheBackend, CacheCompareAndSetOutcome};

use crate::{
    CacheEnvelope, CacheEnvelopeFreshness, CacheLoadPolicy, CacheLoadSource, CacheService,
    DEFAULT_MAX_CACHE_ENVELOPE_BYTES, TypedCacheLoadOptions, TypedCacheLoadResult,
    clock::unix_time_millis,
};

pub const MAX_CACHE_REFRESH_KEY_BYTES: usize = crate::service::MAX_CACHE_LOAD_KEY_BYTES;

#[derive(Debug, Clone)]
pub struct CacheRefreshLoadOptions {
    pub expected_schema_version: u32,
    pub policy: CacheLoadPolicy,
    pub max_encoded_bytes: usize,
    pub now_unix_ms: u64,
}

struct RefreshEnvelopeRequest {
    backend: Arc<dyn CacheBackend>,
    key: String,
    observed_bytes: Vec<u8>,
    options: CacheRefreshLoadOptions,
}

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
/// cache entry and retains the backend allocation until its identity leaves the in-flight set.
/// Failed, cancelled and panicked refreshes leave the stale value untouched until hard expiry and
/// are reflected in the failure counter.
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
            self.inner.metrics.rejected.fetch_add(1, Ordering::Relaxed);
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
                self.inner.metrics.saturated.fetch_add(1, Ordering::Relaxed);
                return CacheRefreshSchedule::AtCapacity;
            }
        };

        self.inner.metrics.started.fetch_add(1, Ordering::Relaxed);
        let inner = Arc::clone(&self.inner);
        let lease = CacheRefreshLease {
            key: refresh_key,
            in_flight: Arc::clone(&inner.in_flight),
            _backend: Arc::clone(backend),
            _permit: permit,
        };
        let completion = CacheRefreshTaskCompletionGuard::new(Arc::clone(&inner));
        runtime.spawn(async move {
            let _lease = lease;
            let mut completion = completion;
            match refresh().await {
                Ok(()) => {
                    inner.metrics.completed.fetch_add(1, Ordering::Relaxed);
                }
                Err(error) => {
                    inner.metrics.failed.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(%error, key, "Stale cache background refresh failed");
                }
            }
            completion.complete();
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
    _backend: Arc<dyn CacheBackend>,
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

struct CacheRefreshTaskCompletionGuard {
    inner: Arc<CacheRefreshInner>,
    completed: bool,
}

impl CacheRefreshTaskCompletionGuard {
    fn new(inner: Arc<CacheRefreshInner>) -> Self {
        Self {
            inner,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for CacheRefreshTaskCompletionGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.inner.metrics.failed.fetch_add(1, Ordering::Relaxed);
        }
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
                write!(
                    formatter,
                    "cache refresh concurrency must be greater than zero"
                )
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
        let loader = move || {
            let loader = loader.clone();
            async move {
                let envelope = loader().await?;
                let completed_at_unix_ms = unix_time_millis()?;
                if envelope.is_hard_expired(completed_at_unix_ms) {
                    return Err(rustok_core::Error::Cache(
                        "cache loader produced an already hard-expired envelope at completion"
                            .to_string(),
                    ));
                }
                Ok(envelope)
            }
        };

        let now_unix_ms = unix_time_millis()?;
        self.load_enveloped_stale_while_revalidate_with_limit_at(
            coordinator,
            backend,
            key,
            CacheRefreshLoadOptions {
                expected_schema_version,
                policy,
                max_encoded_bytes: DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
                now_unix_ms,
            },
            loader,
        )
        .await
    }

    /// Deterministic-clock SWR variant for tests and hosts with an injected clock.
    ///
    /// A stale hit carries the exact encoded bytes observed by the request. The background
    /// loader uses the backend's atomic compare-and-set primitive, so a concurrent replacement
    /// or invalidation wins without any read/write time-of-check gap. The same injected clock is
    /// used to validate the asynchronously refreshed envelope.
    pub async fn load_enveloped_stale_while_revalidate_with_limit_at<T, F, Fut>(
        &self,
        coordinator: &CacheRefreshCoordinator,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        options: CacheRefreshLoadOptions,
        loader: F,
    ) -> rustok_core::Result<StaleWhileRevalidateResult<T>>
    where
        T: Serialize + DeserializeOwned + Send + 'static,
        F: Fn() -> Fut + Clone + Send + 'static,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>> + Send + 'static,
    {
        let key = key.into();
        let CacheRefreshLoadOptions {
            expected_schema_version,
            policy,
            max_encoded_bytes,
            now_unix_ms,
        } = options;
        validate_refresh_request(&key, expected_schema_version, max_encoded_bytes)?;

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
                                RefreshEnvelopeRequest {
                                    backend: refresh_backend,
                                    key: refresh_key,
                                    observed_bytes,
                                    options: CacheRefreshLoadOptions {
                                        expected_schema_version,
                                        policy: refresh_policy,
                                        max_encoded_bytes,
                                        now_unix_ms,
                                    },
                                },
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
                TypedCacheLoadOptions {
                    expected_schema_version,
                    policy,
                    max_encoded_bytes,
                    now_unix_ms,
                },
                foreground_loader,
            )
            .await?;

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
    request: RefreshEnvelopeRequest,
    loader: F,
) -> rustok_core::Result<()>
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>> + Send + 'static,
{
    let RefreshEnvelopeRequest {
        backend,
        key,
        observed_bytes,
        options:
            CacheRefreshLoadOptions {
                expected_schema_version,
                policy,
                max_encoded_bytes,
                now_unix_ms,
            },
    } = request;
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
    if envelope.is_hard_expired(now_unix_ms) {
        return Err(rustok_core::Error::Cache(
            "cache refresh produced an already hard-expired envelope".to_string(),
        ));
    }

    let bytes = envelope
        .encode_with_limit(max_encoded_bytes)
        .map_err(|error| {
            rustok_core::Error::Cache(format!("cache refresh envelope error: {error}"))
        })?;

    match backend
        .compare_and_set(&key, &observed_bytes, bytes, ttl)
        .await?
    {
        CacheCompareAndSetOutcome::Applied => Ok(()),
        CacheCompareAndSetOutcome::Mismatch => {
            tracing::debug!(
                key,
                "Skipping stale cache refresh because the entry changed"
            );
            Ok(())
        }
    }
}

fn validate_refresh_request(
    key: &str,
    expected_schema_version: u32,
    max_encoded_bytes: usize,
) -> rustok_core::Result<()> {
    validate_refresh_key(key)?;
    if expected_schema_version == 0 {
        return Err(rustok_core::Error::Cache(
            "cache refresh expected schema version must be non-zero".to_string(),
        ));
    }
    if max_encoded_bytes == 0 {
        return Err(rustok_core::Error::Cache(
            "cache refresh encoded size limit must be non-zero".to_string(),
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheTtlPolicy;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn dropping_unpolled_refresh_future_releases_lease_and_counts_failure() {
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let backend = CacheService::from_url(None).memory_backend(Duration::from_secs(60), 1);
        let key = CacheRefreshKey {
            backend_id: 1,
            key: "never-polled".to_string(),
        };
        let in_flight = Arc::new(StdMutex::new(HashSet::from([key.clone()])));
        let permit = Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap();
        let lease = CacheRefreshLease {
            key,
            in_flight: Arc::clone(&in_flight),
            _backend: backend,
            _permit: permit,
        };
        coordinator
            .inner
            .metrics
            .started
            .fetch_add(1, Ordering::Relaxed);
        let completion = CacheRefreshTaskCompletionGuard::new(Arc::clone(&coordinator.inner));
        let future = async move {
            let _lease = lease;
            let mut completion = completion;
            std::future::pending::<()>().await;
            completion.complete();
        };

        drop(future);
        assert!(
            in_flight
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
        let stats = coordinator.stats();
        assert_eq!(stats.started, 1);
        assert_eq!(stats.failed, 1);
    }

    #[tokio::test]
    async fn scheduled_refresh_keeps_backend_identity_alive_until_completion() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 1);
        let weak_backend = Arc::downgrade(&backend);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();

        assert_eq!(
            coordinator.schedule(&backend, "identity", move || async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
                Ok(())
            }),
            CacheRefreshSchedule::Spawned
        );
        started_rx.await.unwrap();
        drop(backend);
        assert!(weak_backend.upgrade().is_some());

        let _ = release_tx.send(());
        tokio::time::timeout(Duration::from_secs(1), async {
            while coordinator.in_flight() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refresh task did not finish");
        assert!(weak_backend.upgrade().is_none());
    }

    #[test]
    fn runtime_shutdown_counts_cancelled_refresh_as_failure() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 1);
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            assert_eq!(
                coordinator.schedule(&backend, "shutdown", move || async move {
                    let _ = started_tx.send(());
                    std::future::pending::<rustok_core::Result<()>>().await
                }),
                CacheRefreshSchedule::Spawned
            );
            started_rx.await.unwrap();
            assert_eq!(coordinator.stats().started, 1);
            assert_eq!(coordinator.stats().in_flight, 1);
        });

        runtime.shutdown_timeout(Duration::from_millis(10));
        assert_eq!(coordinator.stats().completed, 0);
        assert_eq!(coordinator.stats().failed, 1);
        assert_eq!(coordinator.stats().in_flight, 0);
    }

    #[tokio::test]
    async fn coordinator_deduplicates_and_releases_refresh_keys() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("refresh-test", Duration::from_secs(60), 16)
            .await;
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
        let backend = service
            .backend_shared_client("refresh-capacity", Duration::from_secs(60), 16)
            .await;
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
        let backend = service
            .backend_shared_client("refresh-invalid", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));

        for key in ["".to_string(), "x".repeat(MAX_CACHE_REFRESH_KEY_BYTES + 1)] {
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
        let backend = service
            .backend_shared_client("refresh-invalid-swr", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);

        let error = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend,
                "x".repeat(MAX_CACHE_REFRESH_KEY_BYTES + 1),
                CacheRefreshLoadOptions {
                    expected_schema_version: 1,
                    policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
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
    async fn invalid_swr_configuration_does_not_delete_a_valid_entry() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("refresh-invalid-config", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let original = CacheEnvelope::new(1, 1_000, "cached".to_string())
            .unwrap()
            .with_expirations(Some(1_500), Some(10_000))
            .unwrap()
            .encode()
            .unwrap();
        backend
            .set("document".to_string(), original.clone())
            .await
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));

        for (expected_schema_version, max_encoded_bytes) in [(0, 1024), (1, 0)] {
            let loader_calls = Arc::clone(&calls);
            let error = service
                .load_enveloped_stale_while_revalidate_with_limit_at(
                    &coordinator,
                    backend.clone(),
                    "document",
                    CacheRefreshLoadOptions {
                        expected_schema_version,
                        policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(
                            60,
                        ))),
                        max_encoded_bytes,
                        now_unix_ms: 2_000,
                    },
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
            assert!(matches!(error, rustok_core::Error::Cache(_)));
            assert_eq!(
                backend.get("document").await.unwrap(),
                Some(original.clone())
            );
            assert_eq!(coordinator.in_flight(), 0);
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn injected_clock_is_used_for_background_refresh_validation() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("refresh-injected-clock", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let stale = CacheEnvelope::new(1, 1_000, "stale".to_string())
            .unwrap()
            .with_expirations(Some(1_500), Some(10_000))
            .unwrap()
            .encode()
            .unwrap();
        backend.set("document".to_string(), stale).await.unwrap();

        let result = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend.clone(),
                "document",
                CacheRefreshLoadOptions {
                    expected_schema_version: 1,
                    policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
                || async {
                    CacheEnvelope::new(1, 2_000, "fresh".to_string())
                        .unwrap()
                        .with_expirations(Some(2_500), Some(5_000))
                        .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                },
            )
            .await
            .unwrap();

        assert_eq!(result.cache.value, "stale");
        assert_eq!(result.refresh, CacheRefreshSchedule::Spawned);
        tokio::time::timeout(Duration::from_secs(1), async {
            while coordinator.in_flight() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refresh task did not finish");

        let bytes = backend.get("document").await.unwrap().unwrap();
        let refreshed = CacheEnvelope::<String>::decode_with_limit(&bytes, 1, 1024).unwrap();
        assert_eq!(refreshed.payload(), "fresh");
        assert_eq!(coordinator.stats().completed, 1);
        assert_eq!(coordinator.stats().failed, 0);
    }

    #[tokio::test]
    async fn system_clock_rejects_refresh_that_expires_while_loader_runs() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("refresh-system-clock-expiry", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let now = unix_time_millis().unwrap();
        let stale = CacheEnvelope::new(1, now.saturating_sub(1_000), "stale".to_string())
            .unwrap()
            .with_expirations(
                Some(now.saturating_sub(500)),
                Some(now.saturating_add(10_000)),
            )
            .unwrap()
            .encode()
            .unwrap();
        backend
            .set("document".to_string(), stale.clone())
            .await
            .unwrap();
        let refresh_expires_at = unix_time_millis().unwrap().saturating_add(20);

        let result = service
            .load_enveloped_stale_while_revalidate(
                &coordinator,
                backend.clone(),
                "document",
                1,
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                move || async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    CacheEnvelope::new(
                        1,
                        refresh_expires_at.saturating_sub(1),
                        "expired-refresh".to_string(),
                    )
                    .unwrap()
                    .with_expirations(None, Some(refresh_expires_at))
                    .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                },
            )
            .await
            .unwrap();

        assert_eq!(result.cache.value, "stale");
        assert_eq!(result.refresh, CacheRefreshSchedule::Spawned);
        tokio::time::timeout(Duration::from_secs(1), async {
            while coordinator.in_flight() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("refresh task did not finish");

        assert_eq!(backend.get("document").await.unwrap(), Some(stale));
        assert_eq!(coordinator.stats().completed, 0);
        assert_eq!(coordinator.stats().failed, 1);
    }

    #[tokio::test]
    async fn stale_value_is_served_while_one_background_refresh_replaces_it() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("refresh-stale", Duration::from_secs(60), 16)
            .await;
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
                CacheRefreshLoadOptions {
                    expected_schema_version: 1,
                    policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
                move || {
                    let loader_calls = Arc::clone(&loader_calls);
                    async move {
                        loader_calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, unix_time_millis().unwrap(), "fresh".to_string())
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
        let backend = service
            .backend_shared_client("refresh-race", Duration::from_secs(60), 16)
            .await;
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
                CacheRefreshLoadOptions {
                    expected_schema_version: 1,
                    policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
                move || {
                    let loader_started = Arc::clone(&loader_started);
                    let loader_release = Arc::clone(&loader_release);
                    async move {
                        loader_started.notify_one();
                        loader_release.notified().await;
                        CacheEnvelope::new(1, unix_time_millis().unwrap(), "refresh".to_string())
                            .map_err(|error| rustok_core::Error::Cache(error.to_string()))
                    }
                },
            )
            .await
            .unwrap();

        assert_eq!(result.refresh, CacheRefreshSchedule::Spawned);
        tokio::time::timeout(std::time::Duration::from_secs(1), started.notified())
            .await
            .expect("refresh loader did not start");

        let replacement =
            CacheEnvelope::new(1, unix_time_millis().unwrap(), "replacement".to_string())
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

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
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
        let backend = service
            .backend_shared_client("refresh-foreground", Duration::from_secs(60), 16)
            .await;
        let coordinator = CacheRefreshCoordinator::new(1).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);

        let result = service
            .load_enveloped_stale_while_revalidate_with_limit_at(
                &coordinator,
                backend,
                "foreground-stale",
                CacheRefreshLoadOptions {
                    expected_schema_version: 1,
                    policy: CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60))),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
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

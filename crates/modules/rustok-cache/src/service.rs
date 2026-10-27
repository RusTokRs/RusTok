use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, broadcast};

#[cfg(feature = "redis-cache")]
use rustok_core::CircuitBreakerConfig;
use rustok_core::{CacheBackend, InMemoryCacheBackend};

use crate::instrumentation::InstrumentedCacheBackend;

pub const MAX_CACHE_INVALIDATION_CHANNEL_BYTES: usize = 256;
pub const MAX_CACHE_INVALIDATION_KEY_BYTES: usize = 4 * 1024;
pub const MAX_CACHE_LOAD_KEY_BYTES: usize = 512;
pub const DEFAULT_MAX_IN_FLIGHT_CACHE_LOADS: usize = 1_024;
/// Hard bound for process-local backend instances and their construction gates.
///
/// The registry prunes entries that no caller still references, so this cap only rejects a
/// deployment that genuinely holds this many live backend configurations at once.
pub const DEFAULT_MAX_SHARED_CACHE_BACKENDS: usize = 4_096;
/// Minimum meaningful cache TTL; a shorter positive TTL is rounded up by the backends anyway.
pub const MIN_CACHE_BACKEND_TTL: Duration = Duration::from_millis(1);

#[cfg(feature = "redis-cache")]
const CACHE_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(feature = "redis-cache")]
async fn redis_with_timeout<T, F, E>(operation: &str, future: F) -> Result<T, String>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(CACHE_REDIS_OPERATION_TIMEOUT, future)
        .await
        .map_err(|_| {
            format!(
                "{operation} timed out after {} ms",
                CACHE_REDIS_OPERATION_TIMEOUT.as_millis()
            )
        })?
        .map_err(|error| format!("{operation} failed: {error}"))
}

/// Shared cache service providing backend creation from a centralized Redis connection.
///
/// Other modules (tenant, RBAC, rate-limit) call `CacheService::backend()` instead of
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SharedBackendKey {
    pub prefix: String,
    pub ttl: Duration,
    pub max_capacity: u64,
    pub weighted: bool,
    pub metrics_enabled: bool,
}

struct ConstructionGateLease {
    key: SharedBackendKey,
    gate: Arc<AsyncMutex<()>>,
    registry: Arc<StdMutex<SharedBackendRegistry>>,
}

impl Drop for ConstructionGateLease {
    fn drop(&mut self) {
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry
            .construction_gates
            .get(&self.key)
            .is_some_and(|current| Arc::ptr_eq(current, &self.gate) && Arc::strong_count(current) <= 2)
        {
            registry.construction_gates.remove(&self.key);
        }
    }
}

/// Process-local backend instances keyed by their construction options, plus the per-key gates
/// that make construction single-flight.
#[derive(Default)]
struct SharedBackendRegistry {
    backends: HashMap<SharedBackendKey, Arc<dyn CacheBackend>>,
    construction_gates: HashMap<SharedBackendKey, Arc<AsyncMutex<()>>>,
    constructions: u64,
    rejected_insertions: u64,
}

impl SharedBackendRegistry {
    /// Drop entries that no caller references any more.
    ///
    /// The registry itself holds exactly one strong reference per entry, so `strong_count == 1`
    /// proves the backend is dead and can be released without changing observable behavior.
    fn prune_unreferenced(&mut self) {
        self.backends
            .retain(|_, backend| Arc::strong_count(backend) > 1);
        self.construction_gates.retain(|_, gate| Arc::strong_count(gate) > 1);
    }
}

/// Resolves Redis URLs once per service instead of having callers construct or
/// resolving Redis URLs themselves. This keeps Redis lifecycle in one place.
#[derive(Clone)]
pub struct CacheService {
    /// Requested Redis URL, present in every build so that a deployment which
    /// configured Redis can never look healthy while the build serves a
    /// memory-only cache.
    redis_url: Option<String>,
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
    default_backend_options: CacheBackendOptions,
    loaders: Arc<CacheLoadCoordinator>,
    invalidations: CacheInvalidationService,
    shared_backends: Arc<StdMutex<SharedBackendRegistry>>,
    invalid_backend_configuration: Arc<AtomicU64>,
}

/// Backend construction options used by `CacheService`.
///
/// Defaults preserve the historical contract: Redis primary with in-memory fallback when
/// Redis is configured, pure in-memory otherwise, and lightweight in-process statistics
/// enabled for every returned backend.
#[derive(Debug, Clone)]
pub struct CacheBackendOptions {
    pub metrics_enabled: bool,
    #[cfg(feature = "redis-cache")]
    pub redis_circuit_breaker: CircuitBreakerConfig,
}

impl Default for CacheBackendOptions {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            #[cfg(feature = "redis-cache")]
            redis_circuit_breaker: CircuitBreakerConfig::default(),
        }
    }
}

impl CacheService {
    /// Build from environment variables (`RUSTOK_REDIS_URL` / `REDIS_URL`).
    pub fn from_env() -> Self {
        Self::from_url(None)
    }

    /// Build from an explicit URL, falling back to env vars when `url` is `None`.
    ///
    /// Priority: `url` argument → `RUSTOK_REDIS_URL` → `REDIS_URL`.
    /// Pass `Some(url)` to override env vars (e.g. from `settings.rustok.cache.redis_url`).
    #[cfg(feature = "redis-cache")]
    pub fn from_url(url: Option<&str>) -> Self {
        Self::from_url_with_options(url, CacheBackendOptions::default())
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url(_url: Option<&str>) -> Self {
        Self::from_url_with_options(_url, CacheBackendOptions::default())
    }

    /// Build from an explicit URL and service-wide backend defaults.
    #[cfg(feature = "redis-cache")]
    pub fn from_url_with_options(url: Option<&str>, options: CacheBackendOptions) -> Self {
        let redis_url = url
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(resolve_redis_url);
        let redis_client = redis_url
            .as_ref()
            .and_then(|u| redis::Client::open(u.as_str()).ok());
        let invalidations = CacheInvalidationService::new(redis_client.clone());
        Self {
            redis_url,
            redis_client,
            default_backend_options: options,
            loaders: Arc::new(CacheLoadCoordinator::default()),
            invalidations,
            shared_backends: Arc::new(StdMutex::new(SharedBackendRegistry::default())),
            invalid_backend_configuration: Arc::new(AtomicU64::new(0)),
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url_with_options(url: Option<&str>, options: CacheBackendOptions) -> Self {
        // This build has no Redis client. The requested configuration is still
        // resolved and kept, so `redis_status()`/`health()` report a
        // configured-but-unavailable cache instead of a healthy memory-only
        // deployment.
        let redis_url = url
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .or_else(resolve_redis_url);
        Self {
            redis_url,
            default_backend_options: options,
            loaders: Arc::new(CacheLoadCoordinator::default()),
            invalidations: CacheInvalidationService::new(),
            shared_backends: Arc::new(StdMutex::new(SharedBackendRegistry::default())),
            invalid_backend_configuration: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) fn get_shared_backend(
        &self,
        key: &SharedBackendKey,
    ) -> Option<Arc<dyn CacheBackend>> {
        self.shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .backends
            .get(key)
            .cloned()
    }

    fn shared_registry(&self) -> &StdMutex<SharedBackendRegistry> {
        &self.shared_backends
    }

    /// Register a backend, returning the instance that callers must use.
    ///
    /// The registry is bounded: unreferenced entries are pruned first, and a deployment that
    /// genuinely holds the maximum number of live configurations gets an error log plus the newly
    /// built backend without registration (correct, just unshared) instead of an unbounded map or a
    /// silently duplicated instance.
    pub(crate) fn insert_shared_backend(
        &self,
        key: SharedBackendKey,
        backend: Arc<dyn CacheBackend>,
    ) -> Arc<dyn CacheBackend> {
        let mut registry = self
            .shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(existing) = registry.backends.get(&key) {
            return Arc::clone(existing);
        }
        if registry.backends.len() >= DEFAULT_MAX_SHARED_CACHE_BACKENDS {
            registry.prune_unreferenced();
        }
        if registry.backends.len() >= DEFAULT_MAX_SHARED_CACHE_BACKENDS {
            registry.rejected_insertions = registry.rejected_insertions.saturating_add(1);
            tracing::error!(
                maximum = DEFAULT_MAX_SHARED_CACHE_BACKENDS,
                prefix = %key.prefix,
                "Shared cache backend registry is full of live backends; serving an unregistered instance"
            );
            return backend;
        }

        registry
            .backends
            .insert(key, Arc::clone(&backend));
        backend
    }

    /// Build a backend at most once per construction key.
    ///
    /// Two concurrent callers for the same `(prefix, ttl, capacity, metrics)` options must not
    /// observe independent fallback trackers or open duplicate Redis connections, so construction is
    /// single-flight per key rather than check-then-act.
    pub(crate) async fn shared_backend_slot<F, Fut>(
        &self,
        key: SharedBackendKey,
        build: F,
    ) -> Arc<dyn CacheBackend>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Arc<dyn CacheBackend>>,
    {
        if let Some(existing) = self.get_shared_backend(&key) {
            return existing;
        }

        let Some(gate) = self.construction_gate(&key) else {
            // Saturated registry: build without registering rather than serializing forever.
            return build().await;
        };
        let _guard = gate.gate.lock().await;
        if let Some(existing) = self.get_shared_backend(&key) {
            return existing;
        }

        let backend = build().await;
        let registered = self.insert_shared_backend(key, backend);
        {
            let mut registry = self
                .shared_registry()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registry.constructions = registry.constructions.saturating_add(1);
        }
        registered
    }

    fn construction_gate(&self, key: &SharedBackendKey) -> Option<ConstructionGateLease> {
        let mut registry = self
            .shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let gate = if let Some(existing) = registry.construction_gates.get(key) {
            Arc::clone(existing)
        } else {
            if registry.construction_gates.len() >= DEFAULT_MAX_SHARED_CACHE_BACKENDS {
                registry.prune_unreferenced();
            }
            if registry.construction_gates.len() >= DEFAULT_MAX_SHARED_CACHE_BACKENDS {
                return None;
            }
            let gate = Arc::new(AsyncMutex::new(()));
            registry
                .construction_gates
                .insert(key.clone(), Arc::clone(&gate));
            gate
        };
        Some(ConstructionGateLease {
            key: key.clone(),
            gate,
            registry: Arc::clone(&self.shared_backends),
        })
    }

    /// Number of distinct backend configurations built through the shared registry.
    ///
    /// Exposed so hosts and tests can prove that concurrent factory calls are single-flight and
    /// that long-lived processes do not rebuild backends for unchanged configuration.
    pub fn shared_backend_construction_count(&self) -> u64 {
        self.shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .constructions
    }

    pub fn shared_backend_registry_len(&self) -> usize {
        self.shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .backends
            .len()
    }

    /// Clears cached backend instances so newly requested configurations or credentials rebuild backends.
    pub fn clear_shared_backends(&self) {
        let mut registry = self
            .shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry.backends.clear();
        registry.construction_gates.clear();
    }

    /// Reconcile degradation-aware local cache state with the shared primary.
    ///
    /// Hosts call this after an explicit confirmation that Redis answered (a successful status
    /// probe does it automatically). A backend that suspended reads because unsynchronized local
    /// mutations exceeded its bounded capacity resumes only when this returns `Ok`; failures are
    /// reported so readiness stays degraded instead of silently serving possibly stale bytes.
    pub async fn reconcile_local_state(&self) -> rustok_core::Result<usize> {
        let backends: Vec<Arc<dyn CacheBackend>> = self
            .shared_registry()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .backends
            .values()
            .cloned()
            .collect();

        let mut reconciled = 0usize;
        let mut failures = Vec::new();
        for backend in backends {
            match backend.reconcile_local_state().await {
                Ok(()) => reconciled = reconciled.saturating_add(1),
                Err(error) => failures.push(error.to_string()),
            }
        }

        if failures.is_empty() {
            return Ok(reconciled);
        }
        Err(rustok_core::Error::Cache(format!(
            "cache local state reconciliation failed for {} backend(s): {}",
            failures.len(),
            failures.join("; ")
        )))
    }

    /// Returns `true` if a Redis connection is available.
    pub fn has_redis(&self) -> bool {
        #[cfg(feature = "redis-cache")]
        {
            self.redis_client.is_some()
        }
        #[cfg(not(feature = "redis-cache"))]
        {
            false
        }
    }

    /// Returns the resolved Redis URL, if any.
    pub fn redis_url(&self) -> Option<&str> {
        self.redis_url.as_deref()
    }

    /// Returns the default backend options used by `backend()`.
    pub fn default_backend_options(&self) -> &CacheBackendOptions {
        &self.default_backend_options
    }

    /// Returns a reference to the underlying Redis client, if available.
    #[cfg(feature = "redis-cache")]
    pub fn redis_client(&self) -> Option<&redis::Client> {
        self.redis_client.as_ref()
    }

    /// Reject degenerate backend configuration loudly and return the effective values.
    ///
    /// A zero TTL means every write expires immediately and a zero capacity means every entry is
    /// evicted immediately; both silently turn the cache into a permanent miss and are reachable from
    /// operator configuration. They are counted, logged and floored to the smallest usable value so
    /// the defect is visible in metrics instead of degrading silently.
    pub fn validated_backend_configuration(
        &self,
        prefix: &str,
        ttl: Duration,
        capacity: u64,
    ) -> (Duration, u64) {
        if ttl.is_zero() || capacity == 0 {
            self.invalid_backend_configuration
                .fetch_add(1, Ordering::Relaxed);
            tracing::error!(
                prefix,
                ttl_ms = ttl.as_millis(),
                capacity,
                zero_ttl = ttl.is_zero(),
                zero_capacity = capacity == 0,
                "Invalid cache backend configuration; applying the documented minimum TTL and capacity"
            );
        }
        let effective_ttl = if ttl.is_zero() {
            MIN_CACHE_BACKEND_TTL
        } else {
            ttl
        };
        (effective_ttl, capacity.max(1))
    }

    /// Number of backend factory calls that received degenerate configuration.
    pub fn invalid_backend_configuration_total(&self) -> u64 {
        self.invalid_backend_configuration.load(Ordering::Relaxed)
    }

    /// Create a cache backend with the given prefix, TTL, and capacity.
    ///
    /// Redis backends reuse the client owned by this service. When Redis is unavailable,
    /// request paths retain the bounded in-memory fallback while health remains degraded.
    pub async fn backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
    ) -> Arc<dyn CacheBackend> {
        self.backend_shared_client(prefix, ttl, max_capacity).await
    }

    /// Create a cache backend with per-call construction options.
    pub async fn backend_with_options(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        self.backend_shared_client_with_options(prefix, ttl, max_capacity, options)
            .await
    }

    /// Create a pure in-memory backend (no Redis).
    pub fn memory_backend(&self, ttl: Duration, max_capacity: u64) -> Arc<dyn CacheBackend> {
        let (ttl, max_capacity) =
            self.validated_backend_configuration("memory", ttl, max_capacity);
        let backend = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
        if self.default_backend_options.metrics_enabled {
            Arc::new(InstrumentedCacheBackend::new("memory", backend))
        } else {
            backend
        }
    }

    /// Load a cache entry with per-key request coalescing.
    ///
    /// The first caller for a missing key runs `loader`; concurrent callers for the same key
    /// and backend wait for that fill and then read the populated backend. Different backend
    /// instances are deliberately isolated even when their user-facing keys are identical.
    /// Empty/oversized keys and excess unique in-flight keys fail before running the loader.
    pub async fn load_or_fill<F, Fut>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        ttl: Option<Duration>,
        loader: F,
    ) -> rustok_core::Result<CacheLoadResult>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<Vec<u8>>>,
    {
        let key = key.into();
        validate_cache_load_key(&key)?;
        // Backends interpret a zero TTL as deletion, which would make every caller a permanent
        // leader and report `Filled` for a value that was never stored.
        if ttl.is_some_and(|ttl| ttl.is_zero()) {
            return Err(rustok_core::Error::Cache(
                "cache load TTL must be greater than zero; use unbounded caching instead"
                    .to_string(),
            ));
        }
        tracing::debug!(
            key = %key,
            ttl_ms = ttl.map(|t| t.as_millis()),
            "load_or_fill cache entry"
        );
        self.loaders.load_or_fill(backend, key, ttl, loader).await
    }

    /// Returns the generic cache invalidation coordination service.
    ///
    /// Hosts and modules should use this capability for cross-instance cache invalidation
    /// instead of opening Redis pub/sub clients directly at each call site.
    pub fn invalidations(&self) -> CacheInvalidationService {
        self.invalidations.clone()
    }

    /// Stable clone-shared identity for process-local generation store ownership.
    ///
    /// The opaque token keeps the coordinator allocation alive while it is registered, preventing
    /// address reuse from attaching a new `CacheService` to stale generation snapshots.
    pub(crate) fn generation_store_identity(&self) -> Arc<dyn std::any::Any + Send + Sync> {
        self.loaders.clone()
    }

    /// Publish a cache invalidation message on a namespaced channel.
    ///
    /// With Redis enabled this publishes to Redis pub/sub; in all builds it also notifies
    /// local subscribers so tests and single-instance runtimes use the same contract.
    pub async fn publish_invalidation(
        &self,
        message: CacheInvalidationMessage,
    ) -> Result<CacheInvalidationOutcome, CacheInvalidationError> {
        tracing::debug!(
            channel = %message.channel,
            key = %message.key,
            "Publishing cache invalidation"
        );
        self.invalidations.publish(message).await
    }

    /// Returns currently tracked in-flight loader keys.
    ///
    /// This is primarily an operability/debugging signal; entries are removed once a fill
    /// completes, errors, or is cancelled and all waiters release their gate leases.
    pub async fn in_flight_loads(&self) -> usize {
        self.loaders.in_flight()
    }

    /// Render capability-level Prometheus metrics for the cache runtime.
    ///
    /// Backend-specific hit/miss counters remain exposed by each host-owned backend via
    /// `CacheBackend::stats()`. These service metrics cover central lifecycle signals that
    /// are not tied to a single backend instance: Redis health/configuration, default
    /// instrumentation state, and in-flight anti-stampede loaders.
    pub async fn prometheus_metrics(&self) -> String {
        format_cache_service_prometheus_metrics(
            &self.health().await,
            self.in_flight_loads().await,
            &self.invalidations.stats(),
            self.invalid_backend_configuration_total(),
        )
    }

    /// Health check backed by the canonical Redis lifecycle status.
    pub async fn health(&self) -> CacheHealthReport {
        let status = self.redis_status().await;
        CacheHealthReport {
            redis_configured: status.url_present,
            redis_healthy: status.connectivity_healthy,
            redis_error: status.last_error,
            metrics_enabled: self.default_backend_options.metrics_enabled,
            #[cfg(feature = "redis-cache")]
            redis_circuit_breaker_failure_threshold: self
                .default_backend_options
                .redis_circuit_breaker
                .failure_threshold,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheInvalidationMessage {
    pub channel: String,
    pub key: String,
}

impl CacheInvalidationMessage {
    pub fn new(channel: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            key: key.into(),
        }
    }

    /// Build a validated invalidation message.
    pub fn try_new(
        channel: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<Self, CacheInvalidationMessageError> {
        let message = Self::new(channel, key);
        message.validate()?;
        Ok(message)
    }

    pub fn validate(&self) -> Result<(), CacheInvalidationMessageError> {
        if self.channel.trim().is_empty() {
            return Err(CacheInvalidationMessageError::EmptyChannel);
        }
        if self.channel.len() > MAX_CACHE_INVALIDATION_CHANNEL_BYTES {
            return Err(CacheInvalidationMessageError::ChannelTooLong {
                length: self.channel.len(),
                maximum: MAX_CACHE_INVALIDATION_CHANNEL_BYTES,
            });
        }
        if self.key.trim().is_empty() {
            return Err(CacheInvalidationMessageError::EmptyKey);
        }
        if self.key.len() > MAX_CACHE_INVALIDATION_KEY_BYTES {
            return Err(CacheInvalidationMessageError::KeyTooLong {
                length: self.key.len(),
                maximum: MAX_CACHE_INVALIDATION_KEY_BYTES,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheInvalidationMessageError {
    EmptyChannel,
    ChannelTooLong { length: usize, maximum: usize },
    EmptyKey,
    KeyTooLong { length: usize, maximum: usize },
}

impl std::fmt::Display for CacheInvalidationMessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChannel => f.write_str("cache invalidation channel must not be empty"),
            Self::ChannelTooLong { length, maximum } => write!(
                f,
                "cache invalidation channel is {length} bytes; maximum is {maximum}"
            ),
            Self::EmptyKey => f.write_str("cache invalidation key must not be empty"),
            Self::KeyTooLong { length, maximum } => write!(
                f,
                "cache invalidation key is {length} bytes; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for CacheInvalidationMessageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInvalidationOutcome {
    pub local_subscribers: usize,
    pub redis_published: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheInvalidationStats {
    /// Messages forwarded to the process-local broadcast bus, independent of subscriber count
    /// (the Prometheus series keeps its historical name for dashboard compatibility).
    pub local_published_total: u64,
    pub redis_publish_success_total: u64,
    pub redis_publish_failure_total: u64,
    pub rejected_total: u64,
}

#[derive(Clone)]
pub struct CacheInvalidationService {
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
    /// Reused publish connection. Opening a connection per `PUBLISH` turns a burst of
    /// invalidations into a connection storm and adds handshake latency to every fan-out.
    #[cfg(feature = "redis-cache")]
    publish_manager: Arc<AsyncMutex<Option<redis::aio::ConnectionManager>>>,
    local: broadcast::Sender<CacheInvalidationMessage>,
    stats: Arc<CacheInvalidationMetrics>,
}

#[derive(Default)]
struct CacheInvalidationMetrics {
    local_published_total: AtomicU64,
    redis_publish_success_total: AtomicU64,
    redis_publish_failure_total: AtomicU64,
    rejected_total: AtomicU64,
}

impl CacheInvalidationService {
    #[cfg(feature = "redis-cache")]
    fn new(redis_client: Option<redis::Client>) -> Self {
        let (local, _) = broadcast::channel(256);
        Self {
            redis_client,
            publish_manager: Arc::new(AsyncMutex::new(None)),
            local,
            stats: Arc::new(CacheInvalidationMetrics::default()),
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    fn new() -> Self {
        let (local, _) = broadcast::channel(256);
        Self {
            local,
            stats: Arc::new(CacheInvalidationMetrics::default()),
        }
    }

    pub fn subscribe_local(&self) -> broadcast::Receiver<CacheInvalidationMessage> {
        self.local.subscribe()
    }

    pub fn subscribe_local_channel(
        &self,
        channel: impl Into<String>,
    ) -> LocalCacheInvalidationSubscription {
        LocalCacheInvalidationSubscription {
            channel: channel.into(),
            receiver: self.local.subscribe(),
        }
    }

    pub fn stats(&self) -> CacheInvalidationStats {
        CacheInvalidationStats {
            local_published_total: self.stats.local_published_total.load(Ordering::Relaxed),
            redis_publish_success_total: self
                .stats
                .redis_publish_success_total
                .load(Ordering::Relaxed),
            redis_publish_failure_total: self
                .stats
                .redis_publish_failure_total
                .load(Ordering::Relaxed),
            rejected_total: self.stats.rejected_total.load(Ordering::Relaxed),
        }
    }

    #[cfg(feature = "redis-cache")]
    pub async fn consume_subscription<F, Fut>(
        &self,
        channel: &str,
        handler: F,
    ) -> Result<(), String>
    where
        F: FnMut(CacheInvalidationMessage) -> Fut,
        Fut: Future<Output = ()>,
    {
        self.consume_subscription_with_ready(channel, || async {}, handler)
            .await
    }

    #[cfg(feature = "redis-cache")]
    pub async fn consume_subscription_with_ready<F, Fut, R, ReadyFut>(
        &self,
        channel: &str,
        ready: R,
        mut handler: F,
    ) -> Result<(), String>
    where
        F: FnMut(CacheInvalidationMessage) -> Fut,
        Fut: Future<Output = ()>,
        R: FnOnce() -> ReadyFut,
        ReadyFut: Future<Output = ()>,
    {
        if let Err(error) = CacheInvalidationMessage::try_new(channel, "subscription-probe") {
            self.stats.rejected_total.fetch_add(1, Ordering::Relaxed);
            return Err(format!(
                "invalid cache invalidation subscription channel: {error:?}"
            ));
        }

        let Some(client) = &self.redis_client else {
            return Err("redis invalidation subscription is not configured".to_string());
        };

        let mut pubsub = redis_with_timeout(
            "Redis invalidation pub/sub connection",
            client.get_async_pubsub(),
        )
        .await?;
        redis_with_timeout("Redis invalidation subscribe", pubsub.subscribe(channel)).await?;

        ready().await;

        let mut messages = pubsub.on_message();
        use futures_util::StreamExt;

        while let Some(msg) = messages.next().await {
            let payload: Result<String, _> = msg.get_payload();
            let Ok(key) = payload else {
                tracing::warn!(
                    channel = channel,
                    "Ignoring cache invalidation message with non-string payload"
                );
                continue;
            };

            match CacheInvalidationMessage::try_new(channel, key) {
                Ok(message) => handler(message).await,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        channel = channel,
                        "Ignoring invalid cache invalidation message from Redis pub/sub"
                    );
                    self.stats.rejected_total.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Err("pubsub stream closed".to_string())
    }

    /// Publish one validated invalidation to local subscribers and, when configured, to Redis.
    ///
    /// Local delivery is always attempted first so single-instance runtimes and tests use the same
    /// contract. A configured-but-unreachable Redis is reported as an error: acknowledging a
    /// best-effort fan-out as success would let other instances keep stale entries without any
    /// signal to the caller.
    pub async fn publish(
        &self,
        message: CacheInvalidationMessage,
    ) -> Result<CacheInvalidationOutcome, CacheInvalidationError> {
        if let Err(error) = message.validate() {
            tracing::warn!(?error, "Rejected invalid cache invalidation message");
            self.stats.rejected_total.fetch_add(1, Ordering::Relaxed);
            return Err(CacheInvalidationError::InvalidMessage(error));
        }

        let local_subscribers = self.local.send(message.clone()).unwrap_or(0);
        self.stats
            .local_published_total
            .fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            return match self.publish_shared(client, &message).await {
                Ok(()) => {
                    self.stats
                        .redis_publish_success_total
                        .fetch_add(1, Ordering::Relaxed);
                    Ok(CacheInvalidationOutcome {
                        local_subscribers,
                        redis_published: true,
                    })
                }
                Err(detail) => {
                    self.stats
                        .redis_publish_failure_total
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        channel = %message.channel,
                        error = %detail,
                        "Redis invalidation publish failed"
                    );
                    Err(CacheInvalidationError::SharedTransport {
                        local_subscribers,
                        detail,
                    })
                }
            };
        }

        Ok(CacheInvalidationOutcome {
            local_subscribers,
            redis_published: false,
        })
    }

    #[cfg(feature = "redis-cache")]
    async fn publish_connection_manager(
        &self,
        client: &redis::Client,
    ) -> Result<redis::aio::ConnectionManager, String> {
        let mut state = self.publish_manager.lock().await;
        if let Some(manager) = state.as_ref() {
            return Ok(manager.clone());
        }
        let manager = redis_with_timeout(
            "Redis invalidation publish connection",
            client.get_connection_manager(),
        )
        .await?;
        *state = Some(manager.clone());
        Ok(manager)
    }

    #[cfg(feature = "redis-cache")]
    async fn publish_shared(
        &self,
        client: &redis::Client,
        message: &CacheInvalidationMessage,
    ) -> Result<(), String> {
        let mut manager = self.publish_connection_manager(client).await?;
        let published = redis_with_timeout(
            "Redis invalidation PUBLISH",
            redis::cmd("PUBLISH")
                .arg(&message.channel)
                .arg(&message.key)
                .query_async::<i64>(&mut manager),
        )
        .await;

        match published {
            // A zero receiver count is a successful publish: no peer happens to be subscribed.
            Ok(_receivers) => Ok(()),
            Err(detail) => {
                // Drop the broken connection so the next publish can reconnect instead of reusing it.
                *self.publish_manager.lock().await = None;
                Err(detail)
            }
        }
    }
}

/// Failure of the cache invalidation fan-out.
///
/// Both variants reach the caller instead of being reduced to a best-effort counter, because a lost
/// invalidation means another process may keep serving a stale entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheInvalidationError {
    /// The message violated the transport contract and was not published anywhere.
    InvalidMessage(CacheInvalidationMessageError),
    /// Redis is configured but the shared publish failed after local delivery was attempted.
    SharedTransport {
        local_subscribers: usize,
        detail: String,
    },
}

impl std::fmt::Display for CacheInvalidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMessage(error) => write!(formatter, "invalid cache invalidation: {error}"),
            Self::SharedTransport {
                local_subscribers,
                detail,
            } => write!(
                formatter,
                "cache invalidation reached {local_subscribers} local subscriber(s) but failed on the shared transport: {detail}"
            ),
        }
    }
}

impl std::error::Error for CacheInvalidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidMessage(error) => Some(error),
            Self::SharedTransport { .. } => None,
        }
    }
}

pub struct LocalCacheInvalidationSubscription {
    channel: String,
    receiver: broadcast::Receiver<CacheInvalidationMessage>,
}

impl LocalCacheInvalidationSubscription {
    pub fn channel(&self) -> &str {
        &self.channel
    }

    /// Receive the next message for this channel.
    ///
    /// `RecvError::Lagged(n)` means this subscription fell behind the bounded local bus and `n`
    /// messages were dropped; the caller must not treat it as a transport hiccup. The documented
    /// contract is that a lagged consumer clears (or rebuilds) its owned namespace before resuming
    /// fast-path processing, because an unknown number of invalidations for it were lost.
    pub async fn recv(&mut self) -> Result<CacheInvalidationMessage, broadcast::error::RecvError> {
        loop {
            let message = self.receiver.recv().await?;
            if message.channel == self.channel {
                return Ok(message);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLoadSource {
    Hit,
    Filled,
    Coalesced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLoadResult {
    pub value: Vec<u8>,
    pub source: CacheLoadSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheLoadKey {
    backend_id: usize,
    key: String,
}

impl CacheLoadKey {
    fn new(backend: &Arc<dyn CacheBackend>, key: &str) -> Self {
        Self {
            backend_id: Arc::as_ptr(backend) as *const () as usize,
            key: key.to_string(),
        }
    }
}

type CacheLoadGateMap = HashMap<CacheLoadKey, Arc<AsyncMutex<()>>>;

struct CacheLoadCoordinator {
    locks: Arc<StdMutex<CacheLoadGateMap>>,
    max_in_flight: usize,
}

impl Default for CacheLoadCoordinator {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_IN_FLIGHT_CACHE_LOADS)
    }
}

impl CacheLoadCoordinator {
    fn new(max_in_flight: usize) -> Self {
        assert!(max_in_flight > 0, "cache load capacity must be positive");
        Self {
            locks: Arc::new(StdMutex::new(HashMap::new())),
            max_in_flight,
        }
    }

    async fn load_or_fill<F, Fut>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: String,
        ttl: Option<Duration>,
        loader: F,
    ) -> rustok_core::Result<CacheLoadResult>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<Vec<u8>>>,
    {
        if let Some(value) = backend.get(&key).await? {
            return Ok(CacheLoadResult {
                value,
                source: CacheLoadSource::Hit,
            });
        }

        let lease = self.gate_for(CacheLoadKey::new(&backend, &key))?;
        let _guard = lease.gate.lock().await;

        if let Some(value) = backend.get(&key).await? {
            return Ok(CacheLoadResult {
                value,
                source: CacheLoadSource::Coalesced,
            });
        }

        let value = loader().await?;
        match ttl {
            Some(ttl) => {
                backend
                    .set_with_ttl(key.clone(), value.clone(), ttl)
                    .await?
            }
            None => backend.set(key, value.clone()).await?,
        }

        Ok(CacheLoadResult {
            value,
            source: CacheLoadSource::Filled,
        })
    }

    fn gate_for(&self, key: CacheLoadKey) -> rustok_core::Result<CacheLoadGateLease> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let gate = if let Some(current) = locks.get(&key) {
            Arc::clone(current)
        } else {
            if locks.len() >= self.max_in_flight {
                return Err(rustok_core::Error::Cache(format!(
                    "cache load coordinator saturated at {} unique in-flight keys",
                    self.max_in_flight
                )));
            }
            let gate = Arc::new(AsyncMutex::new(()));
            locks.insert(key.clone(), Arc::clone(&gate));
            gate
        };
        Ok(CacheLoadGateLease {
            key,
            gate,
            locks: Arc::clone(&self.locks),
        })
    }

    fn in_flight(&self) -> usize {
        self.locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

struct CacheLoadGateLease {
    key: CacheLoadKey,
    gate: Arc<AsyncMutex<()>>,
    locks: Arc<StdMutex<CacheLoadGateMap>>,
}

impl Drop for CacheLoadGateLease {
    fn drop(&mut self) {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if locks.get(&self.key).is_some_and(|current| {
            Arc::ptr_eq(current, &self.gate) && Arc::strong_count(current) <= 2
        }) {
            locks.remove(&self.key);
        }
    }
}

fn validate_cache_load_key(key: &str) -> rustok_core::Result<()> {
    if key.trim().is_empty() {
        return Err(rustok_core::Error::Cache(
            "cache load key must not be empty".to_string(),
        ));
    }
    if key.len() > MAX_CACHE_LOAD_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "cache load key is {} bytes; maximum is {}",
            key.len(),
            MAX_CACHE_LOAD_KEY_BYTES
        )));
    }
    Ok(())
}

pub fn format_cache_service_prometheus_metrics(
    report: &CacheHealthReport,
    in_flight_loads: usize,
    invalidation_stats: &CacheInvalidationStats,
    invalid_backend_configurations: u64,
) -> String {
    format!(
        "rustok_cache_redis_configured {redis_configured}\n\
rustok_cache_redis_healthy {redis_healthy}\n\
rustok_cache_metrics_enabled {metrics_enabled}\n\
rustok_cache_in_flight_loads {in_flight_loads}\n\
rustok_cache_invalid_backend_configuration_total {invalid_backend_configurations}\n\
rustok_cache_invalidation_local_published_total {local_published_total}\n\
rustok_cache_invalidation_redis_publish_success_total {redis_publish_success_total}\n\
rustok_cache_invalidation_redis_publish_failure_total {redis_publish_failure_total}\n\
rustok_cache_invalidation_rejected_total {rejected_total}\n",
        redis_configured = if report.redis_configured { 1 } else { 0 },
        redis_healthy = if report.redis_healthy { 1 } else { 0 },
        metrics_enabled = if report.metrics_enabled { 1 } else { 0 },
        local_published_total = invalidation_stats.local_published_total,
        redis_publish_success_total = invalidation_stats.redis_publish_success_total,
        redis_publish_failure_total = invalidation_stats.redis_publish_failure_total,
        rejected_total = invalidation_stats.rejected_total,
    )
}

#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub redis_configured: bool,
    pub redis_healthy: bool,
    pub redis_error: Option<String>,
    pub metrics_enabled: bool,
    #[cfg(feature = "redis-cache")]
    pub redis_circuit_breaker_failure_threshold: u32,
}

impl CacheHealthReport {
    pub fn is_healthy(&self) -> bool {
        !self.redis_configured || self.redis_healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::{CacheCompareAndSetOutcome, CacheStats};
    use std::sync::atomic::AtomicUsize;
    use tokio::sync::{Barrier, oneshot};

    struct FailingInvalidationBackend;

    #[async_trait::async_trait]
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
    async fn instrumented_backend_tracks_hits_misses_and_invalidations() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);

        assert!(backend.get("missing").await.unwrap().is_none());
        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );
        backend.invalidate("present").await.unwrap();

        let stats = backend.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 0);
    }

    #[tokio::test]
    async fn instrumented_backend_counts_only_successful_invalidations() {
        let backend = InstrumentedCacheBackend::new(
            "failing-invalidation",
            Arc::new(FailingInvalidationBackend),
        );

        assert!(backend.invalidate("key").await.is_err());
        assert_eq!(backend.stats().evictions, 0);
    }

    #[tokio::test]
    async fn instrumented_memory_backend_delegates_atomic_cas() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
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
    async fn load_or_fill_coalesces_concurrent_misses() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let calls = Arc::new(AtomicUsize::new(0));

        let first = {
            let service = service.clone();
            let backend = Arc::clone(&backend);
            let calls = Arc::clone(&calls);
            tokio::spawn(async move {
                service
                    .load_or_fill(backend, "shared", None, move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(25)).await;
                        Ok(b"filled".to_vec())
                    })
                    .await
                    .unwrap()
            })
        };

        tokio::time::sleep(Duration::from_millis(1)).await;

        let second = {
            let service = service.clone();
            let backend = Arc::clone(&backend);
            let calls = Arc::clone(&calls);
            tokio::spawn(async move {
                service
                    .load_or_fill(backend, "shared", None, move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(b"duplicate".to_vec())
                    })
                    .await
                    .unwrap()
            })
        };

        let first = first.await.unwrap();
        let second = second.await.unwrap();

        assert_eq!(first.value, b"filled".to_vec());
        assert_eq!(second.value, b"filled".to_vec());
        assert_eq!(first.source, CacheLoadSource::Filled);
        assert_eq!(second.source, CacheLoadSource::Coalesced);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(service.in_flight_loads().await, 0);
    }

    #[tokio::test]
    async fn load_or_fill_rejects_empty_and_oversized_keys_before_loader() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let calls = Arc::new(AtomicUsize::new(0));

        for key in ["".to_string(), "x".repeat(MAX_CACHE_LOAD_KEY_BYTES + 1)] {
            let calls = Arc::clone(&calls);
            let error = service
                .load_or_fill(Arc::clone(&backend), key, None, move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(b"unexpected".to_vec())
                })
                .await
                .unwrap_err();
            assert!(matches!(error, rustok_core::Error::Cache(_)));
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unique_in_flight_loads_are_bounded_without_breaking_same_key_coalescing() {
        let coordinator = Arc::new(CacheLoadCoordinator::new(1));
        let backend = CacheService::from_url(None).memory_backend(Duration::from_secs(60), 16);
        let (started_tx, started_rx) = oneshot::channel();

        let first = {
            let coordinator = Arc::clone(&coordinator);
            let backend = Arc::clone(&backend);
            tokio::spawn(async move {
                coordinator
                    .load_or_fill(backend, "first".to_string(), None, move || async move {
                        let _ = started_tx.send(());
                        std::future::pending::<rustok_core::Result<Vec<u8>>>().await
                    })
                    .await
            })
        };

        started_rx.await.unwrap();
        let error = coordinator
            .load_or_fill(backend, "second".to_string(), None, || async {
                Ok(b"unexpected".to_vec())
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("saturated"));

        first.abort();
        let _ = first.await;
        tokio::task::yield_now().await;
        assert_eq!(coordinator.in_flight(), 0);
    }

    #[tokio::test]
    async fn identical_keys_on_different_backends_do_not_block_each_other() {
        let service = CacheService::from_url(None);
        let first_backend = service.memory_backend(Duration::from_secs(60), 16);
        let second_backend = service.memory_backend(Duration::from_secs(60), 16);
        let barrier = Arc::new(Barrier::new(2));

        let first = {
            let service = service.clone();
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                service
                    .load_or_fill(first_backend, "shared", None, move || async move {
                        barrier.wait().await;
                        Ok(b"first".to_vec())
                    })
                    .await
                    .unwrap()
            })
        };
        let second = {
            let service = service.clone();
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                service
                    .load_or_fill(second_backend, "shared", None, move || async move {
                        barrier.wait().await;
                        Ok(b"second".to_vec())
                    })
                    .await
                    .unwrap()
            })
        };

        let (first, second) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first, second)
        })
        .await
        .expect("different cache backends should load concurrently");

        assert_eq!(first.unwrap().value, b"first".to_vec());
        assert_eq!(second.unwrap().value, b"second".to_vec());
        assert_eq!(service.in_flight_loads().await, 0);
    }

    #[tokio::test]
    async fn cancelled_loader_releases_its_gate() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let (started_tx, started_rx) = oneshot::channel();

        let task = {
            let service = service.clone();
            tokio::spawn(async move {
                let _ = service
                    .load_or_fill(backend, "cancelled", None, move || async move {
                        let _ = started_tx.send(());
                        std::future::pending::<rustok_core::Result<Vec<u8>>>().await
                    })
                    .await;
            })
        };

        started_rx.await.unwrap();
        assert_eq!(service.in_flight_loads().await, 1);
        task.abort();
        let _ = task.await;
        tokio::task::yield_now().await;

        assert_eq!(service.in_flight_loads().await, 0);
    }

    #[tokio::test]
    async fn load_or_fill_reports_existing_hit_without_loader() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        backend
            .set("cached".to_string(), b"ready".to_vec())
            .await
            .unwrap();

        let result = service
            .load_or_fill(Arc::clone(&backend), "cached", None, || async {
                Ok(b"should-not-run".to_vec())
            })
            .await
            .unwrap();

        assert_eq!(result.value, b"ready".to_vec());
        assert_eq!(result.source, CacheLoadSource::Hit);
    }

    #[tokio::test]
    async fn invalidation_service_notifies_local_subscribers_without_redis() {
        let service = CacheService::from_url(None);
        let mut subscriber = service.invalidations().subscribe_local();

        let outcome = service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", "key-1"))
            .await
            .unwrap();
        let message = subscriber.recv().await.unwrap();

        assert_eq!(message.channel, "cache.test");
        assert_eq!(message.key, "key-1");
        assert_eq!(outcome.local_subscribers, 1);
        assert!(!outcome.redis_published);
    }

    #[tokio::test]
    async fn load_or_fill_rejects_a_zero_ttl_instead_of_reporting_an_unsaved_fill() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let calls = Arc::new(AtomicUsize::new(0));
        let loader_calls = Arc::clone(&calls);

        let error = service
            .load_or_fill(
                Arc::clone(&backend),
                "zero-ttl",
                Some(Duration::ZERO),
                move || async move {
                    loader_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(b"value".to_vec())
                },
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("TTL must be greater than zero"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(backend.get("zero-ttl").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn degenerate_backend_configuration_is_counted_and_floored() {
        let service = CacheService::from_url(None);

        let (ttl, capacity) = service.validated_backend_configuration("test", Duration::ZERO, 0);
        assert_eq!(ttl, MIN_CACHE_BACKEND_TTL);
        assert_eq!(capacity, 1);
        assert_eq!(service.invalid_backend_configuration_total(), 1);

        let (ttl, capacity) =
            service.validated_backend_configuration("test", Duration::from_secs(5), 8);
        assert_eq!(ttl, Duration::from_secs(5));
        assert_eq!(capacity, 8);
        assert_eq!(service.invalid_backend_configuration_total(), 1);

        assert!(
            service
                .prometheus_metrics()
                .await
                .contains("rustok_cache_invalid_backend_configuration_total 1")
        );
    }

    #[tokio::test]
    async fn concurrent_factory_calls_build_each_backend_configuration_once() {
        let service = CacheService::from_url(None);
        let barrier = Arc::new(Barrier::new(4));

        let mut tasks = Vec::new();
        for _ in 0..4 {
            let service = service.clone();
            let barrier = Arc::clone(&barrier);
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                service
                    .backend_shared_client("shared-single-flight", Duration::from_secs(30), 8)
                    .await
            }));
        }

        let mut backends = Vec::new();
        for task in tasks {
            backends.push(task.await.unwrap());
        }

        assert!(
            backends
                .iter()
                .all(|backend| Arc::ptr_eq(backend, &backends[0])),
            "concurrent factory calls must share one backend instance"
        );
        assert_eq!(service.shared_backend_construction_count(), 1);
        assert_eq!(service.shared_backend_registry_len(), 1);
    }

    #[tokio::test]
    async fn clearing_shared_backends_releases_registry_entries() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_shared_client("shared-clear", Duration::from_secs(30), 8)
            .await;
        assert_eq!(service.shared_backend_registry_len(), 1);

        service.clear_shared_backends();
        assert_eq!(service.shared_backend_registry_len(), 0);
        drop(backend);

        let rebuilt = service
            .backend_shared_client("shared-clear", Duration::from_secs(30), 8)
            .await;
        assert_eq!(rebuilt.stats().entries, 0);
        assert_eq!(service.shared_backend_construction_count(), 2);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn unreachable_configured_redis_surfaces_a_publish_error_instead_of_a_silent_false() {
        let service = CacheService::from_url(Some("redis://127.0.0.1:1/"));
        let mut subscriber = service.invalidations().subscribe_local();

        let error = service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", "key-1"))
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            CacheInvalidationError::SharedTransport {
                local_subscribers: 1,
                ..
            }
        ));
        // Local delivery still happened before the shared transport failed.
        assert_eq!(subscriber.recv().await.unwrap().key, "key-1");
        assert_eq!(service.invalidations().stats().redis_publish_failure_total, 1);
    }

    #[tokio::test]
    async fn invalidation_message_validation_rejects_empty_and_oversized_parts() {
        assert_eq!(
            CacheInvalidationMessage::try_new("", "key").unwrap_err(),
            CacheInvalidationMessageError::EmptyChannel
        );
        assert_eq!(
            CacheInvalidationMessage::try_new("cache.test", "   ").unwrap_err(),
            CacheInvalidationMessageError::EmptyKey
        );
        assert_eq!(
            CacheInvalidationMessage::try_new(
                "x".repeat(MAX_CACHE_INVALIDATION_CHANNEL_BYTES + 1),
                "key"
            )
            .unwrap_err(),
            CacheInvalidationMessageError::ChannelTooLong {
                length: MAX_CACHE_INVALIDATION_CHANNEL_BYTES + 1,
                maximum: MAX_CACHE_INVALIDATION_CHANNEL_BYTES,
            }
        );
        assert_eq!(
            CacheInvalidationMessage::try_new(
                "cache.test",
                "x".repeat(MAX_CACHE_INVALIDATION_KEY_BYTES + 1)
            )
            .unwrap_err(),
            CacheInvalidationMessageError::KeyTooLong {
                length: MAX_CACHE_INVALIDATION_KEY_BYTES + 1,
                maximum: MAX_CACHE_INVALIDATION_KEY_BYTES,
            }
        );

        let valid = CacheInvalidationMessage::try_new("cache.test", "key").unwrap();
        assert_eq!(valid.channel, "cache.test");
        assert_eq!(valid.key, "key");
    }

    #[tokio::test]
    async fn invalid_invalidation_message_is_not_published_locally() {
        let service = CacheService::from_url(None);
        let mut subscriber = service.invalidations().subscribe_local();

        let error = service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", ""))
            .await
            .unwrap_err();

        assert_eq!(
            error,
            CacheInvalidationError::InvalidMessage(CacheInvalidationMessageError::EmptyKey)
        );
        assert!(subscriber.try_recv().is_err());
    }

    #[tokio::test]
    async fn invalidation_stats_track_local_publish_and_rejections() {
        let service = CacheService::from_url(None);

        service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", "key-1"))
            .await
            .unwrap();
        service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", ""))
            .await
            .unwrap_err();

        let stats = service.invalidations().stats();
        assert_eq!(stats.local_published_total, 1);
        assert_eq!(stats.redis_publish_success_total, 0);
        assert_eq!(stats.redis_publish_failure_total, 0);
        assert_eq!(stats.rejected_total, 1);
    }

    #[tokio::test]
    async fn prometheus_metrics_include_invalidation_counters() {
        let service = CacheService::from_url(None);

        service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", "key-1"))
            .await
            .unwrap();
        service
            .publish_invalidation(CacheInvalidationMessage::new("", "key-2"))
            .await
            .unwrap_err();

        let metrics = service.prometheus_metrics().await;
        assert!(metrics.contains("rustok_cache_invalidation_local_published_total 1"));
        assert!(metrics.contains("rustok_cache_invalidation_rejected_total 1"));
        assert!(metrics.contains("rustok_cache_invalidation_redis_publish_success_total 0"));
        assert!(metrics.contains("rustok_cache_invalidation_redis_publish_failure_total 0"));
    }

    #[tokio::test]
    async fn local_channel_subscription_filters_other_namespaces() {
        let service = CacheService::from_url(None);
        let mut tenant_subscriber = service
            .invalidations()
            .subscribe_local_channel("tenant.cache.invalidate");
        let mut rbac_subscriber = service
            .invalidations()
            .subscribe_local_channel("rbac.cache.invalidate");

        service
            .publish_invalidation(CacheInvalidationMessage::new(
                "rbac.cache.invalidate",
                "role:admin",
            ))
            .await
            .unwrap();
        service
            .publish_invalidation(CacheInvalidationMessage::new(
                "tenant.cache.invalidate",
                "tenant-a",
            ))
            .await
            .unwrap();

        let tenant_message = tenant_subscriber.recv().await.unwrap();
        let rbac_message = rbac_subscriber.recv().await.unwrap();

        assert_eq!(tenant_subscriber.channel(), "tenant.cache.invalidate");
        assert_eq!(tenant_message.key, "tenant-a");
        assert_eq!(rbac_message.key, "role:admin");
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn service_redis_timeout_bounds_stalled_operations() {
        let error = redis_with_timeout(
            "test Redis operation",
            std::future::pending::<Result<(), std::io::Error>>(),
        )
        .await
        .unwrap_err();

        assert!(error.contains("timed out"));
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn redis_subscription_rejects_empty_channel_before_connecting() {
        let service = CacheService::from_url(None);

        let error = service
            .invalidations()
            .consume_subscription(" ", |_| async {})
            .await
            .unwrap_err();

        assert!(error.contains("invalid cache invalidation subscription channel"));
        assert_eq!(service.invalidations().stats().rejected_total, 1);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    #[ignore = "requires a live Redis instance; set RUSTOK_CACHE_REAL_REDIS_URL"]
    async fn real_redis_publish_and_subscription_share_validated_channel_contract() {
        let Ok(redis_url) = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL") else {
            eprintln!(
                "skipping real Redis cache invalidation test: RUSTOK_CACHE_REAL_REDIS_URL is not set"
            );
            return;
        };
        let service = CacheService::from_url(Some(&redis_url));
        let channel = format!(
            "cache.integration.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let key = "tenant:real-redis";
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (message_tx, mut message_rx) = tokio::sync::mpsc::unbounded_channel();
        let listener = service.invalidations();
        let listener_channel = channel.clone();

        let listener_task = tokio::spawn(async move {
            listener
                .consume_subscription_with_ready(
                    &listener_channel,
                    move || async move {
                        let _ = ready_tx.send(());
                    },
                    move |message| {
                        let message_tx = message_tx.clone();
                        async move {
                            let _ = message_tx.send(message);
                        }
                    },
                )
                .await
        });

        tokio::time::timeout(Duration::from_secs(5), ready_rx)
            .await
            .expect("Redis subscription did not become ready")
            .expect("Redis subscription ready signal dropped");

        let outcome = service
            .publish_invalidation(CacheInvalidationMessage::try_new(&channel, key).unwrap())
            .await
            .expect("Redis invalidation publish must succeed against a live instance");
        assert!(outcome.redis_published);

        let message = tokio::time::timeout(Duration::from_secs(5), message_rx.recv())
            .await
            .expect("Redis invalidation message was not received")
            .expect("Redis invalidation receiver closed");
        assert_eq!(message.channel, channel);
        assert_eq!(message.key, key);

        listener_task.abort();
    }

    #[tokio::test]
    async fn backend_options_can_disable_instrumentation() {
        let service = CacheService::from_url(None);
        let backend = service
            .backend_with_options(
                "test-uninstrumented",
                Duration::from_secs(60),
                16,
                CacheBackendOptions {
                    metrics_enabled: false,
                    #[cfg(feature = "redis-cache")]
                    redis_circuit_breaker: CircuitBreakerConfig::default(),
                },
            )
            .await;

        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );

        let stats = backend.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.entries, 1);
    }
}

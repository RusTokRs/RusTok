use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{broadcast, Mutex as AsyncMutex};

use rustok_core::{CacheBackend, CacheStats, FallbackCacheBackend, InMemoryCacheBackend};
#[cfg(feature = "redis-cache")]
use rustok_core::{CircuitBreakerConfig, RedisCacheBackend};

/// Shared cache service providing backend creation from a centralized Redis connection.
///
/// Other modules (tenant, RBAC, rate-limit) call `CacheService::backend()` instead of
/// resolving Redis URLs themselves. This keeps Redis lifecycle in one place.
#[derive(Clone)]
pub struct CacheService {
    #[cfg(feature = "redis-cache")]
    redis_url: Option<String>,
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
    default_backend_options: CacheBackendOptions,
    loaders: Arc<CacheLoadCoordinator>,
    invalidations: CacheInvalidationService,
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
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    pub fn from_url_with_options(_url: Option<&str>, options: CacheBackendOptions) -> Self {
        Self {
            default_backend_options: options,
            loaders: Arc::new(CacheLoadCoordinator::default()),
            invalidations: CacheInvalidationService::new(),
        }
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
        #[cfg(feature = "redis-cache")]
        {
            self.redis_url.as_deref()
        }
        #[cfg(not(feature = "redis-cache"))]
        {
            None
        }
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

    /// Create a cache backend with the given prefix, TTL, and capacity.
    ///
    /// If Redis is available, returns a `FallbackCacheBackend` (Redis primary + in-memory fallback).
    /// Otherwise returns a pure in-memory backend. All returned backends are instrumented by default
    /// so `CacheBackend::stats()` exposes hits, misses, invalidations, and current entries.
    pub async fn backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
    ) -> Arc<dyn CacheBackend> {
        self.backend_with_options(
            prefix,
            ttl,
            max_capacity,
            self.default_backend_options.clone(),
        )
        .await
    }

    /// Create a cache backend with per-call construction options.
    pub async fn backend_with_options(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        let backend = self.raw_backend(prefix, ttl, max_capacity, &options).await;
        if options.metrics_enabled {
            Arc::new(InstrumentedCacheBackend::new(prefix, backend))
        } else {
            backend
        }
    }

    async fn raw_backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_capacity: u64,
        options: &CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        #[cfg(feature = "redis-cache")]
        if let Some(url) = &self.redis_url {
            if let Ok(redis_backend) = RedisCacheBackend::with_circuit_breaker(
                url,
                prefix,
                ttl,
                options.redis_circuit_breaker.clone(),
            )
            .await
            {
                let memory = Arc::new(InMemoryCacheBackend::new(ttl, max_capacity));
                return Arc::new(FallbackCacheBackend::new(Arc::new(redis_backend), memory));
            }
        }

        Arc::new(InMemoryCacheBackend::new(ttl, max_capacity))
    }

    /// Create a pure in-memory backend (no Redis).
    pub fn memory_backend(&self, ttl: Duration, max_capacity: u64) -> Arc<dyn CacheBackend> {
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
        self.loaders
            .load_or_fill(backend, key.into(), ttl, loader)
            .await
    }

    /// Returns the generic cache invalidation coordination service.
    ///
    /// Hosts and modules should use this capability for cross-instance cache invalidation
    /// instead of opening Redis pub/sub clients directly at each call site.
    pub fn invalidations(&self) -> CacheInvalidationService {
        self.invalidations.clone()
    }

    /// Publish a cache invalidation message on a namespaced channel.
    ///
    /// With Redis enabled this publishes to Redis pub/sub; in all builds it also notifies
    /// local subscribers so tests and single-instance runtimes use the same contract.
    pub async fn publish_invalidation(
        &self,
        message: CacheInvalidationMessage,
    ) -> CacheInvalidationOutcome {
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
        )
    }

    /// Health check: verify Redis connectivity (if configured).
    pub async fn health(&self) -> CacheHealthReport {
        let mut report = CacheHealthReport {
            redis_configured: self.has_redis(),
            redis_healthy: false,
            redis_error: None,
            metrics_enabled: self.default_backend_options.metrics_enabled,
            #[cfg(feature = "redis-cache")]
            redis_circuit_breaker_failure_threshold: self
                .default_backend_options
                .redis_circuit_breaker
                .failure_threshold,
        };

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    let pong: redis::RedisResult<String> =
                        redis::cmd("PING").query_async(&mut conn).await;
                    match pong {
                        Ok(ref s) if s == "PONG" => {
                            report.redis_healthy = true;
                        }
                        Ok(s) => {
                            report.redis_error = Some(format!("unexpected PING response: {s}"));
                        }
                        Err(e) => {
                            report.redis_error = Some(e.to_string());
                        }
                    }
                }
                Err(e) => {
                    report.redis_error = Some(e.to_string());
                }
            }
        }

        report
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
    ///
    /// Invalidation channels are shared across Redis pub/sub and local fan-out,
    /// so empty/whitespace-only channels or keys are rejected before a message
    /// can be published. The non-validating `new` constructor remains available
    /// for existing callers and tests that already own validation.
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
        if self.key.trim().is_empty() {
            return Err(CacheInvalidationMessageError::EmptyKey);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheInvalidationMessageError {
    EmptyChannel,
    EmptyKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInvalidationOutcome {
    pub local_subscribers: usize,
    pub redis_published: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheInvalidationStats {
    pub local_published_total: u64,
    pub redis_publish_success_total: u64,
    pub redis_publish_failure_total: u64,
    pub rejected_total: u64,
}

#[derive(Clone)]
pub struct CacheInvalidationService {
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
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

    /// Subscribe to local invalidations for a single namespace.
    ///
    /// This mirrors Redis channel subscription semantics for in-process listeners:
    /// unrelated local invalidations are skipped, while lag/closed receiver errors
    /// are still surfaced to the caller.
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

    /// Consume Redis pub/sub messages for a cache invalidation channel until the
    /// underlying stream closes or returns an error.
    ///
    /// The payload contract matches `publish`: Redis messages carry the invalidated
    /// key as payload, while the channel name remains the invalidation namespace.
    /// Callers keep ownership of retry/backoff and domain-specific side effects,
    /// but no longer need to open Redis pub/sub connections directly.
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

    /// Same as `consume_subscription`, but calls `ready` after Redis successfully
    /// subscribes and before the message stream is consumed. Host listeners use
    /// this hook to update health state only after subscription is established.
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

        let mut pubsub = client
            .get_async_pubsub()
            .await
            .map_err(|error| format!("pubsub connection failed: {error}"))?;

        pubsub
            .subscribe(channel)
            .await
            .map_err(|error| format!("pubsub subscribe failed: {error}"))?;

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

    pub async fn publish(&self, message: CacheInvalidationMessage) -> CacheInvalidationOutcome {
        if let Err(error) = message.validate() {
            tracing::warn!(?error, "Ignoring invalid cache invalidation message");
            self.stats.rejected_total.fetch_add(1, Ordering::Relaxed);
            return CacheInvalidationOutcome {
                local_subscribers: 0,
                redis_published: false,
            };
        }

        let mut outcome = CacheInvalidationOutcome {
            local_subscribers: 0,
            redis_published: false,
        };

        outcome.local_subscribers = self.local.send(message.clone()).unwrap_or(0);
        self.stats
            .local_published_total
            .fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "redis-cache")]
        {
            if let Some(client) = &self.redis_client {
                if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                    let published: redis::RedisResult<i64> = redis::cmd("PUBLISH")
                        .arg(&message.channel)
                        .arg(&message.key)
                        .query_async(&mut conn)
                        .await;
                    outcome.redis_published = published.is_ok();
                    if outcome.redis_published {
                        self.stats
                            .redis_publish_success_total
                            .fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.stats
                            .redis_publish_failure_total
                            .fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    self.stats
                        .redis_publish_failure_total
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        outcome
    }
}

/// Local channel-scoped invalidation receiver.
pub struct LocalCacheInvalidationSubscription {
    channel: String,
    receiver: broadcast::Receiver<CacheInvalidationMessage>,
}

impl LocalCacheInvalidationSubscription {
    pub fn channel(&self) -> &str {
        &self.channel
    }

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
    /// Value was already present before this call.
    Hit,
    /// This call executed the loader and stored the result.
    Filled,
    /// Another concurrent caller filled the key while this call waited.
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

#[derive(Default)]
struct CacheLoadCoordinator {
    locks: Arc<StdMutex<CacheLoadGateMap>>,
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

impl CacheLoadCoordinator {
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

        let lease = self.gate_for(CacheLoadKey::new(&backend, &key));
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

    fn gate_for(&self, key: CacheLoadKey) -> CacheLoadGateLease {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let gate = Arc::clone(
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
        );
        CacheLoadGateLease {
            key,
            gate,
            locks: Arc::clone(&self.locks),
        }
    }

    fn in_flight(&self) -> usize {
        self.locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

pub fn format_cache_service_prometheus_metrics(
    report: &CacheHealthReport,
    in_flight_loads: usize,
    invalidation_stats: &CacheInvalidationStats,
) -> String {
    format!(
        "rustok_cache_redis_configured {redis_configured}\n\
rustok_cache_redis_healthy {redis_healthy}\n\
rustok_cache_metrics_enabled {metrics_enabled}\n\
rustok_cache_in_flight_loads {in_flight_loads}\n\
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

struct InstrumentedCacheBackend {
    name: String,
    inner: Arc<dyn CacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl InstrumentedCacheBackend {
    fn new(name: impl Into<String>, inner: Arc<dyn CacheBackend>) -> Self {
        Self {
            name: name.into(),
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl CacheBackend for InstrumentedCacheBackend {
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

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.inner.invalidate(key).await
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
                .evictions
                .load(Ordering::Relaxed)
                .saturating_add(inner.evictions),
            entries: inner.entries,
        }
    }
}

impl Drop for InstrumentedCacheBackend {
    fn drop(&mut self) {
        tracing::debug!(cache = %self.name, stats = ?self.stats(), "cache backend dropped");
    }
}

#[cfg(feature = "redis-cache")]
fn resolve_redis_url() -> Option<String> {
    std::env::var("RUSTOK_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .filter(|url| !url.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::sync::{oneshot, Barrier};

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
            .await;
        let message = subscriber.recv().await.unwrap();

        assert_eq!(message.channel, "cache.test");
        assert_eq!(message.key, "key-1");
        assert_eq!(outcome.local_subscribers, 1);
        assert!(!outcome.redis_published);
    }

    #[tokio::test]
    async fn invalidation_message_validation_rejects_empty_parts() {
        assert_eq!(
            CacheInvalidationMessage::try_new("", "key").unwrap_err(),
            CacheInvalidationMessageError::EmptyChannel
        );
        assert_eq!(
            CacheInvalidationMessage::try_new("cache.test", "   ").unwrap_err(),
            CacheInvalidationMessageError::EmptyKey
        );

        let valid = CacheInvalidationMessage::try_new("cache.test", "key").unwrap();
        assert_eq!(valid.channel, "cache.test");
        assert_eq!(valid.key, "key");
    }

    #[tokio::test]
    async fn invalid_invalidation_message_is_not_published_locally() {
        let service = CacheService::from_url(None);
        let mut subscriber = service.invalidations().subscribe_local();

        let outcome = service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", ""))
            .await;

        assert_eq!(outcome.local_subscribers, 0);
        assert!(!outcome.redis_published);
        assert!(subscriber.try_recv().is_err());
    }

    #[tokio::test]
    async fn invalidation_stats_track_local_publish_and_rejections() {
        let service = CacheService::from_url(None);

        service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", "key-1"))
            .await;
        service
            .publish_invalidation(CacheInvalidationMessage::new("cache.test", ""))
            .await;

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
            .await;
        service
            .publish_invalidation(CacheInvalidationMessage::new("", "key-2"))
            .await;

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
            .await;
        service
            .publish_invalidation(CacheInvalidationMessage::new(
                "tenant.cache.invalidate",
                "tenant-a",
            ))
            .await;

        let tenant_message = tenant_subscriber.recv().await.unwrap();
        let rbac_message = rbac_subscriber.recv().await.unwrap();

        assert_eq!(tenant_subscriber.channel(), "tenant.cache.invalidate");
        assert_eq!(tenant_message.key, "tenant-a");
        assert_eq!(rbac_message.key, "role:admin");
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
            eprintln!("skipping real Redis cache invalidation test: RUSTOK_CACHE_REAL_REDIS_URL is not set");
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
            .await;
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

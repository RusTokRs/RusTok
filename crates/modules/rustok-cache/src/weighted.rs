use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats, InMemoryCacheBackend};

#[cfg(feature = "redis-cache")]
use crate::fallback::DegradationAwareFallbackBackend;
#[cfg(feature = "redis-cache")]
use crate::shared_backend::SharedClientRedisCacheBackend;
use crate::{CacheBackendOptions, CacheService};

impl CacheService {
    /// Create a backend whose in-process capacity is measured in bytes rather than entries.
    ///
    /// Redis remains the shared primary when configured. The local fallback uses Moka's
    /// weighted capacity and accounts for cache key bytes, serialized payload bytes and
    /// per-entry metadata. This is the preferred factory for variable-size documents. Logical
    /// keys are transparently scoped by the same monotonic generation contract as entry-count
    /// backends, so namespace rotation works uniformly for large tenant/document caches.
    pub async fn backend_weighted(
        &self,
        prefix: &str,
        ttl: Duration,
        max_weight_bytes: u64,
    ) -> Arc<dyn CacheBackend> {
        self.backend_weighted_with_options(
            prefix,
            ttl,
            max_weight_bytes,
            self.default_backend_options().clone(),
        )
        .await
    }

    /// Create a byte-weighted backend with per-call backend options.
    pub async fn backend_weighted_with_options(
        &self,
        prefix: &str,
        ttl: Duration,
        max_weight_bytes: u64,
        options: CacheBackendOptions,
    ) -> Arc<dyn CacheBackend> {
        let backend = self
            .raw_weighted_backend(prefix, ttl, max_weight_bytes, &options)
            .await;
        let backend = self.wrap_generation_aware_backend(prefix, backend).await;
        let backend = self.wrap_generation_recovery_health(prefix, backend);
        if options.metrics_enabled {
            Arc::new(InstrumentedWeightedCacheBackend::new(prefix, backend))
        } else {
            backend
        }
    }

    /// Create a pure byte-weighted in-memory backend.
    pub fn memory_backend_weighted(
        &self,
        ttl: Duration,
        max_weight_bytes: u64,
    ) -> Arc<dyn CacheBackend> {
        let backend = Arc::new(InMemoryCacheBackend::new_weighted(ttl, max_weight_bytes));
        if self.default_backend_options().metrics_enabled {
            Arc::new(InstrumentedWeightedCacheBackend::new(
                "memory-weighted",
                backend,
            ))
        } else {
            backend
        }
    }

    async fn raw_weighted_backend(
        &self,
        prefix: &str,
        ttl: Duration,
        max_weight_bytes: u64,
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
                Ok(redis_backend) => {
                    let memory =
                        Arc::new(InMemoryCacheBackend::new_weighted(ttl, max_weight_bytes));
                    return Arc::new(DegradationAwareFallbackBackend::new(
                        Arc::new(redis_backend),
                        memory,
                    ));
                }
                Err(error) => {
                    tracing::warn!(%error, prefix, "Weighted Redis backend initialization failed; using memory backend");
                }
            }
        }

        Arc::new(InMemoryCacheBackend::new_weighted(ttl, max_weight_bytes))
    }
}

struct InstrumentedWeightedCacheBackend {
    name: String,
    inner: Arc<dyn CacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl InstrumentedWeightedCacheBackend {
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

#[async_trait]
impl CacheBackend for InstrumentedWeightedCacheBackend {
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
        self.evictions.fetch_add(1, Ordering::Relaxed);
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
                .evictions
                .load(Ordering::Relaxed)
                .saturating_add(inner.evictions),
            entries: inner.entries,
        }
    }
}

impl Drop for InstrumentedWeightedCacheBackend {
    fn drop(&mut self) {
        tracing::debug!(
            cache = %self.name,
            stats = ?self.stats(),
            "weighted cache backend dropped"
        );
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
    async fn weighted_memory_factory_preserves_instrumentation_contract() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend_weighted(Duration::from_secs(60), 4096);

        assert!(backend.get("missing").await.unwrap().is_none());
        backend
            .set("present".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(
            backend.get("present").await.unwrap(),
            Some(b"value".to_vec())
        );

        let stats = backend.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.entries, 1);
    }

    #[tokio::test]
    async fn weighted_instrumentation_counts_only_successful_invalidations() {
        let backend = InstrumentedWeightedCacheBackend::new(
            "weighted-failing-invalidation",
            Arc::new(FailingInvalidationBackend),
        );

        assert!(backend.invalidate("key").await.is_err());
        assert_eq!(backend.stats().evictions, 0);
    }

    #[tokio::test]
    async fn weighted_memory_factory_delegates_atomic_cas() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend_weighted(Duration::from_secs(60), 4096);
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
    }

    #[tokio::test]
    async fn weighted_factory_switches_namespace_on_generation_change() {
        let service = CacheService::from_url(None);
        let prefix = format!("weighted-generation:{}", Uuid::new_v4().simple());
        let backend = service
            .backend_weighted(&prefix, Duration::from_secs(60), 4096)
            .await;
        backend
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"old".to_vec()));

        crate::observe_cache_backend_generation(&prefix, 1).unwrap();
        assert_eq!(backend.get("key").await.unwrap(), None);
    }
}

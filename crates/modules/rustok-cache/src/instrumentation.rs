//! Process-local cache backend instrumentation.
//!
//! There is exactly one instrumentation wrapper. It used to exist as three near-identical copies
//! (`InstrumentedCacheBackend`, `SharedInstrumentedCacheBackend`,
//! `InstrumentedWeightedCacheBackend`) whose `stats()` disagreed with each other, so the same
//! logical cache reported different numbers depending on which factory produced it.
//!
//! Counting rules:
//!
//! - `hits`/`misses` are the calls **this wrapper observed**. Inner counters are deliberately not
//!   added: the wrapper sees every `get` that reaches the backend, so summing would double count
//!   each hit (the previous behaviour).
//! - `evictions` are the capacity evictions reported by the inner backend plus the successful
//!   invalidations this wrapper observed. These are distinct event sources, so they add up.
//! - `entries` is delegated: only the underlying store knows its occupancy.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{
    CacheBackend, CacheCompareAndSetOutcome, CacheStats,
};

/// Instrumentation wrapper shared by every cache backend factory.
///
/// It delegates every operation unchanged (including the atomic compare-and-set primitive and
/// `reconcile_local_state`) and only observes outcomes, so wrapping never alters cache semantics.
pub(crate) struct InstrumentedCacheBackend {
    name: String,
    inner: Arc<dyn CacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
    invalidations: AtomicU64,
}

impl InstrumentedCacheBackend {
    pub(crate) fn new(name: impl Into<String>, inner: Arc<dyn CacheBackend>) -> Self {
        Self {
            name: name.into(),
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
        }
    }
}

#[async_trait]
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

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        self.inner
            .compare_and_set(key, expected, value, ttl)
            .await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.inner.invalidate(key).await?;
        // Only successful invalidations count: a failed one leaves the entry in place.
        self.invalidations.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn reconcile_local_state(&self) -> rustok_core::Result<()> {
        self.inner.reconcile_local_state().await
    }

    fn stats(&self) -> CacheStats {
        let inner = self.inner.stats();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self
                .invalidations
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::InMemoryCacheBackend;

    /// Inner backend that reports its own hits, so double counting would be visible.
    struct SelfCountingBackend {
        hits: AtomicU64,
        misses: AtomicU64,
        evictions: AtomicU64,
    }

    #[async_trait]
    impl CacheBackend for SelfCountingBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            self.hits.fetch_add(1, Ordering::Relaxed);
            Ok(Some(b"value".to_vec()))
        }

        async fn set(&self, _key: String, _value: Vec<u8>) -> rustok_core::Result<()> {
            Ok(())
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
            Ok(CacheCompareAndSetOutcome::Applied)
        }

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            self.misses.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats {
                hits: self.hits.load(Ordering::Relaxed),
                misses: self.misses.load(Ordering::Relaxed),
                evictions: self.evictions.load(Ordering::Relaxed),
                entries: 7,
            }
        }
    }

    #[tokio::test]
    async fn observations_are_not_double_counted_with_inner_statistics() {
        let inner = Arc::new(SelfCountingBackend {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        });
        let backend = InstrumentedCacheBackend::new("self-counting", inner);

        backend.get("key").await.unwrap();
        backend.invalidate("key").await.unwrap();

        let stats = backend.stats();
        assert_eq!(stats.hits, 1, "inner hits must not be added again");
        assert_eq!(stats.misses, 0);
        // One explicit invalidation; the inner "miss" bump on invalidate is not an eviction.
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 7, "occupancy is delegated to the inner store");
    }

    #[tokio::test]
    async fn memory_backend_observations_include_capacity_evictions() {
        let inner = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 4));
        let backend = InstrumentedCacheBackend::new("memory", Arc::clone(&inner) as Arc<dyn CacheBackend>);

        backend
            .set("key".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"value".to_vec()));
        assert_eq!(backend.get("absent").await.unwrap(), None);
        backend.invalidate("key").await.unwrap();

        let stats = backend.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 0);
    }

    #[tokio::test]
    async fn failed_invalidation_is_not_counted_as_an_eviction() {
        struct FailingInvalidation;

        #[async_trait]
        impl CacheBackend for FailingInvalidation {
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
                unreachable!("not exercised")
            }

            async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
                Err(rustok_core::Error::Cache("invalidation failed".to_string()))
            }

            fn stats(&self) -> CacheStats {
                CacheStats::default()
            }
        }

        let backend =
            InstrumentedCacheBackend::new("failing-invalidation", Arc::new(FailingInvalidation));
        assert!(backend.invalidate("key").await.is_err());
        assert_eq!(backend.stats().evictions, 0);
    }
}

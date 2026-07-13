use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats, Result as CoreResult};

#[derive(Default)]
struct CacheCompareAndSetCounters {
    attempted: AtomicU64,
    applied: AtomicU64,
    mismatches: AtomicU64,
    failed: AtomicU64,
    in_flight: AtomicU64,
}

/// Bounded, label-free snapshot of compare-and-set activity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheCompareAndSetStats {
    pub attempted: u64,
    pub applied: u64,
    pub mismatches: u64,
    pub failed: u64,
    pub in_flight: u64,
}

/// Shared metrics handle returned alongside an observed backend.
#[derive(Clone, Default)]
pub struct CacheCompareAndSetMetrics {
    counters: Arc<CacheCompareAndSetCounters>,
}

impl CacheCompareAndSetMetrics {
    pub fn snapshot(&self) -> CacheCompareAndSetStats {
        CacheCompareAndSetStats {
            attempted: self.counters.attempted.load(Ordering::Relaxed),
            applied: self.counters.applied.load(Ordering::Relaxed),
            mismatches: self.counters.mismatches.load(Ordering::Relaxed),
            failed: self.counters.failed.load(Ordering::Relaxed),
            in_flight: self.counters.in_flight.load(Ordering::Relaxed),
        }
    }
}

/// Render CAS metrics without cache-key or namespace labels.
pub fn format_cache_compare_and_set_prometheus_metrics(stats: &CacheCompareAndSetStats) -> String {
    format!(
        "rustok_cache_cas_attempted_total {attempted}\n\
         rustok_cache_cas_applied_total {applied}\n\
         rustok_cache_cas_mismatch_total {mismatches}\n\
         rustok_cache_cas_failed_total {failed}\n\
         rustok_cache_cas_in_flight {in_flight}\n",
        attempted = stats.attempted,
        applied = stats.applied,
        mismatches = stats.mismatches,
        failed = stats.failed,
        in_flight = stats.in_flight,
    )
}

/// Wrap a backend without changing cache behavior while observing atomic CAS outcomes.
pub fn observe_cache_compare_and_set(
    backend: Arc<dyn CacheBackend>,
) -> (Arc<dyn CacheBackend>, CacheCompareAndSetMetrics) {
    let metrics = CacheCompareAndSetMetrics::default();
    let observed = ObservedCacheBackend {
        backend,
        metrics: metrics.clone(),
    };
    (Arc::new(observed), metrics)
}

struct ObservedCacheBackend {
    backend: Arc<dyn CacheBackend>,
    metrics: CacheCompareAndSetMetrics,
}

struct InFlightGuard<'a> {
    counter: &'a AtomicU64,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl CacheBackend for ObservedCacheBackend {
    async fn health(&self) -> CoreResult<()> {
        self.backend.health().await
    }

    async fn get(&self, key: &str) -> CoreResult<Option<Vec<u8>>> {
        self.backend.get(key).await
    }

    async fn set(&self, key: String, value: Vec<u8>) -> CoreResult<()> {
        self.backend.set(key, value).await
    }

    async fn set_with_ttl(&self, key: String, value: Vec<u8>, ttl: Duration) -> CoreResult<()> {
        self.backend.set_with_ttl(key, value, ttl).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> CoreResult<CacheCompareAndSetOutcome> {
        self.metrics
            .counters
            .attempted
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .counters
            .in_flight
            .fetch_add(1, Ordering::Relaxed);
        let _in_flight = InFlightGuard {
            counter: &self.metrics.counters.in_flight,
        };

        match self
            .backend
            .compare_and_set(key, expected, value, ttl)
            .await
        {
            Ok(CacheCompareAndSetOutcome::Applied) => {
                self.metrics
                    .counters
                    .applied
                    .fetch_add(1, Ordering::Relaxed);
                Ok(CacheCompareAndSetOutcome::Applied)
            }
            Ok(CacheCompareAndSetOutcome::Mismatch) => {
                self.metrics
                    .counters
                    .mismatches
                    .fetch_add(1, Ordering::Relaxed);
                Ok(CacheCompareAndSetOutcome::Mismatch)
            }
            Err(error) => {
                self.metrics.counters.failed.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    async fn invalidate(&self, key: &str) -> CoreResult<()> {
        self.backend.invalidate(key).await
    }

    fn stats(&self) -> CacheStats {
        self.backend.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::Error;

    struct UnsupportedBackend;

    #[async_trait]
    impl CacheBackend for UnsupportedBackend {
        async fn health(&self) -> CoreResult<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> CoreResult<Option<Vec<u8>>> {
            Ok(None)
        }

        async fn set(&self, _key: String, _value: Vec<u8>) -> CoreResult<()> {
            Ok(())
        }

        async fn set_with_ttl(
            &self,
            _key: String,
            _value: Vec<u8>,
            _ttl: Duration,
        ) -> CoreResult<()> {
            Ok(())
        }

        async fn invalidate(&self, _key: &str) -> CoreResult<()> {
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats::default()
        }
    }

    #[tokio::test]
    async fn observed_backend_counts_applied_mismatch_and_failure() {
        let service = crate::CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        backend
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();
        let (backend, metrics) = observe_cache_compare_and_set(backend);

        assert_eq!(
            backend
                .compare_and_set("key", b"old", b"new".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Applied
        );
        assert_eq!(
            backend
                .compare_and_set("key", b"old", b"ignored".to_vec(), None)
                .await
                .unwrap(),
            CacheCompareAndSetOutcome::Mismatch
        );

        let (unsupported, unsupported_metrics) =
            observe_cache_compare_and_set(Arc::new(UnsupportedBackend));
        let error = unsupported
            .compare_and_set("key", b"old", b"new".to_vec(), None)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Cache(_)));

        assert_eq!(
            metrics.snapshot(),
            CacheCompareAndSetStats {
                attempted: 2,
                applied: 1,
                mismatches: 1,
                failed: 0,
                in_flight: 0,
            }
        );
        assert_eq!(unsupported_metrics.snapshot().failed, 1);
    }

    #[test]
    fn prometheus_metrics_are_label_free() {
        let metrics = format_cache_compare_and_set_prometheus_metrics(&CacheCompareAndSetStats {
            attempted: 9,
            applied: 4,
            mismatches: 3,
            failed: 2,
            in_flight: 1,
        });
        assert!(metrics.contains("rustok_cache_cas_attempted_total 9"));
        assert!(metrics.contains("rustok_cache_cas_mismatch_total 3"));
        assert!(metrics.contains("rustok_cache_cas_failed_total 2"));
        assert!(!metrics.contains('{'));
    }
}

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{
    CacheBackend, CacheCompareAndSetOutcome, CacheStats, FallbackCacheBackend,
    InMemoryCacheBackend,
};

/// Availability-preserving fallback whose health remains strict about the shared primary.
///
/// Reads and ordinary writes keep the existing Redis-to-memory fallback behavior, but health
/// checks and atomic compare-and-set remain strict about the shared primary. A Redis outage must
/// never turn a distributed CAS into a process-local acknowledged write.
pub(crate) struct DegradationAwareFallbackBackend {
    primary: Arc<dyn CacheBackend>,
    fallback: Arc<InMemoryCacheBackend>,
    inner: FallbackCacheBackend,
}

impl DegradationAwareFallbackBackend {
    pub(crate) fn new(
        primary: Arc<dyn CacheBackend>,
        fallback: Arc<InMemoryCacheBackend>,
    ) -> Self {
        Self {
            inner: FallbackCacheBackend::new(Arc::clone(&primary), Arc::clone(&fallback)),
            primary,
            fallback,
        }
    }
}

#[async_trait]
impl CacheBackend for DegradationAwareFallbackBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.primary.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        let value = self.inner.get(key).await?;
        if value.is_none() {
            // A successful primary miss is authoritative unless this key is a tracked degraded
            // write (in which case `inner.get` would have returned the fallback value). Remove any
            // older local mirror so a later Redis outage cannot resurrect stale bytes.
            if let Err(error) = self.fallback.invalidate(key).await {
                tracing::warn!(%error, key, "Failed to discard stale local cache mirror");
            }
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
        self.inner.invalidate(key).await
    }

    fn stats(&self) -> CacheStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    #[tokio::test]
    async fn reports_primary_degradation_while_local_fallback_still_serves_writes() {
        let primary = Arc::new(HealthControlledBackend {
            healthy: AtomicBool::new(false),
        });
        let fallback = Arc::new(InMemoryCacheBackend::new(Duration::from_secs(30), 16));
        let backend = DegradationAwareFallbackBackend::new(primary, fallback);

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
        let backend = DegradationAwareFallbackBackend::new(primary, Arc::clone(&fallback));

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
        let backend = DegradationAwareFallbackBackend::new(
            primary_backend,
            Arc::clone(&fallback),
        );

        assert_eq!(backend.get("key").await.unwrap(), None);
        assert_eq!(fallback.get("key").await.unwrap(), None);

        primary.failing.store(true, Ordering::SeqCst);
        assert_eq!(backend.get("key").await.unwrap(), None);
    }
}

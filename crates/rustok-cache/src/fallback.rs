use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheStats, FallbackCacheBackend, InMemoryCacheBackend};

/// Availability-preserving fallback whose health remains strict about the shared primary.
///
/// Reads and writes keep the existing Redis-to-memory fallback behavior, but health checks do
/// not convert a Redis outage into `Ok(())` merely because the in-process layer is available.
/// This lets readiness and operator diagnostics report degraded cross-instance consistency while
/// request paths can continue using bounded local data.
pub(crate) struct DegradationAwareFallbackBackend {
    primary: Arc<dyn CacheBackend>,
    inner: FallbackCacheBackend,
}

impl DegradationAwareFallbackBackend {
    pub(crate) fn new(
        primary: Arc<dyn CacheBackend>,
        fallback: Arc<InMemoryCacheBackend>,
    ) -> Self {
        Self {
            inner: FallbackCacheBackend::new(Arc::clone(&primary), fallback),
            primary,
        }
    }
}

#[async_trait]
impl CacheBackend for DegradationAwareFallbackBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.primary.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        self.inner.get(key).await
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

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            Err(rustok_core::Error::Cache(
                "shared primary unavailable".to_string(),
            ))
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
}

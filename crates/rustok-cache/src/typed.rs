use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;

use rustok_core::CacheBackend;

use crate::{
    clock::unix_time_millis, CacheEnvelope, CacheEnvelopeError, CacheEnvelopeFreshness,
    CacheLoadPolicy, CacheLoadSource, CacheService, DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
};

pub const MAX_TYPED_CACHE_KEY_BYTES: usize = crate::service::MAX_CACHE_LOAD_KEY_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCacheLoadResult<T> {
    pub value: T,
    pub source: CacheLoadSource,
    pub freshness: CacheEnvelopeFreshness,
    pub generated_at_unix_ms: u64,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TypedCacheLoadOptions {
    pub expected_schema_version: u32,
    pub policy: CacheLoadPolicy,
    pub max_encoded_bytes: usize,
    pub now_unix_ms: u64,
}

impl CacheService {
    /// Load a typed, versioned envelope using the system clock for freshness decisions.
    pub async fn load_enveloped_or_fill<T, F, Fut>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        expected_schema_version: u32,
        policy: CacheLoadPolicy,
        loader: F,
    ) -> rustok_core::Result<TypedCacheLoadResult<T>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>>,
    {
        self.load_enveloped_or_fill_with_limit_at(
            backend,
            key,
            TypedCacheLoadOptions {
                expected_schema_version,
                policy,
                max_encoded_bytes: DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
                now_unix_ms: unix_time_millis()?,
            },
            loader,
        )
        .await
    }

    /// Load a typed cache value with an explicit size limit and clock value.
    ///
    /// The explicit time makes freshness behavior deterministic in tests. Before invoking
    /// the coalesced loader, malformed, schema-incompatible and hard-expired entries are
    /// synchronously invalidated. A stale-but-not-hard-expired envelope is returned with
    /// `CacheEnvelopeFreshness::Stale`; callers may schedule an asynchronous refresh.
    pub async fn load_enveloped_or_fill_with_limit_at<T, F, Fut>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        options: TypedCacheLoadOptions,
        loader: F,
    ) -> rustok_core::Result<TypedCacheLoadResult<T>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>>,
    {
        let key = key.into();
        let TypedCacheLoadOptions {
            expected_schema_version,
            policy,
            max_encoded_bytes,
            now_unix_ms,
        } = options;
        validate_typed_cache_request(&key, expected_schema_version, max_encoded_bytes)?;

        if let Some(bytes) = backend.get(&key).await? {
            match CacheEnvelope::<T>::decode_with_limit(
                &bytes,
                expected_schema_version,
                max_encoded_bytes,
            ) {
                Ok(envelope) if !envelope.is_hard_expired(now_unix_ms) => {
                    return Ok(typed_result(envelope, CacheLoadSource::Hit, now_unix_ms));
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

        let result = self
            .load_or_fill_with_policy(
                Arc::clone(&backend),
                key.clone(),
                policy,
                move || async move {
                    let envelope = loader().await?;
                    if envelope.schema_version() != expected_schema_version {
                        return Err(rustok_core::Error::Cache(format!(
                            "cache loader produced schema version {}; expected {}",
                            envelope.schema_version(),
                            expected_schema_version
                        )));
                    }
                    if envelope.is_hard_expired(now_unix_ms) {
                        return Err(rustok_core::Error::Cache(
                            "cache loader produced an already hard-expired envelope".to_string(),
                        ));
                    }
                    envelope
                        .encode_with_limit(max_encoded_bytes)
                        .map_err(envelope_error_to_core)
                },
            )
            .await?;

        let envelope = match CacheEnvelope::<T>::decode_with_limit(
            &result.value,
            expected_schema_version,
            max_encoded_bytes,
        ) {
            Ok(envelope) => envelope,
            Err(error) => {
                // Another writer may have populated the key after the initial typed probe and
                // before the generic coalescing probe. Never leave that incompatible value in
                // place, otherwise every caller repeats the same decode failure indefinitely.
                tracing::error!(%error, key, "Coalesced cache value failed typed validation");
                backend.invalidate(&key).await?;
                return Err(envelope_error_to_core(error));
            }
        };

        if envelope.is_hard_expired(now_unix_ms) {
            // A value returned by another concurrent writer can still cross hard expiry between
            // the generic load and typed validation. Propagate invalidation failure because a
            // stale shared value may otherwise remain visible to other instances.
            backend.invalidate(&key).await?;
            return Err(rustok_core::Error::Cache(
                "coalesced cache value is already hard-expired".to_string(),
            ));
        }

        Ok(typed_result(envelope, result.source, now_unix_ms))
    }
}

fn validate_typed_cache_request(
    key: &str,
    expected_schema_version: u32,
    max_encoded_bytes: usize,
) -> rustok_core::Result<()> {
    validate_typed_cache_key(key)?;
    if expected_schema_version == 0 {
        return Err(rustok_core::Error::Cache(
            "typed cache expected schema version must be non-zero".to_string(),
        ));
    }
    if max_encoded_bytes == 0 {
        return Err(rustok_core::Error::Cache(
            "typed cache encoded size limit must be non-zero".to_string(),
        ));
    }
    Ok(())
}

fn validate_typed_cache_key(key: &str) -> rustok_core::Result<()> {
    if key.trim().is_empty() {
        return Err(rustok_core::Error::Cache(
            "typed cache key must not be empty".to_string(),
        ));
    }
    if key.len() > MAX_TYPED_CACHE_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "typed cache key is {} bytes; maximum is {}",
            key.len(),
            MAX_TYPED_CACHE_KEY_BYTES
        )));
    }
    Ok(())
}

fn typed_result<T>(
    envelope: CacheEnvelope<T>,
    source: CacheLoadSource,
    now_unix_ms: u64,
) -> TypedCacheLoadResult<T> {
    let freshness = envelope.freshness(now_unix_ms);
    let generated_at_unix_ms = envelope.generated_at_unix_ms();
    let source_revision = envelope.source_revision().map(ToOwned::to_owned);
    TypedCacheLoadResult {
        value: envelope.into_payload(),
        source,
        freshness,
        generated_at_unix_ms,
        source_revision,
    }
}

fn envelope_error_to_core(error: CacheEnvelopeError) -> rustok_core::Error {
    rustok_core::Error::Cache(format!("cache envelope error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheTtlPolicy;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn policy() -> CacheLoadPolicy {
        CacheLoadPolicy::new(CacheTtlPolicy::fixed(std::time::Duration::from_secs(60)))
    }

    #[tokio::test]
    async fn typed_loader_fills_then_hits_without_running_loader_again() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);
        let calls = AtomicUsize::new(0);

        let first = service
            .load_enveloped_or_fill_with_limit_at(
                backend.clone(),
                "typed",
                TypedCacheLoadOptions {
                    expected_schema_version: 2,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 1_500,
                },
                || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    CacheEnvelope::new(2, 1_000, "value".to_string())
                        .map_err(envelope_error_to_core)
                },
            )
            .await
            .unwrap();
        let second = service
            .load_enveloped_or_fill_with_limit_at(
                backend,
                "typed",
                TypedCacheLoadOptions {
                    expected_schema_version: 2,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 1_500,
                },
                || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    CacheEnvelope::new(2, 1_000, "duplicate".to_string())
                        .map_err(envelope_error_to_core)
                },
            )
            .await
            .unwrap();

        assert_eq!(first.value, "value");
        assert_eq!(first.source, CacheLoadSource::Filled);
        assert_eq!(second.value, "value");
        assert_eq!(second.source, CacheLoadSource::Hit);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn incompatible_schema_is_invalidated_and_reloaded() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);
        let old = CacheEnvelope::new(1, 1_000, "old".to_string())
            .unwrap()
            .encode()
            .unwrap();
        backend.set("typed".to_string(), old).await.unwrap();

        let result = service
            .load_enveloped_or_fill_with_limit_at(
                backend,
                "typed",
                TypedCacheLoadOptions {
                    expected_schema_version: 2,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 1_500,
                },
                || async {
                    CacheEnvelope::new(2, 1_400, "new".to_string()).map_err(envelope_error_to_core)
                },
            )
            .await
            .unwrap();

        assert_eq!(result.value, "new");
        assert_eq!(result.source, CacheLoadSource::Filled);
    }

    #[tokio::test]
    async fn hard_expired_value_is_invalidated_while_stale_value_is_returned() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);
        let hard_expired = CacheEnvelope::new(1, 1_000, "expired".to_string())
            .unwrap()
            .with_expirations(Some(1_100), Some(1_200))
            .unwrap()
            .encode()
            .unwrap();
        backend
            .set("typed".to_string(), hard_expired)
            .await
            .unwrap();

        let refreshed = service
            .load_enveloped_or_fill_with_limit_at(
                backend.clone(),
                "typed",
                TypedCacheLoadOptions {
                    expected_schema_version: 1,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 1_500,
                },
                || async {
                    CacheEnvelope::new(1, 1_400, "fresh".to_string())
                        .map_err(envelope_error_to_core)
                },
            )
            .await
            .unwrap();
        assert_eq!(refreshed.value, "fresh");

        let stale = CacheEnvelope::new(1, 2_000, "stale".to_string())
            .unwrap()
            .with_expirations(Some(2_100), Some(3_000))
            .unwrap()
            .encode()
            .unwrap();
        backend.set("stale".to_string(), stale).await.unwrap();

        let stale_result = service
            .load_enveloped_or_fill_with_limit_at(
                backend,
                "stale",
                TypedCacheLoadOptions {
                    expected_schema_version: 1,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_500,
                },
                || async {
                    CacheEnvelope::new(1, 2_500, "should-not-run".to_string())
                        .map_err(envelope_error_to_core)
                },
            )
            .await
            .unwrap();
        assert_eq!(stale_result.value, "stale");
        assert_eq!(stale_result.freshness, CacheEnvelopeFreshness::Stale);
        assert_eq!(stale_result.source, CacheLoadSource::Hit);
    }

    #[tokio::test]
    async fn typed_cache_rejects_invalid_keys_before_backend_or_loader_work() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);
        let calls = AtomicUsize::new(0);

        for key in ["".to_string(), "x".repeat(MAX_TYPED_CACHE_KEY_BYTES + 1)] {
            let error = service
                .load_enveloped_or_fill_with_limit_at(
                    backend.clone(),
                    key,
                    TypedCacheLoadOptions {
                        expected_schema_version: 1,
                        policy: policy(),
                        max_encoded_bytes: 1024,
                        now_unix_ms: 1_500,
                    },
                    || async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, 1_400, "unexpected".to_string())
                            .map_err(envelope_error_to_core)
                    },
                )
                .await
                .unwrap_err();
            assert!(matches!(error, rustok_core::Error::Cache(_)));
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let stats = backend.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.entries, 0);
    }

    #[tokio::test]
    async fn invalid_typed_configuration_does_not_delete_a_valid_entry() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);
        let original = CacheEnvelope::new(1, 1_000, "cached".to_string())
            .unwrap()
            .encode()
            .unwrap();
        backend
            .set("typed-config".to_string(), original.clone())
            .await
            .unwrap();
        let calls = AtomicUsize::new(0);

        for (expected_schema_version, max_encoded_bytes) in [(0, 1024), (1, 0)] {
            let error = service
                .load_enveloped_or_fill_with_limit_at(
                    backend.clone(),
                    "typed-config",
                    TypedCacheLoadOptions {
                        expected_schema_version,
                        policy: policy(),
                        max_encoded_bytes,
                        now_unix_ms: 1_500,
                    },
                    || async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        CacheEnvelope::new(1, 1_400, "unexpected".to_string())
                            .map_err(envelope_error_to_core)
                    },
                )
                .await
                .unwrap_err();
            assert!(matches!(error, rustok_core::Error::Cache(_)));
            assert_eq!(
                backend.get("typed-config").await.unwrap(),
                Some(original.clone())
            );
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[derive(Default)]
    struct RacedBackend {
        gets: AtomicUsize,
        invalidated: AtomicBool,
        value: Mutex<Option<Vec<u8>>>,
    }

    #[async_trait]
    impl CacheBackend for RacedBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, _key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            if self.gets.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(None)
            } else {
                Ok(self.value.lock().unwrap().clone())
            }
        }

        async fn set(&self, _key: String, value: Vec<u8>) -> rustok_core::Result<()> {
            *self.value.lock().unwrap() = Some(value);
            Ok(())
        }

        async fn set_with_ttl(
            &self,
            key: String,
            value: Vec<u8>,
            _ttl: std::time::Duration,
        ) -> rustok_core::Result<()> {
            self.set(key, value).await
        }

        async fn invalidate(&self, _key: &str) -> rustok_core::Result<()> {
            self.invalidated.store(true, Ordering::SeqCst);
            *self.value.lock().unwrap() = None;
            Ok(())
        }

        fn stats(&self) -> rustok_core::CacheStats {
            rustok_core::CacheStats::default()
        }
    }

    #[tokio::test]
    async fn incompatible_value_racing_after_initial_probe_is_invalidated() {
        let service = CacheService::from_url(None);
        let incompatible = CacheEnvelope::new(1, 1_000, "old".to_string())
            .unwrap()
            .encode()
            .unwrap();
        let backend = Arc::new(RacedBackend {
            value: Mutex::new(Some(incompatible)),
            ..RacedBackend::default()
        });
        let trait_backend: Arc<dyn CacheBackend> = backend.clone();

        let result = service
            .load_enveloped_or_fill_with_limit_at(
                trait_backend,
                "raced",
                TypedCacheLoadOptions {
                    expected_schema_version: 2,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 1_500,
                },
                || async {
                    CacheEnvelope::new(2, 1_400, "new".to_string()).map_err(envelope_error_to_core)
                },
            )
            .await;

        assert!(result.is_err());
        assert!(backend.invalidated.load(Ordering::SeqCst));
        assert!(backend.value.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn hard_expired_loader_is_rejected_before_store() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(std::time::Duration::from_secs(60), 16);

        let result = service
            .load_enveloped_or_fill_with_limit_at(
                backend.clone(),
                "expired-loader",
                TypedCacheLoadOptions {
                    expected_schema_version: 1,
                    policy: policy(),
                    max_encoded_bytes: 1024,
                    now_unix_ms: 2_000,
                },
                || async {
                    CacheEnvelope::new(1, 1_000, "expired".to_string())
                        .unwrap()
                        .with_expirations(Some(1_100), Some(1_200))
                        .map_err(envelope_error_to_core)
                },
            )
            .await;

        assert!(result.is_err());
        assert!(backend.get("expired-loader").await.unwrap().is_none());
    }
}

use std::future::Future;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Serialize;

use rustok_core::CacheBackend;

use crate::{
    CacheEnvelope, CacheEnvelopeError, CacheEnvelopeFreshness, CacheLoadPolicy, CacheLoadSource,
    CacheService, DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCacheLoadResult<T> {
    pub value: T,
    pub source: CacheLoadSource,
    pub freshness: CacheEnvelopeFreshness,
    pub generated_at_unix_ms: u64,
    pub source_revision: Option<String>,
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
            expected_schema_version,
            policy,
            DEFAULT_MAX_CACHE_ENVELOPE_BYTES,
            current_unix_ms(),
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
        expected_schema_version: u32,
        policy: CacheLoadPolicy,
        max_encoded_bytes: usize,
        now_unix_ms: u64,
        loader: F,
    ) -> rustok_core::Result<TypedCacheLoadResult<T>>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<CacheEnvelope<T>>>,
    {
        let key = key.into();

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
            .load_or_fill_with_policy(Arc::clone(&backend), key.clone(), policy, move || async move {
                let envelope = loader().await?;
                if envelope.schema_version() != expected_schema_version {
                    return Err(rustok_core::Error::Cache(format!(
                        "cache loader produced schema version {}; expected {}",
                        envelope.schema_version(),
                        expected_schema_version
                    )));
                }
                envelope
                    .encode_with_limit(max_encoded_bytes)
                    .map_err(envelope_error_to_core)
            })
            .await?;

        let envelope = CacheEnvelope::<T>::decode_with_limit(
            &result.value,
            expected_schema_version,
            max_encoded_bytes,
        )
        .map_err(|error| {
            tracing::error!(%error, key, "Newly loaded cache envelope failed validation");
            envelope_error_to_core(error)
        })?;

        if envelope.is_hard_expired(now_unix_ms) {
            let _ = backend.invalidate(&key).await;
            return Err(rustok_core::Error::Cache(
                "cache loader produced an already hard-expired envelope".to_string(),
            ));
        }

        Ok(typed_result(envelope, result.source, now_unix_ms))
    }
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
                2,
                policy(),
                1024,
                1_500,
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
                2,
                policy(),
                1024,
                1_500,
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
                2,
                policy(),
                1024,
                1_500,
                || async {
                    CacheEnvelope::new(2, 1_400, "new".to_string())
                        .map_err(envelope_error_to_core)
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
                1,
                policy(),
                1024,
                1_500,
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
                1,
                policy(),
                1024,
                2_500,
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
}

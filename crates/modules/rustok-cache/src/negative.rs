use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use rustok_core::CacheBackend;

use crate::{
    CacheEnvelope, CacheEnvelopeError, CacheEnvelopeFreshness, CachePolicyError, CacheService,
    CacheTtlPolicy, clock::unix_time_millis,
};

pub const DEFAULT_MAX_NEGATIVE_CACHE_BYTES: usize = 64 * 1024;
pub const MAX_NEGATIVE_CACHE_KEY_BYTES: usize = crate::service::MAX_CACHE_LOAD_KEY_BYTES;

/// Explicit policy for negative cache entries.
///
/// Only callers that have classified a result as a stable domain negative should call
/// `store_negative`. Transport, timeout and dependency failures are deliberately not accepted
/// by this API, preventing transient outages from being converted into cached not-found results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeCachePolicy {
    schema_version: u32,
    ttl: CacheTtlPolicy,
    max_encoded_bytes: usize,
}

impl NegativeCachePolicy {
    pub fn fixed(schema_version: u32, ttl: Duration) -> Result<Self, NegativeCachePolicyError> {
        validate_schema_and_ttl(schema_version, ttl)?;
        Ok(Self {
            schema_version,
            ttl: CacheTtlPolicy::fixed(ttl),
            max_encoded_bytes: DEFAULT_MAX_NEGATIVE_CACHE_BYTES,
        })
    }

    pub fn deterministic_jittered(
        schema_version: u32,
        ttl: Duration,
        max_jitter_percent: u8,
        namespace: impl Into<String>,
    ) -> Result<Self, NegativeCachePolicyError> {
        validate_schema_and_ttl(schema_version, ttl)?;
        let ttl = CacheTtlPolicy::deterministic_jitter(ttl, max_jitter_percent, namespace)
            .map_err(NegativeCachePolicyError::CachePolicy)?;
        Ok(Self {
            schema_version,
            ttl,
            max_encoded_bytes: DEFAULT_MAX_NEGATIVE_CACHE_BYTES,
        })
    }

    pub fn with_max_encoded_bytes(
        mut self,
        max_encoded_bytes: usize,
    ) -> Result<Self, NegativeCachePolicyError> {
        if max_encoded_bytes == 0 {
            return Err(NegativeCachePolicyError::ZeroSizeLimit);
        }
        self.max_encoded_bytes = max_encoded_bytes;
        Ok(self)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn max_encoded_bytes(&self) -> usize {
        self.max_encoded_bytes
    }

    pub fn ttl_for(&self, key: &str) -> Result<Duration, NegativeCachePolicyError> {
        match self.ttl.ttl_for(key) {
            Some(ttl) if !ttl.is_zero() => Ok(ttl),
            _ => Err(NegativeCachePolicyError::ZeroTtl),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegativeCacheEntry<R> {
    reason: R,
}

impl<R> NegativeCacheEntry<R> {
    pub fn new(reason: R) -> Self {
        Self { reason }
    }

    pub fn reason(&self) -> &R {
        &self.reason
    }

    pub fn into_reason(self) -> R {
        self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeCacheHit<R> {
    pub reason: R,
    pub freshness: CacheEnvelopeFreshness,
    pub generated_at_unix_ms: u64,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegativeCachePolicyError {
    ZeroSchemaVersion,
    ZeroTtl,
    ZeroSizeLimit,
    CachePolicy(CachePolicyError),
}

impl std::fmt::Display for NegativeCachePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroSchemaVersion => {
                write!(formatter, "negative cache schema version must be non-zero")
            }
            Self::ZeroTtl => write!(formatter, "negative cache TTL must be greater than zero"),
            Self::ZeroSizeLimit => {
                write!(
                    formatter,
                    "negative cache encoded size limit must be non-zero"
                )
            }
            Self::CachePolicy(error) => write!(formatter, "negative cache policy error: {error}"),
        }
    }
}

impl std::error::Error for NegativeCachePolicyError {}

impl CacheService {
    /// Read a typed negative entry using the system clock.
    pub async fn get_negative<R>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: &str,
        policy: &NegativeCachePolicy,
    ) -> rustok_core::Result<Option<NegativeCacheHit<R>>>
    where
        R: DeserializeOwned,
    {
        let now_unix_ms = unix_time_millis()?;
        self.get_negative_at(backend, key, policy, now_unix_ms)
            .await
    }

    /// Read a typed negative entry using an explicit clock value.
    ///
    /// Corrupted, incompatible and hard-expired entries are invalidated and treated as misses.
    pub async fn get_negative_at<R>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: &str,
        policy: &NegativeCachePolicy,
        now_unix_ms: u64,
    ) -> rustok_core::Result<Option<NegativeCacheHit<R>>>
    where
        R: DeserializeOwned,
    {
        validate_negative_key(key)?;
        let Some(bytes) = backend.get(key).await? else {
            return Ok(None);
        };

        let envelope = match CacheEnvelope::<NegativeCacheEntry<R>>::decode_with_limit(
            &bytes,
            policy.schema_version,
            policy.max_encoded_bytes,
        ) {
            Ok(envelope) => envelope,
            Err(error) => {
                tracing::warn!(%error, key, "Invalidating incompatible negative cache entry");
                backend.invalidate(key).await?;
                return Ok(None);
            }
        };

        if envelope.is_hard_expired(now_unix_ms) {
            backend.invalidate(key).await?;
            return Ok(None);
        }

        let freshness = envelope.freshness(now_unix_ms);
        let generated_at_unix_ms = envelope.generated_at_unix_ms();
        let source_revision = envelope.source_revision().map(ToOwned::to_owned);
        Ok(Some(NegativeCacheHit {
            reason: envelope.into_payload().into_reason(),
            freshness,
            generated_at_unix_ms,
            source_revision,
        }))
    }

    /// Store an explicitly classified domain negative.
    pub async fn store_negative<R>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        reason: R,
        generated_at_unix_ms: u64,
        source_revision: Option<String>,
        policy: &NegativeCachePolicy,
    ) -> rustok_core::Result<()>
    where
        R: Serialize,
    {
        let key = key.into();
        validate_negative_key(&key)?;
        let ttl = policy
            .ttl_for(&key)
            .map_err(negative_policy_error_to_core)?;
        let hard_expires_at_unix_ms =
            generated_at_unix_ms.saturating_add(duration_millis_ceil(ttl));

        let mut envelope = CacheEnvelope::new(
            policy.schema_version,
            generated_at_unix_ms,
            NegativeCacheEntry::new(reason),
        )
        .map_err(envelope_error_to_core)?;
        if let Some(revision) = source_revision {
            envelope = envelope
                .with_source_revision(revision)
                .map_err(envelope_error_to_core)?;
        }
        let envelope = envelope
            .with_expirations(None, Some(hard_expires_at_unix_ms))
            .map_err(envelope_error_to_core)?;
        let bytes = envelope
            .encode_with_limit(policy.max_encoded_bytes)
            .map_err(envelope_error_to_core)?;

        backend.set_with_ttl(key, bytes, ttl).await
    }

    pub async fn invalidate_negative(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: &str,
    ) -> rustok_core::Result<()> {
        validate_negative_key(key)?;
        backend.invalidate(key).await
    }
}

fn validate_schema_and_ttl(
    schema_version: u32,
    ttl: Duration,
) -> Result<(), NegativeCachePolicyError> {
    if schema_version == 0 {
        return Err(NegativeCachePolicyError::ZeroSchemaVersion);
    }
    if ttl.is_zero() {
        return Err(NegativeCachePolicyError::ZeroTtl);
    }
    Ok(())
}

fn validate_negative_key(key: &str) -> rustok_core::Result<()> {
    if key.trim().is_empty() {
        return Err(rustok_core::Error::Cache(
            "negative cache key must not be empty".to_string(),
        ));
    }
    if key.len() > MAX_NEGATIVE_CACHE_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "negative cache key is {} bytes; maximum is {}",
            key.len(),
            MAX_NEGATIVE_CACHE_KEY_BYTES
        )));
    }
    Ok(())
}

fn duration_millis_ceil(duration: Duration) -> u64 {
    if duration.is_zero() {
        return 0;
    }
    let nanos = duration.as_nanos();
    nanos
        .saturating_add(999_999)
        .checked_div(1_000_000)
        .unwrap_or(u128::MAX)
        .min(u128::from(u64::MAX)) as u64
}

fn envelope_error_to_core(error: CacheEnvelopeError) -> rustok_core::Error {
    rustok_core::Error::Cache(format!("negative cache envelope error: {error}"))
}

fn negative_policy_error_to_core(error: NegativeCachePolicyError) -> rustok_core::Error {
    rustok_core::Error::Cache(format!("negative cache policy error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn explicit_negative_is_stored_read_and_expired_by_envelope_time() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let policy = NegativeCachePolicy::fixed(1, Duration::from_secs(5)).unwrap();

        service
            .store_negative(
                backend.clone(),
                "missing-user",
                "not-found".to_string(),
                1_000,
                Some("users:42".to_string()),
                &policy,
            )
            .await
            .unwrap();

        let hit = service
            .get_negative_at::<String>(backend.clone(), "missing-user", &policy, 2_000)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hit.reason, "not-found");
        assert_eq!(hit.source_revision.as_deref(), Some("users:42"));

        assert!(
            service
                .get_negative_at::<String>(backend.clone(), "missing-user", &policy, 6_000)
                .await
                .unwrap()
                .is_none()
        );
        assert!(backend.get("missing-user").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn incompatible_negative_entry_is_invalidated() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let old = CacheEnvelope::new(1, 1_000, NegativeCacheEntry::new("old".to_string()))
            .unwrap()
            .with_expirations(None, Some(10_000))
            .unwrap()
            .encode()
            .unwrap();
        backend.set("negative".to_string(), old).await.unwrap();

        let policy = NegativeCachePolicy::fixed(2, Duration::from_secs(5)).unwrap();
        assert!(
            service
                .get_negative_at::<String>(backend.clone(), "negative", &policy, 2_000)
                .await
                .unwrap()
                .is_none()
        );
        assert!(backend.get("negative").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn negative_cache_rejects_empty_and_oversized_keys_before_backend_work() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let policy = NegativeCachePolicy::fixed(1, Duration::from_secs(5)).unwrap();

        for key in ["".to_string(), "x".repeat(MAX_NEGATIVE_CACHE_KEY_BYTES + 1)] {
            assert!(
                service
                    .store_negative(
                        backend.clone(),
                        key.clone(),
                        "not-found".to_string(),
                        1_000,
                        None,
                        &policy,
                    )
                    .await
                    .is_err()
            );
            assert!(
                service
                    .get_negative_at::<String>(backend.clone(), &key, &policy, 2_000)
                    .await
                    .is_err()
            );
            assert!(
                service
                    .invalidate_negative(backend.clone(), &key)
                    .await
                    .is_err()
            );
        }

        let stats = backend.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.entries, 0);
    }

    #[test]
    fn policy_rejects_unbounded_or_invalid_configuration() {
        assert_eq!(
            NegativeCachePolicy::fixed(0, Duration::from_secs(1)).unwrap_err(),
            NegativeCachePolicyError::ZeroSchemaVersion
        );
        assert_eq!(
            NegativeCachePolicy::fixed(1, Duration::ZERO).unwrap_err(),
            NegativeCachePolicyError::ZeroTtl
        );
    }
}

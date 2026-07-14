use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rustok_core::CacheBackend;

use crate::{CacheLoadResult, CacheService};

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const MAX_JITTER_PERCENT: u8 = 50;
pub const MAX_CACHE_POLICY_KEY_BYTES: usize = crate::service::MAX_CACHE_LOAD_KEY_BYTES;

/// TTL selection policy for a cache fill.
///
/// Jitter is deterministic for a `(namespace, key)` pair. It only shortens the configured TTL,
/// spreading expiration without allowing cached data to outlive the caller's freshness bound.
/// Stable inputs keep retries reproducible and make unit tests deterministic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheTtlPolicy {
    None,
    Fixed(Duration),
    DeterministicJitter {
        ttl: Duration,
        max_jitter_percent: u8,
        namespace: String,
    },
}

impl CacheTtlPolicy {
    pub fn none() -> Self {
        Self::None
    }

    pub fn fixed(ttl: Duration) -> Self {
        Self::Fixed(ttl)
    }

    pub fn deterministic_jitter(
        ttl: Duration,
        max_jitter_percent: u8,
        namespace: impl Into<String>,
    ) -> Result<Self, CachePolicyError> {
        if ttl.is_zero() {
            return Err(CachePolicyError::ZeroTtl);
        }
        if max_jitter_percent > MAX_JITTER_PERCENT {
            return Err(CachePolicyError::JitterPercentTooLarge {
                value: max_jitter_percent,
                maximum: MAX_JITTER_PERCENT,
            });
        }

        let namespace = namespace.into();
        if namespace.trim().is_empty() {
            return Err(CachePolicyError::EmptyNamespace);
        }

        Ok(Self::DeterministicJitter {
            ttl,
            max_jitter_percent,
            namespace,
        })
    }

    pub fn ttl_for(&self, key: &str) -> Option<Duration> {
        match self {
            Self::None => None,
            Self::Fixed(ttl) => Some(*ttl),
            Self::DeterministicJitter {
                ttl,
                max_jitter_percent,
                namespace,
            } => Some(deterministic_jittered_ttl(
                *ttl,
                *max_jitter_percent,
                namespace,
                key,
            )),
        }
    }
}

/// Request-coalescing and loader execution policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLoadPolicy {
    pub ttl: CacheTtlPolicy,
    pub loader_timeout: Option<Duration>,
}

impl CacheLoadPolicy {
    pub fn new(ttl: CacheTtlPolicy) -> Self {
        Self {
            ttl,
            loader_timeout: None,
        }
    }

    pub fn with_loader_timeout(mut self, timeout: Duration) -> Result<Self, CachePolicyError> {
        if timeout.is_zero() {
            return Err(CachePolicyError::ZeroLoaderTimeout);
        }
        self.loader_timeout = Some(timeout);
        Ok(self)
    }
}

impl Default for CacheLoadPolicy {
    fn default() -> Self {
        Self::new(CacheTtlPolicy::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CachePolicyError {
    EmptyNamespace,
    JitterPercentTooLarge { value: u8, maximum: u8 },
    ZeroTtl,
    ZeroLoaderTimeout,
}

impl std::fmt::Display for CachePolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyNamespace => write!(formatter, "cache jitter namespace must not be empty"),
            Self::JitterPercentTooLarge { value, maximum } => write!(
                formatter,
                "cache jitter percent {value} exceeds maximum {maximum}"
            ),
            Self::ZeroTtl => write!(formatter, "cache load TTL must be greater than zero"),
            Self::ZeroLoaderTimeout => {
                write!(formatter, "cache loader timeout must be greater than zero")
            }
        }
    }
}

impl std::error::Error for CachePolicyError {}

impl CacheService {
    /// Load a cache entry with backend-scoped coalescing, deterministic TTL jitter and
    /// an optional deadline around the source-of-truth loader.
    ///
    /// Cache reads and waiting for the local coalescing gate are not included in the
    /// loader deadline. Only the leader's loader future is bounded. A zero TTL is rejected
    /// before cache I/O because cache backends consistently interpret it as deletion.
    pub async fn load_or_fill_with_policy<F, Fut>(
        &self,
        backend: Arc<dyn CacheBackend>,
        key: impl Into<String>,
        policy: CacheLoadPolicy,
        loader: F,
    ) -> rustok_core::Result<CacheLoadResult>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = rustok_core::Result<Vec<u8>>>,
    {
        let key = key.into();
        validate_policy_key(&key)?;
        let ttl = policy.ttl.ttl_for(&key);
        if ttl.is_some_and(|ttl| ttl.is_zero()) {
            return Err(rustok_core::Error::Cache(
                CachePolicyError::ZeroTtl.to_string(),
            ));
        }
        let loader_timeout = policy.loader_timeout;

        self.load_or_fill(backend, key, ttl, move || async move {
            match loader_timeout {
                Some(timeout) => tokio::time::timeout(timeout, loader()).await.map_err(|_| {
                    rustok_core::Error::Cache(format!(
                        "cache loader timed out after {} ms",
                        timeout.as_millis()
                    ))
                })?,
                None => loader().await,
            }
        })
        .await
    }
}

fn validate_policy_key(key: &str) -> rustok_core::Result<()> {
    if key.trim().is_empty() {
        return Err(rustok_core::Error::Cache(
            "cache load key must not be empty".to_string(),
        ));
    }
    if key.len() > MAX_CACHE_POLICY_KEY_BYTES {
        return Err(rustok_core::Error::Cache(format!(
            "cache load key is {} bytes; maximum is {}",
            key.len(),
            MAX_CACHE_POLICY_KEY_BYTES
        )));
    }
    Ok(())
}

fn deterministic_jittered_ttl(
    ttl: Duration,
    max_jitter_percent: u8,
    namespace: &str,
    key: &str,
) -> Duration {
    if ttl.is_zero() || max_jitter_percent == 0 {
        return ttl;
    }

    let base_nanos = ttl.as_nanos();
    let max_delta = base_nanos.saturating_mul(u128::from(max_jitter_percent)) / 100;
    if max_delta == 0 {
        return ttl;
    }

    let hash = stable_fnv1a64(namespace.as_bytes(), key.as_bytes());
    let offset = u128::from(hash) % max_delta.saturating_add(1);
    let adjusted = base_nanos.saturating_sub(offset).max(1);

    duration_from_nanos_saturating(adjusted)
}

fn stable_fnv1a64(namespace: &[u8], key: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET_BASIS;
    for byte in namespace
        .iter()
        .copied()
        .chain(std::iter::once(0xff))
        .chain(key.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn duration_from_nanos_saturating(nanos: u128) -> Duration {
    let seconds = nanos / NANOS_PER_SECOND;
    let subsec_nanos = (nanos % NANOS_PER_SECOND) as u32;
    if seconds > u128::from(u64::MAX) {
        Duration::new(u64::MAX, 999_999_999)
    } else {
        Duration::new(seconds as u64, subsec_nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn deterministic_jitter_is_stable_and_bounded() {
        let policy =
            CacheTtlPolicy::deterministic_jitter(Duration::from_secs(100), 10, "tenant-cache:v1")
                .unwrap();

        let first = policy.ttl_for("tenant-a").unwrap();
        let second = policy.ttl_for("tenant-a").unwrap();
        assert_eq!(first, second);
        assert!(first >= Duration::from_secs(90));
        assert!(first <= Duration::from_secs(100));
    }

    #[test]
    fn deterministic_jitter_never_extends_the_configured_ttl() {
        let configured_ttl = Duration::from_secs(300);
        let policy =
            CacheTtlPolicy::deterministic_jitter(configured_ttl, 50, "authorization:v1").unwrap();

        for index in 0..1_024 {
            let jittered = policy.ttl_for(&format!("subject-{index}")).unwrap();
            assert!(jittered <= configured_ttl);
            assert!(jittered >= Duration::from_secs(150));
        }
    }

    #[test]
    fn deterministic_jitter_spreads_keys() {
        let policy =
            CacheTtlPolicy::deterministic_jitter(Duration::from_secs(100), 10, "tenant-cache:v1")
                .unwrap();

        let values = (0..32)
            .map(|index| policy.ttl_for(&format!("tenant-{index}")).unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert!(values.len() > 1);
    }

    #[test]
    fn jitter_validation_rejects_unsafe_configuration() {
        assert_eq!(
            CacheTtlPolicy::deterministic_jitter(Duration::ZERO, 10, "cache").unwrap_err(),
            CachePolicyError::ZeroTtl
        );
        assert_eq!(
            CacheTtlPolicy::deterministic_jitter(Duration::from_secs(1), 51, "cache").unwrap_err(),
            CachePolicyError::JitterPercentTooLarge {
                value: 51,
                maximum: 50,
            }
        );
        assert_eq!(
            CacheTtlPolicy::deterministic_jitter(Duration::from_secs(1), 10, "  ").unwrap_err(),
            CachePolicyError::EmptyNamespace
        );
    }

    #[tokio::test]
    async fn policy_rejects_invalid_keys_before_loader_work() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let policy = CacheLoadPolicy::new(
            CacheTtlPolicy::deterministic_jitter(Duration::from_secs(60), 10, "bounded:v1")
                .unwrap(),
        );
        let calls = AtomicUsize::new(0);

        for key in ["".to_string(), "x".repeat(MAX_CACHE_POLICY_KEY_BYTES + 1)] {
            let error = service
                .load_or_fill_with_policy(backend.clone(), key, policy.clone(), || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(b"unexpected".to_vec())
                })
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
    async fn zero_ttl_policy_is_rejected_before_cache_or_loader_work() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        backend
            .set("present".to_string(), b"cached".to_vec())
            .await
            .unwrap();
        let calls = AtomicUsize::new(0);

        let error = service
            .load_or_fill_with_policy(
                backend.clone(),
                "present",
                CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::ZERO)),
                || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(b"unexpected".to_vec())
                },
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("TTL"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.get("present").await.unwrap(), Some(b"cached".to_vec()));
    }

    #[tokio::test]
    async fn loader_timeout_releases_coalescing_gate() {
        let service = CacheService::from_url(None);
        let backend = service.memory_backend(Duration::from_secs(60), 16);
        let policy = CacheLoadPolicy::new(CacheTtlPolicy::fixed(Duration::from_secs(60)))
            .with_loader_timeout(Duration::from_millis(5))
            .unwrap();

        let error = service
            .load_or_fill_with_policy(backend.clone(), "slow", policy, || {
                std::future::pending::<rustok_core::Result<Vec<u8>>>()
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert_eq!(service.in_flight_loads().await, 0);

        let calls = AtomicUsize::new(0);
        let result = service
            .load_or_fill_with_policy(backend, "slow", CacheLoadPolicy::default(), || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(b"recovered".to_vec())
            })
            .await
            .unwrap();
        assert_eq!(result.value, b"recovered".to_vec());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

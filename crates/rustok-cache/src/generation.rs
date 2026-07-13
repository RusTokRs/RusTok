use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::CacheService;

const GENERATION_KEY_PREFIX: &str = "rustok:cache-generation:v1";
const DEFAULT_GENERATION_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_GENERATION_NAMESPACE_BYTES: usize = 512;
pub const DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheNamespaceGeneration {
    value: u64,
    source: CacheGenerationSource,
}

impl CacheNamespaceGeneration {
    pub fn value(self) -> u64 {
        self.value
    }

    pub fn source(self) -> CacheGenerationSource {
        self.source
    }

    pub fn is_shared(self) -> bool {
        self.source == CacheGenerationSource::SharedRedis
    }

    /// Canonical component that can be appended to a versioned cache key.
    pub fn key_component(self) -> String {
        format!("g-{}", self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheGenerationSource {
    SharedRedis,
    LocalOnly,
    LocalFallback,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheGenerationStats {
    pub shared_reads: u64,
    pub shared_bumps: u64,
    pub read_failures: u64,
    pub bump_failures: u64,
    pub local_fallback_reads: u64,
}

#[derive(Default)]
struct CacheGenerationMetrics {
    shared_reads: AtomicU64,
    shared_bumps: AtomicU64,
    read_failures: AtomicU64,
    bump_failures: AtomicU64,
    local_fallback_reads: AtomicU64,
}

/// Shared namespace generation store.
///
/// Cache keys can include `CacheNamespaceGeneration::key_component()`. Bumping the generation
/// makes every value from the previous namespace generation unreachable without scanning or
/// deleting Redis keys. When Redis is configured, a failed bump is returned as an error because
/// acknowledging a process-local bump would not invalidate other instances.
///
/// Generation snapshots are monotonic. A Redis value lower than the process-local observation is
/// treated as shared state loss, and a Redis read failure can fall back only when this process has
/// previously observed or explicitly seeded a generation for the namespace. Trusted snapshots are
/// intentionally not evicted because eviction would silently discard fallback state; instead the
/// store rejects new namespaces after a bounded capacity is reached.
#[derive(Clone)]
pub struct CacheNamespaceGenerationStore {
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
    local: Arc<StdMutex<HashMap<String, u64>>>,
    max_local_snapshots: usize,
    operation_timeout: Duration,
    metrics: Arc<CacheGenerationMetrics>,
}

impl CacheNamespaceGenerationStore {
    #[cfg(feature = "redis-cache")]
    fn new(redis_client: Option<redis::Client>) -> Self {
        Self {
            redis_client,
            local: Arc::new(StdMutex::new(HashMap::new())),
            max_local_snapshots: DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS,
            operation_timeout: DEFAULT_GENERATION_OPERATION_TIMEOUT,
            metrics: Arc::new(CacheGenerationMetrics::default()),
        }
    }

    #[cfg(not(feature = "redis-cache"))]
    fn new() -> Self {
        Self {
            local: Arc::new(StdMutex::new(HashMap::new())),
            max_local_snapshots: DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS,
            operation_timeout: DEFAULT_GENERATION_OPERATION_TIMEOUT,
            metrics: Arc::new(CacheGenerationMetrics::default()),
        }
    }

    pub fn with_operation_timeout(
        mut self,
        operation_timeout: Duration,
    ) -> Result<Self, CacheGenerationError> {
        if operation_timeout.is_zero() {
            return Err(CacheGenerationError::ZeroOperationTimeout);
        }
        self.operation_timeout = operation_timeout;
        Ok(self)
    }

    /// Set a hard bound for trusted local namespace snapshots.
    ///
    /// Existing namespaces may continue to advance after the limit is reached. A previously unseen
    /// namespace is rejected instead of evicting trusted state and weakening outage recovery.
    pub fn with_max_local_snapshots(
        mut self,
        max_local_snapshots: usize,
    ) -> Result<Self, CacheGenerationError> {
        if max_local_snapshots == 0 {
            return Err(CacheGenerationError::ZeroLocalSnapshotCapacity);
        }
        let current = self.local_snapshot_count();
        if current > max_local_snapshots {
            return Err(CacheGenerationError::LocalSnapshotCapacityExceeded {
                count: current,
                maximum: max_local_snapshots,
            });
        }
        self.max_local_snapshots = max_local_snapshots;
        Ok(self)
    }

    pub async fn read(
        &self,
        namespace: &str,
    ) -> Result<CacheNamespaceGeneration, CacheGenerationError> {
        let namespace_key = generation_key(namespace)?;

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            match self.read_shared(client, &namespace_key).await {
                Ok(value) => {
                    if let Err(error) = self.observe_shared(&namespace_key, value) {
                        self.metrics.read_failures.fetch_add(1, Ordering::Relaxed);
                        return Err(error);
                    }
                    self.metrics.shared_reads.fetch_add(1, Ordering::Relaxed);
                    return Ok(CacheNamespaceGeneration {
                        value,
                        source: CacheGenerationSource::SharedRedis,
                    });
                }
                Err(error) => {
                    self.metrics.read_failures.fetch_add(1, Ordering::Relaxed);
                    let Some(value) = self.local_snapshot(&namespace_key) else {
                        tracing::warn!(
                            %error,
                            namespace,
                            "Shared cache generation read failed without a trusted local snapshot"
                        );
                        return Err(CacheGenerationError::NoLocalSnapshot);
                    };
                    self.metrics
                        .local_fallback_reads
                        .fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(
                        %error,
                        namespace,
                        generation = value,
                        "Shared cache generation read failed; using last observed local snapshot"
                    );
                    return Ok(CacheNamespaceGeneration {
                        value,
                        source: CacheGenerationSource::LocalFallback,
                    });
                }
            }
        }

        Ok(CacheNamespaceGeneration {
            value: self.local_snapshot(&namespace_key).unwrap_or(0),
            source: CacheGenerationSource::LocalOnly,
        })
    }

    pub async fn bump(
        &self,
        namespace: &str,
    ) -> Result<CacheNamespaceGeneration, CacheGenerationError> {
        let namespace_key = generation_key(namespace)?;

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            match self.bump_shared(client, &namespace_key).await {
                Ok(value) => {
                    if let Err(error) = self.observe_shared(&namespace_key, value) {
                        self.metrics.bump_failures.fetch_add(1, Ordering::Relaxed);
                        return Err(error);
                    }
                    self.metrics.shared_bumps.fetch_add(1, Ordering::Relaxed);
                    return Ok(CacheNamespaceGeneration {
                        value,
                        source: CacheGenerationSource::SharedRedis,
                    });
                }
                Err(error) => {
                    self.metrics.bump_failures.fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }
            }
        }

        let value = {
            let mut local = self
                .local
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.ensure_snapshot_capacity(&local, &namespace_key)?;
            let value = local.entry(namespace_key).or_insert(0);
            *value = value
                .checked_add(1)
                .ok_or(CacheGenerationError::GenerationOverflow)?;
            *value
        };
        Ok(CacheNamespaceGeneration {
            value,
            source: CacheGenerationSource::LocalOnly,
        })
    }

    /// Seed a generation restored from a durable consumer checkpoint.
    ///
    /// A seed can advance or repeat the current snapshot, but it can never lower it.
    pub fn seed_local(
        &self,
        namespace: &str,
        generation: u64,
    ) -> Result<(), CacheGenerationError> {
        let namespace_key = generation_key(namespace)?;
        self.observe_shared(&namespace_key, generation)
    }

    pub fn stats(&self) -> CacheGenerationStats {
        CacheGenerationStats {
            shared_reads: self.metrics.shared_reads.load(Ordering::Relaxed),
            shared_bumps: self.metrics.shared_bumps.load(Ordering::Relaxed),
            read_failures: self.metrics.read_failures.load(Ordering::Relaxed),
            bump_failures: self.metrics.bump_failures.load(Ordering::Relaxed),
            local_fallback_reads: self
                .metrics
                .local_fallback_reads
                .load(Ordering::Relaxed),
        }
    }

    pub fn local_snapshot_count(&self) -> usize {
        self.local
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    pub fn max_local_snapshots(&self) -> usize {
        self.max_local_snapshots
    }

    fn local_snapshot(&self, namespace_key: &str) -> Option<u64> {
        self.local
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(namespace_key)
            .copied()
    }

    fn observe_shared(
        &self,
        namespace_key: &str,
        generation: u64,
    ) -> Result<(), CacheGenerationError> {
        let mut local = self
            .local
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(previous) = local.get(namespace_key).copied() {
            if generation < previous {
                return Err(CacheGenerationError::GenerationRegressed {
                    local: previous,
                    shared: generation,
                });
            }
        } else {
            self.ensure_snapshot_capacity(&local, namespace_key)?;
        }
        local.insert(namespace_key.to_string(), generation);
        Ok(())
    }

    fn ensure_snapshot_capacity(
        &self,
        local: &HashMap<String, u64>,
        namespace_key: &str,
    ) -> Result<(), CacheGenerationError> {
        if local.contains_key(namespace_key) {
            return Ok(());
        }
        let next_count = local.len().checked_add(1).unwrap_or(usize::MAX);
        if next_count > self.max_local_snapshots {
            return Err(CacheGenerationError::LocalSnapshotCapacityExceeded {
                count: next_count,
                maximum: self.max_local_snapshots,
            });
        }
        Ok(())
    }

    #[cfg(feature = "redis-cache")]
    async fn read_shared(
        &self,
        client: &redis::Client,
        key: &str,
    ) -> Result<u64, CacheGenerationError> {
        let mut connection = generation_timeout(
            self.operation_timeout,
            "generation connection",
            client.get_multiplexed_async_connection(),
        )
        .await?;
        let value = generation_timeout(
            self.operation_timeout,
            "generation GET",
            redis::cmd("GET").arg(key).query_async::<Option<u64>>(&mut connection),
        )
        .await?;
        Ok(value.unwrap_or(0))
    }

    #[cfg(feature = "redis-cache")]
    async fn bump_shared(
        &self,
        client: &redis::Client,
        key: &str,
    ) -> Result<u64, CacheGenerationError> {
        let mut connection = generation_timeout(
            self.operation_timeout,
            "generation connection",
            client.get_multiplexed_async_connection(),
        )
        .await?;
        generation_timeout(
            self.operation_timeout,
            "generation INCR",
            redis::cmd("INCR").arg(key).query_async::<u64>(&mut connection),
        )
        .await
    }
}

impl CacheService {
    pub fn namespace_generations(&self) -> CacheNamespaceGenerationStore {
        #[cfg(feature = "redis-cache")]
        {
            CacheNamespaceGenerationStore::new(self.redis_client().cloned())
        }
        #[cfg(not(feature = "redis-cache"))]
        {
            CacheNamespaceGenerationStore::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheGenerationError {
    EmptyNamespace,
    NamespaceTooLarge {
        length: usize,
        maximum: usize,
    },
    ZeroOperationTimeout,
    ZeroLocalSnapshotCapacity,
    LocalSnapshotCapacityExceeded {
        count: usize,
        maximum: usize,
    },
    GenerationOverflow,
    NoLocalSnapshot,
    GenerationRegressed {
        local: u64,
        shared: u64,
    },
    Redis(String),
    Timeout {
        operation: &'static str,
        timeout_ms: u128,
    },
}

impl std::fmt::Display for CacheGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyNamespace => {
                write!(formatter, "cache generation namespace must not be empty")
            }
            Self::NamespaceTooLarge { length, maximum } => write!(
                formatter,
                "cache generation namespace is {length} bytes; maximum is {maximum}"
            ),
            Self::ZeroOperationTimeout => {
                write!(
                    formatter,
                    "cache generation operation timeout must be greater than zero"
                )
            }
            Self::ZeroLocalSnapshotCapacity => {
                write!(
                    formatter,
                    "cache generation local snapshot capacity must be greater than zero"
                )
            }
            Self::LocalSnapshotCapacityExceeded { count, maximum } => write!(
                formatter,
                "cache generation local snapshot count {count} exceeds maximum {maximum}"
            ),
            Self::GenerationOverflow => write!(formatter, "cache generation counter overflowed"),
            Self::NoLocalSnapshot => write!(
                formatter,
                "shared cache generation is unavailable and no trusted local snapshot exists"
            ),
            Self::GenerationRegressed { local, shared } => write!(
                formatter,
                "shared cache generation regressed from local {local} to {shared}"
            ),
            Self::Redis(message) => write!(formatter, "cache generation Redis error: {message}"),
            Self::Timeout {
                operation,
                timeout_ms,
            } => write!(
                formatter,
                "cache {operation} timed out after {timeout_ms} ms"
            ),
        }
    }
}

impl std::error::Error for CacheGenerationError {}

fn generation_key(namespace: &str) -> Result<String, CacheGenerationError> {
    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(CacheGenerationError::EmptyNamespace);
    }
    if namespace.len() > MAX_GENERATION_NAMESPACE_BYTES {
        return Err(CacheGenerationError::NamespaceTooLarge {
            length: namespace.len(),
            maximum: MAX_GENERATION_NAMESPACE_BYTES,
        });
    }
    Ok(format!(
        "{GENERATION_KEY_PREFIX}:{}",
        hex::encode(Sha256::digest(namespace.as_bytes()))
    ))
}

#[cfg(feature = "redis-cache")]
async fn generation_timeout<T, F, E>(
    timeout: Duration,
    operation: &'static str,
    future: F,
) -> Result<T, CacheGenerationError>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| CacheGenerationError::Timeout {
            operation,
            timeout_ms: timeout.as_millis(),
        })?
        .map_err(|error| CacheGenerationError::Redis(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_generation_starts_at_zero_and_bumps_monotonically() {
        let service = CacheService::from_url(None);
        let generations = service.namespace_generations();

        let initial = generations.read("tenant-cache").await.unwrap();
        assert_eq!(initial.value(), 0);
        assert_eq!(initial.source(), CacheGenerationSource::LocalOnly);

        let first = generations.bump("tenant-cache").await.unwrap();
        let second = generations.bump("tenant-cache").await.unwrap();
        assert_eq!(first.value(), 1);
        assert_eq!(second.value(), 2);
        assert_eq!(second.key_component(), "g-2");
        assert_eq!(generations.read("tenant-cache").await.unwrap().value(), 2);
    }

    #[test]
    fn generation_namespace_is_validated_and_hashed() {
        let first = generation_key("tenant:catalog:v1").unwrap();
        let second = generation_key("tenant:catalog:v1").unwrap();
        assert_eq!(first, second);
        assert!(first.starts_with(GENERATION_KEY_PREFIX));
        assert!(!first.contains("tenant:catalog:v1"));
        assert_eq!(
            generation_key("  ").unwrap_err(),
            CacheGenerationError::EmptyNamespace
        );
    }

    #[test]
    fn rejects_zero_operation_timeout() {
        let service = CacheService::from_url(None);
        let error = service
            .namespace_generations()
            .with_operation_timeout(Duration::ZERO)
            .err()
            .expect("zero timeout must be rejected");
        assert_eq!(error, CacheGenerationError::ZeroOperationTimeout);
    }

    #[test]
    fn rejects_zero_local_snapshot_capacity() {
        let service = CacheService::from_url(None);
        let error = service
            .namespace_generations()
            .with_max_local_snapshots(0)
            .err()
            .expect("zero snapshot capacity must be rejected");
        assert_eq!(error, CacheGenerationError::ZeroLocalSnapshotCapacity);
    }

    #[test]
    fn trusted_local_snapshots_are_bounded_without_evicting_existing_namespaces() {
        let service = CacheService::from_url(None);
        let generations = service
            .namespace_generations()
            .with_max_local_snapshots(2)
            .unwrap();

        generations.seed_local("tenant-a", 1).unwrap();
        generations.seed_local("tenant-b", 2).unwrap();
        generations.seed_local("tenant-a", 3).unwrap();

        assert_eq!(generations.local_snapshot_count(), 2);
        assert_eq!(generations.max_local_snapshots(), 2);
        assert_eq!(
            generations.seed_local("tenant-c", 1).unwrap_err(),
            CacheGenerationError::LocalSnapshotCapacityExceeded {
                count: 3,
                maximum: 2,
            }
        );
        let tenant_a = generation_key("tenant-a").unwrap();
        let tenant_b = generation_key("tenant-b").unwrap();
        assert_eq!(generations.local_snapshot(&tenant_a), Some(3));
        assert_eq!(generations.local_snapshot(&tenant_b), Some(2));
    }

    #[tokio::test]
    async fn local_bump_rejects_new_namespace_after_snapshot_capacity_is_reached() {
        let service = CacheService::from_url(None);
        let generations = service
            .namespace_generations()
            .with_max_local_snapshots(1)
            .unwrap();

        generations.bump("tenant-a").await.unwrap();
        assert_eq!(
            generations.bump("tenant-b").await.unwrap_err(),
            CacheGenerationError::LocalSnapshotCapacityExceeded {
                count: 2,
                maximum: 1,
            }
        );
        assert_eq!(generations.bump("tenant-a").await.unwrap().value(), 2);
    }

    #[test]
    fn durable_seed_and_shared_observation_cannot_regress() {
        let service = CacheService::from_url(None);
        let generations = service.namespace_generations();
        generations.seed_local("tenant-cache", 7).unwrap();
        generations.seed_local("tenant-cache", 7).unwrap();

        assert_eq!(
            generations.seed_local("tenant-cache", 6).unwrap_err(),
            CacheGenerationError::GenerationRegressed {
                local: 7,
                shared: 6,
            }
        );
        let key = generation_key("tenant-cache").unwrap();
        assert_eq!(generations.local_snapshot(&key), Some(7));
    }

    #[test]
    fn unknown_namespace_has_no_trusted_fallback_snapshot() {
        let service = CacheService::from_url(None);
        let generations = service.namespace_generations();
        let key = generation_key("never-observed").unwrap();

        assert_eq!(generations.local_snapshot(&key), None);
    }
}

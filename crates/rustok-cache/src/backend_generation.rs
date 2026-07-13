use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats};
use uuid::Uuid;

use crate::{CacheGenerationError, CacheService};

pub const DEFAULT_MAX_CACHE_BACKEND_GENERATIONS: usize = 4_096;
pub const MAX_CACHE_BACKEND_PREFIX_BYTES: usize = 512;

static BACKEND_GENERATIONS: OnceLock<Mutex<HashMap<String, Arc<BackendGenerationState>>>> =
    OnceLock::new();

fn backend_generations() -> &'static Mutex<HashMap<String, Arc<BackendGenerationState>>> {
    BACKEND_GENERATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct BackendGenerationState {
    generation: AtomicU64,
    trusted: AtomicBool,
    boot_namespace: String,
    update_lock: Mutex<()>,
}

impl BackendGenerationState {
    fn untrusted() -> Self {
        Self {
            generation: AtomicU64::new(0),
            trusted: AtomicBool::new(false),
            boot_namespace: format!("boot-{}", Uuid::new_v4().simple()),
            update_lock: Mutex::new(()),
        }
    }

    fn snapshot(&self) -> CacheBackendGenerationSnapshot {
        CacheBackendGenerationSnapshot {
            generation: self.generation.load(Ordering::Acquire),
            trusted: self.trusted.load(Ordering::Acquire),
        }
    }

    fn observe(&self, generation: u64) -> Result<(), CacheBackendGenerationError> {
        let _guard = self
            .update_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.trusted.load(Ordering::Acquire) {
            let current = self.generation.load(Ordering::Acquire);
            if generation < current {
                return Err(CacheBackendGenerationError::GenerationRegressed {
                    current,
                    proposed: generation,
                });
            }
        }
        self.generation.store(generation, Ordering::Release);
        self.trusted.store(true, Ordering::Release);
        Ok(())
    }

    fn physical_key(&self, logical_key: &str) -> String {
        if self.trusted.load(Ordering::Acquire) {
            format!(
                "g-{}:{logical_key}",
                self.generation.load(Ordering::Acquire)
            )
        } else {
            format!("{}:{logical_key}", self.boot_namespace)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheBackendGenerationSnapshot {
    pub generation: u64,
    pub trusted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheBackendGenerationError {
    EmptyPrefix,
    PrefixTooLarge { length: usize, maximum: usize },
    RegistryCapacityExceeded { count: usize, maximum: usize },
    GenerationRegressed { current: u64, proposed: u64 },
    SharedGeneration(String),
}

impl std::fmt::Display for CacheBackendGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPrefix => write!(formatter, "cache backend generation prefix must not be empty"),
            Self::PrefixTooLarge { length, maximum } => write!(
                formatter,
                "cache backend generation prefix is {length} bytes; maximum is {maximum}"
            ),
            Self::RegistryCapacityExceeded { count, maximum } => write!(
                formatter,
                "cache backend generation registry reached capacity {maximum}; current count {count}"
            ),
            Self::GenerationRegressed { current, proposed } => write!(
                formatter,
                "cache backend generation cannot regress from {current} to {proposed}"
            ),
            Self::SharedGeneration(message) => {
                write!(formatter, "shared cache backend generation failed: {message}")
            }
        }
    }
}

impl std::error::Error for CacheBackendGenerationError {}

impl From<CacheGenerationError> for CacheBackendGenerationError {
    fn from(error: CacheGenerationError) -> Self {
        Self::SharedGeneration(error.to_string())
    }
}

fn validate_prefix(prefix: &str) -> Result<(), CacheBackendGenerationError> {
    if prefix.trim().is_empty() {
        return Err(CacheBackendGenerationError::EmptyPrefix);
    }
    if prefix.len() > MAX_CACHE_BACKEND_PREFIX_BYTES {
        return Err(CacheBackendGenerationError::PrefixTooLarge {
            length: prefix.len(),
            maximum: MAX_CACHE_BACKEND_PREFIX_BYTES,
        });
    }
    Ok(())
}

fn generation_state(
    prefix: &str,
) -> Result<Arc<BackendGenerationState>, CacheBackendGenerationError> {
    validate_prefix(prefix)?;
    let mut registry = backend_generations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(state) = registry.get(prefix) {
        return Ok(Arc::clone(state));
    }
    if registry.len() >= DEFAULT_MAX_CACHE_BACKEND_GENERATIONS {
        return Err(CacheBackendGenerationError::RegistryCapacityExceeded {
            count: registry.len(),
            maximum: DEFAULT_MAX_CACHE_BACKEND_GENERATIONS,
        });
    }
    let state = Arc::new(BackendGenerationState::untrusted());
    registry.insert(prefix.to_string(), Arc::clone(&state));
    Ok(state)
}

/// Observe a shared generation delivered by a durable invalidation consumer.
///
/// Calling this before backend construction is supported: the future backend reuses the seeded
/// state. Regressions are rejected so stale or reordered events cannot reopen an old namespace.
pub fn observe_cache_backend_generation(
    prefix: &str,
    generation: u64,
) -> Result<CacheBackendGenerationSnapshot, CacheBackendGenerationError> {
    let state = generation_state(prefix)?;
    state.observe(generation)?;
    Ok(state.snapshot())
}

pub fn cache_backend_generation_snapshot(
    prefix: &str,
) -> Result<CacheBackendGenerationSnapshot, CacheBackendGenerationError> {
    Ok(generation_state(prefix)?.snapshot())
}

pub fn cache_backend_generation_registry_size() -> usize {
    backend_generations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
}

impl CacheService {
    /// Atomically bump the shared generation and immediately switch local backends to it.
    pub async fn bump_cache_backend_generation(
        &self,
        prefix: &str,
    ) -> Result<CacheBackendGenerationSnapshot, CacheBackendGenerationError> {
        validate_prefix(prefix)?;
        let generation = self.namespace_generations().bump(prefix).await?;
        observe_cache_backend_generation(prefix, generation.value())
    }

    pub(crate) async fn wrap_generation_aware_backend(
        &self,
        prefix: &str,
        inner: Arc<dyn CacheBackend>,
    ) -> Arc<dyn CacheBackend> {
        let state = match generation_state(prefix) {
            Ok(state) => state,
            Err(error) => {
                tracing::error!(%error, prefix, "Cache backend generation registry unavailable");
                return Arc::new(GenerationAwareCacheBackend::rejected(inner, error));
            }
        };

        if !state.trusted.load(Ordering::Acquire) {
            match self.namespace_generations().read(prefix).await {
                Ok(generation) => {
                    if let Err(error) = state.observe(generation.value()) {
                        tracing::error!(%error, prefix, "Cache backend generation initialization regressed");
                        return Arc::new(GenerationAwareCacheBackend::rejected(inner, error));
                    }
                }
                Err(error) => {
                    // Keep the unique boot namespace. This is intentionally not `g-0`: if Redis
                    // later recovers, values from an older process/shared generation remain
                    // unreachable until a trusted shared generation is observed.
                    tracing::warn!(%error, prefix, "Shared cache generation unavailable; using isolated boot namespace");
                }
            }
        }

        Arc::new(GenerationAwareCacheBackend::new(inner, state))
    }
}

struct GenerationAwareCacheBackend {
    inner: Arc<dyn CacheBackend>,
    state: Arc<BackendGenerationState>,
    rejected: Option<CacheBackendGenerationError>,
}

impl GenerationAwareCacheBackend {
    fn new(inner: Arc<dyn CacheBackend>, state: Arc<BackendGenerationState>) -> Self {
        Self {
            inner,
            state,
            rejected: None,
        }
    }

    fn rejected(inner: Arc<dyn CacheBackend>, error: CacheBackendGenerationError) -> Self {
        Self {
            inner,
            state: Arc::new(BackendGenerationState::untrusted()),
            rejected: Some(error),
        }
    }

    fn key(&self, logical_key: &str) -> rustok_core::Result<String> {
        if let Some(error) = &self.rejected {
            return Err(rustok_core::Error::Cache(error.to_string()));
        }
        Ok(self.state.physical_key(logical_key))
    }
}

#[async_trait]
impl CacheBackend for GenerationAwareCacheBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.key("health-probe")?;
        self.inner.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        self.inner.get(&self.key(key)?).await
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        self.inner.set(self.key(&key)?, value).await
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        self.inner.set_with_ttl(self.key(&key)?, value, ttl).await
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        self.inner
            .compare_and_set(&self.key(key)?, expected, value, ttl)
            .await
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        self.inner.invalidate(&self.key(key)?).await
    }

    fn stats(&self) -> CacheStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheService;

    fn unique_prefix(name: &str) -> String {
        format!("test:{name}:{}", Uuid::new_v4().simple())
    }

    #[tokio::test]
    async fn generation_switch_makes_old_values_unreachable() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("switch");
        let raw = service.memory_backend(Duration::from_secs(60), 16);
        let backend = service.wrap_generation_aware_backend(&prefix, raw).await;

        backend.set("key".to_string(), b"old".to_vec()).await.unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"old".to_vec()));

        observe_cache_backend_generation(&prefix, 1).unwrap();
        assert_eq!(backend.get("key").await.unwrap(), None);
        backend.set("key".to_string(), b"new".to_vec()).await.unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn state_seeded_before_backend_creation_is_reused() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("preseed");
        observe_cache_backend_generation(&prefix, 7).unwrap();
        let raw = service.memory_backend(Duration::from_secs(60), 16);
        let backend = service.wrap_generation_aware_backend(&prefix, raw).await;

        backend.set("key".to_string(), b"value".to_vec()).await.unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"value".to_vec()));
        assert_eq!(cache_backend_generation_snapshot(&prefix).unwrap().generation, 7);
    }

    #[test]
    fn observed_generation_cannot_regress() {
        let prefix = unique_prefix("monotonic");
        observe_cache_backend_generation(&prefix, 9).unwrap();
        assert!(matches!(
            observe_cache_backend_generation(&prefix, 8),
            Err(CacheBackendGenerationError::GenerationRegressed {
                current: 9,
                proposed: 8
            })
        ));
    }

    #[test]
    fn registry_and_prefix_inputs_are_bounded() {
        assert!(matches!(
            cache_backend_generation_snapshot("  "),
            Err(CacheBackendGenerationError::EmptyPrefix)
        ));
        assert!(matches!(
            cache_backend_generation_snapshot(&"x".repeat(MAX_CACHE_BACKEND_PREFIX_BYTES + 1)),
            Err(CacheBackendGenerationError::PrefixTooLarge { .. })
        ));
        assert!(cache_backend_generation_registry_size() <= DEFAULT_MAX_CACHE_BACKEND_GENERATIONS);
    }
}

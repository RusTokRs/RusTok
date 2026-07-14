use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{CacheBackend, CacheCompareAndSetOutcome, CacheStats};
use uuid::Uuid;

use crate::{CacheGenerationError, CacheService};

pub const DEFAULT_MAX_CACHE_BACKEND_GENERATIONS: usize = 4_096;
pub const MAX_CACHE_BACKEND_PREFIX_BYTES: usize = 512;
const MAX_GENERATION_OPERATION_ATTEMPTS: usize = 4;

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
        // Observe trust first. The transition to trusted stores the generation before publishing
        // `trusted = true`; loading generation afterwards avoids a mixed `trusted=true, g=old`
        // snapshot. A concurrent later rotation is detected by the post-I/O token recheck.
        let trusted = self.trusted.load(Ordering::Acquire);
        let generation = if trusted {
            self.generation.load(Ordering::Acquire)
        } else {
            0
        };
        CacheBackendGenerationSnapshot {
            generation,
            trusted,
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

    fn bump_local(&self) -> Result<u64, CacheBackendGenerationError> {
        let _guard = self
            .update_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = if self.trusted.load(Ordering::Acquire) {
            self.generation.load(Ordering::Acquire)
        } else {
            0
        };
        let next = current
            .checked_add(1)
            .ok_or(CacheBackendGenerationError::GenerationExhausted)?;
        self.generation.store(next, Ordering::Release);
        self.trusted.store(true, Ordering::Release);
        Ok(next)
    }

    fn physical_key(
        &self,
        snapshot: CacheBackendGenerationSnapshot,
        logical_key: &str,
    ) -> String {
        if snapshot.trusted {
            format!("g-{}:{logical_key}", snapshot.generation)
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
    GenerationExhausted,
    RedisClientUnavailable,
    AliasAlreadyBound { alias: String },
    SharedGeneration(String),
}

impl std::fmt::Display for CacheBackendGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPrefix => {
                write!(formatter, "cache backend generation prefix must not be empty")
            }
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
            Self::GenerationExhausted => {
                write!(formatter, "cache backend generation is exhausted")
            }
            Self::RedisClientUnavailable => write!(
                formatter,
                "Redis is configured but its client is unavailable for shared generation updates"
            ),
            Self::AliasAlreadyBound { alias } => write!(
                formatter,
                "cache backend generation alias {alias:?} is already bound to another state"
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

/// Bind multiple physical backend prefixes to one canonical generation state.
///
/// This must run before any aliased backend is constructed. Once a prefix has a distinct state,
/// rebinding is rejected instead of silently leaving existing backends attached to stale state.
/// Every alias then reads the exact same atomic generation and boot namespace as the canonical
/// prefix, so a namespace rotation cannot expose a mixed data/negative-cache generation.
/// Conflict and capacity checks complete before the registry is mutated, making a failed binding
/// atomic: no canonical or alias entry is left behind after an error.
pub fn bind_cache_backend_generation_aliases(
    canonical: &str,
    aliases: &[&str],
) -> Result<CacheBackendGenerationSnapshot, CacheBackendGenerationError> {
    validate_prefix(canonical)?;
    let mut seen = HashSet::new();
    let mut unique_aliases = Vec::with_capacity(aliases.len());
    for alias in aliases {
        validate_prefix(alias)?;
        if *alias != canonical && seen.insert(*alias) {
            unique_aliases.push(*alias);
        }
    }

    let mut registry = backend_generations()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let existing_canonical = registry.get(canonical).cloned();

    for alias in &unique_aliases {
        let Some(existing_alias) = registry.get(*alias) else {
            continue;
        };
        let Some(canonical_state) = existing_canonical.as_ref() else {
            return Err(CacheBackendGenerationError::AliasAlreadyBound {
                alias: (*alias).to_string(),
            });
        };
        if !Arc::ptr_eq(existing_alias, canonical_state) {
            return Err(CacheBackendGenerationError::AliasAlreadyBound {
                alias: (*alias).to_string(),
            });
        }
    }

    let missing_canonical = usize::from(existing_canonical.is_none());
    let missing_aliases = unique_aliases
        .iter()
        .filter(|alias| !registry.contains_key(**alias))
        .count();
    let required = missing_canonical.saturating_add(missing_aliases);
    if registry.len().saturating_add(required) > DEFAULT_MAX_CACHE_BACKEND_GENERATIONS {
        return Err(CacheBackendGenerationError::RegistryCapacityExceeded {
            count: registry.len(),
            maximum: DEFAULT_MAX_CACHE_BACKEND_GENERATIONS,
        });
    }

    let canonical_state = existing_canonical.unwrap_or_else(|| {
        let state = Arc::new(BackendGenerationState::untrusted());
        registry.insert(canonical.to_string(), Arc::clone(&state));
        state
    });
    for alias in unique_aliases {
        registry
            .entry(alias.to_string())
            .or_insert_with(|| Arc::clone(&canonical_state));
    }
    Ok(canonical_state.snapshot())
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
        if self.redis_configuration_present() {
            if !self.redis_client_initialized() {
                return Err(CacheBackendGenerationError::RedisClientUnavailable);
            }
            let generation = self.namespace_generations().bump(prefix).await?;
            return observe_cache_backend_generation(prefix, generation.value());
        }

        let state = generation_state(prefix)?;
        state.bump_local()?;
        Ok(state.snapshot())
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
            if self.redis_configuration_present() && !self.redis_client_initialized() {
                tracing::error!(
                    prefix,
                    "Redis generation client unavailable; using isolated boot namespace"
                );
            } else {
                match self.namespace_generations().read(prefix).await {
                    Ok(generation) => {
                        if let Err(error) = state.observe(generation.value()) {
                            tracing::error!(%error, prefix, "Cache backend generation initialization regressed");
                            return Arc::new(GenerationAwareCacheBackend::rejected(inner, error));
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, prefix, "Shared cache generation unavailable; using isolated boot namespace");
                    }
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

    fn ensure_available(&self) -> rustok_core::Result<()> {
        if let Some(error) = &self.rejected {
            return Err(rustok_core::Error::Cache(error.to_string()));
        }
        Ok(())
    }

    fn key_snapshot(
        &self,
        logical_key: &str,
    ) -> rustok_core::Result<(CacheBackendGenerationSnapshot, String)> {
        self.ensure_available()?;
        let snapshot = self.state.snapshot();
        let physical_key = self.state.physical_key(snapshot, logical_key);
        Ok((snapshot, physical_key))
    }

    fn snapshot_is_current(&self, snapshot: CacheBackendGenerationSnapshot) -> bool {
        self.state.snapshot() == snapshot
    }

    fn generation_changed_error() -> rustok_core::Error {
        rustok_core::Error::Cache(format!(
            "cache backend generation changed during operation for {MAX_GENERATION_OPERATION_ATTEMPTS} consecutive attempts"
        ))
    }
}

#[async_trait]
impl CacheBackend for GenerationAwareCacheBackend {
    async fn health(&self) -> rustok_core::Result<()> {
        self.ensure_available()?;
        self.inner.health().await
    }

    async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
        for _ in 0..MAX_GENERATION_OPERATION_ATTEMPTS {
            let (snapshot, physical_key) = self.key_snapshot(key)?;
            let result = self.inner.get(&physical_key).await;
            if self.snapshot_is_current(snapshot) {
                return result;
            }
        }
        Err(Self::generation_changed_error())
    }

    async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
        for _ in 0..MAX_GENERATION_OPERATION_ATTEMPTS {
            let (snapshot, physical_key) = self.key_snapshot(&key)?;
            let result = self.inner.set(physical_key, value.clone()).await;
            if self.snapshot_is_current(snapshot) {
                return result;
            }
        }
        Err(Self::generation_changed_error())
    }

    async fn set_with_ttl(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> rustok_core::Result<()> {
        for _ in 0..MAX_GENERATION_OPERATION_ATTEMPTS {
            let (snapshot, physical_key) = self.key_snapshot(&key)?;
            let result = self
                .inner
                .set_with_ttl(physical_key, value.clone(), ttl)
                .await;
            if self.snapshot_is_current(snapshot) {
                return result;
            }
        }
        Err(Self::generation_changed_error())
    }

    async fn compare_and_set(
        &self,
        key: &str,
        expected: &[u8],
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
        for _ in 0..MAX_GENERATION_OPERATION_ATTEMPTS {
            let (snapshot, physical_key) = self.key_snapshot(key)?;
            let result = self
                .inner
                .compare_and_set(&physical_key, expected, value.clone(), ttl)
                .await;
            if self.snapshot_is_current(snapshot) {
                return result;
            }
        }
        Err(Self::generation_changed_error())
    }

    async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
        for _ in 0..MAX_GENERATION_OPERATION_ATTEMPTS {
            let (snapshot, physical_key) = self.key_snapshot(key)?;
            let result = self.inner.invalidate(&physical_key).await;
            if self.snapshot_is_current(snapshot) {
                return result;
            }
        }
        Err(Self::generation_changed_error())
    }

    fn stats(&self) -> CacheStats {
        self.inner.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheService;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use tokio::sync::Notify;

    fn unique_prefix(name: &str) -> String {
        format!("test:{name}:{}", Uuid::new_v4().simple())
    }

    struct BlockingGenerationBackend {
        values: Mutex<HashMap<String, Vec<u8>>>,
        block_get_once: AtomicBool,
        block_set_once: AtomicBool,
        get_started: Arc<Notify>,
        get_release: Arc<Notify>,
        set_started: Arc<Notify>,
        set_release: Arc<Notify>,
    }

    impl BlockingGenerationBackend {
        fn new(block_get_once: bool, block_set_once: bool) -> Self {
            Self {
                values: Mutex::new(HashMap::new()),
                block_get_once: AtomicBool::new(block_get_once),
                block_set_once: AtomicBool::new(block_set_once),
                get_started: Arc::new(Notify::new()),
                get_release: Arc::new(Notify::new()),
                set_started: Arc::new(Notify::new()),
                set_release: Arc::new(Notify::new()),
            }
        }

        fn insert_raw(&self, key: impl Into<String>, value: Vec<u8>) {
            self.values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key.into(), value);
        }

        fn get_raw(&self, key: &str) -> Option<Vec<u8>> {
            self.values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(key)
                .cloned()
        }
    }

    #[async_trait]
    impl CacheBackend for BlockingGenerationBackend {
        async fn health(&self) -> rustok_core::Result<()> {
            Ok(())
        }

        async fn get(&self, key: &str) -> rustok_core::Result<Option<Vec<u8>>> {
            if self.block_get_once.swap(false, AtomicOrdering::SeqCst) {
                self.get_started.notify_one();
                self.get_release.notified().await;
            }
            Ok(self.get_raw(key))
        }

        async fn set(&self, key: String, value: Vec<u8>) -> rustok_core::Result<()> {
            if self.block_set_once.swap(false, AtomicOrdering::SeqCst) {
                self.set_started.notify_one();
                self.set_release.notified().await;
            }
            self.insert_raw(key, value);
            Ok(())
        }

        async fn set_with_ttl(
            &self,
            key: String,
            value: Vec<u8>,
            ttl: Duration,
        ) -> rustok_core::Result<()> {
            if ttl.is_zero() {
                return self.invalidate(&key).await;
            }
            self.set(key, value).await
        }

        async fn compare_and_set(
            &self,
            key: &str,
            expected: &[u8],
            value: Vec<u8>,
            ttl: Option<Duration>,
        ) -> rustok_core::Result<CacheCompareAndSetOutcome> {
            let mut values = self
                .values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if values.get(key).map(Vec::as_slice) != Some(expected) {
                return Ok(CacheCompareAndSetOutcome::Mismatch);
            }
            if ttl.is_some_and(Duration::is_zero) {
                values.remove(key);
            } else {
                values.insert(key.to_string(), value);
            }
            Ok(CacheCompareAndSetOutcome::Applied)
        }

        async fn invalidate(&self, key: &str) -> rustok_core::Result<()> {
            self.values
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(key);
            Ok(())
        }

        fn stats(&self) -> CacheStats {
            CacheStats {
                entries: self
                    .values
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .len() as u64,
                ..CacheStats::default()
            }
        }
    }

    #[tokio::test]
    async fn generation_switch_makes_old_values_unreachable() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("switch");
        let raw = service.memory_backend(Duration::from_secs(60), 16);
        let backend = service.wrap_generation_aware_backend(&prefix, raw).await;

        backend
            .set("key".to_string(), b"old".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"old".to_vec()));

        observe_cache_backend_generation(&prefix, 1).unwrap();
        assert_eq!(backend.get("key").await.unwrap(), None);
        backend
            .set("key".to_string(), b"new".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn get_discards_old_namespace_result_when_generation_changes_midflight() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("get-race");
        let raw = Arc::new(BlockingGenerationBackend::new(true, false));
        raw.insert_raw("g-0:key", b"stale".to_vec());
        let raw_backend: Arc<dyn CacheBackend> = raw.clone();
        let backend = service
            .wrap_generation_aware_backend(&prefix, raw_backend)
            .await;
        let get_started = Arc::clone(&raw.get_started);
        let get_release = Arc::clone(&raw.get_release);

        let read_backend = Arc::clone(&backend);
        let read = tokio::spawn(async move { read_backend.get("key").await });
        get_started.notified().await;
        observe_cache_backend_generation(&prefix, 1).unwrap();
        get_release.notify_one();

        assert_eq!(read.await.unwrap().unwrap(), None);
        assert_eq!(backend.get("key").await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_retries_current_namespace_when_generation_changes_midflight() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("set-race");
        let raw = Arc::new(BlockingGenerationBackend::new(false, true));
        let raw_backend: Arc<dyn CacheBackend> = raw.clone();
        let backend = service
            .wrap_generation_aware_backend(&prefix, raw_backend)
            .await;
        let set_started = Arc::clone(&raw.set_started);
        let set_release = Arc::clone(&raw.set_release);

        let write_backend = Arc::clone(&backend);
        let write = tokio::spawn(async move {
            write_backend
                .set("key".to_string(), b"value".to_vec())
                .await
        });
        set_started.notified().await;
        observe_cache_backend_generation(&prefix, 1).unwrap();
        set_release.notify_one();

        write.await.unwrap().unwrap();
        assert_eq!(raw.get_raw("g-0:key"), Some(b"value".to_vec()));
        assert_eq!(raw.get_raw("g-1:key"), Some(b"value".to_vec()));
        assert_eq!(backend.get("key").await.unwrap(), Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn aliased_backends_share_one_atomic_generation_state() {
        let service = CacheService::from_url(None);
        let canonical = unique_prefix("canonical");
        let data = unique_prefix("data");
        let negative = unique_prefix("negative");
        bind_cache_backend_generation_aliases(&canonical, &[&data, &negative]).unwrap();

        let data_backend = service
            .wrap_generation_aware_backend(
                &data,
                service.memory_backend(Duration::from_secs(60), 16),
            )
            .await;
        let negative_backend = service
            .wrap_generation_aware_backend(
                &negative,
                service.memory_backend(Duration::from_secs(60), 16),
            )
            .await;
        data_backend
            .set("key".to_string(), b"data".to_vec())
            .await
            .unwrap();
        negative_backend
            .set("key".to_string(), b"negative".to_vec())
            .await
            .unwrap();

        observe_cache_backend_generation(&canonical, 1).unwrap();
        assert_eq!(data_backend.get("key").await.unwrap(), None);
        assert_eq!(negative_backend.get("key").await.unwrap(), None);
        assert_eq!(
            cache_backend_generation_snapshot(&data).unwrap(),
            cache_backend_generation_snapshot(&negative).unwrap()
        );
    }

    #[test]
    fn alias_binding_is_atomic_when_a_later_alias_conflicts() {
        let canonical = unique_prefix("canonical-atomic");
        let new_alias = unique_prefix("new-alias-atomic");
        let conflicting_alias = unique_prefix("conflict-alias-atomic");
        cache_backend_generation_snapshot(&conflicting_alias).unwrap();

        assert!(matches!(
            bind_cache_backend_generation_aliases(
                &canonical,
                &[&new_alias, &conflicting_alias]
            ),
            Err(CacheBackendGenerationError::AliasAlreadyBound { .. })
        ));

        let registry = backend_generations()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(!registry.contains_key(&canonical));
        assert!(!registry.contains_key(&new_alias));
        assert!(registry.contains_key(&conflicting_alias));
    }

    #[test]
    fn duplicate_and_canonical_aliases_are_deduplicated() {
        let canonical = unique_prefix("canonical-deduplicated");
        let alias = unique_prefix("alias-deduplicated");

        bind_cache_backend_generation_aliases(
            &canonical,
            &[&canonical, &alias, &alias, &canonical],
        )
        .unwrap();

        let registry = backend_generations()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(Arc::ptr_eq(
            registry.get(&canonical).unwrap(),
            registry.get(&alias).unwrap()
        ));
    }

    #[test]
    fn alias_rebinding_is_rejected_after_distinct_state_creation() {
        let canonical = unique_prefix("canonical-reject");
        let alias = unique_prefix("alias-reject");
        cache_backend_generation_snapshot(&alias).unwrap();
        assert!(matches!(
            bind_cache_backend_generation_aliases(&canonical, &[&alias]),
            Err(CacheBackendGenerationError::AliasAlreadyBound { .. })
        ));
    }

    #[tokio::test]
    async fn local_generation_bumps_are_persistent_and_monotonic() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("local-bump");
        let first = service
            .bump_cache_backend_generation(&prefix)
            .await
            .unwrap();
        let second = service
            .bump_cache_backend_generation(&prefix)
            .await
            .unwrap();
        assert_eq!(second.generation, first.generation + 1);
        assert!(second.trusted);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn invalid_config_does_not_fall_back_to_process_local_generation() {
        let service = CacheService::from_url(Some("://invalid-redis-url"));
        let prefix = unique_prefix("invalid-config");
        assert!(matches!(
            service.bump_cache_backend_generation(&prefix).await,
            Err(CacheBackendGenerationError::RedisClientUnavailable)
        ));

        let raw = service.memory_backend(Duration::from_secs(60), 16);
        let _backend = service.wrap_generation_aware_backend(&prefix, raw).await;
        assert!(!cache_backend_generation_snapshot(&prefix).unwrap().trusted);
    }

    #[tokio::test]
    async fn state_seeded_before_backend_creation_is_reused() {
        let service = CacheService::from_url(None);
        let prefix = unique_prefix("preseed");
        observe_cache_backend_generation(&prefix, 7).unwrap();
        let raw = service.memory_backend(Duration::from_secs(60), 16);
        let backend = service.wrap_generation_aware_backend(&prefix, raw).await;

        backend
            .set("key".to_string(), b"value".to_vec())
            .await
            .unwrap();
        assert_eq!(backend.get("key").await.unwrap(), Some(b"value".to_vec()));
        assert_eq!(
            cache_backend_generation_snapshot(&prefix)
                .unwrap()
                .generation,
            7
        );
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

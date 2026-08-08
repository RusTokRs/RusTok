use std::collections::HashMap;
use std::error::Error;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_pages::{
    PAGES_STOREFRONT_CACHE_TTL_SECS, PageCacheError, PageCacheGenerationSnapshot,
    PagesCacheReadPort, PagesCacheReadRuntime, storefront_pages_cache_key,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct StorefrontSnapshot {
    selected_slug: String,
    page_count: u64,
    source_revision: u64,
}

impl StorefrontSnapshot {
    fn new(source_revision: u64) -> Self {
        Self {
            selected_slug: "home".to_string(),
            page_count: 3,
            source_revision,
        }
    }
}

#[derive(Default)]
struct CacheState {
    generations: PageCacheGenerationSnapshot,
    values: HashMap<String, Vec<u8>>,
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
    generation_error: bool,
    get_error: bool,
}

struct RecordingCachePort {
    state: Mutex<CacheState>,
}

impl RecordingCachePort {
    fn new(generations: PageCacheGenerationSnapshot) -> Self {
        Self {
            state: Mutex::new(CacheState {
                generations,
                ..CacheState::default()
            }),
        }
    }

    fn set_generations(&self, generations: PageCacheGenerationSnapshot) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations = generations;
    }

    fn set_get_error(&self, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_error = enabled;
    }

    fn set_generation_error(&self, enabled: bool) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation_error = enabled;
    }

    fn snapshot(&self) -> CacheSnapshot {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        CacheSnapshot {
            generations: state.generations,
            keys: state.values.keys().cloned().collect(),
            generation_reads: state.generation_reads,
            get_keys: state.get_keys.clone(),
            put_keys: state.put_keys.clone(),
            put_ttls: state.put_ttls.clone(),
        }
    }
}

struct CacheSnapshot {
    generations: PageCacheGenerationSnapshot,
    keys: Vec<String>,
    generation_reads: usize,
    get_keys: Vec<String>,
    put_keys: Vec<String>,
    put_ttls: Vec<Duration>,
}

#[async_trait]
impl PagesCacheReadPort for RecordingCachePort {
    async fn generation_snapshot(
        &self,
        _tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.generation_reads += 1;
        if state.generation_error {
            return Err(PageCacheError::Provider(
                "injected storefront generation failure".to_string(),
            ));
        }
        Ok(state.generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.get_keys.push(key.to_string());
        if state.get_error {
            return Err(PageCacheError::Provider(
                "injected storefront cache read failure".to_string(),
            ));
        }
        Ok(state.values.get(key).cloned())
    }

    async fn put(
        &self,
        key: String,
        value: Vec<u8>,
        ttl: Duration,
    ) -> Result<(), PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.put_keys.push(key.clone());
        state.put_ttls.push(ttl);
        state.values.insert(key, value);
        Ok(())
    }
}

async fn load_native_storefront_contract<Load, LoadFuture>(
    cache_runtime: Option<&PagesCacheReadRuntime>,
    tenant_id: Uuid,
    cache_variant: &str,
    load_source: Load,
) -> TestResult<StorefrontSnapshot>
where
    Load: FnOnce() -> LoadFuture,
    LoadFuture: Future<Output = TestResult<StorefrontSnapshot>>,
{
    let cache_key = if let Some(cache_runtime) = cache_runtime {
        match cache_runtime.generation_snapshot(tenant_id).await {
            Ok(generations) => {
                storefront_pages_cache_key(tenant_id, generations, cache_variant).ok()
            }
            Err(_) => None,
        }
    } else {
        None
    };

    if let (Some(cache_runtime), Some(cache_key)) = (cache_runtime, cache_key.as_ref())
        && let Ok(Some(cached)) = cache_runtime
            .get_json::<StorefrontSnapshot>(cache_key)
            .await
    {
        return Ok(cached);
    }

    let data = load_source().await?;
    if let (Some(cache_runtime), Some(cache_key)) = (cache_runtime, cache_key) {
        let _ = cache_runtime.put_json(cache_key, &data).await;
    }
    Ok(data)
}

#[tokio::test]
async fn native_storefront_cache_misses_refills_hits_rotates_and_fails_open() -> TestResult<()> {
    let tenant_id = Uuid::from_u128(9001);
    let cache_variant = serde_json::to_string(&("home", "en", "en", "web"))?;
    let cache = Arc::new(RecordingCachePort::new(
        PageCacheGenerationSnapshot::new(3, 5, 7),
    ));
    let cache_port: Arc<dyn PagesCacheReadPort> = cache.clone();
    let runtime = PagesCacheReadRuntime::new(cache_port);
    let source_calls = Arc::new(AtomicUsize::new(0));

    let first = load_native_storefront_contract(Some(&runtime), tenant_id, &cache_variant, {
        let source_calls = source_calls.clone();
        move || async move {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StorefrontSnapshot::new(1))
        }
    })
    .await?;
    assert_eq!(first, StorefrontSnapshot::new(1));
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);

    let first_cache = cache.snapshot();
    assert_eq!(first_cache.generations, PageCacheGenerationSnapshot::new(3, 5, 7));
    assert_eq!(first_cache.get_keys.len(), 1);
    assert_eq!(first_cache.put_keys.len(), 1);
    assert_eq!(first_cache.get_keys[0], first_cache.put_keys[0]);
    let old_generation_key = first_cache.put_keys[0].clone();

    let second = load_native_storefront_contract(Some(&runtime), tenant_id, &cache_variant, {
        let source_calls = source_calls.clone();
        move || async move {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StorefrontSnapshot::new(99))
        }
    })
    .await?;
    assert_eq!(second, StorefrontSnapshot::new(1));
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(cache.snapshot().put_keys.len(), 1);

    cache.set_generations(PageCacheGenerationSnapshot::new(4, 6, 8));
    let third = load_native_storefront_contract(Some(&runtime), tenant_id, &cache_variant, {
        let source_calls = source_calls.clone();
        move || async move {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StorefrontSnapshot::new(2))
        }
    })
    .await?;
    assert_eq!(third, StorefrontSnapshot::new(2));
    assert_eq!(source_calls.load(Ordering::SeqCst), 2);

    let rotated_cache = cache.snapshot();
    assert_eq!(rotated_cache.put_keys.len(), 2);
    let new_generation_key = rotated_cache.put_keys[1].clone();
    assert_ne!(new_generation_key, old_generation_key);
    assert!(rotated_cache.keys.contains(&old_generation_key));
    assert!(rotated_cache.keys.contains(&new_generation_key));

    cache.set_generations(PageCacheGenerationSnapshot::new(5, 7, 9));
    cache.set_get_error(true);
    let fourth = load_native_storefront_contract(Some(&runtime), tenant_id, &cache_variant, {
        let source_calls = source_calls.clone();
        move || async move {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StorefrontSnapshot::new(3))
        }
    })
    .await?;
    assert_eq!(fourth, StorefrontSnapshot::new(3));
    assert_eq!(source_calls.load(Ordering::SeqCst), 3);
    let read_failure_cache = cache.snapshot();
    assert_eq!(read_failure_cache.put_keys.len(), 3);
    assert_eq!(read_failure_cache.put_ttls.len(), 3);
    assert!(
        read_failure_cache
            .put_ttls
            .iter()
            .all(|ttl| *ttl == Duration::from_secs(PAGES_STOREFRONT_CACHE_TTL_SECS))
    );

    cache.set_get_error(false);
    cache.set_generation_error(true);
    let before_generation_failure = cache.snapshot();
    let fifth = load_native_storefront_contract(Some(&runtime), tenant_id, &cache_variant, {
        let source_calls = source_calls.clone();
        move || async move {
            source_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StorefrontSnapshot::new(4))
        }
    })
    .await?;
    assert_eq!(fifth, StorefrontSnapshot::new(4));
    assert_eq!(source_calls.load(Ordering::SeqCst), 4);

    let after_generation_failure = cache.snapshot();
    assert_eq!(
        after_generation_failure.generation_reads,
        before_generation_failure.generation_reads + 1
    );
    assert_eq!(
        after_generation_failure.get_keys,
        before_generation_failure.get_keys
    );
    assert_eq!(
        after_generation_failure.put_keys,
        before_generation_failure.put_keys
    );
    Ok(())
}

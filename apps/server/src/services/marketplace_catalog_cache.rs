use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use moka::Expiry;
use moka::future::Cache;
use rustok_core::ModuleRegistry;
use sha2::{Digest, Sha256};

use crate::modules::{CatalogManifestModule, ModulesManifest};
use crate::services::marketplace_catalog::{MarketplaceCatalogProvider, MarketplaceCatalogQuery};

mod base {
    include!("marketplace_catalog_cache_base.rs");
}

const DEFAULT_REGISTRY_DETAIL_CACHE_MAX_WEIGHT_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_REGISTRY_DETAIL_NEGATIVE_TTL_SECS: u64 = 5;
const MAX_REGISTRY_DETAIL_CACHE_KEY_BYTES: usize = 4 * 1024;
const REGISTRY_DETAIL_CACHE_KEY_PREFIX: &str = "registry-module:v1";

#[derive(Clone)]
struct CachedRegistryModule {
    module: Option<CatalogManifestModule>,
    estimated_bytes: usize,
}

impl CachedRegistryModule {
    fn new(module: Option<CatalogManifestModule>) -> Self {
        let estimated_bytes = module
            .as_ref()
            .map(estimate_catalog_module_bytes)
            .unwrap_or(1);
        Self {
            module,
            estimated_bytes,
        }
    }
}

struct RegistryModuleExpiry {
    positive_ttl: Duration,
    negative_ttl: Duration,
}

impl Expiry<String, Arc<CachedRegistryModule>> for RegistryModuleExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &Arc<CachedRegistryModule>,
        _created_at: Instant,
    ) -> Option<Duration> {
        Some(if value.module.is_some() {
            self.positive_ttl
        } else {
            self.negative_ttl
        })
    }

    fn expire_after_update(
        &self,
        _key: &String,
        value: &Arc<CachedRegistryModule>,
        _updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        self.expire_after_create(_key, value, _updated_at)
    }
}

fn cached_registry_module_weight(key: &str, value: &Arc<CachedRegistryModule>) -> u32 {
    key.len()
        .saturating_add(value.estimated_bytes)
        .saturating_add(std::mem::size_of::<CachedRegistryModule>())
        .clamp(1, u32::MAX as usize) as u32
}

/// Registry provider that preserves the hardened catalog implementation while
/// adding a bounded, single-flight cache for module-detail requests.
pub struct HardenedRegistryMarketplaceProvider {
    inner: Arc<base::HardenedRegistryMarketplaceProvider>,
    module_cache: Cache<String, Arc<CachedRegistryModule>>,
}

impl HardenedRegistryMarketplaceProvider {
    pub fn from_env() -> Self {
        let positive_ttl = Duration::from_secs(positive_env_u64(
            "RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS",
            60,
        ));
        let negative_ttl = Duration::from_secs(
            positive_env_u64(
                "RUSTOK_MARKETPLACE_REGISTRY_DETAIL_NEGATIVE_TTL_SECS",
                DEFAULT_REGISTRY_DETAIL_NEGATIVE_TTL_SECS,
            )
            .min(positive_ttl.as_secs().max(1)),
        );
        let maximum_weight = positive_env_u64(
            "RUSTOK_MARKETPLACE_REGISTRY_DETAIL_CACHE_MAX_BYTES",
            DEFAULT_REGISTRY_DETAIL_CACHE_MAX_WEIGHT_BYTES,
        );
        Self::with_inner(
            base::HardenedRegistryMarketplaceProvider::from_env(),
            positive_ttl,
            negative_ttl,
            maximum_weight,
        )
    }

    fn with_inner(
        inner: base::HardenedRegistryMarketplaceProvider,
        positive_ttl: Duration,
        negative_ttl: Duration,
        maximum_weight: u64,
    ) -> Self {
        let positive_ttl = positive_ttl.max(Duration::from_millis(1));
        let negative_ttl = negative_ttl.max(Duration::from_millis(1)).min(positive_ttl);
        let module_cache = Cache::<String, Arc<CachedRegistryModule>>::builder()
            .weigher(|key, value| cached_registry_module_weight(key, value))
            .max_capacity(maximum_weight.max(1))
            .expire_after(RegistryModuleExpiry {
                positive_ttl,
                negative_ttl,
            })
            .build();
        Self {
            inner: Arc::new(inner),
            module_cache,
        }
    }
}

#[async_trait]
impl MarketplaceCatalogProvider for HardenedRegistryMarketplaceProvider {
    fn provider_key(&self) -> &'static str {
        self.inner.provider_key()
    }

    async fn list_modules(
        &self,
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
        query: &MarketplaceCatalogQuery,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        self.inner.list_modules(manifest, registry, query).await
    }

    async fn get_module(
        &self,
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
        query: &MarketplaceCatalogQuery,
        slug: &str,
    ) -> anyhow::Result<Option<CatalogManifestModule>> {
        let cache_key = registry_module_cache_key(slug)?;
        let inner = Arc::clone(&self.inner);
        let manifest = manifest.clone();
        let registry = registry.clone();
        let query = query.clone();
        let slug = slug.trim().to_string();
        let cached = load_module_detail(&self.module_cache, cache_key, move || async move {
            inner.get_module(&manifest, &registry, &query, &slug).await
        })
        .await?;
        Ok(cached.module.clone())
    }
}

async fn load_module_detail<F, Fut>(
    cache: &Cache<String, Arc<CachedRegistryModule>>,
    cache_key: String,
    loader: F,
) -> anyhow::Result<Arc<CachedRegistryModule>>
where
    F: FnOnce() -> Fut + Send,
    Fut: Future<Output = anyhow::Result<Option<CatalogManifestModule>>> + Send,
{
    cache
        .try_get_with(cache_key, async move {
            loader()
                .await
                .map(|module| Arc::new(CachedRegistryModule::new(module)))
        })
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

fn registry_module_cache_key(slug: &str) -> anyhow::Result<String> {
    let slug = slug.trim();
    if slug.is_empty() {
        anyhow::bail!("marketplace registry module slug must not be empty");
    }
    if slug.len() > MAX_REGISTRY_DETAIL_CACHE_KEY_BYTES {
        anyhow::bail!(
            "marketplace registry module slug is {} bytes; maximum is {}",
            slug.len(),
            MAX_REGISTRY_DETAIL_CACHE_KEY_BYTES
        );
    }

    let mut digest = Sha256::new();
    digest.update(REGISTRY_DETAIL_CACHE_KEY_PREFIX.as_bytes());
    digest.update((slug.len() as u64).to_be_bytes());
    digest.update(slug.as_bytes());
    Ok(format!(
        "{REGISTRY_DETAIL_CACHE_KEY_PREFIX}:{}",
        hex::encode(digest.finalize())
    ))
}

fn estimate_catalog_module_bytes(module: &CatalogManifestModule) -> usize {
    let mut bytes = std::mem::size_of::<CatalogManifestModule>();
    for value in [
        Some(module.slug.as_str()),
        Some(module.source.as_str()),
        Some(module.crate_name.as_str()),
        module.name.as_deref(),
        module.category.as_deref(),
        module.icon_url.as_deref(),
        module.banner_url.as_deref(),
        module.version.as_deref(),
        module.description.as_deref(),
        module.git.as_deref(),
        module.rev.as_deref(),
        module.path.as_deref(),
        Some(module.ownership.as_str()),
        Some(module.trust_level.as_str()),
        module.rustok_min_version.as_deref(),
        module.rustok_max_version.as_deref(),
        module.publisher.as_deref(),
        module.checksum_sha256.as_deref(),
        module.signature.as_deref(),
        Some(module.ui_classification.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        bytes = bytes.saturating_add(value.len());
    }
    for values in [
        &module.tags,
        &module.screenshots,
        &module.depends_on,
        &module.recommended_admin_surfaces,
        &module.showcase_admin_surfaces,
    ] {
        for value in values {
            bytes = bytes.saturating_add(value.len());
        }
    }
    for version in &module.versions {
        bytes = bytes.saturating_add(version.version.len());
        for value in [
            version.changelog.as_deref(),
            version.published_at.as_deref(),
            version.checksum_sha256.as_deref(),
            version.signature.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            bytes = bytes.saturating_add(value.len());
        }
    }
    bytes.saturating_add(
        serde_json::to_vec(&module.settings_schema)
            .map(|encoded| encoded.len())
            .unwrap_or(0),
    )
}

fn positive_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod wrapper_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn detail_cache(
        positive_ttl: Duration,
        negative_ttl: Duration,
    ) -> Cache<String, Arc<CachedRegistryModule>> {
        Cache::<String, Arc<CachedRegistryModule>>::builder()
            .weigher(|key, value| cached_registry_module_weight(key, value))
            .max_capacity(1024 * 1024)
            .expire_after(RegistryModuleExpiry {
                positive_ttl,
                negative_ttl,
            })
            .build()
    }

    #[test]
    fn detail_cache_key_is_bounded_and_hashed() {
        let key = registry_module_cache_key("private-module").unwrap();
        assert!(key.starts_with("registry-module:v1:"));
        assert_eq!(key.len(), REGISTRY_DETAIL_CACHE_KEY_PREFIX.len() + 1 + 64);
        assert!(!key.contains("private-module"));
        assert!(registry_module_cache_key("").is_err());
        assert!(
            registry_module_cache_key(&"x".repeat(MAX_REGISTRY_DETAIL_CACHE_KEY_BYTES + 1))
                .is_err()
        );
    }

    #[tokio::test]
    async fn concurrent_module_detail_misses_are_single_flight() {
        let cache = Arc::new(detail_cache(
            Duration::from_secs(60),
            Duration::from_secs(5),
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let key = registry_module_cache_key("module-a").unwrap();

        let first = {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            let key = key.clone();
            tokio::spawn(async move {
                load_module_detail(&cache, key, move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    Ok(None)
                })
                .await
                .unwrap()
            })
        };
        let second = {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            tokio::spawn(async move {
                load_module_detail(&cache, key, move || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(None)
                })
                .await
                .unwrap()
            })
        };

        let (first, second) = tokio::join!(first, second);
        assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn negative_module_details_expire_quickly() {
        let cache = detail_cache(Duration::from_secs(60), Duration::from_millis(10));
        let calls = Arc::new(AtomicUsize::new(0));
        let key = registry_module_cache_key("missing").unwrap();

        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            load_module_detail(&cache, key.clone(), move || async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            })
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(25)).await;
            cache.run_pending_tasks().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

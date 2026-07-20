use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use futures_util::StreamExt;
use moka::future::Cache;
use reqwest::{Client, Response};
use rustok_core::ModuleRegistry;
use semver::Version;
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::modules::{CatalogManifestModule, CatalogModuleVersion, ModulesManifest};
use crate::services::marketplace_catalog::{
    registry_catalog_module_path, registry_catalog_path, MarketplaceCatalogProvider,
    MarketplaceCatalogQuery, RegistryCatalogModule, RegistryCatalogResponse,
    RegistryCatalogVersion, REGISTRY_CATALOG_SCHEMA_VERSION,
};

const DEFAULT_REGISTRY_TIMEOUT_MS: u64 = 3_000;
const DEFAULT_REGISTRY_CACHE_TTL_SECS: u64 = 60;
const DEFAULT_REGISTRY_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_REGISTRY_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_REGISTRY_MAX_CONCURRENT_FETCHES: usize = 16;
const MAX_REGISTRY_CACHE_KEY_COMPONENT_BYTES: usize = 4 * 1024;
const REGISTRY_CACHE_KEY_PREFIX: &str = "registry-catalog:v1";

#[derive(Clone)]
struct CachedRegistryCatalog {
    modules: Arc<Vec<CatalogManifestModule>>,
    encoded_bytes: usize,
}

impl CachedRegistryCatalog {
    fn new(modules: Vec<CatalogManifestModule>, encoded_bytes: usize) -> Self {
        Self {
            modules: Arc::new(modules),
            encoded_bytes,
        }
    }
}

fn cached_registry_catalog_weight(key: &str, value: &Arc<CachedRegistryCatalog>) -> u32 {
    key.len()
        .saturating_add(value.encoded_bytes)
        .saturating_add(std::mem::size_of::<CachedRegistryCatalog>())
        .clamp(1, u32::MAX as usize) as u32
}

/// Registry-backed marketplace provider with bounded keys, response bodies and cache weight.
pub struct HardenedRegistryMarketplaceProvider {
    registry_url: Option<String>,
    client: Client,
    catalog_cache: Cache<String, Arc<CachedRegistryCatalog>>,
    max_response_bytes: usize,
    fetch_permits: Arc<Semaphore>,
    saturated_fetches: Arc<AtomicU64>,
}

impl HardenedRegistryMarketplaceProvider {
    pub fn from_env() -> Self {
        let registry_url = std::env::var("RUSTOK_MARKETPLACE_REGISTRY_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let timeout_ms = positive_env_u64(
            "RUSTOK_MARKETPLACE_REGISTRY_TIMEOUT_MS",
            DEFAULT_REGISTRY_TIMEOUT_MS,
        );
        let cache_ttl_secs = positive_env_u64(
            "RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS",
            DEFAULT_REGISTRY_CACHE_TTL_SECS,
        );
        let cache_max_weight_bytes = positive_env_u64(
            "RUSTOK_MARKETPLACE_REGISTRY_CACHE_MAX_BYTES",
            DEFAULT_REGISTRY_CACHE_MAX_WEIGHT_BYTES,
        );
        let max_response_bytes = positive_env_usize(
            "RUSTOK_MARKETPLACE_REGISTRY_RESPONSE_MAX_BYTES",
            DEFAULT_REGISTRY_RESPONSE_MAX_BYTES,
        );
        let max_concurrent_fetches = positive_env_usize(
            "RUSTOK_MARKETPLACE_REGISTRY_MAX_CONCURRENT_FETCHES",
            DEFAULT_REGISTRY_MAX_CONCURRENT_FETCHES,
        );
        let client = Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|error| {
                panic!("failed to construct bounded marketplace registry client: {error}")
            });

        Self::new(
            registry_url,
            client,
            Duration::from_secs(cache_ttl_secs),
            cache_max_weight_bytes,
            max_response_bytes,
            max_concurrent_fetches,
        )
    }

    fn new(
        registry_url: Option<String>,
        client: Client,
        cache_ttl: Duration,
        cache_max_weight_bytes: u64,
        max_response_bytes: usize,
        max_concurrent_fetches: usize,
    ) -> Self {
        let catalog_cache = Cache::<String, Arc<CachedRegistryCatalog>>::builder()
            .weigher(|key, value| cached_registry_catalog_weight(key, value))
            .max_capacity(cache_max_weight_bytes.max(1))
            .time_to_live(cache_ttl.max(Duration::from_millis(1)))
            .build();
        Self {
            registry_url,
            client,
            catalog_cache,
            max_response_bytes: max_response_bytes.max(1),
            fetch_permits: Arc::new(Semaphore::new(max_concurrent_fetches.max(1))),
            saturated_fetches: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn load_catalog(
        &self,
        registry_url: &str,
        query: &MarketplaceCatalogQuery,
    ) -> anyhow::Result<Arc<CachedRegistryCatalog>> {
        let cache_key = registry_catalog_cache_key(registry_url, query)?;
        let client = self.client.clone();
        let registry_url = registry_url.to_string();
        let query = query.clone();
        let max_response_bytes = self.max_response_bytes;
        let fetch_permits = Arc::clone(&self.fetch_permits);
        let saturated_fetches = Arc::clone(&self.saturated_fetches);

        self.catalog_cache
            .try_get_with(cache_key, async move {
                let _permit = try_acquire_fetch_permit(fetch_permits, &saturated_fetches)?;
                fetch_catalog(&client, &registry_url, &query, max_response_bytes).await
            })
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    async fn fetch_module(
        &self,
        registry_url: &str,
        slug: &str,
    ) -> anyhow::Result<Option<CatalogManifestModule>> {
        validate_cache_key_component("registry_url", registry_url)?;
        validate_cache_key_component("slug", slug)?;

        let _permit =
            try_acquire_fetch_permit(Arc::clone(&self.fetch_permits), &self.saturated_fetches)?;
        fetch_module_from_path(
            &self.client,
            registry_url,
            registry_catalog_module_path(),
            slug,
            self.max_response_bytes,
        )
        .await
        .map(Some)
    }

    #[cfg(test)]
    fn saturated_fetches(&self) -> u64 {
        self.saturated_fetches.load(AtomicOrdering::Relaxed)
    }
}

#[async_trait]
impl MarketplaceCatalogProvider for HardenedRegistryMarketplaceProvider {
    fn provider_key(&self) -> &'static str {
        "registry"
    }

    async fn list_modules(
        &self,
        _manifest: &ModulesManifest,
        _registry: &ModuleRegistry,
        query: &MarketplaceCatalogQuery,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let Some(registry_url) = &self.registry_url else {
            return Ok(Vec::new());
        };

        match self.load_catalog(registry_url, query).await {
            Ok(catalog) => Ok(catalog.modules.as_ref().clone()),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Bounded marketplace registry fetch failed; falling back to local catalog only"
                );
                Ok(Vec::new())
            }
        }
    }

    async fn get_module(
        &self,
        _manifest: &ModulesManifest,
        _registry: &ModuleRegistry,
        _query: &MarketplaceCatalogQuery,
        slug: &str,
    ) -> anyhow::Result<Option<CatalogManifestModule>> {
        let Some(registry_url) = &self.registry_url else {
            return Ok(None);
        };

        match self.fetch_module(registry_url, slug).await {
            Ok(module) => Ok(module),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Bounded marketplace registry detail fetch failed; falling back to local catalog only"
                );
                Ok(None)
            }
        }
    }
}

async fn fetch_catalog(
    client: &Client,
    registry_url: &str,
    query: &MarketplaceCatalogQuery,
    max_response_bytes: usize,
) -> anyhow::Result<Arc<CachedRegistryCatalog>> {
    let (payload, encoded_bytes) = fetch_catalog_from_path(
        client,
        registry_url,
        registry_catalog_path(),
        query,
        max_response_bytes,
    )
    .await?;

    validate_registry_schema_version(payload.schema_version)?;
    let modules = payload
        .modules
        .into_iter()
        .map(registry_module_into_catalog)
        .collect();
    Ok(Arc::new(CachedRegistryCatalog::new(modules, encoded_bytes)))
}

async fn fetch_catalog_from_path(
    client: &Client,
    registry_url: &str,
    path: &str,
    query: &MarketplaceCatalogQuery,
    max_response_bytes: usize,
) -> anyhow::Result<(RegistryCatalogResponse, usize)> {
    let endpoint = format!("{}{}", registry_url.trim_end_matches('/'), path);
    let mut request = client.get(endpoint);
    if let Some(search) = normalized(&query.search) {
        request = request.query(&[("search", search)]);
    }
    if let Some(category) = normalized(&query.category) {
        request = request.query(&[("category", category)]);
    }
    if let Some(tag) = normalized(&query.tag) {
        request = request.query(&[("tag", tag)]);
    }
    let response = request.send().await?.error_for_status()?;
    let bytes = read_bounded_response(response, max_response_bytes).await?;
    let encoded_bytes = bytes.len();
    let payload = serde_json::from_slice::<RegistryCatalogResponse>(&bytes)?;
    Ok((payload, encoded_bytes))
}

async fn fetch_module_from_path(
    client: &Client,
    registry_url: &str,
    path: &str,
    slug: &str,
    max_response_bytes: usize,
) -> anyhow::Result<CatalogManifestModule> {
    let endpoint = format!(
        "{}{}",
        registry_url.trim_end_matches('/'),
        path.replace("{slug}", slug)
    );
    let response = client.get(endpoint).send().await?.error_for_status()?;
    let bytes = read_bounded_response(response, max_response_bytes).await?;
    let module = serde_json::from_slice::<RegistryCatalogModule>(&bytes)?;
    Ok(registry_module_into_catalog(module))
}

async fn read_bounded_response(response: Response, maximum: usize) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum as u64)
    {
        anyhow::bail!("marketplace registry response exceeds {maximum} bytes");
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let next_len = bytes
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| anyhow::anyhow!("marketplace registry response length overflow"))?;
        if next_len > maximum {
            anyhow::bail!("marketplace registry response exceeds {maximum} bytes");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn try_acquire_fetch_permit(
    permits: Arc<Semaphore>,
    saturated_fetches: &AtomicU64,
) -> anyhow::Result<OwnedSemaphorePermit> {
    permits.try_acquire_owned().map_err(|_| {
        saturated_fetches.fetch_add(1, AtomicOrdering::Relaxed);
        anyhow::anyhow!("marketplace registry fetch capacity is saturated")
    })
}

fn registry_catalog_cache_key(
    registry_url: &str,
    query: &MarketplaceCatalogQuery,
) -> anyhow::Result<String> {
    let registry_url = registry_url.trim().trim_end_matches('/');
    let search = normalized(&query.search).unwrap_or_default();
    let category = normalized(&query.category).unwrap_or_default();
    let tag = normalized(&query.tag).unwrap_or_default();
    for (name, value) in [
        ("registry_url", registry_url),
        ("search", search),
        ("category", category),
        ("tag", tag),
    ] {
        validate_cache_key_component(name, value)?;
    }

    let mut digest = Sha256::new();
    digest.update(REGISTRY_CACHE_KEY_PREFIX.as_bytes());
    for value in [registry_url, search, category, tag] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    Ok(format!(
        "{REGISTRY_CACHE_KEY_PREFIX}:{}",
        hex::encode(digest.finalize())
    ))
}

fn validate_cache_key_component(name: &str, value: &str) -> anyhow::Result<()> {
    if value.len() > MAX_REGISTRY_CACHE_KEY_COMPONENT_BYTES {
        anyhow::bail!(
            "marketplace registry {name} is {} bytes; maximum is {}",
            value.len(),
            MAX_REGISTRY_CACHE_KEY_COMPONENT_BYTES
        );
    }
    Ok(())
}

fn normalized(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn positive_env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn positive_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn validate_registry_schema_version(schema_version: u32) -> anyhow::Result<()> {
    if schema_version == REGISTRY_CATALOG_SCHEMA_VERSION {
        Ok(())
    } else {
        anyhow::bail!(
            "Unsupported registry catalog schema_version={schema_version}; expected {}",
            REGISTRY_CATALOG_SCHEMA_VERSION
        )
    }
}

fn registry_module_into_catalog(module: RegistryCatalogModule) -> CatalogManifestModule {
    let versions = normalize_registry_versions(
        module
            .versions
            .into_iter()
            .map(registry_version_into_catalog)
            .collect(),
    );
    CatalogManifestModule {
        slug: module.slug,
        source: module.source,
        crate_name: module.crate_name,
        name: module.name,
        category: module.category,
        tags: module.tags,
        icon_url: module.icon_url,
        banner_url: module.banner_url,
        screenshots: module.screenshots,
        version: module.version,
        description: module.description,
        git: module.git,
        rev: module.rev,
        path: module.path,
        required: module.required,
        depends_on: module.depends_on,
        ownership: module.ownership,
        trust_level: module.trust_level,
        rustok_min_version: module.rustok_min_version,
        rustok_max_version: module.rustok_max_version,
        publisher: normalize_optional_publisher(module.publisher),
        checksum_sha256: normalize_optional_checksum(module.checksum_sha256),
        signature: module.signature,
        versions,
        has_admin_ui: false,
        has_storefront_ui: false,
        ui_classification: "no-ui".to_string(),
        recommended_admin_surfaces: module.recommended_admin_surfaces,
        showcase_admin_surfaces: module.showcase_admin_surfaces,
        settings_schema: module.settings_schema,
    }
}

fn registry_version_into_catalog(version: RegistryCatalogVersion) -> CatalogModuleVersion {
    CatalogModuleVersion {
        version: version.version,
        changelog: version.changelog,
        yanked: version.yanked,
        published_at: normalize_optional_published_at(version.published_at),
        checksum_sha256: normalize_optional_checksum(version.checksum_sha256),
        signature: version.signature,
    }
}

fn normalize_optional_publisher(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_optional_checksum(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_ascii_lowercase();
    (value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())).then_some(value)
}

fn normalize_optional_published_at(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(&value).ok().map(|value| {
        value
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    })
}

fn normalize_registry_versions(
    mut versions: Vec<CatalogModuleVersion>,
) -> Vec<CatalogModuleVersion> {
    for version in &mut versions {
        version.published_at = normalize_optional_published_at(version.published_at.take());
        version.checksum_sha256 = normalize_optional_checksum(version.checksum_sha256.take());
    }
    versions.sort_by(compare_registry_versions);
    versions
}

fn compare_registry_versions(
    left: &CatalogModuleVersion,
    right: &CatalogModuleVersion,
) -> Ordering {
    left.yanked
        .cmp(&right.yanked)
        .then_with(|| compare_registry_semver_desc(&left.version, &right.version))
        .then_with(|| right.published_at.cmp(&left.published_at))
        .then_with(|| right.version.cmp(&left.version))
}

fn compare_registry_semver_desc(left: &str, right: &str) -> Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::{Json, Router};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tokio::net::TcpListener;
    use tokio::sync::{oneshot, Notify};

    fn query(search: &str, category: &str, tag: &str) -> MarketplaceCatalogQuery {
        MarketplaceCatalogQuery {
            search: Some(search.to_string()),
            category: Some(category.to_string()),
            tag: Some(tag.to_string()),
        }
    }

    #[test]
    fn cache_key_is_bounded_hashed_and_length_delimited() {
        let registry_url = "https://registry.example.test/private/path";
        let first =
            registry_catalog_cache_key(registry_url, &query("a|category=b", "c", "d")).unwrap();
        let second =
            registry_catalog_cache_key(registry_url, &query("a", "b|category=c", "d")).unwrap();

        assert_ne!(first, second);
        assert!(first.starts_with("registry-catalog:v1:"));
        assert_eq!(first.len(), REGISTRY_CACHE_KEY_PREFIX.len() + 1 + 64);
        assert!(!first.contains("registry.example"));
        assert!(!first.contains("category"));
    }

    #[test]
    fn oversized_cache_key_component_is_rejected_before_hashing() {
        let error = registry_catalog_cache_key(
            "https://registry.example.test",
            &MarketplaceCatalogQuery {
                search: Some("x".repeat(MAX_REGISTRY_CACHE_KEY_COMPONENT_BYTES + 1)),
                ..MarketplaceCatalogQuery::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("maximum"));
    }

    #[test]
    fn cache_weight_tracks_payload_bytes() {
        let small = Arc::new(CachedRegistryCatalog::new(Vec::new(), 32));
        let large = Arc::new(CachedRegistryCatalog::new(Vec::new(), 4_096));
        assert!(
            cached_registry_catalog_weight("key", &large)
                > cached_registry_catalog_weight("key", &small)
        );
    }

    #[tokio::test]
    async fn concurrent_catalog_misses_are_single_flight() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let app = Router::new().route(
            registry_catalog_path(),
            get(move || {
                let handler_calls = Arc::clone(&handler_calls);
                async move {
                    handler_calls.fetch_add(1, AtomicOrdering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Json(RegistryCatalogResponse {
                        schema_version: REGISTRY_CATALOG_SCHEMA_VERSION,
                        modules: Vec::new(),
                    })
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let provider = Arc::new(HardenedRegistryMarketplaceProvider::new(
            Some(format!("http://{address}")),
            Client::new(),
            Duration::from_secs(60),
            1024 * 1024,
            1024 * 1024,
            8,
        ));
        let first = {
            let provider = Arc::clone(&provider);
            tokio::spawn(async move {
                provider
                    .load_catalog(
                        provider.registry_url.as_deref().unwrap(),
                        &MarketplaceCatalogQuery::default(),
                    )
                    .await
                    .unwrap()
            })
        };
        let second = {
            let provider = Arc::clone(&provider);
            tokio::spawn(async move {
                provider
                    .load_catalog(
                        provider.registry_url.as_deref().unwrap(),
                        &MarketplaceCatalogQuery::default(),
                    )
                    .await
                    .unwrap()
            })
        };

        let (first, second) = tokio::join!(first, second);
        assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn distinct_catalog_misses_respect_the_global_fetch_budget() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let started = Arc::new(Notify::new());
        let handler_started = Arc::clone(&started);
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let release = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
        let handler_release = Arc::clone(&release);
        let app = Router::new().route(
            registry_catalog_path(),
            get(move || {
                let handler_calls = Arc::clone(&handler_calls);
                let handler_started = Arc::clone(&handler_started);
                let handler_release = Arc::clone(&handler_release);
                async move {
                    handler_calls.fetch_add(1, AtomicOrdering::SeqCst);
                    handler_started.notify_one();
                    if let Some(release_rx) = handler_release.lock().await.take() {
                        let _ = release_rx.await;
                    }
                    Json(RegistryCatalogResponse {
                        schema_version: REGISTRY_CATALOG_SCHEMA_VERSION,
                        modules: Vec::new(),
                    })
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = Arc::new(HardenedRegistryMarketplaceProvider::new(
            Some(format!("http://{address}")),
            Client::new(),
            Duration::from_secs(60),
            1024 * 1024,
            1024 * 1024,
            1,
        ));

        let first = {
            let provider = Arc::clone(&provider);
            tokio::spawn(async move {
                provider
                    .load_catalog(
                        provider.registry_url.as_deref().unwrap(),
                        &query("first", "", ""),
                    )
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(1), started.notified())
            .await
            .expect("first marketplace fetch did not start");

        let error = match provider
            .load_catalog(
                provider.registry_url.as_deref().unwrap(),
                &query("second", "", ""),
            )
            .await
        {
            Ok(_) => panic!("the saturated fetch should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("capacity is saturated"));
        assert_eq!(provider.saturated_fetches(), 1);
        assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);

        let _ = release_tx.send(());
        first.await.unwrap().unwrap();
        server.abort();
    }

    #[tokio::test]
    async fn chunked_oversized_response_is_rejected_before_json_parse() {
        let app = Router::new().route(registry_catalog_path(), get(|| async { "x".repeat(4_096) }));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let provider = HardenedRegistryMarketplaceProvider::new(
            Some(format!("http://{address}")),
            Client::new(),
            Duration::from_secs(60),
            1024 * 1024,
            128,
            8,
        );

        let error = match provider
            .load_catalog(
                provider.registry_url.as_deref().unwrap(),
                &MarketplaceCatalogQuery::default(),
            )
            .await
        {
            Ok(_) => panic!("the oversized response should fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("exceeds 128 bytes"));
        server.abort();
    }
}

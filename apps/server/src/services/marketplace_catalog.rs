use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use moka::future::Cache;
use reqwest::Client;
use rustok_core::ModuleRegistry;
use serde::Deserialize;

use crate::modules::{
    CatalogManifestModule, CatalogModuleVersion, ManifestManager, ModulesManifest,
};

const REGISTRY_CATALOG_SCHEMA_VERSION: u32 = 1;

#[async_trait]
pub trait MarketplaceCatalogProvider: Send + Sync {
    fn provider_key(&self) -> &'static str;

    async fn list_modules(
        &self,
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
    ) -> anyhow::Result<Vec<CatalogManifestModule>>;
}

pub struct LocalManifestMarketplaceProvider;

#[async_trait]
impl MarketplaceCatalogProvider for LocalManifestMarketplaceProvider {
    fn provider_key(&self) -> &'static str {
        "local-manifest"
    }

    async fn list_modules(
        &self,
        manifest: &ModulesManifest,
        _registry: &ModuleRegistry,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        ManifestManager::catalog_modules(manifest).map_err(Into::into)
    }
}

pub struct RegistryMarketplaceProvider {
    registry_url: Option<String>,
    client: Client,
    catalog_cache: Cache<String, Arc<Vec<CatalogManifestModule>>>,
}

impl RegistryMarketplaceProvider {
    pub fn from_env() -> Self {
        let registry_url = std::env::var("RUSTOK_MARKETPLACE_REGISTRY_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let timeout_ms = std::env::var("RUSTOK_MARKETPLACE_REGISTRY_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(3_000);
        let cache_ttl_secs = std::env::var("RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(60);
        let client = Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_else(|_| Client::new());
        let catalog_cache = Cache::builder()
            .max_capacity(1)
            .time_to_live(Duration::from_secs(cache_ttl_secs))
            .build();

        Self {
            registry_url,
            client,
            catalog_cache,
        }
    }

    async fn fetch_catalog(
        &self,
        registry_url: &str,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let endpoint = format!("{}/catalog", registry_url.trim_end_matches('/'));
        let response = self.client.get(&endpoint).send().await?;
        let response = response.error_for_status()?;
        let payload = response.json::<RegistryCatalogResponse>().await?;
        validate_registry_schema_version(payload.schema_version)?;

        Ok(payload
            .modules
            .into_iter()
            .map(RegistryCatalogModule::into_catalog_module)
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct RegistryCatalogResponse {
    #[serde(default = "default_registry_catalog_schema_version")]
    schema_version: u32,
    #[serde(default)]
    modules: Vec<RegistryCatalogModule>,
}

#[derive(Debug, Deserialize)]
struct RegistryCatalogModule {
    slug: String,
    #[serde(default = "default_registry_source")]
    source: String,
    #[serde(rename = "crate", alias = "crate_name")]
    crate_name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    git: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default = "default_registry_ownership")]
    ownership: String,
    #[serde(default = "default_registry_trust_level")]
    trust_level: String,
    #[serde(default)]
    rustok_min_version: Option<String>,
    #[serde(default)]
    rustok_max_version: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    checksum_sha256: Option<String>,
    #[serde(default)]
    signature: Option<String>,
    #[serde(default)]
    versions: Vec<RegistryCatalogVersion>,
    #[serde(default)]
    recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    showcase_admin_surfaces: Vec<String>,
}

impl RegistryCatalogModule {
    fn into_catalog_module(self) -> CatalogManifestModule {
        let versions = self
            .versions
            .into_iter()
            .map(RegistryCatalogVersion::into_catalog_version)
            .collect::<Vec<_>>();

        CatalogManifestModule {
            slug: self.slug,
            source: self.source,
            crate_name: self.crate_name,
            version: self.version,
            git: self.git,
            rev: self.rev,
            path: self.path,
            required: self.required,
            depends_on: self.depends_on,
            ownership: self.ownership,
            trust_level: self.trust_level,
            rustok_min_version: self.rustok_min_version,
            rustok_max_version: self.rustok_max_version,
            publisher: self.publisher,
            checksum_sha256: self.checksum_sha256,
            signature: self.signature,
            versions,
            recommended_admin_surfaces: self.recommended_admin_surfaces,
            showcase_admin_surfaces: self.showcase_admin_surfaces,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegistryCatalogVersion {
    version: String,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    yanked: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    checksum_sha256: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

impl RegistryCatalogVersion {
    fn into_catalog_version(self) -> CatalogModuleVersion {
        CatalogModuleVersion {
            version: self.version,
            changelog: self.changelog,
            yanked: self.yanked,
            published_at: self.published_at,
            checksum_sha256: self.checksum_sha256,
            signature: self.signature,
        }
    }
}

#[async_trait]
impl MarketplaceCatalogProvider for RegistryMarketplaceProvider {
    fn provider_key(&self) -> &'static str {
        "registry"
    }

    async fn list_modules(
        &self,
        _manifest: &ModulesManifest,
        _registry: &ModuleRegistry,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let Some(registry_url) = &self.registry_url else {
            return Ok(Vec::new());
        };

        if let Some(modules) = self.catalog_cache.get(registry_url).await {
            return Ok(modules.as_ref().clone());
        }

        match self.fetch_catalog(registry_url).await {
            Ok(modules) => {
                let modules = Arc::new(modules);
                self.catalog_cache
                    .insert(registry_url.clone(), modules.clone())
                    .await;
                Ok(modules.as_ref().clone())
            }
            Err(err) => {
                tracing::warn!(
                    registry_url,
                    error = %err,
                    "Registry marketplace provider fetch failed; falling back to local catalog only"
                );
                Ok(Vec::new())
            }
        }
    }
}

#[derive(Clone)]
pub struct SharedMarketplaceCatalogService(pub Arc<MarketplaceCatalogService>);

pub struct MarketplaceCatalogService {
    providers: Vec<Arc<dyn MarketplaceCatalogProvider>>,
}

impl MarketplaceCatalogService {
    pub fn new(providers: Vec<Arc<dyn MarketplaceCatalogProvider>>) -> Self {
        Self { providers }
    }

    pub fn local_only() -> Self {
        Self::new(vec![Arc::new(LocalManifestMarketplaceProvider)])
    }

    pub fn evolutionary_defaults() -> Self {
        Self::new(vec![
            Arc::new(LocalManifestMarketplaceProvider),
            Arc::new(RegistryMarketplaceProvider::from_env()),
        ])
    }

    pub async fn list_modules(
        &self,
        manifest: &ModulesManifest,
        registry: &ModuleRegistry,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let mut modules_by_slug = HashMap::<String, CatalogManifestModule>::new();

        for provider in &self.providers {
            let modules = provider.list_modules(manifest, registry).await?;
            for module in modules {
                modules_by_slug.entry(module.slug.clone()).or_insert(module);
            }
        }

        let mut modules = modules_by_slug.into_values().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(modules)
    }

    pub fn provider_keys(&self) -> Vec<&'static str> {
        self.providers
            .iter()
            .map(|provider| provider.provider_key())
            .collect()
    }
}

pub fn marketplace_catalog_from_context(ctx: &AppContext) -> Arc<MarketplaceCatalogService> {
    if let Some(shared) = ctx.shared_store.get::<SharedMarketplaceCatalogService>() {
        return shared.0.clone();
    }

    let service = Arc::new(MarketplaceCatalogService::evolutionary_defaults());
    ctx.shared_store
        .insert(SharedMarketplaceCatalogService(service.clone()));
    service
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ModulesManifest;

    struct TestProvider {
        key: &'static str,
        modules: Vec<CatalogManifestModule>,
    }

    #[async_trait]
    impl MarketplaceCatalogProvider for TestProvider {
        fn provider_key(&self) -> &'static str {
            self.key
        }

        async fn list_modules(
            &self,
            _manifest: &ModulesManifest,
            _registry: &ModuleRegistry,
        ) -> anyhow::Result<Vec<CatalogManifestModule>> {
            Ok(self.modules.clone())
        }
    }

    fn catalog_module(slug: &str, source: &str, crate_name: &str) -> CatalogManifestModule {
        CatalogManifestModule {
            slug: slug.to_string(),
            source: source.to_string(),
            crate_name: crate_name.to_string(),
            version: None,
            git: None,
            rev: None,
            path: None,
            required: false,
            depends_on: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
        }
    }

    #[tokio::test]
    async fn provider_order_keeps_first_provider_as_canonical_slug_source() {
        let service = MarketplaceCatalogService::new(vec![
            Arc::new(TestProvider {
                key: "local-manifest",
                modules: vec![
                    catalog_module("blog", "path", "rustok-blog"),
                    catalog_module("forum", "path", "rustok-forum"),
                ],
            }),
            Arc::new(TestProvider {
                key: "registry",
                modules: vec![
                    catalog_module("blog", "registry", "community-blog"),
                    catalog_module("seo", "registry", "rustok-seo"),
                ],
            }),
        ]);

        let modules = service
            .list_modules(&ModulesManifest::default(), &ModuleRegistry::new())
            .await
            .expect("catalog providers should resolve");

        assert_eq!(
            modules
                .iter()
                .map(|module| module.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["blog", "forum", "seo"]
        );
        assert_eq!(modules[0].source, "path");
        assert_eq!(modules[0].crate_name, "rustok-blog");
    }

    #[test]
    fn evolutionary_defaults_include_local_manifest_and_registry_skeleton() {
        let service = MarketplaceCatalogService::evolutionary_defaults();

        assert_eq!(service.provider_keys(), vec!["local-manifest", "registry"]);
    }

    #[test]
    fn registry_catalog_module_defaults_to_registry_metadata() {
        let module = RegistryCatalogModule {
            slug: "seo".to_string(),
            source: default_registry_source(),
            crate_name: "rustok-seo".to_string(),
            version: Some("1.2.0".to_string()),
            git: None,
            rev: None,
            path: None,
            required: false,
            depends_on: vec!["content".to_string()],
            ownership: default_registry_ownership(),
            trust_level: default_registry_trust_level(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: Some("RusTok Labs".to_string()),
            checksum_sha256: Some("abc123".to_string()),
            signature: Some("sig-1".to_string()),
            versions: vec![RegistryCatalogVersion {
                version: "1.2.0".to_string(),
                changelog: Some("Initial release".to_string()),
                yanked: false,
                published_at: Some("2026-03-08T00:00:00Z".to_string()),
                checksum_sha256: Some("abc123".to_string()),
                signature: Some("sig-1".to_string()),
            }],
            recommended_admin_surfaces: vec!["leptos-admin".to_string()],
            showcase_admin_surfaces: vec!["next-admin".to_string()],
        }
        .into_catalog_module();

        assert_eq!(module.source, "registry");
        assert_eq!(module.ownership, "third_party");
        assert_eq!(module.trust_level, "unverified");
        assert_eq!(module.crate_name, "rustok-seo");
        assert_eq!(module.publisher.as_deref(), Some("RusTok Labs"));
        assert_eq!(module.versions.len(), 1);
    }

    #[test]
    fn registry_catalog_response_deserializes_internal_contract() {
        let payload = serde_json::json!({
            "schema_version": 1,
            "modules": [
                {
                    "slug": "seo",
                    "crate": "rustok-seo",
                    "version": "1.2.0",
                    "depends_on": ["content"],
                    "publisher": "RusTok Labs",
                    "checksum_sha256": "abc123",
                    "signature": "sig-1",
                    "recommended_admin_surfaces": ["leptos-admin"],
                    "versions": [
                        {
                            "version": "1.2.0",
                            "checksum_sha256": "abc123",
                            "signature": "sig-1",
                            "published_at": "2026-03-08T00:00:00Z"
                        }
                    ]
                }
            ]
        });

        let response: RegistryCatalogResponse =
            serde_json::from_value(payload).expect("registry payload should deserialize");
        let module = response
            .modules
            .into_iter()
            .next()
            .expect("registry payload should contain module");

        assert_eq!(module.slug, "seo");
        assert_eq!(module.source, "registry");
        assert_eq!(module.crate_name, "rustok-seo");
        assert_eq!(module.ownership, "third_party");
        assert_eq!(module.trust_level, "unverified");
        assert_eq!(module.publisher.as_deref(), Some("RusTok Labs"));
        assert_eq!(module.versions.len(), 1);
    }

    #[test]
    fn registry_schema_version_is_validated() {
        assert!(validate_registry_schema_version(1).is_ok());
        assert!(validate_registry_schema_version(2).is_err());
    }
}

fn default_registry_source() -> String {
    "registry".to_string()
}

fn default_registry_catalog_schema_version() -> u32 {
    REGISTRY_CATALOG_SCHEMA_VERSION
}

fn default_registry_ownership() -> String {
    "third_party".to_string()
}

fn default_registry_trust_level() -> String {
    "unverified".to_string()
}

fn validate_registry_schema_version(schema_version: u32) -> anyhow::Result<()> {
    if schema_version == REGISTRY_CATALOG_SCHEMA_VERSION {
        return Ok(());
    }

    anyhow::bail!(
        "Unsupported registry catalog schema_version={schema_version}; expected {}",
        REGISTRY_CATALOG_SCHEMA_VERSION
    );
}

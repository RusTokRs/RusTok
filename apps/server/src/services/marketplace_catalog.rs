use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use moka::future::Cache;
use reqwest::{Client, StatusCode};
use rustok_core::ModuleRegistry;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::modules::{
    CatalogManifestModule, CatalogModuleVersion, ManifestManager, ModuleSettingSpec,
    ModulesManifest,
};

pub const REGISTRY_CATALOG_SCHEMA_VERSION: u32 = 1;
const REGISTRY_CATALOG_PATH: &str = "/v1/catalog";
const LEGACY_REGISTRY_CATALOG_PATH: &str = "/catalog";

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
        match self
            .fetch_catalog_from_path(registry_url, REGISTRY_CATALOG_PATH)
            .await
        {
            Ok(modules) => Ok(modules),
            Err(err) if should_fallback_to_legacy_catalog_path(&err) => {
                tracing::info!(
                    registry_url,
                    primary_path = REGISTRY_CATALOG_PATH,
                    fallback_path = LEGACY_REGISTRY_CATALOG_PATH,
                    "Registry marketplace provider falling back to legacy catalog path"
                );
                self.fetch_catalog_from_path(registry_url, LEGACY_REGISTRY_CATALOG_PATH)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn fetch_catalog_from_path(
        &self,
        registry_url: &str,
        path: &str,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let endpoint = format!("{}{}", registry_url.trim_end_matches('/'), path);
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegistryCatalogResponse {
    #[serde(default = "default_registry_catalog_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub modules: Vec<RegistryCatalogModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegistryCatalogModule {
    pub slug: String,
    #[serde(default = "default_registry_source")]
    pub source: String,
    #[serde(rename = "crate", alias = "crate_name")]
    pub crate_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_registry_ownership")]
    pub ownership: String,
    #[serde(default = "default_registry_trust_level")]
    pub trust_level: String,
    #[serde(default)]
    pub rustok_min_version: Option<String>,
    #[serde(default)]
    pub rustok_max_version: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub versions: Vec<RegistryCatalogVersion>,
    #[serde(default)]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub showcase_admin_surfaces: Vec<String>,
    #[serde(default)]
    pub settings_schema: HashMap<String, ModuleSettingSpec>,
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
            name: self.name,
            category: self.category,
            tags: self.tags,
            icon_url: self.icon_url,
            banner_url: self.banner_url,
            screenshots: self.screenshots,
            version: self.version,
            description: self.description,
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
            settings_schema: self.settings_schema,
        }
    }

    pub fn from_catalog_module(module: CatalogManifestModule) -> Self {
        let CatalogManifestModule {
            slug,
            source: _source,
            crate_name,
            name,
            category,
            tags,
            icon_url,
            banner_url,
            screenshots,
            version,
            description,
            git: _git,
            rev: _rev,
            path: _path,
            required,
            depends_on,
            ownership,
            trust_level,
            rustok_min_version,
            rustok_max_version,
            publisher,
            checksum_sha256,
            signature,
            versions,
            recommended_admin_surfaces,
            showcase_admin_surfaces,
            settings_schema,
        } = module;

        let versions = if versions.is_empty() {
            version
                .clone()
                .map(|version| {
                    vec![RegistryCatalogVersion {
                        version,
                        changelog: None,
                        yanked: false,
                        published_at: None,
                        checksum_sha256: checksum_sha256.clone(),
                        signature: signature.clone(),
                    }]
                })
                .unwrap_or_default()
        } else {
            versions
                .into_iter()
                .map(RegistryCatalogVersion::from_catalog_version)
                .collect()
        };

        Self {
            slug,
            source: default_registry_source(),
            crate_name,
            name,
            category,
            tags,
            icon_url,
            banner_url,
            screenshots,
            version,
            description,
            git: None,
            rev: None,
            path: None,
            required,
            depends_on,
            ownership,
            trust_level,
            rustok_min_version,
            rustok_max_version,
            publisher,
            checksum_sha256,
            signature,
            versions,
            recommended_admin_surfaces,
            showcase_admin_surfaces,
            settings_schema,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegistryCatalogVersion {
    pub version: String,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub checksum_sha256: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
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

    fn from_catalog_version(version: CatalogModuleVersion) -> Self {
        Self {
            version: version.version,
            changelog: version.changelog,
            yanked: version.yanked,
            published_at: version.published_at,
            checksum_sha256: version.checksum_sha256,
            signature: version.signature,
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

pub fn registry_catalog_path() -> &'static str {
    REGISTRY_CATALOG_PATH
}

pub fn legacy_registry_catalog_path() -> &'static str {
    LEGACY_REGISTRY_CATALOG_PATH
}

pub fn registry_catalog_from_modules(
    modules: Vec<CatalogManifestModule>,
) -> RegistryCatalogResponse {
    let mut modules = modules
        .into_iter()
        .map(RegistryCatalogModule::from_catalog_module)
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.slug.cmp(&right.slug));

    RegistryCatalogResponse {
        schema_version: REGISTRY_CATALOG_SCHEMA_VERSION,
        modules,
    }
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
            name: None,
            category: None,
            tags: Vec::new(),
            icon_url: None,
            banner_url: None,
            screenshots: Vec::new(),
            version: None,
            description: None,
            git: None,
            rev: None,
            path: None,
            required: false,
            depends_on: Vec::new(),
            ownership: "third_party".to_string(),
            trust_level: "unverified".to_string(),
            rustok_min_version: None,
            rustok_max_version: None,
            publisher: None,
            checksum_sha256: None,
            signature: None,
            versions: Vec::new(),
            recommended_admin_surfaces: Vec::new(),
            showcase_admin_surfaces: Vec::new(),
            settings_schema: HashMap::new(),
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
            name: Some("SEO".to_string()),
            category: Some("marketing".to_string()),
            tags: vec!["marketing".to_string(), "seo".to_string()],
            icon_url: Some("https://cdn.example.test/modules/seo/icon.svg".to_string()),
            banner_url: Some("https://cdn.example.test/modules/seo/banner.png".to_string()),
            screenshots: vec![
                "https://cdn.example.test/modules/seo/screenshot-1.png".to_string(),
                "https://cdn.example.test/modules/seo/screenshot-2.png".to_string(),
            ],
            version: Some("1.2.0".to_string()),
            description: Some("SEO tools".to_string()),
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
            settings_schema: HashMap::new(),
        }
        .into_catalog_module();

        assert_eq!(module.source, "registry");
        assert_eq!(module.ownership, "third_party");
        assert_eq!(module.trust_level, "unverified");
        assert_eq!(module.crate_name, "rustok-seo");
        assert_eq!(module.name.as_deref(), Some("SEO"));
        assert_eq!(module.category.as_deref(), Some("marketing"));
        assert_eq!(module.tags, vec!["marketing", "seo"]);
        assert_eq!(
            module.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/icon.svg")
        );
        assert_eq!(
            module.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/banner.png")
        );
        assert_eq!(module.screenshots.len(), 2);
        assert_eq!(module.description.as_deref(), Some("SEO tools"));
        assert_eq!(module.publisher.as_deref(), Some("RusTok Labs"));
        assert_eq!(module.versions.len(), 1);
    }

    #[test]
    fn registry_catalog_from_modules_normalizes_registry_surface() {
        let response = registry_catalog_from_modules(vec![CatalogManifestModule {
            slug: "blog".to_string(),
            source: "path".to_string(),
            crate_name: "rustok-blog".to_string(),
            name: Some("Blog".to_string()),
            category: Some("content".to_string()),
            tags: vec!["content".to_string(), "editorial".to_string()],
            icon_url: Some("https://cdn.example.test/modules/blog/icon.svg".to_string()),
            banner_url: Some("https://cdn.example.test/modules/blog/banner.png".to_string()),
            screenshots: vec!["https://cdn.example.test/modules/blog/screenshot-1.png".to_string()],
            version: Some("1.4.0".to_string()),
            description: Some("Blog module".to_string()),
            git: Some("https://example.test/blog.git".to_string()),
            rev: Some("abc123".to_string()),
            path: Some("../../crates/rustok-blog".to_string()),
            required: false,
            depends_on: vec!["content".to_string()],
            ownership: "first_party".to_string(),
            trust_level: "verified".to_string(),
            rustok_min_version: Some("0.9.0".to_string()),
            rustok_max_version: None,
            publisher: Some("RusTok Labs".to_string()),
            checksum_sha256: Some("sum123".to_string()),
            signature: Some("sig123".to_string()),
            versions: Vec::new(),
            recommended_admin_surfaces: vec!["leptos-admin".to_string()],
            showcase_admin_surfaces: vec!["next-admin".to_string()],
            settings_schema: HashMap::new(),
        }]);

        let module = response
            .modules
            .into_iter()
            .next()
            .expect("catalog response should contain module");

        assert_eq!(response.schema_version, REGISTRY_CATALOG_SCHEMA_VERSION);
        assert_eq!(module.source, "registry");
        assert_eq!(module.name.as_deref(), Some("Blog"));
        assert_eq!(module.category.as_deref(), Some("content"));
        assert_eq!(module.tags, vec!["content", "editorial"]);
        assert_eq!(
            module.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/blog/icon.svg")
        );
        assert_eq!(
            module.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/blog/banner.png")
        );
        assert_eq!(
            module.screenshots,
            vec!["https://cdn.example.test/modules/blog/screenshot-1.png"]
        );
        assert_eq!(module.description.as_deref(), Some("Blog module"));
        assert_eq!(module.path, None);
        assert_eq!(module.git, None);
        assert_eq!(module.rev, None);
        assert_eq!(module.versions.len(), 1);
        assert_eq!(module.versions[0].version, "1.4.0");
    }

    #[test]
    fn registry_catalog_response_deserializes_internal_contract() {
        let payload = serde_json::json!({
            "schema_version": 1,
            "modules": [
                {
                    "slug": "seo",
                    "crate": "rustok-seo",
                    "name": "SEO",
                    "category": "marketing",
                    "tags": ["marketing", "seo"],
                    "icon_url": "https://cdn.example.test/modules/seo/icon.svg",
                    "banner_url": "https://cdn.example.test/modules/seo/banner.png",
                    "screenshots": [
                        "https://cdn.example.test/modules/seo/screenshot-1.png"
                    ],
                    "version": "1.2.0",
                    "description": "SEO tools",
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
        assert_eq!(module.name.as_deref(), Some("SEO"));
        assert_eq!(module.category.as_deref(), Some("marketing"));
        assert_eq!(module.tags, vec!["marketing", "seo"]);
        assert_eq!(
            module.icon_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/icon.svg")
        );
        assert_eq!(
            module.banner_url.as_deref(),
            Some("https://cdn.example.test/modules/seo/banner.png")
        );
        assert_eq!(
            module.screenshots,
            vec!["https://cdn.example.test/modules/seo/screenshot-1.png"]
        );
        assert_eq!(module.description.as_deref(), Some("SEO tools"));
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

fn should_fallback_to_legacy_catalog_path(err: &anyhow::Error) -> bool {
    err.downcast_ref::<reqwest::Error>()
        .and_then(|error| error.status())
        .is_some_and(|status| {
            matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            )
        })
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

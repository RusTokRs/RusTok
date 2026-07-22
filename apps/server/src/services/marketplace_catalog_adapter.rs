use async_trait::async_trait;
use rustok_core::ModuleRegistry;
use rustok_modules::{
    normalize_module_marketplace_slug, ModuleMarketplaceCatalog, ModuleMarketplaceEntry,
    ModuleMarketplaceError, ModuleMarketplaceQuery, ModuleMarketplaceVersion,
    MODULE_MARKETPLACE_MAX_LIMIT,
};
use semver::{Version, VersionReq};

use crate::modules::{CatalogManifestModule, ManifestManager};
use crate::services::marketplace_catalog::{
    marketplace_catalog_from_context, MarketplaceCatalogQuery,
};
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::registry_governance::RegistryGovernanceService;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
pub struct ServerMarketplaceCatalog {
    runtime: ServerRuntimeContext,
    registry: ModuleRegistry,
}

impl ServerMarketplaceCatalog {
    pub fn new(runtime: ServerRuntimeContext, registry: ModuleRegistry) -> Self {
        Self { runtime, registry }
    }

    async fn projected_modules(
        &self,
        query: &ModuleMarketplaceQuery,
    ) -> Result<
        (
            Vec<CatalogManifestModule>,
            Vec<crate::modules::InstalledManifestModule>,
        ),
        ModuleMarketplaceError,
    > {
        let manifest = PlatformCompositionService::active_manifest(self.runtime.db())
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let installed = ManifestManager::installed_modules(&manifest);
        let provider_query = MarketplaceCatalogQuery {
            search: query.search.clone(),
            category: query.category.clone(),
            tag: query.tag.clone(),
        };
        let modules = marketplace_catalog_from_context(&self.runtime)
            .list_modules(&manifest, &self.registry, &provider_query)
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let modules = RegistryGovernanceService::new(self.runtime.db_clone())
            .apply_catalog_projection(
                modules,
                query.preferred_locale.as_deref(),
                query.fallback_locale.as_deref(),
            )
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        Ok((modules, installed))
    }
}

#[async_trait]
impl ModuleMarketplaceCatalog for ServerMarketplaceCatalog {
    async fn list(
        &self,
        query: ModuleMarketplaceQuery,
    ) -> Result<Vec<ModuleMarketplaceEntry>, ModuleMarketplaceError> {
        let (modules, installed) = self.projected_modules(&query).await?;
        let source = normalized_filter(query.source.as_deref());
        let trust_level = normalized_filter(query.trust_level.as_deref());
        let limit = query.limit.clamp(1, MODULE_MARKETPLACE_MAX_LIMIT) as usize;
        let mut entries = Vec::new();
        for module in modules {
            let entry = map_catalog_entry(module, &self.registry, &installed)?;
            if entry.kind != "optional"
                || (query.only_compatible && !entry.compatible && !entry.installed)
                || (query.installed_only && !entry.installed)
                || source
                    .as_ref()
                    .is_some_and(|value| !entry.source.eq_ignore_ascii_case(value))
                || trust_level
                    .as_ref()
                    .is_some_and(|value| !entry.trust_level.eq_ignore_ascii_case(value))
            {
                continue;
            }
            entries.push(entry);
            if entries.len() == limit {
                break;
            }
        }
        Ok(entries)
    }

    async fn get(
        &self,
        slug: &str,
        preferred_locale: Option<String>,
        fallback_locale: Option<String>,
    ) -> Result<Option<ModuleMarketplaceEntry>, ModuleMarketplaceError> {
        let slug =
            normalize_module_marketplace_slug(slug).ok_or(ModuleMarketplaceError::InvalidQuery)?;
        let manifest = PlatformCompositionService::active_manifest(self.runtime.db())
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let installed = ManifestManager::installed_modules(&manifest);
        let provider_query = MarketplaceCatalogQuery::default();
        let Some(module) = marketplace_catalog_from_context(&self.runtime)
            .get_module(&manifest, &self.registry, &provider_query, &slug)
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?
        else {
            return Ok(None);
        };
        let mut projected = RegistryGovernanceService::new(self.runtime.db_clone())
            .apply_catalog_projection(
                vec![module],
                preferred_locale.as_deref(),
                fallback_locale.as_deref(),
            )
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        let Some(module) = projected.pop() else {
            return Ok(None);
        };
        let mut entry = map_catalog_entry(module, &self.registry, &installed)?;
        if entry.kind != "optional" {
            return Ok(None);
        }
        entry.registry_lifecycle = rustok_modules::ModuleControlPlane::new(self.runtime.db_clone())
            .release()
            .lifecycle_snapshot(&entry.slug)
            .await
            .map_err(|_| ModuleMarketplaceError::Unavailable)?;
        Ok(Some(entry))
    }
}

fn map_catalog_entry(
    entry: CatalogManifestModule,
    registry: &ModuleRegistry,
    installed_modules: &[crate::modules::InstalledManifestModule],
) -> Result<ModuleMarketplaceEntry, ModuleMarketplaceError> {
    let catalog_version_fallback = entry
        .versions
        .first()
        .map(|version| version.version.clone());
    let compatible = is_compatible(&entry);
    let signature_present = entry.signature.is_some();
    let runtime_module = registry.get(&entry.slug);
    let installed_module = installed_modules
        .iter()
        .find(|module| module.slug == entry.slug);
    let latest_version = runtime_module
        .map(|module| module.version().to_string())
        .or_else(|| entry.version.clone())
        .or(catalog_version_fallback)
        .unwrap_or_else(|| "workspace".to_string());
    let installed_version = installed_module.and_then(|module| module.version.clone());
    let dependencies = runtime_module
        .map(|module| {
            module
                .dependencies()
                .iter()
                .map(|dependency| dependency.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| entry.depends_on.clone());
    let versions = if entry.versions.is_empty() {
        vec![ModuleMarketplaceVersion {
            version: latest_version.clone(),
            changelog: None,
            yanked: false,
            published_at: None,
            checksum_sha256: entry.checksum_sha256.clone(),
            signature_present,
        }]
    } else {
        entry
            .versions
            .iter()
            .map(|version| ModuleMarketplaceVersion {
                version: version.version.clone(),
                changelog: version.changelog.clone(),
                yanked: version.yanked,
                published_at: version.published_at.clone(),
                checksum_sha256: version.checksum_sha256.clone(),
                signature_present: version.signature.is_some(),
            })
            .collect()
    };
    let settings_schema = serde_json::from_value(
        serde_json::to_value(entry.settings_schema)
            .map_err(|_| ModuleMarketplaceError::InvalidContract)?,
    )
    .map_err(|_| ModuleMarketplaceError::InvalidContract)?;

    Ok(ModuleMarketplaceEntry {
        slug: entry.slug.clone(),
        name: entry
            .name
            .or_else(|| runtime_module.map(|module| module.name().to_string()))
            .unwrap_or_else(|| humanize_slug(&entry.slug)),
        latest_version: latest_version.clone(),
        description: entry
            .description
            .or_else(|| runtime_module.map(|module| module.description().to_string()))
            .unwrap_or_else(|| {
                format!(
                    "{} module from {} source",
                    humanize_slug(&entry.slug),
                    entry.source
                )
            }),
        source: entry.source,
        kind: if entry.required || registry.is_core(&entry.slug) {
            "core".to_string()
        } else {
            "optional".to_string()
        },
        category: entry
            .category
            .unwrap_or_else(|| fallback_category(&entry.slug).to_string()),
        tags: entry.tags,
        icon_url: entry.icon_url,
        banner_url: entry.banner_url,
        screenshots: entry.screenshots,
        crate_name: entry.crate_name,
        dependencies,
        ownership: entry.ownership,
        trust_level: entry.trust_level,
        rustok_min_version: entry.rustok_min_version,
        rustok_max_version: entry.rustok_max_version,
        publisher: entry.publisher,
        checksum_sha256: entry.checksum_sha256,
        signature_present,
        versions,
        has_admin_ui: entry.has_admin_ui,
        has_storefront_ui: entry.has_storefront_ui,
        ui_classification: entry.ui_classification,
        registry_lifecycle: None,
        compatible,
        recommended_admin_surfaces: entry.recommended_admin_surfaces,
        showcase_admin_surfaces: entry.showcase_admin_surfaces,
        settings_schema,
        installed: installed_module.is_some(),
        installed_version: installed_version.clone(),
        update_available: installed_version
            .as_ref()
            .is_some_and(|version| version != &latest_version),
    })
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn is_compatible(entry: &CatalogManifestModule) -> bool {
    let Ok(platform_version) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        return false;
    };
    let min_ok = entry
        .rustok_min_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, false)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    let max_ok = entry
        .rustok_max_version
        .as_deref()
        .and_then(|raw| VersionReq::parse(&normalize_version_req(raw, true)).ok())
        .is_none_or(|req| req.matches(&platform_version));
    min_ok && max_ok
}

fn normalize_version_req(value: &str, is_max: bool) -> String {
    let wildcard = value.trim().replace(".x", ".*").replace(".X", ".*");
    let has_operator = wildcard.contains('<')
        || wildcard.contains('>')
        || wildcard.contains('=')
        || wildcard.contains('~')
        || wildcard.contains('^')
        || wildcard.contains('*')
        || wildcard.contains(',');
    if has_operator {
        wildcard
    } else if is_max {
        format!("<= {wildcard}")
    } else {
        format!(">= {wildcard}")
    }
}

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" | "pricing" | "product" | "inventory" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

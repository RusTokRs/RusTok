mod bulk;
mod diagnostics;
mod meta;
mod redirects;
mod robots;
mod routing;
mod sitemaps;
mod targets;
mod templates;

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_content::normalize_locale_code;
use rustok_core::{normalize_locale_tag, ModuleRuntimeExtensions};
#[cfg(test)]
use rustok_core::{MemoryTransport, RusToKModule};
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::{
    seo_target_registry_from_extensions, SeoTargetCapabilityKind, SeoTargetRegistry,
    SeoTargetRegistryEntry, SeoTargetSlug,
};
use rustok_tenant::entities::tenant_module;

use crate::dto::{SeoAlternateLink, SeoModuleSettings, SeoOpenGraph};
use crate::entities::{self as seo_meta, meta_translation, seo_redirect};
use crate::{SeoError, SeoResult};

pub use rustok_blog::PostService;
pub use rustok_pages::PageService;
pub use rustok_product::CatalogService;

const MODULE_SLUG: &str = "seo";
const REDIRECT_CACHE_TTL_SECS: u64 = 30;
const SITEMAP_CHUNK_SIZE: usize = 500;

static REDIRECT_CACHE: Lazy<Cache<Uuid, Arc<Vec<seo_redirect::Model>>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(REDIRECT_CACHE_TTL_SECS))
        .max_capacity(512)
        .build()
});

#[derive(Clone)]
pub struct SeoService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    registry: Arc<SeoTargetRegistry>,
}

#[derive(Clone)]
struct LoadedMeta {
    meta: seo_meta::Model,
    translations: Vec<meta_translation::Model>,
}

#[derive(Clone)]
struct TargetState {
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    requested_locale: Option<String>,
    effective_locale: String,
    title: String,
    description: Option<String>,
    canonical_path: String,
    alternates: Vec<SeoAlternateLink>,
    open_graph: SeoOpenGraph,
    structured_data: serde_json::Value,
    fallback_source: String,
    template_fields: std::collections::BTreeMap<String, String>,
}

impl SeoService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        registry: Arc<SeoTargetRegistry>,
    ) -> Self {
        Self {
            db,
            event_bus,
            registry,
        }
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        extensions: &ModuleRuntimeExtensions,
    ) -> SeoResult<Self> {
        let registry = seo_target_registry_from_extensions(extensions)
            .ok_or_else(|| SeoError::configuration("SEO target registry is not initialized"))?;
        Ok(Self::new(db, event_bus, registry))
    }

    #[cfg(test)]
    pub(crate) fn with_builtin_registry(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> Self {
        Self::new(db, event_bus, built_in_target_registry())
    }

    #[cfg(test)]
    pub(crate) fn new_memory(db: DatabaseConnection) -> Self {
        Self::with_builtin_registry(
            db,
            TransactionalEventBus::new(Arc::new(MemoryTransport::new())),
        )
    }

    pub async fn is_enabled(&self, tenant_id: Uuid) -> SeoResult<bool> {
        tenant_module::Entity::is_enabled(&self.db, tenant_id, MODULE_SLUG)
            .await
            .map_err(SeoError::from)
    }

    pub async fn load_settings(&self, tenant_id: Uuid) -> SeoResult<SeoModuleSettings> {
        let Some(module) = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(MODULE_SLUG))
            .one(&self.db)
            .await?
        else {
            return Ok(SeoModuleSettings::default());
        };

        Ok(Self::normalize_settings(
            serde_json::from_value::<SeoModuleSettings>(module.settings.into()).unwrap_or_default(),
        ))
    }

    pub fn normalize_settings(mut settings: SeoModuleSettings) -> SeoModuleSettings {
        settings.default_robots = robots::normalize_robots(settings.default_robots.as_slice());
        settings.allowed_redirect_hosts =
            redirects::normalize_hosts(settings.allowed_redirect_hosts.as_slice());
        settings.allowed_canonical_hosts =
            redirects::normalize_hosts(settings.allowed_canonical_hosts.as_slice());
        settings.x_default_locale = settings
            .x_default_locale
            .as_deref()
            .and_then(normalize_locale_tag);
        settings.template_defaults = templates::normalize_rule_set(settings.template_defaults);
        settings.template_overrides = settings
            .template_overrides
            .into_iter()
            .filter_map(|(slug, rules)| {
                let normalized_slug = slug.trim().to_ascii_lowercase();
                if normalized_slug.is_empty() {
                    return None;
                }
                Some((normalized_slug, templates::normalize_rule_set(rules)))
            })
            .collect();
        settings
    }

    pub fn target_registry_entries(
        &self,
        capability: Option<SeoTargetCapabilityKind>,
    ) -> Vec<SeoTargetRegistryEntry> {
        match capability {
            Some(capability) => self.registry.entries_with_capability(capability),
            None => self.registry.entries(),
        }
    }
}

#[cfg(test)]
fn built_in_target_registry() -> Arc<SeoTargetRegistry> {
    let mut extensions = ModuleRuntimeExtensions::default();
    rustok_pages::PagesModule.register_runtime_extensions(&mut extensions);
    rustok_product::ProductModule.register_runtime_extensions(&mut extensions);
    rustok_blog::BlogModule.register_runtime_extensions(&mut extensions);
    rustok_forum::ForumModule.register_runtime_extensions(&mut extensions);
    seo_target_registry_from_extensions(&extensions)
        .unwrap_or_else(|| Arc::new(SeoTargetRegistry::default()))
}

pub(super) fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

pub(super) fn normalize_effective_locale(locale: &str, fallback_locale: &str) -> SeoResult<String> {
    normalize_locale_tag(locale)
        .or_else(|| normalize_locale_code(locale))
        .or_else(|| normalize_locale_tag(fallback_locale))
        .ok_or_else(|| SeoError::validation("invalid locale"))
}

pub(super) fn normalize_route(route: &str) -> SeoResult<String> {
    let route = route.trim();
    if route.is_empty() {
        return Err(SeoError::validation("route must not be empty"));
    }
    if !route.starts_with('/') {
        return Err(SeoError::validation("route must start with `/`"));
    }
    if route.chars().any(char::is_whitespace) {
        return Err(SeoError::validation("route must not contain whitespace"));
    }
    Ok(route.to_string())
}

mod bulk;
mod cross_links;
mod diagnostics;
mod events;
mod meta;
mod redirects;
mod robots;
mod routing;
mod schema_validation;
mod sitemaps;
mod targets;
mod templates;

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_api::normalize_locale_tag;
use rustok_content::normalize_locale_code;
use rustok_core::ModuleRuntimeExtensions;
#[cfg(test)]
use rustok_core::{MemoryTransport, RusToKModule};
use rustok_media::MediaAssetReadPort;
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::{
    seo_target_registry_from_extensions, SeoTargetCapabilityKind, SeoTargetRegistry,
    SeoTargetRegistryEntry, SeoTargetSlug,
};
use rustok_tenant::entities::tenant_module;

use crate::dto::{SeoAlternateLink, SeoModuleSettings, SeoOpenGraph};
use crate::entities::{self as seo_meta, meta_translation, seo_redirect};
use crate::{SeoError, SeoResult};

const MODULE_SLUG: &str = "seo";
const REDIRECT_CACHE_TTL_SECS: u64 = 30;
const REDIRECT_CACHE_MAX_WEIGHT_BYTES: u64 = 8 * 1024 * 1024;
const SITEMAP_CHUNK_SIZE: usize = 500;
const SEO_SETTINGS_KEYS: &[&str] = &[
    "default_robots",
    "sitemap_enabled",
    "allowed_redirect_hosts",
    "allowed_canonical_hosts",
    "x_default_locale",
    "template_defaults",
    "template_overrides",
    "sitemap_submission_endpoints",
];

static REDIRECT_CACHE: Lazy<Cache<Uuid, Arc<Vec<seo_redirect::Model>>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(REDIRECT_CACHE_TTL_SECS))
        .weigher(redirect_cache_entry_weight)
        .max_capacity(REDIRECT_CACHE_MAX_WEIGHT_BYTES)
        .build()
});

fn redirect_cache_entry_weight(
    _tenant_id: &Uuid,
    redirects: &Arc<Vec<seo_redirect::Model>>,
) -> u32 {
    let mut weight = std::mem::size_of::<Uuid>()
        .saturating_add(std::mem::size_of::<Arc<Vec<seo_redirect::Model>>>())
        .saturating_add(std::mem::size_of::<Vec<seo_redirect::Model>>());
    for redirect in redirects.iter() {
        weight = weight
            .saturating_add(std::mem::size_of::<seo_redirect::Model>())
            .saturating_add(redirect.match_type.len())
            .saturating_add(redirect.source_pattern.len())
            .saturating_add(redirect.target_url.len());
    }
    weight.clamp(1, u32::MAX as usize) as u32
}

#[derive(Clone)]
pub struct SeoService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    registry: Arc<SeoTargetRegistry>,
    media_asset_read_port: Option<Arc<dyn MediaAssetReadPort>>,
}

#[derive(Clone)]
pub struct SeoMediaAssetReadProvider {
    port: Arc<dyn MediaAssetReadPort>,
}

impl SeoMediaAssetReadProvider {
    pub fn new(port: Arc<dyn MediaAssetReadPort>) -> Self {
        Self { port }
    }
    fn port(&self) -> Arc<dyn MediaAssetReadPort> {
        Arc::clone(&self.port)
    }
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
            media_asset_read_port: None,
        }
    }

    pub fn with_media_asset_read_port(mut self, port: Arc<dyn MediaAssetReadPort>) -> Self {
        self.media_asset_read_port = Some(port);
        self
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        extensions: &ModuleRuntimeExtensions,
    ) -> SeoResult<Self> {
        let registry = seo_target_registry_from_extensions(extensions)
            .ok_or_else(|| SeoError::configuration("SEO target registry is not initialized"))?;
        let service = Self::new(db, event_bus, registry);
        if let Some(provider) = extensions.get::<SeoMediaAssetReadProvider>() {
            Ok(service.with_media_asset_read_port(provider.port()))
        } else {
            Ok(service)
        }
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

        Ok(Self::normalize_settings(parse_persisted_settings(
            module.settings,
        )?))
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
        settings.sitemap_submission_endpoints = sitemaps::normalize_sitemap_submission_endpoints(
            settings.sitemap_submission_endpoints.as_slice(),
        );
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

fn parse_persisted_settings(value: serde_json::Value) -> SeoResult<SeoModuleSettings> {
    let object = value.as_object().ok_or_else(|| {
        SeoError::configuration("persisted SEO settings must be a JSON object")
    })?;
    if let Some(unknown) = object
        .keys()
        .find(|key| !SEO_SETTINGS_KEYS.contains(&key.as_str()))
    {
        return Err(SeoError::configuration(format!(
            "unknown persisted SEO setting `{unknown}`"
        )));
    }

    let settings: SeoModuleSettings = serde_json::from_value(value).map_err(|error| {
        SeoError::configuration(format!("invalid persisted SEO settings: {error}"))
    })?;
    validate_persisted_settings(&settings)?;
    Ok(settings)
}

fn validate_persisted_settings(settings: &SeoModuleSettings) -> SeoResult<()> {
    if let Some(locale) = settings.x_default_locale.as_deref() {
        if normalize_locale_tag(locale).is_none() {
            return Err(SeoError::configuration(format!(
                "invalid persisted SEO x_default_locale `{locale}`"
            )));
        }
    }

    for (field, hosts) in [
        ("allowed_redirect_hosts", settings.allowed_redirect_hosts.as_slice()),
        ("allowed_canonical_hosts", settings.allowed_canonical_hosts.as_slice()),
    ] {
        for host in hosts {
            validate_settings_host(host, field)?;
        }
    }

    for slug in settings.template_overrides.keys() {
        if slug.trim().is_empty() {
            return Err(SeoError::configuration(
                "persisted SEO template override slug must not be empty",
            ));
        }
    }

    for endpoint in &settings.sitemap_submission_endpoints {
        validate_sitemap_submission_endpoint(endpoint)?;
    }

    Ok(())
}

fn validate_settings_host(value: &str, field: &str) -> SeoResult<()> {
    let host = value.trim().trim_end_matches('.');
    if host.is_empty()
        || host.chars().any(char::is_whitespace)
        || host.contains("://")
        || ['/', '?', '#', '@']
            .iter()
            .any(|marker| host.contains(*marker))
    {
        return Err(SeoError::configuration(format!(
            "invalid persisted SEO {field} entry `{value}`"
        )));
    }
    let parsed = url::Url::parse(format!("https://{host}").as_str()).map_err(|_| {
        SeoError::configuration(format!("invalid persisted SEO {field} entry `{value}`"))
    })?;
    if parsed.host_str().is_none() || parsed.path() != "/" {
        return Err(SeoError::configuration(format!(
            "invalid persisted SEO {field} entry `{value}`"
        )));
    }
    Ok(())
}

fn validate_sitemap_submission_endpoint(value: &str) -> SeoResult<()> {
    let parsed = url::Url::parse(value.trim()).map_err(|_| {
        SeoError::configuration(format!(
            "invalid persisted SEO sitemap submission endpoint `{value}`"
        ))
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.host_str().is_none()
        || parsed.fragment().is_some()
    {
        return Err(SeoError::configuration(format!(
            "invalid persisted SEO sitemap submission endpoint `{value}`"
        )));
    }
    Ok(())
}

#[cfg(test)]
fn built_in_target_registry() -> Arc<SeoTargetRegistry> {
    let mut extensions = ModuleRuntimeExtensions::default();
    rustok_pages::PagesModule
        .register_runtime_extensions(&mut extensions)
        .expect("Pages SEO target provider should register");
    rustok_product::ProductModule
        .register_runtime_extensions(&mut extensions)
        .expect("Product SEO target provider should register");
    rustok_blog::BlogModule
        .register_runtime_extensions(&mut extensions)
        .expect("Blog SEO target provider should register");
    rustok_forum::ForumModule
        .register_runtime_extensions(&mut extensions)
        .expect("Forum runtime providers should register");
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

#[cfg(test)]
mod settings_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn persisted_settings_accept_valid_payload() {
        let settings = parse_persisted_settings(json!({
            "default_robots": ["index", "follow"],
            "sitemap_enabled": false
        }))
        .expect("valid settings should load");

        assert_eq!(settings.default_robots, vec!["index", "follow"]);
        assert!(!settings.sitemap_enabled);
    }

    #[test]
    fn persisted_settings_reject_malformed_field_types() {
        let error = parse_persisted_settings(json!({
            "sitemap_enabled": "yes"
        }))
        .expect_err("invalid settings must not silently fall back to defaults");

        assert!(error.to_string().contains("invalid persisted SEO settings"));
        assert!(error.to_string().contains("sitemap_enabled"));
    }

    #[test]
    fn persisted_settings_reject_unknown_fields() {
        let error = parse_persisted_settings(json!({
            "sitemap_enabeld": false
        }))
        .expect_err("typoed settings must not silently use defaults");

        assert!(error.to_string().contains("unknown persisted SEO setting"));
        assert!(error.to_string().contains("sitemap_enabeld"));
    }

    #[test]
    fn persisted_settings_reject_invalid_semantic_values() {
        for value in [
            json!({"x_default_locale": "not a locale"}),
            json!({"allowed_redirect_hosts": ["https://example.com/path"]}),
            json!({"sitemap_submission_endpoints": ["file:///tmp/sitemap"]}),
            json!({"template_overrides": {" ": {}}}),
        ] {
            assert!(parse_persisted_settings(value).is_err());
        }
    }

    #[test]
    fn persisted_settings_reject_non_object_payload() {
        let error = parse_persisted_settings(json!(["index", "follow"]))
            .expect_err("settings root must remain a JSON object");

        assert!(error.to_string().contains("must be a JSON object"));
    }
}

#[cfg(test)]
mod cache_weight_tests {
    use super::*;
    use chrono::Utc;

    fn redirect(source_pattern: String) -> seo_redirect::Model {
        let now = Utc::now().fixed_offset();
        seo_redirect::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            match_type: "exact".to_string(),
            source_pattern,
            target_url: "/target".to_string(),
            status_code: 301,
            expires_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn redirect_cache_weight_accounts_for_dynamic_routes() {
        let tenant_id = Uuid::new_v4();
        let short = Arc::new(vec![redirect("/a".to_string())]);
        let long = Arc::new(vec![redirect(format!("/{}", "x".repeat(2_048)))]);

        assert!(
            redirect_cache_entry_weight(&tenant_id, &long)
                > redirect_cache_entry_weight(&tenant_id, &short)
        );
    }
}

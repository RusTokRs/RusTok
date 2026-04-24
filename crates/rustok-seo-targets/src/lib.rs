use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use anyhow::Result as AnyResult;
use async_graphql::{
    Enum, InputValueError, InputValueResult, Scalar, ScalarType, SimpleObject, Value,
};
use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

pub mod builtin_slug {
    pub const PAGE: &str = "page";
    pub const PRODUCT: &str = "product";
    pub const BLOG_POST: &str = "blog_post";
    pub const FORUM_CATEGORY: &str = "forum_category";
    pub const FORUM_TOPIC: &str = "forum_topic";
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeoTargetSlug(String);

impl SeoTargetSlug {
    pub fn new(value: impl Into<String>) -> Result<Self, SeoTargetSlugError> {
        let value = value.into();
        validate_slug(value.as_str())?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for SeoTargetSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SeoTargetSlug {
    type Err = SeoTargetSlugError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SeoTargetSlug {
    type Error = SeoTargetSlugError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for SeoTargetSlug {
    type Error = SeoTargetSlugError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[Scalar(name = "SeoTargetSlug")]
impl ScalarType for SeoTargetSlug {
    fn parse(value: Value) -> InputValueResult<Self> {
        let Value::String(value) = value else {
            return Err(InputValueError::expected_type(value));
        };
        Self::new(value).map_err(InputValueError::custom)
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum SeoTargetSlugError {
    #[error("SEO target slug must not be empty")]
    Empty,
    #[error("SEO target slug `{0}` must start and end with an ASCII lowercase letter or digit")]
    Boundary(String),
    #[error(
        "SEO target slug `{slug}` contains invalid character `{invalid}`; use lowercase ASCII letters, digits, `_`, or `-`"
    )]
    InvalidCharacter { slug: String, invalid: char },
}

fn validate_slug(value: &str) -> Result<(), SeoTargetSlugError> {
    if value.is_empty() {
        return Err(SeoTargetSlugError::Empty);
    }

    let mut chars = value.chars();
    let first = chars
        .next()
        .expect("validated SEO target slug must contain a first character");
    let last = value
        .chars()
        .last()
        .expect("validated SEO target slug must contain a last character");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(SeoTargetSlugError::Boundary(value.to_string()));
    }
    if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
        return Err(SeoTargetSlugError::Boundary(value.to_string()));
    }

    for ch in value.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-' {
            continue;
        }
        return Err(SeoTargetSlugError::InvalidCharacter {
            slug: value.to_string(),
            invalid: ch,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, SimpleObject)]
pub struct SeoTargetCapabilities {
    pub authoring: bool,
    pub routing: bool,
    pub bulk: bool,
    pub sitemaps: bool,
}

impl SeoTargetCapabilities {
    pub const fn new(authoring: bool, routing: bool, bulk: bool, sitemaps: bool) -> Self {
        Self {
            authoring,
            routing,
            bulk,
            sitemaps,
        }
    }

    pub fn supports(self, capability: SeoTargetCapabilityKind) -> bool {
        match capability {
            SeoTargetCapabilityKind::Authoring => self.authoring,
            SeoTargetCapabilityKind::Routing => self.routing,
            SeoTargetCapabilityKind::Bulk => self.bulk,
            SeoTargetCapabilityKind::Sitemaps => self.sitemaps,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Enum)]
#[serde(rename_all = "snake_case")]
pub enum SeoTargetCapabilityKind {
    Authoring,
    Routing,
    Bulk,
    Sitemaps,
}

#[derive(Clone)]
pub struct SeoTargetRuntimeContext {
    pub db: DatabaseConnection,
    pub event_bus: TransactionalEventBus,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SeoTargetLoadScope {
    Authoring,
    PublicRoute,
}

#[derive(Debug, Clone, Copy)]
pub struct SeoTargetLoadRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
    pub target_id: Uuid,
    pub scope: SeoTargetLoadScope,
    pub channel_slug: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct SeoTargetRouteResolveRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
    pub route: &'a str,
    pub channel_slug: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct SeoTargetBulkListRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct SeoTargetSitemapRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, SimpleObject)]
pub struct SeoTargetRegistryEntry {
    pub slug: SeoTargetSlug,
    pub display_name: String,
    pub owner_module_slug: String,
    pub capabilities: SeoTargetCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SeoRouteMatchRecord {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct SeoTargetAlternateRoute {
    pub locale: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct SeoTargetImageRecord {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct SeoTargetOpenGraphRecord {
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub locale: Option<String>,
    pub images: Vec<SeoTargetImageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct SeoTemplateFieldMap {
    pub values: BTreeMap<String, String>,
}

impl SeoTemplateFieldMap {
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if key.trim().is_empty() || value.trim().is_empty() {
            return;
        }
        self.values.insert(key, value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeoLoadedTargetRecord {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub requested_locale: Option<String>,
    pub effective_locale: String,
    pub title: String,
    pub description: Option<String>,
    pub canonical_route: String,
    pub alternates: Vec<SeoTargetAlternateRoute>,
    pub open_graph: SeoTargetOpenGraphRecord,
    pub structured_data: JsonValue,
    pub fallback_source: String,
    #[serde(default)]
    pub template_fields: SeoTemplateFieldMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SeoBulkSummaryRecord {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub effective_locale: String,
    pub label: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SeoSitemapCandidateRecord {
    pub target_kind: SeoTargetSlug,
    pub target_id: Uuid,
    pub locale: String,
    pub route: String,
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum SeoTargetRegistryError {
    #[error("SEO target slug `{0}` is already registered")]
    DuplicateSlug(SeoTargetSlug),
}

#[async_trait]
pub trait SeoTargetProvider: Send + Sync {
    fn slug(&self) -> SeoTargetSlug;

    fn display_name(&self) -> &'static str;

    fn owner_module_slug(&self) -> &'static str;

    fn capabilities(&self) -> SeoTargetCapabilities;

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>>;

    async fn resolve_route(
        &self,
        _runtime: &SeoTargetRuntimeContext,
        _request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        Ok(None)
    }

    async fn list_bulk_summaries(
        &self,
        _runtime: &SeoTargetRuntimeContext,
        _request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        Ok(Vec::new())
    }

    async fn sitemap_candidates(
        &self,
        _runtime: &SeoTargetRuntimeContext,
        _request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Default)]
pub struct SeoTargetRegistry {
    providers: BTreeMap<SeoTargetSlug, Arc<dyn SeoTargetProvider>>,
}

impl SeoTargetRegistry {
    pub fn register<P>(&mut self, provider: P) -> Result<(), SeoTargetRegistryError>
    where
        P: SeoTargetProvider + 'static,
    {
        self.register_arc(Arc::new(provider))
    }

    pub fn register_arc(
        &mut self,
        provider: Arc<dyn SeoTargetProvider>,
    ) -> Result<(), SeoTargetRegistryError> {
        let slug = provider.slug();
        if self.providers.contains_key(&slug) {
            return Err(SeoTargetRegistryError::DuplicateSlug(slug));
        }
        self.providers.insert(slug, provider);
        Ok(())
    }

    pub fn get(&self, slug: &SeoTargetSlug) -> Option<Arc<dyn SeoTargetProvider>> {
        self.providers.get(slug).cloned()
    }

    pub fn get_by_str(&self, slug: &str) -> Option<Arc<dyn SeoTargetProvider>> {
        SeoTargetSlug::new(slug)
            .ok()
            .and_then(|slug| self.providers.get(&slug).cloned())
    }

    pub fn entries(&self) -> Vec<SeoTargetRegistryEntry> {
        self.providers
            .iter()
            .map(|(slug, provider)| SeoTargetRegistryEntry {
                slug: slug.clone(),
                display_name: provider.display_name().to_string(),
                owner_module_slug: provider.owner_module_slug().to_string(),
                capabilities: provider.capabilities(),
            })
            .collect()
    }

    pub fn entries_with_capability(
        &self,
        capability: SeoTargetCapabilityKind,
    ) -> Vec<SeoTargetRegistryEntry> {
        self.entries()
            .into_iter()
            .filter(|entry| entry.capabilities.supports(capability))
            .collect()
    }

    pub fn providers_with_capability(
        &self,
        capability: SeoTargetCapabilityKind,
    ) -> Vec<Arc<dyn SeoTargetProvider>> {
        self.providers
            .values()
            .filter(|provider| provider.capabilities().supports(capability))
            .cloned()
            .collect()
    }
}

pub fn register_seo_target_provider<P>(
    extensions: &mut ModuleRuntimeExtensions,
    provider: P,
) -> Result<(), SeoTargetRegistryError>
where
    P: SeoTargetProvider + 'static,
{
    let registry = extensions
        .get_or_insert_with::<Arc<SeoTargetRegistry>, _>(|| Arc::new(SeoTargetRegistry::default()));
    Arc::make_mut(registry).register(provider)
}

pub fn seo_target_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<SeoTargetRegistry>> {
    extensions.get::<Arc<SeoTargetRegistry>>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct DummyProvider {
        slug: &'static str,
        display_name: &'static str,
        owner_module_slug: &'static str,
        capabilities: SeoTargetCapabilities,
    }

    #[async_trait]
    impl SeoTargetProvider for DummyProvider {
        fn slug(&self) -> SeoTargetSlug {
            SeoTargetSlug::new(self.slug).expect("dummy slug should stay valid")
        }

        fn display_name(&self) -> &'static str {
            self.display_name
        }

        fn owner_module_slug(&self) -> &'static str {
            self.owner_module_slug
        }

        fn capabilities(&self) -> SeoTargetCapabilities {
            self.capabilities
        }

        async fn load_target(
            &self,
            _runtime: &SeoTargetRuntimeContext,
            request: SeoTargetLoadRequest<'_>,
        ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
            Ok(Some(SeoLoadedTargetRecord {
                target_kind: self.slug(),
                target_id: request.target_id,
                requested_locale: Some(request.locale.to_string()),
                effective_locale: request.locale.to_string(),
                title: "Demo".to_string(),
                description: Some("Demo description".to_string()),
                canonical_route: "/modules/demo".to_string(),
                alternates: vec![SeoTargetAlternateRoute {
                    locale: request.locale.to_string(),
                    route: "/modules/demo".to_string(),
                }],
                open_graph: SeoTargetOpenGraphRecord::default(),
                structured_data: json!({"@type":"Thing"}),
                fallback_source: "dummy".to_string(),
                template_fields: SeoTemplateFieldMap::default(),
            }))
        }
    }

    #[test]
    fn slug_validation_rejects_uppercase_whitespace_and_invalid_chars() {
        assert!(SeoTargetSlug::new("page").is_ok());
        assert!(SeoTargetSlug::new("blog_post").is_ok());
        assert!(SeoTargetSlug::new("forum-topic").is_ok());
        assert!(SeoTargetSlug::new("Page").is_err());
        assert!(SeoTargetSlug::new(" blog ").is_err());
        assert!(SeoTargetSlug::new("forum topic").is_err());
        assert!(SeoTargetSlug::new("forum.topic").is_err());
    }

    #[test]
    fn registry_rejects_duplicate_slugs() {
        let mut registry = SeoTargetRegistry::default();
        registry
            .register(DummyProvider {
                slug: builtin_slug::PAGE,
                display_name: "Page",
                owner_module_slug: "pages",
                capabilities: SeoTargetCapabilities::new(true, true, false, false),
            })
            .expect("first registration should succeed");

        let error = registry
            .register(DummyProvider {
                slug: builtin_slug::PAGE,
                display_name: "Page",
                owner_module_slug: "pages",
                capabilities: SeoTargetCapabilities::new(true, true, false, false),
            })
            .expect_err("duplicate registration must fail");

        assert_eq!(
            error,
            SeoTargetRegistryError::DuplicateSlug(
                SeoTargetSlug::new(builtin_slug::PAGE).expect("builtin slug stays valid")
            )
        );
    }

    #[test]
    fn registry_filters_entries_by_capability() {
        let mut registry = SeoTargetRegistry::default();
        registry
            .register(DummyProvider {
                slug: builtin_slug::PAGE,
                display_name: "Page",
                owner_module_slug: "pages",
                capabilities: SeoTargetCapabilities::new(true, true, true, true),
            })
            .expect("page provider should register");
        registry
            .register(DummyProvider {
                slug: builtin_slug::PRODUCT,
                display_name: "Product",
                owner_module_slug: "product",
                capabilities: SeoTargetCapabilities::new(true, false, true, false),
            })
            .expect("product provider should register");

        let routing = registry.entries_with_capability(SeoTargetCapabilityKind::Routing);
        let sitemaps = registry.entries_with_capability(SeoTargetCapabilityKind::Sitemaps);

        assert_eq!(routing.len(), 1);
        assert_eq!(routing[0].slug.as_str(), builtin_slug::PAGE);
        assert_eq!(routing[0].display_name, "Page");
        assert_eq!(routing[0].owner_module_slug, "pages");
        assert_eq!(sitemaps.len(), 1);
        assert_eq!(sitemaps[0].slug.as_str(), builtin_slug::PAGE);
    }

    #[test]
    fn helper_registers_registry_in_runtime_extensions() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_seo_target_provider(
            &mut extensions,
            DummyProvider {
                slug: builtin_slug::BLOG_POST,
                display_name: "Blog Post",
                owner_module_slug: "blog",
                capabilities: SeoTargetCapabilities::new(true, true, true, true),
            },
        )
        .expect("provider registration via runtime extensions should succeed");

        let registry = seo_target_registry_from_extensions(&extensions)
            .expect("SEO target registry should be stored in runtime extensions");

        assert!(registry.get_by_str(builtin_slug::BLOG_POST).is_some());
    }
}

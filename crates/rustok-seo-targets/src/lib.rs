use std::collections::BTreeMap;
use std::fmt;
#[cfg(feature = "server")]
use std::sync::Arc;

#[cfg(feature = "server")]
use anyhow::Result as AnyResult;
use async_graphql::{
    Enum, InputValueError, InputValueResult, Scalar, ScalarType, SimpleObject, Value,
};
#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::ModuleRuntimeExtensions;
#[cfg(feature = "server")]
use rustok_outbox::TransactionalEventBus;
#[cfg(feature = "server")]
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

pub mod schema {
    use serde_json::{json, Map, Value};

    pub fn web_page(name: &str, description: Option<&str>, in_language: &str) -> Value {
        base_page("WebPage", name, description, in_language)
    }

    pub fn collection_page(name: &str, description: Option<&str>, in_language: &str) -> Value {
        base_page("CollectionPage", name, description, in_language)
    }

    pub fn product(
        name: &str,
        description: Option<&str>,
        image_url: Option<&str>,
        in_language: &str,
    ) -> Value {
        let mut object = schema_object("Product");
        insert_string(&mut object, "name", Some(name));
        insert_string(&mut object, "description", description);
        insert_string(&mut object, "image", image_url);
        insert_string(&mut object, "inLanguage", Some(in_language));
        Value::Object(object)
    }

    pub fn blog_posting(
        headline: &str,
        description: Option<&str>,
        image_url: Option<&str>,
        in_language: &str,
        date_published: Option<Value>,
        date_modified: Option<Value>,
    ) -> Value {
        let mut object = schema_object("BlogPosting");
        insert_string(&mut object, "headline", Some(headline));
        insert_string(&mut object, "description", description);
        insert_string(&mut object, "image", image_url);
        insert_string(&mut object, "inLanguage", Some(in_language));
        insert_value(&mut object, "datePublished", date_published);
        insert_value(&mut object, "dateModified", date_modified);
        Value::Object(object)
    }

    pub fn discussion_forum_posting(
        headline: &str,
        article_body: &str,
        description: Option<&str>,
        in_language: &str,
        date_published: Option<Value>,
        date_modified: Option<Value>,
    ) -> Value {
        let mut object = schema_object("DiscussionForumPosting");
        insert_string(&mut object, "headline", Some(headline));
        insert_string(&mut object, "articleBody", Some(article_body));
        insert_string(&mut object, "description", description);
        insert_string(&mut object, "inLanguage", Some(in_language));
        insert_value(&mut object, "datePublished", date_published);
        insert_value(&mut object, "dateModified", date_modified);
        Value::Object(object)
    }

    pub fn offer(price: f64, price_currency: &str, availability: Option<&str>) -> Value {
        let mut object = schema_object("Offer");
        insert_number(&mut object, "price", Some(price));
        let currency = normalize_currency_code(price_currency);
        insert_string(&mut object, "priceCurrency", currency.as_deref());
        let availability = availability.and_then(normalize_schema_org_availability);
        insert_string(&mut object, "availability", availability);
        Value::Object(object)
    }

    pub fn review(
        author_name: Option<&str>,
        review_body: Option<&str>,
        rating_value: Option<f64>,
        best_rating: Option<f64>,
    ) -> Value {
        let mut object = schema_object("Review");
        insert_string(&mut object, "reviewBody", review_body);
        if let Some(author_name) = author_name.map(str::trim).filter(|value| !value.is_empty()) {
            let mut author = typed_object("Person");
            author.insert("name".to_string(), json!(author_name));
            object.insert("author".to_string(), Value::Object(author));
        }
        if rating_value.is_some() || best_rating.is_some() {
            let mut rating = typed_object("Rating");
            insert_number(&mut rating, "ratingValue", rating_value);
            insert_number(&mut rating, "bestRating", best_rating);
            if rating.len() > 1 {
                object.insert("reviewRating".to_string(), Value::Object(rating));
            }
        }
        Value::Object(object)
    }

    pub fn breadcrumb_list<I, N, U>(items: I) -> Value
    where
        I: IntoIterator<Item = (N, U)>,
        N: AsRef<str>,
        U: AsRef<str>,
    {
        let mut object = schema_object("BreadcrumbList");
        let item_list = items
            .into_iter()
            .filter_map(|(name, item)| {
                let name = name.as_ref().trim();
                let item = item.as_ref().trim();
                if name.is_empty() || item.is_empty() {
                    return None;
                }
                Some((name.to_string(), item.to_string()))
            })
            .enumerate()
            .map(|(position, (name, item))| {
                json!({
                    "@type": "ListItem",
                    "position": position + 1,
                    "name": name,
                    "item": item
                })
            })
            .collect::<Vec<_>>();
        object.insert("itemListElement".to_string(), Value::Array(item_list));
        Value::Object(object)
    }

    pub fn faq_page<I, Q, A>(questions_and_answers: I) -> Value
    where
        I: IntoIterator<Item = (Q, A)>,
        Q: AsRef<str>,
        A: AsRef<str>,
    {
        let mut object = schema_object("FAQPage");
        let entities = questions_and_answers
            .into_iter()
            .filter_map(|(question, answer)| {
                let question = question.as_ref().trim();
                let answer = answer.as_ref().trim();
                if question.is_empty() || answer.is_empty() {
                    return None;
                }
                Some(json!({
                    "@type":"Question",
                    "name": question,
                    "acceptedAnswer": {
                        "@type":"Answer",
                        "text": answer
                    }
                }))
            })
            .collect::<Vec<_>>();
        object.insert("mainEntity".to_string(), Value::Array(entities));
        Value::Object(object)
    }

    fn base_page(kind: &str, name: &str, description: Option<&str>, in_language: &str) -> Value {
        let mut object = schema_object(kind);
        insert_string(&mut object, "name", Some(name));
        insert_string(&mut object, "description", description);
        insert_string(&mut object, "inLanguage", Some(in_language));
        Value::Object(object)
    }

    fn schema_object(kind: &str) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("@context".to_string(), json!("https://schema.org"));
        object.insert("@type".to_string(), json!(kind));
        object
    }

    fn typed_object(kind: &str) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("@type".to_string(), json!(kind));
        object
    }

    fn insert_string(object: &mut Map<String, Value>, key: &str, value: Option<&str>) {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            object.insert(key.to_string(), Value::String(value.to_string()));
        }
    }

    fn insert_value(object: &mut Map<String, Value>, key: &str, value: Option<Value>) {
        if let Some(value) = value.filter(|value| !value.is_null()) {
            object.insert(key.to_string(), value);
        }
    }

    fn insert_number(object: &mut Map<String, Value>, key: &str, value: Option<f64>) {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            object.insert(key.to_string(), json!(value));
        }
    }

    fn normalize_currency_code(value: &str) -> Option<String> {
        let value = value.trim();
        if value.is_empty() || value.len() != 3 || !value.chars().all(|ch| ch.is_ascii_alphabetic())
        {
            return None;
        }
        match value.to_ascii_uppercase().as_str() {
            "XXX" => None,
            normalized => Some(normalized.to_string()),
        }
    }

    fn normalize_schema_org_availability(value: &str) -> Option<&str> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        let suffix = trimmed
            .strip_prefix("https://schema.org/")
            .or_else(|| trimmed.strip_prefix("http://schema.org/"))?;
        if matches!(
            suffix,
            "Discontinued"
                | "InStock"
                | "InStoreOnly"
                | "LimitedAvailability"
                | "OnlineOnly"
                | "OutOfStock"
                | "PreOrder"
                | "PreSale"
                | "SoldOut"
        ) {
            return Some(trimmed);
        }
        None
    }
}

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

#[cfg(feature = "server")]
#[derive(Clone)]
pub struct SeoTargetRuntimeContext {
    pub db: DatabaseConnection,
    pub event_bus: TransactionalEventBus,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SeoTargetLoadScope {
    Authoring,
    PublicRoute,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy)]
pub struct SeoTargetLoadRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
    pub target_id: Uuid,
    pub scope: SeoTargetLoadScope,
    pub channel_slug: Option<&'a str>,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy)]
pub struct SeoTargetRouteResolveRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
    pub route: &'a str,
    pub channel_slug: Option<&'a str>,
}

#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy)]
pub struct SeoTargetBulkListRequest<'a> {
    pub tenant_id: Uuid,
    pub default_locale: &'a str,
    pub locale: &'a str,
}

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum SeoTargetRegistryError {
    #[error("SEO target slug `{0}` is already registered")]
    DuplicateSlug(SeoTargetSlug),
}

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
#[derive(Clone, Default)]
pub struct SeoTargetRegistry {
    providers: BTreeMap<SeoTargetSlug, Arc<dyn SeoTargetProvider>>,
}

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
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

#[cfg(feature = "server")]
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
    fn schema_builders_emit_typed_json_ld_without_null_fields() {
        let product = schema::product("Demo", None, Some("https://cdn.test/demo.png"), "en-US");

        assert_eq!(product["@context"], json!("https://schema.org"));
        assert_eq!(product["@type"], json!("Product"));
        assert_eq!(product["name"], json!("Demo"));
        assert_eq!(product["image"], json!("https://cdn.test/demo.png"));
        assert!(product.get("description").is_none());

        let discussion = schema::discussion_forum_posting(
            "Welcome",
            "Forum body",
            Some("Intro"),
            "en-US",
            Some(json!("2026-05-01T00:00:00Z")),
            None,
        );

        assert_eq!(discussion["@type"], json!("DiscussionForumPosting"));
        assert_eq!(discussion["headline"], json!("Welcome"));
        assert_eq!(discussion["articleBody"], json!("Forum body"));
        assert_eq!(discussion["datePublished"], json!("2026-05-01T00:00:00Z"));
        assert!(discussion.get("dateModified").is_none());

        let offer = schema::offer(49.9, "USD", Some("https://schema.org/InStock"));
        assert_eq!(offer["@type"], json!("Offer"));
        assert_eq!(offer["price"], json!(49.9));
        assert_eq!(offer["priceCurrency"], json!("USD"));
        assert_eq!(offer["availability"], json!("https://schema.org/InStock"));
        let offer_without_valid_price = schema::offer(f64::NAN, " usd ", None);
        assert!(offer_without_valid_price.get("price").is_none());
        assert_eq!(offer_without_valid_price["priceCurrency"], json!("USD"));
        let offer_with_invalid_currency = schema::offer(10.0, "USDT", None);
        assert!(offer_with_invalid_currency.get("priceCurrency").is_none());
        let offer_with_no_currency = schema::offer(10.0, "XXX", None);
        assert!(offer_with_no_currency.get("priceCurrency").is_none());
        let offer_with_invalid_availability = schema::offer(10.0, "USD", Some("InStock"));
        assert!(offer_with_invalid_availability
            .get("availability")
            .is_none());
        let offer_with_http_availability =
            schema::offer(10.0, "USD", Some("http://schema.org/InStock"));
        assert_eq!(
            offer_with_http_availability["availability"],
            json!("http://schema.org/InStock")
        );
        let offer_with_unknown_availability =
            schema::offer(10.0, "USD", Some("https://schema.org/UnknownAvailability"));
        assert!(offer_with_unknown_availability
            .get("availability")
            .is_none());

        let review = schema::review(Some("Jane"), Some("Great"), Some(5.0), Some(5.0));
        assert_eq!(review["@type"], json!("Review"));
        assert_eq!(review["author"]["name"], json!("Jane"));
        assert_eq!(review["reviewRating"]["ratingValue"], json!(5.0));
        assert!(review["reviewRating"].get("@context").is_none());
        let review_with_invalid_rating = schema::review(
            Some("Jane"),
            Some("Great"),
            Some(f64::NAN),
            Some(f64::INFINITY),
        );
        assert!(review_with_invalid_rating.get("reviewRating").is_none());

        let breadcrumbs = schema::breadcrumb_list([
            ("Catalog", "https://demo.test/catalog".to_string()),
            ("Shoes", "https://demo.test/catalog/shoes".to_string()),
            (" ", "https://demo.test/catalog/ignored".to_string()),
        ]);
        assert_eq!(breadcrumbs["@type"], json!("BreadcrumbList"));
        assert_eq!(breadcrumbs["itemListElement"][0]["position"], json!(1));
        assert_eq!(
            breadcrumbs["itemListElement"].as_array().map(Vec::len),
            Some(2)
        );

        let faq = schema::faq_page([
            ("How long is shipping?", "2-3 days".to_string()),
            ("", "ignored".to_string()),
        ]);
        assert_eq!(faq["@type"], json!("FAQPage"));
        assert_eq!(faq["mainEntity"][0]["@type"], json!("Question"));
        assert_eq!(faq["mainEntity"].as_array().map(Vec::len), Some(1));
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

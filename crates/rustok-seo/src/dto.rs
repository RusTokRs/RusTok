use async_graphql::{Enum, InputObject, Json, SimpleObject};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoTargetKind {
    Page,
    Product,
    BlogPost,
    ForumCategory,
    ForumTopic,
}

impl SeoTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Product => "product",
            Self::BlogPost => "blog_post",
            Self::ForumCategory => "forum_category",
            Self::ForumTopic => "forum_topic",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "page" => Some(Self::Page),
            "product" => Some(Self::Product),
            "blog_post" => Some(Self::BlogPost),
            "forum_category" => Some(Self::ForumCategory),
            "forum_topic" => Some(Self::ForumTopic),
            _ => None,
        }
    }
}

#[derive(Enum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[graphql(rename_items = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SeoRedirectMatchType {
    Exact,
    Wildcard,
}

impl SeoRedirectMatchType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Wildcard => "wildcard",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "wildcard" => Some(Self::Wildcard),
            _ => None,
        }
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoAlternateLink {
    pub locale: String,
    pub href: String,
    pub x_default: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoRedirectDecision {
    pub target_url: String,
    pub status_code: i32,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoRouteContext {
    pub target_kind: Option<SeoTargetKind>,
    pub target_id: Option<Uuid>,
    pub requested_locale: Option<String>,
    pub effective_locale: String,
    pub canonical_url: String,
    pub redirect: Option<SeoRedirectDecision>,
    pub alternates: Vec<SeoAlternateLink>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoImageAsset {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoOpenGraph {
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub locale: Option<String>,
    pub images: Vec<SeoImageAsset>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoTwitterCard {
    pub card: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub site: Option<String>,
    pub creator: Option<String>,
    pub images: Vec<SeoImageAsset>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoVerificationTag {
    pub name: String,
    pub value: String,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoVerification {
    pub google: Vec<String>,
    pub yandex: Vec<String>,
    pub yahoo: Vec<String>,
    pub other: Vec<SeoVerificationTag>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoPagination {
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoStructuredDataBlock {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub payload: Json<Value>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoMetaTag {
    pub name: Option<String>,
    pub property: Option<String>,
    pub http_equiv: Option<String>,
    pub content: String,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoLinkTag {
    pub rel: String,
    pub href: String,
    pub hreflang: Option<String>,
    pub media: Option<String>,
    pub mime_type: Option<String>,
    pub title: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRobots {
    pub index: bool,
    pub follow: bool,
    pub noarchive: bool,
    pub nosnippet: bool,
    pub noimageindex: bool,
    pub notranslate: bool,
    pub max_snippet: Option<i32>,
    pub max_image_preview: Option<String>,
    pub max_video_preview: Option<i32>,
    pub custom: Vec<String>,
}

impl Default for SeoRobots {
    fn default() -> Self {
        Self {
            index: true,
            follow: true,
            noarchive: false,
            nosnippet: false,
            noimageindex: false,
            notranslate: false,
            max_snippet: None,
            max_image_preview: None,
            max_video_preview: None,
            custom: Vec::new(),
        }
    }
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoDocument {
    pub title: String,
    pub description: Option<String>,
    pub robots: SeoRobots,
    pub open_graph: Option<SeoOpenGraph>,
    pub twitter: Option<SeoTwitterCard>,
    pub verification: Option<SeoVerification>,
    pub pagination: Option<SeoPagination>,
    pub structured_data_blocks: Vec<SeoStructuredDataBlock>,
    pub meta_tags: Vec<SeoMetaTag>,
    pub link_tags: Vec<SeoLinkTag>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoPageContext {
    pub route: SeoRouteContext,
    pub document: SeoDocument,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaTranslationInput {
    pub locale: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaInput {
    pub target_kind: SeoTargetKind,
    pub target_id: Uuid,
    #[graphql(default)]
    pub noindex: bool,
    #[graphql(default)]
    pub nofollow: bool,
    pub canonical_url: Option<String>,
    pub structured_data: Option<Json<Value>>,
    #[graphql(default)]
    pub translations: Vec<SeoMetaTranslationInput>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone, Default)]
pub struct SeoMetaTranslationRecord {
    pub locale: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoMetaRecord {
    pub target_kind: SeoTargetKind,
    pub target_id: Uuid,
    pub requested_locale: Option<String>,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub noindex: bool,
    pub nofollow: bool,
    pub canonical_url: Option<String>,
    pub translation: SeoMetaTranslationRecord,
    pub source: String,
    pub open_graph: Option<SeoOpenGraph>,
    pub structured_data: Option<Json<Value>>,
}

#[derive(InputObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRedirectInput {
    pub id: Option<Uuid>,
    pub match_type: SeoRedirectMatchType,
    pub source_pattern: String,
    pub target_url: String,
    pub status_code: i32,
    pub expires_at: Option<DateTime<Utc>>,
    #[graphql(default = true)]
    pub is_active: bool,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRedirectRecord {
    pub id: Uuid,
    pub match_type: SeoRedirectMatchType,
    pub source_pattern: String,
    pub target_url: String,
    pub status_code: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRevisionRecord {
    pub id: Uuid,
    pub target_kind: SeoTargetKind,
    pub target_id: Uuid,
    pub revision: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoSitemapFileRecord {
    pub id: Uuid,
    pub path: String,
    pub url_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoSitemapStatusRecord {
    pub enabled: bool,
    pub latest_job_id: Option<Uuid>,
    pub status: Option<String>,
    pub file_count: i32,
    pub generated_at: Option<DateTime<Utc>>,
    pub files: Vec<SeoSitemapFileRecord>,
}

#[derive(SimpleObject, Serialize, Deserialize, Debug, Clone)]
pub struct SeoRobotsPreviewRecord {
    pub body: String,
    pub public_url: String,
    pub sitemap_index_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SeoModuleSettings {
    #[serde(default = "default_robots")]
    pub default_robots: Vec<String>,
    #[serde(default = "default_sitemap_enabled")]
    pub sitemap_enabled: bool,
    #[serde(default)]
    pub allowed_redirect_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_canonical_hosts: Vec<String>,
    #[serde(default)]
    pub x_default_locale: Option<String>,
}

impl Default for SeoModuleSettings {
    fn default() -> Self {
        Self {
            default_robots: default_robots(),
            sitemap_enabled: default_sitemap_enabled(),
            allowed_redirect_hosts: Vec::new(),
            allowed_canonical_hosts: Vec::new(),
            x_default_locale: None,
        }
    }
}

fn default_robots() -> Vec<String> {
    vec!["index".to_string(), "follow".to_string()]
}

fn default_sitemap_enabled() -> bool {
    true
}

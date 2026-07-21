use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_content::entities::node::ContentStatus;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePageInput {
    pub translations: Vec<PageTranslationInput>,
    pub template: Option<String>,
    pub body: Option<PageBodyInput>,
    pub channel_slugs: Option<Vec<String>>,
    #[serde(default)]
    pub publish: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageTranslationInput {
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageBodyInput {
    pub locale: String,
    pub content: String,
    pub format: Option<String>,
    pub content_json: Option<Value>,
}

/// Metadata-only write contract.
///
/// This command cannot carry a page body, Fly project or lifecycle transition.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PatchPageMetadataInput {
    pub expected_version: i32,
    pub translations: Option<Vec<PageTranslationInput>>,
    pub template: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

/// Current visual-document write contract.
///
/// The expected revision is the current body `updated_at` value, or
/// `page:<page_id>:initial` while the locale has no body yet.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SavePageDocumentInput {
    pub expected_revision: String,
    pub body: PageBodyInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, utoipa::IntoParams)]
pub struct ListPagesFilter {
    pub status: Option<ContentStatus>,
    pub template: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageResponse {
    pub id: Uuid,
    pub version: i32,
    pub status: ContentStatus,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub template: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translation: Option<PageTranslationResponse>,
    pub translations: Vec<PageTranslationResponse>,
    pub body: Option<PageBodyResponse>,
    pub channel_slugs: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageTranslationResponse {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageBodyResponse {
    pub locale: String,
    pub content: String,
    pub format: String,
    pub content_json: Option<Value>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PageListItem {
    pub id: Uuid,
    pub status: ContentStatus,
    pub template: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub channel_slugs: Vec<String>,
    pub updated_at: String,
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_content::entities::node::ContentStatus;

use super::{BlockResponse, CreateBlockInput};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePageInput {
    pub translations: Vec<PageTranslationInput>,
    pub template: Option<String>,
    pub body: Option<PageBodyInput>,
    pub blocks: Option<Vec<CreateBlockInput>>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdatePageInput {
    pub translations: Option<Vec<PageTranslationInput>>,
    pub template: Option<String>,
    pub body: Option<PageBodyInput>,
    pub status: Option<ContentStatus>,
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
    pub status: ContentStatus,
    pub template: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translations: Vec<PageTranslationResponse>,
    pub body: Option<PageBodyResponse>,
    pub blocks: Vec<BlockResponse>,
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
    pub updated_at: String,
}

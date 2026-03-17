use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTagInput {
    pub locale: String,
    #[schema(max_length = 100)]
    pub name: String,
    #[schema(max_length = 100)]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateTagInput {
    pub locale: String,
    #[schema(max_length = 100)]
    pub name: Option<String>,
    #[schema(max_length = 100)]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TagResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub use_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TagListItem {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub use_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, IntoParams)]
pub struct ListTagsFilter {
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

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateCategoryInput {
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: Option<i32>,
    pub moderated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateCategoryInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    /// Compatibility field retained for decoding only. The owner service rejects it;
    /// use `MoveCategoryInput` or `ReorderCategorySiblingsInput` instead.
    pub position: Option<i32>,
    pub moderated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryResponse {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub topic_count: i32,
    pub reply_count: i32,
    pub moderated: bool,
    pub archived_at: Option<String>,
    pub is_archived: bool,
    pub is_subscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryListItem {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub moderated: bool,
    pub archived_at: Option<String>,
    pub is_archived: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub is_subscribed: bool,
}

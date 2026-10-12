use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request payload to create a collaborative page / document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePageDto {
    pub title: String,
    pub slug: Option<String>,
    pub parent_id: Option<Uuid>,
    pub icon: Option<String>,
    pub cover_image: Option<String>,
}

/// Request payload to update page metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePageDto {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub parent_id: Option<Option<Uuid>>,
    pub icon: Option<Option<String>>,
    pub cover_image: Option<Option<String>>,
    pub is_archived: Option<bool>,
}

/// Summary representation of a collaborative page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSummaryDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub slug: String,
    pub title: String,
    pub icon: Option<String>,
    pub cover_image: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Hierarchical tree node for nested page navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTreeNodeDto {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_archived: bool,
    pub children: Vec<PageTreeNodeDto>,
}

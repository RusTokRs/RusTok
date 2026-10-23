use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{DocsError, DocsResult};

/// Request payload to create a collaborative page / document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePageDto {
    pub title: String,
    pub slug: Option<String>,
    pub parent_id: Option<Uuid>,
    pub icon: Option<String>,
    pub cover_image: Option<String>,
}

impl CreatePageDto {
    pub fn validate(&self) -> DocsResult<()> {
        let trimmed_title = self.title.trim();
        if trimmed_title.is_empty() {
            return Err(DocsError::Validation("Page title cannot be empty".to_string()));
        }
        if trimmed_title.chars().count() > 255 {
            return Err(DocsError::Validation(
                "Page title cannot exceed 255 characters".to_string(),
            ));
        }
        if let Some(ref slug) = self.slug {
            let trimmed_slug = slug.trim();
            if trimmed_slug.is_empty() {
                return Err(DocsError::Validation(
                    "Page slug cannot be empty when provided".to_string(),
                ));
            }
            if trimmed_slug.chars().count() > 255 {
                return Err(DocsError::Validation(
                    "Page slug cannot exceed 255 characters".to_string(),
                ));
            }
        }
        if let Some(parent_id) = self.parent_id {
            if parent_id.is_nil() {
                return Err(DocsError::Validation(
                    "Parent page ID cannot be a nil UUID".to_string(),
                ));
            }
        }
        Ok(())
    }
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

impl UpdatePageDto {
    pub fn validate(&self) -> DocsResult<()> {
        if let Some(ref title) = self.title {
            let trimmed_title = title.trim();
            if trimmed_title.is_empty() {
                return Err(DocsError::Validation("Page title cannot be empty".to_string()));
            }
            if trimmed_title.chars().count() > 255 {
                return Err(DocsError::Validation(
                    "Page title cannot exceed 255 characters".to_string(),
                ));
            }
        }
        if let Some(ref slug) = self.slug {
            let trimmed_slug = slug.trim();
            if trimmed_slug.is_empty() {
                return Err(DocsError::Validation("Page slug cannot be empty".to_string()));
            }
            if trimmed_slug.chars().count() > 255 {
                return Err(DocsError::Validation(
                    "Page slug cannot exceed 255 characters".to_string(),
                ));
            }
        }
        if let Some(Some(parent_id)) = self.parent_id {
            if parent_id.is_nil() {
                return Err(DocsError::Validation(
                    "Parent page ID cannot be a nil UUID".to_string(),
                ));
            }
        }
        Ok(())
    }
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

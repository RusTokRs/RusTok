use async_graphql::{InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::dto::{MediaItem, MediaTranslationItem};

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMediaItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub uploaded_by: Option<Uuid>,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_driver: String,
    pub public_url: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl From<MediaItem> for GqlMediaItem {
    fn from(item: MediaItem) -> Self {
        Self {
            id: item.id,
            tenant_id: item.tenant_id,
            uploaded_by: item.uploaded_by,
            filename: item.filename,
            original_name: item.original_name,
            mime_type: item.mime_type,
            size: item.size,
            storage_driver: item.storage_driver,
            public_url: item.public_url,
            width: item.width,
            height: item.height,
            created_at: item.created_at,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMediaList {
    pub items: Vec<GqlMediaItem>,
    pub total: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MediaUsageStats {
    pub tenant_id: Uuid,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMediaTranslation {
    pub id: Uuid,
    pub media_id: Uuid,
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

impl From<MediaTranslationItem> for GqlMediaTranslation {
    fn from(translation: MediaTranslationItem) -> Self {
        Self {
            id: translation.id,
            media_id: translation.media_id,
            locale: translation.locale,
            title: translation.title,
            alt_text: translation.alt_text,
            caption: translation.caption,
        }
    }
}

#[derive(InputObject, Clone, Debug)]
pub struct UpsertMediaTranslationInput {
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

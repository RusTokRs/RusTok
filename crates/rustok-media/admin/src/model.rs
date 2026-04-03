use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaUsageSnapshot {
    pub tenant_id: String,
    pub file_count: i64,
    pub total_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaListPayload {
    pub items: Vec<MediaListItem>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaListItem {
    pub id: String,
    pub tenant_id: String,
    pub uploaded_by: Option<String>,
    pub filename: String,
    pub original_name: String,
    pub mime_type: String,
    pub size: i64,
    pub storage_driver: String,
    pub public_url: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTranslationPayload {
    pub id: String,
    pub media_id: String,
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTranslationPayload {
    pub locale: String,
    pub title: Option<String>,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

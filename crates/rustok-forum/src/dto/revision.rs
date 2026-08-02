use rustok_api::RichTextView;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicRevisionResponse {
    pub id: i64,
    pub topic_id: Uuid,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub body: RichTextView,
    pub metadata: Value,
    pub revision_reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyRevisionResponse {
    pub id: i64,
    pub reply_id: Uuid,
    pub locale: String,
    pub body: RichTextView,
    pub revision_reason: String,
    pub created_at: String,
}

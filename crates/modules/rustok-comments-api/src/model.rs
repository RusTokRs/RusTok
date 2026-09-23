use rustok_api::{RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    Pending,
    Approved,
    Spam,
    Trash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommentInput {
    pub target_type: String,
    pub target_id: Uuid,
    pub locale: String,
    pub body: RichTextDocument,
    pub parent_comment_id: Option<Uuid>,
    pub status: CommentStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCommentInput {
    pub locale: String,
    pub body: Option<RichTextDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCommentsFilter {
    pub locale: String,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentRecord {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub author_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub body: RichTextView,
    pub body_text: String,
    pub status: CommentStatus,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentListItem {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub author_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub body_preview: String,
    pub status: CommentStatus,
    pub position: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetCommentStatusRequest {
    pub status: CommentStatus,
    pub fallback_locale: Option<String>,
}

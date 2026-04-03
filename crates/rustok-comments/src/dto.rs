use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum CommentThreadStatus {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "closed")]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "spam")]
    Spam,
    #[sea_orm(string_value = "trash")]
    Trash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommentInput {
    pub target_type: String,
    pub target_id: Uuid,
    pub locale: String,
    pub body: String,
    pub body_format: String,
    pub parent_comment_id: Option<Uuid>,
    pub status: CommentStatus,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCommentInput {
    pub locale: String,
    pub body: Option<String>,
    pub body_format: Option<String>,
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
    pub body: String,
    pub body_format: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThreadSummary {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub status: CommentThreadStatus,
    pub comment_count: i32,
    pub last_commented_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThreadDetail {
    pub thread: CommentThreadSummary,
    pub comments: Vec<CommentRecord>,
    pub total_comments: u64,
}

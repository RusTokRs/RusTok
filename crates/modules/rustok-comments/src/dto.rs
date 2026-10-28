use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::{RichTextDocument, RichTextView};

#[cfg(feature = "server")]
use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(EnumIter, DeriveActiveEnum))]
#[cfg_attr(
    feature = "server",
    sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")
)]
#[serde(rename_all = "snake_case")]
pub enum CommentThreadStatus {
    #[cfg_attr(feature = "server", sea_orm(string_value = "open"))]
    Open,
    #[cfg_attr(feature = "server", sea_orm(string_value = "closed"))]
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(EnumIter, DeriveActiveEnum))]
#[cfg_attr(
    feature = "server",
    sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")
)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    #[cfg_attr(feature = "server", sea_orm(string_value = "pending"))]
    Pending,
    #[cfg_attr(feature = "server", sea_orm(string_value = "approved"))]
    Approved,
    #[cfg_attr(feature = "server", sea_orm(string_value = "spam"))]
    Spam,
    #[cfg_attr(feature = "server", sea_orm(string_value = "trash"))]
    Trash,
}

impl CommentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Spam => "spam",
            Self::Trash => "trash",
        }
    }

    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }

    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

impl std::fmt::Display for CommentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetCommentStatusRequest {
    pub status: CommentStatus,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListThreadsFilter {
    pub page: u64,
    pub per_page: u64,
    pub target_type: Option<String>,
    pub thread_status: Option<CommentThreadStatus>,
    pub comment_status: Option<CommentStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetThreadDetailFilter {
    pub thread_id: Uuid,
    pub locale: String,
    pub fallback_locale: Option<String>,
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

#[cfg(feature = "server")]
use crate::error::{CommentsError, CommentsResult};

#[cfg(feature = "server")]
impl CreateCommentInput {
    pub fn validate(&self) -> CommentsResult<()> {
        if self.target_type.trim().is_empty() {
            return Err(CommentsError::Validation(
                "target_type cannot be empty".to_string(),
            ));
        }
        if self.target_id.is_nil() {
            return Err(CommentsError::Validation(
                "target_id cannot be nil".to_string(),
            ));
        }
        if self.parent_comment_id.is_some_and(|id| id.is_nil()) {
            return Err(CommentsError::Validation(
                "parent_comment_id cannot be nil".to_string(),
            ));
        }
        if self.locale.trim().is_empty() {
            return Err(CommentsError::Validation(
                "locale cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "server")]
impl UpdateCommentInput {
    pub fn validate(&self) -> CommentsResult<()> {
        if self.locale.trim().is_empty() {
            return Err(CommentsError::Validation(
                "locale cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "server")]
impl ListCommentsFilter {
    pub fn validate(&self) -> CommentsResult<()> {
        if self.locale.trim().is_empty() {
            return Err(CommentsError::Validation(
                "locale cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "server")]
impl GetThreadDetailFilter {
    pub fn validate(&self) -> CommentsResult<()> {
        if self.thread_id.is_nil() {
            return Err(CommentsError::Validation(
                "thread_id cannot be nil".to_string(),
            ));
        }
        if self.locale.trim().is_empty() {
            return Err(CommentsError::Validation(
                "locale cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn create_comment_input_validate_rejects_empty_target_type() {
        let input = CreateCommentInput {
            target_type: "".to_string(),
            target_id: Uuid::new_v4(),
            locale: "en".to_string(),
            body: RichTextDocument::empty(),
            parent_comment_id: None,
            status: CommentStatus::Pending,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_comment_input_validate_rejects_nil_target_id() {
        let input = CreateCommentInput {
            target_type: "post".to_string(),
            target_id: Uuid::nil(),
            locale: "en".to_string(),
            body: RichTextDocument::empty(),
            parent_comment_id: None,
            status: CommentStatus::Pending,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_comment_input_validate_rejects_nil_parent_comment_id() {
        let input = CreateCommentInput {
            target_type: "post".to_string(),
            target_id: Uuid::new_v4(),
            locale: "en".to_string(),
            body: RichTextDocument::empty(),
            parent_comment_id: Some(Uuid::nil()),
            status: CommentStatus::Pending,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_comment_input_validate_accepts_valid() {
        let input = CreateCommentInput {
            target_type: "post".to_string(),
            target_id: Uuid::new_v4(),
            locale: "en".to_string(),
            body: RichTextDocument::empty(),
            parent_comment_id: None,
            status: CommentStatus::Pending,
        };
        assert!(input.validate().is_ok());
    }
}

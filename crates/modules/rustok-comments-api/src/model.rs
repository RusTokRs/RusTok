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

impl CreateCommentInput {
    pub fn validate(&self) -> Result<(), rustok_api::PortError> {
        if self.target_type.trim().is_empty() {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_target_type",
                "target_type cannot be empty",
            ));
        }
        if self.target_id.is_nil() {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_target_id",
                "target_id cannot be nil",
            ));
        }
        if self.parent_comment_id.is_some_and(|id| id.is_nil()) {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_parent_comment_id",
                "parent_comment_id cannot be nil",
            ));
        }
        if self.locale.trim().is_empty() {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_locale",
                "locale cannot be empty",
            ));
        }
        Ok(())
    }
}

impl UpdateCommentInput {
    pub fn validate(&self) -> Result<(), rustok_api::PortError> {
        if self.locale.trim().is_empty() {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_locale",
                "locale cannot be empty",
            ));
        }
        Ok(())
    }
}

impl ListCommentsFilter {
    pub fn validate(&self) -> Result<(), rustok_api::PortError> {
        if self.locale.trim().is_empty() {
            return Err(rustok_api::PortError::validation(
                "comments.invalid_locale",
                "locale cannot be empty",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_status_display_and_as_str() {
        assert_eq!(CommentStatus::Pending.as_str(), "pending");
        assert_eq!(CommentStatus::Approved.as_str(), "approved");
        assert_eq!(CommentStatus::Spam.as_str(), "spam");
        assert_eq!(CommentStatus::Trash.as_str(), "trash");
        assert_eq!(CommentStatus::Pending.to_string(), "pending");
        assert!(CommentStatus::Approved.is_approved());
        assert!(!CommentStatus::Pending.is_approved());
        assert!(CommentStatus::Pending.is_pending());
        assert!(!CommentStatus::Approved.is_pending());
    }

    #[test]
    fn create_comment_input_validation() {
        let mut input = CreateCommentInput {
            target_type: "blog_post".to_string(),
            target_id: Uuid::new_v4(),
            locale: "en".to_string(),
            body: RichTextDocument::empty(),
            parent_comment_id: None,
            status: CommentStatus::Pending,
        };
        assert!(input.validate().is_ok());

        input.target_type = "   ".to_string();
        assert_eq!(
            input.validate().unwrap_err().code,
            "comments.invalid_target_type"
        );

        input.target_type = "blog_post".to_string();
        input.target_id = Uuid::nil();
        assert_eq!(
            input.validate().unwrap_err().code,
            "comments.invalid_target_id"
        );

        input.target_id = Uuid::new_v4();
        input.parent_comment_id = Some(Uuid::nil());
        assert_eq!(
            input.validate().unwrap_err().code,
            "comments.invalid_parent_comment_id"
        );

        input.parent_comment_id = None;
        input.locale = "".to_string();
        assert_eq!(
            input.validate().unwrap_err().code,
            "comments.invalid_locale"
        );
    }

    #[test]
    fn update_comment_input_validation() {
        let mut input = UpdateCommentInput {
            locale: "en".to_string(),
            body: None,
        };
        assert!(input.validate().is_ok());

        input.locale = "  ".to_string();
        assert_eq!(
            input.validate().unwrap_err().code,
            "comments.invalid_locale"
        );
    }

    #[test]
    fn list_comments_filter_validation() {
        let mut filter = ListCommentsFilter {
            locale: "en".to_string(),
            page: 1,
            per_page: 20,
        };
        assert!(filter.validate().is_ok());

        filter.locale = "".to_string();
        assert_eq!(
            filter.validate().unwrap_err().code,
            "comments.invalid_locale"
        );
    }

    #[test]
    fn serde_roundtrip_comment_status() {
        let serialized = serde_json::to_string(&CommentStatus::Approved).unwrap();
        assert_eq!(serialized, "\"approved\"");
        let deserialized: CommentStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, CommentStatus::Approved);
    }
}

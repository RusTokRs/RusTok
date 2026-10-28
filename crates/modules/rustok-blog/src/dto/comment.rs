use serde::{Deserialize, Serialize};

use rustok_api::{RichTextDocument, RichTextView};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateCommentInput {
    /// Stable identity of this logical create command. Reuse across retries.
    pub command_id: Uuid,
    pub locale: String,
    pub content: RichTextDocument,
    pub parent_comment_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateCommentInput {
    /// Stable identity of this logical update command. Reuse across retries.
    pub command_id: Uuid,
    pub locale: String,
    pub content: Option<RichTextDocument>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModerateCommentStatus {
    Approved,
    Spam,
    Trash,
}

impl From<ModerateCommentStatus> for rustok_comments::CommentStatus {
    fn from(value: ModerateCommentStatus) -> Self {
        match value {
            ModerateCommentStatus::Approved => Self::Approved,
            ModerateCommentStatus::Spam => Self::Spam,
            ModerateCommentStatus::Trash => Self::Trash,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModerateCommentInput {
    /// Stable identity of this logical moderation command. Reuse across retries.
    pub command_id: Uuid,
    pub status: ModerateCommentStatus,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, IntoParams)]
pub struct ListCommentsFilter {
    pub locale: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

/// Upper bound of one comment list page. Every Blog transport that already
/// clamps (GraphQL moderation, the admin adapter, the public snapshot read)
/// uses 100; the bound is applied in the Blog comment service, at the
/// owner-service boundary, so a transport that forgets to clamp (the REST
/// controller passes the query through) cannot ask the Comments owner for
/// an unbounded page.
pub(crate) const MAX_COMMENTS_PER_PAGE: u64 = 100;

/// Bounds a comment list page request: pages are 1-based and one page holds
/// at most [`MAX_COMMENTS_PER_PAGE`] comments.
pub(crate) fn bounded_comment_page(page: u64, per_page: u64) -> (u64, u64) {
    (page.max(1), per_page.clamp(1, MAX_COMMENTS_PER_PAGE))
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentResponse {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub post_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: RichTextView,
    pub content_text: String,
    pub status: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentListItem {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub post_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content_preview: String,
    pub status: String,
    pub parent_comment_id: Option<Uuid>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::{CommentResponse, ModerateCommentInput, ModerateCommentStatus};
    use rustok_api::{RichTextDocument, RichTextView};
    use serde_json::json;
    use uuid::Uuid;

    fn sample() -> CommentResponse {
        let document: RichTextDocument = serde_json::from_value(json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "plain"}]
            }]
        }))
        .expect("test richtext");
        CommentResponse {
            id: Uuid::new_v4(),
            requested_locale: "en".into(),
            locale: "en".into(),
            effective_locale: "en".into(),
            post_id: Uuid::new_v4(),
            author_id: None,
            content: RichTextView {
                document,
                html: "<p class=\"richtext-paragraph\">plain</p>".into(),
            },
            content_text: "plain".into(),
            status: "pending".into(),
            parent_comment_id: None,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn comment_response_serde_uses_one_richtext_contract() {
        let r = sample();
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["content"]["document"]["type"], "doc");
        assert_eq!(v["content_text"], "plain");
        assert!(v.get("content_format").is_none());
        assert!(v.get("content_json").is_none());
        let d: CommentResponse = serde_json::from_value(v).expect("deserialize");
        assert_eq!(d.content.document.kind, "doc");
        assert_eq!(d.content_text, "plain");
    }

    #[test]
    fn comment_pages_are_one_based_and_bounded() {
        use super::{MAX_COMMENTS_PER_PAGE, bounded_comment_page};

        assert_eq!(bounded_comment_page(0, 20), (1, 20));
        assert_eq!(bounded_comment_page(2, 0), (2, 1));
        assert_eq!(
            bounded_comment_page(1, MAX_COMMENTS_PER_PAGE),
            (1, MAX_COMMENTS_PER_PAGE)
        );
        assert_eq!(
            bounded_comment_page(1, u64::MAX),
            (1, MAX_COMMENTS_PER_PAGE),
            "an unbounded page size is clamped, not honoured"
        );
    }

    #[test]
    fn moderate_comment_input_serde_snake_case_status() {
        let payload = ModerateCommentInput {
            command_id: Uuid::new_v4(),
            status: ModerateCommentStatus::Approved,
            locale: Some("en".to_string()),
        };

        let value = serde_json::to_value(payload).expect("serialize moderation payload");
        assert_eq!(value["status"], "approved");
        assert_eq!(value["locale"], "en");
    }
}

use serde::{Deserialize, Serialize};

use rustok_api::{RichTextDocument, RichTextView};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::ForumQuoteReferenceInput;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateReplyInput {
    pub locale: String,
    pub content: RichTextDocument,
    pub parent_reply_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateReplyCommandInput {
    pub locale: String,
    pub content: RichTextDocument,
    pub parent_reply_id: Option<Uuid>,
    #[serde(default)]
    pub quotes: Vec<ForumQuoteReferenceInput>,
}

impl CreateReplyCommandInput {
    pub fn into_parts(self) -> (CreateReplyInput, Vec<ForumQuoteReferenceInput>) {
        (
            CreateReplyInput {
                locale: self.locale,
                content: self.content,
                parent_reply_id: self.parent_reply_id,
            },
            self.quotes,
        )
    }
}

impl From<CreateReplyInput> for CreateReplyCommandInput {
    fn from(input: CreateReplyInput) -> Self {
        Self {
            locale: input.locale,
            content: input.content,
            parent_reply_id: input.parent_reply_id,
            quotes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateReplyInput {
    pub locale: String,
    pub content: Option<RichTextDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateReplyCommandInput {
    pub locale: String,
    pub content: Option<RichTextDocument>,
    pub quotes: Option<Vec<ForumQuoteReferenceInput>>,
}

impl UpdateReplyCommandInput {
    pub fn into_parts(self) -> (UpdateReplyInput, Option<Vec<ForumQuoteReferenceInput>>) {
        (
            UpdateReplyInput {
                locale: self.locale,
                content: self.content,
            },
            self.quotes,
        )
    }
}

impl From<UpdateReplyInput> for UpdateReplyCommandInput {
    fn from(input: UpdateReplyInput) -> Self {
        Self {
            locale: input.locale,
            content: input.content,
            quotes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListRepliesFilter {
    pub locale: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(
        default = "default_per_page",
        deserialize_with = "crate::dto::deserialize_forum_read_limit"
    )]
    pub per_page: u64,
}

impl Default for ListRepliesFilter {
    fn default() -> Self {
        Self {
            locale: None,
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    crate::dto::DEFAULT_FORUM_READ_LIMIT
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyResponse {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: RichTextView,
    pub content_plain_text: String,
    pub status: String,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_solution: bool,
    pub parent_reply_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyListItem {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content_preview: String,
    pub status: String,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_solution: bool,
    pub parent_reply_id: Option<Uuid>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::{ListRepliesFilter, ReplyResponse, UpdateReplyCommandInput};
    use rustok_api::{RichTextDocument, RichTextView};
    use serde_json::json;
    use uuid::Uuid;

    fn sample(content: &str) -> ReplyResponse {
        let document = RichTextDocument::single_paragraph(content);
        ReplyResponse {
            id: Uuid::new_v4(),
            requested_locale: "en".into(),
            locale: "en".into(),
            effective_locale: "en".into(),
            topic_id: Uuid::new_v4(),
            author_id: None,
            content: RichTextView {
                document,
                html: format!("<p class=\"richtext-paragraph\">{content}</p>"),
            },
            content_plain_text: content.to_string(),
            status: "approved".into(),
            vote_score: 0,
            current_user_vote: None,
            is_solution: false,
            parent_reply_id: None,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn list_replies_filter_caps_external_page_size() {
        let filter: ListRepliesFilter =
            serde_json::from_value(json!({"per_page": 50_000})).expect("deserialize page size");
        assert_eq!(filter.per_page, crate::dto::MAX_FORUM_READ_LIMIT);
    }

    #[test]
    fn update_command_distinguishes_omitted_quotes_from_explicit_clear() {
        let omitted: UpdateReplyCommandInput = serde_json::from_value(json!({"locale": "en"}))
            .expect("omitted quotes should deserialize");
        assert!(omitted.quotes.is_none());

        let clear: UpdateReplyCommandInput =
            serde_json::from_value(json!({"locale": "en", "quotes": []}))
                .expect("explicit clear should deserialize");
        assert_eq!(clear.quotes, Some(Vec::new()));
    }

    #[test]
    fn reply_response_serde_uses_one_richtext_view() {
        let r = sample("plain");
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["content"]["document"]["type"], "doc");
        assert_eq!(
            v["content"]["html"],
            "<p class=\"richtext-paragraph\">plain</p>"
        );
        assert!(v.get("content_format").is_none());
        assert!(v.get("content_json").is_none());
        let d: ReplyResponse = serde_json::from_value(v).expect("deserialize");
        assert_eq!(
            d.content.document,
            RichTextDocument::single_paragraph("plain")
        );
    }
}

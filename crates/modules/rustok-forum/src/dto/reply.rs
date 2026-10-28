use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::{Deserialize, Serialize};

use rustok_api::{RichTextDocument, RichTextView};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::ForumQuoteReferenceInput;
use crate::error::{ForumError, ForumResult};

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

/// Keyset position of a reply list: `(created_at, id)` of the last reply on the
/// previous page. Ordering is `created_at ASC, id ASC`. Reply ids are UUIDv7, so the
/// id breaks ties in creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyCursor {
    pub created_at: DateTime<FixedOffset>,
    pub id: Uuid,
}

impl ReplyCursor {
    /// Opaque, URL-safe token. Clients must not parse it.
    pub fn encode(&self) -> String {
        let raw = format!(
            "{}|{}",
            self.created_at.to_rfc3339_opts(SecondsFormat::Nanos, false),
            self.id
        );
        URL_SAFE_NO_PAD.encode(raw)
    }

    pub fn decode(value: &str) -> ForumResult<Self> {
        let invalid = || ForumError::Validation("Forum reply list cursor is invalid".to_string());
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| invalid())?;
        let raw = String::from_utf8(bytes).map_err(|_| invalid())?;
        let (created_at, id) = raw.split_once('|').ok_or_else(invalid)?;
        Ok(Self {
            created_at: DateTime::parse_from_rfc3339(created_at).map_err(|_| invalid())?,
            id: Uuid::parse_str(id).map_err(|_| invalid())?,
        })
    }
}

/// One keyset page of replies. `next_cursor` is present only when another page exists;
/// the list never counts rows.
#[derive(Debug, Clone)]
pub struct ReplyPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// REST body of the reply list: one page of list items plus the cursor for the next.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyListItemPage {
    pub items: Vec<ReplyListItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListRepliesFilter {
    pub locale: Option<String>,
    /// Opaque cursor returned as `next_cursor` by the previous page.
    pub after: Option<String>,
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
            after: None,
            per_page: default_per_page(),
        }
    }
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
    pub available_locales: Vec<String>,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: RichTextView,
    pub content_plain_text: String,
    pub status: String,
    pub is_deleted: bool,
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
    pub available_locales: Vec<String>,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content_preview: String,
    pub status: String,
    pub is_deleted: bool,
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
            available_locales: vec!["en".into()],
            topic_id: Uuid::new_v4(),
            author_id: None,
            content: RichTextView {
                document,
                html: format!("<p class=\"richtext-paragraph\">{content}</p>"),
            },
            content_plain_text: content.to_string(),
            status: "approved".into(),
            is_deleted: false,
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

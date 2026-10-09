use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, FixedOffset, SecondsFormat};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustok_api::{RichTextDocument, RichTextView};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::ForumQuoteReferenceInput;
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTopicInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: RichTextDocument,
    #[serde(default)]
    pub metadata: Value,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTopicCommandInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: RichTextDocument,
    #[serde(default)]
    pub metadata: Value,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
    #[serde(default)]
    pub quotes: Vec<ForumQuoteReferenceInput>,
}

impl CreateTopicCommandInput {
    pub fn into_parts(self) -> (CreateTopicInput, Vec<ForumQuoteReferenceInput>) {
        (
            CreateTopicInput {
                locale: self.locale,
                category_id: self.category_id,
                title: self.title,
                slug: self.slug,
                body: self.body,
                metadata: self.metadata,
                tags: self.tags,
                channel_slugs: self.channel_slugs,
            },
            self.quotes,
        )
    }
}

impl From<CreateTopicInput> for CreateTopicCommandInput {
    fn from(input: CreateTopicInput) -> Self {
        Self {
            locale: input.locale,
            category_id: input.category_id,
            title: input.title,
            slug: input.slug,
            body: input.body,
            metadata: input.metadata,
            tags: input.tags,
            channel_slugs: input.channel_slugs,
            quotes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateTopicInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<RichTextDocument>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateTopicCommandInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<RichTextDocument>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
    pub quotes: Option<Vec<ForumQuoteReferenceInput>>,
}

impl UpdateTopicCommandInput {
    pub fn into_parts(self) -> (UpdateTopicInput, Option<Vec<ForumQuoteReferenceInput>>) {
        (
            UpdateTopicInput {
                locale: self.locale,
                title: self.title,
                body: self.body,
                metadata: self.metadata,
                tags: self.tags,
                channel_slugs: self.channel_slugs,
            },
            self.quotes,
        )
    }
}

impl From<UpdateTopicInput> for UpdateTopicCommandInput {
    fn from(input: UpdateTopicInput) -> Self {
        Self {
            locale: input.locale,
            title: input.title,
            body: input.body,
            metadata: input.metadata,
            tags: input.tags,
            channel_slugs: input.channel_slugs,
            quotes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListTopicsFilter {
    pub category_id: Option<Uuid>,
    pub status: Option<TopicStatus>,
    pub locale: Option<String>,
    /// Opaque cursor returned as `next_cursor` by the previous page.
    pub after: Option<String>,
    #[serde(
        default = "default_per_page",
        deserialize_with = "crate::dto::deserialize_forum_read_limit"
    )]
    pub per_page: u64,
}

impl Default for ListTopicsFilter {
    fn default() -> Self {
        Self {
            category_id: None,
            status: None,
            locale: None,
            after: None,
            per_page: default_per_page(),
        }
    }
}

/// Keyset position of a topic list. The order is
/// `is_pinned DESC, last_reply_at DESC NULLS LAST, updated_at DESC, id DESC`. The cursor
/// carries the sort key of the last topic on the previous page plus its id as the final
/// tie-breaker, so pages neither repeat nor skip topics when sort keys tie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopicListCursor {
    pub is_pinned: bool,
    pub last_reply_at: Option<DateTime<FixedOffset>>,
    pub updated_at: DateTime<FixedOffset>,
    pub id: Uuid,
}

impl TopicListCursor {
    /// Opaque, URL-safe token. Clients must not parse it.
    pub fn encode(&self) -> String {
        let last_reply_at = self
            .last_reply_at
            .map(|value| value.to_rfc3339_opts(SecondsFormat::Nanos, false))
            .unwrap_or_else(|| "-".to_string());
        let raw = format!(
            "v1|{}|{}|{}|{}",
            u8::from(self.is_pinned),
            last_reply_at,
            self.updated_at.to_rfc3339_opts(SecondsFormat::Nanos, false),
            self.id
        );
        URL_SAFE_NO_PAD.encode(raw)
    }

    pub fn decode(value: &str) -> ForumResult<Self> {
        let invalid = || ForumError::Validation("Forum topic list cursor is invalid".to_string());
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| invalid())?;
        let raw = String::from_utf8(bytes).map_err(|_| invalid())?;
        let parts: Vec<&str> = raw.split('|').collect();
        let [version, is_pinned, last_reply_at, updated_at, id] = parts.as_slice() else {
            return Err(invalid());
        };
        if *version != "v1" {
            return Err(invalid());
        }
        let is_pinned = match *is_pinned {
            "0" => false,
            "1" => true,
            _ => return Err(invalid()),
        };
        let last_reply_at = match *last_reply_at {
            "-" => None,
            raw_value => Some(DateTime::parse_from_rfc3339(raw_value).map_err(|_| invalid())?),
        };
        Ok(Self {
            is_pinned,
            last_reply_at,
            updated_at: DateTime::parse_from_rfc3339(updated_at).map_err(|_| invalid())?,
            id: Uuid::parse_str(id).map_err(|_| invalid())?,
        })
    }
}

/// One keyset page of topics. `next_cursor` is present only when another page exists;
/// the list never counts rows.
#[derive(Debug, Clone)]
pub struct TopicPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// REST body of the topic list: one page of list items plus the cursor for the next.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicListItemPage {
    pub items: Vec<TopicListItem>,
    pub next_cursor: Option<String>,
}

fn default_per_page() -> u64 {
    crate::dto::DEFAULT_FORUM_READ_LIMIT
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicResponse {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub body: RichTextView,
    pub body_plain_text: String,
    pub metadata: Value,
    pub status: String,
    pub is_deleted: bool,
    pub tags: Vec<String>,
    pub channel_slugs: Vec<String>,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicListItem {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub metadata: Value,
    pub status: String,
    pub is_deleted: bool,
    pub channel_slugs: Vec<String>,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    /// Keyset sort key of this row, set by the owner listing. Never serialized.
    #[serde(skip)]
    pub(crate) sort_key: Option<TopicListCursor>,
}

#[cfg(test)]
mod tests {
    use super::{ListTopicsFilter, TopicResponse, UpdateTopicCommandInput};
    use crate::state_machine::TopicStatus;
    use rustok_api::{RichTextDocument, RichTextView};
    use serde_json::json;
    use uuid::Uuid;

    fn sample(body: &str) -> TopicResponse {
        let document = RichTextDocument::single_paragraph(body);
        TopicResponse {
            id: Uuid::new_v4(),
            requested_locale: "en".into(),
            locale: "en".into(),
            effective_locale: "en".into(),
            available_locales: vec!["en".into()],
            category_id: Uuid::new_v4(),
            author_id: None,
            title: "title".into(),
            slug: "slug".into(),
            body: RichTextView {
                document,
                html: format!("<p class=\"richtext-paragraph\">{body}</p>"),
            },
            body_plain_text: body.to_string(),
            metadata: json!({}),
            status: "open".into(),
            is_deleted: false,
            tags: vec![],
            channel_slugs: vec![],
            vote_score: 0,
            current_user_vote: None,
            is_subscribed: false,
            solution_reply_id: None,
            is_pinned: false,
            is_locked: false,
            reply_count: 0,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn list_topics_filter_uses_typed_status_wire_value() {
        let filter: ListTopicsFilter =
            serde_json::from_value(json!({"status": "closed"})).expect("deserialize status");
        assert_eq!(filter.status, Some(TopicStatus::Closed));
        assert_eq!(
            serde_json::to_value(&filter).expect("serialize filter")["status"],
            "closed"
        );
    }

    #[test]
    fn list_topics_filter_caps_external_page_size() {
        let filter: ListTopicsFilter =
            serde_json::from_value(json!({"per_page": 50_000})).expect("deserialize page size");
        assert_eq!(filter.per_page, crate::dto::MAX_FORUM_READ_LIMIT);
    }

    #[test]
    fn update_command_distinguishes_omitted_quotes_from_explicit_clear() {
        let omitted: UpdateTopicCommandInput = serde_json::from_value(json!({"locale": "en"}))
            .expect("omitted quotes should deserialize");
        assert!(omitted.quotes.is_none());

        let clear: UpdateTopicCommandInput =
            serde_json::from_value(json!({"locale": "en", "quotes": []}))
                .expect("explicit clear should deserialize");
        assert_eq!(clear.quotes, Some(Vec::new()));
    }

    #[test]
    fn topic_response_serde_uses_one_richtext_view() {
        let r = sample("plain");
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["body"]["document"]["type"], "doc");
        assert_eq!(
            v["body"]["html"],
            "<p class=\"richtext-paragraph\">plain</p>"
        );
        assert!(v.get("body_format").is_none());
        assert!(v.get("content_json").is_none());
        let d: TopicResponse = serde_json::from_value(v).expect("deserialize");
        assert_eq!(d.body.document, RichTextDocument::single_paragraph("plain"));
    }
}

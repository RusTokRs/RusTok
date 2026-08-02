use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustok_api::{RichTextDocument, RichTextView};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::ForumQuoteReferenceInput;
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
    #[serde(default = "default_page")]
    pub page: u64,
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
    pub channel_slugs: Vec<String>,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
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

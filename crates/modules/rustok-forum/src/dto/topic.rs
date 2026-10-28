use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, FixedOffset};
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

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    ToSchema,
    async_graphql::Enum,
)]
#[serde(rename_all = "snake_case")]
pub enum ForumTopicListOrder {
    #[default]
    Latest,
    Top,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListTopicsFilter {
    pub search: Option<String>,
    #[serde(default)]
    pub order: ForumTopicListOrder,
    #[serde(default)]
    pub solved_only: bool,
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
            search: None,
            order: ForumTopicListOrder::Latest,
            solved_only: false,
            category_id: None,
            status: None,
            locale: None,
            after: None,
            per_page: default_per_page(),
        }
    }
}

impl ListTopicsFilter {
    pub(crate) fn normalize_selection(&mut self) -> ForumResult<()> {
        let raw = self.search.as_deref().unwrap_or("").trim();
        let invalid = || ForumError::Validation("Forum topic search exceeds 200 characters".into());
        if raw.chars().take(201).count() > 200 {
            return Err(invalid());
        }
        let search = raw.to_lowercase();
        if search.chars().take(201).count() > 200 {
            return Err(invalid());
        }
        self.search = (!search.is_empty()).then_some(search);
        Ok(())
    }

    pub(crate) fn matches_selection(&self, topic: &TopicListItem) -> bool {
        (!self.solved_only || topic.solution_reply_id.is_some())
            && self.search.as_deref().is_none_or(|search| {
                topic.title.to_lowercase().contains(search)
                    || topic.slug.to_lowercase().contains(search)
            })
    }

    /// Locale/channel arguments are already normalized by their canonical owner helpers.
    pub(crate) fn cursor_scope(
        &self,
        tenant_id: Uuid,
        viewer_id: Option<Uuid>,
        locale: &str,
        fallback: Option<&str>,
        channel: Option<&str>,
        storefront: bool,
    ) -> ForumResult<[u8; 32]> {
        use sha2::{Digest, Sha256};
        let context = serde_json::to_vec(&(
            tenant_id,
            viewer_id,
            self.category_id,
            self.status,
            locale,
            fallback,
            channel,
            storefront,
            &self.search,
            self.order,
            self.solved_only,
        ))
        .map_err(|error| ForumError::Validation(error.to_string()))?;
        Ok(Sha256::digest(context).into())
    }
}

/// Opaque ranking position. Bound list cursors cannot cross a selection/context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopicListCursor {
    pub is_pinned: bool,
    pub last_reply_at: Option<DateTime<FixedOffset>>,
    pub updated_at: DateTime<FixedOffset>,
    pub id: Uuid,
    pub reply_count: i32,
    pub order: ForumTopicListOrder,
    pub selection_scope: Option<[u8; 32]>,
}

impl TopicListCursor {
    pub fn encode(&self) -> String {
        // These concrete scalar fields are infallible JSON values.
        URL_SAFE_NO_PAD.encode(serde_json::json!(self).to_string())
    }

    pub fn decode(value: &str) -> ForumResult<Self> {
        let invalid = || ForumError::Validation("Forum topic list cursor is invalid".to_string());
        if value.len() > 4096 {
            return Err(invalid());
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| invalid())?;
        let cursor: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        if cursor.reply_count < 0 {
            return Err(invalid());
        }
        Ok(cursor)
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

    #[test]
    fn list_selection_wire_contract_has_one_default_and_typed_order() {
        use super::ForumTopicListOrder;
        let default: ListTopicsFilter = serde_json::from_value(json!({})).unwrap();
        assert_eq!(default.order, ForumTopicListOrder::Latest);
        assert!(!default.solved_only);
        let selected: ListTopicsFilter = serde_json::from_value(json!({
            "search": "topic", "order": "top", "solved_only": true,
        }))
        .unwrap();
        assert_eq!(selected.order, ForumTopicListOrder::Top);
        assert!(selected.solved_only);
        assert!(serde_json::from_value::<ListTopicsFilter>(json!({"order": "popular"})).is_err());
    }

    #[test]
    fn topic_selection_normalizes_unicode_and_validates_before_scanning() {
        let mut filter = ListTopicsFilter {
            search: Some("  ЁЖ  ".into()),
            ..Default::default()
        };
        filter.normalize_selection().expect("normalize");
        assert_eq!(filter.search.as_deref(), Some("ёж"));
        filter.normalize_selection().expect("idempotent");
        filter.search = Some(" \n ".into());
        filter.normalize_selection().expect("empty search");
        assert!(filter.search.is_none());
        for value in ["x".repeat(201), "İ".repeat(200)] {
            filter.search = Some(value);
            assert!(filter.normalize_selection().is_err());
        }
    }

    #[test]
    fn topic_cursor_roundtrips_rank_and_context_and_rejects_stale_or_malformed_tokens() {
        use super::{ForumTopicListOrder, TopicListCursor};
        let cursor = TopicListCursor {
            is_pinned: true,
            last_reply_at: Some(
                chrono::DateTime::parse_from_rfc3339("2026-10-17T12:30:00.123456789+02:00")
                    .unwrap(),
            ),
            updated_at: chrono::DateTime::parse_from_rfc3339("2026-10-17T11:30:00Z").unwrap(),
            id: Uuid::new_v4(),
            reply_count: 42,
            order: ForumTopicListOrder::Top,
            selection_scope: Some([23; 32]),
        };
        assert_eq!(TopicListCursor::decode(&cursor.encode()).unwrap(), cursor);
        let unbound = TopicListCursor {
            selection_scope: None,
            order: ForumTopicListOrder::Latest,
            ..cursor
        };
        assert_eq!(TopicListCursor::decode(&unbound.encode()).unwrap(), unbound);
        for token in [
            "not a cursor".into(),
            "x".repeat(4097),
            "djF8MHwtfDIwMjYtMTAtMTdUMDA6MDA6MDBafGlk".into(),
        ] {
            assert!(TopicListCursor::decode(&token).is_err());
        }
        assert!(TopicListCursor::decode(
            &TopicListCursor {
                reply_count: -1,
                ..cursor
            }
            .encode()
        )
        .is_err());
    }

    #[test]
    fn selection_scope_binds_context_but_not_page_size_or_after() {
        let tenant = Uuid::new_v4();
        let viewer = Some(Uuid::new_v4());
        let mut filter = ListTopicsFilter::default();
        let scope = filter
            .cursor_scope(tenant, viewer, "en", Some("ru"), Some("web"), true)
            .unwrap();
        filter.per_page = 1;
        filter.after = Some("position".into());
        assert_eq!(
            scope,
            filter
                .cursor_scope(tenant, viewer, "en", Some("ru"), Some("web"), true)
                .unwrap()
        );
        for changed in [
            filter.cursor_scope(Uuid::new_v4(), viewer, "en", Some("ru"), Some("web"), true),
            filter.cursor_scope(tenant, None, "en", Some("ru"), Some("web"), true),
            filter.cursor_scope(tenant, viewer, "fr", Some("ru"), Some("web"), true),
            filter.cursor_scope(tenant, viewer, "en", Some("fr"), Some("web"), true),
            filter.cursor_scope(tenant, viewer, "en", Some("ru"), Some("mobile"), true),
            filter.cursor_scope(tenant, viewer, "en", Some("ru"), Some("web"), false),
        ] {
            assert_ne!(scope, changed.unwrap());
        }
    }

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

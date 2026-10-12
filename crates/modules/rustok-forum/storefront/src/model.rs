use rustok_api::RichTextView;
use rustok_ui_core::normalize_css_hex_color;
use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_optional_css_hex_color<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.and_then(|value| normalize_css_hex_color(value.as_str())))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontForumData {
    pub categories: ForumCategoryConnection,
    pub topics: ForumTopicPage,
    pub selected_category_id: Option<String>,
    pub selected_topic_id: Option<String>,
    pub selected_topic: Option<ForumTopicDetail>,
    pub replies: ForumReplyPage,
    #[serde(default)]
    pub member_cards: Vec<ForumMemberCard>,
    #[serde(default)]
    pub read_state_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumCategoryConnection {
    pub items: Vec<ForumCategoryListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumTopicPage {
    pub items: Vec<ForumTopicListItem>,
    /// Present only when another page exists. Topics are not counted.
    #[serde(rename = "nextCursor", default)]
    pub next_cursor: Option<String>,
}

impl ForumTopicPage {
    /// Whether the owner offered a cursor, i.e. topics exist beyond what is loaded here.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Appends one fetched page and adopts its cursor.
    ///
    /// Keyset cursors are stable under concurrent writes, so a page can repeat a topic the surface
    /// already renders. Ids already present are skipped: a repeated read must not duplicate a card
    /// or rewind the feed.
    pub fn append_page(&mut self, page: Self) {
        for item in page.items {
            if !self.items.iter().any(|seen| seen.id == item.id) {
                self.items.push(item);
            }
        }
        self.next_cursor = page.next_cursor;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumReplyPage {
    pub items: Vec<ForumReplyDetail>,
    /// Present only when another page exists.
    #[serde(rename = "nextCursor", default)]
    pub next_cursor: Option<String>,
}

impl ForumReplyPage {
    /// Whether the owner offered a cursor for this thread.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Appends one fetched page, skipping replies already rendered, so a repeated cursor read
    /// cannot duplicate a post in the thread.
    pub fn append_page(&mut self, page: Self) {
        for item in page.items {
            if !self.items.iter().any(|seen| seen.id == item.id) {
                self.items.push(item);
            }
        }
        self.next_cursor = page.next_cursor;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumMemberCard {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub profile: ForumMemberProfileSummary,
    #[serde(rename = "forumStats")]
    pub forum_stats: ForumMemberStats,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumMemberProfileSummary {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub handle: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub tags: Vec<String>,
    #[serde(rename = "avatarMediaId")]
    pub avatar_media_id: Option<String>,
    #[serde(rename = "preferredLocale")]
    pub preferred_locale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumMemberStats {
    #[serde(rename = "topicCount")]
    pub topic_count: i32,
    #[serde(rename = "replyCount")]
    pub reply_count: i32,
    #[serde(rename = "solutionCount")]
    pub solution_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ForumCategoryListItem {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_css_hex_color")]
    pub color: Option<String>,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "topicCount")]
    pub topic_count: i32,
    #[serde(rename = "replyCount")]
    pub reply_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumTopicListItem {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(default, rename = "authorId")]
    pub author_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub status: String,
    #[serde(rename = "isPinned")]
    pub is_pinned: bool,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    #[serde(rename = "replyCount")]
    pub reply_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(default, rename = "readStateExplicit")]
    pub read_state_explicit: Option<bool>,
    #[serde(default, rename = "lastReadPosition")]
    pub last_read_position: Option<i64>,
    #[serde(default, rename = "lastReadRevision")]
    pub last_read_revision: Option<i64>,
    #[serde(default, rename = "unreadCount")]
    pub unread_count: Option<i64>,
    #[serde(default, rename = "hasUnreadTopicRevision")]
    pub has_unread_topic_revision: Option<bool>,
    #[serde(default, rename = "isUnread")]
    pub is_unread: Option<bool>,
    #[serde(default, rename = "solutionReplyId")]
    pub solution_reply_id: Option<String>,
    #[serde(default, rename = "voteScore")]
    pub vote_score: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumTopicDetail {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "availableLocales")]
    pub available_locales: Vec<String>,
    #[serde(rename = "categoryId")]
    pub category_id: String,
    #[serde(default, rename = "authorId")]
    pub author_id: Option<String>,
    pub title: String,
    pub slug: String,
    pub body: RichTextView,
    #[serde(rename = "bodyPlainText")]
    pub body_plain_text: String,
    pub status: String,
    pub tags: Vec<String>,
    #[serde(rename = "isPinned")]
    pub is_pinned: bool,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    #[serde(rename = "replyCount")]
    pub reply_count: i32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumReplyDetail {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "topicId")]
    pub topic_id: String,
    #[serde(default, rename = "authorId")]
    pub author_id: Option<String>,
    pub content: RichTextView,
    #[serde(rename = "contentPlainText")]
    pub content_plain_text: String,
    pub status: String,
    #[serde(rename = "parentReplyId")]
    pub parent_reply_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorefrontForumCategoryRouteDisposition {
    Canonical,
    Redirect,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontForumCategoryRouteDescriptor {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub locale: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontForumCategoryRouteResolution {
    #[serde(rename = "requestedLocale")]
    pub requested_locale: String,
    #[serde(rename = "requestedSlug")]
    pub requested_slug: String,
    pub disposition: StorefrontForumCategoryRouteDisposition,
    pub canonical: StorefrontForumCategoryRouteDescriptor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorefrontForumTopicRouteDisposition {
    Canonical,
    Redirect,
    Gone,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontForumTopicRouteDescriptor {
    #[serde(rename = "topicId")]
    pub topic_id: String,
    pub locale: String,
    #[serde(rename = "shortId")]
    pub short_id: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontForumTopicRouteResolution {
    #[serde(rename = "requestedLocale")]
    pub requested_locale: String,
    #[serde(rename = "requestedShortId")]
    pub requested_short_id: String,
    #[serde(rename = "requestedSlug")]
    pub requested_slug: String,
    pub disposition: StorefrontForumTopicRouteDisposition,
    pub canonical: Option<StorefrontForumTopicRouteDescriptor>,
}

/// One inline quote relation selected in the storefront composer.
///
/// `target_kind` carries the canonical quote target vocabulary the owner inputs
/// accept (`TOPIC` or `REPLY`, normalized by
/// `crate::core::forum_storefront_quote_target_kind`), and `revision_id` pins the
/// exact quoted source revision, so the owner can reject a stale quote instead of
/// silently re-pointing it. Only quotes with a positive revision leave the
/// composer; an unpinned quote stays quoted text in the body.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StorefrontForumComposerQuote {
    #[serde(rename = "targetKind")]
    pub target_kind: String,
    #[serde(rename = "targetId")]
    pub target_id: String,
    #[serde(rename = "revisionId")]
    pub revision_id: i64,
}

/// Topic creation draft submitted from the storefront composer.
///
/// `body_text` is plain author text; the transport turns it into the canonical
/// rich-text document so both transports send one identical wire shape.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreateStorefrontForumTopicDraft {
    #[serde(rename = "categoryId")]
    pub category_id: String,
    pub title: String,
    #[serde(rename = "bodyText")]
    pub body_text: String,
    pub locale: String,
    #[serde(default)]
    pub quotes: Vec<StorefrontForumComposerQuote>,
}

/// Reply creation draft submitted from the storefront composer.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreateStorefrontForumReplyDraft {
    #[serde(rename = "topicId")]
    pub topic_id: String,
    #[serde(rename = "bodyText")]
    pub body_text: String,
    #[serde(rename = "parentReplyId", default)]
    pub parent_reply_id: Option<String>,
    pub locale: String,
    #[serde(default)]
    pub quotes: Vec<StorefrontForumComposerQuote>,
}

/// Identity the storefront needs back after a successful create. The feed and
/// thread panel reload through the read transport, so no body is echoed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StorefrontForumCreatedTopic {
    #[serde(rename = "topicId")]
    pub topic_id: String,
    pub slug: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StorefrontForumCreatedReply {
    #[serde(rename = "replyId")]
    pub reply_id: String,
    #[serde(rename = "topicId")]
    pub topic_id: String,
}

#[cfg(test)]
mod tests {

    use super::{
        CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, ForumCategoryListItem,
        ForumReplyPage, ForumTopicListItem, ForumTopicPage,
        StorefrontForumCategoryRouteDisposition, StorefrontForumCategoryRouteResolution,
        StorefrontForumComposerQuote, StorefrontForumTopicRouteDisposition,
        StorefrontForumTopicRouteResolution,
    };

    fn topic_page_item(id: &str) -> ForumTopicListItem {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "effectiveLocale": "en",
            "categoryId": "category-1",
            "title": id,
            "slug": id,
            "status": "open",
            "isPinned": false,
            "isLocked": false,
            "replyCount": 0,
            "createdAt": "2026-07-24T00:00:00Z"
        }))
        .expect("topic list item")
    }

    #[test]
    fn topic_pages_append_once_and_follow_the_newest_cursor() {
        let mut page = ForumTopicPage {
            items: vec![topic_page_item("a")],
            next_cursor: Some("cursor-1".to_string()),
        };
        assert!(page.has_more());

        page.append_page(ForumTopicPage {
            items: vec![topic_page_item("a"), topic_page_item("b")],
            next_cursor: Some("cursor-2".to_string()),
        });

        let ids = page
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["a", "b"]);
        assert_eq!(page.next_cursor.as_deref(), Some("cursor-2"));
    }

    #[test]
    fn exhausted_pages_stop_offering_more_topics_and_replies() {
        let mut topics = ForumTopicPage {
            items: vec![topic_page_item("a")],
            next_cursor: Some("cursor-1".to_string()),
        };
        topics.append_page(ForumTopicPage {
            items: Vec::new(),
            next_cursor: None,
        });
        assert!(!topics.has_more());
        assert_eq!(topics.items.len(), 1);

        let mut replies = ForumReplyPage {
            items: Vec::new(),
            next_cursor: Some("cursor-1".to_string()),
        };
        replies.append_page(ForumReplyPage {
            items: Vec::new(),
            next_cursor: None,
        });
        assert!(!replies.has_more());
    }

    #[test]
    fn composer_drafts_use_the_owner_wire_shape() {
        let draft = CreateStorefrontForumTopicDraft {
            category_id: "0a0a0a0a-0a0a-4a0a-8a0a-0a0a0a0a0a0a".to_string(),
            title: "Release notes".to_string(),
            body_text: "What changed".to_string(),
            locale: "en-US".to_string(),
            quotes: vec![StorefrontForumComposerQuote {
                target_kind: "REPLY".to_string(),
                target_id: "0b0b0b0b-0b0b-4b0b-8b0b-0b0b0b0b0b0b".to_string(),
                revision_id: 7,
            }],
        };
        let encoded = serde_json::to_value(&draft).expect("topic draft");
        assert_eq!(encoded["categoryId"], serde_json::json!(draft.category_id));
        assert_eq!(encoded["bodyText"], serde_json::json!("What changed"));
        assert_eq!(
            encoded["quotes"][0]["targetKind"],
            serde_json::json!("REPLY")
        );
        assert_eq!(encoded["quotes"][0]["revisionId"], serde_json::json!(7));

        let reply = CreateStorefrontForumReplyDraft {
            topic_id: "0c0c0c0c-0c0c-4c0c-8c0c-0c0c0c0c0c0c".to_string(),
            body_text: "Thanks".to_string(),
            parent_reply_id: None,
            locale: "ru".to_string(),
            quotes: Vec::new(),
        };
        let encoded = serde_json::to_value(&reply).expect("reply draft");
        assert_eq!(encoded["topicId"], serde_json::json!(reply.topic_id));
        assert_eq!(encoded["quotes"], serde_json::json!([]));
    }

    fn category_json(color: &str) -> String {
        serde_json::json!({
            "id": "category-1",
            "effectiveLocale": "en",
            "name": "General",
            "slug": "general",
            "description": null,
            "icon": null,
            "color": color,
            "topicCount": 0,
            "replyCount": 0
        })
        .to_string()
    }

    #[test]
    fn category_models_normalize_hex_colors_at_transport_boundary() {
        let category: ForumCategoryListItem =
            serde_json::from_str(category_json(" #0ea5e9 ").as_str()).expect("category");
        assert_eq!(category.color.as_deref(), Some("#0ea5e9"));
    }

    #[test]
    fn category_models_drop_css_declaration_injection() {
        let category: ForumCategoryListItem = serde_json::from_str(
            category_json("#fff;background:url(https://attacker.invalid/x)").as_str(),
        )
        .expect("category");
        assert_eq!(category.color, None);
    }

    #[test]
    fn public_topic_payload_keeps_unread_state_absent() {
        let topic: ForumTopicListItem = serde_json::from_value(serde_json::json!({
            "id": "topic-1",
            "effectiveLocale": "en",
            "categoryId": "category-1",
            "title": "Welcome",
            "slug": "welcome",
            "status": "open",
            "isPinned": false,
            "isLocked": false,
            "replyCount": 0,
            "createdAt": "2026-07-24T00:00:00Z"
        }))
        .expect("topic");
        assert_eq!(topic.author_id, None);
        assert_eq!(topic.is_unread, None);
        assert_eq!(topic.unread_count, None);
        assert_eq!(topic.solution_reply_id, None);
        assert_eq!(topic.vote_score, None);
    }

    #[test]
    fn category_route_payload_uses_graphql_enum_and_field_names() {
        let route: StorefrontForumCategoryRouteResolution =
            serde_json::from_value(serde_json::json!({
                "requestedLocale": "en",
                "requestedSlug": "old-general",
                "disposition": "REDIRECT",
                "canonical": {
                    "categoryId": "12345678-9abc-4def-8123-456789abcdef",
                    "locale": "en",
                    "slug": "general",
                    "path": "/en/forum/c/general"
                }
            }))
            .expect("category route payload");
        assert_eq!(
            route.disposition,
            StorefrontForumCategoryRouteDisposition::Redirect
        );
        assert_eq!(route.canonical.slug, "general");
    }

    #[test]
    fn route_payload_uses_graphql_enum_and_field_names() {
        let route: StorefrontForumTopicRouteResolution =
            serde_json::from_value(serde_json::json!({
                "requestedLocale": "en",
                "requestedShortId": "123456789abc",
                "requestedSlug": "old-welcome",
                "disposition": "REDIRECT",
                "canonical": {
                    "topicId": "12345678-9abc-4def-8123-456789abcdef",
                    "locale": "en",
                    "shortId": "123456789abc",
                    "slug": "welcome",
                    "path": "/en/forum/t/123456789abc/welcome"
                }
            }))
            .expect("route payload");
        assert_eq!(
            route.disposition,
            StorefrontForumTopicRouteDisposition::Redirect
        );
        assert_eq!(
            route
                .canonical
                .as_ref()
                .map(|canonical| canonical.slug.as_str()),
            Some("welcome")
        );
    }

    #[test]
    fn gone_route_payload_has_no_canonical_target() {
        let route: StorefrontForumTopicRouteResolution =
            serde_json::from_value(serde_json::json!({
                "requestedLocale": "en",
                "requestedShortId": "123456789abc",
                "requestedSlug": "retired-topic",
                "disposition": "GONE",
                "canonical": null
            }))
            .expect("gone route payload");
        assert_eq!(
            route.disposition,
            StorefrontForumTopicRouteDisposition::Gone
        );
        assert!(route.canonical.is_none());
    }
}

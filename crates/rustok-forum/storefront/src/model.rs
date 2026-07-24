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
    pub topics: ForumTopicConnection,
    pub selected_category_id: Option<String>,
    pub selected_topic_id: Option<String>,
    pub selected_topic: Option<ForumTopicDetail>,
    pub replies: ForumReplyConnection,
    #[serde(default)]
    pub read_state_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumCategoryConnection {
    pub items: Vec<ForumCategoryListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumTopicConnection {
    pub items: Vec<ForumTopicListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForumReplyConnection {
    pub items: Vec<ForumReplyDetail>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub title: String,
    pub slug: String,
    pub body: String,
    #[serde(rename = "bodyFormat")]
    pub body_format: String,
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
    pub content: String,
    #[serde(rename = "contentFormat")]
    pub content_format: String,
    pub status: String,
    #[serde(rename = "parentReplyId")]
    pub parent_reply_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::{ForumCategoryListItem, ForumTopicListItem};

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
        assert_eq!(topic.is_unread, None);
        assert_eq!(topic.unread_count, None);
    }
}

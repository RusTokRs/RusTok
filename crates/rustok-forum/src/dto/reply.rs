use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustok_core::CONTENT_FORMAT_MARKDOWN;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateReplyInput {
    pub locale: String,
    pub content: String,
    #[serde(default = "default_content_format")]
    pub content_format: String,
    pub content_json: Option<Value>,
    pub parent_reply_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateReplyInput {
    pub locale: String,
    pub content: Option<String>,
    pub content_format: Option<String>,
    pub content_json: Option<Value>,
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

fn default_content_format() -> String {
    CONTENT_FORMAT_MARKDOWN.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyResponse {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: String,
    pub content_format: String,
    pub content_json: Option<Value>,
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
    use super::{ListRepliesFilter, ReplyResponse};
    use serde_json::json;
    use uuid::Uuid;

    fn sample(
        content: &str,
        content_format: &str,
        content_json: Option<serde_json::Value>,
    ) -> ReplyResponse {
        ReplyResponse {
            id: Uuid::new_v4(),
            requested_locale: "en".into(),
            locale: "en".into(),
            effective_locale: "en".into(),
            topic_id: Uuid::new_v4(),
            author_id: None,
            content: content.into(),
            content_format: content_format.into(),
            content_json,
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
    fn reply_response_serde_markdown() {
        let r = sample("plain", "markdown", None);
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["content_format"], "markdown");
        assert_eq!(v["content_json"], serde_json::Value::Null);
        let d: ReplyResponse = serde_json::from_value(v).expect("deserialize");
        assert_eq!(d.content, "plain");
        assert!(d.content_json.is_none());
    }

    #[test]
    fn reply_response_serde_rt_json_v1() {
        let rich = json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let r = sample(&rich.to_string(), "rt_json_v1", Some(rich.clone()));
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["content_format"], "rt_json_v1");
        assert_eq!(v["content_json"], rich);
    }
}

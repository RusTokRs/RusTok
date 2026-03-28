use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustok_core::CONTENT_FORMAT_MARKDOWN;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateTopicInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    #[serde(default = "default_content_format")]
    pub body_format: String,
    pub content_json: Option<Value>,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct UpdateTopicInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, IntoParams)]
pub struct ListTopicsFilter {
    pub category_id: Option<Uuid>,
    pub status: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

fn default_content_format() -> String {
    CONTENT_FORMAT_MARKDOWN.to_string()
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
    pub body: String,
    pub body_format: String,
    pub content_json: Option<Value>,
    pub status: String,
    pub tags: Vec<String>,
    pub channel_slugs: Vec<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicListItem {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub channel_slugs: Vec<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::TopicResponse;
    use serde_json::json;
    use uuid::Uuid;

    fn sample(
        body: &str,
        body_format: &str,
        content_json: Option<serde_json::Value>,
    ) -> TopicResponse {
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
            body: body.into(),
            body_format: body_format.into(),
            content_json,
            status: "open".into(),
            tags: vec![],
            channel_slugs: vec![],
            is_pinned: false,
            is_locked: false,
            reply_count: 0,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn topic_response_serde_markdown() {
        let r = sample("plain", "markdown", None);
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["body_format"], "markdown");
        assert_eq!(v["content_json"], serde_json::Value::Null);
        let d: TopicResponse = serde_json::from_value(v).expect("deserialize");
        assert_eq!(d.body, "plain");
        assert!(d.content_json.is_none());
    }

    #[test]
    fn topic_response_serde_rt_json_v1() {
        let rich = json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let r = sample(&rich.to_string(), "rt_json_v1", Some(rich.clone()));
        let v = serde_json::to_value(&r).expect("serialize");
        assert_eq!(v["body_format"], "rt_json_v1");
        assert_eq!(v["content_json"], rich);
    }
}

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state_machine::TopicStatus;

pub const DEFAULT_FORUM_READ_LIMIT: u64 = 20;
pub const MAX_FORUM_READ_LIMIT: u64 = 100;

pub fn bounded_forum_read_limit(requested: Option<u64>) -> u64 {
    requested
        .unwrap_or(DEFAULT_FORUM_READ_LIMIT)
        .clamp(1, MAX_FORUM_READ_LIMIT)
}

pub fn deserialize_forum_read_limit<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let requested = Option::<u64>::deserialize(deserializer)?;
    Ok(bounded_forum_read_limit(requested))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct CategoryCursorQuery {
    pub cursor: Option<String>,
    pub limit: Option<u64>,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct TopicCursorQuery {
    pub cursor: Option<String>,
    pub limit: Option<u64>,
    pub category_id: Option<Uuid>,
    pub status: Option<TopicStatus>,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ReplyCursorQuery {
    pub cursor: Option<String>,
    pub limit: Option<u64>,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryReadModel {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub moderated: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub is_subscribed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicReadModel {
    pub id: Uuid,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub title: String,
    pub slug: String,
    pub metadata: Value,
    pub status: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyReadModel {
    pub id: Uuid,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub parent_reply_id: Option<Uuid>,
    pub position: i64,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub content_preview: String,
    pub status: String,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_solution: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryCursorPage {
    pub items: Vec<CategoryReadModel>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TopicCursorPage {
    pub items: Vec<TopicReadModel>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyCursorPage {
    pub items: Vec<ReplyReadModel>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_limit_is_always_bounded() {
        assert_eq!(bounded_forum_read_limit(None), 20);
        assert_eq!(bounded_forum_read_limit(Some(0)), 1);
        assert_eq!(bounded_forum_read_limit(Some(50)), 50);
        assert_eq!(bounded_forum_read_limit(Some(10_000)), 100);
    }
}

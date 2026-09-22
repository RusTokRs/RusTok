use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumUserStatsResponse {
    pub user_id: Uuid,
    pub topic_count: i32,
    pub reply_count: i32,
    pub solution_count: i32,
    pub updated_at: String,
}

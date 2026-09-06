use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateCategoryTopicPolicyInput {
    pub allows_topics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryTopicPolicyResponse {
    pub category_id: Uuid,
    pub allows_topics: bool,
}

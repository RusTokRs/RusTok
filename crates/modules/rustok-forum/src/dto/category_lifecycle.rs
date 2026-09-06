use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategorySubtreeLifecycleResponse {
    pub root_id: Uuid,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub affected_category_ids: Vec<Uuid>,
    pub changed_category_ids: Vec<Uuid>,
    pub affected_count: u32,
    pub changed_count: u32,
}

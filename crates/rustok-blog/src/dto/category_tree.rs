use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryTreeNode {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
    pub settings: serde_json::Value,
    #[schema(no_recursion)]
    pub children: Vec<CategoryTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryTreeResponse {
    pub roots: Vec<CategoryTreeNode>,
    pub total_nodes: u32,
    pub max_depth: i32,
}

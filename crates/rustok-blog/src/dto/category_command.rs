use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Maximum number of Blog categories admitted into one structural tree command.
pub const MAX_BLOG_CATEGORY_TREE_NODES: u64 = 512;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveCategoryInput {
    /// Destination parent. `None` moves the category to the root level.
    pub parent_id: Option<Uuid>,
    /// Zero-based index inside the destination sibling list.
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct CategoryPlacementResponse {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveCategoryResponse {
    pub moved: CategoryPlacementResponse,
    /// Source/destination siblings plus descendants whose materialized depth changed.
    pub updated: Vec<CategoryPlacementResponse>,
}

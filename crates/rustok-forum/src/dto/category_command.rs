use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveCategoryInput {
    pub parent_id: Option<Uuid>,
    /// Zero-based index inside the destination sibling list.
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderCategorySiblingsInput {
    pub parent_id: Option<Uuid>,
    /// Complete ordered set of direct children for `parent_id`.
    pub ordered_category_ids: Vec<Uuid>,
}

/// Replace or clear the single Forum-owned category cover relation.
///
/// Forum stores only the typed Media identity. Media remains authoritative for
/// lifecycle, quarantine/deletion state, storage and delivery policy.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetCategoryCoverInput {
    pub media_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryCoverResponse {
    pub category_id: Uuid,
    pub media_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryPlacementResponse {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveCategoryResponse {
    pub moved: CategoryPlacementResponse,
    /// All siblings whose placement changed in the source or destination list.
    pub updated: Vec<CategoryPlacementResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReorderCategorySiblingsResponse {
    pub parent_id: Option<Uuid>,
    pub siblings: Vec<CategoryPlacementResponse>,
}

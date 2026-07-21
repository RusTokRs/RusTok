use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Maximum number of categories returned by the canonical tree read.
///
/// The tree is intentionally bounded independently from cursor page limits so
/// one owner call can return a complete admin/storefront hierarchy without
/// permitting unbounded tenant data to enter memory.
pub const MAX_FORUM_CATEGORY_TREE_NODES: u64 = 512;

/// Maximum supported zero-based category depth.
pub const MAX_FORUM_CATEGORY_TREE_DEPTH: usize = 16;

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct CategoryTreeQuery {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryBreadcrumb {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryTreeNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub depth: u16,
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
    /// Whether new topics may be created directly in this category.
    pub allows_topics: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub is_subscribed: bool,
    pub has_children: bool,
    /// Number of direct children, not total descendants.
    pub children_count: u32,
    /// Ancestor chain including this category.
    pub breadcrumbs: Vec<CategoryBreadcrumb>,
    #[schema(no_recursion)]
    pub children: Vec<CategoryTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CategoryTreeResponse {
    pub roots: Vec<CategoryTreeNode>,
    pub total_nodes: u32,
    pub max_depth: u16,
}

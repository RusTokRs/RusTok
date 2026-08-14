use async_graphql::{InputObject, Json, SimpleObject};
use uuid::Uuid;

use crate::dto::{
    CategoryPlacementResponse, CategoryResponse, CategoryTreeNode, CategoryTreeResponse,
    CreateCategoryInput as DomainCreateCategoryInput, MoveCategoryInput as DomainMoveCategoryInput,
    MoveCategoryResponse, UpdateCategoryInput as DomainUpdateCategoryInput,
};

#[derive(SimpleObject, Clone)]
#[graphql(name = "BlogCategory")]
pub struct GqlBlogCategory {
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
    pub settings: Json<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "BlogCategoryTreeNode")]
pub struct GqlBlogCategoryTreeNode {
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
    pub settings: Json<serde_json::Value>,
    pub children: Vec<GqlBlogCategoryTreeNode>,
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "BlogCategoryTree")]
pub struct GqlBlogCategoryTree {
    pub roots: Vec<GqlBlogCategoryTreeNode>,
    pub total_nodes: u32,
    pub max_depth: i32,
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "BlogCategoryPlacement")]
pub struct GqlBlogCategoryPlacement {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
}

#[derive(SimpleObject, Clone)]
#[graphql(name = "MoveBlogCategoryPayload")]
pub struct GqlMoveBlogCategoryPayload {
    pub moved: GqlBlogCategoryPlacement,
    pub updated: Vec<GqlBlogCategoryPlacement>,
}

#[derive(InputObject, Clone)]
#[graphql(name = "CreateBlogCategoryInput")]
pub struct GqlCreateBlogCategoryInput {
    pub locale: String,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: Option<i32>,
    pub settings: Option<Json<serde_json::Value>>,
}

#[derive(InputObject, Clone)]
#[graphql(name = "UpdateBlogCategoryInput")]
pub struct GqlUpdateBlogCategoryInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub settings: Option<Json<serde_json::Value>>,
}

#[derive(InputObject, Clone)]
#[graphql(name = "MoveBlogCategoryInput")]
pub struct GqlMoveBlogCategoryInput {
    pub parent_id: Option<Uuid>,
    pub position: u32,
}

impl From<CategoryResponse> for GqlBlogCategory {
    fn from(category: CategoryResponse) -> Self {
        Self {
            id: category.id,
            tenant_id: category.tenant_id,
            requested_locale: category.locale,
            effective_locale: category.effective_locale,
            available_locales: category.available_locales,
            name: category.name,
            slug: category.slug,
            description: category.description,
            parent_id: category.parent_id,
            position: category.position,
            settings: Json(category.settings),
            created_at: category.created_at.to_rfc3339(),
            updated_at: category.updated_at.to_rfc3339(),
        }
    }
}

impl From<CategoryTreeNode> for GqlBlogCategoryTreeNode {
    fn from(node: CategoryTreeNode) -> Self {
        Self {
            id: node.id,
            tenant_id: node.tenant_id,
            requested_locale: node.requested_locale,
            effective_locale: node.effective_locale,
            available_locales: node.available_locales,
            name: node.name,
            slug: node.slug,
            description: node.description,
            parent_id: node.parent_id,
            position: node.position,
            depth: node.depth,
            settings: Json(node.settings),
            children: node.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CategoryTreeResponse> for GqlBlogCategoryTree {
    fn from(tree: CategoryTreeResponse) -> Self {
        Self {
            roots: tree.roots.into_iter().map(Into::into).collect(),
            total_nodes: tree.total_nodes,
            max_depth: tree.max_depth,
        }
    }
}

impl From<CategoryPlacementResponse> for GqlBlogCategoryPlacement {
    fn from(placement: CategoryPlacementResponse) -> Self {
        Self {
            id: placement.id,
            parent_id: placement.parent_id,
            position: placement.position,
            depth: placement.depth,
        }
    }
}

impl From<MoveCategoryResponse> for GqlMoveBlogCategoryPayload {
    fn from(response: MoveCategoryResponse) -> Self {
        Self {
            moved: response.moved.into(),
            updated: response.updated.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<GqlCreateBlogCategoryInput> for DomainCreateCategoryInput {
    fn from(input: GqlCreateBlogCategoryInput) -> Self {
        Self {
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            description: input.description,
            parent_id: input.parent_id,
            position: input.position,
            settings: input.settings.map(|value| value.0).unwrap_or_default(),
        }
    }
}

impl From<GqlUpdateBlogCategoryInput> for DomainUpdateCategoryInput {
    fn from(input: GqlUpdateBlogCategoryInput) -> Self {
        Self {
            locale: input.locale,
            name: input.name,
            slug: input.slug,
            description: input.description,
            position: None,
            settings: input.settings.map(|value| value.0),
        }
    }
}

impl From<GqlMoveBlogCategoryInput> for DomainMoveCategoryInput {
    fn from(input: GqlMoveBlogCategoryInput) -> Self {
        Self {
            parent_id: input.parent_id,
            position: input.position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_update_cannot_encode_structural_position() {
        let domain: DomainUpdateCategoryInput = GqlUpdateBlogCategoryInput {
            locale: "en".to_string(),
            name: Some("Systems".to_string()),
            slug: None,
            description: None,
            settings: None,
        }
        .into();
        assert_eq!(domain.position, None);
    }

    #[test]
    fn move_input_keeps_move_to_root_unambiguous() {
        let domain: DomainMoveCategoryInput = GqlMoveBlogCategoryInput {
            parent_id: None,
            position: 2,
        }
        .into();
        assert_eq!(domain.parent_id, None);
        assert_eq!(domain.position, 2);
    }
}

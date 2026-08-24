use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::graphql::{extract_graphql_context, require_module_enabled};
use uuid::Uuid;

use crate::{
    CategoryPlacementResponse, CategoryService, MoveCategoryInput, MoveCategoryResponse,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse,
};

use super::{require_forum_permission, resolve_tenant_scope};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumCategoryCommandMutation;

#[Object]
impl ForumCategoryCommandMutation {
    async fn move_forum_category(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
        input: MoveForumCategoryInput,
    ) -> Result<GqlForumCategoryMove> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let (db, tenant) = extract_graphql_context(ctx)?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_MANAGE],
            "Permission denied: forum_categories:manage required",
        )?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let position = u32::try_from(input.position).map_err(|_| {
            async_graphql::Error::new("position must be a non-negative integer within u32 range")
        })?;
        let response = CategoryService::new(db.clone())
            .move_category(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                MoveCategoryInput {
                    parent_id: input.parent_id,
                    position,
                },
            )
            .await?;
        Ok(response.into())
    }

    async fn reorder_forum_category_siblings(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: ReorderForumCategorySiblingsInput,
    ) -> Result<GqlForumCategorySiblingOrder> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let (db, tenant) = extract_graphql_context(ctx)?;
        let auth = require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_MANAGE],
            "Permission denied: forum_categories:manage required",
        )?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let response = CategoryService::new(db.clone())
            .reorder_siblings(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                ReorderCategorySiblingsInput {
                    parent_id: input.parent_id,
                    ordered_category_ids: input.ordered_category_ids,
                },
            )
            .await?;
        Ok(response.into())
    }
}

#[derive(InputObject)]
pub struct MoveForumCategoryInput {
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

#[derive(InputObject)]
pub struct ReorderForumCategorySiblingsInput {
    pub parent_id: Option<Uuid>,
    pub ordered_category_ids: Vec<Uuid>,
}

#[derive(Clone, SimpleObject)]
pub struct GqlForumCategoryPlacement {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

#[derive(SimpleObject)]
pub struct GqlForumCategoryMove {
    pub moved: GqlForumCategoryPlacement,
    pub updated: Vec<GqlForumCategoryPlacement>,
}

#[derive(SimpleObject)]
pub struct GqlForumCategorySiblingOrder {
    pub parent_id: Option<Uuid>,
    pub siblings: Vec<GqlForumCategoryPlacement>,
}

impl From<CategoryPlacementResponse> for GqlForumCategoryPlacement {
    fn from(value: CategoryPlacementResponse) -> Self {
        Self {
            id: value.id,
            parent_id: value.parent_id,
            position: value.position,
        }
    }
}

impl From<MoveCategoryResponse> for GqlForumCategoryMove {
    fn from(value: MoveCategoryResponse) -> Self {
        Self {
            moved: value.moved.into(),
            updated: value.updated.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ReorderCategorySiblingsResponse> for GqlForumCategorySiblingOrder {
    fn from(value: ReorderCategorySiblingsResponse) -> Self {
        Self {
            parent_id: value.parent_id,
            siblings: value.siblings.into_iter().map(Into::into).collect(),
        }
    }
}

use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::{
    AuthContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CategoryPlacementResponse, CategoryService, MoveCategoryInput, MoveCategoryResponse,
    ReorderCategorySiblingsInput, ReorderCategorySiblingsResponse,
};

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
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_category_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let position = u32::try_from(input.position).map_err(|_| {
            async_graphql::Error::new("Category position must be a non-negative integer")
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
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_category_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
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

fn require_category_manage_permission(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_MANAGE]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_categories:manage required",
        ));
    }
    Ok(auth)
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
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

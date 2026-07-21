use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext, TenantContext,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{CategoryService, CategorySubtreeLifecycleResponse};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumCategoryLifecycleMutation;

#[Object]
impl ForumCategoryLifecycleMutation {
    async fn archive_forum_category_subtree(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
    ) -> Result<GqlForumCategorySubtreeLifecycle> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_category_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let response = CategoryService::new(db.clone())
            .archive_subtree(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;
        Ok(response.into())
    }

    async fn restore_forum_category_subtree(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
    ) -> Result<GqlForumCategorySubtreeLifecycle> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_category_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let response = CategoryService::new(db.clone())
            .restore_subtree(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
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
    if !has_any_effective_permission(
        &auth.permissions,
        &[Permission::FORUM_CATEGORIES_MANAGE],
    ) {
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

#[derive(SimpleObject)]
pub struct GqlForumCategorySubtreeLifecycle {
    pub root_id: Uuid,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub affected_category_ids: Vec<Uuid>,
    pub changed_category_ids: Vec<Uuid>,
    pub affected_count: i32,
    pub changed_count: i32,
}

impl From<CategorySubtreeLifecycleResponse> for GqlForumCategorySubtreeLifecycle {
    fn from(value: CategorySubtreeLifecycleResponse) -> Self {
        Self {
            root_id: value.root_id,
            archived: value.archived,
            archived_at: value.archived_at,
            affected_category_ids: value.affected_category_ids,
            changed_category_ids: value.changed_category_ids,
            affected_count: value.affected_count as i32,
            changed_count: value.changed_count as i32,
        }
    }
}

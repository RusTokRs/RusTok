use async_graphql::{Context, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::{TenantContext, graphql::require_module_enabled};
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_MANAGE],
            "Permission denied: forum_categories:manage required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
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
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_MANAGE],
            "Permission denied: forum_categories:manage required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;
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

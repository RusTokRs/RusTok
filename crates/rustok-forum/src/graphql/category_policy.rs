use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext, TenantContext,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{CategoryService, UpdateCategoryTopicPolicyInput};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumCategoryTopicPolicyQuery;

#[Object]
impl ForumCategoryTopicPolicyQuery {
    async fn forum_category_topic_policy(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
    ) -> Result<GqlForumCategoryTopicPolicy> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_READ],
            "Permission denied: forum_categories:read required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let policy = CategoryService::new(db.clone())
            .topic_policy(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
            )
            .await?;
        Ok(policy.into())
    }
}

#[derive(Default)]
pub(crate) struct ForumCategoryTopicPolicyMutation;

#[Object]
impl ForumCategoryTopicPolicyMutation {
    async fn set_forum_category_topic_policy(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        category_id: Uuid,
        input: UpdateForumCategoryTopicPolicyInput,
    ) -> Result<GqlForumCategoryTopicPolicy> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_permission(
            ctx,
            &[Permission::FORUM_CATEGORIES_MANAGE],
            "Permission denied: forum_categories:manage required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let policy = CategoryService::new(db.clone())
            .set_topic_policy(
                tenant_id,
                category_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                UpdateCategoryTopicPolicyInput {
                    allows_topics: input.allows_topics,
                },
            )
            .await?;
        Ok(policy.into())
    }
}

#[derive(InputObject)]
pub struct UpdateForumCategoryTopicPolicyInput {
    pub allows_topics: bool,
}

#[derive(SimpleObject)]
pub struct GqlForumCategoryTopicPolicy {
    pub category_id: Uuid,
    pub allows_topics: bool,
}

impl From<crate::CategoryTopicPolicyResponse> for GqlForumCategoryTopicPolicy {
    fn from(value: crate::CategoryTopicPolicyResponse) -> Self {
        Self {
            category_id: value.category_id,
            allows_topics: value.allows_topics,
        }
    }
}

fn require_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
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

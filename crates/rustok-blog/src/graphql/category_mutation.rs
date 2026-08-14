use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::{CategoryCommandService, CategoryService};

use super::category_types::{
    GqlBlogCategory, GqlCreateBlogCategoryInput, GqlMoveBlogCategoryInput,
    GqlMoveBlogCategoryPayload, GqlUpdateBlogCategoryInput,
};

const MODULE_SLUG: &str = "blog";

#[derive(Default)]
pub struct BlogCategoryMutation;

#[Object]
impl BlogCategoryMutation {
    async fn create_blog_category(
        &self,
        ctx: &Context<'_>,
        input: GqlCreateBlogCategoryInput,
    ) -> Result<GqlBlogCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_CREATE,
            "Permission denied: blog_categories:create required",
        )?;
        let tenant = current_authenticated_tenant(ctx, &auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = security_context(&auth);
        let locale = input.locale.clone();
        let service = CategoryService::new(db.clone(), event_bus.clone());
        let category_id = service
            .create(tenant.id, security.clone(), input.into())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let category = service
            .get(tenant.id, security, category_id, &locale)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(category.into())
    }

    async fn update_blog_category(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlUpdateBlogCategoryInput,
    ) -> Result<GqlBlogCategory> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_UPDATE,
            "Permission denied: blog_categories:update required",
        )?;
        let tenant = current_authenticated_tenant(ctx, &auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let category = CategoryService::new(db.clone(), event_bus.clone())
            .update(tenant.id, id, security_context(&auth), input.into())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(category.into())
    }

    async fn move_blog_category(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlMoveBlogCategoryInput,
    ) -> Result<GqlMoveBlogCategoryPayload> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_MANAGE,
            "Permission denied: blog_categories:manage required",
        )?;
        let tenant = current_authenticated_tenant(ctx, &auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let moved = CategoryCommandService::new(db.clone())
            .move_category(tenant.id, id, security_context(&auth), input.into())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(moved.into())
    }

    async fn delete_blog_category(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_DELETE,
            "Permission denied: blog_categories:delete required",
        )?;
        let tenant = current_authenticated_tenant(ctx, &auth)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        CategoryService::new(db.clone(), event_bus.clone())
            .delete(tenant.id, id, security_context(&auth))
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(true)
    }
}

fn require_category_permission(
    ctx: &Context<'_>,
    permission: Permission,
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }
    Ok(auth)
}

fn current_authenticated_tenant<'a>(
    ctx: &'a Context<'_>,
    auth: &AuthContext,
) -> Result<&'a TenantContext> {
    let tenant = ctx.data::<TenantContext>()?;
    if tenant.id != auth.tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Blog category mutations must use the current authenticated tenant",
        ));
    }
    Ok(tenant)
}

fn security_context(auth: &AuthContext) -> SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[cfg(test)]
mod tests {
    use super::security_context;
    use rustok_api::{AuthContext, Permission};
    use uuid::Uuid;

    #[test]
    fn category_security_context_keeps_exact_blog_permissions() {
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![Permission::BLOG_CATEGORIES_MANAGE.to_string()],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        };
        let security = security_context(&auth);
        assert_eq!(security.user_id, Some(auth.user_id));
    }
}

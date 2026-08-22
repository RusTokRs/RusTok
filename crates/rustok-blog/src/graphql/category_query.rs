use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::error::BlogError;
use crate::services::{CategoryService, CategoryTreeService};

use super::category_types::{GqlBlogCategory, GqlBlogCategoryTree};

const MODULE_SLUG: &str = "blog";

#[derive(Default)]
pub struct BlogCategoryQuery;

#[Object]
impl BlogCategoryQuery {
    async fn blog_category(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlBlogCategory>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_READ,
            "Permission denied: blog_categories:read required",
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = CategoryService::new(db.clone(), event_bus.clone());

        match service
            .get(tenant.id, security_context(&auth), id, &locale)
            .await
        {
            Ok(category) => Ok(Some(category.into())),
            Err(BlogError::CategoryNotFound(_)) => Ok(None),
            Err(error) => Err(async_graphql::Error::new(error.to_string())),
        }
    }

    async fn blog_category_tree(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
    ) -> Result<GqlBlogCategoryTree> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_category_permission(
            ctx,
            Permission::BLOG_CATEGORIES_LIST,
            "Permission denied: blog_categories:list required",
        )?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let tree = CategoryTreeService::new(db.clone())
            .read(tenant.id, security_context(&auth), Some(locale.as_str()))
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(tree.into())
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

fn security_context(auth: &AuthContext) -> SecurityContext {
    SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
}

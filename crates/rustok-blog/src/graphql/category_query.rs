use async_graphql::{Context, Object, Result};
use rustok_api::{
    AuthContext, TenantContext,
    graphql::{require_module_enabled, resolve_graphql_locale},
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let service = CategoryService::new(db.clone(), event_bus.clone());

        match service
            .get(tenant.id, request_security_context(ctx), id, &locale)
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
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());
        let tree = CategoryTreeService::new(db.clone())
            .read(
                tenant.id,
                request_security_context(ctx),
                Some(locale.as_str()),
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(tree.into())
    }
}

fn request_security_context(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(|auth| {
            SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)
        })
        .unwrap_or_else(SecurityContext::public_read)
}

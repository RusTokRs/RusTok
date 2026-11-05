use async_graphql::{Context, Json, Result};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};

use crate::{
    BlogWidgetCatalog, BlogWidgetInput, BlogWidgetPreview, BlogWidgetService, BlogWidgetValidation,
};

async fn authorize<'a>(ctx: &'a Context<'_>) -> Result<(&'a TenantContext, &'a AuthContext)> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| missing_runtime())?;
    let auth = ctx
        .data_opt::<AuthContext>()
        .ok_or_else(async_graphql::FieldError::unauthenticated)?;
    super::query_tenant_id(ctx, tenant, None)?;
    if auth.user_id.is_nil()
        || !rustok_api::has_any_effective_permission(
            &auth.permissions,
            &[Permission::BLOG_POSTS_READ],
        )
    {
        return Err(async_graphql::FieldError::permission_denied(
            "Permission denied: blog_posts:read required",
        ));
    }
    ctx.data::<sea_orm::DatabaseConnection>().map_err(|_| missing_runtime())?;
    require_module_enabled(ctx, "blog").await?;
    Ok((tenant, auth))
}

pub(super) async fn catalog(ctx: &Context<'_>) -> Result<Json<BlogWidgetCatalog>> {
    authorize(ctx).await?;
    Ok(Json(BlogWidgetService::catalog()))
}

pub(super) async fn validate(
    ctx: &Context<'_>,
    widget_type: String,
    props: serde_json::Value,
) -> Result<Json<BlogWidgetValidation>> {
    authorize(ctx).await?;
    Ok(Json(BlogWidgetService::validate(BlogWidgetInput {
        widget_type,
        props,
    })))
}

pub(super) async fn preview(
    ctx: &Context<'_>,
    widget_type: String,
    props: serde_json::Value,
) -> Result<Json<BlogWidgetPreview>> {
    let (tenant, auth) = authorize(ctx).await?;
    let request = ctx.data::<RequestContext>().map_err(|_| {
        crate::error::public::to_graphql_error(crate::BlogError::invariant(
            "Widget preview requires request context",
        ))
    })?;
    let db = ctx.data::<sea_orm::DatabaseConnection>().map_err(|_| missing_runtime())?;
    let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>().map_err(|_| missing_runtime())?;
    BlogWidgetService::new(db.clone(), event_bus.clone())
        .preview(
            tenant.id,
            super::security_context_from_auth(auth),
            &request.locale,
            Some(&tenant.default_locale),
            request.channel_slug.as_deref(),
            BlogWidgetInput { widget_type, props },
        )
        .await
        .map(Json)
        .map_err(crate::error::public::to_graphql_error)
}

fn missing_runtime() -> async_graphql::Error {
    crate::error::public::to_graphql_error(crate::BlogError::invariant("Blog widget runtime context is unavailable"))
}

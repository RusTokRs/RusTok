use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::{AuthContext, RequestContext, TenantContext, graphql::{require_module_enabled, resolve_graphql_locale}};
use rustok_channel::ChannelService;
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;
use crate::{MenuBindingService, MenuService, NavigationError};
use crate::services::{MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE};
use super::types::*;

const MODULE_SLUG: &str = "navigation";

#[derive(Default)]
pub struct NavigationQuery;

#[Object]
impl NavigationQuery {
    async fn menu(&self, ctx: &Context<'_>, id: Uuid, locale: Option<String>) -> Result<Option<GqlMenu>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let effective_locale = resolve_graphql_locale(ctx, locale.as_deref());
        match MenuService::new(db.clone()).get(tenant.id, request_security_context(ctx), id, &effective_locale).await {
            Ok(menu) => Ok(Some(menu.into())),
            Err(NavigationError::MenuNotFound(_)) => Ok(None),
            Err(NavigationError::Rich(rich)) if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) => Ok(None),
            Err(error) => Err(map_query_error(error)),
        }
    }

    async fn active_menu(&self, ctx: &Context<'_>, location: GqlMenuLocation, locale: Option<String>) -> Result<Option<GqlMenu>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let Some(channel_id) = ctx.data_opt::<RequestContext>().and_then(|request| request.channel_id) else { return Ok(None); };
        let effective_locale = resolve_graphql_locale(ctx, locale.as_deref());
        match MenuBindingService::new(db.clone()).get_active(
            tenant.id, request_security_context(ctx), channel_id, location.into(), &effective_locale,
        ).await {
            Ok(menu) => Ok(menu.map(Into::into)),
            Err(NavigationError::MenuNotFound(_)) => Ok(None),
            Err(NavigationError::Rich(rich)) if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) => Ok(None),
            Err(error) => Err(map_query_error(error)),
        }
    }
}

fn request_security_context(ctx: &Context<'_>) -> SecurityContext {
    ctx.data_opt::<AuthContext>()
        .map(|auth| SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions))
        .unwrap_or_else(SecurityContext::public_read)
}

async fn require_public_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    if ctx.data_opt::<AuthContext>().is_some() { return Ok(()); }
    let Some(request) = ctx.data_opt::<RequestContext>() else { return Ok(()); };
    let Some(channel_id) = request.channel_id else { return Ok(()); };
    let enabled = ChannelService::new(ctx.data::<DatabaseConnection>()?.clone())
        .is_module_enabled(channel_id, MODULE_SLUG).await
        .map_err(|error| async_graphql::Error::new(format!("Channel module check failed: {error}"))
            .extend_with(|_, extensions| extensions.set("code", "INTERNAL_SERVER_ERROR")))?;
    if enabled { Ok(()) } else { Err(async_graphql::Error::new("Navigation is not enabled for the current channel")
        .extend_with(|_, extensions| extensions.set("code", "MODULE_NOT_ENABLED"))) }
}

fn map_query_error(error: NavigationError) -> async_graphql::Error {
    let code = match &error {
        NavigationError::Rich(rich) if rich.error_code.as_deref() == Some(MENU_TRANSLATION_INTEGRITY_ERROR_CODE) => MENU_TRANSLATION_INTEGRITY_ERROR_CODE,
        NavigationError::Forbidden(_) => "NAVIGATION_PERMISSION_DENIED",
        NavigationError::Database(_) => "INTERNAL_SERVER_ERROR",
        NavigationError::Rich(rich) => rich.error_code.as_deref().unwrap_or("NAVIGATION_OPERATION_FAILED"),
        _ => "NAVIGATION_OPERATION_FAILED",
    };
    async_graphql::Error::new(error.to_string()).extend_with(|_, extensions| extensions.set("code", code))
}

use super::types::*;
use crate::{
    CreateMenuInput, MenuBindingService, MenuItemInput, MenuItemTranslationInput, MenuLocation,
    MenuService, MenuTranslationInput, NavigationError,
};
use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::{
    AuthContext, Permission, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

const MODULE_SLUG: &str = "navigation";
const CHANNEL_CONTEXT_REQUIRED: &str = "CHANNEL_CONTEXT_REQUIRED";

#[derive(Default)]
pub struct NavigationMutation;

#[Object]
impl NavigationMutation {
    async fn create_menu(
        &self,
        ctx: &Context<'_>,
        input: CreateGqlMenuInput,
        locale: Option<String>,
    ) -> Result<GqlMenu> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_navigation_permission(ctx, Permission::NAVIGATION_CREATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        ensure_current_tenant(tenant, &auth)?;
        let effective_locale = resolve_graphql_locale(ctx, locale.as_deref());
        MenuService::new(db.clone())
            .create(
                tenant.id,
                security(&auth),
                &effective_locale,
                CreateMenuInput {
                    translations: input
                        .translations
                        .into_iter()
                        .map(|item| MenuTranslationInput {
                            locale: item.locale,
                            name: item.name,
                        })
                        .collect(),
                    location: input.location.into(),
                    items: input.items.into_iter().map(menu_item_input).collect(),
                },
            )
            .await
            .map(Into::into)
            .map_err(map_error)
    }

    async fn bind_active_menu(
        &self,
        ctx: &Context<'_>,
        input: BindGqlActiveMenuInput,
    ) -> Result<GqlActiveMenuBinding> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_navigation_permission(ctx, Permission::NAVIGATION_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        ensure_current_tenant(tenant, &auth)?;
        let channel_id = ctx
            .data_opt::<RequestContext>()
            .and_then(|request| request.channel_id)
            .ok_or_else(|| {
                async_graphql::Error::new("Active menu binding requires a resolved current channel")
                    .extend_with(|_, extensions| extensions.set("code", CHANNEL_CONTEXT_REQUIRED))
            })?;
        MenuBindingService::new(db.clone())
            .bind(
                tenant.id,
                security(&auth),
                channel_id,
                input.location.into(),
                input.menu_id,
            )
            .await
            .map(Into::into)
            .map_err(map_error)
    }
}

fn menu_item_input(input: GqlMenuItemInput) -> MenuItemInput {
    MenuItemInput {
        translations: input
            .translations
            .into_iter()
            .map(|item| MenuItemTranslationInput {
                locale: item.locale,
                title: item.title,
            })
            .collect(),
        url: input.url,
        icon: input.icon,
        position: input.position,
        children: input
            .children
            .map(|children| children.into_iter().map(menu_item_input).collect()),
    }
}
fn security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}
fn ensure_current_tenant(tenant: &TenantContext, auth: &AuthContext) -> Result<()> {
    if auth.tenant_id == tenant.id {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ))
    }
}
fn require_navigation_permission(ctx: &Context<'_>, permission: Permission) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(auth)
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: navigation:* required",
        ))
    }
}
fn map_error(error: NavigationError) -> async_graphql::Error {
    let code = match &error {
        NavigationError::MenuNotFound(_) => "MENU_NOT_FOUND",
        NavigationError::Forbidden(_) => "NAVIGATION_PERMISSION_DENIED",
        NavigationError::Database(_) => "INTERNAL_SERVER_ERROR",
        NavigationError::Rich(rich) => rich
            .error_code
            .as_deref()
            .unwrap_or("NAVIGATION_OPERATION_FAILED"),
        _ => "NAVIGATION_OPERATION_FAILED",
    };
    async_graphql::Error::new(error.to_string())
        .extend_with(|_, extensions| extensions.set("code", code))
}

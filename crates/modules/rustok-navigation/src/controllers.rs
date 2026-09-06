use crate::services::{MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE};
use crate::{
    ActiveMenuBindingResponse, BindActiveMenuInput, CreateMenuInput, MenuBindingService,
    MenuLocation, MenuResponse, MenuService, NavigationError,
};
use anyhow::Context;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, RequestContext, TenantContext,
    has_any_effective_permission,
};
use rustok_channel::ChannelService;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct GetMenuParams {
    pub locale: Option<String>,
}
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct CreateMenuParams {
    pub locale: Option<String>,
}

#[derive(Clone)]
pub struct NavigationHttpRuntime {
    db: DatabaseConnection,
}
impl NavigationHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        Ok(Self {
            db: runtime.db_clone(),
        })
    }
}
fn security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[utoipa::path(get, path = "/api/menus/{id}", tag = "navigation",
    params(("id" = Uuid, Path, description = "Menu ID"), GetMenuParams),
    responses((status = 200, body = MenuResponse), (status = 404), (status = 500)))]
pub async fn get_menu(
    State(runtime): State<NavigationHttpRuntime>,
    tenant: TenantContext,
    request: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<GetMenuParams>,
) -> HttpResult<Json<MenuResponse>> {
    ensure_enabled(&runtime, &request).await?;
    let locale = params.locale.unwrap_or_else(|| request.locale.clone());
    MenuService::new(runtime.db.clone())
        .get(
            tenant.id,
            rustok_core::SecurityContext::public_read(),
            id,
            &locale,
        )
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(get, path = "/api/menus/active/{location}", tag = "navigation",
    params(("location" = MenuLocation, Path), GetMenuParams),
    responses((status = 200, body = MenuResponse), (status = 404), (status = 500)))]
pub async fn get_active_menu(
    State(runtime): State<NavigationHttpRuntime>,
    tenant: TenantContext,
    request: RequestContext,
    Path(location): Path<MenuLocation>,
    Query(params): Query<GetMenuParams>,
) -> HttpResult<Json<MenuResponse>> {
    let channel_id = request.channel_id.ok_or_else(|| {
        HttpError::not_found("active_menu_not_found", "Active menu was not found")
    })?;
    ensure_enabled(&runtime, &request).await?;
    let locale = params.locale.unwrap_or_else(|| request.locale.clone());
    MenuBindingService::new(runtime.db.clone())
        .get_active(
            tenant.id,
            rustok_core::SecurityContext::public_read(),
            channel_id,
            location,
            &locale,
        )
        .await
        .map_err(map_error)?
        .map(Json)
        .ok_or_else(|| HttpError::not_found("active_menu_not_found", "Active menu was not found"))
}

#[utoipa::path(post, path = "/api/admin/menus", tag = "navigation", params(CreateMenuParams), request_body = CreateMenuInput,
    responses((status = 201, body = MenuResponse), (status = 400), (status = 401), (status = 403)))]
pub async fn create_menu(
    State(runtime): State<NavigationHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    Query(params): Query<CreateMenuParams>,
    Json(input): Json<CreateMenuInput>,
) -> HttpResult<(StatusCode, Json<MenuResponse>)> {
    require_permission(&auth, Permission::NAVIGATION_CREATE)?;
    let locale = params.locale.unwrap_or_else(|| request.locale.clone());
    let menu = MenuService::new(runtime.db.clone())
        .create(tenant.id, security(&auth), &locale, input)
        .await
        .map_err(map_error)?;
    Ok((StatusCode::CREATED, Json(menu)))
}

#[utoipa::path(put, path = "/api/admin/menus/active/{location}", tag = "navigation",
    params(("location" = MenuLocation, Path)), request_body = BindActiveMenuInput,
    responses((status = 200, body = ActiveMenuBindingResponse), (status = 400), (status = 401), (status = 403), (status = 404)))]
pub async fn bind_active_menu(
    State(runtime): State<NavigationHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    Path(location): Path<MenuLocation>,
    Json(input): Json<BindActiveMenuInput>,
) -> HttpResult<Json<ActiveMenuBindingResponse>> {
    require_permission(&auth, Permission::NAVIGATION_UPDATE)?;
    let channel_id = request.channel_id.ok_or_else(|| {
        HttpError::bad_request(
            "channel_context_required",
            "Active menu binding requires a resolved current channel",
        )
    })?;
    MenuBindingService::new(runtime.db.clone())
        .bind(
            tenant.id,
            security(&auth),
            channel_id,
            location,
            input.menu_id,
        )
        .await
        .map(Json)
        .map_err(map_error)
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = NavigationHttpRuntime::from_host(runtime).context("navigation HTTP runtime")?;
    Ok(axum::Router::new()
        .route("/api/menus/{id}", axum::routing::get(get_menu))
        .route(
            "/api/menus/active/{location}",
            axum::routing::get(get_active_menu),
        )
        .route("/api/admin/menus", axum::routing::post(create_menu))
        .route(
            "/api/admin/menus/active/{location}",
            axum::routing::put(bind_active_menu),
        )
        .with_state(state))
}

async fn ensure_enabled(
    runtime: &NavigationHttpRuntime,
    request: &RequestContext,
) -> HttpResult<()> {
    let Some(channel_id) = request.channel_id else {
        return Ok(());
    };
    let enabled = ChannelService::new(runtime.db.clone())
        .is_module_enabled(channel_id, "navigation")
        .await
        .map_err(|error| {
            tracing::error!(%channel_id, %error, "navigation channel gate failed");
            HttpError::internal("Unable to evaluate channel availability")
        })?;
    if enabled {
        Ok(())
    } else {
        Err(HttpError::not_found("menu_not_found", "Menu was not found"))
    }
}
fn require_permission(auth: &AuthContext, permission: Permission) -> HttpResult<()> {
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            "navigation_permission_denied",
            "Permission denied",
        ))
    }
}
fn map_error(error: NavigationError) -> HttpError {
    let message = error.to_string();
    match error {
        NavigationError::Rich(rich)
            if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
        {
            HttpError::not_found("menu_locale_not_found", message)
        }
        NavigationError::Rich(rich)
            if rich.error_code.as_deref() == Some(MENU_TRANSLATION_INTEGRITY_ERROR_CODE) =>
        {
            HttpError::internal(message)
        }
        NavigationError::MenuNotFound(_) => HttpError::not_found("menu_not_found", message),
        NavigationError::Forbidden(_) => {
            HttpError::forbidden("navigation_permission_denied", message)
        }
        NavigationError::Database(_) => HttpError::internal(message),
        _ => HttpError::bad_request("navigation_operation_failed", message),
    }
}

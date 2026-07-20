use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_fulfillment::FulfillmentService;
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListShippingOptionsParams, ListShippingProfilesParams,
};
use crate::{
    ShippingProfileService,
    dto::{
        CreateShippingOptionInput, CreateShippingProfileInput, ListShippingProfilesInput,
        ShippingOptionResponse, ShippingProfileResponse, UpdateShippingOptionInput,
        UpdateShippingProfileInput,
    },
};

/// List admin shipping profiles
#[utoipa::path(
    get,
    path = "/admin/shipping-profiles",
    tag = "admin",
    params(ListShippingProfilesParams),
    responses(
        (status = 200, description = "Shipping profiles", body = PaginatedResponse<ShippingProfileResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_shipping_profiles(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListShippingProfilesParams>,
) -> HttpResult<Json<PaginatedResponse<ShippingProfileResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (items, total) = ShippingProfileService::new(runtime.db_clone())
        .list_shipping_profiles(
            tenant.id,
            ListShippingProfilesInput {
                page: pagination.page,
                per_page: pagination.limit(),
                active: params.active,
                search: params.search,
                locale: Some(request_context.locale.clone()),
            },
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Create admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles",
    tag = "admin",
    request_body = CreateShippingProfileInput,
    responses(
        (status = 201, description = "Shipping profile created successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_shipping_profile(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateShippingProfileInput>,
) -> HttpResult<(StatusCode, Json<ShippingProfileResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    let profile = ShippingProfileService::new(runtime.db_clone())
        .create_shipping_profile(tenant.id, input)
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok((StatusCode::CREATED, Json(profile)))
}

/// Show admin shipping profile
#[utoipa::path(
    get,
    path = "/admin/shipping-profiles/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile details", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn show_shipping_profile(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let profile = ShippingProfileService::new(runtime.db_clone())
        .get_shipping_profile(
            tenant.id,
            id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Update admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    request_body = UpdateShippingProfileInput,
    responses(
        (status = 200, description = "Shipping profile updated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn update_shipping_profile(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateShippingProfileInput>,
) -> HttpResult<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(runtime.db_clone())
        .update_shipping_profile(tenant.id, id, input)
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Deactivate admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}/deactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile deactivated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn deactivate_shipping_profile(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(runtime.db_clone())
        .deactivate_shipping_profile(tenant.id, id)
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// Reactivate admin shipping profile
#[utoipa::path(
    post,
    path = "/admin/shipping-profiles/{id}/reactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping profile ID")),
    responses(
        (status = 200, description = "Shipping profile reactivated successfully", body = ShippingProfileResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping profile not found")
    )
)]
pub async fn reactivate_shipping_profile(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingProfileResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let profile = ShippingProfileService::new(runtime.db_clone())
        .reactivate_shipping_profile(tenant.id, id)
        .await
        .map_err(super::map_shipping_profile_error)?;

    Ok(Json(profile))
}

/// List admin shipping options
#[utoipa::path(
    get,
    path = "/admin/shipping-options",
    tag = "admin",
    params(ListShippingOptionsParams),
    responses(
        (status = 200, description = "Shipping options", body = PaginatedResponse<ShippingOptionResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_shipping_options(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Query(params): Query<ListShippingOptionsParams>,
) -> HttpResult<Json<PaginatedResponse<ShippingOptionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let mut items = FulfillmentService::new(runtime.db_clone())
        .list_all_shipping_options(
            tenant.id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
    if let Some(active) = params.active {
        items.retain(|option| option.active == active);
    }
    if let Some(currency_code) = params.currency_code.as_deref() {
        items.retain(|option| option.currency_code.eq_ignore_ascii_case(currency_code));
    }
    if let Some(provider_id) = params.provider_id.as_deref() {
        items.retain(|option| option.provider_id.eq_ignore_ascii_case(provider_id));
    }
    if let Some(search) = params.search.as_deref() {
        let search = search.trim().to_ascii_lowercase();
        if !search.is_empty() {
            items.retain(|option| option.name.to_ascii_lowercase().contains(&search));
        }
    }
    let total = items.len() as u64;
    let data = items
        .into_iter()
        .skip(pagination.offset() as usize)
        .take(pagination.limit() as usize)
        .collect();

    Ok(Json(PaginatedResponse {
        data,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Create admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options",
    tag = "admin",
    request_body = CreateShippingOptionInput,
    responses(
        (status = 201, description = "Shipping option created successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_shipping_option(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateShippingOptionInput>,
) -> HttpResult<(StatusCode, Json<ShippingOptionResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    super::validate_shipping_option_profile_inputs(
        runtime.db(),
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let option = FulfillmentService::new(runtime.db_clone())
        .create_shipping_option(tenant.id, input)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok((StatusCode::CREATED, Json(option)))
}

/// Show admin shipping option
#[utoipa::path(
    get,
    path = "/admin/shipping-options/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option details", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn show_shipping_option(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: rustok_api::RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let option = FulfillmentService::new(runtime.db_clone())
        .get_shipping_option(
            tenant.id,
            id,
            Some(request_context.locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
            }
            other => HttpError::bad_request("commerce_operation_failed", other.to_string()),
        })?;

    Ok(Json(option))
}

/// Update admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    request_body = UpdateShippingOptionInput,
    responses(
        (status = 200, description = "Shipping option updated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn update_shipping_option(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateShippingOptionInput>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    super::validate_shipping_option_profile_inputs(
        runtime.db(),
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let option = FulfillmentService::new(runtime.db_clone())
        .update_shipping_option(tenant.id, id, input)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
            }
            other => HttpError::bad_request("commerce_operation_failed", other.to_string()),
        })?;

    Ok(Json(option))
}

/// Deactivate admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}/deactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option deactivated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn deactivate_shipping_option(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let option = FulfillmentService::new(runtime.db_clone())
        .deactivate_shipping_option(tenant.id, id)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
            }
            other => HttpError::bad_request("commerce_operation_failed", other.to_string()),
        })?;

    Ok(Json(option))
}

/// Reactivate admin shipping option
#[utoipa::path(
    post,
    path = "/admin/shipping-options/{id}/reactivate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Shipping option ID")),
    responses(
        (status = 200, description = "Shipping option reactivated successfully", body = ShippingOptionResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Shipping option not found")
    )
)]
pub async fn reactivate_shipping_option(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let option = FulfillmentService::new(runtime.db_clone())
        .reactivate_shipping_option(tenant.id, id)
        .await
        .map_err(|err| match err {
            rustok_fulfillment::error::FulfillmentError::ShippingOptionNotFound(_) => {
                HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
            }
            other => HttpError::bad_request("commerce_operation_failed", other.to_string()),
        })?;

    Ok(Json(option))
}

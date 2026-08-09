use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_fulfillment::{
    CreateAdminShippingOptionRequest, DeactivateAdminShippingOptionRequest,
    ListAllShippingOptionProjectionsRequest, ReactivateAdminShippingOptionRequest,
    ReadShippingOptionProjectionRequest, UpdateAdminShippingOptionRequest,
};
use rustok_web::{HttpError, HttpResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListShippingOptionsParams, ListShippingProfilesParams,
};
use crate::{
    CommerceError, ShippingProfileService,
    dto::{
        CreateShippingOptionInput, CreateShippingProfileInput, ListShippingProfilesInput,
        ShippingOptionResponse, ShippingProfileResponse, UpdateShippingOptionInput,
        UpdateShippingProfileInput,
    },
};

const ADMIN_SHIPPING_OPTION_OWNER: &str = "rustok_fulfillment.admin_shipping_options";
const ADMIN_SHIPPING_BOUNDARY: &str = "commerce_admin_shipping_http";

struct AdminShippingOptionErrorContext {
    tenant_id: Uuid,
    shipping_option_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminShippingOptionErrorContext {
    fn new(tenant_id: Uuid, shipping_option_id: Option<Uuid>, operation: &'static str) -> Self {
        Self {
            tenant_id,
            shipping_option_id,
            operation,
        }
    }
}

struct AdminShippingOptionDiagnosticContext {
    tenant_id: &'static str,
    shipping_option_id: &'static str,
    operation: &'static str,
}

impl From<&AdminShippingOptionErrorContext> for AdminShippingOptionDiagnosticContext {
    fn from(context: &AdminShippingOptionErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            shipping_option_id: optional_uuid_shape(context.shipping_option_id),
            operation: context.operation,
        }
    }
}

struct AdminShippingOptionPortDiagnosticContext {
    correlation_id: &'static str,
    actor: &'static str,
    channel: &'static str,
    locale: usize,
    deadline_ms: Option<u64>,
}

impl From<&PortContext> for AdminShippingOptionPortDiagnosticContext {
    fn from(context: &PortContext) -> Self {
        Self {
            correlation_id: text_presence_shape(context.correlation_id.as_str()),
            actor: text_presence_shape(context.actor.id.as_str()),
            channel: optional_text_presence_shape(context.channel.as_deref()),
            locale: context.locale.len(),
            deadline_ms: context.deadline_ms,
        }
    }
}

struct AdminShippingOptionPortDiagnosticError<'a> {
    code: &'a str,
    retryable: bool,
}

impl std::fmt::Debug for AdminShippingOptionPortDiagnosticError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct AdminShippingDiagnosticError;

impl std::fmt::Debug for AdminShippingDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn text_presence_shape(value: &str) -> &'static str {
    if value.is_empty() {
        "empty"
    } else {
        "present_non_empty"
    }
}

fn optional_text_presence_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "present_empty",
        Some(_) => "present_non_empty",
    }
}

fn map_shipping_profile_error(error: CommerceError) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        CommerceError::ShippingProfileNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::DuplicateShippingProfileSlug(_) => (
            StatusCode::CONFLICT,
            "commerce_admin_shipping_profile_conflict",
            "A shipping profile with this slug already exists",
            "duplicate_slug",
        ),
        CommerceError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_shipping_profile_invalid",
            "Shipping profile request is invalid",
            "validation",
        ),
        CommerceError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_shipping_profile_storage_unavailable",
            "Shipping profile storage is temporarily unavailable",
            "database",
        ),
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::DuplicateHandle { .. }
        | CommerceError::DuplicateSku(_)
        | CommerceError::InvalidPrice(_)
        | CommerceError::InsufficientInventory { .. }
        | CommerceError::InvalidOptionCombination
        | CommerceError::NoVariants
        | CommerceError::CannotDeletePublished
        | CommerceError::Rich(_)
        | CommerceError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_shipping_profile_failed",
            "Shipping profile operation could not be completed safely",
            "unexpected_commerce_error",
        ),
    };
    let error = AdminShippingDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = "rustok_commerce.shipping_profile",
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_SHIPPING_BOUNDARY,
        "commerce admin shipping profile operation failed"
    );
    HttpError::new(status, code, message)
}

fn admin_shipping_option_command_idempotency_key<T: Serialize>(
    tenant_id: Uuid,
    actor_id: Uuid,
    shipping_option_id: Option<Uuid>,
    operation: &'static str,
    payload: &T,
) -> HttpResult<String> {
    let payload = serde_json::to_vec(payload).map_err(|_| {
        let error = AdminShippingDiagnosticError;
        tracing::error!(
            error = ?error,
            owner = ADMIN_SHIPPING_OPTION_OWNER,
            tenant_id = %uuid_shape(tenant_id),
            shipping_option_id = %optional_uuid_shape(shipping_option_id),
            operation,
            error_kind = "request_identity_serialization",
            public_code = "commerce_admin_fulfillment_failed",
            boundary = ADMIN_SHIPPING_BOUNDARY,
            "commerce admin shipping option command identity could not be materialized"
        );
        HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
            "Fulfillment operation could not be completed safely",
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(actor_id.as_bytes());
    digest.update(operation.as_bytes());
    if let Some(shipping_option_id) = shipping_option_id {
        digest.update(shipping_option_id.as_bytes());
    }
    digest.update(payload);
    Ok(format!(
        "commerce-admin-shipping-option:{operation}:{}",
        hex::encode(digest.finalize())
    ))
}

fn admin_shipping_option_read_port_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    shipping_option_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let resource_id = shipping_option_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-shipping-option:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_shipping_option_command_port_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    shipping_option_id: Option<Uuid>,
    operation: &'static str,
    idempotency_key: String,
) -> PortContext {
    admin_shipping_option_read_port_context(
        tenant_id,
        auth,
        request_context,
        shipping_option_id,
        operation,
    )
    .with_idempotency_key(idempotency_key)
}

fn map_admin_shipping_option_port_error(
    context: AdminShippingOptionErrorContext,
    port_context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_fulfillment_invalid",
            "Fulfillment request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
            "Fulfillment operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
            "Fulfillment storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
            "Fulfillment operation could not be completed safely",
            "invariant_violation",
        ),
    };
    let context = AdminShippingOptionDiagnosticContext::from(&context);
    let port_context = AdminShippingOptionPortDiagnosticContext::from(port_context);
    let error = AdminShippingOptionPortDiagnosticError {
        code: error.code.as_str(),
        retryable: error.retryable,
    };
    tracing::error!(
        error = ?error,
        owner = ADMIN_SHIPPING_OPTION_OWNER,
        owner_operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %context.tenant_id,
        shipping_option_id = ?context.shipping_option_id,
        operation = %context.operation,
        actor = ?port_context.actor,
        channel = ?port_context.channel,
        locale = %port_context.locale,
        deadline_ms = ?port_context.deadline_ms,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_SHIPPING_BOUNDARY,
        "commerce admin shipping option owner call failed"
    );
    HttpError::new(status, code, message)
}

async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> HttpResult<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}

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
    request_context: RequestContext,
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
        .map_err(map_shipping_profile_error)?;

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
        .map_err(map_shipping_profile_error)?;

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
    request_context: RequestContext,
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
        .map_err(map_shipping_profile_error)?;

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
        .map_err(map_shipping_profile_error)?;

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
        .map_err(map_shipping_profile_error)?;

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
        .map_err(map_shipping_profile_error)?;

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
    request_context: RequestContext,
    Query(params): Query<ListShippingOptionsParams>,
) -> HttpResult<Json<PaginatedResponse<ShippingOptionResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let read_context = admin_shipping_option_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        None,
        "list_shipping_options",
    );
    let mut items = runtime
        .shipping_option_admin_read_port()
        .list_all_shipping_option_projections(
            read_context.clone(),
            ListAllShippingOptionProjectionsRequest {
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(tenant.id, None, "list_shipping_options"),
                &read_context,
                "list_all_shipping_option_projections",
                error,
            )
        })?;
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
    request_context: RequestContext,
    Json(input): Json<CreateShippingOptionInput>,
) -> HttpResult<(StatusCode, Json<ShippingOptionResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    validate_shipping_option_profile_inputs(
        runtime.db(),
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let request = CreateAdminShippingOptionRequest { input };
    let idempotency_key = admin_shipping_option_command_idempotency_key(
        tenant.id,
        auth.user_id,
        None,
        "create_shipping_option",
        &request,
    )?;
    let command_context = admin_shipping_option_command_port_context(
        tenant.id,
        &auth,
        &request_context,
        None,
        "create_shipping_option",
        idempotency_key,
    );
    let option = runtime
        .shipping_option_admin_command_port()
        .create_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(tenant.id, None, "create_shipping_option"),
                &command_context,
                "create_admin_shipping_option",
                error,
            )
        })?;

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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let read_context = admin_shipping_option_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "get_shipping_option",
    );
    let option = runtime
        .shipping_option_read_port()
        .read_shipping_option_projection(
            read_context.clone(),
            ReadShippingOptionProjectionRequest {
                shipping_option_id: id,
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(tenant.id, Some(id), "get_shipping_option"),
                &read_context,
                "read_shipping_option_projection",
                error,
            )
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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateShippingOptionInput>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    validate_shipping_option_profile_inputs(
        runtime.db(),
        tenant.id,
        input.allowed_shipping_profile_slugs.as_ref(),
    )
    .await?;

    let request = UpdateAdminShippingOptionRequest {
        shipping_option_id: id,
        input,
    };
    let idempotency_key = admin_shipping_option_command_idempotency_key(
        tenant.id,
        auth.user_id,
        Some(id),
        "update_shipping_option",
        &request,
    )?;
    let command_context = admin_shipping_option_command_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "update_shipping_option",
        idempotency_key,
    );
    let option = runtime
        .shipping_option_admin_command_port()
        .update_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(tenant.id, Some(id), "update_shipping_option"),
                &command_context,
                "update_admin_shipping_option",
                error,
            )
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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let request = DeactivateAdminShippingOptionRequest {
        shipping_option_id: id,
    };
    let idempotency_key = admin_shipping_option_command_idempotency_key(
        tenant.id,
        auth.user_id,
        Some(id),
        "deactivate_shipping_option",
        &request,
    )?;
    let command_context = admin_shipping_option_command_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "deactivate_shipping_option",
        idempotency_key,
    );
    let option = runtime
        .shipping_option_admin_command_port()
        .deactivate_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(
                    tenant.id,
                    Some(id),
                    "deactivate_shipping_option",
                ),
                &command_context,
                "deactivate_admin_shipping_option",
                error,
            )
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
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ShippingOptionResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let request = ReactivateAdminShippingOptionRequest {
        shipping_option_id: id,
    };
    let idempotency_key = admin_shipping_option_command_idempotency_key(
        tenant.id,
        auth.user_id,
        Some(id),
        "reactivate_shipping_option",
        &request,
    )?;
    let command_context = admin_shipping_option_command_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "reactivate_shipping_option",
        idempotency_key,
    );
    let option = runtime
        .shipping_option_admin_command_port()
        .reactivate_shipping_option(command_context.clone(), request)
        .await
        .map_err(|error| {
            map_admin_shipping_option_port_error(
                AdminShippingOptionErrorContext::new(
                    tenant.id,
                    Some(id),
                    "reactivate_shipping_option",
                ),
                &command_context,
                "reactivate_admin_shipping_option",
                error,
            )
        })?;

    Ok(Json(option))
}

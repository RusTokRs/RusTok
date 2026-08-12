use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_fulfillment::{
    CancelAdminFulfillmentRequest, DeliverAdminFulfillmentRequest, ReopenAdminFulfillmentRequest,
    ReshipAdminFulfillmentRequest, ShipAdminFulfillmentRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{super::CommerceHttpRuntime, super::common::ensure_permissions};
use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, DeliverFulfillmentInput, FulfillmentResponse,
    ReopenFulfillmentInput, ReshipFulfillmentInput, ShipFulfillmentInput,
};
use crate::services::AdminManualFulfillmentOrchestrationService;

const ADMIN_FULFILLMENT_COMMAND_OWNER: &str = "rustok_fulfillment.admin_command";
const ADMIN_FULFILLMENT_CREATE_OWNER: &str = "rustok_fulfillment.admin_create_command";
const ADMIN_FULFILLMENT_COMMAND_BOUNDARY: &str = "commerce_admin_fulfillment_command_http";

type AdminFulfillmentCommandHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

fn admin_fulfillment_command_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    fulfillment_id: Uuid,
    operation: &'static str,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-fulfillment-command:{operation}:{fulfillment_id}"),
    )
    .with_idempotency_key(format!("admin-fulfillment:{fulfillment_id}:{operation}"))
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_fulfillment_create_read_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    order_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-fulfillment-create-read:{order_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn admin_fulfillment_create_command_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    input: &CreateFulfillmentInput,
) -> Result<PortContext, HttpError> {
    let payload = serde_json::to_vec(input).map_err(|_| {
        HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_fulfillment_failed",
            "Fulfillment operation could not be completed safely",
        )
    })?;
    let first = fnv1a64(&payload, 0xcbf29ce484222325);
    let second = fnv1a64(&payload, 0x84222325cbf29ce4);
    let context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!(
            "commerce-admin-fulfillment-create-command:{}",
            input.order_id
        ),
    )
    .with_idempotency_key(format!(
        "admin-fulfillment:create:{}:{first:016x}{second:016x}",
        input.order_id
    ))
    .with_deadline(std::time::Duration::from_secs(2));
    Ok(match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    })
}

fn fnv1a64(bytes: &[u8], offset_basis: u64) -> u64 {
    bytes.iter().fold(offset_basis, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn fulfillment_command_error_policy(error: &PortError) -> AdminFulfillmentCommandHttpPolicy {
    match error.code.as_str() {
        "fulfillment.reconciliation_required" => (
            StatusCode::CONFLICT,
            "commerce_admin_fulfillment_reconciliation_required",
            "Fulfillment operation requires reconciliation",
            "reconciliation_required",
        ),
        "fulfillment.database_unavailable" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
            "Fulfillment storage is temporarily unavailable",
            "database",
        ),
        "fulfillment.invalid_transition" => (
            StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
            "Fulfillment operation conflicts with the current state",
            "state_conflict",
        ),
        _ => match &error.kind {
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
        },
    }
}

fn map_fulfillment_command_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    fulfillment_id: Uuid,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = fulfillment_command_error_policy(&error);
    tracing::error!(
        owner = ADMIN_FULFILLMENT_COMMAND_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        fulfillment_id_non_nil = !fulfillment_id.is_nil(),
        operation,
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        "commerce admin fulfillment owner command failed"
    );
    HttpError::new(status, code, message)
}

fn map_fulfillment_create_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    order_id: Uuid,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = fulfillment_command_error_policy(&error);
    tracing::error!(
        owner = ADMIN_FULFILLMENT_CREATE_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        order_id_non_nil = !order_id.is_nil(),
        operation = "create_fulfillment",
        correlation_id = %context.correlation_id,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        "commerce admin fulfillment cross-owner create failed"
    );
    HttpError::new(status, code, message)
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments",
    tag = "admin",
    request_body = CreateFulfillmentInput,
    responses(
        (status = 201, description = "Fulfillment created", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Json(input): Json<CreateFulfillmentInput>,
) -> HttpResult<(StatusCode, Json<FulfillmentResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;
    let order_id = input.order_id;
    let read_context =
        admin_fulfillment_create_read_context(&tenant, &auth, &request_context, order_id);
    let write_context =
        admin_fulfillment_create_command_context(&tenant, &auth, &request_context, &input)?;
    let service = AdminManualFulfillmentOrchestrationService::new(
        runtime.order_read_port(),
        runtime.fulfillment_read_port(),
        runtime.shipping_option_read_port(),
        runtime.fulfillment_admin_create_command_port(),
    );
    let fulfillment = service
        .create_manual_fulfillment(read_context, write_context.clone(), input)
        .await
        .map_err(|error| {
            map_fulfillment_create_error(tenant.id, auth.user_id, order_id, &write_context, error)
        })?;
    Ok((StatusCode::CREATED, Json(fulfillment)))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/ship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ShipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment shipped", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn ship_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ShipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    let context = admin_fulfillment_command_context(&tenant, &auth, &request_context, id, "ship");
    let fulfillment = runtime
        .fulfillment_admin_command_port()
        .ship_fulfillment(
            context.clone(),
            ShipAdminFulfillmentRequest {
                fulfillment_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "ship_fulfillment",
                &context,
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/deliver",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = DeliverFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment delivered", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn deliver_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, id, "deliver");
    let fulfillment = runtime
        .fulfillment_admin_command_port()
        .deliver_fulfillment(
            context.clone(),
            DeliverAdminFulfillmentRequest {
                fulfillment_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "deliver_fulfillment",
                &context,
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reopen",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReopenFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment reopened", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reopen_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReopenFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    let context = admin_fulfillment_command_context(&tenant, &auth, &request_context, id, "reopen");
    let fulfillment = runtime
        .fulfillment_admin_command_port()
        .reopen_fulfillment(
            context.clone(),
            ReopenAdminFulfillmentRequest {
                fulfillment_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "reopen_fulfillment",
                &context,
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/reship",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = ReshipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment marked for reship", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reship_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReshipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    let context = admin_fulfillment_command_context(&tenant, &auth, &request_context, id, "reship");
    let fulfillment = runtime
        .fulfillment_admin_command_port()
        .reship_fulfillment(
            context.clone(),
            ReshipAdminFulfillmentRequest {
                fulfillment_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "reship_fulfillment",
                &context,
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments/{id}/cancel",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    request_body = CancelFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment cancelled", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn cancel_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    let context = admin_fulfillment_command_context(&tenant, &auth, &request_context, id, "cancel");
    let fulfillment = runtime
        .fulfillment_admin_command_port()
        .cancel_fulfillment(
            context.clone(),
            CancelAdminFulfillmentRequest {
                fulfillment_id: id,
                input,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_command_error(
                tenant.id,
                auth.user_id,
                id,
                "cancel_fulfillment",
                &context,
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

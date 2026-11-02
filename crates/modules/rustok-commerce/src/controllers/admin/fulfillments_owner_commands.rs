use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::{
    AuthContext, Permission, PortContext, PortError, PortErrorKind, RequestContext, TenantContext,
};
use rustok_fulfillment::{
    CancelAdminFulfillmentRequest, DeliverAdminFulfillmentRequest,
    ListFulfillmentProjectionsRequest, ReadFulfillmentProjectionRequest,
    ReopenAdminFulfillmentRequest, ReshipAdminFulfillmentRequest, ShipAdminFulfillmentRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListFulfillmentsParams,
};
use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, DeliverFulfillmentInput, FulfillmentResponse,
    ReopenFulfillmentInput, ReshipFulfillmentInput, ShipFulfillmentInput,
};
use crate::services::AdminManualFulfillmentOrchestrationService;

const ADMIN_FULFILLMENT_COMMAND_OWNER: &str = "rustok_fulfillment.admin_command";
const ADMIN_FULFILLMENT_CREATE_OWNER: &str = "rustok_fulfillment.admin_create_command";
const ADMIN_FULFILLMENT_COMMAND_BOUNDARY: &str = "commerce_admin_fulfillment_command_http";

type AdminFulfillmentCommandHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

/// Shape-only facts a fulfillment route entry line may carry.
#[derive(Default)]
struct AdminFulfillmentRouteFacts {
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    status_filter_present: bool,
    customer_filter_present: bool,
    carrier_length: Option<usize>,
}

/// Record that a fulfillment route was entered without repeating any caller value.
///
/// The request span already carries tenant and actor identity, so this line records only whether
/// they are non-nil, which resource the route addresses, and the shape of the caller's filters. The
/// carrier is caller text, so only its length is kept.
fn log_admin_fulfillment_route_entry(
    operation: &'static str,
    tenant_id: Uuid,
    actor_id: Uuid,
    facts: AdminFulfillmentRouteFacts,
) {
    tracing::debug!(
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        operation,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        fulfillment_id_present = facts.fulfillment_id.is_some(),
        fulfillment_id_non_nil = facts.fulfillment_id.is_some_and(|value| !value.is_nil()),
        order_id_present = facts.order_id.is_some(),
        order_id_non_nil = facts.order_id.is_some_and(|value| !value.is_nil()),
        status_filter_present = facts.status_filter_present,
        customer_filter_present = facts.customer_filter_present,
        carrier_length = facts.carrier_length.unwrap_or_default(),
        "commerce admin fulfillment route entered with bounded diagnostics"
    );
}

fn require_idempotency_key(headers: &HeaderMap) -> Result<String, HttpError> {
    let value = headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_required",
                "Idempotency-Key header is required for this write operation",
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_invalid",
                "Idempotency-Key header is invalid",
            )
        })?
        .trim()
        .to_string();

    if value.is_empty() || value.len() > 191 {
        return Err(HttpError::new(
            StatusCode::BAD_REQUEST,
            "commerce_admin_idempotency_key_invalid",
            "Idempotency-Key header must contain 1 to 191 bytes",
        ));
    }

    Ok(value)
}

fn admin_fulfillment_command_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
    idempotency_key: String,
) -> HttpResult<PortContext> {
    Ok(
        admin_fulfillment_read_context(tenant, auth, request_context)?
            .with_idempotency_key(idempotency_key),
    )
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
                StatusCode::FORBIDDEN,
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
                StatusCode::CONFLICT,
                "commerce_admin_fulfillment_reconciliation_required",
                "Fulfillment operation requires reconciliation",
                "reconciliation_required",
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
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        "commerce admin fulfillment owner command failed"
    );
    HttpError::new(status, code, message).with_retryable(error.retryable)
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
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        "commerce admin fulfillment cross-owner create failed"
    );
    HttpError::new(status, code, message).with_retryable(error.retryable)
}

fn admin_fulfillment_read_context(
    tenant: &TenantContext,
    auth: &AuthContext,
    request_context: &RequestContext,
) -> HttpResult<PortContext> {
    if tenant.id.is_nil() || auth.tenant_id != tenant.id || request_context.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "commerce_admin_fulfillment_context_mismatch",
            "Fulfillment request tenant, authenticated tenant and canonical request scope must agree".to_string(),
        ));
    }
    let mut context = PortContext::new(
        tenant.id.to_string(),
        auth.port_actor(),
        request_context.locale.as_str(),
        request_context.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    context.claims = auth
        .permissions
        .iter()
        .map(ToString::to_string)
        .chain(auth.scopes.iter().cloned())
        .collect();
    Ok(match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    })
}

fn map_fulfillment_read_error(
    tenant_id: Uuid,
    actor_id: Uuid,
    fulfillment_id: Option<Uuid>,
    operation: &'static str,
    context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = fulfillment_command_error_policy(&error);
    tracing::error!(
        owner = ADMIN_FULFILLMENT_COMMAND_OWNER,
        tenant_id_non_nil = !tenant_id.is_nil(),
        actor_id_non_nil = !actor_id.is_nil(),
        fulfillment_id_present = fulfillment_id.is_some(),
        operation,
        correlation_id = %context.correlation_id,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_COMMAND_BOUNDARY,
        "commerce admin fulfillment owner read failed"
    );
    HttpError::new(status, code, message).with_retryable(error.retryable)
}

/// List admin fulfillments
#[utoipa::path(
    get,
    path = "/admin/fulfillments",
    tag = "admin",
    params(ListFulfillmentsParams),
    responses(
        (status = 200, description = "Fulfillments", body = PaginatedResponse<FulfillmentResponse>),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden")
    )
)]
pub async fn list_fulfillments(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListFulfillmentsParams>,
) -> HttpResult<Json<PaginatedResponse<FulfillmentResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "list_fulfillments",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            status_filter_present: params.status.is_some(),
            order_id: params.order_id,
            customer_filter_present: params.customer_id.is_some(),
            ..Default::default()
        },
    );

    let pagination = params.pagination.unwrap_or_default();
    let read_context = admin_fulfillment_read_context(&tenant, &auth, &request_context)?;
    let page = runtime
        .fulfillment_read_port()
        .list_fulfillment_projections(
            read_context.clone(),
            ListFulfillmentProjectionsRequest {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|error| {
            map_fulfillment_read_error(
                tenant.id,
                auth.user_id,
                None,
                "list_fulfillments",
                &read_context,
                error,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: page.items,
        meta: super::super::common::PaginationMeta::new(
            pagination.page,
            pagination.limit(),
            page.total,
        ),
    }))
}

/// Show admin fulfillment
#[utoipa::path(
    get,
    path = "/admin/fulfillments/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    responses(
        (status = 200, description = "Fulfillment details", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn show_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "show_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            ..Default::default()
        },
    );

    let read_context = admin_fulfillment_read_context(&tenant, &auth, &request_context)?;
    let fulfillment = runtime
        .fulfillment_read_port()
        .read_fulfillment_projection(
            read_context.clone(),
            ReadFulfillmentProjectionRequest { fulfillment_id: id },
        )
        .await
        .map_err(|error| {
            map_fulfillment_read_error(
                tenant.id,
                auth.user_id,
                Some(id),
                "get_fulfillment",
                &read_context,
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillments",
    tag = "admin",
    params(("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes")),
    request_body = CreateFulfillmentInput,
    responses(
        (status = 201, description = "Fulfillment created", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Order not found")
    )
)]
pub async fn create_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Json(input): Json<CreateFulfillmentInput>,
) -> HttpResult<(StatusCode, Json<FulfillmentResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;
    let order_id = input.order_id;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if order_id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Order ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "create_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            order_id: Some(order_id),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let read_context = admin_fulfillment_read_context(&tenant, &auth, &request_context)?;
    let write_context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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
    params(
        ("id" = Uuid, Path, description = "Fulfillment ID"),
        ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes"),
    ),
    request_body = ShipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment shipped", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn ship_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ShipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "ship_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            carrier_length: Some(input.carrier.chars().count()),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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
    params(
        ("id" = Uuid, Path, description = "Fulfillment ID"),
        ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes"),
    ),
    request_body = DeliverFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment delivered", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn deliver_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<DeliverFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "deliver_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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
    params(
        ("id" = Uuid, Path, description = "Fulfillment ID"),
        ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes"),
    ),
    request_body = ReopenFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment reopened", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reopen_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReopenFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "reopen_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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
    params(
        ("id" = Uuid, Path, description = "Fulfillment ID"),
        ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes"),
    ),
    request_body = ReshipFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment marked for reship", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn reship_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReshipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "reship_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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
    params(
        ("id" = Uuid, Path, description = "Fulfillment ID"),
        ("Idempotency-Key" = String, Header, description = "Stable write operation identity, maximum 191 bytes"),
    ),
    request_body = CancelFulfillmentInput,
    responses(
        (status = 200, description = "Fulfillment cancelled", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"), (status = 403, description = "Forbidden"),
        (status = 404, description = "Fulfillment not found")
    )
)]
pub async fn cancel_fulfillment(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CancelFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_tenant",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "commerce_invalid_id",
            "Fulfillment ID must not be nil",
        ));
    }
    log_admin_fulfillment_route_entry(
        "cancel_fulfillment",
        tenant.id,
        auth.user_id,
        AdminFulfillmentRouteFacts {
            fulfillment_id: Some(id),
            ..Default::default()
        },
    );
    let idempotency_key = require_idempotency_key(&headers)?;
    let context =
        admin_fulfillment_command_context(&tenant, &auth, &request_context, idempotency_key)?;
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

#[cfg(test)]
mod shipment_context_tests {
    use super::*;

    fn contexts() -> (TenantContext, AuthContext, RequestContext) {
        let id = Uuid::new_v4();
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: id,
            permissions: vec![],
            client_id: Some(Uuid::new_v4()),
            scopes: vec!["fulfillments:update".into()],
            grant_type: "client_credentials".into(),
        };
        let tenant = TenantContext {
            id,
            name: "tenant".into(),
            slug: "tenant".into(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "fr".into(),
            is_active: true,
        };
        let request = RequestContext {
            tenant_id: id,
            user_id: Some(auth.user_id),
            channel_id: None,
            channel_slug: Some("backoffice".into()),
            channel_resolution_source: None,
            locale: "fr".into(),
            correlation_id: "canonical-request".into(),
        };
        (tenant, auth, request)
    }

    #[test]
    fn shipment_http_context_preserves_canonical_actor_claims_scope_and_literal_key() {
        let (tenant, auth, request) = contexts();
        let context = admin_fulfillment_command_context(
            &tenant,
            &auth,
            &request,
            "legacy:literal:key".into(),
        )
        .expect("coherent context");
        assert_eq!(context.tenant_id, tenant.id.to_string());
        assert_eq!(context.actor, auth.port_actor());
        assert_eq!(context.claims, auth.scopes);
        assert!(context.roles.is_empty(), "never invent HTTP roles");
        assert_eq!(context.channel.as_deref(), Some("backoffice"));
        assert_eq!(context.correlation_id, request.correlation_id);
        assert_eq!(context.locale, "fr");
        assert_eq!(context.deadline_ms, Some(2_000));
        assert_eq!(
            context.idempotency_key.as_deref(),
            Some("legacy:literal:key")
        );
    }

    #[test]
    fn shipment_http_context_rejects_disagreeing_or_nil_tenants_before_selected_calls() {
        let (tenant, auth, request) = contexts();
        let mut foreign_auth = auth.clone();
        foreign_auth.tenant_id = Uuid::new_v4();
        let mut foreign_request = request.clone();
        foreign_request.tenant_id = Uuid::new_v4();
        let mut nil_tenant = tenant.clone();
        nil_tenant.id = Uuid::nil();
        assert!(admin_fulfillment_read_context(&tenant, &foreign_auth, &request).is_err());
        assert!(admin_fulfillment_read_context(&tenant, &auth, &foreign_request).is_err());
        assert!(
            admin_fulfillment_command_context(&nil_tenant, &auth, &request, "unchanged".into())
                .is_err()
        );
    }
}

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
    FulfillmentError, FulfillmentService, ListFulfillmentProjectionsRequest,
    ReadFulfillmentProjectionRequest,
};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime,
    super::common::{PaginatedResponse, ensure_permissions},
    ListFulfillmentsParams,
};
use crate::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationService,
    dto::{
        CancelFulfillmentInput, CreateFulfillmentInput, DeliverFulfillmentInput,
        FulfillmentResponse, ReopenFulfillmentInput, ReshipFulfillmentInput, ShipFulfillmentInput,
    },
};

const ADMIN_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_routes";
const ADMIN_FULFILLMENT_ORCHESTRATION_OWNER: &str =
    "rustok_commerce.admin_fulfillment_orchestration";
const ADMIN_FULFILLMENT_BOUNDARY: &str = "commerce_admin_fulfillment_http";

struct AdminFulfillmentErrorContext {
    tenant_id: Uuid,
    fulfillment_id: Option<Uuid>,
    order_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminFulfillmentErrorContext {
    fn new(
        tenant_id: Uuid,
        fulfillment_id: Option<Uuid>,
        order_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            fulfillment_id,
            order_id,
            operation,
        }
    }
}

struct AdminFulfillmentDiagnosticContext {
    tenant_id: &'static str,
    fulfillment_id: &'static str,
    order_id: &'static str,
    operation: &'static str,
}

impl From<&AdminFulfillmentErrorContext> for AdminFulfillmentDiagnosticContext {
    fn from(context: &AdminFulfillmentErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            fulfillment_id: optional_uuid_shape(context.fulfillment_id),
            order_id: optional_uuid_shape(context.order_id),
            operation: context.operation,
        }
    }
}

struct AdminFulfillmentPortDiagnosticContext {
    correlation_id: &'static str,
    actor: &'static str,
    channel: &'static str,
    locale: usize,
    deadline_ms: Option<u64>,
}

impl From<&PortContext> for AdminFulfillmentPortDiagnosticContext {
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

struct AdminFulfillmentPortDiagnosticError<'a> {
    code: &'a str,
    retryable: bool,
}

impl std::fmt::Debug for AdminFulfillmentPortDiagnosticError<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct AdminFulfillmentDiagnosticError;

impl std::fmt::Debug for AdminFulfillmentDiagnosticError {
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

fn admin_fulfillment_read_port_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    fulfillment_id: Option<Uuid>,
    operation: &'static str,
) -> PortContext {
    let resource_id = fulfillment_id.unwrap_or(tenant_id);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        format!("commerce-admin-fulfillment:{operation}:{resource_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn map_admin_fulfillment_port_error(
    context: AdminFulfillmentErrorContext,
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
    let context = AdminFulfillmentDiagnosticContext::from(&context);
    let port_context = AdminFulfillmentPortDiagnosticContext::from(port_context);
    let error = AdminFulfillmentPortDiagnosticError {
        code: error.code.as_str(),
        retryable: error.retryable,
    };
    tracing::error!(
        error = ?error,
        owner = ADMIN_FULFILLMENT_OWNER,
        owner_operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %context.tenant_id,
        fulfillment_id = ?context.fulfillment_id,
        order_id = ?context.order_id,
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
        boundary = ADMIN_FULFILLMENT_BOUNDARY,
        "commerce admin fulfillment owner read failed"
    );
    HttpError::new(status, code, message)
}

/// List admin fulfillments
#[utoipa::path(
    get,
    path = "/admin/fulfillments",
    tag = "admin",
    params(ListFulfillmentsParams),
    responses(
        (status = 200, description = "Fulfillments", body = PaginatedResponse<FulfillmentResponse>),
        (status = 401, description = "Unauthorized")
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

    let pagination = params.pagination.unwrap_or_default();
    let read_context = admin_fulfillment_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        None,
        "list_fulfillments",
    );
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
            map_admin_fulfillment_port_error(
                AdminFulfillmentErrorContext::new(tenant.id, None, None, "list_fulfillments"),
                &read_context,
                "list_fulfillment_projections",
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

/// Create admin fulfillment
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
    Json(input): Json<CreateFulfillmentInput>,
) -> HttpResult<(StatusCode, Json<FulfillmentResponse>)> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_CREATE],
        "Permission denied: fulfillments:create required",
    )?;

    let order_id = input.order_id;
    let fulfillment = FulfillmentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .create_manual_fulfillment(tenant.id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_orchestration_error(
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    None,
                    Some(order_id),
                    "create_manual_fulfillment",
                ),
                error,
            )
        })?;

    Ok((StatusCode::CREATED, Json(fulfillment)))
}

/// Show admin fulfillment
#[utoipa::path(
    get,
    path = "/admin/fulfillments/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Fulfillment ID")),
    responses(
        (status = 200, description = "Fulfillment details", body = FulfillmentResponse),
        (status = 401, description = "Unauthorized"),
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

    let read_context = admin_fulfillment_read_port_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "get_fulfillment",
    );
    let fulfillment = runtime
        .fulfillment_read_port()
        .read_fulfillment_projection(
            read_context.clone(),
            ReadFulfillmentProjectionRequest { fulfillment_id: id },
        )
        .await
        .map_err(|error| {
            map_admin_fulfillment_port_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "get_fulfillment"),
                &read_context,
                "read_fulfillment_projection",
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

/// Ship admin fulfillment
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
    Path(id): Path<Uuid>,
    Json(input): Json<ShipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .ship_fulfillment(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_orchestration_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "ship_fulfillment"),
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

/// Deliver admin fulfillment
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
    Path(id): Path<Uuid>,
    Json(input): Json<DeliverFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(runtime.db_clone())
        .deliver_fulfillment(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "deliver_fulfillment"),
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

/// Reopen admin fulfillment
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
    Path(id): Path<Uuid>,
    Json(input): Json<ReopenFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentService::new(runtime.db_clone())
        .reopen_fulfillment(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "reopen_fulfillment"),
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

/// Reship admin fulfillment
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
    Path(id): Path<Uuid>,
    Json(input): Json<ReshipFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .reship_fulfillment(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_orchestration_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "reship_fulfillment"),
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

/// Cancel admin fulfillment
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
    Path(id): Path<Uuid>,
    Json(input): Json<CancelFulfillmentInput>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_UPDATE],
        "Permission denied: fulfillments:update required",
    )?;

    let fulfillment = FulfillmentOrchestrationService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .cancel_fulfillment(tenant.id, id, input)
        .await
        .map_err(|error| {
            map_admin_fulfillment_orchestration_error(
                AdminFulfillmentErrorContext::new(tenant.id, Some(id), None, "cancel_fulfillment"),
                error,
            )
        })?;

    Ok(Json(fulfillment))
}

fn map_admin_fulfillment_error(
    context: AdminFulfillmentErrorContext,
    error: FulfillmentError,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        FulfillmentError::Validation(_) => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_admin_fulfillment_invalid",
            "Fulfillment request is invalid",
            "validation",
        ),
        FulfillmentError::ShippingOptionNotFound(_) | FulfillmentError::FulfillmentNotFound(_) => (
            axum::http::StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        FulfillmentError::InvalidTransition { .. } => (
            axum::http::StatusCode::CONFLICT,
            "commerce_admin_fulfillment_state_conflict",
            "Fulfillment operation conflicts with the current state",
            "state_conflict",
        ),
        FulfillmentError::Database(_) => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_fulfillment_storage_unavailable",
            "Fulfillment storage is temporarily unavailable",
            "database",
        ),
    };
    let context = AdminFulfillmentDiagnosticContext::from(&context);
    let error = AdminFulfillmentDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_FULFILLMENT_OWNER,
        tenant_id = %context.tenant_id,
        fulfillment_id = ?context.fulfillment_id,
        order_id = ?context.order_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_FULFILLMENT_BOUNDARY,
        "commerce admin fulfillment owner operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_admin_fulfillment_orchestration_error(
    context: AdminFulfillmentErrorContext,
    error: FulfillmentOrchestrationError,
) -> HttpError {
    match error {
        FulfillmentOrchestrationError::Fulfillment(error) => {
            map_admin_fulfillment_error(context, error)
        }
        error => {
            let mut context = context;
            if let FulfillmentOrchestrationError::ProviderAfterPersistence {
                fulfillment_id, ..
            }
            | FulfillmentOrchestrationError::PersistenceAfterProvider {
                fulfillment_id, ..
            } = &error
            {
                context.fulfillment_id = Some(*fulfillment_id);
            }
            let (status, code, message, error_kind) = match &error {
                FulfillmentOrchestrationError::OrderNotFound(_) => (
                    axum::http::StatusCode::NOT_FOUND,
                    "commerce_admin_not_found",
                    "Commerce resource not found",
                    "order_not_found",
                ),
                FulfillmentOrchestrationError::Database(_) => (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "commerce_admin_fulfillment_storage_unavailable",
                    "Fulfillment storage is temporarily unavailable",
                    "database",
                ),
                FulfillmentOrchestrationError::Validation(_) => (
                    axum::http::StatusCode::BAD_REQUEST,
                    "commerce_admin_fulfillment_invalid",
                    "Fulfillment request is invalid",
                    "validation",
                ),
                FulfillmentOrchestrationError::ProviderAfterPersistence { .. }
                | FulfillmentOrchestrationError::PersistenceAfterProvider { .. } => (
                    axum::http::StatusCode::CONFLICT,
                    "commerce_admin_fulfillment_reconciliation_required",
                    "Fulfillment operation requires reconciliation",
                    "reconciliation_required",
                ),
                FulfillmentOrchestrationError::Fulfillment(_) => unreachable!(
                    "nested fulfillment errors are handled before orchestration mapping"
                ),
            };
            let context = AdminFulfillmentDiagnosticContext::from(&context);
            let error = AdminFulfillmentDiagnosticError;
            tracing::error!(
                error = ?error,
                owner = ADMIN_FULFILLMENT_ORCHESTRATION_OWNER,
                tenant_id = %context.tenant_id,
                fulfillment_id = ?context.fulfillment_id,
                order_id = ?context.order_id,
                operation = %context.operation,
                error_kind,
                public_code = code,
                status = %status,
                boundary = ADMIN_FULFILLMENT_BOUNDARY,
                "commerce admin fulfillment orchestration failed"
            );
            HttpError::new(status, code, message)
        }
    }
}

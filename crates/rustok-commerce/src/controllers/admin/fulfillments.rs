use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext};
use rustok_fulfillment::{FulfillmentError, FulfillmentService};
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
        FulfillmentResponse, ListFulfillmentsInput, ReopenFulfillmentInput, ReshipFulfillmentInput,
        ShipFulfillmentInput,
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
    Query(params): Query<ListFulfillmentsParams>,
) -> HttpResult<Json<PaginatedResponse<FulfillmentResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let pagination = params.pagination.unwrap_or_default();
    let (fulfillments, total) = FulfillmentService::new(runtime.db_clone())
        .list_fulfillments(
            tenant.id,
            ListFulfillmentsInput {
                page: pagination.page,
                per_page: pagination.limit(),
                status: params.status,
                order_id: params.order_id,
                customer_id: params.customer_id,
            },
        )
        .await
        .map_err(|error| {
            map_admin_fulfillment_error(
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    None,
                    None,
                    "list_fulfillments",
                ),
                error,
            )
        })?;

    Ok(Json(PaginatedResponse {
        data: fulfillments,
        meta: super::super::common::PaginationMeta::new(pagination.page, pagination.limit(), total),
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
    Path(id): Path<Uuid>,
) -> HttpResult<Json<FulfillmentResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::FULFILLMENTS_READ],
        "Permission denied: fulfillments:read required",
    )?;

    let fulfillment = FulfillmentService::new(runtime.db_clone())
        .get_fulfillment(tenant.id, id)
        .await
        .map_err(|error| {
            map_admin_fulfillment_error(
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "get_fulfillment",
                ),
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
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "ship_fulfillment",
                ),
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
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "deliver_fulfillment",
                ),
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
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "reopen_fulfillment",
                ),
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
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "reship_fulfillment",
                ),
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
                AdminFulfillmentErrorContext::new(
                    tenant.id,
                    Some(id),
                    None,
                    "cancel_fulfillment",
                ),
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
        FulfillmentError::ShippingOptionNotFound(_)
        | FulfillmentError::FulfillmentNotFound(_) => (
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

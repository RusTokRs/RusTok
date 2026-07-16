use axum::{
    extract::{Path, Query, State},
    Json,
};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::HttpResult;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    common::{ensure_permissions, PaginatedResponse, PaginationMeta, PaginationParams},
    CommerceHttpRuntime,
};
use crate::services::{
    ListReturnCompletionOperationsInput, ReturnCompletionOperationResponse,
};
use crate::{OrderReturnResponse, ReturnCompletionOrchestrationService};

#[derive(Clone, Debug, Default, Deserialize, ToSchema, IntoParams)]
pub struct AdminListReturnCompletionOperationsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub status: Option<String>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route("/", axum::routing::get(list_return_completion_operations))
        .route("/{id}", axum::routing::get(show_return_completion_operation))
        .route(
            "/{id}/retry",
            axum::routing::post(retry_return_completion_operation),
        )
}

#[utoipa::path(
    get,
    path = "/admin/return-completion-operations",
    tag = "admin",
    params(AdminListReturnCompletionOperationsParams),
    responses(
        (status = 200, description = "Return completion operations", body = PaginatedResponse<ReturnCompletionOperationResponse>),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_return_completion_operations(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<AdminListReturnCompletionOperationsParams>,
) -> HttpResult<Json<PaginatedResponse<ReturnCompletionOperationResponse>>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let pagination = params.pagination.unwrap_or_default();
    let (items, total) = ReturnCompletionOrchestrationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .list_operations(
        tenant.id,
        ListReturnCompletionOperationsInput {
            page: pagination.page,
            per_page: pagination.limit(),
            status: params.status,
        },
    )
    .await
    .map_err(super::admin::map_post_order_orchestration_error)?;

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

#[utoipa::path(
    get,
    path = "/admin/return-completion-operations/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return completion operation ID")),
    responses(
        (status = 200, description = "Return completion operation", body = ReturnCompletionOperationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return completion operation not found")
    )
)]
pub async fn show_return_completion_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ReturnCompletionOperationResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let operation = ReturnCompletionOrchestrationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .get_operation(tenant.id, id)
    .await
    .map_err(super::admin::map_post_order_orchestration_error)?;
    Ok(Json(operation))
}

#[utoipa::path(
    post,
    path = "/admin/return-completion-operations/{id}/retry",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Return completion operation ID")),
    responses(
        (status = 200, description = "Return completion retried", body = OrderReturnResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Return completion operation not found"),
        (status = 409, description = "Operation is leased or requires reconciliation")
    )
)]
pub async fn retry_return_completion_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<OrderReturnResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_MANAGE, Permission::PAYMENTS_MANAGE],
        "Permission denied: orders:manage and payments:manage required",
    )?;
    let order_return = ReturnCompletionOrchestrationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .retry_operation(tenant.id, auth.user_id, id)
    .await
    .map_err(super::admin::map_post_order_orchestration_error)?;
    Ok(Json(order_return))
}

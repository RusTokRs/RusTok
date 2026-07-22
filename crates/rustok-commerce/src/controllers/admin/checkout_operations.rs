use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_cart::in_process_cart_checkout_port;
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{CommerceHttpRuntime, common::ensure_permissions};

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutOperationResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub status: String,
    pub stage: String,
    pub order_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub attempt_count: i32,
    pub lease_expires_at: Option<DateTime<FixedOffset>>,
    pub last_error_code: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepInput {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepFailure {
    pub operation_id: Uuid,
    pub manual_reconciliation: bool,
    pub error_code: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdminCheckoutCompensationSweepResponse {
    pub scanned: usize,
    pub compensated: usize,
    pub retryable: usize,
    pub manual_reconciliation: usize,
    pub failures: Vec<AdminCheckoutCompensationSweepFailure>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route(
            "/compensation-sweep",
            axum::routing::post(sweep_checkout_compensations),
        )
        .route("/{id}", axum::routing::get(show_checkout_operation))
        .route(
            "/{id}/compensate",
            axum::routing::post(compensate_checkout_operation),
        )
}

#[utoipa::path(
    get,
    path = "/admin/checkout-operations/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Checkout operation", body = AdminCheckoutOperationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Checkout operation not found")
    )
)]
pub async fn show_checkout_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminCheckoutOperationResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_READ],
        "Permission denied: orders:read required",
    )?;
    let operation = crate::CheckoutOperationJournal::new(runtime.db_clone())
        .get(tenant.id, id)
        .await
        .map_err(map_operation_error)?;
    Ok(Json(map_operation(operation)))
}

#[utoipa::path(
    post,
    path = "/admin/checkout-operations/{id}/compensate",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Checkout operation compensated", body = AdminCheckoutOperationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Checkout operation not found"),
        (status = 409, description = "Compensation requires retry or manual reconciliation")
    )
)]
pub async fn compensate_checkout_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<AdminCheckoutOperationResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_MANAGE],
        "Permission denied: orders:manage required",
    )?;
    let service = crate::CheckoutCompensationService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone()),
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry());
    let operation = service
        .compensate(
            tenant.id,
            auth.user_id,
            id,
            format!(
                "admin-checkout-compensation:{}:{}",
                auth.user_id,
                Uuid::new_v4()
            ),
        )
        .await
        .map_err(map_compensation_error)?;
    Ok(Json(map_operation(operation)))
}

#[utoipa::path(
    post,
    path = "/admin/checkout-operations/compensation-sweep",
    tag = "admin",
    request_body = AdminCheckoutCompensationSweepInput,
    responses(
        (status = 200, description = "Checkout compensation sweep report", body = AdminCheckoutCompensationSweepResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn sweep_checkout_compensations(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<AdminCheckoutCompensationSweepInput>,
) -> HttpResult<Json<AdminCheckoutCompensationSweepResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::ORDERS_MANAGE],
        "Permission denied: orders:manage required",
    )?;
    let report = crate::CheckoutCompensationSweepService::new(
        runtime.db_clone(),
        runtime.event_bus(),
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone()),
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(runtime.payment_provider_registry())
    .run(
        tenant.id,
        auth.user_id,
        format!("admin:{}", auth.user_id),
        input.limit,
    )
    .await
    .map_err(|_| HttpError::internal("Checkout compensation storage is unavailable"))?;

    Ok(Json(AdminCheckoutCompensationSweepResponse {
        scanned: report.scanned,
        compensated: report.compensated,
        retryable: report.retryable,
        manual_reconciliation: report.manual_reconciliation,
        failures: report
            .failures
            .into_iter()
            .map(|failure| AdminCheckoutCompensationSweepFailure {
                operation_id: failure.operation_id,
                manual_reconciliation: failure.manual_reconciliation,
                error_code: failure.error_code,
            })
            .collect(),
    }))
}

fn map_operation(
    operation: crate::entities::checkout_operation::Model,
) -> AdminCheckoutOperationResponse {
    AdminCheckoutOperationResponse {
        id: operation.id,
        cart_id: operation.cart_id,
        status: operation.status,
        stage: operation.stage,
        order_id: operation.order_id,
        payment_collection_id: operation.payment_collection_id,
        attempt_count: operation.attempt_count,
        lease_expires_at: operation.lease_expires_at,
        last_error_code: operation.last_error_code,
        created_at: operation.created_at,
        updated_at: operation.updated_at,
        completed_at: operation.completed_at,
    }
}

fn map_operation_error(error: crate::CheckoutOperationError) -> HttpError {
    match error {
        crate::CheckoutOperationError::NotFound(_) => HttpError::not_found(
            "checkout_operation_not_found",
            "Checkout operation not found",
        ),
        crate::CheckoutOperationError::Conflict(message) => {
            HttpError::new(StatusCode::CONFLICT, "checkout_operation_conflict", message)
        }
        crate::CheckoutOperationError::Validation(message) => {
            HttpError::bad_request("checkout_operation_invalid", message)
        }
        crate::CheckoutOperationError::Database(_) => {
            HttpError::internal("Checkout operation storage is unavailable")
        }
    }
}

fn map_compensation_error(error: crate::CheckoutCompensationError) -> HttpError {
    match error {
        crate::CheckoutCompensationError::Operation(error) => map_operation_error(error),
        crate::CheckoutCompensationError::ManualReconciliation(_) => HttpError::new(
            StatusCode::CONFLICT,
            "checkout_reconciliation_required",
            "Checkout requires manual reconciliation",
        ),
        crate::CheckoutCompensationError::Conflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            "checkout_compensation_conflict",
            "Checkout compensation cannot proceed from the current state",
        ),
        crate::CheckoutCompensationError::Boundary {
            retryable: true, ..
        }
        | crate::CheckoutCompensationError::ReservationJournal(_) => HttpError::new(
            StatusCode::CONFLICT,
            "checkout_compensation_pending",
            "Checkout compensation will be retried",
        ),
        crate::CheckoutCompensationError::Boundary { .. }
        | crate::CheckoutCompensationError::CompensationAndJournal { .. } => {
            HttpError::internal("Checkout compensation is unavailable")
        }
    }
}

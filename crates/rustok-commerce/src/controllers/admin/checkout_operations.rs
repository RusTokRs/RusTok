use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_cart::in_process_cart_checkout_port;
use rustok_order::error::OrderError;
use rustok_payment::error::PaymentError;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DbErr;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{CommerceHttpRuntime, common::ensure_permissions};
use crate::{
    CheckoutCompensationError, CheckoutInventoryReservationError, CheckoutOperationError,
    PaymentOrchestrationError,
};

const ADMIN_CHECKOUT_OPERATION_OWNER: &str = "rustok_commerce.admin_checkout_operation";
const ADMIN_CHECKOUT_OPERATION_BOUNDARY: &str = "commerce_admin_checkout_operation_http";

type AdminCheckoutOperationHttpPolicy = (
    StatusCode,
    &'static str,
    &'static str,
    &'static str,
);

struct AdminCheckoutOperationErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    checkout_operation_id: Option<Uuid>,
    reservation_id: Option<Uuid>,
    payment_collection_id: Option<Uuid>,
    payment_id: Option<Uuid>,
    refund_id: Option<Uuid>,
    order_id: Option<Uuid>,
    order_return_id: Option<Uuid>,
    order_change_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminCheckoutOperationErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        checkout_operation_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            checkout_operation_id,
            reservation_id: None,
            payment_collection_id: None,
            payment_id: None,
            refund_id: None,
            order_id: None,
            order_return_id: None,
            order_change_id: None,
            operation,
        }
    }
}

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
        .map_err(|error| {
            map_operation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "show_checkout_operation",
                ),
                error,
            )
        })?;
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
        .map_err(|error| {
            map_compensation_error(
                AdminCheckoutOperationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "compensate_checkout_operation",
                ),
                error,
            )
        })?;
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
    .map_err(|error| {
        map_sweep_error(
            AdminCheckoutOperationErrorContext::new(
                tenant.id,
                auth.user_id,
                None,
                "sweep_checkout_compensations",
            ),
            error,
        )
    })?;

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

fn checkout_operation_error_policy(
    error: &CheckoutOperationError,
) -> AdminCheckoutOperationHttpPolicy {
    match error {
        CheckoutOperationError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "checkout_operation_not_found",
            "Checkout operation not found",
            "not_found",
        ),
        CheckoutOperationError::Conflict(_) => (
            StatusCode::CONFLICT,
            "checkout_operation_conflict",
            "Checkout operation conflicts with the current state",
            "conflict",
        ),
        CheckoutOperationError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "checkout_operation_invalid",
            "Checkout operation request is invalid",
            "validation",
        ),
        CheckoutOperationError::Database(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Checkout operation storage is unavailable",
            "database",
        ),
    }
}

fn payment_error_policy(error: &PaymentError) -> AdminCheckoutOperationHttpPolicy {
    match error {
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PaymentError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
            "validation",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_state_conflict",
            "Payment operation conflicts with the current state",
            "state_conflict",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
            "provider_unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_invalid_response",
            "Payment provider returned an invalid response; reconciliation may be required",
            "provider_invalid_response",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome is unknown and requires reconciliation",
            "provider_outcome_unknown",
        ),
        PaymentError::ProviderConfiguration { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
            "provider_configuration",
        ),
        PaymentError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
            "database",
        ),
    }
}

fn reserved_refund_error_policy(error: &PaymentError) -> AdminCheckoutOperationHttpPolicy {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderInvalidResponse { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
            "refund_reconciliation_required",
        ),
        PaymentError::ProviderUnavailable { .. } => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_refund_provider_unavailable",
            "Refund remains reserved and the provider operation may be retried safely",
            "refund_provider_unavailable",
        ),
        error => payment_error_policy(error),
    }
}

fn order_error_policy(error: &OrderError) -> AdminCheckoutOperationHttpPolicy {
    match error {
        OrderError::Validation(_) => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_order_invalid",
            "Order request is invalid",
            "validation",
        ),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        OrderError::InvalidTransition { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_order_state_conflict",
            "Order operation conflicts with the current state",
            "state_conflict",
        ),
        OrderError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_order_storage_unavailable",
            "Order storage is temporarily unavailable",
            "database",
        ),
        OrderError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_order_failed",
            "Order operation could not be completed safely",
            "core",
        ),
    }
}

fn adopt_operation_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &CheckoutOperationError,
) {
    if let CheckoutOperationError::NotFound(id) = error {
        context.checkout_operation_id = Some(*id);
    }
}

fn adopt_reservation_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &CheckoutInventoryReservationError,
) {
    if let CheckoutInventoryReservationError::NotFound(id) = error {
        context.reservation_id = Some(*id);
    }
}

fn adopt_payment_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &PaymentError,
) {
    match error {
        PaymentError::PaymentCollectionNotFound(id) => context.payment_collection_id = Some(*id),
        PaymentError::PaymentNotFound(id) => context.payment_id = Some(*id),
        PaymentError::RefundNotFound(id) => context.refund_id = Some(*id),
        _ => {}
    }
}

fn adopt_order_error_identity(
    context: &mut AdminCheckoutOperationErrorContext,
    error: &OrderError,
) {
    match error {
        OrderError::OrderNotFound(id) => context.order_id = Some(*id),
        OrderError::OrderReturnNotFound(id) => context.order_return_id = Some(*id),
        OrderError::OrderChangeNotFound(id) => context.order_change_id = Some(*id),
        _ => {}
    }
}

fn admin_checkout_operation_http_error<E>(
    context: &AdminCheckoutOperationErrorContext,
    error: &E,
    source_owner: &'static str,
    policy: AdminCheckoutOperationHttpPolicy,
    log_message: &'static str,
) -> HttpError
where
    E: std::fmt::Debug,
{
    let (status, code, message, error_kind) = policy;
    tracing::error!(
        error = ?error,
        owner = ADMIN_CHECKOUT_OPERATION_OWNER,
        source_owner,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        checkout_operation_id = ?context.checkout_operation_id,
        reservation_id = ?context.reservation_id,
        payment_collection_id = ?context.payment_collection_id,
        payment_id = ?context.payment_id,
        refund_id = ?context.refund_id,
        order_id = ?context.order_id,
        order_return_id = ?context.order_return_id,
        order_change_id = ?context.order_change_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_CHECKOUT_OPERATION_BOUNDARY,
        "{log_message}"
    );
    HttpError::new(status, code, message)
}

fn map_operation_error(
    mut context: AdminCheckoutOperationErrorContext,
    error: CheckoutOperationError,
) -> HttpError {
    adopt_operation_error_identity(&mut context, &error);
    let policy = checkout_operation_error_policy(&error);
    admin_checkout_operation_http_error(
        &context,
        &error,
        "rustok_commerce.checkout_operation",
        policy,
        "commerce admin checkout operation lookup failed",
    )
}

fn map_compensation_error(
    mut context: AdminCheckoutOperationErrorContext,
    error: CheckoutCompensationError,
) -> HttpError {
    let (policy, source_owner) = match &error {
        CheckoutCompensationError::Operation(source) => {
            adopt_operation_error_identity(&mut context, source);
            (
                checkout_operation_error_policy(source),
                "rustok_commerce.checkout_operation",
            )
        }
        CheckoutCompensationError::ReservationJournal(source) => {
            adopt_reservation_error_identity(&mut context, source);
            (
                (
                    StatusCode::CONFLICT,
                    "checkout_compensation_pending",
                    "Checkout compensation will be retried",
                    "reservation_journal",
                ),
                "rustok_commerce.checkout_inventory_reservation",
            )
        }
        CheckoutCompensationError::Payment(source) => {
            adopt_payment_error_identity(&mut context, source);
            (payment_error_policy(source), "rustok_payment")
        }
        CheckoutCompensationError::PaymentOrchestration(source) => match source {
            PaymentOrchestrationError::Provider(source)
            | PaymentOrchestrationError::Payment(source) => {
                adopt_payment_error_identity(&mut context, source);
                (payment_error_policy(source), "rustok_payment")
            }
            PaymentOrchestrationError::ProviderAfterRefundReservation {
                refund_id,
                source,
            } => {
                context.refund_id = Some(*refund_id);
                (reserved_refund_error_policy(source), "rustok_payment")
            }
        },
        CheckoutCompensationError::Order(source) => {
            adopt_order_error_identity(&mut context, source);
            (order_error_policy(source), "rustok_order")
        }
        CheckoutCompensationError::ManualReconciliation(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_reconciliation_required",
                "Checkout requires manual reconciliation",
                "manual_reconciliation",
            ),
            "rustok_commerce.checkout_compensation",
        ),
        CheckoutCompensationError::Conflict(_) => (
            (
                StatusCode::CONFLICT,
                "checkout_compensation_conflict",
                "Checkout compensation cannot proceed from the current state",
                "conflict",
            ),
            "rustok_commerce.checkout_compensation",
        ),
        CheckoutCompensationError::Boundary {
            retryable: true, ..
        } => (
            (
                StatusCode::CONFLICT,
                "checkout_compensation_pending",
                "Checkout compensation will be retried",
                "retryable_boundary",
            ),
            "rustok_commerce.checkout_compensation",
        ),
        CheckoutCompensationError::Boundary { .. }
        | CheckoutCompensationError::CompensationAndJournal { .. } => (
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Checkout compensation is unavailable",
                "compensation_failed",
            ),
            "rustok_commerce.checkout_compensation",
        ),
    };

    admin_checkout_operation_http_error(
        &context,
        &error,
        source_owner,
        policy,
        "commerce admin checkout compensation failed",
    )
}

fn map_sweep_error(
    context: AdminCheckoutOperationErrorContext,
    error: DbErr,
) -> HttpError {
    admin_checkout_operation_http_error(
        &context,
        &error,
        "rustok_commerce.checkout_compensation_sweep",
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Checkout compensation storage is unavailable",
            "database",
        ),
        "commerce admin checkout compensation sweep failed",
    )
}

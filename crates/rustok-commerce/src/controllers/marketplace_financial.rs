use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{CommerceHttpRuntime, common::ensure_permissions};

#[derive(Clone, Debug, Default, Deserialize, IntoParams, ToSchema)]
pub struct MarketplaceFinancialOperatorListQuery {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct MarketplaceFinancialSweepInput {
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceFinancialOperationResponse {
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub currency_code: String,
    pub status: String,
    pub stage: String,
    pub attempt_count: i32,
    pub ledger_transaction_id: Option<Uuid>,
    pub ledger_debit_total_amount: Option<i64>,
    pub ledger_credit_total_amount: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplacePaidEventResponse {
    pub id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub captured_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub captured_amount: Decimal,
    pub status: String,
    pub attempt_count: i32,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceFinancialSweepFailureResponse {
    pub inbox_id: Uuid,
    pub retryable: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MarketplaceFinancialSweepResponse {
    pub selected: usize,
    pub processed: usize,
    pub retryable_failures: usize,
    pub operator_review_failures: usize,
    pub failures: Vec<MarketplaceFinancialSweepFailureResponse>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route(
            "/operations/operator-review",
            axum::routing::get(list_financial_operator_review),
        )
        .route(
            "/operations/{id}",
            axum::routing::get(show_financial_operation),
        )
        .route(
            "/operations/{id}/retry",
            axum::routing::post(retry_financial_operation),
        )
        .route(
            "/paid-events/operator-review",
            axum::routing::get(list_paid_event_operator_review),
        )
        .route(
            "/paid-events/{id}",
            axum::routing::get(show_paid_event),
        )
        .route(
            "/paid-events/{id}/retry",
            axum::routing::post(retry_paid_event),
        )
        .route(
            "/recovery-sweep",
            axum::routing::post(run_recovery_sweep),
        )
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/operations/operator-review",
    tag = "admin-marketplace-financial",
    params(MarketplaceFinancialOperatorListQuery),
    responses(
        (status = 200, description = "Marketplace financial operations requiring operator review", body = [MarketplaceFinancialOperationResponse]),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_financial_operator_review(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<MarketplaceFinancialOperatorListQuery>,
) -> HttpResult<Json<Vec<MarketplaceFinancialOperationResponse>>> {
    ensure_read_permission(&auth)?;
    let service = runtime.marketplace_financial_operator_service();
    let items = service
        .list_financial_operator_review(tenant.id, query.limit.unwrap_or(50))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/operations/{id}",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Marketplace financial operation", body = MarketplaceFinancialOperationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Operation not found")
    )
)]
pub async fn show_financial_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceFinancialOperationResponse>> {
    ensure_read_permission(&auth)?;
    runtime
        .marketplace_financial_operator_service()
        .get_financial_operation(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/operations/{id}/retry",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Checkout operation ID")),
    responses(
        (status = 200, description = "Marketplace financial operation reset for safe retry", body = MarketplaceFinancialOperationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Operation is not safely retryable")
    )
)]
pub async fn retry_financial_operation(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplaceFinancialOperationResponse>> {
    ensure_manage_permission(&auth)?;
    runtime
        .marketplace_financial_operator_service()
        .retry_financial_operation(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/paid-events/operator-review",
    tag = "admin-marketplace-financial",
    params(MarketplaceFinancialOperatorListQuery),
    responses(
        (status = 200, description = "Paid events requiring operator review", body = [MarketplacePaidEventResponse]),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_paid_event_operator_review(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<MarketplaceFinancialOperatorListQuery>,
) -> HttpResult<Json<Vec<MarketplacePaidEventResponse>>> {
    ensure_read_permission(&auth)?;
    let items = runtime
        .marketplace_financial_operator_service()
        .list_paid_event_operator_review(tenant.id, query.limit.unwrap_or(50))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    get,
    path = "/admin/marketplace-financial/paid-events/{id}",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Paid-event inbox ID")),
    responses(
        (status = 200, description = "Marketplace paid event", body = MarketplacePaidEventResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Paid event not found")
    )
)]
pub async fn show_paid_event(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplacePaidEventResponse>> {
    ensure_read_permission(&auth)?;
    runtime
        .marketplace_financial_operator_service()
        .get_paid_event(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/paid-events/{id}/retry",
    tag = "admin-marketplace-financial",
    params(("id" = Uuid, Path, description = "Paid-event inbox ID")),
    responses(
        (status = 200, description = "Paid event processed after an explicit safe retry", body = MarketplacePaidEventResponse),
        (status = 401, description = "Unauthorized"),
        (status = 409, description = "Paid event is not safely retryable")
    )
)]
pub async fn retry_paid_event(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<MarketplacePaidEventResponse>> {
    ensure_manage_permission(&auth)?;
    runtime
        .marketplace_financial_operator_service()
        .retry_paid_event(tenant.id, id)
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_operator_error)
}

#[utoipa::path(
    post,
    path = "/admin/marketplace-financial/recovery-sweep",
    tag = "admin-marketplace-financial",
    request_body = MarketplaceFinancialSweepInput,
    responses(
        (status = 200, description = "Bounded tenant-scoped marketplace financial recovery sweep", body = MarketplaceFinancialSweepResponse),
        (status = 401, description = "Unauthorized"),
        (status = 503, description = "Recovery storage unavailable")
    )
)]
pub async fn run_recovery_sweep(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<MarketplaceFinancialSweepInput>,
) -> HttpResult<Json<MarketplaceFinancialSweepResponse>> {
    ensure_manage_permission(&auth)?;
    let report = runtime
        .marketplace_financial_operator_service()
        .sweep_tenant(tenant.id, input.limit.unwrap_or(100))
        .await
        .map_err(map_operator_error)?;
    Ok(Json(MarketplaceFinancialSweepResponse {
        selected: report.selected,
        processed: report.processed,
        retryable_failures: report.retryable_failures,
        operator_review_failures: report.operator_review_failures,
        failures: report
            .failures
            .into_iter()
            .map(|failure| MarketplaceFinancialSweepFailureResponse {
                inbox_id: failure.inbox_id,
                retryable: failure.retryable,
            })
            .collect(),
    }))
}

fn ensure_read_permission(auth: &AuthContext) -> HttpResult<()> {
    ensure_permissions(
        auth,
        &[Permission::PAYMENTS_READ],
        "Permission denied: payments:read required",
    )
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    ensure_permissions(
        auth,
        &[Permission::PAYMENTS_MANAGE],
        "Permission denied: payments:manage required",
    )
}

fn map_operator_error(error: crate::MarketplaceFinancialOperatorError) -> HttpError {
    match error {
        crate::MarketplaceFinancialOperatorError::Validation(_) => HttpError::bad_request(
            "marketplace_financial_operator_invalid",
            "Marketplace financial operator request is invalid",
        ),
        crate::MarketplaceFinancialOperatorError::Conflict(message)
            if message.contains("was not found") =>
        {
            HttpError::not_found(
                "marketplace_financial_operator_not_found",
                "Marketplace financial operation or paid event was not found",
            )
        }
        crate::MarketplaceFinancialOperatorError::Conflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            "marketplace_financial_operator_conflict",
            "Marketplace financial operation requires reconciliation or is not safely retryable",
        ),
        crate::MarketplaceFinancialOperatorError::Database(_) => {
            HttpError::internal("Marketplace financial operator storage is unavailable")
        }
        crate::MarketplaceFinancialOperatorError::Inbox(error) => map_inbox_error(error),
    }
}

fn map_inbox_error(error: crate::MarketplacePaidEventInboxError) -> HttpError {
    if error.retryable() {
        HttpError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "marketplace_financial_recovery_unavailable",
            "Marketplace financial recovery is temporarily unavailable",
        )
    } else {
        HttpError::new(
            StatusCode::CONFLICT,
            "marketplace_financial_reconciliation_required",
            "Marketplace financial recovery requires operator review",
        )
    }
}

impl From<crate::MarketplaceFinancialOperationOperatorView>
    for MarketplaceFinancialOperationResponse
{
    fn from(value: crate::MarketplaceFinancialOperationOperatorView) -> Self {
        Self {
            checkout_operation_id: value.checkout_operation_id,
            order_id: value.order_id,
            payment_collection_id: value.payment_collection_id,
            currency_code: value.currency_code,
            status: value.status,
            stage: value.stage,
            attempt_count: value.attempt_count,
            ledger_transaction_id: value.ledger_transaction_id,
            ledger_debit_total_amount: value.ledger_debit_total_amount,
            ledger_credit_total_amount: value.ledger_credit_total_amount,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            created_at: value.created_at,
            updated_at: value.updated_at,
            completed_at: value.completed_at,
        }
    }
}

impl From<crate::MarketplacePaidEventOperatorView> for MarketplacePaidEventResponse {
    fn from(value: crate::MarketplacePaidEventOperatorView) -> Self {
        Self {
            id: value.id,
            event_source: value.event_source,
            event_id: value.event_id,
            checkout_operation_id: value.checkout_operation_id,
            order_id: value.order_id,
            payment_collection_id: value.payment_collection_id,
            captured_at: value.captured_at,
            currency_code: value.currency_code,
            captured_amount: value.captured_amount,
            status: value.status,
            attempt_count: value.attempt_count,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            created_at: value.created_at,
            updated_at: value.updated_at,
            processed_at: value.processed_at,
        }
    }
}

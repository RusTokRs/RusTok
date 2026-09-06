use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_fulfillment::providers::FulfillmentProviderOperationResult;
use rustok_fulfillment::{
    FulfillmentError, FulfillmentProviderOperationRecovery, entities::provider_operation,
};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CommerceHttpRuntime;
use crate::{
    FulfillmentCreateLabelRecoveryService, FulfillmentOrchestrationError,
    FulfillmentReconciliationService,
};

const ADMIN_RECONCILIATION_FULFILLMENT_OWNER: &str = "rustok_fulfillment.admin_reconciliation";
const ADMIN_RECONCILIATION_ORCHESTRATION_OWNER: &str = "rustok_commerce.fulfillment_reconciliation";
const ADMIN_RECONCILIATION_BOUNDARY: &str = "commerce_admin_reconciliation_http";

#[derive(Clone, Copy)]
struct AdminReconciliationErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    provider_operation_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminReconciliationErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        provider_operation_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            provider_operation_id,
            operation,
        }
    }
}

struct AdminReconciliationDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    provider_operation_id: &'static str,
    operation: &'static str,
}

impl From<&AdminReconciliationErrorContext> for AdminReconciliationDiagnosticContext {
    fn from(context: &AdminReconciliationErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            provider_operation_id: optional_uuid_shape(context.provider_operation_id),
            operation: context.operation,
        }
    }
}

struct AdminReconciliationDiagnosticError;

impl std::fmt::Debug for AdminReconciliationDiagnosticError {
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

#[derive(Debug, Clone, Deserialize)]
pub struct ListReconciliationParams {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuarantineStaleInput {
    pub stale_after_seconds: u64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveUnknownFailedInput {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveUnknownSucceededInput {
    pub provider_result: FulfillmentProviderOperationResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuarantineStaleResponse {
    pub quarantined: u64,
}

pub fn axum_router() -> Router<CommerceHttpRuntime> {
    Router::new()
        .route("/reconciliation", get(list_reconciliation_required))
        .route("/quarantine-stale", post(quarantine_stale_executing))
        .route("/{id}/resolve-failed", post(resolve_unknown_as_failed))
        .route(
            "/{id}/resolve-succeeded",
            post(resolve_unknown_as_succeeded),
        )
        .route("/{id}/retry-local", post(retry_local_persistence))
        .route("/{id}/retry-create-label", post(retry_create_label))
}

async fn list_reconciliation_required(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListReconciliationParams>,
) -> HttpResult<Json<Vec<provider_operation::Model>>> {
    require_manage_permission(&auth)?;
    let operations = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .list_reconciliation_required(tenant.id, params.limit.unwrap_or(100))
        .await
        .map_err(|error| {
            map_reconciliation_fulfillment_error(
                AdminReconciliationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    None,
                    "list_reconciliation_required",
                ),
                error,
            )
        })?;
    Ok(Json(operations))
}

async fn quarantine_stale_executing(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<QuarantineStaleInput>,
) -> HttpResult<Json<QuarantineStaleResponse>> {
    require_manage_permission(&auth)?;
    let stale_after_seconds = input.stale_after_seconds.clamp(60, 7 * 24 * 60 * 60);
    let stale_before = Utc::now() - Duration::seconds(stale_after_seconds as i64);
    let quarantined = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .quarantine_stale_executing(tenant.id, stale_before, input.limit.unwrap_or(100))
        .await
        .map_err(|error| {
            map_reconciliation_fulfillment_error(
                AdminReconciliationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    None,
                    "quarantine_stale_executing",
                ),
                error,
            )
        })?;
    Ok(Json(QuarantineStaleResponse { quarantined }))
}

async fn resolve_unknown_as_failed(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownFailedInput>,
) -> HttpResult<Json<provider_operation::Model>> {
    require_manage_permission(&auth)?;
    let operation = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .resolve_unknown_as_failed(tenant.id, operation_id, input.reason)
        .await
        .map_err(|error| {
            map_reconciliation_fulfillment_error(
                AdminReconciliationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(operation_id),
                    "resolve_unknown_as_failed",
                ),
                error,
            )
        })?;
    Ok(Json(operation))
}

async fn resolve_unknown_as_succeeded(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownSucceededInput>,
) -> HttpResult<Json<provider_operation::Model>> {
    require_manage_permission(&auth)?;
    let context = AdminReconciliationErrorContext::new(
        tenant.id,
        auth.user_id,
        Some(operation_id),
        "resolve_unknown_as_succeeded",
    );
    let provider_reference = input.provider_result.external_reference.clone();
    let provider_result = serde_json::to_value(input.provider_result)
        .map_err(|error| map_provider_result_encoding_error(context, error))?;
    let operation = FulfillmentProviderOperationRecovery::new(runtime.db_clone())
        .resolve_unknown_as_succeeded(tenant.id, operation_id, provider_reference, provider_result)
        .await
        .map_err(|error| map_reconciliation_fulfillment_error(context, error))?;
    Ok(Json(operation))
}

async fn retry_local_persistence(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
) -> HttpResult<Json<crate::dto::FulfillmentResponse>> {
    require_manage_permission(&auth)?;
    let fulfillment = FulfillmentReconciliationService::new(runtime.db_clone())
        .retry_local_persistence(tenant.id, operation_id)
        .await
        .map_err(|error| {
            map_reconciliation_orchestration_error(
                AdminReconciliationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(operation_id),
                    "retry_local_persistence",
                ),
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

async fn retry_create_label(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
) -> HttpResult<Json<crate::dto::FulfillmentResponse>> {
    require_manage_permission(&auth)?;
    let fulfillment = FulfillmentCreateLabelRecoveryService::new(runtime.db_clone())
        .with_provider_registry(runtime.fulfillment_provider_registry())
        .retry(tenant.id, operation_id)
        .await
        .map_err(|error| {
            map_reconciliation_orchestration_error(
                AdminReconciliationErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(operation_id),
                    "retry_create_label",
                ),
                error,
            )
        })?;
    Ok(Json(fulfillment))
}

fn map_reconciliation_fulfillment_error(
    context: AdminReconciliationErrorContext,
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
    let context = AdminReconciliationDiagnosticContext::from(&context);
    let error = AdminReconciliationDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_RECONCILIATION_FULFILLMENT_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        provider_operation_id = %context.provider_operation_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_RECONCILIATION_BOUNDARY,
        "commerce admin fulfillment reconciliation owner operation failed"
    );
    HttpError::new(status, code, message)
}

fn map_reconciliation_orchestration_error(
    context: AdminReconciliationErrorContext,
    error: FulfillmentOrchestrationError,
) -> HttpError {
    match error {
        FulfillmentOrchestrationError::Fulfillment(error) => {
            map_reconciliation_fulfillment_error(context, error)
        }
        error => {
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
            let context = AdminReconciliationDiagnosticContext::from(&context);
            let error = AdminReconciliationDiagnosticError;
            tracing::error!(
                error = ?error,
                owner = ADMIN_RECONCILIATION_ORCHESTRATION_OWNER,
                tenant_id = %context.tenant_id,
                actor_id = %context.actor_id,
                provider_operation_id = %context.provider_operation_id,
                operation = %context.operation,
                error_kind,
                public_code = code,
                status = %status,
                boundary = ADMIN_RECONCILIATION_BOUNDARY,
                "commerce admin fulfillment reconciliation orchestration failed"
            );
            HttpError::new(status, code, message)
        }
    }
}

fn map_provider_result_encoding_error(
    context: AdminReconciliationErrorContext,
    _error: serde_json::Error,
) -> HttpError {
    let status = axum::http::StatusCode::INTERNAL_SERVER_ERROR;
    let code = "commerce_admin_fulfillment_reconciliation_encoding_failed";
    let context = AdminReconciliationDiagnosticContext::from(&context);
    let error = AdminReconciliationDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_RECONCILIATION_ORCHESTRATION_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        provider_operation_id = %context.provider_operation_id,
        operation = %context.operation,
        error_kind = "encoding",
        public_code = code,
        status = %status,
        boundary = ADMIN_RECONCILIATION_BOUNDARY,
        "commerce admin fulfillment reconciliation provider result encoding failed"
    );
    HttpError::new(
        status,
        code,
        "Fulfillment reconciliation result could not be processed safely",
    )
}

fn require_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    super::common::ensure_permissions(
        auth,
        &[Permission::FULFILLMENTS_MANAGE],
        "Permission denied: fulfillments:manage required",
    )
}

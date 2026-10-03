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
use utoipa::{IntoParams, ToSchema};
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

#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams)]
pub struct ListReconciliationParams {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct QuarantineStaleInput {
    pub stale_after_seconds: u64,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ResolveUnknownFailedInput {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ResolveUnknownSucceededInput {
    pub provider_result: FulfillmentProviderOperationResult,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QuarantineStaleResponse {
    pub quarantined: u64,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminReconciliationProviderOperationResponse {
    pub id: Uuid,
    pub fulfillment_id: Uuid,
    pub operation: String,
    pub provider_id: String,
    pub status: String,
    pub provider_reference: Option<String>,
    pub provider_result_present: bool,
    pub error_present: bool,
    pub created_at: String,
    pub updated_at: String,
    pub provider_completed_at: Option<String>,
    pub committed_at: Option<String>,
}

impl From<provider_operation::Model> for AdminReconciliationProviderOperationResponse {
    fn from(operation: provider_operation::Model) -> Self {
        Self {
            id: operation.id,
            fulfillment_id: operation.fulfillment_id,
            operation: operation.operation,
            provider_id: operation.provider_id,
            status: operation.status,
            provider_reference: operation.provider_reference,
            provider_result_present: operation.provider_result.is_some(),
            error_present: operation.error_message.is_some(),
            created_at: operation.created_at.to_rfc3339(),
            updated_at: operation.updated_at.to_rfc3339(),
            provider_completed_at: operation
                .provider_completed_at
                .map(|value| value.to_rfc3339()),
            committed_at: operation.committed_at.map(|value| value.to_rfc3339()),
        }
    }
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

#[utoipa::path(
    get,
    path = "/admin/fulfillment-provider-operations/reconciliation",
    tag = "admin",
    params(ListReconciliationParams),
    responses(
        (status = 200, description = "Provider operations requiring reconciliation", body = Vec<AdminReconciliationProviderOperationResponse>),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 503, description = "Recovery storage is unavailable")
    )
)]
pub async fn list_reconciliation_required(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(params): Query<ListReconciliationParams>,
) -> HttpResult<Json<Vec<AdminReconciliationProviderOperationResponse>>> {
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
    Ok(Json(operations.into_iter().map(Into::into).collect()))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillment-provider-operations/quarantine-stale",
    tag = "admin",
    request_body = QuarantineStaleInput,
    responses(
        (status = 200, description = "Stale provider operations quarantined", body = QuarantineStaleResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 503, description = "Recovery storage is unavailable")
    )
)]
pub async fn quarantine_stale_executing(
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

#[utoipa::path(
    post,
    path = "/admin/fulfillment-provider-operations/{id}/resolve-failed",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Provider operation ID")),
    request_body = ResolveUnknownFailedInput,
    responses(
        (status = 200, description = "Provider operation resolved as failed", body = AdminReconciliationProviderOperationResponse),
        (status = 400, description = "Invalid request or operation state"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 503, description = "Recovery storage is unavailable")
    )
)]
pub async fn resolve_unknown_as_failed(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownFailedInput>,
) -> HttpResult<Json<AdminReconciliationProviderOperationResponse>> {
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
    Ok(Json(operation.into()))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillment-provider-operations/{id}/resolve-succeeded",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Provider operation ID")),
    request_body = ResolveUnknownSucceededInput,
    responses(
        (status = 200, description = "Provider operation resolved as succeeded", body = AdminReconciliationProviderOperationResponse),
        (status = 400, description = "Invalid request or provider result"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 500, description = "Provider result could not be encoded safely"),
        (status = 503, description = "Recovery storage is unavailable")
    )
)]
pub async fn resolve_unknown_as_succeeded(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(operation_id): Path<Uuid>,
    Json(input): Json<ResolveUnknownSucceededInput>,
) -> HttpResult<Json<AdminReconciliationProviderOperationResponse>> {
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
    Ok(Json(operation.into()))
}

#[utoipa::path(
    post,
    path = "/admin/fulfillment-provider-operations/{id}/retry-local",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Provider operation ID")),
    responses(
        (status = 200, description = "Local fulfillment persistence retried", body = crate::dto::FulfillmentResponse),
        (status = 400, description = "Invalid reconciliation request"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 404, description = "Fulfillment or provider operation not found"),
        (status = 409, description = "Fulfillment reconciliation is required or conflicts with current state"),
        (status = 503, description = "Reconciliation storage is unavailable")
    )
)]
pub async fn retry_local_persistence(
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

#[utoipa::path(
    post,
    path = "/admin/fulfillment-provider-operations/{id}/retry-create-label",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Provider operation ID")),
    responses(
        (status = 200, description = "Provider label creation retried", body = crate::dto::FulfillmentResponse),
        (status = 400, description = "Invalid reconciliation request"),
        (status = 401, description = "Authentication is required"),
        (status = 403, description = "fulfillments:manage is required"),
        (status = 404, description = "Fulfillment or provider operation not found"),
        (status = 409, description = "Fulfillment reconciliation is required or conflicts with current state"),
        (status = 503, description = "Reconciliation storage is unavailable")
    )
)]
pub async fn retry_create_label(
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
        FulfillmentError::InvalidTransition { .. }
        | FulfillmentError::ShippingOptionTranslationRevisionConflict(_)
        | FulfillmentError::ProviderResultInvalid(_) => (
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_projection_excludes_persistence_payloads() {
        let operation = provider_operation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            operation: "create_label".to_string(),
            provider_id: "carrier".to_string(),
            idempotency_key: "opaque-key".to_string(),
            status: "reconciliation_required".to_string(),
            request_payload: serde_json::json!({
                "metadata": {
                    "sensitive": "request-payload"
                }
            }),
            provider_reference: Some("external-reference".to_string()),
            provider_result: Some(serde_json::json!({
                "tracking_number": "tracking-value",
                "metadata": {
                    "sensitive": "provider-result"
                }
            })),
            error_message: Some(
                "database detail that must not cross the HTTP boundary".to_string(),
            ),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            provider_completed_at: Some(Utc::now().into()),
            committed_at: None,
        };

        let response: AdminReconciliationProviderOperationResponse = operation.into();
        let value = serde_json::to_value(response).expect("response projection must serialize");

        for field in [
            "request_payload",
            "provider_result",
            "error_message",
            "tenant_id",
            "idempotency_key",
        ] {
            assert!(
                value.get(field).is_none(),
                "sensitive persistence field {field} must not be exposed"
            );
        }
        assert_eq!(value["provider_reference"], "external-reference");
        assert_eq!(value["provider_result_present"], true);
        assert_eq!(value["error_present"], true);
    }
}

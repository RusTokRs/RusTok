use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    CommerceHttpRuntime,
    common::{PaginatedResponse, PaginationMeta, PaginationParams, ensure_permissions},
};
use crate::dto::OrderReturnResponse;
use crate::services::{ListReturnCompletionOperationsInput, ReturnCompletionOperationResponse};
use crate::{PostOrderOrchestrationError, ReturnCompletionOrchestrationService};

const RETURN_COMPLETION_OPERATOR_OWNER: &str = "rustok_commerce.return_completion_operation";
const RETURN_COMPLETION_OPERATOR_BOUNDARY: &str = "commerce_admin_return_completion_operation_http";

type ReturnCompletionOperatorHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
struct ReturnCompletionOperatorErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    operation_id: Option<Uuid>,
    operation: &'static str,
}

impl ReturnCompletionOperatorErrorContext {
    fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            operation_id,
            operation,
        }
    }
}

struct ReturnCompletionOperatorDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    operation_id: &'static str,
    operation: &'static str,
}

impl From<&ReturnCompletionOperatorErrorContext> for ReturnCompletionOperatorDiagnosticContext {
    fn from(context: &ReturnCompletionOperatorErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            operation_id: optional_uuid_shape(context.operation_id),
            operation: context.operation,
        }
    }
}

struct ReturnCompletionOperatorDiagnosticError;

impl std::fmt::Debug for ReturnCompletionOperatorDiagnosticError {
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

#[derive(Clone, Debug, Default, Deserialize, ToSchema, IntoParams)]
pub struct AdminListReturnCompletionOperationsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub status: Option<String>,
}

pub fn axum_router() -> axum::Router<CommerceHttpRuntime> {
    axum::Router::new()
        .route("/", axum::routing::get(list_return_completion_operations))
        .route(
            "/{id}",
            axum::routing::get(show_return_completion_operation),
        )
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
    let (items, total) =
        ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
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
            .map_err(|error| {
                map_operator_error(
                    ReturnCompletionOperatorErrorContext::new(
                        tenant.id,
                        auth.user_id,
                        None,
                        "list_return_completion_operations",
                    ),
                    error,
                )
            })?;

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
    let operation =
        ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
            .with_payment_provider_registry(runtime.payment_provider_registry())
            .get_operation(tenant.id, id)
            .await
            .map_err(|error| {
                map_operator_error(
                    ReturnCompletionOperatorErrorContext::new(
                        tenant.id,
                        auth.user_id,
                        Some(id),
                        "show_return_completion_operation",
                    ),
                    error,
                )
            })?;
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
        (status = 409, description = "Operation is leased or requires reconciliation"),
        (status = 503, description = "Recovery storage or provider is unavailable")
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
    let order_return =
        ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())
            .with_payment_provider_registry(runtime.payment_provider_registry())
            .retry_operation(tenant.id, auth.user_id, id)
            .await
            .map_err(|error| {
                map_operator_error(
                    ReturnCompletionOperatorErrorContext::new(
                        tenant.id,
                        auth.user_id,
                        Some(id),
                        "retry_return_completion_operation",
                    ),
                    error,
                )
            })?;
    Ok(Json(order_return))
}

fn return_completion_operator_policy(
    error: &PostOrderOrchestrationError,
) -> Option<ReturnCompletionOperatorHttpPolicy> {
    match error {
        PostOrderOrchestrationError::Validation(message) if message.contains("was not found") => {
            Some((
                StatusCode::NOT_FOUND,
                "return_completion_operation_not_found",
                "Return completion operation not found",
                "not_found",
            ))
        }
        PostOrderOrchestrationError::Validation(message)
            if message.contains("currently leased")
                || message.contains("requires reconciliation")
                || message.contains("terminally failed")
                || message.contains("already completed")
                || message.contains("different completion command")
                || message.contains("already bound to another command")
                || message.contains("command hash does not match") =>
        {
            Some((
                StatusCode::CONFLICT,
                "return_completion_operation_conflict",
                "Return completion operation conflicts with the current state",
                "conflict",
            ))
        }
        PostOrderOrchestrationError::Order(
            rustok_order::error::OrderError::Database(_) | rustok_order::error::OrderError::Core(_),
        ) => Some((
            StatusCode::SERVICE_UNAVAILABLE,
            "return_completion_storage_unavailable",
            "Return completion recovery storage is unavailable",
            "storage_unavailable",
        )),
        _ => None,
    }
}

fn map_operator_error(
    context: ReturnCompletionOperatorErrorContext,
    error: PostOrderOrchestrationError,
) -> HttpError {
    let Some((status, code, message, error_kind)) = return_completion_operator_policy(&error)
    else {
        return super::admin::map_post_order_orchestration_error(error);
    };
    let context = ReturnCompletionOperatorDiagnosticContext::from(&context);
    let error = ReturnCompletionOperatorDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = RETURN_COMPLETION_OPERATOR_OWNER,
        source_owner = "rustok_commerce.post_order_orchestration",
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        operation_id = ?context.operation_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = RETURN_COMPLETION_OPERATOR_BOUNDARY,
        "commerce admin return completion operation failed"
    );
    HttpError::new(status, code, message)
}

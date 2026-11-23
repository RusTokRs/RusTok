use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use crate::{WorkflowCronReconciliationItem, WorkflowError, WorkflowExecutionResponse, WorkflowService};

fn map_workflow_execution_error(
    error: WorkflowError,
    operation: &'static str,
    tenant_id: Uuid,
    workflow_id: Option<Uuid>,
    execution_id: Option<Uuid>,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        WorkflowError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_not_found",
            "Workflow was not found",
            "workflow_not_found",
        ),
        WorkflowError::VersionNotFound(_, _) | WorkflowError::TemplateNotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_not_found",
            "Workflow entity was not found",
            "not_found",
        ),
        WorkflowError::StepNotFound(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "unexpected_step_not_found",
        ),
        WorkflowError::ExecutionNotFound(_) => (
            StatusCode::NOT_FOUND,
            "workflow_execution_not_found",
            "Workflow execution was not found",
            "execution_not_found",
        ),
        WorkflowError::ExecutionOutcomeUncertain(_) => (
            StatusCode::CONFLICT,
            "workflow_execution_reconciliation_required",
            "Workflow execution outcome requires operator reconciliation",
            "execution_outcome_ambiguous",
        ),
        WorkflowError::NotActive(_) => (
            StatusCode::CONFLICT,
            "workflow_state_conflict",
            "Workflow operation conflicts with the current state",
            "state_conflict",
        ),
        WorkflowError::UnknownStepType(_)
        | WorkflowError::InvalidTriggerConfig(_)
        | WorkflowError::InvalidStepConfig(_) => (
            StatusCode::BAD_REQUEST,
            "workflow_execution_invalid",
            "Workflow execution request is invalid",
            "validation",
        ),
        WorkflowError::WebhookSignatureMissing
        | WorkflowError::WebhookSignatureInvalid
        | WorkflowError::WebhookSecretNotConfigured => (
            StatusCode::UNAUTHORIZED,
            "workflow_webhook_unauthorized",
            "Webhook signature verification failed",
            "webhook_unauthorized",
        ),
        WorkflowError::StepFailed(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "step_failed",
        ),
        WorkflowError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "workflow_storage_unavailable",
            "Workflow storage is temporarily unavailable",
            "database",
        ),
        WorkflowError::Serialization(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "workflow_execution_failed",
            "Workflow execution could not be loaded safely",
            "serialization",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_workflow.workflow_service",
        operation,
        tenant_id = %tenant_id,
        workflow_id = ?workflow_id,
        execution_id = ?execution_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "workflow_execution_http",
        "workflow execution operation failed"
    );
    HttpError::new(status, code, message)
}

pub async fn list_executions(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(workflow_id): Path<Uuid>,
) -> HttpResult<Json<Vec<WorkflowExecutionResponse>>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_LIST],
        "Permission denied: workflow_executions:list required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let executions = service
        .list_executions(tenant.id, workflow_id)
        .await
        .map_err(|error| {
            map_workflow_execution_error(
                error,
                "list_executions",
                tenant.id,
                Some(workflow_id),
                None,
            )
        })?;
    Ok(Json(executions))
}

pub async fn get_execution(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(execution_id): Path<Uuid>,
) -> HttpResult<Json<WorkflowExecutionResponse>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_READ],
        "Permission denied: workflow_executions:read required",
    )?;

    let service = WorkflowService::new(runtime.db_clone());
    let execution = service
        .get_execution(tenant.id, execution_id)
        .await
        .map_err(|error| {
            map_workflow_execution_error(
                error,
                "get_execution",
                tenant.id,
                None,
                Some(execution_id),
            )
        })?;
    Ok(Json(execution))
}

/// Review-only projection. A stale claim cannot be retried from this endpoint.
pub async fn list_cron_reconciliation(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
) -> HttpResult<Json<Vec<WorkflowCronReconciliationItem>>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOW_EXECUTIONS_LIST],
        "Permission denied: workflow_executions:list required",
    )?;

    let items = crate::services::reconciliation_required_for_tenant(
        &runtime.db_clone(), tenant.id, 100,
    )
    .await
    .map_err(|error| map_workflow_execution_error(
        error, "list_cron_reconciliation", tenant.id, None, None,
    ))?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[derive(serde::Deserialize)]
pub struct CronReconciliationRequest {
    pub decision: crate::services::CronReconciliationDecision,
    pub evidence: String,
    /// Operator attests that the previous worker process is fully fenced
    /// before requesting a retry that cannot be proven by elapsed time alone.
    #[serde(default)]
    pub prior_worker_fenced: bool,
}

/// High-privilege recovery: records actor, evidence and attempt fence in one
/// owner transaction. The endpoint cannot blindly replay a running execution.
pub async fn reconcile_cron_firing(
    State(runtime): State<crate::controllers::WorkflowHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(firing_id): Path<Uuid>,
    Json(request): Json<CronReconciliationRequest>,
) -> HttpResult<Json<WorkflowCronReconciliationItem>> {
    ensure_execution_permission(
        &auth,
        &[Permission::WORKFLOWS_MANAGE],
        "Permission denied: workflows:manage required",
    )?;
    let actor_id = auth.human_user_id().ok_or_else(|| HttpError::forbidden(
        "workflow_reconciliation_actor_required",
        "An authenticated human operator is required".to_string(),
    ))?;
    let outcome = crate::services::reconcile_firing(
        &runtime.db_clone(),
        tenant.id,
        firing_id,
        actor_id,
        request.decision,
        &request.evidence,
        request.prior_worker_fenced,
    ).await.map_err(|error| map_workflow_execution_error(
        error, "reconcile_cron_firing", tenant.id, None, Some(firing_id),
    ))?;
    Ok(Json(outcome.into()))
}

fn ensure_execution_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "workflow_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}

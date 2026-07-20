use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::entities::workflow::{
    ExecutionStatus, OnError, StepExecution, StepType, WorkflowStatus, WorkflowStep,
};
use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};

use crate::features::workflow::model::{
    CreateStepInput, CreateWorkflowInput, WorkflowTemplateDto, WorkflowVersionSummaryDto,
};

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
async fn workflow_server_context(
    required: &[rustok_api::Permission],
    permission_error: &'static str,
) -> Result<
    (
        sea_orm::DatabaseConnection,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use rustok_api::{
        AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
    };

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if !has_any_effective_permission(&auth.permissions, required) {
        return Err(ServerFnError::new(permission_error));
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    Ok((runtime_ctx.db_clone(), auth, tenant))
}

#[cfg(feature = "ssr")]
fn parse_uuid_arg(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value).map_err(|err| server_error(format!("invalid {field_name}: {err}")))
}

#[cfg(feature = "ssr")]
fn parse_step_type_arg(
    step_type: &str,
) -> Result<rustok_workflow::entities::StepType, ServerFnError> {
    match step_type {
        "ACTION" => Ok(rustok_workflow::entities::StepType::Action),
        "CONDITION" => Ok(rustok_workflow::entities::StepType::Condition),
        "DELAY" => Ok(rustok_workflow::entities::StepType::Delay),
        "ALLOY_SCRIPT" => Ok(rustok_workflow::entities::StepType::AlloyScript),
        "EMIT_EVENT" => Ok(rustok_workflow::entities::StepType::EmitEvent),
        "HTTP" => Ok(rustok_workflow::entities::StepType::Http),
        "NOTIFY" => Ok(rustok_workflow::entities::StepType::Notify),
        "TRANSFORM" => Ok(rustok_workflow::entities::StepType::Transform),
        other => Err(server_error(format!("unsupported step type: {other}"))),
    }
}

#[cfg(feature = "ssr")]
fn parse_on_error_arg(on_error: &str) -> Result<rustok_workflow::entities::OnError, ServerFnError> {
    match on_error {
        "STOP" => Ok(rustok_workflow::entities::OnError::Stop),
        "SKIP" => Ok(rustok_workflow::entities::OnError::Skip),
        "RETRY" => Ok(rustok_workflow::entities::OnError::Retry),
        other => Err(server_error(format!("unsupported on_error value: {other}"))),
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_status(status: rustok_workflow::entities::WorkflowStatus) -> WorkflowStatus {
    match status {
        rustok_workflow::entities::WorkflowStatus::Draft => WorkflowStatus::Draft,
        rustok_workflow::entities::WorkflowStatus::Active => WorkflowStatus::Active,
        rustok_workflow::entities::WorkflowStatus::Paused => WorkflowStatus::Paused,
        rustok_workflow::entities::WorkflowStatus::Archived => WorkflowStatus::Archived,
    }
}

#[cfg(feature = "ssr")]
fn map_step_type(step_type: rustok_workflow::entities::StepType) -> StepType {
    match step_type {
        rustok_workflow::entities::StepType::Action => StepType::Action,
        rustok_workflow::entities::StepType::Condition => StepType::Condition,
        rustok_workflow::entities::StepType::Delay => StepType::Delay,
        rustok_workflow::entities::StepType::AlloyScript => StepType::AlloyScript,
        rustok_workflow::entities::StepType::EmitEvent => StepType::EmitEvent,
        rustok_workflow::entities::StepType::Http => StepType::Http,
        rustok_workflow::entities::StepType::Notify => StepType::Notify,
        rustok_workflow::entities::StepType::Transform => StepType::Transform,
    }
}

#[cfg(feature = "ssr")]
fn map_on_error(on_error: rustok_workflow::entities::OnError) -> OnError {
    match on_error {
        rustok_workflow::entities::OnError::Stop => OnError::Stop,
        rustok_workflow::entities::OnError::Skip => OnError::Skip,
        rustok_workflow::entities::OnError::Retry => OnError::Retry,
    }
}

#[cfg(feature = "ssr")]
fn map_execution_status(status: rustok_workflow::entities::ExecutionStatus) -> ExecutionStatus {
    match status {
        rustok_workflow::entities::ExecutionStatus::Running => ExecutionStatus::Running,
        rustok_workflow::entities::ExecutionStatus::Completed => ExecutionStatus::Completed,
        rustok_workflow::entities::ExecutionStatus::Failed => ExecutionStatus::Failed,
        rustok_workflow::entities::ExecutionStatus::TimedOut => ExecutionStatus::TimedOut,
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_summary(value: rustok_workflow::WorkflowSummary) -> WorkflowSummary {
    WorkflowSummary {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        name: value.name,
        status: map_workflow_status(value.status),
        failure_count: value.failure_count,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_step(value: rustok_workflow::WorkflowStepResponse) -> WorkflowStep {
    WorkflowStep {
        id: value.id.to_string(),
        workflow_id: value.workflow_id.to_string(),
        position: value.position,
        step_type: map_step_type(value.step_type),
        config: value.config,
        on_error: map_on_error(value.on_error),
        timeout_ms: value.timeout_ms,
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_detail(value: rustok_workflow::WorkflowResponse) -> WorkflowDetail {
    WorkflowDetail {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        name: value.name,
        description: value.description,
        status: map_workflow_status(value.status),
        trigger_config: value.trigger_config,
        created_by: value.created_by.map(|id| id.to_string()),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
        failure_count: value.failure_count,
        auto_disabled_at: value.auto_disabled_at.map(|value| value.to_rfc3339()),
        steps: value.steps.into_iter().map(map_workflow_step).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_execution(value: rustok_workflow::WorkflowExecutionResponse) -> WorkflowExecution {
    WorkflowExecution {
        id: value.id.to_string(),
        workflow_id: value.workflow_id.to_string(),
        status: map_execution_status(value.status),
        error: value.error,
        started_at: value.started_at.to_rfc3339(),
        completed_at: value.completed_at.map(|value| value.to_rfc3339()),
        step_executions: value
            .step_executions
            .into_iter()
            .map(|step| StepExecution {
                id: step.id.to_string(),
                step_id: step.step_id.to_string(),
                status: step.status.to_string().to_ascii_uppercase(),
                error: step.error,
                started_at: step.started_at.to_rfc3339(),
                completed_at: step.completed_at.map(|value| value.to_rfc3339()),
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-workflows")]
pub(super) async fn list_workflows_native() -> Result<Vec<WorkflowSummary>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_LIST]) {
            return Err(ServerFnError::new("workflows:list required"));
        }

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .list(tenant.id)
            .await
            .map(|items| items.into_iter().map(map_workflow_summary).collect())
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-workflows requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/workflow")]
pub(super) async fn workflow_native(id: String) -> Result<Option<WorkflowDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_READ]) {
            return Err(ServerFnError::new("workflows:read required"));
        }

        let workflow_id = uuid::Uuid::parse_str(&id)
            .map_err(|err| server_error(format!("invalid workflow id: {err}")))?;
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        match rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .get(tenant.id, workflow_id)
            .await
        {
            Ok(workflow) => Ok(Some(map_workflow_detail(workflow))),
            Err(rustok_workflow::error::WorkflowError::NotFound(_)) => Ok(None),
            Err(err) => Err(server_error(err.to_string())),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/workflow requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/workflow-executions")]
pub(super) async fn workflow_executions_native(
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOW_EXECUTIONS_LIST])
        {
            return Err(ServerFnError::new("workflow_executions:list required"));
        }

        let workflow_id = uuid::Uuid::parse_str(&workflow_id)
            .map_err(|err| server_error(format!("invalid workflow id: {err}")))?;
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .list_executions(tenant.id, workflow_id)
            .await
            .map(|items| items.into_iter().map(map_workflow_execution).collect())
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = workflow_id;
        Err(ServerFnError::new(
            "admin/workflow-executions requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/create-workflow")]
pub(super) async fn create_workflow_native(
    input: CreateWorkflowInput,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_CREATE], "workflows:create required")
                .await?;

        rustok_workflow::WorkflowService::new(db)
            .create(
                tenant.id,
                Some(auth.user_id),
                rustok_workflow::CreateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    trigger_config: input.trigger_config,
                    webhook_slug: None,
                },
            )
            .await
            .map(|id| id.to_string())
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "admin/create-workflow requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/delete-workflow")]
pub(super) async fn delete_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, _auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_DELETE], "workflows:delete required")
                .await?;
        let workflow_id = parse_uuid_arg(&id, "workflow id")?;

        rustok_workflow::WorkflowService::new(db)
            .delete(tenant.id, workflow_id)
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/delete-workflow requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/activate-workflow")]
pub(super) async fn activate_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_UPDATE], "workflows:update required")
                .await?;
        let workflow_id = parse_uuid_arg(&id, "workflow id")?;

        rustok_workflow::WorkflowService::new(db)
            .update(
                tenant.id,
                workflow_id,
                Some(auth.user_id),
                rustok_workflow::UpdateWorkflowInput {
                    status: Some(rustok_workflow::entities::WorkflowStatus::Active),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/activate-workflow requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/pause-workflow")]
pub(super) async fn pause_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_UPDATE], "workflows:update required")
                .await?;
        let workflow_id = parse_uuid_arg(&id, "workflow id")?;

        rustok_workflow::WorkflowService::new(db)
            .update(
                tenant.id,
                workflow_id,
                Some(auth.user_id),
                rustok_workflow::UpdateWorkflowInput {
                    status: Some(rustok_workflow::entities::WorkflowStatus::Paused),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/pause-workflow requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/add-workflow-step")]
pub(super) async fn add_step_native(
    workflow_id: String,
    input: CreateStepInput,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, _auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_UPDATE], "workflows:update required")
                .await?;
        let workflow_id = parse_uuid_arg(&workflow_id, "workflow id")?;
        let step_type = parse_step_type_arg(&input.step_type)?;
        let on_error = parse_on_error_arg(&input.on_error)?;

        rustok_workflow::WorkflowService::new(db)
            .add_step(
                tenant.id,
                workflow_id,
                rustok_workflow::CreateWorkflowStepInput {
                    position: input.position,
                    step_type,
                    config: input.config,
                    on_error,
                    timeout_ms: input.timeout_ms,
                },
            )
            .await
            .map(|id| id.to_string())
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (workflow_id, input);
        Err(ServerFnError::new(
            "admin/add-workflow-step requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/delete-workflow-step")]
pub(super) async fn delete_step_native(
    workflow_id: String,
    step_id: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, _auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_UPDATE], "workflows:update required")
                .await?;
        let workflow_id = parse_uuid_arg(&workflow_id, "workflow id")?;
        let step_id = parse_uuid_arg(&step_id, "step id")?;

        rustok_workflow::WorkflowService::new(db)
            .delete_step(tenant.id, workflow_id, step_id)
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (workflow_id, step_id);
        Err(ServerFnError::new(
            "admin/delete-workflow-step requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/create-workflow-from-template")]
pub(super) async fn create_from_template_native(
    template_id: String,
    name: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_CREATE], "workflows:create required")
                .await?;

        rustok_workflow::WorkflowService::new(db)
            .create_from_template(tenant.id, Some(auth.user_id), &template_id, name)
            .await
            .map(|id| id.to_string())
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (template_id, name);
        Err(ServerFnError::new(
            "admin/create-workflow-from-template requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/workflow-templates")]
pub(super) async fn workflow_templates_native() -> Result<Vec<WorkflowTemplateDto>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(rustok_workflow::BUILTIN_TEMPLATES
            .iter()
            .map(|template| WorkflowTemplateDto {
                id: template.id.to_string(),
                name: template.name.to_string(),
                description: template.description.to_string(),
                category: template.category.to_string(),
                trigger_config: template.trigger_config.clone(),
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/workflow-templates requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/workflow-versions")]
pub(super) async fn workflow_versions_native(
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_READ]) {
            return Err(ServerFnError::new("workflows:read required"));
        }

        let workflow_id = uuid::Uuid::parse_str(&workflow_id)
            .map_err(|err| server_error(format!("invalid workflow id: {err}")))?;
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .list_versions(tenant.id, workflow_id)
            .await
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| WorkflowVersionSummaryDto {
                        id: item.id.to_string(),
                        version: item.version,
                        created_by: item.created_by.map(|id| id.to_string()),
                        created_at: item.created_at.to_rfc3339(),
                    })
                    .collect()
            })
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = workflow_id;
        Err(ServerFnError::new(
            "admin/workflow-versions requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/restore-workflow-version")]
pub(super) async fn restore_version_native(
    workflow_id: String,
    version: i32,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;

        let (db, auth, tenant) =
            workflow_server_context(&[Permission::WORKFLOWS_UPDATE], "workflows:update required")
                .await?;
        let workflow_id = parse_uuid_arg(&workflow_id, "workflow id")?;

        rustok_workflow::WorkflowService::new(db)
            .restore_version(tenant.id, workflow_id, version, Some(auth.user_id))
            .await
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (workflow_id, version);
        Err(ServerFnError::new(
            "admin/restore-workflow-version requires the `ssr` feature",
        ))
    }
}

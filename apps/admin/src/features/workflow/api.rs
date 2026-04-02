use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "ssr")]
use crate::entities::workflow::{
    ExecutionStatus, OnError, StepExecution, StepType, WorkflowStatus, WorkflowStep,
};
use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};
use crate::shared::api::{request, ApiError};

pub const WORKFLOWS_QUERY: &str =
    "query Workflows { workflows { id tenantId name status failureCount createdAt updatedAt } }";

pub const WORKFLOW_QUERY: &str =
    "query Workflow($id: UUID!) { workflow(id: $id) { id tenantId name description status triggerConfig createdBy createdAt updatedAt failureCount autoDisabledAt steps { id workflowId position stepType config onError timeoutMs } } }";

pub const WORKFLOW_EXECUTIONS_QUERY: &str =
    "query WorkflowExecutions($workflowId: UUID!) { workflowExecutions(workflowId: $workflowId) { id workflowId status error startedAt completedAt stepExecutions { id stepId status error startedAt completedAt } } }";

pub const CREATE_WORKFLOW_MUTATION: &str =
    "mutation CreateWorkflow($input: GqlCreateWorkflowInput!) { createWorkflow(input: $input) }";

pub const UPDATE_WORKFLOW_MUTATION: &str =
    "mutation UpdateWorkflow($id: UUID!, $input: GqlUpdateWorkflowInput!) { updateWorkflow(id: $id, input: $input) }";

pub const DELETE_WORKFLOW_MUTATION: &str =
    "mutation DeleteWorkflow($id: UUID!) { deleteWorkflow(id: $id) }";

pub const ACTIVATE_WORKFLOW_MUTATION: &str =
    "mutation ActivateWorkflow($id: UUID!) { activateWorkflow(id: $id) }";

pub const PAUSE_WORKFLOW_MUTATION: &str =
    "mutation PauseWorkflow($id: UUID!) { pauseWorkflow(id: $id) }";

pub const ADD_STEP_MUTATION: &str =
    "mutation AddWorkflowStep($workflowId: UUID!, $input: GqlCreateStepInput!) { addWorkflowStep(workflowId: $workflowId, input: $input) }";

pub const DELETE_STEP_MUTATION: &str =
    "mutation DeleteWorkflowStep($workflowId: UUID!, $stepId: UUID!) { deleteWorkflowStep(workflowId: $workflowId, stepId: $stepId) }";

#[derive(Clone, Debug, Serialize)]
pub struct IdVars {
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkflowExecutionsVars {
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateWorkflowInput {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateWorkflowVars {
    pub input: CreateWorkflowInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateWorkflowInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateWorkflowVars {
    pub id: String,
    pub input: UpdateWorkflowInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStepInput {
    pub position: i32,
    #[serde(rename = "stepType")]
    pub step_type: String,
    pub config: Value,
    #[serde(rename = "onError")]
    pub on_error: String,
    #[serde(rename = "timeoutMs")]
    pub timeout_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddStepVars {
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    pub input: CreateStepInput,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteStepVars {
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
    #[serde(rename = "stepId")]
    pub step_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WorkflowsResponse {
    pub workflows: Vec<WorkflowSummary>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WorkflowResponse {
    pub workflow: Option<WorkflowDetail>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WorkflowExecutionsResponse {
    #[serde(rename = "workflowExecutions")]
    pub workflow_executions: Vec<WorkflowExecution>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateWorkflowResponse {
    #[serde(rename = "createWorkflow")]
    pub create_workflow: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AddStepResponse {
    #[serde(rename = "addWorkflowStep")]
    pub add_workflow_step: String,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

fn combine_native_and_graphql_error(server_err: ServerFnError, graphql_err: ApiError) -> ApiError {
    ApiError::Graphql(format!(
        "native path failed: {}; graphql path failed: {}",
        server_err, graphql_err
    ))
}

#[cfg(feature = "ssr")]
async fn workflow_server_context(
    required: &[rustok_core::Permission],
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
    use loco_rs::app::AppContext;
    use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if !has_any_effective_permission(&auth.permissions, required) {
        return Err(ServerFnError::new(permission_error));
    }

    let app_ctx = expect_context::<AppContext>();
    Ok((app_ctx.db.clone(), auth, tenant))
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

async fn fetch_workflows_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, ApiError> {
    let resp: WorkflowsResponse =
        request(WORKFLOWS_QUERY, serde_json::json!({}), token, tenant_slug).await?;
    Ok(resp.workflows)
}

async fn fetch_workflow_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<WorkflowDetail>, ApiError> {
    let resp: WorkflowResponse = request(WORKFLOW_QUERY, IdVars { id }, token, tenant_slug).await?;
    Ok(resp.workflow)
}

async fn fetch_workflow_executions_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, ApiError> {
    let resp: WorkflowExecutionsResponse = request(
        WORKFLOW_EXECUTIONS_QUERY,
        WorkflowExecutionsVars { workflow_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(resp.workflow_executions)
}

async fn fetch_workflows_server() -> Result<Vec<WorkflowSummary>, ServerFnError> {
    list_workflows_native().await
}

async fn fetch_workflow_server(id: String) -> Result<Option<WorkflowDetail>, ServerFnError> {
    workflow_native(id).await
}

async fn fetch_workflow_executions_server(
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, ServerFnError> {
    workflow_executions_native(workflow_id).await
}

#[server(prefix = "/api/fn", endpoint = "admin/list-workflows")]
async fn list_workflows_native() -> Result<Vec<WorkflowSummary>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_LIST]) {
            return Err(ServerFnError::new("workflows:list required"));
        }

        let app_ctx = expect_context::<AppContext>();
        rustok_workflow::WorkflowService::new(app_ctx.db.clone())
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
async fn workflow_native(id: String) -> Result<Option<WorkflowDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

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
        let app_ctx = expect_context::<AppContext>();
        match rustok_workflow::WorkflowService::new(app_ctx.db.clone())
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
async fn workflow_executions_native(
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

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
        let app_ctx = expect_context::<AppContext>();
        rustok_workflow::WorkflowService::new(app_ctx.db.clone())
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

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, String> {
    match fetch_workflows_server().await {
        Ok(response) => Ok(response),
        Err(server_err) => {
            fetch_workflows_graphql(token, tenant_slug)
                .await
                .map_err(|graphql_err| {
                    format!(
                        "native path failed: {}; graphql path failed: {}",
                        server_err, graphql_err
                    )
                })
        }
    }
}

pub async fn fetch_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<WorkflowDetail>, String> {
    match fetch_workflow_server(id.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_workflow_graphql(token, tenant_slug, id)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

pub async fn fetch_workflow_executions(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, String> {
    match fetch_workflow_executions_server(workflow_id.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_workflow_executions_graphql(token, tenant_slug, workflow_id)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/create-workflow")]
async fn create_workflow_native(input: CreateWorkflowInput) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn delete_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn activate_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn pause_workflow_native(id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn add_step_native(
    workflow_id: String,
    input: CreateStepInput,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn delete_step_native(workflow_id: String, step_id: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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

pub async fn create_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: CreateWorkflowInput,
) -> Result<String, ApiError> {
    match create_workflow_native(input.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => {
            let resp: CreateWorkflowResponse = request(
                CREATE_WORKFLOW_MUTATION,
                CreateWorkflowVars { input },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(resp.create_workflow)
        }
    }
}

pub async fn delete_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match delete_workflow_native(id.clone()).await {
        Ok(()) => Ok(()),
        Err(server_err) => {
            let _: serde_json::Value =
                request(DELETE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug)
                    .await
                    .map_err(|graphql_err| {
                        combine_native_and_graphql_error(server_err, graphql_err)
                    })?;
            Ok(())
        }
    }
}

pub async fn activate_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match activate_workflow_native(id.clone()).await {
        Ok(()) => Ok(()),
        Err(server_err) => {
            let _: serde_json::Value = request(
                ACTIVATE_WORKFLOW_MUTATION,
                IdVars { id },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(())
        }
    }
}

pub async fn pause_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match pause_workflow_native(id.clone()).await {
        Ok(()) => Ok(()),
        Err(server_err) => {
            let _: serde_json::Value =
                request(PAUSE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug)
                    .await
                    .map_err(|graphql_err| {
                        combine_native_and_graphql_error(server_err, graphql_err)
                    })?;
            Ok(())
        }
    }
}

pub async fn add_step(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    input: CreateStepInput,
) -> Result<String, ApiError> {
    match add_step_native(workflow_id.clone(), input.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => {
            let resp: AddStepResponse = request(
                ADD_STEP_MUTATION,
                AddStepVars { workflow_id, input },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(resp.add_workflow_step)
        }
    }
}

pub async fn delete_step(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    step_id: String,
) -> Result<(), ApiError> {
    match delete_step_native(workflow_id.clone(), step_id.clone()).await {
        Ok(()) => Ok(()),
        Err(server_err) => {
            let _: serde_json::Value = request(
                DELETE_STEP_MUTATION,
                DeleteStepVars {
                    workflow_id,
                    step_id,
                },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(())
        }
    }
}

pub const WORKFLOW_TEMPLATES_QUERY: &str =
    "query WorkflowTemplates { workflowTemplates { id name description category triggerConfig } }";

pub const CREATE_FROM_TEMPLATE_MUTATION: &str = "mutation CreateWorkflowFromTemplate($templateId: String!, $name: String!) { createWorkflowFromTemplate(templateId: $templateId, name: $name) }";

pub const WORKFLOW_VERSIONS_QUERY: &str = "query WorkflowVersions($workflowId: UUID!) { workflowVersions(workflowId: $workflowId) { id version createdBy createdAt } }";

pub const RESTORE_VERSION_MUTATION: &str = "mutation RestoreWorkflowVersion($workflowId: UUID!, $version: Int!) { restoreWorkflowVersion(workflowId: $workflowId, version: $version) }";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTemplateDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(rename = "triggerConfig")]
    pub trigger_config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowVersionSummaryDto {
    pub id: String,
    pub version: i32,
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Serialize)]
struct TemplatesVars {}

#[derive(Serialize)]
struct CreateFromTemplateVars {
    #[serde(rename = "templateId")]
    template_id: String,
    name: String,
}

#[derive(Serialize)]
struct VersionsVars {
    #[serde(rename = "workflowId")]
    workflow_id: String,
}

#[derive(Serialize)]
struct RestoreVersionVars {
    #[serde(rename = "workflowId")]
    workflow_id: String,
    version: i32,
}

#[derive(Deserialize)]
struct TemplatesResponse {
    #[serde(rename = "workflowTemplates")]
    workflow_templates: Vec<WorkflowTemplateDto>,
}

#[derive(Deserialize)]
struct CreateFromTemplateResponse {
    #[serde(rename = "createWorkflowFromTemplate")]
    create_workflow_from_template: String,
}

#[derive(Deserialize)]
struct VersionsResponse {
    #[serde(rename = "workflowVersions")]
    workflow_versions: Vec<WorkflowVersionSummaryDto>,
}

async fn fetch_templates_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, ApiError> {
    let resp: TemplatesResponse = request(
        WORKFLOW_TEMPLATES_QUERY,
        TemplatesVars {},
        token,
        tenant_slug,
    )
    .await?;
    Ok(resp.workflow_templates)
}

async fn fetch_versions_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, ApiError> {
    let resp: VersionsResponse = request(
        WORKFLOW_VERSIONS_QUERY,
        VersionsVars { workflow_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(resp.workflow_versions)
}

async fn fetch_templates_server() -> Result<Vec<WorkflowTemplateDto>, ServerFnError> {
    workflow_templates_native().await
}

async fn fetch_versions_server(
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, ServerFnError> {
    workflow_versions_native(workflow_id).await
}

#[server(prefix = "/api/fn", endpoint = "admin/create-workflow-from-template")]
async fn create_from_template_native(
    template_id: String,
    name: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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
async fn workflow_templates_native() -> Result<Vec<WorkflowTemplateDto>, ServerFnError> {
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
async fn workflow_versions_native(
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_any_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

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
        let app_ctx = expect_context::<AppContext>();
        rustok_workflow::WorkflowService::new(app_ctx.db.clone())
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
async fn restore_version_native(workflow_id: String, version: i32) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_core::Permission;

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

pub async fn fetch_templates(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, String> {
    match fetch_templates_server().await {
        Ok(response) => Ok(response),
        Err(server_err) => {
            fetch_templates_graphql(token, tenant_slug)
                .await
                .map_err(|graphql_err| {
                    format!(
                        "native path failed: {}; graphql path failed: {}",
                        server_err, graphql_err
                    )
                })
        }
    }
}

pub async fn create_from_template(
    token: Option<String>,
    tenant_slug: Option<String>,
    template_id: String,
    name: String,
) -> Result<String, ApiError> {
    match create_from_template_native(template_id.clone(), name.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => {
            let resp: CreateFromTemplateResponse = request(
                CREATE_FROM_TEMPLATE_MUTATION,
                CreateFromTemplateVars { template_id, name },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(resp.create_workflow_from_template)
        }
    }
}

pub async fn fetch_versions(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, String> {
    match fetch_versions_server(workflow_id.clone()).await {
        Ok(response) => Ok(response),
        Err(server_err) => fetch_versions_graphql(token, tenant_slug, workflow_id)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
    }
}

pub async fn restore_version(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    version: i32,
) -> Result<(), ApiError> {
    match restore_version_native(workflow_id.clone(), version).await {
        Ok(()) => Ok(()),
        Err(server_err) => {
            let _: serde_json::Value = request(
                RESTORE_VERSION_MUTATION,
                RestoreVersionVars {
                    workflow_id,
                    version,
                },
                token,
                tenant_slug,
            )
            .await
            .map_err(|graphql_err| combine_native_and_graphql_error(server_err, graphql_err))?;
            Ok(())
        }
    }
}

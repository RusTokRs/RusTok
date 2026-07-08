use leptos::prelude::*;
use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "ssr")]
use crate::entities::workflow::{
    ExecutionStatus, OnError, StepExecution, StepType, WorkflowStatus, WorkflowStep,
};
use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};
use crate::shared::api::{map_server_fn_error, request, ApiError};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

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
    super::native_server_adapter::list_workflows_native().await
}

async fn fetch_workflow_server(id: String) -> Result<Option<WorkflowDetail>, ServerFnError> {
    super::native_server_adapter::workflow_native(id).await
}

async fn fetch_workflow_executions_server(
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, ServerFnError> {
    super::native_server_adapter::workflow_executions_native(workflow_id).await
}

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_workflows_server()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_workflows_graphql(token, tenant_slug)
            .await
            .map_err(|error| error.to_string()),
    }
}

pub async fn fetch_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<WorkflowDetail>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_workflow_server(id)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_workflow_graphql(token, tenant_slug, id)
            .await
            .map_err(|error| error.to_string()),
    }
}

pub async fn fetch_workflow_executions(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowExecution>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_workflow_executions_server(workflow_id)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => {
            fetch_workflow_executions_graphql(token, tenant_slug, workflow_id)
                .await
                .map_err(|error| error.to_string())
        }
    }
}

pub async fn create_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: CreateWorkflowInput,
) -> Result<String, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            super::native_server_adapter::create_workflow_native(input)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            let resp: CreateWorkflowResponse = request(
                CREATE_WORKFLOW_MUTATION,
                CreateWorkflowVars { input },
                token,
                tenant_slug,
            )
            .await?;
            Ok(resp.create_workflow)
        }
    }
}

pub async fn delete_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => super::native_server_adapter::delete_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let _: serde_json::Value =
                request(DELETE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug).await?;
            Ok(())
        }
    }
}

pub async fn activate_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => super::native_server_adapter::activate_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let _: serde_json::Value = request(
                ACTIVATE_WORKFLOW_MUTATION,
                IdVars { id },
                token,
                tenant_slug,
            )
            .await?;
            Ok(())
        }
    }
}

pub async fn pause_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => super::native_server_adapter::pause_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            let _: serde_json::Value =
                request(PAUSE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug).await?;
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
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            super::native_server_adapter::add_step_native(workflow_id, input)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            let resp: AddStepResponse = request(
                ADD_STEP_MUTATION,
                AddStepVars { workflow_id, input },
                token,
                tenant_slug,
            )
            .await?;
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
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            super::native_server_adapter::delete_step_native(workflow_id, step_id)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            let _: serde_json::Value = request(
                DELETE_STEP_MUTATION,
                DeleteStepVars {
                    workflow_id,
                    step_id,
                },
                token,
                tenant_slug,
            )
            .await?;
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
    super::native_server_adapter::workflow_templates_native().await
}

async fn fetch_versions_server(
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, ServerFnError> {
    super::native_server_adapter::workflow_versions_native(workflow_id).await
}

pub async fn fetch_templates(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_templates_server()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_templates_graphql(token, tenant_slug)
            .await
            .map_err(|error| error.to_string()),
    }
}

pub async fn create_from_template(
    token: Option<String>,
    tenant_slug: Option<String>,
    template_id: String,
    name: String,
) -> Result<String, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            super::native_server_adapter::create_from_template_native(template_id, name)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            let resp: CreateFromTemplateResponse = request(
                CREATE_FROM_TEMPLATE_MUTATION,
                CreateFromTemplateVars { template_id, name },
                token,
                tenant_slug,
            )
            .await?;
            Ok(resp.create_workflow_from_template)
        }
    }
}

pub async fn fetch_versions(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => fetch_versions_server(workflow_id)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_versions_graphql(token, tenant_slug, workflow_id)
            .await
            .map_err(|error| error.to_string()),
    }
}

pub async fn restore_version(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    version: i32,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            super::native_server_adapter::restore_version_native(workflow_id, version)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            let _: serde_json::Value = request(
                RESTORE_VERSION_MUTATION,
                RestoreVersionVars {
                    workflow_id,
                    version,
                },
                token,
                tenant_slug,
            )
            .await?;
            Ok(())
        }
    }
}

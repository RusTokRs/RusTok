use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};
use crate::shared::api::{request, ApiError};

// ── GraphQL documents (tenantId resolved from X-Tenant-Slug header) ────────────

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

// ── Variables ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct IdVars {
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkflowExecutionsVars {
    #[serde(rename = "workflowId")]
    pub workflow_id: String,
}

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
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

#[derive(Clone, Debug, Serialize)]
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

// ── Response wrappers ──────────────────────────────────────────────────────────

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

// ── Async fetch functions ──────────────────────────────────────────────────────

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, ApiError> {
    let resp: WorkflowsResponse =
        request(WORKFLOWS_QUERY, serde_json::json!({}), token, tenant_slug).await?;
    Ok(resp.workflows)
}

pub async fn fetch_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<WorkflowDetail>, ApiError> {
    let resp: WorkflowResponse =
        request(WORKFLOW_QUERY, IdVars { id }, token, tenant_slug).await?;
    Ok(resp.workflow)
}

pub async fn fetch_workflow_executions(
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

pub async fn create_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: CreateWorkflowInput,
) -> Result<String, ApiError> {
    let resp: CreateWorkflowResponse =
        request(CREATE_WORKFLOW_MUTATION, CreateWorkflowVars { input }, token, tenant_slug).await?;
    Ok(resp.create_workflow)
}

pub async fn delete_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let _: serde_json::Value =
        request(DELETE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug).await?;
    Ok(())
}

pub async fn activate_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let _: serde_json::Value =
        request(ACTIVATE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug).await?;
    Ok(())
}

pub async fn pause_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    let _: serde_json::Value =
        request(PAUSE_WORKFLOW_MUTATION, IdVars { id }, token, tenant_slug).await?;
    Ok(())
}

pub async fn add_step(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    input: CreateStepInput,
) -> Result<String, ApiError> {
    let resp: AddStepResponse = request(
        ADD_STEP_MUTATION,
        AddStepVars { workflow_id, input },
        token,
        tenant_slug,
    )
    .await?;
    Ok(resp.add_workflow_step)
}

pub async fn delete_step(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    step_id: String,
) -> Result<(), ApiError> {
    let _: serde_json::Value = request(
        DELETE_STEP_MUTATION,
        DeleteStepVars { workflow_id, step_id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(())
}

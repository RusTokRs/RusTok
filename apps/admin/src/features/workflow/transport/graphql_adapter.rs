use serde::{Deserialize, Serialize};

use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};
use crate::features::workflow::model::{
    CreateStepInput, CreateWorkflowInput, WorkflowTemplateDto, WorkflowVersionSummaryDto,
};
use crate::shared::api::{ApiError, request};

const WORKFLOWS_QUERY: &str =
    "query Workflows { workflows { id tenantId name status failureCount createdAt updatedAt } }";

const WORKFLOW_QUERY: &str = "query Workflow($id: UUID!) { workflow(id: $id) { id tenantId name description status triggerConfig createdBy createdAt updatedAt failureCount autoDisabledAt steps { id workflowId position stepType config onError timeoutMs } } }";

const WORKFLOW_EXECUTIONS_QUERY: &str = "query WorkflowExecutions($workflowId: UUID!) { workflowExecutions(workflowId: $workflowId) { id workflowId status error startedAt completedAt stepExecutions { id stepId status error startedAt completedAt } } }";

const CREATE_WORKFLOW_MUTATION: &str =
    "mutation CreateWorkflow($input: GqlCreateWorkflowInput!) { createWorkflow(input: $input) }";

const DELETE_WORKFLOW_MUTATION: &str =
    "mutation DeleteWorkflow($id: UUID!) { deleteWorkflow(id: $id) }";

const ACTIVATE_WORKFLOW_MUTATION: &str =
    "mutation ActivateWorkflow($id: UUID!) { activateWorkflow(id: $id) }";

const PAUSE_WORKFLOW_MUTATION: &str =
    "mutation PauseWorkflow($id: UUID!) { pauseWorkflow(id: $id) }";

const ADD_STEP_MUTATION: &str = "mutation AddWorkflowStep($workflowId: UUID!, $input: GqlCreateStepInput!) { addWorkflowStep(workflowId: $workflowId, input: $input) }";

const DELETE_STEP_MUTATION: &str = "mutation DeleteWorkflowStep($workflowId: UUID!, $stepId: UUID!) { deleteWorkflowStep(workflowId: $workflowId, stepId: $stepId) }";

const WORKFLOW_TEMPLATES_QUERY: &str =
    "query WorkflowTemplates { workflowTemplates { id name description category triggerConfig } }";

const CREATE_FROM_TEMPLATE_MUTATION: &str = "mutation CreateWorkflowFromTemplate($templateId: String!, $name: String!) { createWorkflowFromTemplate(templateId: $templateId, name: $name) }";

const WORKFLOW_VERSIONS_QUERY: &str = "query WorkflowVersions($workflowId: UUID!) { workflowVersions(workflowId: $workflowId) { id version createdBy createdAt } }";

const RESTORE_VERSION_MUTATION: &str = "mutation RestoreWorkflowVersion($workflowId: UUID!, $version: Int!) { restoreWorkflowVersion(workflowId: $workflowId, version: $version) }";

#[derive(Clone, Debug, Serialize)]
struct IdVars {
    id: String,
}

#[derive(Clone, Debug, Serialize)]
struct WorkflowExecutionsVars {
    #[serde(rename = "workflowId")]
    workflow_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct CreateWorkflowVars {
    input: CreateWorkflowInput,
}

#[derive(Clone, Debug, Serialize)]
struct AddStepVars {
    #[serde(rename = "workflowId")]
    workflow_id: String,
    input: CreateStepInput,
}

#[derive(Clone, Debug, Serialize)]
struct DeleteStepVars {
    #[serde(rename = "workflowId")]
    workflow_id: String,
    #[serde(rename = "stepId")]
    step_id: String,
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

#[derive(Clone, Debug, Deserialize)]
struct WorkflowsResponse {
    workflows: Vec<WorkflowSummary>,
}

#[derive(Clone, Debug, Deserialize)]
struct WorkflowResponse {
    workflow: Option<WorkflowDetail>,
}

#[derive(Clone, Debug, Deserialize)]
struct WorkflowExecutionsResponse {
    #[serde(rename = "workflowExecutions")]
    workflow_executions: Vec<WorkflowExecution>,
}

#[derive(Clone, Debug, Deserialize)]
struct CreateWorkflowResponse {
    #[serde(rename = "createWorkflow")]
    create_workflow: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AddStepResponse {
    #[serde(rename = "addWorkflowStep")]
    add_workflow_step: String,
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
    let resp: WorkflowResponse = request(WORKFLOW_QUERY, IdVars { id }, token, tenant_slug).await?;
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
    let resp: CreateWorkflowResponse = request(
        CREATE_WORKFLOW_MUTATION,
        CreateWorkflowVars { input },
        token,
        tenant_slug,
    )
    .await?;
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
    let _: serde_json::Value = request(
        ACTIVATE_WORKFLOW_MUTATION,
        IdVars { id },
        token,
        tenant_slug,
    )
    .await?;
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

pub async fn fetch_templates(
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

pub async fn create_from_template(
    token: Option<String>,
    tenant_slug: Option<String>,
    template_id: String,
    name: String,
) -> Result<String, ApiError> {
    let resp: CreateFromTemplateResponse = request(
        CREATE_FROM_TEMPLATE_MUTATION,
        CreateFromTemplateVars { template_id, name },
        token,
        tenant_slug,
    )
    .await?;
    Ok(resp.create_workflow_from_template)
}

pub async fn fetch_versions(
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

pub async fn restore_version(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    version: i32,
) -> Result<(), ApiError> {
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

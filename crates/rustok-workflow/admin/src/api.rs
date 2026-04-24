#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{WorkflowSummary, WorkflowTemplateDto};

pub type ApiError = GraphqlHttpError;

const WORKFLOWS_QUERY: &str =
    "query Workflows { workflows { id tenantId name status failureCount createdAt updatedAt } }";
const WORKFLOW_TEMPLATES_QUERY: &str =
    "query WorkflowTemplates { workflowTemplates { id name description category triggerConfig } }";
const CREATE_FROM_TEMPLATE_MUTATION: &str = "mutation CreateWorkflowFromTemplate($templateId: String!, $name: String!) { createWorkflowFromTemplate(templateId: $templateId, name: $name) }";

#[derive(Debug, Deserialize)]
struct WorkflowsResponse {
    workflows: Vec<WorkflowSummary>,
}

#[derive(Debug, Deserialize)]
struct TemplatesResponse {
    #[serde(rename = "workflowTemplates")]
    workflow_templates: Vec<WorkflowTemplateDto>,
}

#[derive(Debug, Deserialize)]
struct CreateFromTemplateResponse {
    #[serde(rename = "createWorkflowFromTemplate")]
    create_workflow_from_template: String,
}

#[derive(Debug, Serialize)]
struct EmptyVars {}

#[derive(Debug, Serialize)]
struct CreateFromTemplateVars {
    #[serde(rename = "templateId")]
    template_id: String,
    name: String,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, ApiError> {
    let response: WorkflowsResponse =
        request(WORKFLOWS_QUERY, EmptyVars {}, token, tenant_slug).await?;
    Ok(response.workflows)
}

pub async fn fetch_templates(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, ApiError> {
    let response: TemplatesResponse =
        request(WORKFLOW_TEMPLATES_QUERY, EmptyVars {}, token, tenant_slug).await?;
    Ok(response.workflow_templates)
}

pub async fn create_from_template(
    token: Option<String>,
    tenant_slug: Option<String>,
    template_id: String,
    name: String,
) -> Result<String, ApiError> {
    let response: CreateFromTemplateResponse = request(
        CREATE_FROM_TEMPLATE_MUTATION,
        CreateFromTemplateVars { template_id, name },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_workflow_from_template)
}

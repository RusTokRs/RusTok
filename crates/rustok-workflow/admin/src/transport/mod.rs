mod graphql_adapter;

use crate::model::{WorkflowSummary, WorkflowTemplateDto};

pub use graphql_adapter::TransportError;

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, TransportError> {
    graphql_adapter::fetch_workflows(token, tenant_slug).await
}

pub async fn fetch_templates(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, TransportError> {
    graphql_adapter::fetch_templates(token, tenant_slug).await
}

pub async fn create_from_template(
    token: Option<String>,
    tenant_slug: Option<String>,
    template_id: String,
    name: String,
) -> Result<String, TransportError> {
    graphql_adapter::create_from_template(token, tenant_slug, template_id, name).await
}

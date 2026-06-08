mod graphql_adapter;

use crate::core::{WorkflowAdminTransportContext, WorkflowTemplateCreateCommand};
use crate::model::{WorkflowSummary, WorkflowTemplateDto};

pub use graphql_adapter::TransportError;

pub async fn fetch_workflows(
    context: WorkflowAdminTransportContext,
) -> Result<Vec<WorkflowSummary>, TransportError> {
    graphql_adapter::fetch_workflows(context.token, context.tenant_slug).await
}

pub async fn fetch_templates(
    context: WorkflowAdminTransportContext,
) -> Result<Vec<WorkflowTemplateDto>, TransportError> {
    graphql_adapter::fetch_templates(context.token, context.tenant_slug).await
}

pub async fn create_from_template(
    context: WorkflowAdminTransportContext,
    command: WorkflowTemplateCreateCommand,
) -> Result<String, TransportError> {
    graphql_adapter::create_from_template(
        context.token,
        context.tenant_slug,
        command.template_id,
        command.workflow_name,
    )
    .await
}

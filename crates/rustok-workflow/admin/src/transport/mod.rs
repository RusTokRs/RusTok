mod graphql_adapter;
mod native_server_adapter;

use crate::core::{WorkflowAdminTransportContext, WorkflowTemplateCreateCommand};
use crate::model::{WorkflowSummary, WorkflowTemplateDto};
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};

pub type TransportError = UiTransportError;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_workflows(
    context: WorkflowAdminTransportContext,
) -> Result<Vec<WorkflowSummary>, TransportError> {
    execute_selected_transport(
        "workflow_admin",
        selected_transport_path(),
        native_server_adapter::fetch_workflows_native,
        move || graphql_adapter::fetch_workflows(context.token, context.tenant_slug),
    )
    .await
}

pub async fn fetch_templates(
    context: WorkflowAdminTransportContext,
) -> Result<Vec<WorkflowTemplateDto>, TransportError> {
    execute_selected_transport(
        "workflow_admin",
        selected_transport_path(),
        native_server_adapter::fetch_templates_native,
        move || graphql_adapter::fetch_templates(context.token, context.tenant_slug),
    )
    .await
}

pub async fn create_from_template(
    context: WorkflowAdminTransportContext,
    command: WorkflowTemplateCreateCommand,
) -> Result<String, TransportError> {
    let native_template_id = command.template_id.clone();
    let native_workflow_name = command.workflow_name.clone();
    execute_selected_transport(
        "workflow_admin",
        selected_transport_path(),
        move || {
            native_server_adapter::create_from_template_native(
                native_template_id,
                native_workflow_name,
            )
        },
        move || {
            graphql_adapter::create_from_template(
                context.token,
                context.tenant_slug,
                command.template_id,
                command.workflow_name,
            )
        },
    )
    .await
}

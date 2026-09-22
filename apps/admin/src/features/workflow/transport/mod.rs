mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;

use crate::entities::workflow::{WorkflowDetail, WorkflowExecution, WorkflowSummary};
use crate::shared::api::{ApiError, map_server_fn_error};

pub use crate::features::workflow::model::{
    CreateStepInput, CreateWorkflowInput, WorkflowTemplateDto, WorkflowVersionSummaryDto,
};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_workflows(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowSummary>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::list_workflows_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => graphql_adapter::fetch_workflows(token, tenant_slug)
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
        UiTransportPath::NativeServer => native_server_adapter::workflow_native(id)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => graphql_adapter::fetch_workflow(token, tenant_slug, id)
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
        UiTransportPath::NativeServer => {
            native_server_adapter::workflow_executions_native(workflow_id)
                .await
                .map_err(|error| error.to_string())
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_workflow_executions(token, tenant_slug, workflow_id)
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
        UiTransportPath::NativeServer => native_server_adapter::create_workflow_native(input)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            graphql_adapter::create_workflow(token, tenant_slug, input).await
        }
    }
}

pub async fn delete_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::delete_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => graphql_adapter::delete_workflow(token, tenant_slug, id).await,
    }
}

pub async fn activate_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::activate_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            graphql_adapter::activate_workflow(token, tenant_slug, id).await
        }
    }
}

pub async fn pause_workflow(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::pause_workflow_native(id)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => graphql_adapter::pause_workflow(token, tenant_slug, id).await,
    }
}

pub async fn add_step(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
    input: CreateStepInput,
) -> Result<String, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::add_step_native(workflow_id, input)
            .await
            .map_err(map_server_fn_error),
        UiTransportPath::Graphql => {
            graphql_adapter::add_step(token, tenant_slug, workflow_id, input).await
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
            native_server_adapter::delete_step_native(workflow_id, step_id)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            graphql_adapter::delete_step(token, tenant_slug, workflow_id, step_id).await
        }
    }
}

pub async fn fetch_templates(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<WorkflowTemplateDto>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::workflow_templates_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => graphql_adapter::fetch_templates(token, tenant_slug)
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
            native_server_adapter::create_from_template_native(template_id, name)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            graphql_adapter::create_from_template(token, tenant_slug, template_id, name).await
        }
    }
}

pub async fn fetch_versions(
    token: Option<String>,
    tenant_slug: Option<String>,
    workflow_id: String,
) -> Result<Vec<WorkflowVersionSummaryDto>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::workflow_versions_native(workflow_id)
                .await
                .map_err(|error| error.to_string())
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_versions(token, tenant_slug, workflow_id)
                .await
                .map_err(|error| error.to_string())
        }
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
            native_server_adapter::restore_version_native(workflow_id, version)
                .await
                .map_err(map_server_fn_error)
        }
        UiTransportPath::Graphql => {
            graphql_adapter::restore_version(token, tenant_slug, workflow_id, version).await
        }
    }
}

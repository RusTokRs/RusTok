use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::model::WorkflowStatus;
use crate::model::{WorkflowSummary, WorkflowTemplateDto};

#[server(prefix = "/api/fn", endpoint = "workflow-admin/list-workflows")]
pub async fn fetch_workflows_native() -> Result<Vec<WorkflowSummary>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("Permission denied: tenant mismatch"));
        }

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_LIST]) {
            return Err(ServerFnError::new("workflows:list required"));
        }

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .list(tenant.id)
            .await
            .map(|items| items.into_iter().map(map_workflow_summary).collect())
            .map_err(map_workflow_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "workflow-admin/list-workflows requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "workflow-admin/templates")]
pub async fn fetch_templates_native() -> Result<Vec<WorkflowTemplateDto>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, has_any_effective_permission};

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::WORKFLOWS_READ,
                Permission::WORKFLOWS_LIST,
                Permission::WORKFLOWS_CREATE,
            ],
        ) {
            return Err(ServerFnError::new(
                "workflow read, list, or create permission required",
            ));
        }

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
            "workflow-admin/templates requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "workflow-admin/create-from-template")]
pub async fn create_from_template_native(
    template_id: String,
    name: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("Permission denied: tenant mismatch"));
        }

        if !has_any_effective_permission(&auth.permissions, &[Permission::WORKFLOWS_CREATE]) {
            return Err(ServerFnError::new("workflows:create required"));
        }

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        rustok_workflow::WorkflowService::new(runtime_ctx.db_clone())
            .create_from_template(tenant.id, auth.human_user_id(), &template_id, name)
            .await
            .map(|id| id.to_string())
            .map_err(map_workflow_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (template_id, name);
        Err(ServerFnError::new(
            "workflow-admin/create-from-template requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_workflow_error(err: rustok_workflow::WorkflowError) -> ServerFnError {
    match err {
        rustok_workflow::WorkflowError::NotFound(id) => {
            ServerFnError::new(format!("Workflow not found: {id}"))
        }
        rustok_workflow::WorkflowError::VersionNotFound(id, version) => ServerFnError::new(
            format!("Workflow version not found: workflow_id={id}, version={version}"),
        ),
        rustok_workflow::WorkflowError::TemplateNotFound(id) => {
            ServerFnError::new(format!("Workflow template not found: {id}"))
        }
        rustok_workflow::WorkflowError::StepNotFound(id) => {
            ServerFnError::new(format!("Workflow step not found: {id}"))
        }
        rustok_workflow::WorkflowError::ExecutionNotFound(id) => {
            ServerFnError::new(format!("Workflow execution not found: {id}"))
        }
        rustok_workflow::WorkflowError::NotActive(status) => {
            ServerFnError::new(format!("Workflow is not active (status: {status})"))
        }
        rustok_workflow::WorkflowError::InvalidTriggerConfig(msg) => {
            ServerFnError::new(format!("Invalid trigger configuration: {msg}"))
        }
        rustok_workflow::WorkflowError::InvalidStepConfig(msg) => {
            ServerFnError::new(format!("Invalid step configuration: {msg}"))
        }
        rustok_workflow::WorkflowError::Database(db_err) => {
            tracing::error!(%db_err, "Workflow database operation failed");
            ServerFnError::new("Internal database error")
        }
        rustok_workflow::WorkflowError::Serialization(ser_err) => {
            tracing::error!(%ser_err, "Workflow serialization failed");
            ServerFnError::new("Serialization error")
        }
        other => {
            tracing::error!(%other, "Workflow operation failed");
            ServerFnError::new("Workflow operation failed")
        }
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

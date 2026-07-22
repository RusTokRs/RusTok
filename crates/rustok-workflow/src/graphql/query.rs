use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use rustok_api::{TenantContext, graphql::require_module_enabled};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::WorkflowService;

use super::{MODULE_SLUG, require_workflow_permission, types::*};

#[derive(Default)]
pub struct WorkflowQuery;

#[Object]
impl WorkflowQuery {
    async fn workflows(&self, ctx: &Context<'_>) -> Result<Vec<GqlWorkflowSummary>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_LIST],
            "Permission denied: workflows:list required",
        )?;

        let service = WorkflowService::new(db.clone());
        let workflows = service
            .list(tenant.id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(workflows.into_iter().map(Into::into).collect())
    }

    async fn workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<GqlWorkflow>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_READ],
            "Permission denied: workflows:read required",
        )?;

        let service = WorkflowService::new(db.clone());
        match service.get(tenant.id, id).await {
            Ok(workflow) => Ok(Some(workflow.into())),
            Err(crate::WorkflowError::NotFound(_)) => Ok(None),
            Err(err) => Err(async_graphql::Error::new(err.to_string())),
        }
    }

    async fn workflow_executions(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
    ) -> Result<Vec<GqlWorkflowExecution>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOW_EXECUTIONS_LIST],
            "Permission denied: workflow_executions:list required",
        )?;

        let service = WorkflowService::new(db.clone());
        let executions = service
            .list_executions(tenant.id, workflow_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(executions.into_iter().map(Into::into).collect())
    }

    async fn workflow_execution(
        &self,
        ctx: &Context<'_>,
        execution_id: Uuid,
    ) -> Result<Option<GqlWorkflowExecution>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOW_EXECUTIONS_READ],
            "Permission denied: workflow_executions:read required",
        )?;

        let service = WorkflowService::new(db.clone());
        match service.get_execution(tenant.id, execution_id).await {
            Ok(execution) => Ok(Some(execution.into())),
            Err(crate::WorkflowError::ExecutionNotFound(_)) => Ok(None),
            Err(err) => Err(async_graphql::Error::new(err.to_string())),
        }
    }

    async fn workflow_templates(&self, ctx: &Context<'_>) -> Result<Vec<GqlWorkflowTemplate>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;

        Ok(crate::BUILTIN_TEMPLATES.iter().map(Into::into).collect())
    }

    async fn workflow_versions(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
    ) -> Result<Vec<GqlWorkflowVersionSummary>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_READ],
            "Permission denied: workflows:read required",
        )?;

        let service = WorkflowService::new(db.clone());
        let versions = service
            .list_versions(tenant.id, workflow_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(versions.into_iter().map(Into::into).collect())
    }

    async fn workflow_version(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        version: i32,
    ) -> Result<Option<GqlWorkflowVersionDetail>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;

        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_READ],
            "Permission denied: workflows:read required",
        )?;

        let service = WorkflowService::new(db.clone());
        match service.get_version(tenant.id, workflow_id, version).await {
            Ok(version) => Ok(Some(version.into())),
            Err(_) => Ok(None),
        }
    }
}

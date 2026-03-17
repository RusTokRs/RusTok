use async_graphql::{Context, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;
use rustok_core::Permission;
use rustok_workflow::WorkflowService;

use super::types::*;

#[derive(Default)]
pub struct WorkflowQuery;

#[Object]
impl WorkflowQuery {
    /// List all workflows for the current tenant (resolved from auth headers)
    async fn workflows(&self, ctx: &Context<'_>) -> Result<Vec<GqlWorkflowSummary>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_LIST,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let workflows = service
            .list(tenant.id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(workflows.into_iter().map(Into::into).collect())
    }

    /// Get a single workflow with its steps
    async fn workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<GqlWorkflow>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_READ,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        match service.get(tenant.id, id).await {
            Ok(w) => Ok(Some(w.into())),
            Err(rustok_workflow::WorkflowError::NotFound(_)) => Ok(None),
            Err(e) => Err(async_graphql::Error::new(e.to_string())),
        }
    }

    /// List executions for a workflow
    async fn workflow_executions(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
    ) -> Result<Vec<GqlWorkflowExecution>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_LIST,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let executions = service
            .list_executions(tenant.id, workflow_id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(executions.into_iter().map(Into::into).collect())
    }

    /// Get a single execution by id
    async fn workflow_execution(
        &self,
        ctx: &Context<'_>,
        execution_id: Uuid,
    ) -> Result<Option<GqlWorkflowExecution>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_READ,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        match service.get_execution(tenant.id, execution_id).await {
            Ok(e) => Ok(Some(e.into())),
            Err(rustok_workflow::WorkflowError::ExecutionNotFound(_)) => Ok(None),
            Err(e) => Err(async_graphql::Error::new(e.to_string())),
        }
    }

    // ── Phase 4 queries ─────────────────────────────────────────────────────────

    /// List all built-in marketplace workflow templates.
    /// No authentication required — templates are public catalogue.
    async fn workflow_templates(&self, _ctx: &Context<'_>) -> Result<Vec<GqlWorkflowTemplate>> {
        Ok(rustok_workflow::BUILTIN_TEMPLATES
            .iter()
            .map(Into::into)
            .collect())
    }

    /// List saved versions for a workflow (newest first).
    async fn workflow_versions(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
    ) -> Result<Vec<GqlWorkflowVersionSummary>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_READ,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let versions = service
            .list_versions(tenant.id, workflow_id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(versions.into_iter().map(Into::into).collect())
    }

    /// Get the full snapshot for a specific workflow version.
    async fn workflow_version(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        version: i32,
    ) -> Result<Option<GqlWorkflowVersionDetail>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_READ,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        match service.get_version(tenant.id, workflow_id, version).await {
            Ok(v) => Ok(Some(v.into())),
            Err(_) => Ok(None),
        }
    }
}

async fn require_perm(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    perms: &[rustok_core::Permission],
) -> Result<()> {
    let has_perm = RbacService::has_any_permission(db, &tenant_id, &user_id, perms)
        .await
        .map_err(|e| {
            <async_graphql::FieldError as GraphQLError>::internal_error(&e.to_string())
        })?;

    if !has_perm {
        return Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
            "Permission denied",
        ));
    }
    Ok(())
}

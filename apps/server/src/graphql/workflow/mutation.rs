use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;
use rustok_core::Permission;
use rustok_workflow::{
    CreateWorkflowInput, CreateWorkflowStepInput, UpdateWorkflowInput, UpdateWorkflowStepInput,
    WorkflowService,
};
use rustok_workflow::entities::WorkflowStatus;

use super::types::*;

#[derive(Default)]
pub struct WorkflowMutation;

#[Object]
impl WorkflowMutation {
    /// Create a new workflow (starts as Draft)
    async fn create_workflow(&self, ctx: &Context<'_>, input: GqlCreateWorkflowInput) -> Result<Uuid> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_CREATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let id = service
            .create(
                tenant.id,
                Some(auth.user_id),
                CreateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    trigger_config: input.trigger_config,
                },
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(id)
    }

    /// Update a workflow
    async fn update_workflow(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlUpdateWorkflowInput,
    ) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .update(
                tenant.id,
                id,
                UpdateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    status: input.status.map(Into::into),
                    trigger_config: input.trigger_config,
                },
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Delete a workflow
    async fn delete_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_DELETE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .delete(tenant.id, id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Activate a workflow
    async fn activate_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .update(tenant.id, id, UpdateWorkflowInput {
                status: Some(WorkflowStatus::Active),
                ..Default::default()
            })
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Pause a workflow
    async fn pause_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .update(tenant.id, id, UpdateWorkflowInput {
                status: Some(WorkflowStatus::Paused),
                ..Default::default()
            })
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Manually trigger a workflow execution
    async fn trigger_workflow(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        payload: Option<Value>,
        force: Option<bool>,
    ) -> Result<Uuid> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_EXECUTE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let execution_id = service
            .trigger_manual(
                tenant.id,
                id,
                Some(auth.user_id),
                payload.unwrap_or_default(),
                force.unwrap_or(false),
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(execution_id)
    }

    /// Add a step to a workflow
    async fn add_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        input: GqlCreateStepInput,
    ) -> Result<Uuid> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        let step_id = service
            .add_step(
                tenant.id,
                workflow_id,
                CreateWorkflowStepInput {
                    position: input.position,
                    step_type: input.step_type.into(),
                    config: input.config,
                    on_error: input.on_error.into(),
                    timeout_ms: input.timeout_ms,
                },
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(step_id)
    }

    /// Update a workflow step
    async fn update_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        step_id: Uuid,
        input: GqlUpdateStepInput,
    ) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .update_step(
                tenant.id,
                workflow_id,
                step_id,
                UpdateWorkflowStepInput {
                    position: input.position,
                    step_type: input.step_type.map(Into::into),
                    config: input.config,
                    on_error: input.on_error.map(Into::into),
                    timeout_ms: input.timeout_ms,
                },
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Delete a workflow step
    async fn delete_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        step_id: Uuid,
    ) -> Result<bool> {
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_perm(db, tenant.id, auth.user_id, &[
            Permission::WORKFLOWS_UPDATE,
            Permission::WORKFLOWS_MANAGE,
        ])
        .await?;

        let service = WorkflowService::new(db.clone());
        service
            .delete_step(tenant.id, workflow_id, step_id)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }
}

async fn require_perm(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    perms: &[Permission],
) -> Result<()> {
    let has_perm = RbacService::has_any_permission(db, &tenant_id, &user_id, perms)
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

    if !has_perm {
        return Err(<FieldError as GraphQLError>::permission_denied("Permission denied"));
    }
    Ok(())
}

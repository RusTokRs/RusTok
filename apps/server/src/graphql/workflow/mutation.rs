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
                    webhook_slug: input.webhook_slug,
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
                Some(auth.user_id),
                UpdateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    status: input.status.map(Into::into),
                    trigger_config: input.trigger_config,
                    webhook_slug: input.webhook_slug,
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
            .update(tenant.id, id, Some(auth.user_id), UpdateWorkflowInput {
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
            .update(tenant.id, id, Some(auth.user_id), UpdateWorkflowInput {
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

    // ── Phase 4 mutations ──────────────────────────────────────────────────────

    /// Create a workflow from a built-in marketplace template.
    async fn create_workflow_from_template(
        &self,
        ctx: &Context<'_>,
        template_id: String,
        name: String,
    ) -> Result<Uuid> {
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
        service
            .create_from_template(tenant.id, Some(auth.user_id), &template_id, name)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Restore a workflow to a previously saved version.
    async fn restore_workflow_version(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        version: i32,
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
            .restore_version(tenant.id, workflow_id, version, Some(auth.user_id))
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(true)
    }

    /// Generate a workflow skeleton from a natural-language description.
    /// Uses the Alloy script engine to interpret the description and produce
    /// a workflow JSON which is saved as a Draft.
    ///
    /// Requires a registered Alloy script named `system/generate_workflow`.
    async fn generate_workflow_from_description(
        &self,
        ctx: &Context<'_>,
        description: String,
    ) -> Result<Uuid> {
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

        // If an Alloy ScriptRunner is available, delegate generation to the
        // registered `system/generate_workflow` Rhai script.
        // Otherwise fall back to a minimal Draft skeleton.
        let generated = if let Ok(runner) = ctx.data::<std::sync::Arc<dyn rustok_workflow::steps::ScriptRunner>>() {
            let params = serde_json::json!({ "description": description });
            match runner.run_script("system/generate_workflow", params).await {
                Ok(result) => result,
                Err(_) => default_generated_workflow(&description),
            }
        } else {
            default_generated_workflow(&description)
        };

        let trigger_config = generated
            .get("trigger_config")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({ "type": "manual" }));

        let service = WorkflowService::new(db.clone());
        let workflow_id = service
            .create(
                tenant.id,
                Some(auth.user_id),
                rustok_workflow::CreateWorkflowInput {
                    name: generated
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Generated Workflow")
                        .to_string(),
                    description: generated
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    trigger_config,
                    webhook_slug: None,
                },
            )
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(workflow_id)
    }
}

/// Fallback: generate a minimal Draft workflow when no Alloy script is registered.
fn default_generated_workflow(description: &str) -> serde_json::Value {
    serde_json::json!({
        "name": format!("Workflow: {}", &description[..description.len().min(50)]),
        "description": description,
        "trigger_config": { "type": "manual" },
        "steps": []
    })
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

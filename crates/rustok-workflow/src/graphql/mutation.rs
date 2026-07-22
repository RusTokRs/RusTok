use async_graphql::{Context, Object, Result};
use rustok_api::Permission;
use rustok_api::graphql::require_module_enabled;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    CreateWorkflowInput, CreateWorkflowStepInput, UpdateWorkflowInput, UpdateWorkflowStepInput,
    WorkflowService, entities::WorkflowStatus,
};

use super::{MODULE_SLUG, require_workflow_permission, types::*};

#[derive(Default)]
pub struct WorkflowMutation;

#[Object]
impl WorkflowMutation {
    async fn create_workflow(
        &self,
        ctx: &Context<'_>,
        input: GqlCreateWorkflowInput,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_CREATE],
            "Permission denied: workflows:create required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .create(
                tenant.id,
                auth.human_user_id(),
                CreateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    trigger_config: input.trigger_config,
                    webhook_slug: input.webhook_slug,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn update_workflow(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: GqlUpdateWorkflowInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .update(
                tenant.id,
                id,
                auth.human_user_id(),
                UpdateWorkflowInput {
                    name: input.name,
                    description: input.description,
                    status: input.status.map(Into::into),
                    trigger_config: input.trigger_config,
                    webhook_slug: input.webhook_slug,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn delete_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_DELETE],
            "Permission denied: workflows:delete required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .delete(tenant.id, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn activate_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .update(
                tenant.id,
                id,
                auth.human_user_id(),
                UpdateWorkflowInput {
                    status: Some(WorkflowStatus::Active),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn pause_workflow(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .update(
                tenant.id,
                id,
                auth.human_user_id(),
                UpdateWorkflowInput {
                    status: Some(WorkflowStatus::Paused),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn trigger_workflow(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        payload: Option<Value>,
        force: Option<bool>,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_EXECUTE],
            "Permission denied: workflows:execute required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .trigger_manual(
                tenant.id,
                id,
                auth.human_user_id(),
                payload.unwrap_or_default(),
                force.unwrap_or(false),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn add_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        input: GqlCreateStepInput,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
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
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn update_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        step_id: Uuid,
        input: GqlUpdateStepInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

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
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn delete_workflow_step(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        step_id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .delete_step(tenant.id, workflow_id, step_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn create_workflow_from_template(
        &self,
        ctx: &Context<'_>,
        template_id: String,
        name: String,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_CREATE],
            "Permission denied: workflows:create required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .create_from_template(tenant.id, auth.human_user_id(), &template_id, name)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn restore_workflow_version(
        &self,
        ctx: &Context<'_>,
        workflow_id: Uuid,
        version: i32,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_UPDATE],
            "Permission denied: workflows:update required",
        )?;

        let service = WorkflowService::new(db.clone());
        service
            .restore_version(tenant.id, workflow_id, version, auth.human_user_id())
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn generate_workflow_from_description(
        &self,
        ctx: &Context<'_>,
        description: String,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let auth = require_workflow_permission(
            ctx,
            &[Permission::WORKFLOWS_CREATE],
            "Permission denied: workflows:create required",
        )?;

        let generated = if let Ok(runner) = ctx.data::<Arc<dyn crate::steps::ScriptRunner>>() {
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
        service
            .create(
                tenant.id,
                auth.human_user_id(),
                CreateWorkflowInput {
                    name: generated
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("Generated Workflow")
                        .to_string(),
                    description: generated
                        .get("description")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    trigger_config,
                    webhook_slug: None,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

fn default_generated_workflow(description: &str) -> serde_json::Value {
    serde_json::json!({
        "name": format!("Workflow: {}", &description[..description.len().min(50)]),
        "description": description,
        "trigger_config": { "type": "manual" },
        "steps": []
    })
}

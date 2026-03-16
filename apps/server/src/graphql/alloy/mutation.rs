use async_graphql::{Context, Object, Result};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use alloy_scripting::model::Script;
use alloy_scripting::runner::ExecutionOutcome;
use alloy_scripting::utils::validate_cron_expression;
use alloy_scripting::ScriptRegistry;
use rhai::Dynamic;

use super::types::{
    CreateScriptInput, GqlExecutionResult, GqlScript, RunScriptInput, ScriptTriggerInput,
    UpdateScriptInput,
};
use crate::context::TenantContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::schema::module_slug;

use super::{dynamic_to_json, json_to_dynamic, require_admin, AlloyState};

fn validate_cron_trigger(trigger: &ScriptTriggerInput) -> Result<()> {
    if let ScriptTriggerInput::Cron(cron) = trigger {
        validate_cron_expression(&cron.expression)
            .map_err(|err| async_graphql::Error::new(format!("Invalid cron expression: {err}")))?;
    }
    Ok(())
}

#[derive(Default)]
pub struct AlloyMutation;

#[Object]
impl AlloyMutation {
    async fn create_script(
        &self,
        ctx: &Context<'_>,
        input: CreateScriptInput,
    ) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        validate_cron_trigger(&input.trigger)?;
        let state = ctx.data::<AlloyState>()?;
        let mut scope = rhai::Scope::new();
        state
            .engine
            .compile(&input.name, &input.code, &mut scope)
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        let tenant_id = ctx
            .data::<TenantContext>()
            .map(|t| t.id)
            .unwrap_or_default();

        let mut script = Script::new(input.name, input.code, input.trigger.into());
        script.tenant_id = tenant_id;
        script.description = input.description;
        script.run_as_system = input.run_as_system;
        script.permissions = input.permissions;
        script.author_id = input.author_id;
        if let Some(status) = input.status {
            script.status = status.into();
        }

        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn update_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateScriptInput,
    ) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        if let Some(name) = input.name {
            state.engine.invalidate(&script.name);
            script.name = name;
        }
        if let Some(description) = input.description {
            script.description = Some(description);
        }
        if let Some(code) = input.code {
            state.engine.invalidate(&script.name);
            let mut scope = rhai::Scope::new();
            state
                .engine
                .compile(&script.name, &code, &mut scope)
                .map_err(|err| async_graphql::Error::new(err.to_string()))?;
            script.code = code;
        }
        if let Some(ref trigger) = input.trigger {
            validate_cron_trigger(trigger)?;
        }
        if let Some(trigger) = input.trigger {
            script.trigger = trigger.into();
        }
        if let Some(status) = input.status {
            script.status = status.into();
        }
        if let Some(run_as_system) = input.run_as_system {
            script.run_as_system = run_as_system;
        }
        if let Some(permissions) = input.permissions {
            script.permissions = permissions;
        }
        if input.clear_author_id {
            script.author_id = None;
        } else if let Some(author_id) = input.author_id {
            script.author_id = Some(author_id);
        }

        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn delete_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        state
            .storage
            .delete(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn run_script(
        &self,
        ctx: &Context<'_>,
        input: RunScriptInput,
    ) -> Result<GqlExecutionResult> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        let auth = require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let user_id = Some(auth.user_id.to_string());

        let params = input
            .params
            .map(|params| -> Result<HashMap<String, Dynamic>> {
                let object = params
                    .0
                    .as_object()
                    .ok_or_else(|| async_graphql::Error::new("params must be a JSON object"))?;
                let params_map: HashMap<String, Dynamic> = object
                    .iter()
                    .map(|(key, value)| (key.clone(), json_to_dynamic(value.clone())))
                    .collect();
                Ok(params_map)
            })
            .transpose()?
            .unwrap_or_default();

        let result = state
            .orchestrator
            .run_manual(&input.script_name, params, user_id.clone())
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        let tenant_id = ctx.data::<TenantContext>().map(|t| t.id).ok();

        let _ = state
            .execution_log
            .record_with_context(&result, user_id, tenant_id)
            .await;

        let (success, error, return_value, changes) = match result.outcome {
            ExecutionOutcome::Success {
                ref return_value,
                ref entity_changes,
            } => (
                true,
                None,
                return_value.clone().map(dynamic_to_json),
                Some(serde_json::Value::Object(
                    entity_changes
                        .iter()
                        .map(|(key, value)| (key.clone(), dynamic_to_json(value.clone())))
                        .collect(),
                )),
            ),
            ExecutionOutcome::Aborted { ref reason } => (false, Some(reason.clone()), None, None),
            ExecutionOutcome::Failed { ref error } => (false, Some(error.to_string()), None, None),
        };

        Ok(GqlExecutionResult {
            execution_id: result.execution_id,
            success,
            duration_ms: result.duration_ms(),
            error,
            return_value: return_value.map(async_graphql::Json),
            changes: changes.map(async_graphql::Json),
        })
    }

    async fn activate_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        script.activate();
        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn pause_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        script.status = alloy_scripting::model::ScriptStatus::Paused;
        script.updated_at = Utc::now();

        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn disable_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        script.disable();
        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn archive_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        script.archive();
        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }

    async fn reset_script_errors(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_module_enabled(ctx, module_slug::ALLOY).await?;
        require_admin(ctx).await?;
        let state = ctx.data::<AlloyState>()?;
        let mut script = state
            .storage
            .get(id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        script.reset_errors();
        script.updated_at = Utc::now();

        let saved = state
            .storage
            .save(script)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(saved.into())
    }
}

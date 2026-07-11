use std::collections::HashMap;

use async_graphql::{Context, Json, Object, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    ScriptRegistry,
    model::{Script, ScriptStatus},
    runner::ExecutionOutcome,
    utils::{dynamic_to_json, json_to_dynamic, validate_cron_expression},
};

use super::{
    CreateScriptInput, GqlExecutionResult, GqlScript, RunScriptInput, ScriptTriggerInput,
    UpdateScriptInput, require_admin, runtime_from_graphql_ctx,
};

fn validate_cron_trigger(trigger: &ScriptTriggerInput) -> Result<()> {
    if let ScriptTriggerInput::Cron(cron) = trigger {
        validate_cron_expression(&cron.expression).map_err(|error| {
            async_graphql::Error::new(format!("Invalid cron expression: {error}"))
        })?;
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
        require_admin(ctx).await?;
        validate_cron_trigger(&input.trigger)?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut scope = rhai_full::Scope::new();
        runtime
            .engine
            .compile(&input.name, &input.code, &mut scope)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        let tenant_id = ctx
            .data::<rustok_api::TenantContext>()
            .map(|tenant| tenant.id)
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

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn update_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateScriptInput,
    ) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        if let Some(name) = input.name {
            runtime.engine.invalidate(&script.name);
            script.name = name;
        }
        if let Some(description) = input.description {
            script.description = Some(description);
        }
        if let Some(code) = input.code {
            runtime.engine.invalidate(&script.name);
            let mut scope = rhai_full::Scope::new();
            runtime
                .engine
                .compile(&script.name, &code, &mut scope)
                .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn delete_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        runtime
            .storage
            .delete(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(true)
    }

    async fn run_script(
        &self,
        ctx: &Context<'_>,
        input: RunScriptInput,
    ) -> Result<GqlExecutionResult> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let user_id = Some(auth.user_id.to_string());

        let params = input
            .params
            .map(|params| -> Result<HashMap<String, rhai_full::Dynamic>> {
                let object = params
                    .0
                    .as_object()
                    .ok_or_else(|| async_graphql::Error::new("params must be a JSON object"))?;
                Ok(object
                    .iter()
                    .map(|(key, value)| (key.clone(), json_to_dynamic(value.clone())))
                    .collect())
            })
            .transpose()?
            .unwrap_or_default();

        let result = runtime
            .orchestrator
            .run_manual(&input.script_name, params, user_id.clone())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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
            return_value: return_value.map(Json),
            changes: changes.map(Json),
        })
    }

    async fn activate_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        script.activate();
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn pause_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        script.status = ScriptStatus::Paused;
        script.updated_at = Utc::now();

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn disable_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        script.disable();
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn archive_script(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        script.archive();
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn reset_script_errors(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlScript> {
        require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        script.reset_errors();
        script.updated_at = Utc::now();

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }
}

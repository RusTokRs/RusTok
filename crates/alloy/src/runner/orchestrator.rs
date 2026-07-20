use std::collections::HashMap;
use std::sync::Arc;

use rhai::Dynamic;

use crate::context::{ExecutionContext, ExecutionPhase};
use crate::error::{ScriptError, ScriptResult};
use crate::execution_log::ExecutionLogSink;
use crate::model::{EntityProxy, EventType, Script};
use crate::storage::{ScriptQuery, ScriptRegistry};
use crate::AlloyDraftRuntime;

use super::executor::ScriptExecutor;
use super::result::{ExecutionOutcome, ExecutionResult, HookOutcome, PhaseResult};

pub struct ScriptOrchestrator<R: ScriptRegistry> {
    executor: ScriptExecutor<R>,
    registry: Arc<R>,
}

impl<R: ScriptRegistry> ScriptOrchestrator<R> {
    pub fn new(runtime: AlloyDraftRuntime, registry: Arc<R>) -> Self {
        Self {
            executor: ScriptExecutor::new(runtime, Arc::clone(&registry)),
            registry,
        }
    }

    pub fn with_execution_log(
        runtime: AlloyDraftRuntime,
        registry: Arc<R>,
        execution_log: Arc<dyn ExecutionLogSink>,
    ) -> Self {
        Self {
            executor: ScriptExecutor::new(runtime, Arc::clone(&registry))
                .with_execution_log(execution_log),
            registry,
        }
    }

    pub async fn run_before(
        &self,
        entity_type: &str,
        event: EventType,
        entity: EntityProxy,
        user_id: Option<String>,
    ) -> HookOutcome {
        let scripts = match self.find_scripts(entity_type, event).await {
            Ok(scripts) => scripts,
            Err(error) => return HookOutcome::Error { error },
        };

        if scripts.is_empty() {
            return HookOutcome::Continue {
                changes: HashMap::new(),
            };
        }

        let mut ctx = ExecutionContext::new(ExecutionPhase::Before);
        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }

        let mut phase_result = PhaseResult::new(ExecutionPhase::Before);
        let mut current_entity = entity;

        for script in scripts {
            let result = self
                .executor
                .execute(
                    &script,
                    &self.context_for_script(&ctx, &script),
                    Some(current_entity.clone()),
                )
                .await;

            if let ExecutionOutcome::Aborted { ref reason } = result.outcome {
                return HookOutcome::Rejected {
                    reason: reason.clone(),
                };
            }

            if let ExecutionOutcome::Failed { ref error } = result.outcome {
                return HookOutcome::Error {
                    error: error.clone(),
                };
            }

            if let ExecutionOutcome::Success {
                ref entity_changes, ..
            } = result.outcome
            {
                for (key, value) in entity_changes {
                    current_entity.set(key, value.clone());
                }
            }

            phase_result.results.push(result);
        }

        phase_result.merge_changes();

        HookOutcome::Continue {
            changes: phase_result.entity_changes,
        }
    }

    pub async fn run_after(
        &self,
        entity_type: &str,
        event: EventType,
        entity: EntityProxy,
        entity_before: Option<EntityProxy>,
        user_id: Option<String>,
    ) -> HookOutcome {
        let scripts = match self.find_scripts(entity_type, event).await {
            Ok(scripts) => scripts,
            Err(error) => return HookOutcome::Error { error },
        };

        if scripts.is_empty() {
            return HookOutcome::Continue {
                changes: HashMap::new(),
            };
        }

        let mut ctx =
            ExecutionContext::new(ExecutionPhase::After).with_entity_proxy(entity.clone());

        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }
        if let Some(before) = entity_before {
            ctx = ctx.with_entity_before_proxy(before);
        }

        let mut phase_result = PhaseResult::new(ExecutionPhase::After);

        for script in scripts {
            let result = self
                .executor
                .execute(
                    &script,
                    &self.context_for_script(&ctx, &script),
                    Some(entity.clone()),
                )
                .await;

            if let ExecutionOutcome::Aborted { ref reason } = result.outcome {
                return HookOutcome::Rejected {
                    reason: reason.clone(),
                };
            }

            if let ExecutionOutcome::Failed { ref error } = result.outcome {
                return HookOutcome::Error {
                    error: error.clone(),
                };
            }

            phase_result.results.push(result);
        }

        phase_result.merge_changes();

        HookOutcome::Continue {
            changes: phase_result.entity_changes,
        }
    }

    pub async fn run_on_commit(
        &self,
        entity_type: &str,
        entity: EntityProxy,
        user_id: Option<String>,
    ) -> Vec<ExecutionResult> {
        let scripts = match self.find_scripts(entity_type, EventType::OnCommit).await {
            Ok(scripts) => scripts,
            Err(_) => return Vec::new(),
        };

        if scripts.is_empty() {
            return Vec::new();
        }

        let mut ctx = ExecutionContext::new(ExecutionPhase::OnCommit);
        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }

        let mut results = Vec::new();
        for script in scripts {
            let result = self
                .executor
                .execute(
                    &script,
                    &self.context_for_script(&ctx, &script),
                    Some(entity.clone()),
                )
                .await;
            results.push(result);
        }

        results
    }

    pub async fn run_manual(
        &self,
        script_name: &str,
        params: HashMap<String, Dynamic>,
        user_id: Option<String>,
    ) -> ScriptResult<ExecutionResult> {
        let script = self.registry.get_by_name(script_name).await?;

        let mut ctx = ExecutionContext::new(ExecutionPhase::Manual)
            .with_params(params.into_iter().map(|(k, v)| (k.into(), v)).collect());

        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }

        Ok(self.execute_manual_snapshot(&script, ctx, None).await)
    }

    pub async fn run_manual_with_entity(
        &self,
        script_name: &str,
        params: HashMap<String, Dynamic>,
        entity: Option<EntityProxy>,
        user_id: Option<String>,
    ) -> ScriptResult<ExecutionResult> {
        let script = self.registry.get_by_name(script_name).await?;

        let mut ctx = ExecutionContext::new(ExecutionPhase::Manual)
            .with_params(params.into_iter().map(|(k, v)| (k.into(), v)).collect());

        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }

        Ok(self.execute_manual_snapshot(&script, ctx, entity).await)
    }

    /// Executes the exact script snapshot selected by the caller. Transport
    /// handlers use this after checking the caller's expected revision so a
    /// second registry lookup cannot replace the admitted source revision.
    pub async fn run_manual_snapshot(
        &self,
        script: &Script,
        params: HashMap<String, Dynamic>,
        entity: Option<EntityProxy>,
        user_id: Option<String>,
    ) -> ExecutionResult {
        let mut context = ExecutionContext::new(ExecutionPhase::Manual).with_params(
            params
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        );
        if let Some(user_id) = user_id {
            context = context.with_user(user_id);
        }
        self.execute_manual_snapshot(script, context, entity).await
    }

    pub async fn run_api(
        &self,
        path: &str,
        params: HashMap<String, Dynamic>,
        user_id: Option<String>,
    ) -> ScriptResult<ExecutionResult> {
        let scripts = self
            .registry
            .find(ScriptQuery::ByApiPath(path.to_string()))
            .await?;

        let script = scripts
            .into_iter()
            .next()
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("API endpoint: {path}"),
            })?;

        let mut ctx = ExecutionContext::new(ExecutionPhase::Manual)
            .with_params(params.into_iter().map(|(k, v)| (k.into(), v)).collect());

        if let Some(uid) = user_id {
            ctx = ctx.with_user(uid);
        }

        Ok(self
            .executor
            .execute(&script, &self.context_for_script(&ctx, &script), None)
            .await)
    }

    fn context_for_script(&self, ctx: &ExecutionContext, script: &Script) -> ExecutionContext {
        match &ctx.tenant_id {
            Some(_) => ctx.clone(),
            None => ctx.clone().with_tenant(script.tenant_id.to_string()),
        }
    }

    async fn execute_manual_snapshot(
        &self,
        script: &Script,
        context: ExecutionContext,
        entity: Option<EntityProxy>,
    ) -> ExecutionResult {
        self.executor
            .execute(script, &self.context_for_script(&context, script), entity)
            .await
    }

    async fn find_scripts(&self, entity_type: &str, event: EventType) -> ScriptResult<Vec<Script>> {
        self.registry
            .find(ScriptQuery::ByEvent {
                entity_type: entity_type.to_string(),
                event,
            })
            .await
    }
}

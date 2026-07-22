use chrono::Utc;
use std::sync::Arc;
use tracing::{Instrument, debug, warn};

use crate::AlloyDraftRuntime;
use crate::context::ExecutionContext;
use crate::error::ScriptError;
use crate::execution_log::ExecutionLogSink;
use crate::model::{EntityProxy, Script};
use crate::storage::ScriptRegistry;

use super::result::{ExecutionOutcome, ExecutionResult};

pub struct ScriptExecutor<R: ScriptRegistry> {
    runtime: AlloyDraftRuntime,
    registry: Arc<R>,
    max_chain_depth: usize,
    execution_log: Option<Arc<dyn ExecutionLogSink>>,
}

impl<R: ScriptRegistry> ScriptExecutor<R> {
    pub fn new(runtime: AlloyDraftRuntime, registry: Arc<R>) -> Self {
        Self {
            runtime,
            registry,
            max_chain_depth: 3,
            execution_log: None,
        }
    }

    pub fn with_max_chain_depth(mut self, depth: usize) -> Self {
        self.max_chain_depth = depth;
        self
    }

    pub fn with_execution_log(mut self, execution_log: Arc<dyn ExecutionLogSink>) -> Self {
        self.execution_log = Some(execution_log);
        self
    }

    pub async fn execute(
        &self,
        script: &Script,
        ctx: &ExecutionContext,
        entity: Option<EntityProxy>,
    ) -> ExecutionResult {
        let span = tracing::info_span!(
            "alloy.script.execute",
            script.id = %script.id,
            script.name = %script.name,
            execution.id = %ctx.execution_id,
            execution.phase = ?ctx.phase,
        );
        self.execute_inner(script, ctx, entity)
            .instrument(span)
            .await
    }

    async fn execute_inner(
        &self,
        script: &Script,
        ctx: &ExecutionContext,
        entity: Option<EntityProxy>,
    ) -> ExecutionResult {
        let execution_id = ctx.execution_id;
        let started_at = Utc::now();

        if ctx.call_depth > self.max_chain_depth {
            warn!(
                script.id = %script.id,
                depth = ctx.call_depth,
                max_depth = self.max_chain_depth,
                "Max call depth exceeded"
            );
            let result = ExecutionResult {
                script_id: script.id,
                script_name: script.name.clone(),
                execution_id,
                phase: ctx.phase,
                started_at,
                finished_at: Utc::now(),
                outcome: ExecutionOutcome::Failed {
                    error: ScriptError::MaxDepthExceeded {
                        depth: ctx.call_depth,
                    },
                },
            };
            self.record_execution(&result, ctx).await;
            return result;
        }

        let ctx_with_entity = match entity {
            Some(proxy) => ctx.clone().with_entity_proxy(proxy),
            None => ctx.clone(),
        };

        debug!(
            script.id = %script.id,
            script.name = %script.name,
            phase = ?ctx.phase,
            "Executing script"
        );

        let outcome = match self.runtime.execute(script, &ctx_with_entity).await {
            Ok((return_value, entity_changes)) => {
                debug!(
                    script.id = %script.id,
                    changes_count = entity_changes.len(),
                    "Script completed successfully"
                );

                ExecutionOutcome::Success {
                    return_value: Some(return_value),
                    entity_changes,
                }
            }
            Err(ScriptError::Aborted(reason)) => {
                debug!(
                    script.id = %script.id,
                    reason = %reason,
                    "Script aborted"
                );
                ExecutionOutcome::Aborted { reason }
            }
            Err(error) => {
                warn!(
                    script.id = %script.id,
                    error = %error,
                    "Script failed"
                );
                let _ = self.registry.record_error(script.id).await;
                ExecutionOutcome::Failed { error }
            }
        };

        let result = ExecutionResult {
            script_id: script.id,
            script_name: script.name.clone(),
            execution_id,
            phase: ctx.phase,
            started_at,
            finished_at: Utc::now(),
            outcome,
        };

        self.record_execution(&result, &ctx_with_entity).await;
        result
    }

    async fn record_execution(&self, result: &ExecutionResult, ctx: &ExecutionContext) {
        if let Some(execution_log) = &self.execution_log {
            if let Err(error) = execution_log.record_result(result, ctx).await {
                warn!(
                    script.id = %result.script_id,
                    execution.id = %result.execution_id,
                    error = %error,
                    "Failed to persist Alloy execution log"
                );
            }
        }
    }
}

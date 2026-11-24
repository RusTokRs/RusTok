use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde_json::Value;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::entities::{
    ExecutionStatus, OnError, StepExecutionStatus, StepType, WorkflowExecutionActiveModel,
    WorkflowStepExecutionActiveModel, workflow_cron_firing, workflow_execution, workflow_step_execution,
};
use crate::error::WorkflowResult;
use crate::steps::{
    ActionStep, AlloyScriptStep, ConditionStep, DelayStep, EmitEventStep, HttpStep, NotifyStep,
    StepContext, StepOutput, WorkflowStep,
};

/// A configured step timeout is an actual deadline, not stored-only
/// metadata. Very large or nonpositive durations are rejected explicitly.
const MAX_STEP_TIMEOUT_MS: i64 = 3_600_000;

pub(crate) fn validate_step_timeout_ms(timeout_ms: Option<i64>) -> WorkflowResult<()> {
    if let Some(ms) = timeout_ms {
        if !(1..=MAX_STEP_TIMEOUT_MS).contains(&ms) {
            return Err(crate::error::WorkflowError::InvalidStepConfig(format!(
                "timeout_ms must be between 1 and {MAX_STEP_TIMEOUT_MS}"
            )));
        }
    }
    Ok(())
}

async fn execute_with_step_timeout(
    executor: &dyn WorkflowStep,
    step: &crate::entities::WorkflowStep,
    context: StepContext,
) -> WorkflowResult<StepOutput> {
    validate_step_timeout_ms(step.timeout_ms)?;
    if let Some(timeout_ms) = step.timeout_ms {
        tokio::time::timeout(
            tokio::time::Duration::from_millis(timeout_ms as u64),
            executor.execute(&step.config, context),
        )
        .await
        .map_err(|_| crate::error::WorkflowError::StepTimedOut {
            step_id: step.id,
            timeout_ms: timeout_ms as u64,
        })?
    } else {
        executor.execute(&step.config, context).await
    }
}

/// External or script steps do not support blind retries. Their owners need
/// an idempotency receipt or explicit reconciliation before re-dispatch.
fn potentially_external_step(step_type: &StepType) -> bool {
    matches!(
        step_type,
        StepType::Http | StepType::Notify | StepType::AlloyScript | StepType::EmitEvent
    )
}

/// Registry of available step executors, keyed by step type string.
type StepRegistry = HashMap<String, Arc<dyn WorkflowStep>>;

fn default_registry() -> StepRegistry {
    let mut map: StepRegistry = HashMap::new();
    map.insert("action".into(), Arc::new(ActionStep));
    map.insert("emit_event".into(), Arc::new(EmitEventStep));
    map.insert("condition".into(), Arc::new(ConditionStep));
    map.insert("delay".into(), Arc::new(DelayStep));
    map.insert("http".into(), Arc::new(HttpStep::new()));
    map.insert("alloy_script".into(), Arc::new(AlloyScriptStep::stub()));
    map.insert("notify".into(), Arc::new(NotifyStep::stub()));
    map.insert("transform".into(), Arc::new(ActionStep)); // placeholder
    map
}

/// Executes workflow step chains linearly, persisting execution logs.
pub struct WorkflowEngine {
    db: DatabaseConnection,
    steps: StepRegistry,
}

impl WorkflowEngine {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            steps: default_registry(),
        }
    }

    pub fn register_step<S: WorkflowStep + 'static>(&mut self, step: S) {
        self.steps.insert(step.step_type().into(), Arc::new(step));
    }

    /// Execute a workflow given its steps and an initial context derived from the trigger event.
    #[instrument(skip(self, steps, initial_context), fields(workflow_id = %workflow_id))]
    pub async fn execute(
        &self,
        workflow_id: Uuid,
        tenant_id: Uuid,
        trigger_event_id: Option<Uuid>,
        steps: Vec<crate::entities::WorkflowStep>,
        initial_context: Value,
    ) -> WorkflowResult<Uuid> {
        if let Some(event_id) = trigger_event_id {
            if let Some(existing) = self.event_execution(workflow_id, event_id).await? {
                return Self::replay_terminal_execution(existing, event_id);
            }
        }

        let execution_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();

        // Create execution record
        let execution = WorkflowExecutionActiveModel {
            id: Set(execution_id),
            workflow_id: Set(workflow_id),
            tenant_id: Set(tenant_id),
            trigger_event_id: Set(trigger_event_id),
            status: Set(ExecutionStatus::Running),
            context: Set(initial_context.clone()),
            error: Set(None),
            started_at: Set(now),
            completed_at: Set(None),
        };
        if let Err(error) = execution.insert(&self.db).await {
            if let Some(event_id) = trigger_event_id {
                if let Some(existing) = self.event_execution(workflow_id, event_id).await? {
                    return Self::replay_terminal_execution(existing, event_id);
                }
            }
            return Err(error.into());
        }

        self.run_recorded_execution(execution_id, steps, initial_context, None).await
    }

    /// The Workflow cron owner inserts a trigger journal and checks its firing
    /// attempt inside one DB transaction before calling this method. Never
    /// insert another execution row or bypass that reservation here.
    #[instrument(skip(self, steps, initial_context), fields(execution_id = %execution_id, firing_id = %firing_id))]
    pub(crate) async fn execute_preclaimed_cron(
        &self,
        execution_id: Uuid,
        firing_id: Uuid,
        attempt_token: Uuid,
        steps: Vec<crate::entities::WorkflowStep>,
        initial_context: Value,
    ) -> WorkflowResult<Uuid> {
        self.run_recorded_execution(
            execution_id, steps, initial_context, Some((firing_id, attempt_token)),
        ).await
    }

    async fn run_recorded_execution(
        &self,
        execution_id: Uuid,
        steps: Vec<crate::entities::WorkflowStep>,
        initial_context: Value,
        cron_fence: Option<(Uuid, Uuid)>,
    ) -> WorkflowResult<Uuid> {
        info!(
            execution_id = %execution_id,
            steps = steps.len(),
            "Starting workflow execution"
        );

        let mut context = StepContext::new(initial_context);
        let mut failed = false;
        let mut timed_out = false;
        let mut failure_msg = String::new();

        'steps: for step in &steps {
            // Another process can classify an expired attempt for review,
            // but cannot authorize this stale owner to issue another effect.
            // Check the DB fence before every next step, not only when the
            // execution is first journaled.
            if let Some((firing_id, token)) = cron_fence {
                self.assert_cron_attempt(firing_id, token).await?;
            }
            let step_execution_id = Uuid::new_v4();
            let step_input = context.data.clone();

            // Record step as running
            let step_exec = WorkflowStepExecutionActiveModel {
                id: Set(step_execution_id),
                execution_id: Set(execution_id),
                step_id: Set(step.id),
                status: Set(StepExecutionStatus::Running),
                input: Set(step_input.clone()),
                output: Set(Value::Null),
                error: Set(None),
                started_at: Set(Utc::now().fixed_offset()),
                completed_at: Set(None),
            };
            step_exec.insert(&self.db).await?;

            let step_type_str = step.step_type.to_string();
            let executor = match self.steps.get(&step_type_str) {
                Some(e) => e.clone(),
                None => {
                    warn!(step_type = %step_type_str, "No executor registered for step type");
                    self.finish_step_execution(
                        step_execution_id,
                        StepExecutionStatus::Failed,
                        step_input,
                        Value::Null,
                        Some(format!("Unknown step type: {step_type_str}")),
                    )
                    .await?;

                    match step.on_error {
                        OnError::Stop => {
                            failed = true;
                            failure_msg = format!("Unknown step type: {step_type_str}");
                            break 'steps;
                        }
                        OnError::Skip => {
                            self.update_step_status(
                                step_execution_id,
                                StepExecutionStatus::Skipped,
                            ).await?;
                            continue 'steps;
                        }
                        OnError::Retry => {
                            // Retry once (simple implementation)
                            failed = true;
                            failure_msg = format!("Unknown step type: {step_type_str}");
                            break 'steps;
                        }
                    }
                }
            };

            let result = execute_with_step_timeout(executor.as_ref(), step, context.clone()).await;

            match result {
                Ok(output) => {
                    self.finish_step_execution(
                        step_execution_id,
                        StepExecutionStatus::Completed,
                        step_input,
                        output.data.clone(),
                        None,
                    )
                    .await?;

                    context = output.context;

                    if !output.should_continue {
                        info!(step_id = %step.id, "Step signalled stop — halting execution");
                        break 'steps;
                    }
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let step_timed_out = matches!(err, crate::error::WorkflowError::StepTimedOut { .. });
                    error!(step_id = %step.id, error = %err_msg, "Step failed");

                    let external_outcome_uncertain = potentially_external_step(&step.step_type);
                    self.finish_step_execution(
                        step_execution_id,
                        if external_outcome_uncertain {
                            StepExecutionStatus::ReconciliationRequired
                        } else {
                            StepExecutionStatus::Failed
                        },
                        step_input.clone(),
                        Value::Null,
                        Some(err_msg.clone()),
                    )
                    .await?;

                    // These steps may have issued a non-transactional effect
                    // before receiving a timeout or a lost response. Neither
                    // on_error=retry nor on_error=skip can prove it is safe to
                    // send again or ignore the result. Preserve uncertainty
                    // for an operator instead of claiming a terminal failure.
                    // An error variant does not prove where the external
                    // effect stopped. Even a misclassified validation error
                    // from a pluggable executor may follow a remote send.
                    if external_outcome_uncertain {
                        let uncertain_reason =
                            Some(format!("step {} has an unconfirmed external outcome", step.id));
                        if let Some((firing_id, token)) = cron_fence {
                            self.finish_claimed_cron_execution(
                                execution_id,
                                firing_id,
                                token,
                                ExecutionStatus::ReconciliationRequired,
                                context.data,
                                uncertain_reason,
                            ).await?;
                        } else {
                            self.finish_execution(
                                execution_id,
                                ExecutionStatus::ReconciliationRequired,
                                context.data,
                                uncertain_reason,
                            ).await?;
                        }
                        return Err(crate::error::WorkflowError::ExecutionOutcomeUncertain(
                            execution_id,
                        ));
                    }

                    match step.on_error {
                        OnError::Stop => {
                            failed = true;
                            timed_out = step_timed_out;
                            failure_msg = err_msg;
                            break 'steps;
                        }
                        OnError::Skip => {
                            // Update step status to skipped and continue
                            self.update_step_status(
                                step_execution_id,
                                StepExecutionStatus::Skipped,
                            )
                            .await?;
                            continue 'steps;
                        }
                        OnError::Retry => {
                            // Retry with exponential backoff
                            // Configurable via step config: { "max_retries": 3, "retry_base_ms": 1000 }
                            let max_retries = step
                                .config
                                .get("max_retries")
                                .and_then(Value::as_u64)
                                .unwrap_or(3)
                                .min(10) as u32;
                            let retry_base_ms = step
                                .config
                                .get("retry_base_ms")
                                .and_then(Value::as_u64)
                                .unwrap_or(1000);

                            let mut last_err = err_msg.clone();
                            let mut last_timed_out = step_timed_out;
                            let mut succeeded = false;
                            let mut stop_after_success = false;

                            for attempt in 1..=max_retries {
                                let multiplier = 2u64
                                    .checked_pow(attempt.saturating_sub(1))
                                    .unwrap_or(u64::MAX);
                                let backoff = retry_base_ms.saturating_mul(multiplier).min(300_000);
                                warn!(
                                    step_id = %step.id,
                                    attempt = attempt,
                                    backoff_ms = backoff,
                                    "Retrying step after failure"
                                );
                                tokio::time::sleep(tokio::time::Duration::from_millis(backoff))
                                    .await;

                                match execute_with_step_timeout(
                                    executor.as_ref(), step, context.clone(),
                                ).await {
                                    Ok(out) => {
                                        stop_after_success = !out.should_continue;
                                        self.finish_step_execution(
                                            step_execution_id,
                                            StepExecutionStatus::Completed,
                                            step_input.clone(),
                                            out.data,
                                            None,
                                        )
                                        .await?;
                                        context = out.context;
                                        succeeded = true;
                                        break;
                                    }
                                    Err(retry_err) => {
                                        last_timed_out = matches!(
                                            retry_err, crate::error::WorkflowError::StepTimedOut { .. }
                                        );
                                        last_err = retry_err.to_string();
                                    }
                                }
                            }

                            if succeeded && stop_after_success {
                                // The same stop signal must be honored on
                                // first attempt and after a successful retry.
                                break 'steps;
                            }
                            if !succeeded {
                                failed = true;
                                timed_out = last_timed_out;
                                failure_msg = last_err.clone();
                                self.finish_step_execution(
                                    step_execution_id,
                                    StepExecutionStatus::Failed,
                                    step_input,
                                    Value::Null,
                                    Some(failure_msg.clone()),
                                )
                                .await?;
                                break 'steps;
                            }
                        }
                    }
                }
            }
        }

        // A superseded attempt must not publish a terminal execution state.
        // Its earlier external effects need reconciliation; a stale worker
        // cannot certify completion after the operator has changed the fence.
        if let Some((firing_id, token)) = cron_fence {
            self.assert_cron_attempt(firing_id, token).await?;
        }

        // Finalize execution
        let final_status = if failed && timed_out {
            ExecutionStatus::TimedOut
        } else if failed {
            ExecutionStatus::Failed
        } else {
            ExecutionStatus::Completed
        };

        let last_error = if failed { Some(failure_msg) } else { None };
        if let Some((firing_id, token)) = cron_fence {
            self.finish_claimed_cron_execution(
                execution_id, firing_id, token, final_status, context.data, last_error,
            ).await?;
        } else {
            self.finish_execution(execution_id, final_status, context.data, last_error).await?;
        }

        info!(execution_id = %execution_id, success = !failed, "Workflow execution finished");

        Ok(execution_id)
    }

    async fn assert_cron_attempt(&self, firing_id: Uuid, token: Uuid) -> WorkflowResult<()> {
        let owned = workflow_cron_firing::Entity::find_by_id(firing_id)
            .filter(workflow_cron_firing::Column::Status.eq(workflow_cron_firing::RUNNING))
            .filter(workflow_cron_firing::Column::AttemptToken.eq(token))
            .one(&self.db).await?.is_some();
        if !owned {
            return Err(crate::error::WorkflowError::CronReconciliationConflict(firing_id));
        }
        Ok(())
    }

    /// A duplicate trigger is safe to replay only when the first execution
    /// has a *terminal* durable result. A running execution may have sent an
    /// external effect and then lost its acknowledgement; returning success
    /// here would incorrectly acknowledge the source event as delivered.
    fn replay_terminal_execution(
        existing: workflow_execution::Model,
        trigger_event_id: Uuid,
    ) -> WorkflowResult<Uuid> {
        if matches!(existing.status, ExecutionStatus::Running | ExecutionStatus::ReconciliationRequired) {
            return Err(crate::error::WorkflowError::ExecutionOutcomeUncertain(existing.id));
        }
        info!(
            execution_id = %existing.id,
            trigger_event_id = %trigger_event_id,
            status = %existing.status,
            "Replaying terminal workflow event execution without re-running steps"
        );
        Ok(existing.id)
    }

    async fn event_execution(
        &self,
        workflow_id: Uuid,
        trigger_event_id: Uuid,
    ) -> WorkflowResult<Option<workflow_execution::Model>> {
        Ok(workflow_execution::Entity::find()
            .filter(workflow_execution::Column::WorkflowId.eq(workflow_id))
            .filter(workflow_execution::Column::TriggerEventId.eq(trigger_event_id))
            .one(&self.db)
            .await?)
    }

    async fn finish_step_execution(
        &self,
        id: Uuid,
        status: StepExecutionStatus,
        _input: Value,
        output: Value,
        error: Option<String>,
    ) -> WorkflowResult<()> {
        use crate::entities::workflow_step_execution::Column;

        workflow_step_execution::Entity::update_many()
            .col_expr(
                workflow_step_execution::Column::Status,
                Expr::value(status.to_string()),
            )
            .col_expr(workflow_step_execution::Column::Output, Expr::value(output))
            .col_expr(workflow_step_execution::Column::Error, Expr::value(error))
            .col_expr(
                workflow_step_execution::Column::CompletedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(Column::Id.eq(id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn update_step_status(
        &self,
        id: Uuid,
        status: StepExecutionStatus,
    ) -> WorkflowResult<()> {
        use crate::entities::workflow_step_execution::Column;

        workflow_step_execution::Entity::update_many()
            .col_expr(
                workflow_step_execution::Column::Status,
                Expr::value(status.to_string()),
            )
            .filter(Column::Id.eq(id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    /// The current cron attempt owns terminal writes only while its firing
    /// token is still current. Lock that row and update the execution journal
    /// in the same transaction so an operator decision cannot be overwritten
    /// between a separate check and the final DB status write.
    async fn finish_claimed_cron_execution(
        &self,
        execution_id: Uuid,
        firing_id: Uuid,
        token: Uuid,
        status: ExecutionStatus,
        context: Value,
        error: Option<String>,
    ) -> WorkflowResult<()> {
        let transaction = self.db.begin().await?;
        let owns = workflow_cron_firing::Entity::update_many()
            .col_expr(
                workflow_cron_firing::Column::ClaimedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(workflow_cron_firing::Column::Id.eq(firing_id))
            .filter(workflow_cron_firing::Column::Status.eq(workflow_cron_firing::RUNNING))
            .filter(workflow_cron_firing::Column::AttemptToken.eq(token))
            .filter(workflow_cron_firing::Column::ExecutionId.eq(execution_id))
            .exec(&transaction).await?.rows_affected;
        if owns != 1 {
            return Err(crate::error::WorkflowError::CronReconciliationConflict(firing_id));
        }
        let completed = workflow_execution::Entity::update_many()
            .col_expr(workflow_execution::Column::Status, Expr::value(status.to_string()))
            .col_expr(workflow_execution::Column::Context, Expr::value(context))
            .col_expr(workflow_execution::Column::Error, Expr::value(error))
            .col_expr(workflow_execution::Column::CompletedAt, Expr::value(Utc::now().fixed_offset()))
            .filter(workflow_execution::Column::Id.eq(execution_id))
            .filter(workflow_execution::Column::TriggerEventId.eq(firing_id))
            .filter(workflow_execution::Column::Status.eq(ExecutionStatus::Running.to_string()))
            .exec(&transaction).await?.rows_affected;
        if completed != 1 {
            return Err(crate::error::WorkflowError::CronReconciliationConflict(firing_id));
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn finish_execution(
        &self,
        id: Uuid,
        status: ExecutionStatus,
        context: Value,
        error: Option<String>,
    ) -> WorkflowResult<()> {
        use crate::entities::workflow_execution::Column;

        workflow_execution::Entity::update_many()
            .col_expr(
                workflow_execution::Column::Status,
                Expr::value(status.to_string()),
            )
            .col_expr(workflow_execution::Column::Context, Expr::value(context))
            .col_expr(workflow_execution::Column::Error, Expr::value(error))
            .col_expr(
                workflow_execution::Column::CompletedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(Column::Id.eq(id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod durable_external_step_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use sea_orm::{Database, EntityTrait};
    use sea_orm_migration::SchemaManager;
    use crate::entities::{workflow, workflow_step, WorkflowStatus};
    use crate::error::WorkflowError;
    use crate::steps::{StepOutput, WorkflowStep};

    struct AmbiguousHttp {
        calls: Arc<AtomicUsize>,
        misclassified_validation_error: bool,
    }

    #[async_trait::async_trait]
    impl WorkflowStep for AmbiguousHttp {
        fn step_type(&self) -> &'static str { "http" }

        async fn execute(
            &self,
            _config: &Value,
            _context: StepContext,
        ) -> WorkflowResult<StepOutput> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.misclassified_validation_error {
                // A plugin can misclassify a post-send failure. The engine
                // must not assume this label proves pre-effect validation.
                Err(WorkflowError::InvalidStepConfig(
                    "provider reported invalid request after accepting it".to_owned(),
                ))
            } else {
                Err(WorkflowError::StepFailed(
                    "remote request may have completed before response was lost".to_owned(),
                ))
            }
        }
    }

    struct RetryThenStopAction {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl WorkflowStep for RetryThenStopAction {
        fn step_type(&self) -> &'static str {
            "action"
        }

        async fn execute(
            &self,
            _config: &Value,
            context: StepContext,
        ) -> WorkflowResult<StepOutput> {
            let attempt = self.calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Err(WorkflowError::StepFailed("transient pure step error".into()))
            } else {
                Ok(StepOutput::stop_with(
                    context,
                    serde_json::json!({"stopped_after_retry": true}),
                ))
            }
        }
    }

    struct SlowAction;

    #[async_trait::async_trait]
    impl WorkflowStep for SlowAction {
        fn step_type(&self) -> &'static str {
            "action"
        }

        async fn execute(
            &self,
            _config: &Value,
            context: StepContext,
        ) -> WorkflowResult<StepOutput> {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            Ok(StepOutput::continue_with(context, Value::Null))
        }
    }

    #[tokio::test]
    async fn configured_step_timeout_is_enforced_and_invalid_bounds_are_rejected() {
        let step_id = Uuid::new_v4();
        let mut step = workflow_step::Model {
            id: step_id,
            workflow_id: Uuid::new_v4(),
            position: 0,
            step_type: StepType::Action,
            config: Value::Null,
            on_error: OnError::Stop,
            timeout_ms: Some(1),
        };
        let result = execute_with_step_timeout(
            &SlowAction, &step, StepContext::default(),
        ).await;
        assert!(matches!(
            result,
            Err(WorkflowError::StepTimedOut { step_id: id, timeout_ms: 1 })
                if id == step_id
        ));

        for invalid in [0, -1, MAX_STEP_TIMEOUT_MS + 1] {
            step.timeout_ms = Some(invalid);
            assert!(matches!(
                execute_with_step_timeout(
                    &SlowAction, &step, StepContext::default(),
                ).await,
                Err(WorkflowError::InvalidStepConfig(_)),
            ));
        }
        step.timeout_ms = None;
        assert!(execute_with_step_timeout(
            &SlowAction, &step, StepContext::default(),
        ).await.is_ok());
    }

    #[tokio::test]
    async fn successful_retry_honors_stop_without_executing_following_step() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        workflow::ActiveModel {
            id: Set(workflow_id),
            tenant_id: Set(tenant_id),
            name: Set("Retry stop semantics".to_owned()),
            description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({"type":"manual"})),
            created_by: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            failure_count: Set(0),
            auto_disabled_at: Set(None),
            webhook_slug: Set(None),
            webhook_secret: Set(None),
        }.insert(&db).await.unwrap();

        let first = workflow_step::ActiveModel {
            id: Set(Uuid::new_v4()),
            workflow_id: Set(workflow_id),
            position: Set(1),
            step_type: Set(StepType::Action),
            config: Set(serde_json::json!({"max_retries":1,"retry_base_ms":0})),
            on_error: Set(OnError::Retry),
            timeout_ms: Set(None),
        }.insert(&db).await.unwrap();
        let second = workflow_step::ActiveModel {
            id: Set(Uuid::new_v4()),
            workflow_id: Set(workflow_id),
            position: Set(2),
            step_type: Set(StepType::Action),
            config: Set(Value::Null),
            on_error: Set(OnError::Stop),
            timeout_ms: Set(None),
        }.insert(&db).await.unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = WorkflowEngine::new(db.clone());
        engine.register_step(RetryThenStopAction { calls: Arc::clone(&calls) });
        let execution_id = engine.execute(
            workflow_id, tenant_id, None, vec![first.clone(), second],
            serde_json::json!({}),
        ).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let rows = crate::entities::workflow_step_execution::Entity::find()
            .filter(
                crate::entities::workflow_step_execution::Column::ExecutionId.eq(execution_id),
            )
            .all(&db).await.unwrap();
        assert_eq!(rows.len(), 1, "a stop after retry must not admit another step");
        assert_eq!(rows[0].step_id, first.id);
        assert_eq!(rows[0].status, StepExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn ambiguous_external_failure_never_retries_or_replays_trigger() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::migrations() {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        let now = Utc::now().fixed_offset();
        let tenant_id = Uuid::new_v4();
        let workflow_id = Uuid::new_v4();
        let trigger_id = Uuid::new_v4();

        workflow::ActiveModel {
            id: Set(workflow_id), tenant_id: Set(tenant_id),
            name: Set("Ambiguous external effect".into()), description: Set(None),
            status: Set(WorkflowStatus::Active),
            trigger_config: Set(serde_json::json!({"type":"event","event_type":"test"})),
            created_by: Set(None), created_at: Set(now), updated_at: Set(now),
            failure_count: Set(0), auto_disabled_at: Set(None),
            webhook_slug: Set(None), webhook_secret: Set(None),
        }.insert(&db).await.unwrap();
        let step = workflow_step::ActiveModel {
            id: Set(Uuid::new_v4()),
            workflow_id: Set(workflow_id),
            position: Set(1),
            step_type: Set(StepType::Http),
            config: Set(serde_json::json!({"max_retries":10})),
            on_error: Set(OnError::Retry),
            timeout_ms: Set(None),
        }.insert(&db).await.unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let mut engine = WorkflowEngine::new(db.clone());
        engine.register_step(AmbiguousHttp {
            calls: Arc::clone(&calls),
            misclassified_validation_error: false,
        });
        for _ in 0..2 {
            assert!(matches!(
                engine.execute(
                    workflow_id, tenant_id, Some(trigger_id),
                    vec![step.clone()], Value::Null,
                ).await,
                Err(WorkflowError::ExecutionOutcomeUncertain(_))
            ));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let execution = workflow_execution::Entity::find()
            .filter(workflow_execution::Column::TriggerEventId.eq(trigger_id))
            .one(&db).await.unwrap().unwrap();
        assert_eq!(execution.status, ExecutionStatus::ReconciliationRequired);
        let step_record = crate::entities::workflow_step_execution::Entity::find()
            .filter(crate::entities::workflow_step_execution::Column::ExecutionId.eq(execution.id))
            .one(&db).await.unwrap().unwrap();
        assert_eq!(step_record.status, StepExecutionStatus::ReconciliationRequired);

        let validation_trigger = Uuid::new_v4();
        let mut engine = WorkflowEngine::new(db.clone());
        engine.register_step(AmbiguousHttp {
            calls: Arc::clone(&calls),
            misclassified_validation_error: true,
        });
        for _ in 0..2 {
            assert!(matches!(
                engine.execute(
                    workflow_id, tenant_id, Some(validation_trigger),
                    vec![step.clone()], Value::Null,
                ).await,
                Err(WorkflowError::ExecutionOutcomeUncertain(_)),
            ));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2,
            "even a validation-labelled post-send error must not be retried");
        let execution = workflow_execution::Entity::find()
            .filter(workflow_execution::Column::TriggerEventId.eq(validation_trigger))
            .one(&db).await.unwrap().unwrap();
        assert_eq!(execution.status, ExecutionStatus::ReconciliationRequired);
    }
}

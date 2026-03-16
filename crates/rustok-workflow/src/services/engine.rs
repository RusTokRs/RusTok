use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::entities::{
    workflow_execution, workflow_step_execution, ExecutionStatus, OnError, StepExecutionStatus,
    WorkflowExecutionActiveModel, WorkflowStepExecutionActiveModel,
};
use crate::error::WorkflowResult;
use crate::steps::{
    ActionStep, ConditionStep, EmitEventStep, StepContext, WorkflowStep,
};

/// Registry of available step executors, keyed by step type string.
type StepRegistry = HashMap<String, Arc<dyn WorkflowStep>>;

fn default_registry() -> StepRegistry {
    let mut map: StepRegistry = HashMap::new();
    map.insert("action".into(), Arc::new(ActionStep));
    map.insert("emit_event".into(), Arc::new(EmitEventStep));
    map.insert("condition".into(), Arc::new(ConditionStep));
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
        execution.insert(&self.db).await?;

        info!(
            execution_id = %execution_id,
            steps = steps.len(),
            "Starting workflow execution"
        );

        let mut context = StepContext::new(initial_context);
        let mut failed = false;
        let mut failure_msg = String::new();

        'steps: for step in &steps {
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
                        OnError::Skip => continue 'steps,
                        OnError::Retry => {
                            // Retry once (simple implementation)
                            failed = true;
                            failure_msg = format!("Unknown step type: {step_type_str}");
                            break 'steps;
                        }
                    }
                }
            };

            let result = executor.execute(&step.config, context.clone()).await;

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
                    error!(step_id = %step.id, error = %err_msg, "Step failed");

                    self.finish_step_execution(
                        step_execution_id,
                        StepExecutionStatus::Failed,
                        step_input.clone(),
                        Value::Null,
                        Some(err_msg.clone()),
                    )
                    .await?;

                    match step.on_error {
                        OnError::Stop => {
                            failed = true;
                            failure_msg = err_msg;
                            break 'steps;
                        }
                        OnError::Skip => {
                            // Update step status to skipped and continue
                            self.update_step_status(step_execution_id, StepExecutionStatus::Skipped)
                                .await?;
                            continue 'steps;
                        }
                        OnError::Retry => {
                            // Simple single retry
                            match executor.execute(&step.config, context.clone()).await {
                                Ok(out) => {
                                    self.finish_step_execution(
                                        step_execution_id,
                                        StepExecutionStatus::Completed,
                                        step_input,
                                        out.data,
                                        None,
                                    )
                                    .await?;
                                    context = out.context;
                                }
                                Err(retry_err) => {
                                    failed = true;
                                    failure_msg = retry_err.to_string();
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
        }

        // Finalize execution
        let final_status = if failed {
            ExecutionStatus::Failed
        } else {
            ExecutionStatus::Completed
        };

        self.finish_execution(
            execution_id,
            final_status,
            context.data,
            if failed { Some(failure_msg) } else { None },
        )
        .await?;

        info!(execution_id = %execution_id, success = !failed, "Workflow execution finished");

        Ok(execution_id)
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
            .col_expr(
                workflow_step_execution::Column::Output,
                Expr::value(output),
            )
            .col_expr(
                workflow_step_execution::Column::Error,
                Expr::value(error),
            )
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
            .col_expr(
                workflow_execution::Column::Context,
                Expr::value(context),
            )
            .col_expr(
                workflow_execution::Column::Error,
                Expr::value(error),
            )
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

use chrono::{DateTime, Utc};
use sea_orm::{
    entity::prelude::*, ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use crate::context::{ExecutionContext, ExecutionPhase};
use crate::error::{ScriptError, ScriptResult};
use crate::model::ScriptId;
use crate::runner::{ExecutionOutcome, ExecutionResult};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "script_executions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub script_id: Uuid,
    pub script_name: String,
    pub phase: String,
    pub outcome: String,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone)]
pub struct ExecutionLogEntry {
    pub id: Uuid,
    pub script_id: ScriptId,
    pub script_name: String,
    pub phase: ExecutionPhase,
    pub outcome: String,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait ExecutionLogSink: Send + Sync {
    async fn record_result(
        &self,
        result: &ExecutionResult,
        ctx: &ExecutionContext,
    ) -> ScriptResult<()>;
}

pub struct SeaOrmExecutionLog {
    db: DatabaseConnection,
}

impl SeaOrmExecutionLog {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn record(&self, result: &ExecutionResult) -> ScriptResult<()> {
        let outcome_str = match &result.outcome {
            ExecutionOutcome::Success { .. } => "success",
            ExecutionOutcome::Aborted { .. } => "aborted",
            ExecutionOutcome::Failed { .. } => "failed",
        };
        let error_str = match &result.outcome {
            ExecutionOutcome::Aborted { reason } => Some(reason.clone()),
            ExecutionOutcome::Failed { error } => Some(error.to_string()),
            ExecutionOutcome::Success { .. } => None,
        };

        let model = ActiveModel {
            id: ActiveValue::Set(result.execution_id),
            script_id: ActiveValue::Set(result.script_id),
            script_name: ActiveValue::Set(result.script_name.clone()),
            phase: ActiveValue::Set(phase_to_str(result.phase)),
            outcome: ActiveValue::Set(outcome_str.to_string()),
            duration_ms: ActiveValue::Set(result.duration_ms()),
            error: ActiveValue::Set(error_str),
            user_id: ActiveValue::Set(None),
            tenant_id: ActiveValue::Set(None),
            created_at: ActiveValue::Set(result.started_at),
        };

        model
            .insert(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        Ok(())
    }

    pub async fn record_with_context(
        &self,
        result: &ExecutionResult,
        user_id: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> ScriptResult<()> {
        let outcome_str = match &result.outcome {
            ExecutionOutcome::Success { .. } => "success",
            ExecutionOutcome::Aborted { .. } => "aborted",
            ExecutionOutcome::Failed { .. } => "failed",
        };
        let error_str = match &result.outcome {
            ExecutionOutcome::Aborted { reason } => Some(reason.clone()),
            ExecutionOutcome::Failed { error } => Some(error.to_string()),
            ExecutionOutcome::Success { .. } => None,
        };

        let model = ActiveModel {
            id: ActiveValue::Set(result.execution_id),
            script_id: ActiveValue::Set(result.script_id),
            script_name: ActiveValue::Set(result.script_name.clone()),
            phase: ActiveValue::Set(phase_to_str(result.phase)),
            outcome: ActiveValue::Set(outcome_str.to_string()),
            duration_ms: ActiveValue::Set(result.duration_ms()),
            error: ActiveValue::Set(error_str),
            user_id: ActiveValue::Set(user_id),
            tenant_id: ActiveValue::Set(tenant_id),
            created_at: ActiveValue::Set(result.started_at),
        };

        model
            .insert(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        Ok(())
    }
    pub async fn list_for_script(
        &self,
        script_id: ScriptId,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        let models = Entity::find()
            .filter(Column::ScriptId.eq(script_id))
            .order_by_desc(Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        Ok(models.into_iter().map(model_to_entry).collect())
    }

    pub async fn list_recent(&self, limit: u64) -> ScriptResult<Vec<ExecutionLogEntry>> {
        let models = Entity::find()
            .order_by_desc(Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        Ok(models.into_iter().map(model_to_entry).collect())
    }
}

#[async_trait::async_trait]
impl ExecutionLogSink for SeaOrmExecutionLog {
    async fn record_result(
        &self,
        result: &ExecutionResult,
        ctx: &ExecutionContext,
    ) -> ScriptResult<()> {
        let tenant_id = ctx
            .tenant_id
            .as_deref()
            .and_then(|tenant_id| Uuid::parse_str(tenant_id).ok());

        self.record_with_context(result, ctx.user_id.clone(), tenant_id)
            .await
    }
}

fn phase_to_str(phase: ExecutionPhase) -> String {
    match phase {
        ExecutionPhase::Before => "before".to_string(),
        ExecutionPhase::After => "after".to_string(),
        ExecutionPhase::OnCommit => "on_commit".to_string(),
        ExecutionPhase::Manual => "manual".to_string(),
        ExecutionPhase::Scheduled => "scheduled".to_string(),
    }
}

fn str_to_phase(s: &str) -> ExecutionPhase {
    match s {
        "before" => ExecutionPhase::Before,
        "after" => ExecutionPhase::After,
        "on_commit" => ExecutionPhase::OnCommit,
        "scheduled" => ExecutionPhase::Scheduled,
        _ => ExecutionPhase::Manual,
    }
}

fn model_to_entry(model: Model) -> ExecutionLogEntry {
    ExecutionLogEntry {
        id: model.id,
        script_id: model.script_id,
        script_name: model.script_name,
        phase: str_to_phase(&model.phase),
        outcome: model.outcome,
        duration_ms: model.duration_ms,
        error: model.error,
        user_id: model.user_id,
        tenant_id: model.tenant_id,
        created_at: model.created_at,
    }
}

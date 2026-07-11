use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Select, entity::prelude::*,
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
        self.record_with_context(result, None, None).await
    }

    pub async fn record_with_context(
        &self,
        result: &ExecutionResult,
        user_id: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> ScriptResult<()> {
        let (outcome, error) = outcome_fields(&result.outcome);

        let model = ActiveModel {
            id: ActiveValue::Set(result.execution_id),
            script_id: ActiveValue::Set(result.script_id),
            script_name: ActiveValue::Set(result.script_name.clone()),
            phase: ActiveValue::Set(phase_to_str(result.phase)),
            outcome: ActiveValue::Set(outcome.to_string()),
            duration_ms: ActiveValue::Set(result.duration_ms()),
            error: ActiveValue::Set(error),
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
        self.list_for_script_paginated(script_id, 0, limit).await
    }

    pub async fn list_for_script_paginated(
        &self,
        script_id: ScriptId,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_for_script_scoped(script_id, None, offset, limit)
            .await
    }

    pub async fn count_for_script(&self, script_id: ScriptId) -> ScriptResult<u64> {
        self.count_for_script_scoped(script_id, None).await
    }

    pub async fn list_for_script_for_tenant(
        &self,
        script_id: ScriptId,
        tenant_id: Uuid,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_for_script_for_tenant_paginated(script_id, tenant_id, 0, limit)
            .await
    }

    pub async fn list_for_script_for_tenant_paginated(
        &self,
        script_id: ScriptId,
        tenant_id: Uuid,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_for_script_scoped(script_id, Some(tenant_id), offset, limit)
            .await
    }

    pub async fn count_for_script_for_tenant(
        &self,
        script_id: ScriptId,
        tenant_id: Uuid,
    ) -> ScriptResult<u64> {
        self.count_for_script_scoped(script_id, Some(tenant_id))
            .await
    }

    pub async fn list_recent(&self, limit: u64) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_recent_paginated(0, limit).await
    }

    pub async fn list_recent_paginated(
        &self,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_recent_scoped(None, offset, limit).await
    }

    pub async fn count_recent(&self) -> ScriptResult<u64> {
        self.count_recent_scoped(None).await
    }

    pub async fn list_recent_for_tenant(
        &self,
        tenant_id: Uuid,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_recent_for_tenant_paginated(tenant_id, 0, limit)
            .await
    }

    pub async fn list_recent_for_tenant_paginated(
        &self,
        tenant_id: Uuid,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.list_recent_scoped(Some(tenant_id), offset, limit)
            .await
    }

    pub async fn count_recent_for_tenant(&self, tenant_id: Uuid) -> ScriptResult<u64> {
        self.count_recent_scoped(Some(tenant_id)).await
    }

    async fn list_for_script_scoped(
        &self,
        script_id: ScriptId,
        tenant_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        let query = Entity::find().filter(Column::ScriptId.eq(script_id));
        self.fetch_entries(apply_tenant_filter(query, tenant_id), offset, limit)
            .await
    }

    async fn list_recent_scoped(
        &self,
        tenant_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        self.fetch_entries(
            apply_tenant_filter(Entity::find(), tenant_id),
            offset,
            limit,
        )
        .await
    }

    async fn count_for_script_scoped(
        &self,
        script_id: ScriptId,
        tenant_id: Option<Uuid>,
    ) -> ScriptResult<u64> {
        let query = Entity::find().filter(Column::ScriptId.eq(script_id));
        self.count_entries(apply_tenant_filter(query, tenant_id))
            .await
    }

    async fn count_recent_scoped(&self, tenant_id: Option<Uuid>) -> ScriptResult<u64> {
        self.count_entries(apply_tenant_filter(Entity::find(), tenant_id))
            .await
    }

    async fn count_entries(&self, query: Select<Entity>) -> ScriptResult<u64> {
        query
            .count(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))
    }

    async fn fetch_entries(
        &self,
        query: Select<Entity>,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<Vec<ExecutionLogEntry>> {
        let models = query
            .order_by_desc(Column::CreatedAt)
            .offset(offset)
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

fn apply_tenant_filter(query: Select<Entity>, tenant_id: Option<Uuid>) -> Select<Entity> {
    match tenant_id {
        Some(tenant_id) => query.filter(Column::TenantId.eq(tenant_id)),
        None => query,
    }
}

fn outcome_fields(outcome: &ExecutionOutcome) -> (&'static str, Option<String>) {
    match outcome {
        ExecutionOutcome::Success { .. } => ("success", None),
        ExecutionOutcome::Aborted { reason } => ("aborted", Some(reason.clone())),
        ExecutionOutcome::Failed { error } => ("failed", Some(error.to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_log::migration::ScriptExecutionsMigration;
    use sea_orm::Database;
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use std::collections::HashMap;

    async fn execution_log() -> SeaOrmExecutionLog {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory database should connect");
        let manager = SchemaManager::new(&db);
        ScriptExecutionsMigration
            .up(&manager)
            .await
            .expect("script executions migration should apply");
        SeaOrmExecutionLog::new(db)
    }

    fn result_for(
        script_id: Uuid,
        script_name: &str,
        phase: ExecutionPhase,
        outcome: ExecutionOutcome,
        offset_ms: i64,
    ) -> ExecutionResult {
        let started_at = Utc::now() + chrono::Duration::milliseconds(offset_ms);
        ExecutionResult {
            script_id,
            script_name: script_name.to_string(),
            execution_id: Uuid::new_v4(),
            phase,
            started_at,
            finished_at: started_at + chrono::Duration::milliseconds(42),
            outcome,
        }
    }

    #[tokio::test]
    async fn record_result_persists_canonical_context_row() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let result = result_for(
            script_id,
            "contextual_manual",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let ctx = ExecutionContext::new(ExecutionPhase::Manual)
            .with_user("operator-42")
            .with_tenant(tenant_id.to_string());

        log.record_result(&result, &ctx)
            .await
            .expect("contextual execution log row should persist");

        let entries = log
            .list_for_script(script_id, 10)
            .await
            .expect("persisted row should be queryable by script");
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.id, result.execution_id);
        assert_eq!(entry.script_id, script_id);
        assert_eq!(entry.script_name, "contextual_manual");
        assert_eq!(entry.phase, ExecutionPhase::Manual);
        assert_eq!(entry.outcome, "success");
        assert_eq!(entry.duration_ms, 42);
        assert_eq!(entry.error, None);
        assert_eq!(entry.user_id.as_deref(), Some("operator-42"));
        assert_eq!(entry.tenant_id, Some(tenant_id));
        assert_eq!(entry.created_at, result.started_at);
    }

    #[tokio::test]
    async fn record_result_ignores_invalid_tenant_id_but_keeps_user_context() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let result = result_for(
            script_id,
            "invalid_tenant_manual",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let ctx = ExecutionContext::new(ExecutionPhase::Manual)
            .with_user("operator-43")
            .with_tenant("not-a-uuid");

        log.record_result(&result, &ctx)
            .await
            .expect("invalid tenant id should not block execution history");

        let entry = log
            .list_for_script(script_id, 1)
            .await
            .expect("persisted row should be queryable")
            .pop()
            .expect("one execution row should exist");
        assert_eq!(entry.user_id.as_deref(), Some("operator-43"));
        assert_eq!(entry.tenant_id, None);
    }

    #[tokio::test]
    async fn tenant_scoped_history_filters_recent_and_script_rows() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let result_a = result_for(
            script_id,
            "tenant_a_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let result_b = result_for(
            script_id,
            "tenant_b_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            1_000,
        );

        log.record_with_context(&result_a, Some("operator-a".to_string()), Some(tenant_a))
            .await
            .expect("tenant A execution row should persist");
        log.record_with_context(&result_b, Some("operator-b".to_string()), Some(tenant_b))
            .await
            .expect("tenant B execution row should persist");

        let tenant_a_recent = log
            .list_recent_for_tenant(tenant_a, 10)
            .await
            .expect("tenant-scoped recent rows should be queryable");
        assert_eq!(tenant_a_recent.len(), 1);
        assert_eq!(tenant_a_recent[0].id, result_a.execution_id);
        assert_eq!(tenant_a_recent[0].tenant_id, Some(tenant_a));

        let tenant_b_script_rows = log
            .list_for_script_for_tenant(script_id, tenant_b, 10)
            .await
            .expect("tenant-scoped script rows should be queryable");
        assert_eq!(tenant_b_script_rows.len(), 1);
        assert_eq!(tenant_b_script_rows[0].id, result_b.execution_id);
        assert_eq!(tenant_b_script_rows[0].tenant_id, Some(tenant_b));
    }

    #[tokio::test]
    async fn list_recent_orders_rows_by_created_at_desc_and_preserves_failures() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let older = result_for(
            script_id,
            "ordered_history",
            ExecutionPhase::Scheduled,
            ExecutionOutcome::Aborted {
                reason: "guard rejected".to_string(),
            },
            -1_000,
        );
        let newer = result_for(
            Uuid::new_v4(),
            "ordered_history_newer",
            ExecutionPhase::OnCommit,
            ExecutionOutcome::Failed {
                error: ScriptError::Runtime("boom".to_string()),
            },
            1_000,
        );

        log.record(&older)
            .await
            .expect("aborted execution row should persist");
        log.record(&newer)
            .await
            .expect("failed execution row should persist");

        let entries = log
            .list_recent(10)
            .await
            .expect("recent rows should be queryable");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, newer.execution_id);
        assert_eq!(entries[0].phase, ExecutionPhase::OnCommit);
        assert_eq!(entries[0].outcome, "failed");
        assert!(
            entries[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("Runtime error: boom"))
        );
        assert_eq!(entries[1].id, older.execution_id);
        assert_eq!(entries[1].phase, ExecutionPhase::Scheduled);
        assert_eq!(entries[1].outcome, "aborted");
        assert_eq!(entries[1].error.as_deref(), Some("guard rejected"));
    }
    #[tokio::test]
    async fn paginated_history_uses_database_offset_and_limit() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();

        let oldest = result_for(
            script_id,
            "paginated_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let middle = result_for(
            script_id,
            "paginated_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            1_000,
        );
        let newest = result_for(
            script_id,
            "paginated_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            2_000,
        );

        for result in [&oldest, &middle, &newest] {
            log.record(result)
                .await
                .expect("execution row should persist");
        }

        let second_page = log
            .list_for_script_paginated(script_id, 1, 1)
            .await
            .expect("script execution rows should be paginated in storage");
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].id, middle.execution_id);

        let recent_tail = log
            .list_recent_paginated(2, 5)
            .await
            .expect("recent execution rows should support offset pagination");
        assert_eq!(recent_tail.len(), 1);
        assert_eq!(recent_tail[0].id, oldest.execution_id);
    }

    #[tokio::test]
    async fn tenant_paginated_history_stays_scoped_before_offset() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let tenant_a_old = result_for(
            script_id,
            "tenant_paginated_old",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let tenant_b_newer = result_for(
            script_id,
            "tenant_paginated_other",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            1_000,
        );
        let tenant_a_newest = result_for(
            script_id,
            "tenant_paginated_new",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            2_000,
        );

        log.record_with_context(&tenant_a_old, None, Some(tenant_a))
            .await
            .expect("tenant A older row should persist");
        log.record_with_context(&tenant_b_newer, None, Some(tenant_b))
            .await
            .expect("tenant B row should persist");
        log.record_with_context(&tenant_a_newest, None, Some(tenant_a))
            .await
            .expect("tenant A newer row should persist");

        let tenant_a_second = log
            .list_for_script_for_tenant_paginated(script_id, tenant_a, 1, 1)
            .await
            .expect("tenant-scoped script rows should paginate after filtering");
        assert_eq!(tenant_a_second.len(), 1);
        assert_eq!(tenant_a_second[0].id, tenant_a_old.execution_id);
        assert_eq!(tenant_a_second[0].tenant_id, Some(tenant_a));

        let tenant_a_recent = log
            .list_recent_for_tenant_paginated(tenant_a, 0, 2)
            .await
            .expect("tenant-scoped recent rows should paginate after filtering");
        assert_eq!(tenant_a_recent.len(), 2);
        assert_eq!(tenant_a_recent[0].id, tenant_a_newest.execution_id);
        assert_eq!(tenant_a_recent[1].id, tenant_a_old.execution_id);
    }
    #[tokio::test]
    async fn count_helpers_report_canonical_totals_after_scoping() {
        let log = execution_log().await;
        let script_id = Uuid::new_v4();
        let other_script_id = Uuid::new_v4();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        let tenant_a_first = result_for(
            script_id,
            "counted_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            0,
        );
        let tenant_a_second = result_for(
            script_id,
            "counted_history",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            1_000,
        );
        let tenant_b_row = result_for(
            script_id,
            "counted_history_other_tenant",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            2_000,
        );
        let other_script_row = result_for(
            other_script_id,
            "counted_history_other_script",
            ExecutionPhase::Manual,
            ExecutionOutcome::Success {
                return_value: None,
                entity_changes: HashMap::new(),
            },
            3_000,
        );

        log.record_with_context(&tenant_a_first, None, Some(tenant_a))
            .await
            .expect("tenant A first row should persist");
        log.record_with_context(&tenant_a_second, None, Some(tenant_a))
            .await
            .expect("tenant A second row should persist");
        log.record_with_context(&tenant_b_row, None, Some(tenant_b))
            .await
            .expect("tenant B row should persist");
        log.record_with_context(&other_script_row, None, Some(tenant_a))
            .await
            .expect("other script row should persist");

        assert_eq!(log.count_recent().await.expect("recent count"), 4);
        assert_eq!(
            log.count_recent_for_tenant(tenant_a)
                .await
                .expect("tenant recent count"),
            3
        );
        assert_eq!(
            log.count_for_script(script_id).await.expect("script count"),
            3
        );
        assert_eq!(
            log.count_for_script_for_tenant(script_id, tenant_a)
                .await
                .expect("tenant script count"),
            2
        );
    }
}

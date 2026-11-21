//! SeaORM store for persisting module transition checkpoints and retention holds.
//!
//! Provides crash-resilient storage ensuring operations resume idempotently from
//! database checkpoints and retention holds survive server restarts.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ColumnTrait, ConnectionTrait, DeriveEntityModel,
    DerivePrimaryKey, DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter, Set,
    Statement,
};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{
    ConflictFenceSet, GlobalSecurityEpoch, MAX_EXPIRED_RETENTION_HOLDS_PER_REAP,
    ModuleTransitionCheckpoint, ModuleTransitionState, RetentionHoldKind, RetentionHoldLedger,
    RetentionHoldReapReport, RetentionHoldRecord, RetentionTarget,
};

#[derive(Debug, Error)]
pub enum TransitionStoreError {
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Checkpoint not found for operation `{0}`")]
    CheckpointNotFound(Uuid),
    #[error(
        "Checkpoint revision conflict for operation `{operation_id}`: expected `{expected}`, current `{current}`"
    )]
    RevisionConflict {
        operation_id: Uuid,
        expected: u64,
        current: u64,
    },
    #[error("Corrupt stored state data: {0}")]
    CorruptData(String),
    #[error("Retention hold reap limit `{0}` is outside the supported range")]
    InvalidRetentionHoldReapLimit(usize),
}

// ============================================================================
// SeaORM Entity: module_transition_checkpoints
// ============================================================================

pub mod transition_checkpoint_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_transition_checkpoints")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub operation_id: Uuid,
        pub revision: i64,
        pub module_slug: String,
        pub tenant_id: Option<Uuid>,
        pub predecessor_digest: Option<String>,
        pub candidate_digest: String,
        pub state: serde_json::Value,
        pub security_epoch: i64,
        pub fences: serde_json::Value,
        pub recovery_attempt_count: i32,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================================
// SeaORM Entity: module_retention_holds
// ============================================================================

pub mod retention_hold_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "module_retention_holds")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub hold_id: Uuid,
        pub target_type: String,
        pub target_identity: String,
        pub target: serde_json::Value,
        pub kind: serde_json::Value,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ============================================================================
// Transition Checkpoint Store
// ============================================================================

pub struct TransitionCheckpointStore;

impl TransitionCheckpointStore {
    /// Inserts a checkpoint or advances it by exactly one owner revision.
    ///
    /// Production checkpoint creation belongs to the governed transition
    /// service, which acquires the platform slug lock and validates conflicts
    /// in the same transaction. This low-level writer is crate-private to
    /// prevent external callers from bypassing that protocol. Existing rows
    /// are updated with compare-and-swap semantics.
    pub(crate) async fn save_checkpoint<C: ConnectionTrait>(
        db: &C,
        checkpoint: &ModuleTransitionCheckpoint,
    ) -> Result<(), TransitionStoreError> {
        let state_json = serde_json::to_value(&checkpoint.state)?;
        let fences_json = serde_json::to_value(&checkpoint.fences)?;
        let revision = i64::try_from(checkpoint.revision).map_err(|_| {
            TransitionStoreError::CorruptData(
                "Checkpoint revision exceeds the database range".to_string(),
            )
        })?;
        let security_epoch = i64::try_from(checkpoint.security_epoch.value()).map_err(|_| {
            TransitionStoreError::CorruptData(
                "Security epoch exceeds the database range".to_string(),
            )
        })?;
        let recovery_attempt_count =
            i32::try_from(checkpoint.recovery_attempt_count).map_err(|_| {
                TransitionStoreError::CorruptData(
                    "Recovery attempt count exceeds the database range".to_string(),
                )
            })?;

        let model = transition_checkpoint_entity::ActiveModel {
            operation_id: Set(checkpoint.operation_id),
            revision: Set(revision),
            module_slug: Set(checkpoint.module_slug.clone()),
            tenant_id: Set(checkpoint.tenant_id),
            predecessor_digest: Set(checkpoint.predecessor_digest.clone()),
            candidate_digest: Set(checkpoint.candidate_digest.clone()),
            state: Set(state_json.clone()),
            security_epoch: Set(security_epoch),
            fences: Set(fences_json.clone()),
            recovery_attempt_count: Set(recovery_attempt_count),
            created_at: Set(checkpoint.created_at),
            updated_at: Set(checkpoint.updated_at),
        };

        match transition_checkpoint_entity::Entity::find_by_id(checkpoint.operation_id)
            .one(db)
            .await?
        {
            Some(existing) => {
                let current = u64::try_from(existing.revision).map_err(|_| {
                    TransitionStoreError::CorruptData(
                        "Checkpoint revision is outside the supported range".to_string(),
                    )
                })?;
                let expected = checkpoint.revision.checked_sub(1).ok_or_else(|| {
                    TransitionStoreError::CorruptData(
                        "Checkpoint revision must be positive".to_string(),
                    )
                })?;
                if current != expected {
                    return Err(TransitionStoreError::RevisionConflict {
                        operation_id: checkpoint.operation_id,
                        expected,
                        current,
                    });
                }

                let backend = SupportedBackend::new(db.get_database_backend())
                    .map_err(sea_orm::DbErr::from)?;
                let result = db
                    .execute_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
                        format!(
                            "UPDATE module_transition_checkpoints SET revision = {}, state = {}, \
                             security_epoch = {}, fences = {}, recovery_attempt_count = {}, updated_at = {} \
                             WHERE operation_id = {} AND revision = {}",
                            backend.placeholder(1),
                            backend.placeholder(2),
                            backend.placeholder(3),
                            backend.placeholder(4),
                            backend.placeholder(5),
                            backend.placeholder(6),
                            backend.placeholder(7),
                            backend.placeholder(8),
                        ),
                        vec![
                            revision.into(),
                            state_json.into(),
                            security_epoch.into(),
                            fences_json.into(),
                            recovery_attempt_count.into(),
                            checkpoint.updated_at.into(),
                            sea_orm::Value::Uuid(Some(checkpoint.operation_id)),
                            i64::try_from(expected)
                                .map_err(|_| TransitionStoreError::CorruptData(
                                    "Expected checkpoint revision exceeds the database range".to_string(),
                                ))?
                                .into(),
                        ],
                    ))
                    .await?;
                if result.rows_affected() != 1 {
                    let current = Self::load_checkpoint(db, checkpoint.operation_id)
                        .await?
                        .ok_or(TransitionStoreError::CheckpointNotFound(
                            checkpoint.operation_id,
                        ))?
                        .revision;
                    return Err(TransitionStoreError::RevisionConflict {
                        operation_id: checkpoint.operation_id,
                        expected,
                        current,
                    });
                }
            }
            None => {
                model.insert(db).await?;
            }
        }

        Ok(())
    }

    /// Seeds a checkpoint for integration fixtures. This bypasses the owner
    /// admission protocol and must never be enabled in production.
    #[cfg(feature = "test-support")]
    pub async fn save_checkpoint_for_test<C: ConnectionTrait>(
        db: &C,
        checkpoint: &ModuleTransitionCheckpoint,
    ) -> Result<(), TransitionStoreError> {
        Self::save_checkpoint(db, checkpoint).await
    }

    /// Loads a transition checkpoint by operation ID.
    pub async fn load_checkpoint<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<Option<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let model = match transition_checkpoint_entity::Entity::find_by_id(operation_id)
            .one(db)
            .await?
        {
            Some(m) => m,
            None => return Ok(None),
        };

        Ok(Some(checkpoint_from_model(model)?))
    }

    /// Lists all unresolved module transition checkpoints.
    ///
    /// This includes `InterventionRequired`: automatic progression is
    /// terminal in that state, but the checkpoint is not resolved until an
    /// operator completes predecessor recovery (or the transition otherwise
    /// converges), so operator surfaces must still discover it without
    /// already knowing its operation ID.
    pub async fn list_active_checkpoints<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
        let models = transition_checkpoint_entity::Entity::find().all(db).await?;
        active_checkpoints_from_models(models)
    }

    /// Lists unresolved checkpoints for a slug in every scope. Platform
    /// serving changes can affect tenant overlays as well as platform state.
    pub async fn list_active_checkpoints_for_slug<C: ConnectionTrait>(
        db: &C,
        module_slug: &str,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
        ensure_checkpoint_fence_visibility(db).await?;
        let models = transition_checkpoint_entity::Entity::find()
            .filter(transition_checkpoint_entity::Column::ModuleSlug.eq(module_slug))
            .all(db)
            .await?;
        active_checkpoints_from_models(models)
    }

    /// Lists unresolved checkpoints from one exact tenant or platform scope,
    /// including checkpoints waiting for operator intervention.
    pub async fn list_active_checkpoints_for_tenant<C: ConnectionTrait>(
        db: &C,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
        use transition_checkpoint_entity::Column;

        let query = transition_checkpoint_entity::Entity::find();
        let query = match tenant_id {
            Some(tenant_id) => query.filter(checkpoint_tenant_condition(db, tenant_id)),
            None => query.filter(Column::TenantId.is_null()),
        };
        active_checkpoints_from_models(query.all(db).await?)
    }

    /// Lists unresolved checkpoints for one module slug in one tenant scope
    /// **or** the platform scope. This is the tenant-lifecycle fence query:
    /// a platform transition for the slug changes what every tenant
    /// resolves, so it must block tenant lifecycle writes exactly like a
    /// tenant-scoped transition. Scoped to one slug so the fence never
    /// scans (or materializes) other tenants' checkpoints.
    pub async fn list_active_checkpoints_for_slug_and_tenant_or_platform<C: ConnectionTrait>(
        db: &C,
        module_slug: &str,
        tenant_id: Uuid,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
        use transition_checkpoint_entity::Column;

        ensure_checkpoint_fence_visibility(db).await?;
        let query = transition_checkpoint_entity::Entity::find()
            .filter(Column::ModuleSlug.eq(module_slug))
            .filter(
                sea_orm::Condition::any()
                    .add(checkpoint_tenant_condition(db, tenant_id))
                    .add(Column::TenantId.is_null()),
            );
        active_checkpoints_from_models(query.all(db).await?)
    }
}

/// A fence query must see every overlapping checkpoint, including platform
/// rows when the caller is tenant-scoped. A filtered PostgreSQL RLS view
/// cannot prove absence without an audited cross-scope policy. Reading the
/// table in this statement also acquires an AccessShareLock before checking
/// RLS; a fenced caller's transaction retains it until commit, preventing a
/// concurrent policy/row-security DDL change between the check and query.
/// Callers making a mutation must use a transaction and retain the slug lock
/// across this check, the checkpoint query, and the mutation.
async fn ensure_checkpoint_fence_visibility<C: ConnectionTrait>(
    db: &C,
) -> Result<(), TransitionStoreError> {
    if SupportedBackend::new(db.get_database_backend()).map_err(sea_orm::DbErr::from)?
        == SupportedBackend::Postgres
    {
        let row = db.query_one_raw(Statement::from_string(
            sea_orm::DbBackend::Postgres,
            "SELECT row_security_active('module_transition_checkpoints'::regclass) AS enforced \
             FROM (SELECT count(*) FROM module_transition_checkpoints WHERE FALSE) AS locked".to_string(),
        )).await?.ok_or_else(|| sea_orm::DbErr::Custom(
            "could not verify checkpoint row security".to_string(),
        ))?;
        if row.try_get::<bool>("", "enforced")? {
            return Err(sea_orm::DbErr::Custom(
                "checkpoint RLS visibility cannot be proven for the transition fence".to_string(),
            ).into());
        }
    }
    Ok(())
}

// SQLite UUID columns can contain SQLx's native BLOB or historical UUID text.
// Normalize both for the fence lookup; any selected but undecodable row causes
// the query to fail rather than being interpreted as an absent checkpoint.
// PostgreSQL keeps its native indexed UUID comparison.
fn checkpoint_tenant_condition<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
) -> sea_orm::sea_query::Expr {
    use sea_orm::sea_query::Expr;
    use transition_checkpoint_entity::Column;

    match db.get_database_backend() {
        sea_orm::DbBackend::Sqlite => Expr::cust_with_values(
            "replace(replace(replace(replace(lower(CASE WHEN typeof(tenant_id) = 'blob' THEN hex(tenant_id) ELSE CAST(tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') = ?",
            [tenant_id.simple().to_string()],
        ),
        _ => Column::TenantId.eq(tenant_id),
    }
}

fn active_checkpoints_from_models(
    models: Vec<transition_checkpoint_entity::Model>,
) -> Result<Vec<ModuleTransitionCheckpoint>, TransitionStoreError> {
    let mut checkpoints = Vec::new();
    for model in models {
        let checkpoint = checkpoint_from_model(model)?;
        if checkpoint.state.is_unresolved() {
            checkpoints.push(checkpoint);
        }
    }
    Ok(checkpoints)
}

fn checkpoint_from_model(
    model: transition_checkpoint_entity::Model,
) -> Result<ModuleTransitionCheckpoint, TransitionStoreError> {
    let state: ModuleTransitionState = serde_json::from_value(model.state).map_err(|error| {
        TransitionStoreError::CorruptData(format!("Invalid state JSON: {error}"))
    })?;
    let fences: ConflictFenceSet = serde_json::from_value(model.fences).map_err(|error| {
        TransitionStoreError::CorruptData(format!("Invalid fences JSON: {error}"))
    })?;
    let revision = u64::try_from(model.revision).map_err(|_| {
        TransitionStoreError::CorruptData(
            "Checkpoint revision is outside the supported range".to_string(),
        )
    })?;
    let security_epoch = u64::try_from(model.security_epoch).map_err(|_| {
        TransitionStoreError::CorruptData(
            "Security epoch is outside the supported range".to_string(),
        )
    })?;
    let recovery_attempt_count = u32::try_from(model.recovery_attempt_count).map_err(|_| {
        TransitionStoreError::CorruptData(
            "Recovery attempt count is outside the supported range".to_string(),
        )
    })?;

    Ok(ModuleTransitionCheckpoint {
        operation_id: model.operation_id,
        revision,
        module_slug: model.module_slug,
        tenant_id: model.tenant_id,
        predecessor_digest: model.predecessor_digest,
        candidate_digest: model.candidate_digest,
        state,
        security_epoch: GlobalSecurityEpoch(security_epoch),
        fences,
        recovery_attempt_count,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

// ============================================================================
// Retention Hold Store
// ============================================================================

pub struct RetentionHoldStore;

impl RetentionHoldStore {
    fn target_identity_key(target: &RetentionTarget) -> (&'static str, String) {
        target.identity_key()
    }

    /// Persists a new retention hold record to the database.
    pub async fn insert_hold<C: ConnectionTrait>(
        db: &C,
        record: &RetentionHoldRecord,
    ) -> Result<(), TransitionStoreError> {
        let (target_type, target_identity) = Self::target_identity_key(&record.target);
        let target_json = serde_json::to_value(&record.target)?;
        let kind_json = serde_json::to_value(&record.kind)?;

        let model = retention_hold_entity::ActiveModel {
            hold_id: Set(record.hold_id),
            target_type: Set(target_type.to_string()),
            target_identity: Set(target_identity),
            target: Set(target_json),
            kind: Set(kind_json),
            created_at: Set(record.created_at),
        };

        model.insert(db).await?;
        Ok(())
    }

    /// Deletes a retention hold record from the database upon hold release.
    pub async fn delete_hold<C: ConnectionTrait>(
        db: &C,
        hold_id: Uuid,
    ) -> Result<bool, TransitionStoreError> {
        let result = retention_hold_entity::Entity::delete_by_id(hold_id)
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    async fn list_holds<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<RetentionHoldRecord>, TransitionStoreError> {
        let models = retention_hold_entity::Entity::find().all(db).await?;
        let mut records = Vec::with_capacity(models.len());

        for m in models {
            let target: RetentionTarget = serde_json::from_value(m.target).map_err(|e| {
                TransitionStoreError::CorruptData(format!("Invalid target JSON: {e}"))
            })?;
            let kind: RetentionHoldKind = serde_json::from_value(m.kind).map_err(|e| {
                TransitionStoreError::CorruptData(format!("Invalid kind JSON: {e}"))
            })?;

            records.push(RetentionHoldRecord {
                hold_id: m.hold_id,
                target,
                kind,
                created_at: m.created_at,
            });
        }

        Ok(records)
    }

    /// Loads retention holds that still protect their targets now. Expired
    /// rollout-window rows are durable until the reaper deletes them, but
    /// they are not active and must not block collection or projections.
    pub async fn list_active_holds<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<RetentionHoldRecord>, TransitionStoreError> {
        let now = Utc::now();
        Ok(Self::list_holds(db)
            .await?
            .into_iter()
            .filter(|record| record.is_active_at(now))
            .collect())
    }

    /// Loads all active retention holds from the database and constructs a
    /// `RetentionHoldLedger` with the durable hold identities preserved.
    pub async fn load_active_ledger<C: ConnectionTrait>(
        db: &C,
    ) -> Result<RetentionHoldLedger, TransitionStoreError> {
        let records = Self::list_active_holds(db).await?;
        let mut ledger = RetentionHoldLedger::new();
        for record in records {
            ledger.insert_record(record).map_err(|error| {
                TransitionStoreError::CorruptData(format!(
                    "Invalid retention hold ledger record: {error}"
                ))
            })?;
        }
        Ok(ledger)
    }

    /// Deletes a bounded number of expired rollout-window holds. Indefinite
    /// investigation, audit, legal, and standby holds are never reaped by
    /// time; their owners must release them explicitly.
    pub async fn reap_expired_holds<C: ConnectionTrait>(
        db: &C,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<RetentionHoldReapReport, TransitionStoreError> {
        if limit == 0 || limit > MAX_EXPIRED_RETENTION_HOLDS_PER_REAP {
            return Err(TransitionStoreError::InvalidRetentionHoldReapLimit(limit));
        }
        let records = Self::list_holds(db).await?;
        let expired = records
            .iter()
            .filter(|record| record.is_expired_at(now))
            .collect::<Vec<_>>();
        let mut deleted = 0;
        for record in expired.iter().take(limit) {
            if Self::delete_hold(db, record.hold_id).await? {
                deleted += 1;
            }
        }
        Ok(RetentionHoldReapReport {
            scanned: records.len(),
            expired: expired.len(),
            deleted,
        })
    }

    /// Releases all rollout window holds for a given transition operation,
    /// including an elapsed row that the bounded reaper has not deleted yet.
    pub async fn release_holds_for_operation<C: ConnectionTrait>(
        db: &C,
        operation_id: Uuid,
    ) -> Result<u64, TransitionStoreError> {
        let active = Self::list_holds(db).await?;
        let mut released_count = 0;
        for record in active {
            let matches_op = match &record.kind {
                RetentionHoldKind::ActiveRolloutWindow {
                    operation_id: op, ..
                } => *op == operation_id,
                _ => false,
            };
            if matches_op && Self::delete_hold(db, record.hold_id).await? {
                released_count += 1;
            }
        }
        Ok(released_count)
    }
}

#[cfg(test)]
mod fence_visibility_tests {
    use super::*;
    use sea_orm::{Database, DbBackend, TransactionTrait};
    use std::time::Duration;

    // Requires an isolated disposable PostgreSQL database and a role with
    // CREATE ROLE. The schema/role may remain behind if the test panics.
    #[tokio::test]
    #[ignore = "requires disposable RUSTOK_MODULES_TEST_DATABASE_URL, schema and role privileges"]
    async fn postgres_checkpoint_rls_fails_closed_and_policy_ddl_waits_for_fence() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap();
        async fn connect(url: &str) -> sea_orm::DatabaseConnection {
            let mut options = sea_orm::ConnectOptions::new(url);
            options.min_connections(1).max_connections(1).sqlx_logging(false);
            Database::connect(options).await.unwrap()
        }
        let owner = connect(&url).await;
        let worker = connect(&url).await;
        let monitor = connect(&url).await;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema = format!("checkpoint_rls_{suffix}");
        let role = format!("checkpoint_reader_{suffix}");
        owner.execute_unprepared(&format!(
            "CREATE ROLE {role} NOLOGIN; CREATE SCHEMA {schema}; \
             CREATE TABLE {schema}.module_transition_checkpoints (tenant_id UUID, module_slug TEXT); \
             INSERT INTO {schema}.module_transition_checkpoints VALUES (NULL, 'forum'); \
             GRANT USAGE ON SCHEMA {schema} TO {role}; \
             GRANT SELECT ON {schema}.module_transition_checkpoints TO {role}; \
             ALTER TABLE {schema}.module_transition_checkpoints ENABLE ROW LEVEL SECURITY; \
             CREATE POLICY tenant_only ON {schema}.module_transition_checkpoints USING (tenant_id IS NOT NULL)"
        )).await.unwrap();
        worker.execute_unprepared(&format!("SET search_path TO {schema}; SET ROLE {role}")).await.unwrap();
        let visible = worker.query_all_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT * FROM module_transition_checkpoints".to_string())).await.unwrap();
        assert!(visible.is_empty(), "the fixture must hide the platform checkpoint");
        assert!(matches!(
            TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
                &worker, "forum", Uuid::new_v4(),
            ).await,
            Err(TransitionStoreError::Database(_))
        ));
        assert!(matches!(
            TransitionCheckpointStore::list_active_checkpoints_for_slug(&worker, "forum").await,
            Err(TransitionStoreError::Database(_))
        ));
        worker.execute_unprepared("RESET ROLE").await.unwrap();
        owner.execute_unprepared(&format!("ALTER TABLE {schema}.module_transition_checkpoints DISABLE ROW LEVEL SECURITY")).await.unwrap();
        let tx = owner.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        ensure_checkpoint_fence_visibility(&tx).await.unwrap();
        let worker_pid: i32 = worker.query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT pg_backend_pid() AS pid".to_string())).await.unwrap().unwrap().try_get("", "pid").unwrap();
        let ddl_schema = schema.clone();
        let task = tokio::spawn(async move {
            worker.execute_unprepared(&format!(
                "ALTER TABLE {ddl_schema}.module_transition_checkpoints ENABLE ROW LEVEL SECURITY"
            )).await.unwrap();
        });
        let mut blocked = false;
        for _ in 0..200 {
            let row = monitor.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
                "SELECT cardinality(pg_blocking_pids($1)) > 0 AS blocked", vec![worker_pid.into()]
            )).await.unwrap().unwrap();
            if row.try_get::<bool>("", "blocked").unwrap() { blocked = true; break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(blocked, "RLS DDL must wait for the fence transaction");
        tx.commit().await.unwrap();
        task.await.unwrap();
        owner.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE; DROP ROLE {role}")).await.unwrap();
    }
}

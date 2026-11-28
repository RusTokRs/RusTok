use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait, sea_query::Expr,
};

use rustok_core::{Error, Result};

use crate::entity;
use crate::entity::SysEventStatus;

/// Upper bound on the rows one prune run deletes.
///
/// Retention deletes delivered rows in bounded batches so a large backlog is
/// trimmed over several runs instead of holding one long write transaction.
pub const DEFAULT_OUTBOX_RETENTION_BATCH_SIZE: u64 = 500;

/// Retention window and batch size of the delivered-event pruner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutboxRetentionConfig {
    /// How long a delivered event is kept before it may be pruned.
    pub retention: chrono::Duration,
    pub batch_size: u64,
    /// How long a dead-lettered (`failed`) event is kept before it may be
    /// pruned, measured from `created_at`. `None` (the default) keeps the
    /// DLQ forever: pruning it is an explicit operator decision, because a
    /// failed row is the only record that the event never landed. Required
    /// consumer acknowledgements remain mandatory even with this window.
    pub failed_retention: Option<chrono::Duration>,
    /// How long a terminal (`completed`/`failed`) owner-operation receipt
    /// is kept before it may be pruned, measured from `completed_at`.
    /// `None` (the default) keeps receipts forever; once a receipt is
    /// pruned, a late retry with the same idempotency key runs again.
    pub receipt_retention: Option<chrono::Duration>,
}

impl Default for OutboxRetentionConfig {
    fn default() -> Self {
        Self {
            retention: chrono::Duration::days(30),
            batch_size: DEFAULT_OUTBOX_RETENTION_BATCH_SIZE,
            failed_retention: None,
            receipt_retention: None,
        }
    }
}

impl OutboxRetentionConfig {
    /// Rejects a configuration that would either delete too eagerly or never
    /// finish trimming the table.
    pub fn validate(&self) -> Result<()> {
        if self.retention <= chrono::Duration::zero() {
            return Err(Error::Validation(
                "outbox retention window must be positive".to_string(),
            ));
        }
        if let Some(failed_retention) = self.failed_retention
            && failed_retention <= chrono::Duration::zero()
        {
            return Err(Error::Validation(
                "outbox failed-event retention window must be positive".to_string(),
            ));
        }
        if let Some(receipt_retention) = self.receipt_retention
            && receipt_retention <= chrono::Duration::zero()
        {
            return Err(Error::Validation(
                "outbox operation-receipt retention window must be positive".to_string(),
            ));
        }
        for window in [Some(self.retention), self.failed_retention].into_iter().flatten() {
            if window != chrono::Duration::milliseconds(window.num_milliseconds()) {
                return Err(Error::Validation("outbox event retention windows must use whole milliseconds".into()));
            }
        }
        let maximum = chrono::Duration::days(3650);
        if self.retention > maximum
            || self.failed_retention.is_some_and(|window| window > maximum)
            || self.receipt_retention.is_some_and(|window| window > maximum)
        {
            return Err(Error::Validation("outbox retention windows must not exceed 3650 days".to_string()));
        }
        if !(1..=rustok_outbox_storage::MAX_RETENTION_BATCH_SIZE).contains(&self.batch_size) {
            return Err(Error::Validation(format!(
                "outbox retention batch size must be between 1 and {}",
                rustok_outbox_storage::MAX_RETENTION_BATCH_SIZE
            )));
        }
        Ok(())
    }
}

/// Result of one bounded prune run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutboxPruneReport {
    /// Delivered rows deleted by this run.
    pub pruned: u64,
    /// Dead-lettered rows deleted by this run (only when a failed-event
    /// retention window is configured).
    pub pruned_failed: u64,
    /// Terminal owner-operation receipts deleted by this run (only when a
    /// receipt retention window is configured).
    pub pruned_receipts: u64,
    /// Whether the run filled its batch, i.e. another run has work to do.
    pub batch_exhausted: bool,
}

/// Bounded retention for `sys_events`.
///
/// The outbox is both the delivery queue and the durable record of what was
/// delivered. By default only aged dispatched rows are eligible; an explicit
/// failed-event window also permits aged DLQ rows. Every event deletion
/// requires all registered durable consumers to have acknowledged it.
/// Pending rows are never pruned. A dispatched row without `dispatched_at` is left alone,
/// so the pruner can never delete a row whose delivery age is unknown.
#[derive(Clone)]
pub struct OutboxRetention {
    db: DatabaseConnection,
    config: OutboxRetentionConfig,
}

impl OutboxRetention {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            config: OutboxRetentionConfig::default(),
        }
    }

    pub fn with_config(mut self, config: OutboxRetentionConfig) -> Self {
        self.config = config;
        self
    }

    pub fn config(&self) -> OutboxRetentionConfig {
        self.config
    }

    /// Deletes up to `batch_size` delivered rows older than the retention
    /// window and reports how much was pruned.
    ///
    /// Candidate ids are selected first and deleted by id with the same
    /// status, age, and consumer-acknowledgement predicates. The delete rechecks
    /// the consumer predicate so a newly registered obligation or changed
    /// delivery state cannot be bypassed by a stale candidate scan.
    pub async fn prune_once(&self) -> Result<OutboxPruneReport> {
        self.config.validate()?;
        self.verify_event_retention_policy().await?;
        let cutoff = rustok_outbox_storage::observe_database_clock(&self.db).await?
            .checked_sub_signed(self.config.retention)
            .ok_or_else(|| Error::Validation("outbox retention cutoff is out of range".to_string()))?;

        let candidate_ids: Vec<uuid::Uuid> = entity::Entity::find()
            .select_only()
            .column(entity::Column::Id)
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::ClaimedBy.is_null())
            .filter(entity::Column::ClaimedAt.is_null())
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .filter(required_consumers_acknowledged_predicate())
            .filter(no_open_dispatch_attempt_predicate())
            .order_by_asc(entity::Column::DispatchedAt)
            .order_by_asc(entity::Column::Id)
            .limit(self.config.batch_size)
            .into_tuple::<uuid::Uuid>()
            .all(&self.db)
            .await?;

        if candidate_ids.is_empty() {
            let (pruned_failed, pruned_receipts, secondary_exhausted) =
                self.prune_secondary().await?;
            rustok_telemetry::metrics::record_outbox_retention_run();
            return Ok(OutboxPruneReport {
                pruned: 0,
                pruned_failed,
                pruned_receipts,
                batch_exhausted: secondary_exhausted,
            });
        }

        let batch_exhausted = candidate_ids.len() as u64 >= self.config.batch_size;
        let txn = self.begin_event_delete().await?;
        let cutoff = cutoff.min(rustok_outbox_storage::database_now(&txn).await?
            .checked_sub_signed(self.config.retention)
            .ok_or_else(|| Error::Validation("outbox retention cutoff is out of range".to_string()))?);
        let deleted = entity::Entity::delete_many()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::ClaimedBy.is_null())
            .filter(entity::Column::ClaimedAt.is_null())
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .filter(required_consumers_acknowledged_predicate())
            .filter(no_open_dispatch_attempt_predicate())
            .exec(&txn)
            .await?
            .rows_affected;
        txn.commit().await?;

        if deleted > 0 {
            rustok_telemetry::metrics::record_outbox_pruned(deleted);
            tracing::info!(
                pruned_events = deleted,
                batch_exhausted,
                retention_seconds = self.config.retention.num_seconds(),
                "Pruned delivered outbox events"
            );
        }
        let (pruned_failed, pruned_receipts, secondary_exhausted) = self.prune_secondary().await?;
        rustok_telemetry::metrics::record_outbox_retention_run();

        Ok(OutboxPruneReport {
            pruned: deleted,
            pruned_failed,
            pruned_receipts,
            batch_exhausted: batch_exhausted || secondary_exhausted,
        })
    }

    // The first pruner binds the policy, even on an empty queue. Recheck
    // under the same row lock before every destructive event transaction.
    async fn verify_event_retention_policy(&self) -> Result<()> {
        let txn = self.db.begin().await?;
        if txn.get_database_backend() == DatabaseBackend::Postgres {
            txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").await?;
        }
        self.lock_event_retention_policy(&txn).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn lock_event_retention_policy(&self, txn: &DatabaseTransaction) -> Result<()> {
        let backend = txn.get_database_backend();
        let locked = txn.execute_unprepared(
            "UPDATE outbox_event_retention_policy SET delivered_retention_ms = delivered_retention_ms WHERE id = 1"
        ).await?;
        if locked.rows_affected() != 1 {
            return Err(Error::External("outbox event retention policy is missing".into()));
        }
        let read = || async {
            let row = txn.query_one_raw(Statement::from_string(backend,
                "SELECT delivered_retention_ms, failed_retention_ms FROM outbox_event_retention_policy WHERE id = 1",
            )).await?.ok_or_else(|| Error::External("outbox event retention policy is missing".into()))?;
            let delivered: Option<i64> = row.try_get("", "delivered_retention_ms")?;
            let failed: Option<i64> = row.try_get("", "failed_retention_ms")?;
            Ok::<_, Error>((delivered, failed))
        };
        let expected = (Some(self.config.retention.num_milliseconds()),
            self.config.failed_retention.map(|window| window.num_milliseconds()));
        match read().await? {
            (None, None) => {
                let sql = match backend {
                    DatabaseBackend::Postgres => "UPDATE outbox_event_retention_policy SET delivered_retention_ms = $1, failed_retention_ms = $2 WHERE id = 1 AND delivered_retention_ms IS NULL",
                    DatabaseBackend::Sqlite => "UPDATE outbox_event_retention_policy SET delivered_retention_ms = ?, failed_retention_ms = ? WHERE id = 1 AND delivered_retention_ms IS NULL",
                    _ => return Err(Error::Validation("unsupported outbox retention backend".into())),
                };
                let changed = txn.execute_raw(Statement::from_sql_and_values(backend, sql,
                    [expected.0.into(), expected.1.into()])).await?;
                if changed.rows_affected() != 1 || read().await? != expected {
                    return Err(Error::External("outbox event retention policy binding failed".into()));
                }
            }
            stored if stored == expected => {}
            _ => return Err(Error::Validation("outbox event retention windows differ from durable policy; coordinated reconfiguration required".into())),
        }
        Ok(())
    }

    // PostgreSQL's statement snapshot does not see an uncommitted new
    // subscription. A SHARE table lock conflicts with the registry writer's
    // ROW EXCLUSIVE lock, including inserts of previously absent keys. Take
    // it before the final DELETE, then use a fresh READ COMMITTED snapshot.
    // SQLite reserves its writer before the final clock check and DELETE.
    async fn begin_event_delete(&self) -> Result<DatabaseTransaction> {
        let txn = self.db.begin().await?;
        if self.db.get_database_backend() == DatabaseBackend::Postgres {
            txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .await?;
        }
        // Policy -> consumer registry -> event rows.
        self.lock_event_retention_policy(&txn).await?;
        if self.db.get_database_backend() == DatabaseBackend::Postgres {
            txn.execute_unprepared("LOCK TABLE sys_event_consumers IN SHARE MODE")
                .await?;
        } else if self.db.get_database_backend() == DatabaseBackend::Sqlite {
            txn.execute_unprepared("UPDATE sys_events SET claimed_at = claimed_at WHERE 1 = 0")
                .await?;
        }
        Ok(txn)
    }

    /// Prunes the two bounded histories that are disabled by default:
    /// dead-lettered events past their configured window and terminal
    /// owner-operation receipts past theirs.
    async fn prune_secondary(&self) -> Result<(u64, u64, bool)> {
        let mut batch_exhausted = false;
        let mut pruned_failed = 0;
        if let Some(failed_retention) = self.config.failed_retention {
            let cutoff = rustok_outbox_storage::database_now(&self.db).await?
                .checked_sub_signed(failed_retention)
                .ok_or_else(|| Error::Validation("outbox failed retention cutoff is out of range".to_string()))?;
            let candidate_ids: Vec<uuid::Uuid> = entity::Entity::find()
                .select_only()
                .column(entity::Column::Id)
                .filter(entity::Column::Status.eq(SysEventStatus::Failed))
                .filter(entity::Column::ClaimedBy.is_null())
                .filter(entity::Column::ClaimedAt.is_null())
                .filter(entity::Column::CreatedAt.lte(cutoff))
                .filter(required_consumers_acknowledged_predicate())
            .filter(no_open_dispatch_attempt_predicate())
                .order_by_asc(entity::Column::CreatedAt)
                .order_by_asc(entity::Column::Id)
                .limit(self.config.batch_size)
                .into_tuple::<uuid::Uuid>()
                .all(&self.db)
                .await?;
            if candidate_ids.len() as u64 >= self.config.batch_size {
                batch_exhausted = true;
            }
            if !candidate_ids.is_empty() {
                let txn = self.begin_event_delete().await?;
                let cutoff = cutoff.min(rustok_outbox_storage::database_now(&txn).await?
                    .checked_sub_signed(failed_retention)
                    .ok_or_else(|| Error::Validation("outbox failed retention cutoff is out of range".to_string()))?);
                pruned_failed = entity::Entity::delete_many()
                    .filter(entity::Column::Id.is_in(candidate_ids))
                    .filter(entity::Column::Status.eq(SysEventStatus::Failed))
                .filter(entity::Column::ClaimedBy.is_null())
                .filter(entity::Column::ClaimedAt.is_null())
                    .filter(entity::Column::CreatedAt.lte(cutoff))
                    .filter(required_consumers_acknowledged_predicate())
            .filter(no_open_dispatch_attempt_predicate())
                    .exec(&txn)
                    .await?
                    .rows_affected;
                txn.commit().await?;
                if pruned_failed > 0 {
                    tracing::info!(
                        pruned_failed_events = pruned_failed,
                        retention_seconds = failed_retention.num_seconds(),
                        "Pruned dead-lettered outbox events"
                    );
                }
            }
        }

        let mut pruned_receipts = 0;
        if let Some(receipt_retention) = self.config.receipt_retention {
            pruned_receipts = rustok_outbox_storage::idempotency::prune_terminal_receipts(
                &self.db,
                receipt_retention,
                self.config.batch_size,
            )
            .await
            .map_err(|error| Error::External(error.to_string()))?;
            if pruned_receipts > 0 {
                tracing::info!(
                    pruned_receipts,
                    retention_seconds = receipt_retention.num_seconds(),
                    "Pruned terminal owner-operation receipts"
                );
            }
            if pruned_receipts >= self.config.batch_size {
                batch_exhausted = true;
            }
        }

        Ok((pruned_failed, pruned_receipts, batch_exhausted))
    }
}

/// Returns the correlated predicate shared by the retention candidate scan
/// and the final delete, so a stale candidate cannot bypass a consumer hold.
fn no_open_dispatch_attempt_predicate() -> Expr {
    Expr::cust("NOT EXISTS (SELECT 1 FROM outbox_dispatch_attempts WHERE event_id = sys_events.id AND state = 'open')")
}

fn required_consumers_acknowledged_predicate() -> Expr {
    Expr::cust(
        r#"
NOT EXISTS (
    SELECT 1
    FROM sys_event_consumers AS required_consumer
    WHERE required_consumer.event_type = sys_events.event_type
      AND NOT EXISTS (
          SELECT 1
          FROM sys_event_consumer_receipts AS consumer_receipt
          WHERE consumer_receipt.outbox_event_id = sys_events.id
            AND consumer_receipt.consumer_key = required_consumer.consumer_key
            AND consumer_receipt.event_type = required_consumer.event_type
      )
)
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::{OutboxRetention, OutboxRetentionConfig};
    use crate::entity::SysEventStatus;
    use chrono::Utc;
    use rustok_core::generate_id;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Set};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use std::sync::Arc;

    async fn test_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        crate::migration::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("sys_events schema");
        db
    }

    async fn insert_event(
        db: &sea_orm::DatabaseConnection,
        status: SysEventStatus,
    ) -> uuid::Uuid {
        insert_event_at(db, status, Utc::now()).await
    }

    async fn insert_event_at(
        db: &sea_orm::DatabaseConnection,
        status: SysEventStatus,
        created_at: chrono::DateTime<Utc>,
    ) -> uuid::Uuid {
        let id = generate_id();
        let tenant_id = generate_id();
        crate::entity::ActiveModel {
            id: Set(id),
            tenant_id: Set(Some(tenant_id)),
            event_type: Set("test.pruned".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({"tenant_id": tenant_id.to_string()})),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(created_at),
            dispatched_at: Set(None),
        }
        .insert(db)
        .await
        .expect("insert pristine pending sys_event");

        match status.clone() {
            SysEventStatus::Pending => {}
            SysEventStatus::Dispatched => {
                crate::OutboxTransport::new(db.clone())
                    .acknowledge(id).await.expect("acknowledge fixture event");
            }
            SysEventStatus::Failed => {
                let relay = crate::OutboxRelay::new(
                    db.clone(),
                    Arc::new(crate::OutboxTransport::new(db.clone())),
                );
                assert_eq!(relay.process_pending_once(Some(1)).await.expect("DLQ fixture relay"), 1);
            }
        }
        let stored = crate::entity::Entity::find_by_id(id).one(db).await
            .expect("read fixture event").expect("fixture exists");
        assert_eq!(stored.status, status);
        id
    }

    #[tokio::test]
    async fn prune_deletes_only_delivered_rows_older_than_the_window() {
        let db = test_db().await;
        let stale_created_at = Utc::now() - chrono::Duration::days(40);
        let failed = insert_event_at(&db, SysEventStatus::Failed, stale_created_at).await;
        let old_dispatched = insert_event(&db, SysEventStatus::Dispatched).await;
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        let fresh_dispatched = insert_event(&db, SysEventStatus::Dispatched).await;
        let pending = insert_event(&db, SysEventStatus::Pending).await;

        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(100), ..Default::default()
        });
        let report = retention.prune_once().await.expect("prune succeeds");

        assert_eq!(report.pruned, 1);
        assert!(!report.batch_exhausted);
        assert!(
            crate::entity::Entity::find_by_id(old_dispatched)
                .one(&db)
                .await
                .expect("query")
                .is_none()
        );
        for survivor in [fresh_dispatched, pending, failed] {
            assert!(
                crate::entity::Entity::find_by_id(survivor)
                    .one(&db)
                    .await
                    .expect("query")
                    .is_some(),
                "row {survivor} must survive retention"
            );
        }
    }

    #[tokio::test]
    async fn registered_consumers_hold_old_events_until_every_consumer_acknowledges() {
        let db = test_db().await;
        let event_id = insert_event(&db, SysEventStatus::Dispatched).await;
        let event_type = "test.pruned".to_string();
        let first = crate::OutboxConsumer::new(db.clone());
        let second = crate::OutboxConsumer::new(db.clone());
        first
            .register_required_event_types("consumer.one", std::slice::from_ref(&event_type))
            .await
            .expect("register first consumer");
        second
            .register_required_event_types("consumer.two", std::slice::from_ref(&event_type))
            .await
            .expect("register second consumer");

        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(1), ..Default::default()
        });
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        assert_eq!(retention.prune_once().await.expect("held prune").pruned, 0);

        first
            .acknowledge_existing("consumer.one", event_id)
            .await
            .expect("first consumer acknowledgement");
        assert_eq!(
            retention
                .prune_once()
                .await
                .expect("partially acknowledged prune")
                .pruned,
            0
        );

        second
            .acknowledge_existing("consumer.two", event_id)
            .await
            .expect("second consumer acknowledgement");
        assert_eq!(
            retention
                .prune_once()
                .await
                .expect("fully acknowledged prune")
                .pruned,
            1
        );
        assert!(
            crate::entity::Entity::find_by_id(event_id)
                .one(&db)
                .await
                .expect("query")
                .is_none(),
            "the canonical event becomes eligible only after every required consumer acknowledges"
        );
    }

    #[tokio::test]
    async fn failed_rows_are_pruned_only_when_a_failed_window_is_configured() {
        let db = test_db().await;
        let stale = Utc::now() - chrono::Duration::days(40);

        // Default configuration never touches the DLQ.
        // Failed retention is measured from immutable creation time.
        let failed = insert_event_at(&db, SysEventStatus::Failed, stale).await;
        let retention = OutboxRetention::new(db.clone());
        let report = retention.prune_once().await.expect("default prune");
        assert_eq!(report.pruned_failed, 0);
        assert!(
            crate::entity::Entity::find_by_id(failed)
                .one(&db)
                .await
                .expect("query")
                .is_some()
        );

        // A conflicting worker cannot silently enable DLQ pruning.
        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            failed_retention: Some(chrono::Duration::days(30)),
            ..Default::default()
        });
        assert!(retention.prune_once().await.is_err());
        assert!(db.execute_unprepared("DELETE FROM sys_events").await.is_err());
        // Explicit coordinated operator reconfiguration of the bound policy.
        db.execute_unprepared("UPDATE outbox_event_retention_policy SET failed_retention_ms = 2592000000 WHERE id = 1")
            .await.expect("enable DLQ retention");
        assert!(OutboxRetention::new(db.clone()).prune_once().await.is_err());
        let consumer = crate::OutboxConsumer::new(db.clone());
        consumer
            .register_required_event_types("consumer.dlq", &["test.pruned".to_string()])
            .await
            .expect("register durable consumer");
        let held = retention.prune_once().await.expect("held failed prune");
        assert_eq!(held.pruned_failed, 0);
        assert!(!held.batch_exhausted);
        assert!(crate::entity::Entity::find_by_id(failed)
            .one(&db).await.expect("query").is_some());
        consumer
            .acknowledge_existing("consumer.dlq", failed)
            .await
            .expect("terminal consumer receipt");
        let report = retention.prune_once().await.expect("failed prune");
        assert_eq!(report.pruned_failed, 1);
        assert!(
            crate::entity::Entity::find_by_id(failed)
                .one(&db)
                .await
                .expect("query")
                .is_none()
        );
    }

    #[tokio::test]
    async fn prune_bounds_each_run_and_reports_the_remaining_batch() {
        let db = test_db().await;
        for _ in 0..3 {
            insert_event(&db, SysEventStatus::Dispatched).await;
        }
        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::milliseconds(1),
            batch_size: 2,
            ..Default::default()
        });
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let first = retention.prune_once().await.expect("first prune");
        assert_eq!(first.pruned, 2);
        assert!(first.batch_exhausted);

        let second = retention.prune_once().await.expect("second prune");
        assert_eq!(second.pruned, 1);
        assert!(!second.batch_exhausted);

        let third = retention.prune_once().await.expect("third prune");
        assert_eq!(third.pruned, 0);
    }

    #[tokio::test]
    async fn invalid_configuration_is_rejected_before_any_delete() {
        let db = test_db().await;
        let survivor = insert_event(&db, SysEventStatus::Dispatched).await;

        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::zero(),
            batch_size: 10,
            ..Default::default()
        });
        assert!(retention.prune_once().await.is_err());
        assert!(
            crate::entity::Entity::find_by_id(survivor)
                .one(&db)
                .await
                .expect("query")
                .is_some()
        );
    }

    #[test]
    fn retention_batch_size_has_a_hard_upper_bound() {
        let mut config = OutboxRetentionConfig::default();
        config.batch_size = rustok_outbox_storage::MAX_RETENTION_BATCH_SIZE;
        assert!(config.validate().is_ok());
        config.batch_size += 1;
        assert!(config.validate().is_err());
        config.batch_size = 0;
        assert!(config.validate().is_err());
    }

    // The table shape a pruner depends on: the migration must create the index
    // the retention scan reads through, otherwise every run sorts `sys_events`.
    #[tokio::test]
    async fn migration_creates_the_retention_index() {
        let db = test_db().await;
        let indexes: Vec<String> = db
            .query_all_raw(sea_orm::Statement::from_string(
                db.get_database_backend(),
                "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'sys_events'"
                    .to_string(),
            ))
            .await
            .expect("index list")
            .into_iter()
            .filter_map(|row| row.try_get::<String>("", "name").ok())
            .collect();
        assert!(
            indexes
                .iter()
                .any(|name| name == "idx_sys_events_pending_created_at"),
            "claim index missing from {indexes:?}"
        );
        assert!(
            indexes
                .iter()
                .any(|name| name == "idx_sys_events_dispatched_at"),
            "retention index missing from {indexes:?}"
        );
    }
}

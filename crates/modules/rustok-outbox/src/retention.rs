use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::Expr,
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
        let maximum = chrono::Duration::days(3650);
        if self.retention > maximum
            || self.failed_retention.is_some_and(|window| window > maximum)
            || self.receipt_retention.is_some_and(|window| window > maximum)
        {
            return Err(Error::Validation("outbox retention windows must not exceed 3650 days".to_string()));
        }
        if self.batch_size == 0 {
            return Err(Error::Validation(
                "outbox retention batch size must be greater than zero".to_string(),
            ));
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
        let cutoff = rustok_outbox_storage::database_now(&self.db).await?
            .checked_sub_signed(self.config.retention)
            .ok_or_else(|| Error::Validation("outbox retention cutoff is out of range".to_string()))?;

        let candidate_ids: Vec<uuid::Uuid> = entity::Entity::find()
            .select_only()
            .column(entity::Column::Id)
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .filter(required_consumers_acknowledged_predicate())
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
        let deleted = entity::Entity::delete_many()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .filter(required_consumers_acknowledged_predicate())
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

    // PostgreSQL's statement snapshot does not see an uncommitted new
    // subscription. A SHARE table lock conflicts with the registry writer's
    // ROW EXCLUSIVE lock, including inserts of previously absent keys. Take
    // it before the final DELETE, then use a fresh READ COMMITTED snapshot.
    // SQLite serializes writers; the DELETE is the first data statement.
    async fn begin_event_delete(&self) -> Result<DatabaseTransaction> {
        let txn = self.db.begin().await?;
        if self.db.get_database_backend() == DatabaseBackend::Postgres {
            txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .await?;
            txn.execute_unprepared("LOCK TABLE sys_event_consumers IN SHARE MODE")
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
                .filter(entity::Column::CreatedAt.lte(cutoff))
                .filter(required_consumers_acknowledged_predicate())
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
                pruned_failed = entity::Entity::delete_many()
                    .filter(entity::Column::Id.is_in(candidate_ids))
                    .filter(entity::Column::Status.eq(SysEventStatus::Failed))
                    .filter(entity::Column::CreatedAt.lte(cutoff))
                    .filter(required_consumers_acknowledged_predicate())
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
        dispatched_at: Option<chrono::DateTime<Utc>>,
    ) -> uuid::Uuid {
        let id = generate_id();
        let tenant_id = generate_id();
        crate::entity::ActiveModel {
            id: Set(id),
            tenant_id: Set(Some(tenant_id)),
            event_type: Set("test.pruned".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({"tenant_id": tenant_id.to_string()})),
            status: Set(status),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now()),
            dispatched_at: Set(dispatched_at),
        }
        .insert(db)
        .await
        .expect("insert sys_event");
        id
    }

    #[tokio::test]
    async fn prune_deletes_only_delivered_rows_older_than_the_window() {
        let db = test_db().await;
        let stale = Utc::now() - chrono::Duration::days(40);
        let fresh = Utc::now() - chrono::Duration::hours(1);

        let old_dispatched = insert_event(&db, SysEventStatus::Dispatched, Some(stale)).await;
        let fresh_dispatched = insert_event(&db, SysEventStatus::Dispatched, Some(fresh)).await;
        let undated_dispatched = insert_event(&db, SysEventStatus::Dispatched, None).await;
        let pending = insert_event(&db, SysEventStatus::Pending, None).await;
        let failed = insert_event(&db, SysEventStatus::Failed, Some(stale)).await;

        let retention = OutboxRetention::new(db.clone());
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
        for survivor in [fresh_dispatched, undated_dispatched, pending, failed] {
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
        let stale = Utc::now() - chrono::Duration::days(40);
        let event_id = insert_event(&db, SysEventStatus::Dispatched, Some(stale)).await;
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

        let retention = OutboxRetention::new(db.clone());
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
        let failed = insert_event(&db, SysEventStatus::Failed, Some(stale)).await;
        // Failed retention is measured from creation; age the row.
        let mut aged: crate::entity::ActiveModel = crate::entity::Entity::find_by_id(failed)
            .one(&db)
            .await
            .expect("query")
            .expect("failed event")
            .into();
        aged.created_at = Set(stale);
        aged.update(&db).await.expect("age failed event");
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

        // With an explicit operator window, aged failed rows are pruned.
        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            failed_retention: Some(chrono::Duration::days(30)),
            ..Default::default()
        });
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
        let stale = Utc::now() - chrono::Duration::days(40);
        for _ in 0..3 {
            insert_event(&db, SysEventStatus::Dispatched, Some(stale)).await;
        }

        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::days(30),
            batch_size: 2,
            ..Default::default()
        });
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
        let stale = Utc::now() - chrono::Duration::days(40);
        let survivor = insert_event(&db, SysEventStatus::Dispatched, Some(stale)).await;

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

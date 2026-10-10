use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

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
}

impl Default for OutboxRetentionConfig {
    fn default() -> Self {
        Self {
            retention: chrono::Duration::days(30),
            batch_size: DEFAULT_OUTBOX_RETENTION_BATCH_SIZE,
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
    /// Whether the run filled its batch, i.e. another run has work to do.
    pub batch_exhausted: bool,
}

/// Bounded retention for `sys_events`.
///
/// The outbox is both the delivery queue and the durable record of what was
/// delivered, so retention only ever deletes rows that reached the transport
/// (`dispatched` with a non-null `dispatched_at` older than the window).
/// Undelivered (`pending`) and dead-lettered (`failed`) rows are never touched:
/// the first still owe a delivery, the second are the DLQ an operator works
/// through. A delivered row that lost its `dispatched_at` is left alone as well,
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
    /// The candidate ids are selected first and deleted by id with the same
    /// status/age predicates, so a row that changed status between the two
    /// statements (a DLQ replay, for instance) is not deleted.
    pub async fn prune_once(&self) -> Result<OutboxPruneReport> {
        self.config.validate()?;
        let cutoff = Utc::now() - self.config.retention;

        let candidate_ids: Vec<uuid::Uuid> = entity::Entity::find()
            .select_only()
            .column(entity::Column::Id)
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .order_by_asc(entity::Column::DispatchedAt)
            .limit(self.config.batch_size)
            .into_tuple::<uuid::Uuid>()
            .all(&self.db)
            .await?;

        if candidate_ids.is_empty() {
            rustok_telemetry::metrics::record_outbox_retention_run();
            return Ok(OutboxPruneReport::default());
        }

        let batch_exhausted = candidate_ids.len() as u64 >= self.config.batch_size;
        let deleted = entity::Entity::delete_many()
            .filter(entity::Column::Id.is_in(candidate_ids))
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::DispatchedAt.is_not_null())
            .filter(entity::Column::DispatchedAt.lte(cutoff))
            .exec(&self.db)
            .await?
            .rows_affected;

        if deleted > 0 {
            rustok_telemetry::metrics::record_outbox_pruned(deleted);
            tracing::info!(
                pruned_events = deleted,
                batch_exhausted,
                retention_seconds = self.config.retention.num_seconds(),
                "Pruned delivered outbox events"
            );
        }
        rustok_telemetry::metrics::record_outbox_retention_run();

        Ok(OutboxPruneReport {
            pruned: deleted,
            batch_exhausted,
        })
    }
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
        crate::entity::ActiveModel {
            id: Set(id),
            event_type: Set("test.pruned".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({"tenant_id": generate_id().to_string()})),
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
    async fn prune_bounds_each_run_and_reports_the_remaining_batch() {
        let db = test_db().await;
        let stale = Utc::now() - chrono::Duration::days(40);
        for _ in 0..3 {
            insert_event(&db, SysEventStatus::Dispatched, Some(stale)).await;
        }

        let retention = OutboxRetention::new(db.clone()).with_config(OutboxRetentionConfig {
            retention: chrono::Duration::days(30),
            batch_size: 2,
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

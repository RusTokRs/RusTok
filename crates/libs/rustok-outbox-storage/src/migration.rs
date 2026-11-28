use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct SysEventsMigration;

#[async_trait::async_trait]
impl MigrationTrait for SysEventsMigration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let nested = SchemaManager::new(&txn);
        let manager = &nested;
        create_sys_events_schema(manager).await?;
        create_sys_events_claim_index(manager).await?;
        create_sys_events_retention_index(manager).await?;
        add_sys_events_tenant_column(manager).await?;
        create_sys_events_tenant_index(manager).await?;
        add_sys_events_integrity_checks(manager).await?;
        create_owner_operation_receipts_table(manager).await?;
        create_sys_event_consumer_tables(manager).await?;
        create_outbox_temporal_clock(manager).await?;
        create_outbox_execution_policy(manager).await?;
        create_outbox_execution_policy_integrity(manager).await?;
        create_outbox_temporal_clock_integrity(manager).await?;
        create_outbox_dispatch_attempts(manager).await?;
        create_outbox_delivery_checkpoints(manager).await?;
        create_outbox_delivery_history_integrity(manager).await?;
        create_outbox_event_envelope_integrity(manager).await?;
        create_outbox_event_retention_policy(manager).await?;
        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_event_retention_policy(manager).await?;
        drop_outbox_event_envelope_integrity(manager).await?;
        drop_outbox_delivery_history_integrity(manager).await?;
        drop_outbox_delivery_checkpoints(manager).await?;
        drop_outbox_dispatch_attempts(manager).await?;
        drop_outbox_execution_policy(manager).await?;
        drop_outbox_temporal_clock(manager).await?;
        drop_sys_event_consumer_tables(manager).await?;
        drop_owner_operation_receipts_table(manager).await?;
        drop_sys_events_schema(manager).await
    }
}

/// Creates the `sys_events` table owned by `rustok-outbox`.
///
/// This is the only definition of the table shape. Module and test schemas
/// reach it through [`SysEventsMigration`]; the platform migrator reaches it
/// through the append-only wrapper `m20260211_000002_create_sys_events`, which
/// delegates here instead of carrying a second copy of the DDL. A column added
/// to the outbox therefore lands in both migrators with one edit.
///
/// The table alone carries no secondary index: each access path owns a helper
/// next to the query it serves ([`create_sys_events_claim_index`] for the relay
/// claim, [`create_sys_events_retention_index`] for the delivered-event prune).
pub async fn create_sys_events_schema(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(SysEvents::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SysEvents::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(SysEvents::TenantId).uuid())
                .col(
                    ColumnDef::new(SysEvents::EventType)
                        .string_len(255)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEvents::SchemaVersion)
                        .small_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(SysEvents::Payload).json_binary().not_null())
                .col(ColumnDef::new(SysEvents::Status).string_len(32).not_null())
                .col(
                    ColumnDef::new(SysEvents::RetryCount)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(ColumnDef::new(SysEvents::NextAttemptAt).timestamp_with_time_zone())
                .col(ColumnDef::new(SysEvents::LastError).string_len(2048))
                .col(ColumnDef::new(SysEvents::ClaimedBy).string_len(128))
                .col(ColumnDef::new(SysEvents::ClaimedAt).timestamp_with_time_zone())
                .col(
                    ColumnDef::new(SysEvents::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(SysEvents::DispatchedAt).timestamp_with_time_zone())
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the outbox table for a full schema rollback.
///
/// The drop is idempotent: both migrators may roll back the same database, and
/// the second rollback must not fail on an already absent table.
pub async fn drop_sys_events_schema(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(SysEvents::Table).if_exists().to_owned())
        .await
}

/// Creates the index a relay claim batch is read through.
///
/// The claim is `status = 'pending'` plus the liveness conditions on
/// `claimed_at` / `next_attempt_at`, read in `ORDER BY created_at LIMIT
/// batch_size`. The index therefore leads with the status it filters on and
/// carries the column it orders by, so the relay reads one ordered range of the
/// claimable generation instead of sorting the whole pending set on every
/// iteration. The liveness columns stay residual predicates of that same read:
/// a never-claimed, due row passes both, and an abandoned claim is an old row
/// and is therefore at the head of the same range.
///
/// The platform migrator invokes this helper as an append-only migration, while
/// standalone owner/test schemas obtain the same index through
/// [`SysEventsMigration`].
pub async fn create_sys_events_claim_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_events_pending_created_at")
                .table(SysEvents::Table)
                .col(SysEvents::Status)
                .col(SysEvents::CreatedAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the relay claim index created by [`create_sys_events_claim_index`].
pub async fn drop_sys_events_claim_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("idx_sys_events_pending_created_at")
                .table(SysEvents::Table)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Creates the index the delivered-event retention prune is read through.
///
/// The Outbox relay's retention worker selects
/// `status = 'dispatched'` rows whose `dispatched_at` is older than the window,
/// in `ORDER BY dispatched_at LIMIT batch_size`, and then deletes that exact
/// generation. The index therefore leads with the status it filters on and
/// carries the column it orders by, so the prune reads the oldest delivered
/// rows from one ordered range instead of sorting the whole delivered history —
/// which is the largest generation of the table.
///
/// The platform migrator invokes this helper as an append-only migration, while
/// standalone owner/test schemas obtain the same index through
/// [`SysEventsMigration`].
pub async fn create_sys_events_retention_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_events_dispatched_at")
                .table(SysEvents::Table)
                .col(SysEvents::Status)
                .col(SysEvents::DispatchedAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the retention index created by [`create_sys_events_retention_index`].
pub async fn drop_sys_events_retention_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("idx_sys_events_dispatched_at")
                .table(SysEvents::Table)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Adds the denormalised `tenant_id` projection column to `sys_events` and
/// backfills it from the canonical payload.
///
/// The column exists in the fresh table shape, so this helper is a no-op
/// there; deployments created before it gain the column through the
/// append-only platform migration. The backfill prefers the canonical
/// top-level tenant and falls back to the legacy nested shape; only
/// well-formed UUID strings are copied, anything else stays `NULL` and is
/// reported by the operator queries as platform scope. The backfill is
/// PostgreSQL SQL: the platform runs PostgreSQL, while SQLite schemas are
/// always created fresh through [`create_sys_events_schema`].
pub async fn add_sys_events_tenant_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !manager.has_column("sys_events", "tenant_id").await? {
        manager
            .alter_table(
                Table::alter()
                    .table(SysEvents::Table)
                    .add_column(ColumnDef::new(SysEvents::TenantId).uuid())
                    .to_owned(),
            )
            .await?;
    }

    if manager.get_database_backend() == DbBackend::Postgres {
        let db = manager.get_connection();
        db.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            r#"
UPDATE sys_events
SET tenant_id = (
    CASE WHEN payload ? 'tenant_id' THEN payload->>'tenant_id'
         ELSE payload->'event'->>'tenant_id' END
)::uuid
WHERE tenant_id IS NULL
  AND (CASE WHEN payload ? 'tenant_id' THEN payload->>'tenant_id'
            ELSE payload->'event'->>'tenant_id' END)
      ~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
  AND (CASE WHEN payload ? 'tenant_id' THEN payload->>'tenant_id'
            ELSE payload->'event'->>'tenant_id' END) <> '00000000-0000-0000-0000-000000000000'
  -- Reapplying standalone schema setup must not rewrite immutable history.
  AND NOT EXISTS (SELECT 1 FROM pg_trigger
      WHERE tgrelid = 'sys_events'::regclass AND tgname = 'outbox_event_envelope_guard')
"#
            .to_string(),
        ))
        .await?;
    }
    Ok(())
}

/// Drops the `tenant_id` projection column during an append-only rollback.
pub async fn drop_sys_events_tenant_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    drop_sys_events_tenant_index(manager).await?;
    if manager.has_column("sys_events", "tenant_id").await? {
        manager
            .alter_table(
                Table::alter()
                    .table(SysEvents::Table)
                    .drop_column(SysEvents::TenantId)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

/// Creates the index tenant-scoped operator reads (`admin_events`, module
/// admin counters) run against: tenant first, then the status filter they
/// always carry, in claim order.
pub async fn create_sys_events_tenant_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_events_tenant_status_created_at")
                .table(SysEvents::Table)
                .col(SysEvents::TenantId)
                .col(SysEvents::Status)
                .col(SysEvents::CreatedAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the tenant operator-read index.
pub async fn drop_sys_events_tenant_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("idx_sys_events_tenant_status_created_at")
                .table(SysEvents::Table)
                .to_owned(),
        )
        .await
}

/// Integrity constraints the entity model assumes but a bare column list
/// cannot express: a closed status set, a non-negative retry counter, a
/// claim that is always a (worker, timestamp) pair, and a dispatched row
/// that always knows when it was delivered.
///
/// PostgreSQL only: SQLite cannot add CHECK constraints to an existing
/// table, and its schemas are test fixtures created fresh. Constraints are
/// added `NOT VALID` so a deployment with pre-existing drift is not broken
/// mid-upgrade; they bind every new write immediately, and operators can
/// `VALIDATE CONSTRAINT` after cleaning historical rows.
pub async fn add_sys_events_integrity_checks(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    let db = manager.get_connection();
    for (name, check) in [
        (
            "chk_sys_events_status",
            "status IN ('pending', 'dispatched', 'failed')",
        ),
        ("chk_sys_events_retry_count", "retry_count >= 0"),
        (
            "chk_sys_events_claim_pair",
            "(claimed_by IS NULL) = (claimed_at IS NULL)",
        ),
        (
            "chk_sys_events_dispatched_at",
            "status <> 'dispatched' OR dispatched_at IS NOT NULL",
        ),
    ] {
        // A duplicate DDL error otherwise poisons the enclosing transaction.
        db.execute_unprepared(&format!(
            "DO $ BEGIN ALTER TABLE sys_events ADD CONSTRAINT {name} CHECK ({check}) NOT VALID; EXCEPTION WHEN duplicate_object THEN NULL; END $"
        )).await?;
    }
    Ok(())
}

/// Drops the integrity constraints added by [`add_sys_events_integrity_checks`].
pub async fn drop_sys_events_integrity_checks(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    let db = manager.get_connection();
    for name in [
        "chk_sys_events_status",
        "chk_sys_events_retry_count",
        "chk_sys_events_claim_pair",
        "chk_sys_events_dispatched_at",
    ] {
        db.execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!("ALTER TABLE sys_events DROP CONSTRAINT IF EXISTS {name}"),
        ))
        .await?;
    }
    Ok(())
}

/// Restores the two pre-claim-index indexes during an append-only rollback.
///
/// `idx_sys_events_pending_next_attempt (status, next_attempt_at)` and
/// `idx_sys_events_claimed_at (claimed_at)` were created before the relay claim
/// had an index for the order it uses. Neither serves the claim read — the
/// liveness columns are residual predicates of the `(status, created_at)`
/// range — and no other query filters on those columns, so they were pure write
/// amplification. The append-only platform migration
/// `m20261007_000016_drop_sys_events_superseded_indexes` drops them and calls
/// this helper from its `down`.
pub async fn create_sys_events_superseded_indexes(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_events_pending_next_attempt")
                .table(SysEvents::Table)
                .col(SysEvents::Status)
                .col(SysEvents::NextAttemptAt)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_events_claimed_at")
                .table(SysEvents::Table)
                .col(SysEvents::ClaimedAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the two indexes superseded by [`create_sys_events_claim_index`].
///
/// Both drops are `if_exists`: a fresh schema never creates the pair, while a
/// schema that predates the claim index still carries it.
pub async fn drop_sys_events_superseded_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for index in [
        "idx_sys_events_pending_next_attempt",
        "idx_sys_events_claimed_at",
    ] {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(index)
                    .table(SysEvents::Table)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

/// Creates the generic durable-consumer registration and acknowledgement schema.
pub async fn create_sys_event_consumer_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(SysEventConsumers::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SysEventConsumers::EventType)
                        .string_len(255)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEventConsumers::ConsumerKey)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEventConsumers::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(SysEventConsumers::EventType)
                        .col(SysEventConsumers::ConsumerKey),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(SysEventConsumerReceipts::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(SysEventConsumerReceipts::OutboxEventId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEventConsumerReceipts::ConsumerKey)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEventConsumerReceipts::EventType)
                        .string_len(255)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(SysEventConsumerReceipts::AcknowledgedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(SysEventConsumerReceipts::OutboxEventId)
                        .col(SysEventConsumerReceipts::ConsumerKey),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_sys_event_consumer_receipts_event")
                        .from(
                            SysEventConsumerReceipts::Table,
                            SysEventConsumerReceipts::OutboxEventId,
                        )
                        .to(SysEvents::Table, SysEvents::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_event_consumers_consumer_key")
                .table(SysEventConsumers::Table)
                .col(SysEventConsumers::ConsumerKey)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("idx_sys_event_consumer_receipts_consumer_key")
                .table(SysEventConsumerReceipts::Table)
                .col(SysEventConsumerReceipts::ConsumerKey)
                .col(SysEventConsumerReceipts::OutboxEventId)
                .to_owned(),
        )
        .await?;
    Ok(())
}

/// Drops the generic durable-consumer schema during rollback.
pub async fn drop_sys_event_consumer_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(
            Table::drop()
                .table(SysEventConsumerReceipts::Table)
                .if_exists()
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(
            Table::drop()
                .table(SysEventConsumers::Table)
                .if_exists()
                .to_owned(),
        )
        .await
}

/// Creates the schema owned by the generic durable owner-operation receipt
/// primitive. The platform migrator invokes this helper as an append-only
/// migration, while standalone owner/test schemas obtain the same invariant
/// through [`SysEventsMigration`].
pub async fn create_owner_operation_receipts_table(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(OwnerOperationReceipts::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(OwnerOperationReceipts::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(OwnerOperationReceipts::TenantId).uuid())
                .col(
                    ColumnDef::new(OwnerOperationReceipts::ScopeKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::OwnerSlug)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::Operation)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::LeaseToken)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::Status)
                        .string_len(32)
                        .not_null(),
                )
                .col(ColumnDef::new(OwnerOperationReceipts::ResponseJson).json_binary())
                .col(ColumnDef::new(OwnerOperationReceipts::ErrorJson).json_binary())
                .col(
                    ColumnDef::new(OwnerOperationReceipts::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(OwnerOperationReceipts::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(OwnerOperationReceipts::CompletedAt).timestamp_with_time_zone())
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("uidx_owner_operation_receipts_scope_owner_key")
                .table(OwnerOperationReceipts::Table)
                .col(OwnerOperationReceipts::ScopeKey)
                .col(OwnerOperationReceipts::OwnerSlug)
                .col(OwnerOperationReceipts::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

/// Drops the owner-operation receipt schema during a full migration rollback.
pub async fn drop_owner_operation_receipts_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(
            Table::drop()
                .table(OwnerOperationReceipts::Table)
                .if_exists()
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum SysEvents {
    Table,
    Id,
    TenantId,
    EventType,
    SchemaVersion,
    Payload,
    Status,
    RetryCount,
    NextAttemptAt,
    LastError,
    ClaimedBy,
    ClaimedAt,
    CreatedAt,
    DispatchedAt,
}

#[derive(DeriveIden)]
enum SysEventConsumers {
    Table,
    EventType,
    ConsumerKey,
    CreatedAt,
}

#[derive(DeriveIden)]
enum SysEventConsumerReceipts {
    Table,
    OutboxEventId,
    ConsumerKey,
    EventType,
    AcknowledgedAt,
}

#[derive(DeriveIden)]
enum OwnerOperationReceipts {
    Table,
    Id,
    TenantId,
    ScopeKey,
    OwnerSlug,
    IdempotencyKey,
    Operation,
    RequestHash,
    LeaseToken,
    Status,
    ResponseJson,
    ErrorJson,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

/// Append-only owner schema for the durable temporal authority. Do not reset
/// a previously initialized floor when applying this idempotent helper.
pub async fn create_outbox_temporal_clock(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.has_table("outbox_temporal_clock").await? {
        return Ok(());
    }
    let connection = manager.get_connection();
    connection.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS outbox_temporal_clock (id INTEGER PRIMARY KEY CHECK (id = 1), high_water_ms BIGINT NOT NULL CHECK (high_water_ms >= 0))",
    ).await?;
    connection.execute_unprepared(
        "INSERT INTO outbox_temporal_clock (id, high_water_ms) VALUES (1, 0) ON CONFLICT (id) DO NOTHING",
    ).await?;
    Ok(())
}

pub async fn drop_outbox_temporal_clock(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.has_table("outbox_temporal_clock").await? {
        drop_outbox_temporal_clock_integrity(manager).await?;
    }
    manager.get_connection().execute_unprepared("DROP TABLE IF EXISTS outbox_temporal_clock").await?;
    Ok(())
}

// Module-local append-only upgrade for already initialized standalone schemas.
#[derive(DeriveMigrationName)]
pub struct OutboxTemporalClockMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxTemporalClockMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_temporal_clock(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_temporal_clock(manager).await
    }
}

/// A single deployment policy serializes claim reservation and live refresh.
/// The canonical execution permits remain the pending event claim columns.
pub async fn create_outbox_execution_policy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    if !manager.has_table("outbox_execution_policy").await? {
        db.execute_unprepared(
            "CREATE TABLE outbox_execution_policy (id INTEGER PRIMARY KEY CHECK (id = 1), max_in_flight BIGINT NOT NULL CHECK (max_in_flight BETWEEN 1 AND 1024), claim_ttl_ms BIGINT CHECK (claim_ttl_ms BETWEEN 2 AND 315360000000))",
        ).await?;
        db.execute_unprepared(
            "INSERT INTO outbox_execution_policy (id, max_in_flight, claim_ttl_ms) VALUES (1, 8, NULL)",
        ).await?;
    }
    db.execute_unprepared(
        "CREATE INDEX IF NOT EXISTS idx_sys_events_execution_claims ON sys_events (status, claimed_at) WHERE claimed_at IS NOT NULL",
    ).await?;
    Ok(())
}

pub async fn drop_outbox_execution_policy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.has_table("outbox_execution_policy").await? {
        drop_outbox_execution_policy_integrity(manager).await?;
    }
    let db = manager.get_connection();
    db.execute_unprepared("DROP INDEX IF EXISTS idx_sys_events_execution_claims").await?;
    db.execute_unprepared("DROP TABLE IF EXISTS outbox_execution_policy").await?;
    Ok(())
}

#[derive(DeriveMigrationName)]
pub struct OutboxExecutionPolicyMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxExecutionPolicyMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_execution_policy(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_execution_policy(manager).await
    }
}

const POSTGRES_EXECUTION_POLICY_GUARD: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_execution_policy() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    has_claims boolean;
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'outbox execution policy singleton cannot be inserted or removed' USING ERRCODE = '23514';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.id <> 1
       OR (OLD.claim_ttl_ms IS NOT NULL AND NEW.claim_ttl_ms IS NULL) THEN
        RAISE EXCEPTION 'outbox execution policy identity or TTL reset is forbidden' USING ERRCODE = '23514';
    END IF;
    IF NEW.max_in_flight IS NOT DISTINCT FROM OLD.max_in_flight
       AND NEW.claim_ttl_ms IS NOT DISTINCT FROM OLD.claim_ttl_ms THEN
        RETURN NEW;
    END IF;
    -- Every acquisition of a claim takes and writes this singleton first
    -- (raw SQL takes it NOWAIT in the event trigger). Reconfiguration is
    -- conservative: even an expired claim can represent an ambiguous effect.
    EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I.sys_events WHERE claimed_by IS NOT NULL OR claimed_at IS NOT NULL)', TG_TABLE_SCHEMA)
        INTO has_claims;
    IF has_claims THEN
        RAISE EXCEPTION 'outbox execution policy change requires drained claims' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$
"#;
const POSTGRES_EXECUTION_CLAIM_GUARD: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_execution_claim() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    capacity bigint;
    ttl bigint;
    cutoff timestamptz;
    active_count bigint;
BEGIN
    IF NEW.claimed_at IS NULL AND NEW.claimed_by IS NULL THEN
        RETURN NEW;
    END IF;
    IF NEW.claimed_at IS NULL OR NEW.claimed_by IS NULL
       OR btrim(NEW.claimed_by) = '' OR NEW.status <> 'pending' THEN
        RAISE EXCEPTION 'invalid outbox execution claim' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.id IS NOT DISTINCT FROM OLD.id
           AND NEW.status IS NOT DISTINCT FROM OLD.status
           AND NEW.claimed_by IS NOT DISTINCT FROM OLD.claimed_by
           AND NEW.claimed_at IS NOT DISTINCT FROM OLD.claimed_at THEN
            RETURN NEW;
        END IF;
    END IF;
    -- A raw SQL writer can already hold the event row. Never wait for the
    -- earlier singleton in that lock order. Writing the singleton also makes
    -- stale repeatable-read claim snapshots fail with a serialization error.
    EXECUTE format('LOCK TABLE %I.outbox_execution_policy IN ROW EXCLUSIVE MODE NOWAIT', TG_TABLE_SCHEMA);
    EXECUTE format('SELECT max_in_flight, claim_ttl_ms FROM %I.outbox_execution_policy WHERE id = 1 FOR UPDATE NOWAIT', TG_TABLE_SCHEMA)
        INTO capacity, ttl;
    IF capacity IS NULL OR ttl IS NULL THEN
        RAISE EXCEPTION 'outbox execution policy is missing or unbound' USING ERRCODE = '23514';
    END IF;
    EXECUTE format('UPDATE %I.outbox_execution_policy SET max_in_flight = max_in_flight WHERE id = 1', TG_TABLE_SCHEMA);
    cutoff := clock_timestamp() - ttl * interval '1 millisecond';
    IF NEW.claimed_at > cutoff THEN
        EXECUTE format('SELECT COUNT(*) FROM %I.sys_events WHERE status = ''pending'' AND claimed_at > $1 AND id <> $2', TG_TABLE_SCHEMA)
            INTO active_count USING cutoff, NEW.id;
        IF active_count >= capacity THEN
            RAISE EXCEPTION 'outbox execution capacity exhausted' USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN NEW;
END $$
"#;
const SQLITE_EXECUTION_POLICY_GUARD: &str = r#"
CREATE TRIGGER outbox_execution_policy_guard
BEFORE UPDATE ON outbox_execution_policy
WHEN NEW.id IS NOT OLD.id OR NEW.id <> 1
 OR typeof(NEW.max_in_flight) <> 'integer'
 OR (NEW.claim_ttl_ms IS NOT NULL AND typeof(NEW.claim_ttl_ms) <> 'integer')
 OR (OLD.claim_ttl_ms IS NOT NULL AND NEW.claim_ttl_ms IS NULL)
 OR ((NEW.max_in_flight IS NOT OLD.max_in_flight OR NEW.claim_ttl_ms IS NOT OLD.claim_ttl_ms)
     AND EXISTS (SELECT 1 FROM sys_events WHERE claimed_by IS NOT NULL OR claimed_at IS NOT NULL))
BEGIN SELECT RAISE(ABORT, 'invalid or undrained outbox execution policy change'); END
"#;
const SQLITE_EXECUTION_CLAIM_INSERT: &str = r#"
CREATE TRIGGER outbox_execution_claim_insert
BEFORE INSERT ON sys_events
WHEN NEW.claimed_at IS NOT NULL OR NEW.claimed_by IS NOT NULL
BEGIN
    SELECT CASE WHEN NEW.claimed_at IS NULL OR NEW.claimed_by IS NULL
        OR trim(NEW.claimed_by) = '' OR NEW.status <> 'pending'
        OR julianday(NEW.claimed_at) IS NULL
        THEN RAISE(ABORT, 'invalid outbox execution claim') END;
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM outbox_execution_policy WHERE id = 1 AND claim_ttl_ms IS NOT NULL
    ) THEN RAISE(ABORT, 'outbox execution policy is missing or unbound') END;
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM outbox_execution_policy AS policy
        WHERE policy.id = 1
          AND CAST(ROUND((julianday(NEW.claimed_at) - 2440587.5) * 86400000) AS INTEGER)
              > CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) - policy.claim_ttl_ms
          AND (SELECT COUNT(*) FROM sys_events
               WHERE status = 'pending' AND id <> NEW.id
                 AND CAST(ROUND((julianday(claimed_at) - 2440587.5) * 86400000) AS INTEGER)
                     > CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) - policy.claim_ttl_ms
          ) >= policy.max_in_flight
    ) THEN RAISE(ABORT, 'outbox execution capacity exhausted') END;
END
"#;
const SQLITE_EXECUTION_CLAIM_UPDATE: &str = r#"
CREATE TRIGGER outbox_execution_claim_update
BEFORE UPDATE OF id, status, claimed_at, claimed_by ON sys_events
WHEN NEW.claimed_at IS NOT NULL OR NEW.claimed_by IS NOT NULL
BEGIN
    SELECT CASE WHEN NEW.claimed_at IS NULL OR NEW.claimed_by IS NULL
        OR trim(NEW.claimed_by) = '' OR NEW.status <> 'pending'
        OR julianday(NEW.claimed_at) IS NULL
        THEN RAISE(ABORT, 'invalid outbox execution claim') END;
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM outbox_execution_policy WHERE id = 1 AND claim_ttl_ms IS NOT NULL
    ) THEN RAISE(ABORT, 'outbox execution policy is missing or unbound') END;
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM outbox_execution_policy AS policy
        WHERE policy.id = 1
          AND CAST(ROUND((julianday(NEW.claimed_at) - 2440587.5) * 86400000) AS INTEGER)
              > CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) - policy.claim_ttl_ms
          AND (SELECT COUNT(*) FROM sys_events
               WHERE status = 'pending' AND id <> NEW.id
                 AND CAST(ROUND((julianday(claimed_at) - 2440587.5) * 86400000) AS INTEGER)
                     > CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) - policy.claim_ttl_ms
          ) >= policy.max_in_flight
    ) THEN RAISE(ABORT, 'outbox execution capacity exhausted') END;
END
"#;

/// Install execution-policy integrity on an already provisioned schema.
/// Run during a drained, transactional migration; never infer that an expired
/// claim means the external publication has stopped.
pub async fn create_outbox_execution_policy_integrity(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    let nested = SchemaManager::new(&txn);
    install_execution_policy_integrity(&nested).await?;
    txn.commit().await
}

async fn install_execution_policy_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = manager.get_database_backend();
    let integer_guard = match backend {
        DbBackend::Postgres => {
            db.execute_unprepared("LOCK TABLE outbox_execution_policy IN ACCESS EXCLUSIVE MODE").await?;
            db.execute_unprepared("LOCK TABLE sys_events IN SHARE ROW EXCLUSIVE MODE NOWAIT").await?;
            ""
        }
        DbBackend::Sqlite => {
            db.execute_unprepared("UPDATE outbox_execution_policy SET max_in_flight = max_in_flight WHERE id = 1").await?;
            " AND typeof(max_in_flight) = 'integer' AND (claim_ttl_ms IS NULL OR typeof(claim_ttl_ms) = 'integer')"
        }
        _ => return Err(DbErr::Migration("unsupported outbox execution policy backend".into())),
    };
    let row = db.query_one_raw(Statement::from_string(backend, format!(
        "SELECT COUNT(*) AS total, COUNT(CASE WHEN id = 1 AND max_in_flight BETWEEN 1 AND 1024 AND (claim_ttl_ms IS NULL OR claim_ttl_ms BETWEEN 2 AND 315360000000){integer_guard} THEN 1 END) AS valid FROM outbox_execution_policy"
    ))).await?.ok_or_else(|| DbErr::Migration("outbox execution policy validation unavailable".into()))?;
    if row.try_get::<i64>("", "total")? != 1 || row.try_get::<i64>("", "valid")? != 1 {
        return Err(DbErr::Migration("invalid outbox execution policy singleton; offline recovery required".into()));
    }
    if db.query_one_raw(Statement::from_string(backend,
        "SELECT 1 FROM sys_events WHERE claimed_by IS NOT NULL OR claimed_at IS NOT NULL LIMIT 1",
    )).await?.is_some() {
        return Err(DbErr::Migration("drain and reconcile all outbox claims before installing execution integrity".into()));
    }
    drop_outbox_execution_policy_integrity(manager).await?;
    let statements: &[&str] = match backend {
        DbBackend::Postgres => &[
            POSTGRES_EXECUTION_POLICY_GUARD,
            POSTGRES_EXECUTION_CLAIM_GUARD,
            "CREATE TRIGGER outbox_execution_policy_guard BEFORE INSERT OR UPDATE OR DELETE ON outbox_execution_policy FOR EACH ROW EXECUTE FUNCTION outbox_guard_execution_policy()",
            "CREATE TRIGGER outbox_execution_policy_no_truncate BEFORE TRUNCATE ON outbox_execution_policy FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_execution_policy()",
            "CREATE TRIGGER outbox_execution_claim_guard BEFORE INSERT OR UPDATE OF id, status, claimed_by, claimed_at ON sys_events FOR EACH ROW EXECUTE FUNCTION outbox_guard_execution_claim()",
        ],
        DbBackend::Sqlite => &[
            "CREATE TRIGGER outbox_execution_policy_no_insert BEFORE INSERT ON outbox_execution_policy BEGIN SELECT RAISE(ABORT, 'outbox execution policy singleton cannot be inserted'); END",
            "CREATE TRIGGER outbox_execution_policy_no_delete BEFORE DELETE ON outbox_execution_policy BEGIN SELECT RAISE(ABORT, 'outbox execution policy singleton cannot be removed'); END",
            SQLITE_EXECUTION_POLICY_GUARD,
            SQLITE_EXECUTION_CLAIM_INSERT,
            SQLITE_EXECUTION_CLAIM_UPDATE,
        ],
        _ => return Err(DbErr::Migration("unsupported outbox execution policy backend".into())),
    };
    for sql in statements {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

pub async fn drop_outbox_execution_policy_integrity(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let statements: &[&str] = match manager.get_database_backend() {
        DbBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS outbox_execution_claim_guard ON sys_events",
            "DROP TRIGGER IF EXISTS outbox_execution_policy_guard ON outbox_execution_policy",
            "DROP TRIGGER IF EXISTS outbox_execution_policy_no_truncate ON outbox_execution_policy",
            "DROP FUNCTION IF EXISTS outbox_guard_execution_claim()",
            "DROP FUNCTION IF EXISTS outbox_guard_execution_policy()",
        ],
        DbBackend::Sqlite => &[
            "DROP TRIGGER IF EXISTS outbox_execution_claim_insert",
            "DROP TRIGGER IF EXISTS outbox_execution_claim_update",
            "DROP TRIGGER IF EXISTS outbox_execution_policy_no_insert",
            "DROP TRIGGER IF EXISTS outbox_execution_policy_no_delete",
            "DROP TRIGGER IF EXISTS outbox_execution_policy_guard",
        ],
        _ => return Err(DbErr::Migration("unsupported outbox execution policy backend".into())),
    };
    for sql in statements {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

#[derive(DeriveMigrationName)]
pub struct OutboxExecutionPolicyIntegrityMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxExecutionPolicyIntegrityMigration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_execution_policy_integrity(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_execution_policy_integrity(manager).await
    }
}

const POSTGRES_TEMPORAL_CLOCK_GUARD: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_temporal_clock() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'outbox temporal clock singleton cannot be inserted or removed' USING ERRCODE = '23514';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id OR NEW.id <> 1
       OR NEW.high_water_ms < OLD.high_water_ms THEN
        RAISE EXCEPTION 'outbox temporal clock identity or floor regression is forbidden' USING ERRCODE = '23514';
    END IF;
    IF NEW.high_water_ms IS DISTINCT FROM OLD.high_water_ms
       AND NEW.high_water_ms > CAST(FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000) AS BIGINT) THEN
        RAISE EXCEPTION 'outbox temporal clock cannot advance beyond SQL time' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
"#;

const SQLITE_TEMPORAL_CLOCK_GUARD: &str = r#"
CREATE TRIGGER outbox_temporal_clock_guard
BEFORE UPDATE ON outbox_temporal_clock
WHEN NEW.id IS NOT OLD.id OR NEW.id <> 1
 OR typeof(NEW.high_water_ms) <> 'integer'
 OR NEW.high_water_ms < OLD.high_water_ms
 OR (NEW.high_water_ms IS NOT OLD.high_water_ms
     AND NEW.high_water_ms > CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
BEGIN SELECT RAISE(ABORT, 'invalid outbox temporal clock advance'); END
"#;

/// Install guards without resetting the baseline. A preexisting future
/// floor is preserved: observation fails closed until SQL time catches up.
pub async fn create_outbox_temporal_clock_integrity(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    let nested = SchemaManager::new(&txn);
    install_temporal_clock_integrity(&nested).await?;
    txn.commit().await
}

async fn install_temporal_clock_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = manager.get_database_backend();
    let integer_guard = match backend {
        DbBackend::Postgres => {
            db.execute_unprepared("LOCK TABLE outbox_temporal_clock IN ACCESS EXCLUSIVE MODE").await?;
            ""
        }
        DbBackend::Sqlite => {
            db.execute_unprepared("UPDATE outbox_temporal_clock SET high_water_ms = high_water_ms WHERE id = 1").await?;
            " AND typeof(high_water_ms) = 'integer'"
        }
        _ => return Err(DbErr::Migration("unsupported outbox temporal clock backend".into())),
    };
    let row = db.query_one_raw(Statement::from_string(backend, format!(
        "SELECT COUNT(*) AS total, COUNT(CASE WHEN id = 1 AND high_water_ms >= 0{integer_guard} THEN 1 END) AS valid FROM outbox_temporal_clock"
    ))).await?.ok_or_else(|| DbErr::Migration("outbox temporal clock validation unavailable".into()))?;
    if row.try_get::<i64>("", "total")? != 1 || row.try_get::<i64>("", "valid")? != 1 {
        return Err(DbErr::Migration("invalid outbox temporal clock singleton; offline recovery required".into()));
    }
    drop_outbox_temporal_clock_integrity(manager).await?;
    let statements: &[&str] = match backend {
        DbBackend::Postgres => &[
            POSTGRES_TEMPORAL_CLOCK_GUARD,
            "CREATE TRIGGER outbox_temporal_clock_guard BEFORE INSERT OR UPDATE OR DELETE ON outbox_temporal_clock FOR EACH ROW EXECUTE FUNCTION outbox_guard_temporal_clock()",
            "CREATE TRIGGER outbox_temporal_clock_no_truncate BEFORE TRUNCATE ON outbox_temporal_clock FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_temporal_clock()",
        ],
        DbBackend::Sqlite => &[
            "CREATE TRIGGER outbox_temporal_clock_no_insert BEFORE INSERT ON outbox_temporal_clock BEGIN SELECT RAISE(ABORT, 'outbox temporal clock singleton cannot be inserted'); END",
            "CREATE TRIGGER outbox_temporal_clock_no_delete BEFORE DELETE ON outbox_temporal_clock BEGIN SELECT RAISE(ABORT, 'outbox temporal clock singleton cannot be removed'); END",
            SQLITE_TEMPORAL_CLOCK_GUARD,
        ],
        _ => return Err(DbErr::Migration("unsupported outbox temporal clock backend".into())),
    };
    for sql in statements {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

pub async fn drop_outbox_temporal_clock_integrity(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let statements: &[&str] = match manager.get_database_backend() {
        DbBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS outbox_temporal_clock_guard ON outbox_temporal_clock",
            "DROP TRIGGER IF EXISTS outbox_temporal_clock_no_truncate ON outbox_temporal_clock",
            "DROP FUNCTION IF EXISTS outbox_guard_temporal_clock()",
        ],
        DbBackend::Sqlite => &[
            "DROP TRIGGER IF EXISTS outbox_temporal_clock_guard",
            "DROP TRIGGER IF EXISTS outbox_temporal_clock_no_insert",
            "DROP TRIGGER IF EXISTS outbox_temporal_clock_no_delete",
        ],
        _ => return Err(DbErr::Migration("unsupported outbox temporal clock backend".into())),
    };
    for sql in statements {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

#[derive(DeriveMigrationName)]
pub struct OutboxTemporalClockIntegrityMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxTemporalClockIntegrityMigration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_temporal_clock_integrity(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_temporal_clock_integrity(manager).await
    }
}

/// Durable transport intents. The event remains the owner of retention and
/// its UUID remains the remote publication identity.
pub async fn create_outbox_dispatch_attempts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS outbox_dispatch_attempts (
            id UUID NOT NULL PRIMARY KEY,
            event_id UUID NOT NULL REFERENCES sys_events(id) ON DELETE CASCADE,
            claim_token VARCHAR(128) NOT NULL,
            started_at TIMESTAMP WITH TIME ZONE NOT NULL,
            finished_at TIMESTAMP WITH TIME ZONE,
            state VARCHAR(32) NOT NULL CHECK (state IN ('open', 'confirmed', 'uncertain', 'not_started')),
            CHECK ((state = 'open') = (finished_at IS NULL)),
            CHECK (finished_at IS NULL OR finished_at >= started_at),
            UNIQUE (event_id, claim_token)
        )",
    ).await?;
    db.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_outbox_dispatch_attempts_open ON outbox_dispatch_attempts (event_id) WHERE state = 'open'",
    ).await?;
    Ok(())
}

pub async fn drop_outbox_dispatch_attempts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared("DROP TABLE IF EXISTS outbox_dispatch_attempts").await?;
    Ok(())
}

#[derive(DeriveMigrationName)]
pub struct OutboxDispatchAttemptsMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxDispatchAttemptsMigration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_dispatch_attempts(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_dispatch_attempts(manager).await
    }
}

pub async fn create_outbox_delivery_checkpoints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(
        "CREATE TABLE IF NOT EXISTS outbox_delivery_checkpoints (
            event_id UUID NOT NULL PRIMARY KEY REFERENCES sys_events(id) ON DELETE CASCADE,
            route_key VARCHAR(128) NOT NULL,
            local_required BOOLEAN,
            primary_confirmed_at TIMESTAMP WITH TIME ZONE,
            CHECK (primary_confirmed_at IS NULL OR local_required IS NOT NULL)
        )",
    ).await?;
    Ok(())
}

pub async fn drop_outbox_delivery_checkpoints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared("DROP TABLE IF EXISTS outbox_delivery_checkpoints").await?;
    Ok(())
}

#[derive(DeriveMigrationName)]
pub struct OutboxDeliveryCheckpointsMigration;

#[async_trait::async_trait]
impl MigrationTrait for OutboxDeliveryCheckpointsMigration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_outbox_delivery_checkpoints(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_outbox_delivery_checkpoints(manager).await
    }
}

const POSTGRES_DELIVERY_HISTORY_GUARDS: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_dispatch_attempt() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'outbox attempt history cannot be truncated' USING ERRCODE = '23514';
    ELSIF TG_OP = 'DELETE' THEN
        IF OLD.state = 'open' OR EXISTS (SELECT 1 FROM sys_events WHERE id = OLD.event_id) THEN
            RAISE EXCEPTION 'outbox attempt history follows terminal event retention' USING ERRCODE = '23514';
        END IF;
        RETURN OLD;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.state <> 'open' OR NEW.finished_at IS NOT NULL OR length(trim(NEW.claim_token)) = 0 THEN
            RAISE EXCEPTION 'outbox attempt must start open' USING ERRCODE = '23514';
        END IF;
        -- Canonical writers already own this row. Direct child writers must
        -- never wait for the parent while holding a child/unique-index lock.
        PERFORM 1 FROM sys_events WHERE id = NEW.event_id AND status = 'pending'
            AND claimed_by = NEW.claim_token AND claimed_at IS NOT NULL
            AND claimed_at <= NEW.started_at FOR UPDATE NOWAIT;
        IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM outbox_delivery_checkpoints WHERE event_id = NEW.event_id) THEN
            RAISE EXCEPTION 'outbox attempt requires its current claim and route' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF ROW(NEW.id, NEW.event_id, NEW.claim_token, NEW.started_at)
       IS DISTINCT FROM ROW(OLD.id, OLD.event_id, OLD.claim_token, OLD.started_at)
       OR OLD.state <> 'open' OR NEW.state NOT IN ('confirmed', 'uncertain', 'not_started')
       OR NEW.finished_at IS NULL OR NEW.finished_at < OLD.started_at THEN
        RAISE EXCEPTION 'outbox attempt identity and closed outcome are immutable' USING ERRCODE = '23514';
    END IF;
    PERFORM 1 FROM sys_events WHERE id = OLD.event_id AND claimed_by IS NULL AND claimed_at IS NULL
        AND ((NEW.state = 'confirmed' AND status = 'dispatched')
          OR (NEW.state = 'uncertain' AND status IN ('pending', 'failed'))
          OR (NEW.state = 'not_started' AND status = 'pending')) FOR UPDATE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'outbox attempt outcome requires matching event transition' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION outbox_guard_delivery_checkpoint() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'outbox checkpoint history cannot be truncated' USING ERRCODE = '23514';
    ELSIF TG_OP = 'DELETE' THEN
        IF EXISTS (SELECT 1 FROM sys_events WHERE id = OLD.event_id) THEN
            RAISE EXCEPTION 'outbox checkpoint follows event retention' USING ERRCODE = '23514';
        END IF;
        RETURN OLD;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.local_required IS NOT NULL OR NEW.primary_confirmed_at IS NOT NULL
           OR length(NEW.route_key) NOT BETWEEN 1 AND 128 OR trim(NEW.route_key) <> NEW.route_key THEN
            RAISE EXCEPTION 'outbox route must start unconfirmed' USING ERRCODE = '23514';
        END IF;
        PERFORM 1 FROM sys_events WHERE id = NEW.event_id AND status = 'pending'
            AND claimed_by IS NOT NULL AND claimed_at IS NOT NULL FOR UPDATE NOWAIT;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'outbox route requires a pending claim' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.event_id IS DISTINCT FROM OLD.event_id OR NEW.route_key IS DISTINCT FROM OLD.route_key
       OR NOT ((OLD.local_required IS NULL AND NEW.local_required IS NOT NULL AND NEW.primary_confirmed_at IS NULL)
            OR (OLD.local_required IS NOT NULL AND NEW.local_required IS NOT DISTINCT FROM OLD.local_required
                AND OLD.primary_confirmed_at IS NULL AND NEW.primary_confirmed_at IS NOT NULL)) THEN
        RAISE EXCEPTION 'outbox route, topology and confirmed checkpoint are immutable' USING ERRCODE = '23514';
    END IF;
    PERFORM 1 FROM sys_events e WHERE e.id = OLD.event_id AND e.status = 'pending'
        AND e.claimed_at IS NOT NULL AND EXISTS (
            SELECT 1 FROM outbox_dispatch_attempts a WHERE a.event_id = e.id AND a.state = 'open'
                AND a.claim_token = e.claimed_by
                AND (NEW.primary_confirmed_at IS NULL OR NEW.primary_confirmed_at >= a.started_at)
        ) FOR UPDATE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'outbox checkpoint requires an open current attempt' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION outbox_guard_history_parent() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'outbox history cannot be truncated with its parent' USING ERRCODE = '23514';
    END IF;
    IF EXISTS (SELECT 1 FROM outbox_dispatch_attempts WHERE event_id = OLD.id AND state = 'open') THEN
        RAISE EXCEPTION 'outbox event has unresolved dispatch evidence' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END $$;
"#;

const SQLITE_DELIVERY_HISTORY_GUARDS: &str = r#"
CREATE TRIGGER outbox_attempt_insert_guard BEFORE INSERT ON outbox_dispatch_attempts
WHEN NEW.state <> 'open' OR NEW.finished_at IS NOT NULL OR length(trim(NEW.claim_token)) = 0
 OR EXISTS (SELECT 1 FROM outbox_dispatch_attempts a WHERE a.id = NEW.id OR a.rowid = NEW.rowid
      OR (a.event_id = NEW.event_id AND (a.claim_token = NEW.claim_token OR a.state = 'open')))
 OR NOT EXISTS (SELECT 1 FROM sys_events e WHERE e.id = NEW.event_id AND e.status = 'pending'
      AND e.claimed_by = NEW.claim_token AND e.claimed_at IS NOT NULL AND e.claimed_at <= NEW.started_at)
 OR NOT EXISTS (SELECT 1 FROM outbox_delivery_checkpoints WHERE event_id = NEW.event_id)
BEGIN SELECT RAISE(ABORT, 'invalid or replacement outbox attempt admission'); END;

CREATE TRIGGER outbox_attempt_update_guard BEFORE UPDATE ON outbox_dispatch_attempts
WHEN NEW.id IS NOT OLD.id OR NEW.rowid IS NOT OLD.rowid OR NEW.event_id IS NOT OLD.event_id
 OR NEW.claim_token IS NOT OLD.claim_token OR NEW.started_at IS NOT OLD.started_at
 OR OLD.state <> 'open' OR NEW.state NOT IN ('confirmed', 'uncertain', 'not_started')
 OR NEW.finished_at IS NULL OR NEW.finished_at < OLD.started_at
 OR NOT EXISTS (SELECT 1 FROM sys_events e WHERE e.id = OLD.event_id
      AND e.claimed_by IS NULL AND e.claimed_at IS NULL
      AND ((NEW.state = 'confirmed' AND e.status = 'dispatched')
        OR (NEW.state = 'uncertain' AND e.status IN ('pending', 'failed'))
        OR (NEW.state = 'not_started' AND e.status = 'pending')))
BEGIN SELECT RAISE(ABORT, 'invalid outbox attempt transition'); END;

CREATE TRIGGER outbox_attempt_delete_guard BEFORE DELETE ON outbox_dispatch_attempts
WHEN OLD.state = 'open' OR EXISTS (SELECT 1 FROM sys_events WHERE id = OLD.event_id)
BEGIN SELECT RAISE(ABORT, 'outbox attempt history follows terminal event retention'); END;

CREATE TRIGGER outbox_checkpoint_insert_guard BEFORE INSERT ON outbox_delivery_checkpoints
WHEN NEW.local_required IS NOT NULL OR NEW.primary_confirmed_at IS NOT NULL
 OR typeof(NEW.route_key) <> 'text' OR length(NEW.route_key) NOT BETWEEN 1 AND 128
 OR trim(NEW.route_key) <> NEW.route_key
 OR EXISTS (SELECT 1 FROM outbox_delivery_checkpoints c WHERE c.event_id = NEW.event_id OR c.rowid = NEW.rowid)
 OR NOT EXISTS (SELECT 1 FROM sys_events e WHERE e.id = NEW.event_id AND e.status = 'pending'
      AND e.claimed_by IS NOT NULL AND e.claimed_at IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'invalid or replacement outbox route admission'); END;

CREATE TRIGGER outbox_checkpoint_update_guard BEFORE UPDATE ON outbox_delivery_checkpoints
WHEN NEW.event_id IS NOT OLD.event_id OR NEW.rowid IS NOT OLD.rowid OR NEW.route_key IS NOT OLD.route_key
 OR NOT ((OLD.local_required IS NULL AND typeof(NEW.local_required) = 'integer'
          AND NEW.local_required IN (0, 1) AND NEW.primary_confirmed_at IS NULL)
      OR (OLD.local_required IS NOT NULL AND NEW.local_required IS OLD.local_required
          AND OLD.primary_confirmed_at IS NULL AND NEW.primary_confirmed_at IS NOT NULL))
 OR NOT EXISTS (SELECT 1 FROM sys_events e JOIN outbox_dispatch_attempts a ON a.event_id = e.id
      WHERE e.id = OLD.event_id AND e.status = 'pending' AND e.claimed_at IS NOT NULL
        AND a.state = 'open' AND a.claim_token = e.claimed_by
        AND (NEW.primary_confirmed_at IS NULL OR NEW.primary_confirmed_at >= a.started_at))
BEGIN SELECT RAISE(ABORT, 'invalid outbox checkpoint transition'); END;

CREATE TRIGGER outbox_checkpoint_delete_guard BEFORE DELETE ON outbox_delivery_checkpoints
WHEN EXISTS (SELECT 1 FROM sys_events WHERE id = OLD.event_id)
BEGIN SELECT RAISE(ABORT, 'outbox checkpoint follows event retention'); END;

CREATE TRIGGER outbox_history_parent_delete_guard BEFORE DELETE ON sys_events
WHEN EXISTS (SELECT 1 FROM outbox_dispatch_attempts WHERE event_id = OLD.id AND state = 'open')
BEGIN SELECT RAISE(ABORT, 'outbox event has unresolved dispatch evidence'); END;
"#;

/// One installer for the standalone and platform migration paths. Existing
/// history is preserved; old attempts predating checkpoints are not fabricated.
pub async fn create_outbox_delivery_history_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    let nested = SchemaManager::new(&txn);
    let manager = &nested;
    let db = manager.get_connection();
    let backend = manager.get_database_backend();
    match backend {
        DbBackend::Postgres => {
            db.execute_unprepared("LOCK TABLE sys_events, outbox_delivery_checkpoints, outbox_dispatch_attempts IN SHARE ROW EXCLUSIVE MODE NOWAIT").await?;
        }
        DbBackend::Sqlite => {
            db.execute_unprepared("UPDATE sys_events SET retry_count = retry_count WHERE 1 = 0").await?;
        }
        _ => return Err(DbErr::Migration("unsupported outbox delivery history backend".into())),
    }
    drop_outbox_delivery_history_integrity(manager).await?;
    match backend {
        DbBackend::Postgres => {
            db.execute_unprepared(POSTGRES_DELIVERY_HISTORY_GUARDS).await?;
            for sql in [
                "CREATE TRIGGER outbox_attempt_guard BEFORE INSERT OR UPDATE OR DELETE ON outbox_dispatch_attempts FOR EACH ROW EXECUTE FUNCTION outbox_guard_dispatch_attempt()",
                "CREATE TRIGGER outbox_attempt_no_truncate BEFORE TRUNCATE ON outbox_dispatch_attempts FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_dispatch_attempt()",
                "CREATE TRIGGER outbox_checkpoint_guard BEFORE INSERT OR UPDATE OR DELETE ON outbox_delivery_checkpoints FOR EACH ROW EXECUTE FUNCTION outbox_guard_delivery_checkpoint()",
                "CREATE TRIGGER outbox_checkpoint_no_truncate BEFORE TRUNCATE ON outbox_delivery_checkpoints FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_delivery_checkpoint()",
                "CREATE TRIGGER outbox_history_parent_delete_guard BEFORE DELETE ON sys_events FOR EACH ROW EXECUTE FUNCTION outbox_guard_history_parent()",
                "CREATE TRIGGER outbox_history_parent_no_truncate BEFORE TRUNCATE ON sys_events FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_history_parent()",
            ] { db.execute_unprepared(sql).await?; }
        }
        DbBackend::Sqlite => { db.execute_unprepared(SQLITE_DELIVERY_HISTORY_GUARDS).await?; }
        _ => return Err(DbErr::Migration("unsupported outbox delivery history backend".into())),
    }
    txn.commit().await
}

/// Used only by explicit schema teardown, not a production downgrade.
async fn drop_outbox_delivery_history_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let statements: &[&str] = match manager.get_database_backend() {
        DbBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS outbox_attempt_guard ON outbox_dispatch_attempts",
            "DROP TRIGGER IF EXISTS outbox_attempt_no_truncate ON outbox_dispatch_attempts",
            "DROP TRIGGER IF EXISTS outbox_checkpoint_guard ON outbox_delivery_checkpoints",
            "DROP TRIGGER IF EXISTS outbox_checkpoint_no_truncate ON outbox_delivery_checkpoints",
            "DROP TRIGGER IF EXISTS outbox_history_parent_delete_guard ON sys_events",
            "DROP TRIGGER IF EXISTS outbox_history_parent_no_truncate ON sys_events",
            "DROP FUNCTION IF EXISTS outbox_guard_dispatch_attempt()",
            "DROP FUNCTION IF EXISTS outbox_guard_delivery_checkpoint()",
            "DROP FUNCTION IF EXISTS outbox_guard_history_parent()",
        ],
        DbBackend::Sqlite => &[
            "DROP TRIGGER IF EXISTS outbox_attempt_insert_guard",
            "DROP TRIGGER IF EXISTS outbox_attempt_update_guard",
            "DROP TRIGGER IF EXISTS outbox_attempt_delete_guard",
            "DROP TRIGGER IF EXISTS outbox_checkpoint_insert_guard",
            "DROP TRIGGER IF EXISTS outbox_checkpoint_update_guard",
            "DROP TRIGGER IF EXISTS outbox_checkpoint_delete_guard",
            "DROP TRIGGER IF EXISTS outbox_history_parent_delete_guard",
        ],
        _ => return Err(DbErr::Migration("unsupported outbox delivery history backend".into())),
    };
    for sql in statements { db.execute_unprepared(sql).await?; }
    Ok(())
}

const POSTGRES_EVENT_ENVELOPE_GUARD: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_event_envelope() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.id, NEW.tenant_id, NEW.event_type, NEW.schema_version, NEW.payload, NEW.created_at)
           IS DISTINCT FROM ROW(OLD.id, OLD.tenant_id, OLD.event_type, OLD.schema_version, OLD.payload, OLD.created_at) THEN
            RAISE EXCEPTION 'outbox event envelope is immutable' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.status NOT IN ('dispatched', 'failed') OR OLD.claimed_by IS NOT NULL
        OR OLD.claimed_at IS NOT NULL OR (OLD.status = 'dispatched' AND OLD.dispatched_at IS NULL) THEN
        RAISE EXCEPTION 'outbox event is not terminal and unclaimed' USING ERRCODE = '23514';
    END IF;
    -- Retention takes this lock before event rows. A direct DELETE must
    -- fail rather than wait for the registry in reverse lock order.
    IF current_setting('transaction_isolation') <> 'read committed' THEN
        RAISE EXCEPTION 'outbox event deletion requires READ COMMITTED' USING ERRCODE = '23514';
    END IF;
    LOCK TABLE sys_event_consumers IN SHARE MODE NOWAIT;
    IF EXISTS (SELECT 1 FROM sys_event_consumers c WHERE c.event_type = OLD.event_type
    AND NOT EXISTS (SELECT 1 FROM sys_event_consumer_receipts a
        WHERE a.outbox_event_id = OLD.id AND a.consumer_key = c.consumer_key AND a.event_type = c.event_type)) THEN
        RAISE EXCEPTION 'outbox event still has required consumer acknowledgements' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END $$;
"#;

const SQLITE_EVENT_ENVELOPE_GUARDS: &str = r#"
-- REPLACE may otherwise implicitly delete the old row and its children even
-- with recursive_triggers OFF. Preserve the exact-key DO NOTHING contract
-- used by write-once publishers; a different physical alias is not replay.
CREATE UNIQUE INDEX IF NOT EXISTS uq_sys_events_uuid_identity ON sys_events (replace(replace(replace(replace(lower(CASE WHEN typeof(id) = 'blob' THEN hex(id) ELSE CAST(id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''));
CREATE TRIGGER outbox_event_insert_identity_guard BEFORE INSERT ON sys_events
BEGIN
    SELECT CASE
        WHEN EXISTS (SELECT 1 FROM sys_events e WHERE e.rowid = NEW.rowid AND e.id IS NOT NEW.id)
        THEN RAISE(ABORT, 'outbox event rowid replacement is forbidden')
        WHEN EXISTS (SELECT 1 FROM sys_events e WHERE replace(replace(replace(replace(lower(CASE WHEN typeof(e.id) = 'blob' THEN hex(e.id) ELSE CAST(e.id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') = replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.id) = 'blob' THEN hex(NEW.id) ELSE CAST(NEW.id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') AND e.id IS NOT NEW.id)
        THEN RAISE(ABORT, 'outbox event UUID representation conflicts')
        WHEN EXISTS (SELECT 1 FROM sys_events e WHERE e.id = NEW.id)
        THEN RAISE(IGNORE)
    END;
END;

CREATE TRIGGER outbox_event_envelope_guard BEFORE UPDATE ON sys_events
WHEN NEW.id IS NOT OLD.id OR NEW.rowid IS NOT OLD.rowid OR NEW.tenant_id IS NOT OLD.tenant_id
    OR NEW.event_type IS NOT OLD.event_type OR NEW.schema_version IS NOT OLD.schema_version
    OR NEW.payload IS NOT OLD.payload OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'outbox event envelope is immutable'); END;

CREATE TRIGGER outbox_event_retention_guard BEFORE DELETE ON sys_events
WHEN OLD.status NOT IN ('dispatched', 'failed') OR OLD.claimed_by IS NOT NULL
    OR OLD.claimed_at IS NOT NULL OR (OLD.status = 'dispatched' AND OLD.dispatched_at IS NULL)
    OR EXISTS (SELECT 1 FROM sys_event_consumers c WHERE c.event_type = OLD.event_type
    AND NOT EXISTS (SELECT 1 FROM sys_event_consumer_receipts a
        WHERE a.outbox_event_id = OLD.id AND a.consumer_key = c.consumer_key AND a.event_type = c.event_type))
BEGIN SELECT RAISE(ABORT, 'outbox event is not terminal, unclaimed and acknowledged'); END;
"#;

/// Protect the canonical event identity, including SQLite implicit REPLACE.
/// This is deliberately independent of the configurable retention age window.
pub async fn create_outbox_event_envelope_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    let nested = SchemaManager::new(&txn);
    let manager = &nested;
    let db = manager.get_connection();
    match manager.get_database_backend() {
        DbBackend::Postgres => {
            // The same registry -> event order as retention. Fail fast if
            // deployment did not drain all relevant transactions.
            db.execute_unprepared("LOCK TABLE sys_event_consumers, sys_events IN SHARE ROW EXCLUSIVE MODE NOWAIT").await?;
            drop_outbox_event_envelope_integrity(manager).await?;
            db.execute_unprepared(POSTGRES_EVENT_ENVELOPE_GUARD).await?;
            db.execute_unprepared("CREATE TRIGGER outbox_event_envelope_guard BEFORE UPDATE OR DELETE ON sys_events FOR EACH ROW EXECUTE FUNCTION outbox_guard_event_envelope()").await?;
        }
        DbBackend::Sqlite => {
            db.execute_unprepared("UPDATE sys_events SET retry_count = retry_count WHERE 1 = 0").await?;
            drop_outbox_event_envelope_integrity(manager).await?;
            db.execute_unprepared(SQLITE_EVENT_ENVELOPE_GUARDS).await?;
        }
        _ => return Err(DbErr::Migration("unsupported outbox envelope backend".into())),
    }
    txn.commit().await
}

async fn drop_outbox_event_envelope_integrity(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let statements: &[&str] = match manager.get_database_backend() {
        DbBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS outbox_event_envelope_guard ON sys_events",
            "DROP FUNCTION IF EXISTS outbox_guard_event_envelope()",
        ],
        DbBackend::Sqlite => &[
            "DROP TRIGGER IF EXISTS outbox_event_insert_identity_guard",
            "DROP TRIGGER IF EXISTS outbox_event_envelope_guard",
            "DROP TRIGGER IF EXISTS outbox_event_retention_guard",
            "DROP INDEX IF EXISTS uq_sys_events_uuid_identity",
        ],
        _ => return Err(DbErr::Migration("unsupported outbox envelope backend".into())),
    };
    for sql in statements { manager.get_connection().execute_unprepared(sql).await?; }
    Ok(())
}

const POSTGRES_RETENTION_POLICY_GUARDS: &str = r#"
CREATE OR REPLACE FUNCTION outbox_guard_event_retention_policy() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'outbox event retention policy singleton cannot be inserted or removed' USING ERRCODE = '23514';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
        OR (OLD.delivered_retention_ms IS NOT NULL AND NEW.delivered_retention_ms IS NULL) THEN
        RAISE EXCEPTION 'outbox retention policy cannot change identity or reset binding' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION outbox_guard_event_retention_age() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
    window_ms BIGINT;
    event_ms NUMERIC;
    now_ms BIGINT;
    floor_ms BIGINT;
BEGIN
    -- A canonical pruner holds this row before taking registry/event locks.
    -- Direct DELETE must not wait in the opposite order.
    IF current_setting('transaction_isolation') <> 'read committed' THEN
        RAISE EXCEPTION 'outbox retention age requires READ COMMITTED' USING ERRCODE = '23514';
    END IF;
    SELECT CASE WHEN OLD.status = 'dispatched' THEN delivered_retention_ms
                WHEN OLD.status = 'failed' THEN failed_retention_ms END
        INTO window_ms FROM outbox_event_retention_policy WHERE id = 1 FOR SHARE NOWAIT;
    IF NOT FOUND OR window_ms IS NULL THEN
        RAISE EXCEPTION 'outbox event retention window is not enabled' USING ERRCODE = '23514';
    END IF;
    SELECT high_water_ms INTO floor_ms FROM outbox_temporal_clock WHERE id = 1;
    now_ms := CAST(FLOOR(EXTRACT(EPOCH FROM clock_timestamp()) * 1000) AS BIGINT);
    event_ms := EXTRACT(EPOCH FROM CASE WHEN OLD.status = 'dispatched' THEN OLD.dispatched_at
                                      ELSE OLD.created_at END) * 1000;
    IF floor_ms IS NULL OR now_ms < floor_ms OR event_ms IS NULL
        OR NOT (event_ms <= now_ms - window_ms) THEN
        RAISE EXCEPTION 'outbox event retention age or clock authority is invalid' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
END $$;
"#;

const SQLITE_RETENTION_POLICY_GUARDS: &str = r#"
CREATE TRIGGER outbox_event_retention_policy_no_insert BEFORE INSERT ON outbox_event_retention_policy
BEGIN SELECT RAISE(ABORT, 'outbox event retention policy singleton cannot be inserted'); END;
CREATE TRIGGER outbox_event_retention_policy_no_delete BEFORE DELETE ON outbox_event_retention_policy
BEGIN SELECT RAISE(ABORT, 'outbox event retention policy singleton cannot be removed'); END;
CREATE TRIGGER outbox_event_retention_policy_guard BEFORE UPDATE ON outbox_event_retention_policy
WHEN NEW.id IS NOT OLD.id OR NEW.rowid IS NOT OLD.rowid
 OR (OLD.delivered_retention_ms IS NOT NULL AND NEW.delivered_retention_ms IS NULL)
 OR (NEW.delivered_retention_ms IS NOT NULL AND typeof(NEW.delivered_retention_ms) <> 'integer')
 OR (NEW.failed_retention_ms IS NOT NULL AND typeof(NEW.failed_retention_ms) <> 'integer')
BEGIN SELECT RAISE(ABORT, 'invalid outbox event retention policy update'); END;

CREATE TRIGGER outbox_event_retention_age_guard BEFORE DELETE ON sys_events
WHEN NOT EXISTS (
    SELECT 1 FROM outbox_event_retention_policy p CROSS JOIN outbox_temporal_clock c
    WHERE p.id = 1 AND c.id = 1 AND p.delivered_retention_ms IS NOT NULL
      AND CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) >= c.high_water_ms
      AND CAST(ROUND((julianday(CASE WHEN OLD.status = 'dispatched' THEN OLD.dispatched_at
                                    ELSE OLD.created_at END) - 2440587.5) * 86400000) AS INTEGER)
          <= CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
             - CASE WHEN OLD.status = 'dispatched' THEN p.delivered_retention_ms
                    WHEN OLD.status = 'failed' THEN p.failed_retention_ms END
)
BEGIN SELECT RAISE(ABORT, 'outbox event retention age or clock authority is invalid'); END;
"#;

/// Durable event-retention configuration. NULL delivered window means no
/// pruner has bound the database yet; NULL failed window means DLQ is retained.
/// Existing bindings are never reseeded by standalone schema setup.
pub async fn create_outbox_event_retention_policy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    let nested = SchemaManager::new(&txn);
    let manager = &nested;
    let db = manager.get_connection();
    let backend = manager.get_database_backend();
    if !manager.has_table("outbox_event_retention_policy").await? {
        db.execute_unprepared(
            "CREATE TABLE outbox_event_retention_policy (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                delivered_retention_ms BIGINT CHECK (delivered_retention_ms BETWEEN 1 AND 315360000000),
                failed_retention_ms BIGINT CHECK (failed_retention_ms BETWEEN 1 AND 315360000000),
                CHECK (delivered_retention_ms IS NOT NULL OR failed_retention_ms IS NULL)
            )",
        ).await?;
        db.execute_unprepared("INSERT INTO outbox_event_retention_policy (id) VALUES (1)").await?;
    }
    match backend {
        DbBackend::Postgres => {
            db.execute_unprepared("LOCK TABLE outbox_event_retention_policy, sys_events IN ACCESS EXCLUSIVE MODE NOWAIT").await?;
        }
        DbBackend::Sqlite => {
            db.execute_unprepared("UPDATE outbox_event_retention_policy SET delivered_retention_ms = delivered_retention_ms WHERE id = 1").await?;
        }
        _ => return Err(DbErr::Migration("unsupported outbox retention backend".into())),
    }
    let integer_guard = if backend == DbBackend::Sqlite {
        " AND (delivered_retention_ms IS NULL OR typeof(delivered_retention_ms) = 'integer') AND (failed_retention_ms IS NULL OR typeof(failed_retention_ms) = 'integer')"
    } else { "" };
    let row = db.query_one_raw(Statement::from_string(backend, format!(
        "SELECT COUNT(*) AS total, COUNT(CASE WHEN id = 1 AND (delivered_retention_ms IS NULL OR delivered_retention_ms BETWEEN 1 AND 315360000000) AND (failed_retention_ms IS NULL OR failed_retention_ms BETWEEN 1 AND 315360000000) AND (delivered_retention_ms IS NOT NULL OR failed_retention_ms IS NULL){integer_guard} THEN 1 END) AS valid FROM outbox_event_retention_policy"
    ))).await?.ok_or_else(|| DbErr::Migration("outbox retention policy validation unavailable".into()))?;
    if row.try_get::<i64>("", "total")? != 1 || row.try_get::<i64>("", "valid")? != 1 {
        return Err(DbErr::Migration("invalid outbox event retention policy singleton; offline recovery required".into()));
    }
    drop_outbox_event_retention_guards(manager).await?;
    match backend {
        DbBackend::Postgres => {
            db.execute_unprepared(POSTGRES_RETENTION_POLICY_GUARDS).await?;
            for sql in [
                "CREATE TRIGGER outbox_event_retention_policy_guard BEFORE INSERT OR UPDATE OR DELETE ON outbox_event_retention_policy FOR EACH ROW EXECUTE FUNCTION outbox_guard_event_retention_policy()",
                "CREATE TRIGGER outbox_event_retention_policy_no_truncate BEFORE TRUNCATE ON outbox_event_retention_policy FOR EACH STATEMENT EXECUTE FUNCTION outbox_guard_event_retention_policy()",
                "CREATE TRIGGER outbox_event_retention_age_guard BEFORE DELETE ON sys_events FOR EACH ROW EXECUTE FUNCTION outbox_guard_event_retention_age()",
            ] { db.execute_unprepared(sql).await?; }
        }
        DbBackend::Sqlite => db.execute_unprepared(SQLITE_RETENTION_POLICY_GUARDS).await.map(|_| ())?,
        _ => return Err(DbErr::Migration("unsupported outbox retention backend".into())),
    }
    txn.commit().await
}

async fn drop_outbox_event_retention_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let statements: &[&str] = match manager.get_database_backend() {
        DbBackend::Postgres => &[
            "DROP TRIGGER IF EXISTS outbox_event_retention_age_guard ON sys_events",
            "DROP TRIGGER IF EXISTS outbox_event_retention_policy_guard ON outbox_event_retention_policy",
            "DROP TRIGGER IF EXISTS outbox_event_retention_policy_no_truncate ON outbox_event_retention_policy",
            "DROP FUNCTION IF EXISTS outbox_guard_event_retention_age()",
            "DROP FUNCTION IF EXISTS outbox_guard_event_retention_policy()",
        ],
        DbBackend::Sqlite => &[
            "DROP TRIGGER IF EXISTS outbox_event_retention_age_guard",
            "DROP TRIGGER IF EXISTS outbox_event_retention_policy_guard",
            "DROP TRIGGER IF EXISTS outbox_event_retention_policy_no_insert",
            "DROP TRIGGER IF EXISTS outbox_event_retention_policy_no_delete",
        ],
        _ => return Err(DbErr::Migration("unsupported outbox retention backend".into())),
    };
    for sql in statements { manager.get_connection().execute_unprepared(sql).await?; }
    Ok(())
}

async fn drop_outbox_event_retention_policy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.has_table("outbox_event_retention_policy").await? {
        drop_outbox_event_retention_guards(manager).await?;
        manager.get_connection().execute_unprepared("DROP TABLE outbox_event_retention_policy").await?;
    }
    Ok(())
}

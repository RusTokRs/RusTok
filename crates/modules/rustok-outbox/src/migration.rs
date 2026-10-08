use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct SysEventsMigration;

#[async_trait::async_trait]
impl MigrationTrait for SysEventsMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_sys_events_schema(manager).await?;
        create_sys_events_claim_index(manager).await?;
        create_sys_events_retention_index(manager).await?;
        create_owner_operation_receipts_table(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
/// [`crate::retention::OutboxRetention::prune_once`] selects
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
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum SysEvents {
    Table,
    Id,
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

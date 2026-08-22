use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct SysEventsMigration;

#[async_trait::async_trait]
impl MigrationTrait for SysEventsMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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

        create_owner_operation_receipts_table(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_owner_operation_receipts_table(manager).await?;
        manager
            .drop_table(Table::drop().table(SysEvents::Table).to_owned())
            .await
    }
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
                .col(
                    ColumnDef::new(OwnerOperationReceipts::TenantId)
                        .uuid(),
                )
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

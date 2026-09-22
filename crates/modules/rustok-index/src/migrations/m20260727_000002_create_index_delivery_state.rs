use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IndexInbox::Table)
                    .col(ColumnDef::new(IndexInbox::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexInbox::SourceName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::DeliveryId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::MutationKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::ModuleName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::EntityName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexInbox::EntityId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexInbox::LocaleKey)
                            .string_len(32)
                            .not_null()
                            .default(""),
                    )
                    .col(super::source_version_column(
                        manager.get_database_backend(),
                        IndexInbox::SourceVersion,
                        false,
                    ))
                    .col(
                        ColumnDef::new(IndexInbox::PayloadHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::State)
                            .string_len(16)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::AvailableAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(IndexInbox::LeaseOwner).string_len(191))
                    .col(ColumnDef::new(IndexInbox::LeaseExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(IndexInbox::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(IndexInbox::ErrorCode).string_len(128))
                    .col(ColumnDef::new(IndexInbox::ErrorDetails).json_binary())
                    .col(
                        ColumnDef::new(IndexInbox::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexInbox::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_inbox")
                            .col(IndexInbox::TenantId)
                            .col(IndexInbox::SourceName)
                            .col(IndexInbox::DeliveryId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_inbox_tenant")
                            .from(IndexInbox::Table, IndexInbox::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("mutation_kind IN ('upsert', 'delete')"))
                    .check(Expr::cust("schema_version > 0"))
                    .check(Expr::cust("source_version >= 0"))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "length(source_name) BETWEEN 1 AND 128 AND source_name = trim(source_name)",
                    ))
                    .check(Expr::cust(
                        "length(delivery_id) BETWEEN 1 AND 191 AND delivery_id = trim(delivery_id)",
                    ))
                    .check(Expr::cust(
                        "length(module_name) BETWEEN 1 AND 128 AND module_name = trim(module_name)",
                    ))
                    .check(Expr::cust(
                        "length(entity_name) BETWEEN 1 AND 128 AND entity_name = trim(entity_name)",
                    ))
                    .check(Expr::cust(
                        "length(locale_key) <= 32 AND locale_key = trim(locale_key)",
                    ))
                    .check(Expr::cust(
                        "length(payload_hash) = 64 AND payload_hash = lower(payload_hash)",
                    ))
                    .check(Expr::cust(
                        "state IN ('pending', 'processing', 'applied', 'rejected')",
                    ))
                    .check(Expr::cust(
                        "(state = 'processing' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL) OR (state <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)",
                    ))
                    .check(Expr::cust(
                        "(state IN ('applied', 'rejected') AND completed_at IS NOT NULL) OR (state IN ('pending', 'processing') AND completed_at IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_inbox_claim")
                    .table(IndexInbox::Table)
                    .col(IndexInbox::TenantId)
                    .col(IndexInbox::State)
                    .col(IndexInbox::AvailableAt)
                    .col(IndexInbox::LeaseExpiresAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_inbox_entity")
                    .table(IndexInbox::Table)
                    .col(IndexInbox::TenantId)
                    .col(IndexInbox::ModuleName)
                    .col(IndexInbox::EntityName)
                    .col(IndexInbox::SchemaVersion)
                    .col(IndexInbox::EntityId)
                    .col(IndexInbox::LocaleKey)
                    .col(IndexInbox::SourceVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IndexCheckpoints::Table)
                    .col(ColumnDef::new(IndexCheckpoints::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexCheckpoints::CheckpointKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::SourceName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::ModuleName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::EntityName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::SchemaVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::LocaleKey)
                            .string_len(32)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::PartitionKey)
                            .string_len(191)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(IndexCheckpoints::Cursor)
                            .json_binary()
                            .not_null(),
                    )
                    .col(super::source_version_column(
                        manager.get_database_backend(),
                        IndexCheckpoints::SourceVersion,
                        true,
                    ))
                    .col(ColumnDef::new(IndexCheckpoints::LastDeliveryId).string_len(191))
                    .col(
                        ColumnDef::new(IndexCheckpoints::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_checkpoints")
                            .col(IndexCheckpoints::TenantId)
                            .col(IndexCheckpoints::CheckpointKind)
                            .col(IndexCheckpoints::SourceName)
                            .col(IndexCheckpoints::ModuleName)
                            .col(IndexCheckpoints::EntityName)
                            .col(IndexCheckpoints::SchemaVersion)
                            .col(IndexCheckpoints::LocaleKey)
                            .col(IndexCheckpoints::PartitionKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_checkpoints_tenant")
                            .from(IndexCheckpoints::Table, IndexCheckpoints::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("checkpoint_kind IN ('ingestion', 'rebuild')"))
                    .check(Expr::cust("schema_version > 0"))
                    .check(Expr::cust("source_version >= 0"))
                    .check(Expr::cust(
                        "length(source_name) BETWEEN 1 AND 128 AND source_name = trim(source_name)",
                    ))
                    .check(Expr::cust(
                        "length(module_name) BETWEEN 1 AND 128 AND module_name = trim(module_name)",
                    ))
                    .check(Expr::cust(
                        "length(entity_name) BETWEEN 1 AND 128 AND entity_name = trim(entity_name)",
                    ))
                    .check(Expr::cust(
                        "length(locale_key) <= 32 AND locale_key = trim(locale_key)",
                    ))
                    .check(Expr::cust(
                        "length(partition_key) <= 191 AND partition_key = trim(partition_key)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_checkpoints_updated")
                    .table(IndexCheckpoints::Table)
                    .col(IndexCheckpoints::TenantId)
                    .col(IndexCheckpoints::CheckpointKind)
                    .col(IndexCheckpoints::UpdatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(IndexCheckpoints::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(IndexInbox::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum IndexInbox {
    Table,
    TenantId,
    SourceName,
    DeliveryId,
    MutationKind,
    ModuleName,
    EntityName,
    SchemaVersion,
    EntityId,
    LocaleKey,
    SourceVersion,
    PayloadHash,
    State,
    AttemptCount,
    AvailableAt,
    LeaseOwner,
    LeaseExpiresAt,
    CompletedAt,
    ErrorCode,
    ErrorDetails,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum IndexCheckpoints {
    Table,
    TenantId,
    CheckpointKind,
    SourceName,
    ModuleName,
    EntityName,
    SchemaVersion,
    LocaleKey,
    PartitionKey,
    Cursor,
    SourceVersion,
    LastDeliveryId,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Tenants {
    Table,
    Id,
}

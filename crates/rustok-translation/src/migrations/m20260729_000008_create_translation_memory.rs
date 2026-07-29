use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MemoryEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryEntries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MemoryEntries::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(MemoryEntries::SourceLocale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::TargetLocale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::OwnerSlug)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::ResourceKind)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryEntries::ResourceId).text().not_null())
                    .col(ColumnDef::new(MemoryEntries::SubresourceId).text().null())
                    .col(
                        ColumnDef::new(MemoryEntries::FieldKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryEntries::SourceText).text().not_null())
                    .col(ColumnDef::new(MemoryEntries::TargetText).text().not_null())
                    .col(
                        ColumnDef::new(MemoryEntries::SourceKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::SourceHash)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::TargetHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::ContextFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::SegmentationVersion)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::Origin)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::QualityState)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::ReviewerActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::ReviewerActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryEntries::ProposalId).uuid().not_null())
                    .col(
                        ColumnDef::new(MemoryEntries::ApplyReceiptId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::RetentionPolicy)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::RetainUntil)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::TombstonedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::Revision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MemoryEntries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_memory_entries_tenant")
                            .from(MemoryEntries::Table, MemoryEntries::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("source_locale <> target_locale"))
                    .check(Expr::cust("source_locale <> 'und'"))
                    .check(Expr::cust("target_locale <> 'und'"))
                    .check(Expr::cust("length(source_text) > 0"))
                    .check(Expr::cust("length(target_text) > 0"))
                    .check(Expr::cust("origin IN ('manual', 'import', 'memory', 'ai')"))
                    .check(Expr::cust("quality_state = 'human_approved_applied'"))
                    .check(Expr::cust(
                        "reviewer_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust(
                        "retention_policy IN ('owner_lifecycle', 'retain_until', 'legal_hold')",
                    ))
                    .check(Expr::cust(
                        "(retention_policy = 'retain_until' AND retain_until IS NOT NULL) OR (retention_policy <> 'retain_until' AND retain_until IS NULL)",
                    ))
                    .check(Expr::cust("revision > 0"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_memory_proposal_field")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::TenantId)
                    .col(MemoryEntries::ProposalId)
                    .col(MemoryEntries::FieldKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_memory_lookup")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::TenantId)
                    .col(MemoryEntries::SourceLocale)
                    .col(MemoryEntries::TargetLocale)
                    .col(MemoryEntries::SourceKey)
                    .col(MemoryEntries::TombstonedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_memory_resource")
                    .table(MemoryEntries::Table)
                    .col(MemoryEntries::TenantId)
                    .col(MemoryEntries::OwnerSlug)
                    .col(MemoryEntries::ResourceKind)
                    .col(MemoryEntries::TombstonedAt)
                    .to_owned(),
            )
            .await?;
        create_receipts(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: applied translation provenance is durable workflow evidence.
        Ok(())
    }
}

async fn create_receipts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MemoryReceipts::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(MemoryReceipts::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(MemoryReceipts::TenantId).uuid().not_null())
                .col(ColumnDef::new(MemoryReceipts::EntryId).uuid().not_null())
                .col(
                    ColumnDef::new(MemoryReceipts::Operation)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MemoryReceipts::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MemoryReceipts::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MemoryReceipts::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MemoryReceipts::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MemoryReceipts::ResultingEntryRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(MemoryReceipts::Response).json().not_null())
                .col(
                    ColumnDef::new(MemoryReceipts::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_memory_receipts_tenant")
                        .from(MemoryReceipts::Table, MemoryReceipts::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "operation IN ('set_retention', 'tombstone', 'purge')",
                ))
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("resulting_entry_revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_memory_receipts_idempotency")
                .table(MemoryReceipts::Table)
                .col(MemoryReceipts::TenantId)
                .col(MemoryReceipts::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum MemoryEntries {
    #[iden = "translation_memory_entries"]
    Table,
    Id,
    TenantId,
    SourceLocale,
    TargetLocale,
    OwnerSlug,
    ResourceKind,
    ResourceId,
    SubresourceId,
    FieldKey,
    SourceText,
    TargetText,
    SourceKey,
    SourceHash,
    TargetHash,
    ContextFingerprint,
    SegmentationVersion,
    Origin,
    QualityState,
    ReviewerActorKind,
    ReviewerActorId,
    ProposalId,
    ApplyReceiptId,
    RetentionPolicy,
    RetainUntil,
    TombstonedAt,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MemoryReceipts {
    #[iden = "translation_memory_receipts"]
    Table,
    Id,
    TenantId,
    EntryId,
    Operation,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    ResultingEntryRevision,
    Response,
    CreatedAt,
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}

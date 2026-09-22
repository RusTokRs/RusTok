use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MachineOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MachineOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MachineOperations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(MachineOperations::ItemId).uuid().not_null())
                    .col(ColumnDef::new(MachineOperations::ProposalId).uuid().null())
                    .col(
                        ColumnDef::new(MachineOperations::Status)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::CommandHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::MachineRequestDigest)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::AdapterSlug)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::ProviderSlug)
                            .string_len(191)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::ProviderPolicyDigest)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::GlossaryRevision)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::GlossaryDigest)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::MemoryDigest)
                            .string_len(64)
                            .null(),
                    )
                    .col(ColumnDef::new(MachineOperations::ExecutionId).text().null())
                    .col(
                        ColumnDef::new(MachineOperations::ExecutionRequestDigest)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::PromptPolicyDigest)
                            .string_len(64)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::Attempts)
                            .json()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MachineOperations::Usage).json().null())
                    .col(
                        ColumnDef::new(MachineOperations::Diagnostics)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::ReviewRequired)
                            .boolean()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::RequestedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::RequestedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MachineOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_machine_operations_tenant")
                            .from(MachineOperations::Table, MachineOperations::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_machine_operations_item")
                            .from(MachineOperations::Table, MachineOperations::ItemId)
                            .to(TranslationJobItems::Table, TranslationJobItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_machine_operations_proposal")
                            .from(MachineOperations::Table, MachineOperations::ProposalId)
                            .to(TranslationProposals::Table, TranslationProposals::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust(
                        "status IN ('registered', 'saving', 'completed', 'cancelled')",
                    ))
                    .check(Expr::cust(
                        "requested_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust(
                        "(status IN ('registered', 'saving', 'cancelled') AND proposal_id IS NULL AND provider_slug IS NULL AND execution_id IS NULL AND execution_request_digest IS NULL AND prompt_policy_digest IS NULL AND usage IS NULL AND review_required IS NULL) OR (status = 'completed' AND proposal_id IS NOT NULL AND provider_slug IS NOT NULL AND execution_id IS NOT NULL AND execution_request_digest IS NOT NULL AND prompt_policy_digest IS NOT NULL AND usage IS NOT NULL AND review_required IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "(glossary_revision IS NULL AND glossary_digest IS NULL) OR (glossary_revision IS NOT NULL AND glossary_digest IS NOT NULL)",
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_machine_operations_idempotency")
                    .table(MachineOperations::Table)
                    .col(MachineOperations::TenantId)
                    .col(MachineOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_machine_operations_item")
                    .table(MachineOperations::Table)
                    .col(MachineOperations::TenantId)
                    .col(MachineOperations::ItemId)
                    .col(MachineOperations::CreatedAt)
                    .to_owned(),
            )
            .await?;
        create_memory_bindings(manager).await?;
        create_cancellations(manager).await?;
        create_recoveries(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: machine-translation execution provenance is durable workflow evidence.
        Ok(())
    }
}

async fn create_memory_bindings(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MachineMemoryBindings::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(MachineMemoryBindings::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::OperationId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::UnitId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::BatchOrdinal)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::UnitOrdinal)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::MemoryEntryId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::ScoreBasisPoints)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineMemoryBindings::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_memory_operation")
                        .from(
                            MachineMemoryBindings::Table,
                            MachineMemoryBindings::OperationId,
                        )
                        .to(MachineOperations::Table, MachineOperations::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_memory_entry")
                        .from(
                            MachineMemoryBindings::Table,
                            MachineMemoryBindings::MemoryEntryId,
                        )
                        .to(
                            TranslationMemoryEntries::Table,
                            TranslationMemoryEntries::Id,
                        )
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_memory_tenant")
                        .from(
                            MachineMemoryBindings::Table,
                            MachineMemoryBindings::TenantId,
                        )
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust("batch_ordinal >= 0 AND batch_ordinal < 500"))
                .check(Expr::cust("unit_ordinal >= 0 AND unit_ordinal < 5"))
                .check(Expr::cust(
                    "score_basis_points >= 0 AND score_basis_points <= 10000",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_memory_batch_ordinal")
                .table(MachineMemoryBindings::Table)
                .col(MachineMemoryBindings::OperationId)
                .col(MachineMemoryBindings::BatchOrdinal)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_memory_unit_ordinal")
                .table(MachineMemoryBindings::Table)
                .col(MachineMemoryBindings::OperationId)
                .col(MachineMemoryBindings::UnitId)
                .col(MachineMemoryBindings::UnitOrdinal)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_memory_unit_entry")
                .table(MachineMemoryBindings::Table)
                .col(MachineMemoryBindings::OperationId)
                .col(MachineMemoryBindings::UnitId)
                .col(MachineMemoryBindings::MemoryEntryId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_machine_memory_entry")
                .table(MachineMemoryBindings::Table)
                .col(MachineMemoryBindings::MemoryEntryId)
                .col(MachineMemoryBindings::TenantId)
                .to_owned(),
        )
        .await
}

async fn create_cancellations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MachineCancellations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(MachineCancellations::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::OperationId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::Reason)
                        .text()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::ProviderExecutionId)
                        .string_len(256)
                        .null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::ProviderStatus)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::ProviderErrorCode)
                        .string_len(128)
                        .null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::ProviderObservedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineCancellations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_cancellations_operation")
                        .from(
                            MachineCancellations::Table,
                            MachineCancellations::OperationId,
                        )
                        .to(MachineOperations::Table, MachineOperations::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_cancellations_tenant")
                        .from(MachineCancellations::Table, MachineCancellations::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("length(reason) > 0 AND length(reason) <= 4096"))
                .check(Expr::cust(
                    "provider_status IN ('unavailable', 'propagation_failed', 'cancellation_requested', 'completed', 'failed', 'cancelled')",
                ))
                .check(Expr::cust(
                    "(provider_status = 'propagation_failed' AND provider_error_code IS NOT NULL) OR (provider_status <> 'propagation_failed' AND provider_error_code IS NULL)",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_cancellations_operation")
                .table(MachineCancellations::Table)
                .col(MachineCancellations::OperationId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_cancellations_idempotency")
                .table(MachineCancellations::Table)
                .col(MachineCancellations::TenantId)
                .col(MachineCancellations::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_recoveries(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(MachineRecoveries::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(MachineRecoveries::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::OperationId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(ColumnDef::new(MachineRecoveries::Reason).text().not_null())
                .col(
                    ColumnDef::new(MachineRecoveries::ObservedUpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MachineRecoveries::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_recoveries_operation")
                        .from(MachineRecoveries::Table, MachineRecoveries::OperationId)
                        .to(MachineOperations::Table, MachineOperations::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_machine_recoveries_tenant")
                        .from(MachineRecoveries::Table, MachineRecoveries::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("length(reason) > 0 AND length(reason) <= 4096"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_recoveries_operation")
                .table(MachineRecoveries::Table)
                .col(MachineRecoveries::OperationId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_machine_recoveries_idempotency")
                .table(MachineRecoveries::Table)
                .col(MachineRecoveries::TenantId)
                .col(MachineRecoveries::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum MachineOperations {
    #[iden = "translation_machine_operations"]
    Table,
    Id,
    TenantId,
    ItemId,
    ProposalId,
    Status,
    CommandHash,
    MachineRequestDigest,
    AdapterSlug,
    ProviderSlug,
    ProviderPolicyDigest,
    GlossaryRevision,
    GlossaryDigest,
    MemoryDigest,
    ExecutionId,
    ExecutionRequestDigest,
    PromptPolicyDigest,
    Attempts,
    Usage,
    Diagnostics,
    ReviewRequired,
    RequestedByActorKind,
    RequestedByActorId,
    IdempotencyKey,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MachineMemoryBindings {
    #[iden = "translation_machine_memory_bindings"]
    Table,
    Id,
    TenantId,
    OperationId,
    UnitId,
    BatchOrdinal,
    UnitOrdinal,
    MemoryEntryId,
    ScoreBasisPoints,
    CreatedAt,
}

#[derive(Iden)]
enum MachineCancellations {
    #[iden = "translation_machine_cancellations"]
    Table,
    Id,
    TenantId,
    OperationId,
    Reason,
    RequestedByActorKind,
    RequestedByActorId,
    IdempotencyKey,
    RequestHash,
    ProviderExecutionId,
    ProviderStatus,
    ProviderErrorCode,
    ProviderObservedAt,
    CreatedAt,
}

#[derive(Iden)]
enum MachineRecoveries {
    #[iden = "translation_machine_recoveries"]
    Table,
    Id,
    TenantId,
    OperationId,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    Reason,
    ObservedUpdatedAt,
    CreatedAt,
}

#[derive(Iden)]
enum Tenants {
    Table,
    Id,
}

#[derive(Iden)]
enum TranslationJobItems {
    #[iden = "translation_job_items"]
    Table,
    Id,
}

#[derive(Iden)]
enum TranslationProposals {
    #[iden = "translation_proposals"]
    Table,
    Id,
}

#[derive(Iden)]
enum TranslationMemoryEntries {
    #[iden = "translation_memory_entries"]
    Table,
    Id,
}

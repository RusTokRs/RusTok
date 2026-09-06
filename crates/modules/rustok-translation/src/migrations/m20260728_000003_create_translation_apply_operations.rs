use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Items::Table)
                    .add_column(ColumnDef::new(Items::ActiveApplyOperationId).uuid().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(ApplyOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApplyOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ApplyOperations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ApplyOperations::ItemId).uuid().not_null())
                    .col(ColumnDef::new(ApplyOperations::ProposalId).uuid().not_null())
                    .col(
                        ColumnDef::new(ApplyOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ApplyOperations::Patch).json_binary().not_null())
                    .col(
                        ColumnDef::new(ApplyOperations::PatchDigest)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::Status)
                            .string_len(16)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::CreatedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::CreatedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::ApplyingItemRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::AttemptCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::LastErrorKind)
                            .string_len(32)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::LastErrorCode)
                            .string_len(191)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::LastErrorRetryable)
                            .boolean()
                            .null(),
                    )
                    .col(ColumnDef::new(ApplyOperations::LeaseToken).uuid().null())
                    .col(
                        ColumnDef::new(ApplyOperations::LeaseOwnerActorKind)
                            .string_len(16)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::LeaseOwnerActorId)
                            .string_len(191)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ApplyOperations::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_apply_operations_tenant_item")
                            .from(ApplyOperations::Table, ApplyOperations::TenantId)
                            .from_col(ApplyOperations::ItemId)
                            .to(Items::Table, Items::TenantId)
                            .to_col(Items::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_apply_operations_tenant_proposal")
                            .from(ApplyOperations::Table, ApplyOperations::TenantId)
                            .from_col(ApplyOperations::ProposalId)
                            .to(Proposals::Table, Proposals::TenantId)
                            .to_col(Proposals::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust(
                        "status IN ('pending', 'completed', 'conflict', 'failed')",
                    ))
                    .check(Expr::cust(
                        "created_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust("applying_item_revision > 0"))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "(last_error_kind IS NULL AND last_error_code IS NULL AND last_error_retryable IS NULL) OR (last_error_kind IS NOT NULL AND last_error_code IS NOT NULL AND last_error_retryable IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "(lease_token IS NULL AND lease_owner_actor_kind IS NULL AND lease_owner_actor_id IS NULL AND lease_expires_at IS NULL) OR (lease_token IS NOT NULL AND lease_owner_actor_kind IN ('user', 'service', 'system') AND lease_owner_actor_id IS NOT NULL AND lease_expires_at IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "(status = 'completed' AND completed_at IS NOT NULL) OR (status <> 'completed' AND completed_at IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_apply_operations_tenant_id")
                    .table(ApplyOperations::Table)
                    .col(ApplyOperations::TenantId)
                    .col(ApplyOperations::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_apply_operations_idempotency")
                    .table(ApplyOperations::Table)
                    .col(ApplyOperations::TenantId)
                    .col(ApplyOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_apply_operations_item_status")
                    .table(ApplyOperations::Table)
                    .col(ApplyOperations::TenantId)
                    .col(ApplyOperations::ItemId)
                    .col(ApplyOperations::Status)
                    .col(ApplyOperations::UpdatedAt)
                    .to_owned(),
            )
            .await?;
        create_recoveries(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: apply intents and outcome evidence are durable audit data.
        Ok(())
    }
}

async fn create_recoveries(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ApplyRecoveries::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ApplyRecoveries::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(ApplyRecoveries::TenantId).uuid().not_null())
                .col(
                    ColumnDef::new(ApplyRecoveries::OperationId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::Reason)
                        .string_len(500)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::ObservedAttemptCount)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApplyRecoveries::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_apply_recoveries_tenant_operation")
                        .from(ApplyRecoveries::Table, ApplyRecoveries::TenantId)
                        .from_col(ApplyRecoveries::OperationId)
                        .to(ApplyOperations::Table, ApplyOperations::TenantId)
                        .to_col(ApplyOperations::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("observed_attempt_count >= 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_apply_recoveries_idempotency")
                .table(ApplyRecoveries::Table)
                .col(ApplyRecoveries::TenantId)
                .col(ApplyRecoveries::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_apply_recoveries_operation")
                .table(ApplyRecoveries::Table)
                .col(ApplyRecoveries::TenantId)
                .col(ApplyRecoveries::OperationId)
                .col(ApplyRecoveries::CreatedAt)
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum Items {
    #[iden = "translation_job_items"]
    Table,
    Id,
    TenantId,
    ActiveApplyOperationId,
}

#[derive(Iden)]
enum Proposals {
    #[iden = "translation_proposals"]
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum ApplyOperations {
    #[iden = "translation_apply_operations"]
    Table,
    Id,
    TenantId,
    ItemId,
    ProposalId,
    IdempotencyKey,
    RequestHash,
    Patch,
    PatchDigest,
    Status,
    CreatedByActorKind,
    CreatedByActorId,
    ApplyingItemRevision,
    AttemptCount,
    LastErrorKind,
    LastErrorCode,
    LastErrorRetryable,
    LeaseToken,
    LeaseOwnerActorKind,
    LeaseOwnerActorId,
    LeaseExpiresAt,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum ApplyRecoveries {
    #[iden = "translation_apply_recoveries"]
    Table,
    Id,
    TenantId,
    OperationId,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    Reason,
    ObservedAttemptCount,
    CreatedAt,
}

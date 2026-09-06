use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_progress(manager).await?;
        create_retries(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: progress is rebuildable, but retry rows are durable
        // operator audit data and cannot be silently discarded.
        Ok(())
    }
}

async fn create_progress(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    table
        .table(Progress::Table)
        .if_not_exists()
        .col(ColumnDef::new(Progress::Id).uuid().not_null().primary_key())
        .col(ColumnDef::new(Progress::TenantId).uuid().not_null())
        .col(ColumnDef::new(Progress::JobId).uuid().not_null())
        .col(
            ColumnDef::new(Progress::SourceDigest)
                .string_len(64)
                .not_null(),
        );
    for column in [
        Progress::TotalItems,
        Progress::AssignedItems,
        Progress::TerminalItems,
        Progress::MissingItems,
        Progress::DraftItems,
        Progress::InReviewItems,
        Progress::ApprovedItems,
        Progress::ApplyingItems,
        Progress::AppliedItems,
        Progress::StaleItems,
        Progress::ConflictItems,
        Progress::BlockedItems,
        Progress::ExcludedItems,
        Progress::CancelledItems,
        Progress::RequiredUnits,
        Progress::OptionalUnits,
        Progress::AppliedRequiredUnits,
        Progress::AppliedOptionalUnits,
        Progress::ApprovedRequiredUnits,
        Progress::ApprovedOptionalUnits,
        Progress::CompleteResources,
        Progress::SourceCharacters,
        Progress::TranslatedCharacters,
        Progress::Revision,
    ] {
        table.col(ColumnDef::new(column).big_integer().not_null().default(0));
    }
    table
        .col(
            ColumnDef::new(Progress::UpdatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_translation_job_progress_tenant_job")
                .from(Progress::Table, Progress::TenantId)
                .from_col(Progress::JobId)
                .to(Jobs::Table, Jobs::TenantId)
                .to_col(Jobs::Id)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .check(Expr::cust("length(source_digest) = 64"))
        .check(Expr::cust(
            "total_items = missing_items + draft_items + in_review_items + approved_items + applying_items + applied_items + stale_items + conflict_items + blocked_items + excluded_items + cancelled_items",
        ))
        .check(Expr::cust(
            "terminal_items = applied_items + excluded_items + cancelled_items",
        ))
        .check(Expr::cust(
            "assigned_items >= 0 AND assigned_items <= total_items",
        ))
        .check(Expr::cust(
            "applied_required_units >= 0 AND applied_required_units <= required_units",
        ))
        .check(Expr::cust(
            "applied_optional_units >= 0 AND applied_optional_units <= optional_units",
        ))
        .check(Expr::cust(
            "approved_required_units >= 0 AND approved_required_units <= required_units",
        ))
        .check(Expr::cust(
            "approved_optional_units >= 0 AND approved_optional_units <= optional_units",
        ))
        .check(Expr::cust(
            "complete_resources >= 0 AND complete_resources <= total_items",
        ))
        .check(Expr::cust(
            "total_items >= 0 AND terminal_items >= 0 AND missing_items >= 0 AND draft_items >= 0 AND in_review_items >= 0 AND approved_items >= 0 AND applying_items >= 0 AND applied_items >= 0 AND stale_items >= 0 AND conflict_items >= 0 AND blocked_items >= 0 AND excluded_items >= 0 AND cancelled_items >= 0",
        ))
        .check(Expr::cust(
            "required_units >= 0 AND optional_units >= 0 AND source_characters >= 0 AND translated_characters >= 0 AND revision >= 0",
        ));
    manager.create_table(table.to_owned()).await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_job_progress_job")
                .table(Progress::Table)
                .col(Progress::TenantId)
                .col(Progress::JobId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_job_progress_updated")
                .table(Progress::Table)
                .col(Progress::TenantId)
                .col(Progress::UpdatedAt)
                .to_owned(),
        )
        .await
}

async fn create_retries(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Retries::Table)
                .if_not_exists()
                .col(ColumnDef::new(Retries::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Retries::TenantId).uuid().not_null())
                .col(ColumnDef::new(Retries::ItemId).uuid().not_null())
                .col(
                    ColumnDef::new(Retries::PriorStatus)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::ResultingStatus)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(ColumnDef::new(Retries::Reason).string_len(500).not_null())
                .col(
                    ColumnDef::new(Retries::ResultingItemRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Retries::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_item_retries_tenant_item")
                        .from(Retries::Table, Retries::TenantId)
                        .from_col(Retries::ItemId)
                        .to(Items::Table, Items::TenantId)
                        .to_col(Items::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust("prior_status = 'blocked'"))
                .check(Expr::cust("resulting_status = 'approved'"))
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("resulting_item_revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_item_retries_idempotency")
                .table(Retries::Table)
                .col(Retries::TenantId)
                .col(Retries::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_item_retries_item")
                .table(Retries::Table)
                .col(Retries::TenantId)
                .col(Retries::ItemId)
                .col(Retries::CreatedAt)
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum Jobs {
    #[iden = "translation_jobs"]
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum Items {
    #[iden = "translation_job_items"]
    Table,
    Id,
    TenantId,
}

#[derive(Iden, Clone, Copy)]
enum Progress {
    #[iden = "translation_job_progress"]
    Table,
    Id,
    TenantId,
    JobId,
    SourceDigest,
    TotalItems,
    AssignedItems,
    TerminalItems,
    MissingItems,
    DraftItems,
    InReviewItems,
    ApprovedItems,
    ApplyingItems,
    AppliedItems,
    StaleItems,
    ConflictItems,
    BlockedItems,
    ExcludedItems,
    CancelledItems,
    RequiredUnits,
    OptionalUnits,
    AppliedRequiredUnits,
    AppliedOptionalUnits,
    ApprovedRequiredUnits,
    ApprovedOptionalUnits,
    CompleteResources,
    SourceCharacters,
    TranslatedCharacters,
    Revision,
    UpdatedAt,
}

#[derive(Iden)]
enum Retries {
    #[iden = "translation_item_retries"]
    Table,
    Id,
    TenantId,
    ItemId,
    PriorStatus,
    ResultingStatus,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    Reason,
    ResultingItemRevision,
    CreatedAt,
}

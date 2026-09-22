use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_assignments(manager).await?;
        create_cancellations(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: assignment and cancellation rows are durable audit data.
        Ok(())
    }
}

async fn create_assignments(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Assignments::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Assignments::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Assignments::TenantId).uuid().not_null())
                .col(ColumnDef::new(Assignments::ItemId).uuid().not_null())
                .col(
                    ColumnDef::new(Assignments::Operation)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::AssigneeActorKind)
                        .string_len(16)
                        .null(),
                )
                .col(
                    ColumnDef::new(Assignments::AssigneeActorId)
                        .string_len(191)
                        .null(),
                )
                .col(
                    ColumnDef::new(Assignments::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::ResultingItemRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Assignments::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_item_assignments_tenant_item")
                        .from(Assignments::Table, Assignments::TenantId)
                        .from_col(Assignments::ItemId)
                        .to(Items::Table, Items::TenantId)
                        .to_col(Items::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust("operation IN ('assign', 'unassign')"))
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust(
                    "(operation = 'assign' AND assignee_actor_kind IN ('user', 'service', 'system') AND assignee_actor_id IS NOT NULL) OR (operation = 'unassign' AND assignee_actor_kind IS NULL AND assignee_actor_id IS NULL)",
                ))
                .check(Expr::cust("resulting_item_revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_item_assignments_idempotency")
                .table(Assignments::Table)
                .col(Assignments::TenantId)
                .col(Assignments::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_translation_item_assignments_item")
                .table(Assignments::Table)
                .col(Assignments::TenantId)
                .col(Assignments::ItemId)
                .col(Assignments::CreatedAt)
                .to_owned(),
        )
        .await
}

async fn create_cancellations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Cancellations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Cancellations::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Cancellations::TenantId).uuid().not_null())
                .col(ColumnDef::new(Cancellations::JobId).uuid().not_null())
                .col(
                    ColumnDef::new(Cancellations::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::RequestHash)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::RequestedByActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::RequestedByActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::Reason)
                        .string_len(500)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::ResultingJobRevision)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::CancelledItemCount)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Cancellations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_translation_job_cancellations_tenant_job")
                        .from(Cancellations::Table, Cancellations::TenantId)
                        .from_col(Cancellations::JobId)
                        .to(Jobs::Table, Jobs::TenantId)
                        .to_col(Jobs::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust(
                    "requested_by_actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("resulting_job_revision > 0"))
                .check(Expr::cust("cancelled_item_count >= 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_job_cancellations_idempotency")
                .table(Cancellations::Table)
                .col(Cancellations::TenantId)
                .col(Cancellations::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_translation_job_cancellations_job")
                .table(Cancellations::Table)
                .col(Cancellations::TenantId)
                .col(Cancellations::JobId)
                .unique()
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
}

#[derive(Iden)]
enum Jobs {
    #[iden = "translation_jobs"]
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum Assignments {
    #[iden = "translation_item_assignments"]
    Table,
    Id,
    TenantId,
    ItemId,
    Operation,
    AssigneeActorKind,
    AssigneeActorId,
    RequestedByActorKind,
    RequestedByActorId,
    IdempotencyKey,
    RequestHash,
    ResultingItemRevision,
    CreatedAt,
}

#[derive(Iden)]
enum Cancellations {
    #[iden = "translation_job_cancellations"]
    Table,
    Id,
    TenantId,
    JobId,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    Reason,
    ResultingJobRevision,
    CancelledItemCount,
    CreatedAt,
}

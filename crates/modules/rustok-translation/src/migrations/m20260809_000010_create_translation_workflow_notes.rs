use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WorkflowNotes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WorkflowNotes::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(WorkflowNotes::TenantId).uuid().not_null())
                    .col(ColumnDef::new(WorkflowNotes::JobId).uuid().not_null())
                    .col(ColumnDef::new(WorkflowNotes::ItemId).uuid())
                    .col(ColumnDef::new(WorkflowNotes::Body).text().not_null())
                    .col(
                        ColumnDef::new(WorkflowNotes::CreatedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::CreatedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(WorkflowNotes::ResolvedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(WorkflowNotes::ResolvedByActorKind).string_len(16),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::ResolvedByActorId).string_len(191),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::ResolutionIdempotencyKey)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::ResolutionRequestHash).string_len(64),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(WorkflowNotes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_workflow_notes_tenant_job")
                            .from(WorkflowNotes::Table, WorkflowNotes::TenantId)
                            .from_col(WorkflowNotes::JobId)
                            .to(Jobs::Table, Jobs::TenantId)
                            .to_col(Jobs::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_workflow_notes_tenant_item")
                            .from(WorkflowNotes::Table, WorkflowNotes::TenantId)
                            .from_col(WorkflowNotes::ItemId)
                            .to(Items::Table, Items::TenantId)
                            .to_col(Items::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("body <> ''"))
                    .check(Expr::cust(
                        "created_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust("revision >= 0"))
                    .check(Expr::cust(
                        "(resolved_at IS NULL AND resolved_by_actor_kind IS NULL AND resolved_by_actor_id IS NULL AND resolution_idempotency_key IS NULL AND resolution_request_hash IS NULL) OR (resolved_at IS NOT NULL AND resolved_by_actor_kind IN ('user', 'service', 'system') AND resolved_by_actor_id IS NOT NULL AND resolution_idempotency_key IS NOT NULL AND resolution_request_hash IS NOT NULL)",
                    ))
                    .to_owned(),
            )
            .await?;
        for (name, columns) in [
            (
                "uq_translation_workflow_notes_tenant_id",
                vec![WorkflowNotes::TenantId, WorkflowNotes::Id],
            ),
            (
                "uq_translation_workflow_notes_create_idempotency",
                vec![WorkflowNotes::TenantId, WorkflowNotes::IdempotencyKey],
            ),
            (
                "uq_translation_workflow_notes_resolution_idempotency",
                vec![
                    WorkflowNotes::TenantId,
                    WorkflowNotes::ResolutionIdempotencyKey,
                ],
            ),
        ] {
            let mut index = Index::create();
            index.name(name).table(WorkflowNotes::Table);
            for column in columns {
                index.col(column);
            }
            manager.create_index(index.unique().to_owned()).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_workflow_notes_job_feed")
                    .table(WorkflowNotes::Table)
                    .col(WorkflowNotes::TenantId)
                    .col(WorkflowNotes::JobId)
                    .col(WorkflowNotes::ItemId)
                    .col(WorkflowNotes::ResolvedAt)
                    .col(WorkflowNotes::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: workflow notes are durable tenant audit records.
        Ok(())
    }
}

#[derive(Iden)]
enum WorkflowNotes {
    #[iden = "translation_workflow_notes"]
    Table,
    Id,
    TenantId,
    JobId,
    ItemId,
    Body,
    CreatedByActorKind,
    CreatedByActorId,
    IdempotencyKey,
    RequestHash,
    Revision,
    ResolvedAt,
    ResolvedByActorKind,
    ResolvedByActorId,
    ResolutionIdempotencyKey,
    ResolutionRequestHash,
    CreatedAt,
    UpdatedAt,
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

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ContentOrchestrationOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::Operation)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::IdempotencyKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::SourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::TargetId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::MovedComments)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content_orchestration_ops_idempotency")
                    .table(ContentOrchestrationOperations::Table)
                    .col(ContentOrchestrationOperations::TenantId)
                    .col(ContentOrchestrationOperations::Operation)
                    .col(ContentOrchestrationOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ContentOrchestrationAuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::Operation)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::IdempotencyKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ContentOrchestrationAuditLogs::ActorId).uuid())
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::SourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::TargetId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::Payload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentOrchestrationAuditLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ContentOrchestrationAuditLogs::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ContentOrchestrationOperations::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum ContentOrchestrationOperations {
    Table,
    Id,
    TenantId,
    Operation,
    IdempotencyKey,
    SourceId,
    TargetId,
    MovedComments,
    CreatedAt,
}

#[derive(Iden)]
enum ContentOrchestrationAuditLogs {
    Table,
    Id,
    TenantId,
    Operation,
    IdempotencyKey,
    ActorId,
    SourceId,
    TargetId,
    Payload,
    CreatedAt,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct ScriptExecutionsMigration;

#[async_trait::async_trait]
impl MigrationTrait for ScriptExecutionsMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScriptExecutions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScriptExecutions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ScriptExecutions::ScriptId).uuid().not_null())
                    .col(
                        ColumnDef::new(ScriptExecutions::ScriptName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScriptExecutions::Phase)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScriptExecutions::Outcome)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScriptExecutions::DurationMs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ScriptExecutions::Error).text())
                    .col(ColumnDef::new(ScriptExecutions::UserId).string_len(255))
                    .col(ColumnDef::new(ScriptExecutions::TenantId).uuid())
                    .col(ColumnDef::new(ScriptExecutions::SourceRevision).integer())
                    .col(ColumnDef::new(ScriptExecutions::SourceDigest).string_len(71))
                    .col(ColumnDef::new(ScriptExecutions::PolicyDigest).string_len(71))
                    .col(ColumnDef::new(ScriptExecutions::Executor).string_len(32))
                    .col(ColumnDef::new(ScriptExecutions::RuntimeAbi).string_len(128))
                    .col(
                        ColumnDef::new(ScriptExecutions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_script_executions_script_id")
                    .table(ScriptExecutions::Table)
                    .col(ScriptExecutions::ScriptId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_script_executions_created_at")
                    .table(ScriptExecutions::Table)
                    .col(ScriptExecutions::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScriptExecutions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ScriptExecutions {
    Table,
    Id,
    ScriptId,
    ScriptName,
    Phase,
    Outcome,
    DurationMs,
    Error,
    UserId,
    TenantId,
    SourceRevision,
    SourceDigest,
    PolicyDigest,
    Executor,
    RuntimeAbi,
    CreatedAt,
}

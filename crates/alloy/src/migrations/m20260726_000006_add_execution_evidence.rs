use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ScriptExecutions::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ScriptExecutions::SourceRevision).integer(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(ScriptExecutions::SourceDigest).string_len(71),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(ScriptExecutions::PolicyDigest).string_len(71),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(ScriptExecutions::Executor).string_len(32),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(ScriptExecutions::RuntimeAbi).string_len(128),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, column) in [
            ("runtime_abi", ScriptExecutions::RuntimeAbi),
            ("executor", ScriptExecutions::Executor),
            ("policy_digest", ScriptExecutions::PolicyDigest),
            ("source_digest", ScriptExecutions::SourceDigest),
            ("source_revision", ScriptExecutions::SourceRevision),
        ] {
            if manager.has_column("script_executions", name).await? {
                manager
                    .alter_table(
                        Table::alter()
                            .table(ScriptExecutions::Table)
                            .drop_column(column)
                            .to_owned(),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

#[derive(DeriveIden, Clone, Copy)]
enum ScriptExecutions {
    Table,
    SourceRevision,
    SourceDigest,
    PolicyDigest,
    Executor,
    RuntimeAbi,
}

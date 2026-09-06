use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("script_executions", "source_revision")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScriptExecutions::Table)
                        .add_column(ColumnDef::new(ScriptExecutions::SourceRevision).integer())
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("script_executions", "source_digest")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScriptExecutions::Table)
                        .add_column(ColumnDef::new(ScriptExecutions::SourceDigest).string_len(71))
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("script_executions", "policy_digest")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScriptExecutions::Table)
                        .add_column(ColumnDef::new(ScriptExecutions::PolicyDigest).string_len(71))
                        .to_owned(),
                )
                .await?;
        }
        if !manager.has_column("script_executions", "executor").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScriptExecutions::Table)
                        .add_column(ColumnDef::new(ScriptExecutions::Executor).string_len(32))
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("script_executions", "runtime_abi")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(ScriptExecutions::Table)
                        .add_column(ColumnDef::new(ScriptExecutions::RuntimeAbi).string_len(128))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
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

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add failure_count column
        manager
            .alter_table(
                Table::alter()
                    .table(Workflows::Table)
                    .add_column(
                        ColumnDef::new(Workflows::FailureCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Add auto_disabled_at column
        manager
            .alter_table(
                Table::alter()
                    .table(Workflows::Table)
                    .add_column(
                        ColumnDef::new(Workflows::AutoDisabledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Workflows::Table)
                    .drop_column(Workflows::AutoDisabledAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Workflows::Table)
                    .drop_column(Workflows::FailureCount)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Workflows {
    Table,
    FailureCount,
    AutoDisabledAt,
}

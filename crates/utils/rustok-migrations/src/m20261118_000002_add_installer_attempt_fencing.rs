use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(InstallSessions::Table)
                    .add_column(
                        ColumnDef::new(InstallSessions::AttemptGeneration)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(InstallStepReceipts::Table)
                    .add_column(
                        ColumnDef::new(InstallStepReceipts::AttemptGeneration)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(InstallStepReceipts::Table)
                    .drop_column(InstallStepReceipts::AttemptGeneration)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(InstallSessions::Table)
                    .drop_column(InstallSessions::AttemptGeneration)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum InstallSessions {
    Table,
    AttemptGeneration,
}

#[derive(DeriveIden)]
enum InstallStepReceipts {
    Table,
    AttemptGeneration,
}

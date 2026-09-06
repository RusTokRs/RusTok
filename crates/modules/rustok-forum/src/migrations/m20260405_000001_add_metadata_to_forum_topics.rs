use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ForumTopics::Table)
                    .add_column(
                        ColumnDef::new(ForumTopics::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ForumTopics::Table)
                    .drop_column(ForumTopics::Metadata)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ForumTopics {
    Table,
    Metadata,
}

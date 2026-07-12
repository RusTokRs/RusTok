use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RegistryPublishRequests::Table)
                    .add_column(
                        ColumnDef::new(RegistryPublishRequests::PublisherIdentity)
                            .string_len(128)
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
                    .table(RegistryPublishRequests::Table)
                    .drop_column(RegistryPublishRequests::PublisherIdentity)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    PublisherIdentity,
}

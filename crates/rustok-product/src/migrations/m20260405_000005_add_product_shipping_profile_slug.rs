use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Products::ShippingProfileSlug).string_len(100),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Products::Table)
                    .drop_column(Products::ShippingProfileSlug)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Products {
    Table,
    ShippingProfileSlug,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(ColumnDef::new(Carts::RegionId).uuid())
                    .add_column(ColumnDef::new(Carts::CountryCode).string_len(2))
                    .add_column(ColumnDef::new(Carts::LocaleCode).string_len(10))
                    .add_column(ColumnDef::new(Carts::SelectedShippingOptionId).uuid())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::SelectedShippingOptionId)
                    .drop_column(Carts::LocaleCode)
                    .drop_column(Carts::CountryCode)
                    .drop_column(Carts::RegionId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Carts {
    Table,
    RegionId,
    CountryCode,
    LocaleCode,
    SelectedShippingOptionId,
}

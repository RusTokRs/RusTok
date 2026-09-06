use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut region_id = ColumnDef::new(Carts::RegionId);
        region_id.uuid();
        add_column_if_missing(manager, Carts::Table, region_id).await?;

        let mut country_code = ColumnDef::new(Carts::CountryCode);
        country_code.string_len(2);
        add_column_if_missing(manager, Carts::Table, country_code).await?;

        let mut locale_code = ColumnDef::new(Carts::LocaleCode);
        locale_code.string_len(10);
        add_column_if_missing(manager, Carts::Table, locale_code).await?;

        let mut selected_shipping_option_id = ColumnDef::new(Carts::SelectedShippingOptionId);
        selected_shipping_option_id.uuid();
        add_column_if_missing(manager, Carts::Table, selected_shipping_option_id).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column_if_present(manager, Carts::Table, Carts::SelectedShippingOptionId).await?;
        drop_column_if_present(manager, Carts::Table, Carts::LocaleCode).await?;
        drop_column_if_present(manager, Carts::Table, Carts::CountryCode).await?;
        drop_column_if_present(manager, Carts::Table, Carts::RegionId).await
    }
}

async fn add_column_if_missing<T>(
    manager: &SchemaManager<'_>,
    table: T,
    column: ColumnDef,
) -> Result<(), DbErr>
where
    T: Iden + 'static,
{
    manager
        .alter_table(
            Table::alter()
                .table(table)
                .add_column_if_not_exists(column)
                .to_owned(),
        )
        .await
}

async fn drop_column_if_present<T, C>(
    manager: &SchemaManager<'_>,
    table: T,
    column: C,
) -> Result<(), DbErr>
where
    T: Iden + 'static,
    C: IntoIden,
{
    manager
        .alter_table(Table::alter().table(table).drop_column(column).to_owned())
        .await
}

#[derive(Iden)]
enum Carts {
    Table,
    RegionId,
    CountryCode,
    LocaleCode,
    SelectedShippingOptionId,
}

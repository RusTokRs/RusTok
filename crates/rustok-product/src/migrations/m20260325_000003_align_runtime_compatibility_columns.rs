use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Transitional compatibility columns for the legacy/ecommerce umbrella runtime.
        // These keep the split module working on real migrations while the remaining
        // product option/image/translation normalization backlog is closed.
        let mut vendor = ColumnDef::new(Products::Vendor);
        vendor.string_len(255);
        add_column_if_missing(manager, Products::Table, vendor).await?;
        let mut product_type = ColumnDef::new(Products::ProductType);
        product_type.string_len(255);
        add_column_if_missing(manager, Products::Table, product_type).await?;

        let mut meta_title = ColumnDef::new(ProductTranslations::MetaTitle);
        meta_title.string_len(255);
        add_column_if_missing(manager, ProductTranslations::Table, meta_title).await?;
        let mut meta_description = ColumnDef::new(ProductTranslations::MetaDescription);
        meta_description.text();
        add_column_if_missing(manager, ProductTranslations::Table, meta_description).await?;

        let mut alt_text = ColumnDef::new(ProductImages::AltText);
        alt_text.text();
        add_column_if_missing(manager, ProductImages::Table, alt_text).await?;

        let mut option_name = ColumnDef::new(ProductOptions::Name);
        option_name.string_len(100).not_null().default("");
        add_column_if_missing(manager, ProductOptions::Table, option_name).await?;
        let mut option_values = ColumnDef::new(ProductOptions::Values);
        option_values.json_binary().not_null().default("[]");
        add_column_if_missing(manager, ProductOptions::Table, option_values).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column_if_present(manager, ProductOptions::Table, ProductOptions::Values).await?;
        drop_column_if_present(manager, ProductOptions::Table, ProductOptions::Name).await?;
        drop_column_if_present(manager, ProductImages::Table, ProductImages::AltText).await?;
        drop_column_if_present(
            manager,
            ProductTranslations::Table,
            ProductTranslations::MetaDescription,
        )
        .await?;
        drop_column_if_present(
            manager,
            ProductTranslations::Table,
            ProductTranslations::MetaTitle,
        )
        .await?;
        drop_column_if_present(manager, Products::Table, Products::ProductType).await?;
        drop_column_if_present(manager, Products::Table, Products::Vendor).await
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
enum Products {
    Table,
    Vendor,
    ProductType,
}

#[derive(Iden)]
enum ProductTranslations {
    Table,
    MetaTitle,
    MetaDescription,
}

#[derive(Iden)]
enum ProductImages {
    Table,
    AltText,
}

#[derive(Iden)]
enum ProductOptions {
    Table,
    Name,
    Values,
}

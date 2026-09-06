use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::ShippingProfileSlug).string_len(100),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::ShippingProfileSlug)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ProductVariants {
    Table,
    ShippingProfileSlug,
}

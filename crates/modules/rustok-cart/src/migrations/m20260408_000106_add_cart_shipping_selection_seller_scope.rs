use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CartShippingSelections::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(CartShippingSelections::SellerScope).string_len(100),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_cart_shipping_selections_cart_profile")
                    .table(CartShippingSelections::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_shipping_selections_cart_profile_seller")
                    .table(CartShippingSelections::Table)
                    .col(CartShippingSelections::CartId)
                    .col(CartShippingSelections::ShippingProfileSlug)
                    .col(CartShippingSelections::SellerScope)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cart_shipping_selections_cart_profile_seller")
                    .table(CartShippingSelections::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_shipping_selections_cart_profile")
                    .table(CartShippingSelections::Table)
                    .col(CartShippingSelections::CartId)
                    .col(CartShippingSelections::ShippingProfileSlug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartShippingSelections::Table)
                    .drop_column(CartShippingSelections::SellerScope)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CartShippingSelections {
    Table,
    CartId,
    ShippingProfileSlug,
    SellerScope,
}

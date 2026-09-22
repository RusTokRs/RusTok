use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut shipping_profile_slug = ColumnDef::new(CartLineItems::ShippingProfileSlug);
        shipping_profile_slug
            .string_len(100)
            .not_null()
            .default("default");
        add_column_if_missing(manager, CartLineItems::Table, shipping_profile_slug).await?;

        manager
            .create_table(
                Table::create()
                    .table(CartShippingSelections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartShippingSelections::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::CartId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::ShippingProfileSlug)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CartShippingSelections::SelectedShippingOptionId).uuid())
                    .col(
                        ColumnDef::new(CartShippingSelections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CartShippingSelections::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CartShippingSelections::Table,
                                CartShippingSelections::CartId,
                            )
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_shipping_selections_cart")
                    .table(CartShippingSelections::Table)
                    .col(CartShippingSelections::CartId)
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
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CartShippingSelections::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        drop_column_if_present(
            manager,
            CartLineItems::Table,
            CartLineItems::ShippingProfileSlug,
        )
        .await
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
    Id,
}

#[derive(Iden)]
enum CartLineItems {
    Table,
    ShippingProfileSlug,
}

#[derive(Iden)]
enum CartShippingSelections {
    Table,
    Id,
    CartId,
    ShippingProfileSlug,
    SelectedShippingOptionId,
    CreatedAt,
    UpdatedAt,
}

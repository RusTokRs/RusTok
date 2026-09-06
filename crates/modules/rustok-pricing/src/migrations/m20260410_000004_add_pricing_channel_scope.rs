use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut price_list_channel_id = ColumnDef::new(PriceLists::ChannelId);
        price_list_channel_id.uuid();
        add_column_if_missing(manager, PriceLists::Table, price_list_channel_id).await?;

        let mut price_list_channel_slug = ColumnDef::new(PriceLists::ChannelSlug);
        price_list_channel_slug.string_len(100);
        add_column_if_missing(manager, PriceLists::Table, price_list_channel_slug).await?;

        let mut price_channel_id = ColumnDef::new(Prices::ChannelId);
        price_channel_id.uuid();
        add_column_if_missing(manager, Prices::Table, price_channel_id).await?;

        let mut price_channel_slug = ColumnDef::new(Prices::ChannelSlug);
        price_channel_slug.string_len(100);
        add_column_if_missing(manager, Prices::Table, price_channel_slug).await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_price_lists_channel_scope")
                    .if_not_exists()
                    .table(PriceLists::Table)
                    .col(PriceLists::TenantId)
                    .col(PriceLists::ChannelId)
                    .col(PriceLists::ChannelSlug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_prices_channel_scope")
                    .if_not_exists()
                    .table(Prices::Table)
                    .col(Prices::VariantId)
                    .col(Prices::CurrencyCode)
                    .col(Prices::ChannelId)
                    .col(Prices::ChannelSlug)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_prices_channel_scope")
                    .table(Prices::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_price_lists_channel_scope")
                    .table(PriceLists::Table)
                    .to_owned(),
            )
            .await?;

        drop_column_if_present(manager, Prices::Table, Prices::ChannelSlug).await?;
        drop_column_if_present(manager, Prices::Table, Prices::ChannelId).await?;
        drop_column_if_present(manager, PriceLists::Table, PriceLists::ChannelSlug).await?;
        drop_column_if_present(manager, PriceLists::Table, PriceLists::ChannelId).await
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
enum PriceLists {
    Table,
    TenantId,
    ChannelId,
    ChannelSlug,
}

#[derive(Iden)]
enum Prices {
    Table,
    VariantId,
    CurrencyCode,
    ChannelId,
    ChannelSlug,
}

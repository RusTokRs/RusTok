use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut channel_id = ColumnDef::new(Carts::ChannelId);
        channel_id.uuid();
        add_column_if_missing(manager, Carts::Table, channel_id).await?;

        let mut channel_slug = ColumnDef::new(Carts::ChannelSlug);
        channel_slug.string_len(100);
        add_column_if_missing(manager, Carts::Table, channel_slug).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column_if_present(manager, Carts::Table, Carts::ChannelSlug).await?;
        drop_column_if_present(manager, Carts::Table, Carts::ChannelId).await
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
    ChannelId,
    ChannelSlug,
}

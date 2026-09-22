use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderLineItems::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(OrderLineItems::SellerId).string_len(100),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderLineItems::Table)
                    .drop_column(OrderLineItems::SellerId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum OrderLineItems {
    Table,
    SellerId,
}

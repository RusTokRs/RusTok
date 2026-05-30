use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderReturns::Table)
                    .add_column(ColumnDef::new(OrderReturns::ResolutionType).string_len(64))
                    .add_column(ColumnDef::new(OrderReturns::RefundId).uuid())
                    .add_column(ColumnDef::new(OrderReturns::OrderChangeId).uuid())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderReturns::Table)
                    .drop_column(OrderReturns::OrderChangeId)
                    .drop_column(OrderReturns::RefundId)
                    .drop_column(OrderReturns::ResolutionType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum OrderReturns {
    Table,
    ResolutionType,
    RefundId,
    OrderChangeId,
}

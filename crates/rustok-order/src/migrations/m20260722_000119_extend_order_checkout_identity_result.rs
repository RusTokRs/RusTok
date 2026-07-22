use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderCheckoutIdentities::Table)
                    .add_column(
                        ColumnDef::new(OrderCheckoutIdentities::PaymentCollectionId).uuid(),
                    )
                    .add_column(ColumnDef::new(OrderCheckoutIdentities::ShippingOptionId).uuid())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderCheckoutIdentities::Table)
                    .drop_column(OrderCheckoutIdentities::ShippingOptionId)
                    .drop_column(OrderCheckoutIdentities::PaymentCollectionId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum OrderCheckoutIdentities {
    Table,
    PaymentCollectionId,
    ShippingOptionId,
}

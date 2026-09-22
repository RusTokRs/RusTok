use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .add_column(
                            ColumnDef::new(FulfillmentItems::ShippedQuantity)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .add_column(
                            ColumnDef::new(FulfillmentItems::DeliveredQuantity)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .add_column(
                            ColumnDef::new(FulfillmentItems::ShippedQuantity)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .add_column(
                            ColumnDef::new(FulfillmentItems::DeliveredQuantity)
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .drop_column(FulfillmentItems::ShippedQuantity)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .drop_column(FulfillmentItems::DeliveredQuantity)
                        .to_owned(),
                )
                .await
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(FulfillmentItems::Table)
                        .drop_column(FulfillmentItems::ShippedQuantity)
                        .drop_column(FulfillmentItems::DeliveredQuantity)
                        .to_owned(),
                )
                .await
        }
    }
}

#[derive(DeriveIden)]
enum FulfillmentItems {
    Table,
    ShippedQuantity,
    DeliveredQuantity,
}

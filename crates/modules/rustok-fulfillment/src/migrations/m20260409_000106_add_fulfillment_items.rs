use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FulfillmentItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FulfillmentItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::FulfillmentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::OrderLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FulfillmentItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FulfillmentItems::Table, FulfillmentItems::FulfillmentId)
                            .to(Fulfillments::Table, Fulfillments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_fulfillment_items_fulfillment")
                    .table(FulfillmentItems::Table)
                    .col(FulfillmentItems::FulfillmentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_fulfillment_items_order_line")
                    .table(FulfillmentItems::Table)
                    .col(FulfillmentItems::OrderLineItemId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FulfillmentItems::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Fulfillments {
    Table,
    Id,
}

#[derive(Iden)]
enum FulfillmentItems {
    Table,
    Id,
    FulfillmentId,
    OrderLineItemId,
    Quantity,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

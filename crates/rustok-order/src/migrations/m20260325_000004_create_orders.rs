use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Orders::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Orders::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Orders::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Orders::CustomerId).uuid().not_null())
                    .col(
                        ColumnDef::new(Orders::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Orders::TotalAmount)
                            .decimal_len(20, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Orders::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Orders::PaymentId).string_len(255))
                    .col(ColumnDef::new(Orders::PaymentMethod).string_len(100))
                    .col(ColumnDef::new(Orders::TrackingNumber).string_len(255))
                    .col(ColumnDef::new(Orders::Carrier).string_len(100))
                    .col(ColumnDef::new(Orders::CancellationReason).string_len(500))
                    .col(ColumnDef::new(Orders::RefundId).string_len(255))
                    .col(ColumnDef::new(Orders::DeliveredSignature).string_len(255))
                    .col(
                        ColumnDef::new(Orders::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Orders::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Orders::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Orders::ConfirmedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Orders::PaidAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Orders::ShippedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Orders::DeliveredAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Orders::CancelledAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Orders::Table, Orders::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OrderLineItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderLineItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderLineItems::OrderId).uuid().not_null())
                    .col(ColumnDef::new(OrderLineItems::ProductId).uuid())
                    .col(ColumnDef::new(OrderLineItems::VariantId).uuid())
                    .col(ColumnDef::new(OrderLineItems::Sku).string_len(100))
                    .col(
                        ColumnDef::new(OrderLineItems::Title)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::UnitPrice)
                            .decimal_len(20, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::TotalPrice)
                            .decimal_len(20, 6)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(OrderLineItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(OrderLineItems::Table, OrderLineItems::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_orders_tenant_status")
                    .table(Orders::Table)
                    .col(Orders::TenantId)
                    .col(Orders::Status)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_line_items_order")
                    .table(OrderLineItems::Table)
                    .col(OrderLineItems::OrderId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OrderLineItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Orders::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Orders {
    Table,
    Id,
    TenantId,
    CustomerId,
    Status,
    TotalAmount,
    CurrencyCode,
    PaymentId,
    PaymentMethod,
    TrackingNumber,
    Carrier,
    CancellationReason,
    RefundId,
    DeliveredSignature,
    Metadata,
    CreatedAt,
    UpdatedAt,
    ConfirmedAt,
    PaidAt,
    ShippedAt,
    DeliveredAt,
    CancelledAt,
}

#[derive(Iden)]
enum OrderLineItems {
    Table,
    Id,
    OrderId,
    ProductId,
    VariantId,
    Sku,
    Title,
    Quantity,
    UnitPrice,
    TotalPrice,
    CurrencyCode,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum Tenants {
    Table,
    Id,
}

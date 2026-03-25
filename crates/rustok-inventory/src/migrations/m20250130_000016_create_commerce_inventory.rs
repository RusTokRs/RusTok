use super::shared::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StockLocations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StockLocations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(StockLocations::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(StockLocations::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(StockLocations::Code).string_len(50))
                    .col(ColumnDef::new(StockLocations::AddressLine1).string_len(255))
                    .col(ColumnDef::new(StockLocations::AddressLine2).string_len(255))
                    .col(ColumnDef::new(StockLocations::City).string_len(100))
                    .col(ColumnDef::new(StockLocations::Province).string_len(100))
                    .col(ColumnDef::new(StockLocations::PostalCode).string_len(20))
                    .col(ColumnDef::new(StockLocations::CountryCode).string_len(2))
                    .col(ColumnDef::new(StockLocations::Phone).string_len(50))
                    .col(
                        ColumnDef::new(StockLocations::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(StockLocations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(StockLocations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(StockLocations::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(StockLocations::Table, StockLocations::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(InventoryItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InventoryItems::VariantId)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(InventoryItems::Sku).string_len(100))
                    .col(
                        ColumnDef::new(InventoryItems::RequiresShipping)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(InventoryItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(InventoryItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(InventoryItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(InventoryItems::Table, InventoryItems::VariantId)
                            .to(ProductVariants::Table, ProductVariants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(InventoryLevels::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InventoryLevels::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InventoryLevels::InventoryItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryLevels::LocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InventoryLevels::StockedQuantity)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(InventoryLevels::ReservedQuantity)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(InventoryLevels::IncomingQuantity)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(InventoryLevels::LowStockThreshold).integer())
                    .col(
                        ColumnDef::new(InventoryLevels::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(InventoryLevels::Table, InventoryLevels::InventoryItemId)
                            .to(InventoryItems::Table, InventoryItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(InventoryLevels::Table, InventoryLevels::LocationId)
                            .to(StockLocations::Table, StockLocations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ReservationItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReservationItems::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ReservationItems::InventoryItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReservationItems::LocationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReservationItems::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReservationItems::LineItemId).uuid())
                    .col(ColumnDef::new(ReservationItems::Description).string_len(255))
                    .col(ColumnDef::new(ReservationItems::ExternalId).string_len(100))
                    .col(
                        ColumnDef::new(ReservationItems::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(ReservationItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ReservationItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(ReservationItems::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(ReservationItems::Table, ReservationItems::InventoryItemId)
                            .to(InventoryItems::Table, InventoryItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ReservationItems::Table, ReservationItems::LocationId)
                            .to(StockLocations::Table, StockLocations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_stock_locations_tenant")
                    .table(StockLocations::Table)
                    .col(StockLocations::TenantId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_inventory_levels_unique")
                    .table(InventoryLevels::Table)
                    .col(InventoryLevels::InventoryItemId)
                    .col(InventoryLevels::LocationId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_reservation_items_inventory")
                    .table(ReservationItems::Table)
                    .col(ReservationItems::InventoryItemId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_reservation_items_line")
                    .table(ReservationItems::Table)
                    .col(ReservationItems::LineItemId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ReservationItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(InventoryLevels::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(InventoryItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StockLocations::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum StockLocations {
    Table,
    Id,
    TenantId,
    Name,
    Code,
    AddressLine1,
    AddressLine2,
    City,
    Province,
    PostalCode,
    CountryCode,
    Phone,
    Metadata,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum InventoryItems {
    Table,
    Id,
    VariantId,
    Sku,
    RequiresShipping,
    Metadata,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum InventoryLevels {
    Table,
    Id,
    InventoryItemId,
    LocationId,
    StockedQuantity,
    ReservedQuantity,
    IncomingQuantity,
    LowStockThreshold,
    UpdatedAt,
}

#[derive(Iden)]
enum ReservationItems {
    Table,
    Id,
    InventoryItemId,
    LocationId,
    Quantity,
    LineItemId,
    Description,
    ExternalId,
    Metadata,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum ProductVariants {
    Table,
    Id,
}

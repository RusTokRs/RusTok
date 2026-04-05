mod m20260325_000102_create_cart_tables;
mod m20260326_000103_add_cart_context_columns;
mod m20260402_000104_add_cart_channel_columns;
mod m20260405_000105_add_cart_delivery_groups;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000102_create_cart_tables::Migration),
        Box::new(m20260326_000103_add_cart_context_columns::Migration),
        Box::new(m20260402_000104_add_cart_channel_columns::Migration),
        Box::new(m20260405_000105_add_cart_delivery_groups::Migration),
    ]
}

mod m20260325_000101_create_order_tables;
mod m20260402_000102_add_order_channel_columns;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000101_create_order_tables::Migration),
        Box::new(m20260402_000102_add_order_channel_columns::Migration),
    ]
}

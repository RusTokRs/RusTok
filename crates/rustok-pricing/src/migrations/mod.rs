mod shared;

mod m20250130_000015_create_commerce_prices;
mod m20260325_000002_add_decimal_runtime_columns;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000015_create_commerce_prices::Migration),
        Box::new(m20260325_000002_add_decimal_runtime_columns::Migration),
    ]
}

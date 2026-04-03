mod shared;

mod m20250130_000017_create_commerce_collections;
mod m20250130_000018_create_commerce_categories;
mod m20260316_000005_create_order_field_definitions;
mod m20260402_000001_create_shipping_profiles;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000017_create_commerce_collections::Migration),
        Box::new(m20250130_000018_create_commerce_categories::Migration),
        Box::new(m20260316_000005_create_order_field_definitions::Migration),
        Box::new(m20260402_000001_create_shipping_profiles::Migration),
    ]
}

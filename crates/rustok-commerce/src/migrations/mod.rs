mod shared;

mod m20250130_000012_create_commerce_products;
mod m20250130_000013_create_commerce_options;
mod m20250130_000014_create_commerce_variants;
mod m20250130_000015_create_commerce_prices;
mod m20250130_000016_create_commerce_inventory;
mod m20250130_000017_create_commerce_collections;
mod m20250130_000018_create_commerce_categories;
mod m20260301_000001_alter_product_variants_add_fields;
mod m20260316_000002_create_product_field_definitions;
mod m20260316_000005_create_order_field_definitions;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000012_create_commerce_products::Migration),
        Box::new(m20250130_000013_create_commerce_options::Migration),
        Box::new(m20250130_000014_create_commerce_variants::Migration),
        Box::new(m20250130_000015_create_commerce_prices::Migration),
        Box::new(m20250130_000016_create_commerce_inventory::Migration),
        Box::new(m20250130_000017_create_commerce_collections::Migration),
        Box::new(m20250130_000018_create_commerce_categories::Migration),
        Box::new(m20260301_000001_alter_product_variants_add_fields::Migration),
        Box::new(m20260316_000002_create_product_field_definitions::Migration),
        Box::new(m20260316_000005_create_order_field_definitions::Migration),
    ]
}

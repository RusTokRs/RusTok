mod m20260325_000105_create_fulfillment_tables;
mod m20260409_000106_add_fulfillment_items;
mod m20260409_000107_add_fulfillment_item_progress;
mod m20260411_000108_add_shipping_option_translations;
mod m20260713_000109_enforce_fulfillment_integrity;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000105_create_fulfillment_tables::Migration),
        Box::new(m20260409_000106_add_fulfillment_items::Migration),
        Box::new(m20260409_000107_add_fulfillment_item_progress::Migration),
        Box::new(m20260411_000108_add_shipping_option_translations::Migration),
        Box::new(m20260713_000109_enforce_fulfillment_integrity::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260713_000109_enforce_fulfillment_integrity",
        vec!["m20260325_000101_create_order_tables"],
    )]
}

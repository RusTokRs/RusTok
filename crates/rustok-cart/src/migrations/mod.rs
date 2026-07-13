mod m20260325_000102_create_cart_tables;
mod m20260326_000103_add_cart_context_columns;
mod m20260402_000104_add_cart_channel_columns;
mod m20260405_000105_add_cart_delivery_groups;
mod m20260408_000106_add_cart_shipping_selection_seller_scope;
mod m20260409_000107_add_cart_shipping_selection_seller_id;
mod m20260410_000108_create_cart_adjustments;
mod m20260411_000109_add_cart_tax_lines;
mod m20260411_000110_add_cart_line_item_translations;
mod m20260412_000111_add_cart_shipping_total;
mod m20260412_000112_add_cart_tax_line_provider_id;
mod m20260713_000113_enforce_cart_money_integrity;
mod m20260713_000114_serialize_cart_lifecycle;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000102_create_cart_tables::Migration),
        Box::new(m20260326_000103_add_cart_context_columns::Migration),
        Box::new(m20260402_000104_add_cart_channel_columns::Migration),
        Box::new(m20260405_000105_add_cart_delivery_groups::Migration),
        Box::new(m20260408_000106_add_cart_shipping_selection_seller_scope::Migration),
        Box::new(m20260409_000107_add_cart_shipping_selection_seller_id::Migration),
        Box::new(m20260410_000108_create_cart_adjustments::Migration),
        Box::new(m20260411_000109_add_cart_tax_lines::Migration),
        Box::new(m20260411_000110_add_cart_line_item_translations::Migration),
        Box::new(m20260412_000111_add_cart_shipping_total::Migration),
        Box::new(m20260412_000112_add_cart_tax_line_provider_id::Migration),
        Box::new(m20260713_000113_enforce_cart_money_integrity::Migration),
        Box::new(m20260713_000114_serialize_cart_lifecycle::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260713_000113_enforce_cart_money_integrity",
        vec!["m20260325_000105_create_fulfillment_tables"],
    )]
}

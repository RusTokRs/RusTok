mod m20260325_000105_create_fulfillment_tables;
mod m20260409_000106_add_fulfillment_items;
mod m20260409_000107_add_fulfillment_item_progress;
mod m20260411_000108_add_shipping_option_translations;
mod m20260713_000109_enforce_fulfillment_integrity;
mod m20260713_000110_serialize_fulfillment_progress;
mod m20260713_000111_create_provider_operation_journal;
mod m20260713_000112_commit_provider_operations_with_fulfillment;
mod m20260713_000113_allow_provider_execution_reconciliation;
mod m20260713_000114_defer_checkout_create_label_until_paid;
mod m20260713_000115_cleanup_cancelled_checkout_labels;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000105_create_fulfillment_tables::Migration),
        Box::new(m20260409_000106_add_fulfillment_items::Migration),
        Box::new(m20260409_000107_add_fulfillment_item_progress::Migration),
        Box::new(m20260411_000108_add_shipping_option_translations::Migration),
        Box::new(m20260713_000109_enforce_fulfillment_integrity::Migration),
        Box::new(m20260713_000110_serialize_fulfillment_progress::Migration),
        Box::new(m20260713_000111_create_provider_operation_journal::Migration),
        Box::new(m20260713_000112_commit_provider_operations_with_fulfillment::Migration),
        Box::new(m20260713_000113_allow_provider_execution_reconciliation::Migration),
        Box::new(m20260713_000114_defer_checkout_create_label_until_paid::Migration),
        Box::new(m20260713_000115_cleanup_cancelled_checkout_labels::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260713_000109_enforce_fulfillment_integrity",
            vec!["m20260325_000101_create_order_tables"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000114_defer_checkout_create_label_until_paid",
            vec![
                "m20260713_000113_allow_provider_execution_reconciliation",
                "m20260713_000115_serialize_order_lifecycle",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000115_cleanup_cancelled_checkout_labels",
            vec!["m20260713_000114_defer_checkout_create_label_until_paid"],
        ),
    ]
}

mod shared;

mod m20250130_000017_create_commerce_collections;
mod m20250130_000018_create_commerce_categories;
mod m20260316_000005_create_order_field_definitions;
mod m20260402_000001_create_shipping_profiles;
mod m20260405_000003_add_is_localized_to_order_field_definitions;
mod m20260411_000004_add_shipping_profile_translations;
mod m20260713_000005_reserve_inventory_on_order_confirmation;
mod m20260713_000006_consume_inventory_on_order_delivery;
mod m20260713_000007_consume_inventory_on_fulfillment_shipping;
mod m20260713_000008_require_fulfillment_before_order_delivery;
mod m20260713_000009_create_checkout_operations;
mod m20260713_000010_create_checkout_inventory_reservations;
mod m20260713_000011_enforce_checkout_inventory_reservation_quantity;
mod m20260713_000012_adopt_checkout_inventory_into_order_lines;
mod m20260713_000013_cutover_checkout_inventory_lifecycle;
mod m20260713_000014_create_checkout_order_plans;
mod m20260713_000015_bind_checkout_payment_collections;
mod m20260713_000016_block_provider_execution_during_checkout_compensation;
mod m20260713_000017_classify_checkout_reconciliation;
mod m20260716_000003_add_order_field_cache_generation_trigger;
mod m20260716_000004_create_return_completion_operations;
mod m20260716_000005_enforce_return_completion_resolution_identity;
mod m20260716_000006_create_return_completion_commands;
mod m20260721_000001_create_checkout_marketplace_economics_checkpoints;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000017_create_commerce_collections::Migration),
        Box::new(m20250130_000018_create_commerce_categories::Migration),
        Box::new(m20260316_000005_create_order_field_definitions::Migration),
        Box::new(m20260402_000001_create_shipping_profiles::Migration),
        Box::new(m20260405_000003_add_is_localized_to_order_field_definitions::Migration),
        Box::new(m20260411_000004_add_shipping_profile_translations::Migration),
        Box::new(m20260713_000005_reserve_inventory_on_order_confirmation::Migration),
        Box::new(m20260713_000006_consume_inventory_on_order_delivery::Migration),
        Box::new(m20260713_000007_consume_inventory_on_fulfillment_shipping::Migration),
        Box::new(m20260713_000008_require_fulfillment_before_order_delivery::Migration),
        Box::new(m20260713_000009_create_checkout_operations::Migration),
        Box::new(m20260713_000010_create_checkout_inventory_reservations::Migration),
        Box::new(m20260713_000011_enforce_checkout_inventory_reservation_quantity::Migration),
        Box::new(m20260713_000012_adopt_checkout_inventory_into_order_lines::Migration),
        Box::new(m20260713_000013_cutover_checkout_inventory_lifecycle::Migration),
        Box::new(m20260713_000014_create_checkout_order_plans::Migration),
        Box::new(m20260713_000015_bind_checkout_payment_collections::Migration),
        Box::new(m20260713_000016_block_provider_execution_during_checkout_compensation::Migration),
        Box::new(m20260713_000017_classify_checkout_reconciliation::Migration),
        Box::new(m20260716_000003_add_order_field_cache_generation_trigger::Migration),
        Box::new(m20260716_000004_create_return_completion_operations::Migration),
        Box::new(m20260716_000005_enforce_return_completion_resolution_identity::Migration),
        Box::new(m20260716_000006_create_return_completion_commands::Migration),
        Box::new(
            m20260721_000001_create_checkout_marketplace_economics_checkpoints::Migration,
        ),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20250130_000017_create_commerce_collections",
            vec!["m20250130_000012_create_commerce_products"],
        ),
        MigrationDependencyDescriptor::new(
            "m20250130_000018_create_commerce_categories",
            vec!["m20250130_000012_create_commerce_products"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000005_reserve_inventory_on_order_confirmation",
            vec![
                "m20260713_000018_enforce_inventory_state_invariants",
                "m20260713_000114_enforce_order_money_integrity",
                "m20260713_000115_serialize_order_lifecycle",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000006_consume_inventory_on_order_delivery",
            vec!["m20260713_000005_reserve_inventory_on_order_confirmation"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000007_consume_inventory_on_fulfillment_shipping",
            vec![
                "m20260713_000005_reserve_inventory_on_order_confirmation",
                "m20260713_000110_serialize_fulfillment_progress",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000008_require_fulfillment_before_order_delivery",
            vec![
                "m20260713_000006_consume_inventory_on_order_delivery",
                "m20260713_000110_serialize_fulfillment_progress",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000009_create_checkout_operations",
            vec![
                "m20260325_000102_create_cart_tables",
                "m20260325_000101_create_order_tables",
                "m20260325_000104_create_payment_tables",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000010_create_checkout_inventory_reservations",
            vec![
                "m20260713_000009_create_checkout_operations",
                "m20260713_000018_enforce_inventory_state_invariants",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000011_enforce_checkout_inventory_reservation_quantity",
            vec!["m20260713_000010_create_checkout_inventory_reservations"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000012_adopt_checkout_inventory_into_order_lines",
            vec![
                "m20260713_000011_enforce_checkout_inventory_reservation_quantity",
                "m20260325_000101_create_order_tables",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000013_cutover_checkout_inventory_lifecycle",
            vec![
                "m20260713_000012_adopt_checkout_inventory_into_order_lines",
                "m20260713_000116_enforce_checkout_operation_identity",
                "m20260713_000110_serialize_fulfillment_progress",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000014_create_checkout_order_plans",
            vec!["m20260713_000013_cutover_checkout_inventory_lifecycle"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000015_bind_checkout_payment_collections",
            vec![
                "m20260713_000014_create_checkout_order_plans",
                "m20260325_000104_create_payment_tables",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000016_block_provider_execution_during_checkout_compensation",
            vec![
                "m20260713_000015_bind_checkout_payment_collections",
                "m20260713_000110_create_provider_operation_journal",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000017_classify_checkout_reconciliation",
            vec!["m20260713_000016_block_provider_execution_during_checkout_compensation"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260716_000004_create_return_completion_operations",
            vec![
                "m20260530_000113_add_order_return_resolution_columns",
                "m20260529_000112_create_order_changes_table",
                "m20260714_000119_require_refund_creation_identity",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260716_000005_enforce_return_completion_resolution_identity",
            vec!["m20260716_000004_create_return_completion_operations"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260716_000006_create_return_completion_commands",
            vec!["m20260716_000005_enforce_return_completion_resolution_identity"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260721_000001_create_checkout_marketplace_economics_checkpoints",
            vec![
                "m20260713_000014_create_checkout_order_plans",
                "m20260718_000001_create_marketplace_order_allocations",
                "m20260718_000001_create_marketplace_commission",
            ],
        ),
    ]
}

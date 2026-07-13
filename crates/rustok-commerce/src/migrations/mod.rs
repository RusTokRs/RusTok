mod shared;

mod m20250130_000017_create_commerce_collections;
mod m20250130_000018_create_commerce_categories;
mod m20260316_000005_create_order_field_definitions;
mod m20260402_000001_create_shipping_profiles;
mod m20260405_000003_add_is_localized_to_order_field_definitions;
mod m20260411_000004_add_shipping_profile_translations;
mod m20260713_000005_reserve_inventory_on_order_confirmation;

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
    ]
}

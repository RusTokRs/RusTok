mod shared;

mod m20250130_000016_create_commerce_inventory;
mod m20260411_000001_add_stock_location_translations;
mod m20260713_000017_enforce_reservation_identity;
mod m20260713_000018_enforce_inventory_state_invariants;
mod m20260713_000019_reserve_checkout_order_inventory;
mod m20260713_000020_remove_duplicate_checkout_reservation;
mod m20260713_000021_bound_reservation_metadata_and_inactive_locations;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000016_create_commerce_inventory::Migration),
        Box::new(m20260411_000001_add_stock_location_translations::Migration),
        Box::new(m20260713_000017_enforce_reservation_identity::Migration),
        Box::new(m20260713_000018_enforce_inventory_state_invariants::Migration),
        Box::new(m20260713_000019_reserve_checkout_order_inventory::Migration),
        Box::new(m20260713_000020_remove_duplicate_checkout_reservation::Migration),
        Box::new(m20260713_000021_bound_reservation_metadata_and_inactive_locations::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20250130_000016_create_commerce_inventory",
            vec!["m20250130_000014_create_commerce_variants"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000019_reserve_checkout_order_inventory",
            vec!["m20260325_000101_create_order_tables"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000020_remove_duplicate_checkout_reservation",
            vec![
                "m20260713_000019_reserve_checkout_order_inventory",
                "m20260713_000005_reserve_inventory_on_order_confirmation",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260713_000021_bound_reservation_metadata_and_inactive_locations",
            vec!["m20260713_000020_remove_duplicate_checkout_reservation"],
        ),
    ]
}

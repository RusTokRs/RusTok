mod m20260720_000001_create_moderation_core;
mod m20260720_000002_add_active_case_deduplication;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260720_000001_create_moderation_core::Migration),
        Box::new(m20260720_000002_add_active_case_deduplication::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260720_000001_create_moderation_core",
            vec!["m20260713_000117_enforce_checkout_fulfillment_identity"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260720_000002_add_active_case_deduplication",
            vec!["m20260720_000001_create_moderation_core"],
        ),
    ]
}

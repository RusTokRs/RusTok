mod m20260720_000001_create_moderation_core;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260720_000001_create_moderation_core::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260720_000001_create_moderation_core",
        vec!["m20260713_000117_enforce_checkout_fulfillment_identity"],
    )]
}

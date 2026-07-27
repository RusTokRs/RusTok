mod m20260727_000001_create_translation_inventory;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260727_000001_create_translation_inventory::Migration,
    )]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260727_000001_create_translation_inventory",
        vec!["m20250101_000001_create_tenants"],
    )]
}

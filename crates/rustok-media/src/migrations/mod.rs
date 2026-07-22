mod m20260722_000001_create_media_lifecycle;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260722_000001_create_media_lifecycle::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260722_000001_create_media_lifecycle",
        vec![
            "m20250101_000001_create_tenants",
            "m20250101_000002_create_users",
        ],
    )]
}

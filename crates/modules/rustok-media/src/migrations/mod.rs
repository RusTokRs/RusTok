mod m20260722_000001_create_media_lifecycle;
mod m20260726_000002_add_media_translation_revision;
mod m20260727_000003_create_media_translation_changes;
mod m20260916_000004_add_media_owner_module;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260722_000001_create_media_lifecycle::Migration),
        Box::new(m20260726_000002_add_media_translation_revision::Migration),
        Box::new(m20260727_000003_create_media_translation_changes::Migration),
        Box::new(m20260916_000004_add_media_owner_module::Migration),
    ]
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

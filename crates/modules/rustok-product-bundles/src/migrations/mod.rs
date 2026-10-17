use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::prelude::*;

mod m20260916_000001_create_bundles;
mod m20261017_000002_normalize_bundle_translation_locales;
mod m20261017_000004_enforce_bundle_tenant_integrity;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260916_000001_create_bundles::Migration),
            Box::new(m20261017_000002_normalize_bundle_translation_locales::Migration),
            Box::new(m20261017_000004_enforce_bundle_tenant_integrity::Migration),
        ]
    }
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260916_000001_create_bundles::Migration),
        Box::new(m20261017_000002_normalize_bundle_translation_locales::Migration),
        Box::new(m20261017_000004_enforce_bundle_tenant_integrity::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20261017_000002_normalize_bundle_translation_locales",
            vec!["m20260916_000001_create_bundles"],
        ),
        MigrationDependencyDescriptor::new(
            "m20261017_000004_enforce_bundle_tenant_integrity",
            vec![
                "m20261017_000002_normalize_bundle_translation_locales",
                "m20261017_000036_add_sqlite_tenant_identity_keys",
            ],
        ),
    ]
}

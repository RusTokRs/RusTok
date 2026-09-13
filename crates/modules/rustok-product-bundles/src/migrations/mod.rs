use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::prelude::*;

mod m20260916_000001_create_bundles;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260916_000001_create_bundles::Migration)]
    }
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260916_000001_create_bundles::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260916_000001_create_bundles",
        vec!["m20250130_000012_create_commerce_products"],
    )]
}

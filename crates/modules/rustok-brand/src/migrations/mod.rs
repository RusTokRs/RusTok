use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::prelude::*;

mod m20260915_000001_create_brands;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260915_000001_create_brands::Migration)]
    }
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260915_000001_create_brands::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260915_000001_create_brands",
        vec!["m20250130_000012_create_commerce_products"],
    )]
}

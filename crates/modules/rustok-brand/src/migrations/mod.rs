use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::prelude::*;

mod m20260915_000001_create_brands;
mod m20261104_000002_create_brand_categories;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        migrations()
    }
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260915_000001_create_brands::Migration),
        Box::new(m20261104_000002_create_brand_categories::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260915_000001_create_brands",
            vec!["m20250130_000012_create_commerce_products"],
        ),
        MigrationDependencyDescriptor::new(
            "m20261104_000002_create_brand_categories",
            vec!["m20260915_000001_create_brands"],
        ),
    ]
}

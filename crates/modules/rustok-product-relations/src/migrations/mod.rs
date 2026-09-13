mod m20260914_000001_create_product_relations;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260914_000001_create_product_relations::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260914_000001_create_product_relations",
        vec!["m20250130_000012_create_commerce_products"],
    )]
}

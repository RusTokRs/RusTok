mod m20260914_000001_create_product_relations;
mod m20260921_000002_enforce_product_foreign_keys;
mod m20261017_000003_enforce_relation_positions;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260914_000001_create_product_relations::Migration),
        Box::new(m20260921_000002_enforce_product_foreign_keys::Migration),
        Box::new(m20261017_000003_enforce_relation_positions::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260914_000001_create_product_relations",
        vec!["m20250130_000012_create_commerce_products"],
    )]
}

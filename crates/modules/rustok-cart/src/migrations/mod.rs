mod m20261029_000101_create_cart_schema;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261029_000101_create_cart_schema::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20261029_000101_create_cart_schema",
        vec!["m20260325_000105_create_fulfillment_tables"],
    )]
}

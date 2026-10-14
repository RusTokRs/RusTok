mod m20261014_000001_create_messenger_tables;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261014_000001_create_messenger_tables::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![]
}

mod m20261009_000003_create_form_submissions;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20261009_000003_create_form_submissions::Migration,
    )]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    Vec::new()
}

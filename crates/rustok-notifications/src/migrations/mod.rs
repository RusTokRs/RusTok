mod m20260721_000010_create_notification_persistence;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260721_000010_create_notification_persistence::Migration,
    )]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260721_000010_create_notification_persistence",
        vec!["m20250101_000002_create_users"],
    )]
}

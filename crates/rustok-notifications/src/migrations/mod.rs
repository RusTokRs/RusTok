mod m20260721_000010_create_notification_persistence;
mod m20260722_000011_create_notification_source_inbox;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260721_000010_create_notification_persistence::Migration),
        Box::new(m20260722_000011_create_notification_source_inbox::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260721_000010_create_notification_persistence",
            vec!["m20250101_000002_create_users"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260722_000011_create_notification_source_inbox",
            vec!["m20260721_000010_create_notification_persistence"],
        ),
    ]
}

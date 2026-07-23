mod m20260721_000010_create_notification_persistence;
mod m20260722_000011_create_notification_source_inbox;
mod m20260722_000012_add_candidate_processing;
mod m20260723_000013_add_outbox_intake_receipts;
mod m20260723_000014_add_outbox_intake_rejections;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260721_000010_create_notification_persistence::Migration),
        Box::new(m20260722_000011_create_notification_source_inbox::Migration),
        Box::new(m20260722_000012_add_candidate_processing::Migration),
        Box::new(m20260723_000013_add_outbox_intake_receipts::Migration),
        Box::new(m20260723_000014_add_outbox_intake_rejections::Migration),
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
        MigrationDependencyDescriptor::new(
            "m20260722_000012_add_candidate_processing",
            vec!["m20260722_000011_create_notification_source_inbox"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260723_000013_add_outbox_intake_receipts",
            vec!["m20260722_000012_add_candidate_processing"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260723_000014_add_outbox_intake_rejections",
            vec!["m20260723_000013_add_outbox_intake_receipts"],
        ),
    ]
}

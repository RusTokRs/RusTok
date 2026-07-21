mod m20260721_000010_create_notification_persistence;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260721_000010_create_notification_persistence::Migration,
    )]
}

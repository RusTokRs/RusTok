mod m20260316_000004_create_topic_field_definitions;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260316_000004_create_topic_field_definitions::Migration,
    )]
}

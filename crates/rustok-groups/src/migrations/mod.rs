mod m20260721_000001_create_groups;
mod m20260721_000002_create_group_governance;
mod m20260721_000003_enforce_group_language_agnostic_storage;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260721_000001_create_groups::Migration),
        Box::new(m20260721_000002_create_group_governance::Migration),
        Box::new(m20260721_000003_enforce_group_language_agnostic_storage::Migration),
    ]
}

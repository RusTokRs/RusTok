mod m20260716_000000_create_field_definition_cache_generation;
mod m20260822_000001_create_generic_attached_donor_storage;
mod m20260909_000002_add_schema_translation_change_journal;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260716_000000_create_field_definition_cache_generation::Migration),
        Box::new(m20260822_000001_create_generic_attached_donor_storage::Migration),
        Box::new(m20260909_000002_add_schema_translation_change_journal::Migration),
    ]
}

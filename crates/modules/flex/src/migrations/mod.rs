mod m20260716_000000_create_field_definition_cache_generation;
mod m20260822_000001_create_generic_attached_donor_storage;
mod m20260909_000002_add_schema_translation_change_journal;
mod m20260909_000003_add_attached_translation_change_journal;
mod m20260910_000004_add_attached_field_policies;
mod m20260910_000005_add_standalone_translation_change_journal;
mod m20260911_000006_add_standalone_field_policies;
mod m20260911_000007_prune_standalone_field_policies;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260716_000000_create_field_definition_cache_generation::Migration),
        Box::new(m20260822_000001_create_generic_attached_donor_storage::Migration),
        Box::new(m20260909_000002_add_schema_translation_change_journal::Migration),
        Box::new(m20260909_000003_add_attached_translation_change_journal::Migration),
        Box::new(m20260910_000004_add_attached_field_policies::Migration),
        Box::new(m20260910_000005_add_standalone_translation_change_journal::Migration),
        Box::new(m20260911_000006_add_standalone_field_policies::Migration),
        Box::new(m20260911_000007_prune_standalone_field_policies::Migration),
    ]
}

mod m20260326_000001_create_profiles_tables;
mod m20260330_000002_create_profile_tags;
mod m20260721_000009_expand_profile_locale_storage_columns;
mod m20260721_000010_move_profile_display_name_to_translations;
mod m20260813_000011_enforce_profile_tag_tenant_integrity;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260326_000001_create_profiles_tables::Migration),
        Box::new(m20260330_000002_create_profile_tags::Migration),
        Box::new(m20260721_000009_expand_profile_locale_storage_columns::Migration),
        Box::new(m20260721_000010_move_profile_display_name_to_translations::Migration),
        Box::new(m20260813_000011_enforce_profile_tag_tenant_integrity::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260813_000011_enforce_profile_tag_tenant_integrity",
        vec!["m20260711_000001_add_tenant_identity_key"],
    )]
}

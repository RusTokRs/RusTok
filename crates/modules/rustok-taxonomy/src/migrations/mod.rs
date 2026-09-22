mod m20260329_000001_create_taxonomy_tables;
mod m20260711_000001_add_tenant_identity_key;
mod m20260721_000006_expand_taxonomy_locale_storage_columns;
mod m20260803_000007_add_translation_target_support;
mod m20260812_000008_add_route_key_registry;
mod m20260813_000009_remove_term_status;
mod m20260822_000010_create_taxonomy_category_hierarchy;
mod m20260822_000011_create_taxonomy_category_presentations;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260329_000001_create_taxonomy_tables::Migration),
        Box::new(m20260711_000001_add_tenant_identity_key::Migration),
        Box::new(m20260721_000006_expand_taxonomy_locale_storage_columns::Migration),
        Box::new(m20260803_000007_add_translation_target_support::Migration),
        Box::new(m20260812_000008_add_route_key_registry::Migration),
        Box::new(m20260813_000009_remove_term_status::Migration),
        Box::new(m20260822_000010_create_taxonomy_category_hierarchy::Migration),
        Box::new(m20260822_000011_create_taxonomy_category_presentations::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260803_000007_add_translation_target_support",
            vec!["m20260803_000001_create_owner_operation_receipts"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260812_000008_add_route_key_registry",
            vec!["m20260803_000007_add_translation_target_support"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260813_000009_remove_term_status",
            vec!["m20260812_000008_add_route_key_registry"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260822_000010_create_taxonomy_category_hierarchy",
            vec!["m20260813_000009_remove_term_status"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260822_000011_create_taxonomy_category_presentations",
            vec!["m20260822_000010_create_taxonomy_category_hierarchy"],
        ),
    ]
}

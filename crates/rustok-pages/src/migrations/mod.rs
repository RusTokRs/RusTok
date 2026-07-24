mod m20260328_000001_create_pages_tables;
mod m20260329_000001_create_page_channel_visibility_table;
mod m20260714_000001_create_page_builder_scenario_baselines;
mod m20260714_000002_add_scenario_baseline_promotion_metadata;
mod m20260718_000001_canonicalize_grapesjs_format;
mod m20260718_000002_create_static_landing_artifacts;
mod m20260721_000003_expand_pages_locale_storage_columns;
mod m20260721_000004_enforce_language_agnostic_pages;
mod m20260721_000006_add_static_landing_materialization_evidence;
mod m20260721_000007_create_page_publish_operations;
mod m20260722_000009_create_page_rollback_operations;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_pages_tables::Migration),
        Box::new(m20260329_000001_create_page_channel_visibility_table::Migration),
        Box::new(m20260714_000001_create_page_builder_scenario_baselines::Migration),
        Box::new(m20260714_000002_add_scenario_baseline_promotion_metadata::Migration),
        Box::new(m20260718_000001_canonicalize_grapesjs_format::Migration),
        Box::new(m20260718_000002_create_static_landing_artifacts::Migration),
        Box::new(m20260721_000003_expand_pages_locale_storage_columns::Migration),
        Box::new(m20260721_000004_enforce_language_agnostic_pages::Migration),
        Box::new(m20260721_000006_add_static_landing_materialization_evidence::Migration),
        Box::new(m20260721_000007_create_page_publish_operations::Migration),
        Box::new(m20260722_000009_create_page_rollback_operations::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    Vec::new()
}

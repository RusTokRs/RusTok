mod m20260328_000001_create_pages_tables;
mod m20260329_000001_create_page_channel_visibility_table;
mod m20260714_000001_create_page_builder_scenario_baselines;
mod m20260714_000002_add_scenario_baseline_promotion_metadata;
mod m20260718_000002_create_static_landing_artifacts;
mod m20260721_000003_expand_pages_locale_storage_columns;
mod m20260721_000004_enforce_language_agnostic_pages;
mod m20260721_000006_add_static_landing_materialization_evidence;
mod m20260721_000007_create_page_publish_operations;
mod m20260722_000009_create_page_rollback_operations;
mod m20260805_000010_create_page_route_aliases;
mod m20260806_000011_create_page_route_publications;
mod m20260806_000012_create_page_route_history_imports;
mod m20260806_000013_create_page_publish_rebuild_sources;
mod m20260806_000014_add_explicit_artifact_rebuild;
mod m20260806_000014_add_translation_target_support;
mod m20260807_000015_create_page_artifact_binding_replacements;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_pages_tables::Migration),
        Box::new(m20260329_000001_create_page_channel_visibility_table::Migration),
        Box::new(m20260714_000001_create_page_builder_scenario_baselines::Migration),
        Box::new(m20260714_000002_add_scenario_baseline_promotion_metadata::Migration),
        Box::new(m20260718_000002_create_static_landing_artifacts::Migration),
        Box::new(m20260721_000003_expand_pages_locale_storage_columns::Migration),
        Box::new(m20260721_000004_enforce_language_agnostic_pages::Migration),
        Box::new(m20260721_000006_add_static_landing_materialization_evidence::Migration),
        Box::new(m20260721_000007_create_page_publish_operations::Migration),
        Box::new(m20260722_000009_create_page_rollback_operations::Migration),
        Box::new(m20260805_000010_create_page_route_aliases::Migration),
        Box::new(m20260806_000011_create_page_route_publications::Migration),
        Box::new(m20260806_000012_create_page_route_history_imports::Migration),
        Box::new(m20260806_000013_create_page_publish_rebuild_sources::Migration),
        Box::new(m20260806_000014_add_explicit_artifact_rebuild::Migration),
        Box::new(m20260806_000014_add_translation_target_support::Migration),
        Box::new(m20260807_000015_create_page_artifact_binding_replacements::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260806_000014_add_translation_target_support",
        vec!["m20260803_000001_create_owner_operation_receipts"],
    )]
}

mod m20260328_000001_create_pages_tables;
mod m20260329_000001_create_page_channel_visibility_table;
mod m20260713_000001_enforce_pages_ordering_uniqueness;
mod m20260714_000001_create_page_builder_scenario_baselines;
mod m20260714_000002_add_scenario_baseline_promotion_metadata;
mod m20260718_000001_canonicalize_grapesjs_format;
mod m20260718_000002_create_static_landing_artifacts;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_pages_tables::Migration),
        Box::new(m20260329_000001_create_page_channel_visibility_table::Migration),
        Box::new(m20260713_000001_enforce_pages_ordering_uniqueness::Migration),
        Box::new(m20260714_000001_create_page_builder_scenario_baselines::Migration),
        Box::new(m20260714_000002_add_scenario_baseline_promotion_metadata::Migration),
        Box::new(m20260718_000001_canonicalize_grapesjs_format::Migration),
        Box::new(m20260718_000002_create_static_landing_artifacts::Migration),
    ]
}

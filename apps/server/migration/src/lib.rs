#![allow(elided_lifetimes_in_paths)]

pub use sea_orm_migration::prelude::*;

mod m20250101_000001_create_tenants;
mod m20250101_000002_create_users;
mod m20250101_000003_create_tenant_modules;
mod m20250101_000004_create_sessions;
mod m20250101_000005_create_roles_and_permissions;
mod m20250101_000006_add_metadata_to_tenants_and_users;
mod m20250130_000004_create_tenant_locales;
mod m20250130_000005_create_nodes;
mod m20250130_000006_create_categories;
mod m20250130_000007_create_tags;
mod m20250130_000008_create_meta;
mod m20250130_000009_create_media;
mod m20250130_000010_create_index_content;
mod m20250130_000010a_create_search_index;
mod m20250130_000011_create_index_products;
mod m20250130_000012_create_commerce_products;
mod m20250130_000013_create_commerce_options;
mod m20250130_000014_create_commerce_variants;
mod m20250130_000015_create_commerce_prices;
mod m20250130_000016_create_commerce_inventory;
mod m20250130_000017_create_commerce_collections;
mod m20250130_000018_create_commerce_categories;
mod m20250201_000001_alter_status_to_enums;
mod m20250212_000001_create_builds_and_releases;
mod m20260211_000001_add_event_versioning;
mod m20260211_000002_create_sys_events;
mod m20260301_000001_alter_product_variants_add_fields;
mod m20260301_000002_alter_nodes_add_soft_delete;
mod m20260302_000001_create_scripts;
mod m20260302_000002_create_script_executions;
mod m20260308_000001_create_oauth_apps;
mod m20260308_000002_create_oauth_tokens;
mod m20260308_000003_create_oauth_codes;
mod m20260308_000004_create_oauth_consents;
mod m20260311_000001_create_content_orchestration_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101_000001_create_tenants::Migration),
            Box::new(m20250101_000002_create_users::Migration),
            Box::new(m20250101_000003_create_tenant_modules::Migration),
            Box::new(m20250130_000004_create_tenant_locales::Migration),
            Box::new(m20250130_000005_create_nodes::Migration),
            Box::new(m20250130_000006_create_categories::Migration),
            Box::new(m20250130_000007_create_tags::Migration),
            Box::new(m20250130_000008_create_meta::Migration),
            Box::new(m20250130_000009_create_media::Migration),
            Box::new(m20250130_000010a_create_search_index::Migration),
            Box::new(m20250130_000012_create_commerce_products::Migration),
            Box::new(m20250130_000013_create_commerce_options::Migration),
            Box::new(m20250130_000014_create_commerce_variants::Migration),
            Box::new(m20250130_000015_create_commerce_prices::Migration),
            Box::new(m20250130_000016_create_commerce_inventory::Migration),
            Box::new(m20250130_000017_create_commerce_collections::Migration),
            Box::new(m20250130_000018_create_commerce_categories::Migration),
            Box::new(m20250130_000010_create_index_content::Migration),
            Box::new(m20250130_000011_create_index_products::Migration),
            Box::new(m20250101_000004_create_sessions::Migration),
            Box::new(m20250101_000005_create_roles_and_permissions::Migration),
            Box::new(m20250101_000006_add_metadata_to_tenants_and_users::Migration),
            Box::new(m20250201_000001_alter_status_to_enums::Migration),
            Box::new(m20260211_000001_add_event_versioning::Migration),
            Box::new(m20250212_000001_create_builds_and_releases::Migration),
            Box::new(m20260211_000002_create_sys_events::Migration),
            Box::new(m20260301_000001_alter_product_variants_add_fields::Migration),
            Box::new(m20260301_000002_alter_nodes_add_soft_delete::Migration),
            Box::new(m20260302_000001_create_scripts::Migration),
            Box::new(m20260302_000002_create_script_executions::Migration),
            Box::new(m20260308_000001_create_oauth_apps::Migration),
            Box::new(m20260308_000002_create_oauth_tokens::Migration),
            Box::new(m20260308_000003_create_oauth_codes::Migration),
            Box::new(m20260308_000004_create_oauth_consents::Migration),
            Box::new(m20260311_000001_create_content_orchestration_tables::Migration),
        ]
    }
}

#![allow(elided_lifetimes_in_paths)]

pub use sea_orm_migration::prelude::*;

// Platform-core migrations — tables that are always present regardless of which
// optional modules are installed: tenants, users, sessions, roles, permissions,
// tenant-module registry, tenant locales, builds/releases, platform settings.
mod m20250101_000001_create_tenants;
mod m20250101_000002_create_users;
mod m20250101_000003_create_tenant_modules;
mod m20250101_000004_create_sessions;
mod m20250101_000005_create_roles_and_permissions;
mod m20250101_000006_add_metadata_to_tenants_and_users;
mod m20250130_000004_create_tenant_locales;
mod m20250201_000001_alter_status_to_enums;
mod m20250212_000001_create_builds_and_releases;
mod m20260211_000001_add_event_versioning;
mod m20260211_000002_create_sys_events;
mod m20260315_000001_create_user_field_definitions;
mod m20260316_000001_create_platform_settings;
mod m20260316_000002_create_product_field_definitions;
mod m20260316_000003_create_node_field_definitions;
mod m20260316_000004_create_topic_field_definitions;
mod m20260316_000005_create_order_field_definitions;
mod m20260316_000006_create_workflows;
mod m20260316_000007_alter_workflows_add_failure_tracking;
mod m20260317_000001_create_flex_standalone_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        // Platform-core migrations (always required).
        let mut all: Vec<Box<dyn MigrationTrait>> = vec![
            Box::new(m20250101_000001_create_tenants::Migration),
            Box::new(m20250101_000002_create_users::Migration),
            Box::new(m20250101_000003_create_tenant_modules::Migration),
            Box::new(m20250130_000004_create_tenant_locales::Migration),
            Box::new(m20250101_000004_create_sessions::Migration),
            Box::new(m20250101_000005_create_roles_and_permissions::Migration),
            Box::new(m20250101_000006_add_metadata_to_tenants_and_users::Migration),
            Box::new(m20250201_000001_alter_status_to_enums::Migration),
            Box::new(m20250212_000001_create_builds_and_releases::Migration),
            Box::new(m20260211_000001_add_event_versioning::Migration),
            Box::new(m20260211_000002_create_sys_events::Migration),
            Box::new(m20260315_000001_create_user_field_definitions::Migration),
            Box::new(m20260316_000001_create_platform_settings::Migration),
            Box::new(m20260316_000002_create_product_field_definitions::Migration),
            Box::new(m20260316_000003_create_node_field_definitions::Migration),
            Box::new(m20260316_000004_create_topic_field_definitions::Migration),
            Box::new(m20260316_000005_create_order_field_definitions::Migration),
            Box::new(m20260316_000006_create_workflows::Migration),
            Box::new(m20260316_000007_alter_workflows_add_failure_tracking::Migration),
            Box::new(m20260317_000001_create_flex_standalone_tables::Migration),
        ];

        // Module-owned migrations — each module crate manages its own schema.
        // To add a new module: implement migrations() in the crate's migrations/ submodule
        // and add a call here.
        let module_migrations: Vec<Box<dyn MigrationTrait>> = [
            rustok_content::migrations::migrations(),
            rustok_index::migrations::migrations(),
            rustok_commerce::migrations::migrations(),
            rustok_auth::migrations::migrations(),
            alloy_scripting::migrations::migrations(),
            rustok_forum::migrations::migrations(),
            rustok_workflow::migrations::migrations(),
        ]
        .into_iter()
        .flatten()
        .collect();

        // Sort module migrations by name (date prefix) to ensure correct ordering.
        let mut module_migrations = module_migrations;
        module_migrations.sort_by(|a, b| a.name().cmp(b.name()));

        all.extend(module_migrations);
        all
    }
}

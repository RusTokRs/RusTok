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
mod m20260319_000001_create_mcp_management_tables;
mod m20260320_000001_create_mcp_scaffold_drafts;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        // Platform-core migrations plus module-owned migrations, sorted together
        // by migration name so test/runtime schema matches chronological intent.
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
            Box::new(m20260319_000001_create_mcp_management_tables::Migration),
            Box::new(m20260320_000001_create_mcp_scaffold_drafts::Migration),
        ];

        // Pull module-owned migrations from the domain crates and merge them into
        // the server migrator in chronological order.
        all.extend(alloy_scripting::migrations::migrations());
        all.extend(rustok_auth::migrations::migrations());
        all.extend(rustok_channel::migrations::migrations());
        all.extend(rustok_cart::migrations::migrations());
        all.extend(rustok_customer::migrations::migrations());
        all.extend(rustok_product::migrations::migrations());
        all.extend(rustok_region::migrations::migrations());
        all.extend(rustok_pricing::migrations::migrations());
        all.extend(rustok_inventory::migrations::migrations());
        all.extend(rustok_order::migrations::migrations());
        all.extend(rustok_payment::migrations::migrations());
        all.extend(rustok_fulfillment::migrations::migrations());
        all.extend(rustok_commerce::migrations::migrations());
        all.extend(rustok_content::migrations::migrations());
        all.extend(rustok_forum::migrations::migrations());
        all.extend(rustok_index::migrations::migrations());
        all.extend(rustok_workflow::migrations::migrations());
        all.sort_by(|a, b| a.name().cmp(b.name()));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::Migrator;
    use sea_orm_migration::MigratorTrait;

    #[test]
    fn migrator_includes_auth_migrations_in_sorted_order() {
        let names: Vec<String> = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .collect();

        assert!(
            names.contains(&"m20260308_000001_create_oauth_apps".to_string()),
            "server migrator must include oauth app migration"
        );
        assert!(
            names.contains(&"m20260308_000002_create_oauth_tokens".to_string()),
            "server migrator must include oauth token migration"
        );

        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(
            names, sorted,
            "server migrator must remain globally sorted by migration name"
        );
    }
}

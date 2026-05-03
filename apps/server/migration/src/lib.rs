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
mod m20260317_000001_create_flex_standalone_tables;
mod m20260319_000001_create_mcp_management_tables;
mod m20260320_000001_create_mcp_scaffold_drafts;
mod m20260403_000001_create_ai_control_plane_tables;
mod m20260403_000002_create_registry_publish_tables;
mod m20260403_000003_expand_ai_control_plane_for_multiprovider;
mod m20260403_000004_expand_ai_control_plane_for_direct_locale;
mod m20260403_000005_create_registry_module_owners;
mod m20260403_000006_add_sessions_active_lookup_index;
mod m20260403_000007_create_registry_governance_events;
mod m20260403_000008_add_registry_publish_request_publisher_identity;
mod m20260404_000001_create_registry_validation_jobs;
mod m20260404_000002_create_registry_validation_stages;
mod m20260405_000001_expand_locale_storage_columns;
mod m20260405_000002_split_flex_schema_localized_fields;
mod m20260405_000003_add_is_localized_to_server_field_definitions;
mod m20260405_000004_create_flex_attached_localized_values;
mod m20260408_000001_expand_registry_publish_request_governance_states;
mod m20260408_000002_expand_registry_validation_stage_runner_leases;
mod m20260410_000001_cleanup_flex_attached_legacy_inline_metadata;
mod m20260412_000001_reset_registry_identity_and_artifacts;
mod m20260412_000002_split_registry_localized_metadata;
mod m20260419_000001_normalize_registry_governance_event_payloads;
mod m20260426_000001_create_install_sessions;
mod m20260501_000001_create_platform_composition_state;

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
            Box::new(m20260317_000001_create_flex_standalone_tables::Migration),
            Box::new(m20260319_000001_create_mcp_management_tables::Migration),
            Box::new(m20260320_000001_create_mcp_scaffold_drafts::Migration),
            Box::new(m20260403_000001_create_ai_control_plane_tables::Migration),
            Box::new(m20260403_000002_create_registry_publish_tables::Migration),
            Box::new(m20260403_000003_expand_ai_control_plane_for_multiprovider::Migration),
            Box::new(m20260403_000004_expand_ai_control_plane_for_direct_locale::Migration),
            Box::new(m20260403_000005_create_registry_module_owners::Migration),
            Box::new(m20260403_000006_add_sessions_active_lookup_index::Migration),
            Box::new(m20260403_000007_create_registry_governance_events::Migration),
            Box::new(m20260403_000008_add_registry_publish_request_publisher_identity::Migration),
            Box::new(m20260404_000001_create_registry_validation_jobs::Migration),
            Box::new(m20260404_000002_create_registry_validation_stages::Migration),
            Box::new(m20260405_000001_expand_locale_storage_columns::Migration),
            Box::new(m20260405_000002_split_flex_schema_localized_fields::Migration),
            Box::new(m20260405_000003_add_is_localized_to_server_field_definitions::Migration),
            Box::new(m20260405_000004_create_flex_attached_localized_values::Migration),
            Box::new(m20260410_000001_cleanup_flex_attached_legacy_inline_metadata::Migration),
            Box::new(m20260408_000001_expand_registry_publish_request_governance_states::Migration),
            Box::new(m20260408_000002_expand_registry_validation_stage_runner_leases::Migration),
            Box::new(m20260412_000001_reset_registry_identity_and_artifacts::Migration),
            Box::new(m20260412_000002_split_registry_localized_metadata::Migration),
            Box::new(m20260419_000001_normalize_registry_governance_event_payloads::Migration),
            Box::new(m20260426_000001_create_install_sessions::Migration),
        ];

        // Pull module-owned migrations from the domain crates and merge them into
        // the server migrator in chronological order.
        all.extend(alloy::migrations::migrations());
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
        all.extend(rustok_blog::migrations::migrations());
        all.extend(rustok_comments::migrations::migrations());
        all.extend(rustok_pages::migrations::migrations());
        all.extend(rustok_seo::migrations::migrations());
        all.extend(rustok_forum::migrations::migrations());
        all.extend(rustok_index::migrations::migrations());
        all.extend(rustok_search::migrations::migrations());
        all.extend(rustok_taxonomy::migrations::migrations());
        all.extend(rustok_workflow::migrations::migrations());
        all.push(Box::new(
            m20260501_000001_create_platform_composition_state::Migration,
        ));
        all.sort_by(|a, b| a.name().cmp(b.name()));
        sort_migrations_by_dependencies(&mut all);
        all
    }
}

fn migration_dependencies(name: &str) -> &'static [&'static str] {
    match name {
        "m20260329_000001_create_product_tags" => &["m20260329_000001_create_taxonomy_tables"],
        _ => &[],
    }
}

fn sort_migrations_by_dependencies(
    migrations: &mut Vec<Box<dyn sea_orm_migration::MigrationTrait>>,
) {
    let mut sorted: Vec<Box<dyn sea_orm_migration::MigrationTrait>> =
        Vec::with_capacity(migrations.len());
    let mut remaining = std::mem::take(migrations);

    while !remaining.is_empty() {
        let before = remaining.len();
        let mut index = 0;
        while index < remaining.len() {
            let name = remaining[index].name().to_string();
            let deps_satisfied = migration_dependencies(&name).iter().all(|dependency| {
                sorted
                    .iter()
                    .any(|migration| migration.name() == *dependency)
            });
            if deps_satisfied {
                sorted.push(remaining.remove(index));
            } else {
                index += 1;
            }
        }

        if remaining.len() == before {
            sorted.append(&mut remaining);
        }
    }

    *migrations = sorted;
}

#[cfg(test)]
mod tests {
    use super::Migrator;
    use rustok_test_utils::setup_test_db;
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

        for name in &names {
            let migration_index = names
                .iter()
                .position(|candidate| candidate == name)
                .expect("migration exists");
            for dependency in super::migration_dependencies(name) {
                let dependency_index = names
                    .iter()
                    .position(|candidate| candidate == dependency)
                    .expect("dependency migration exists");
                assert!(
                    dependency_index < migration_index,
                    "migration {name} must run after dependency {dependency}"
                );
            }
        }
    }

    #[test]
    fn migrator_orders_taxonomy_before_product_tags() {
        let names: Vec<String> = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .collect();

        let taxonomy = names
            .iter()
            .position(|name| name == "m20260329_000001_create_taxonomy_tables")
            .expect("taxonomy migration must be present");
        let product_tags = names
            .iter()
            .position(|name| name == "m20260329_000001_create_product_tags")
            .expect("product tags migration must be present");

        assert!(
            taxonomy < product_tags,
            "taxonomy_terms must exist before product_tags adds its FK"
        );
    }

    #[test]
    fn migrator_includes_search_storage_migrations() {
        let names: Vec<String> = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .collect();

        assert!(
            names.contains(&"m20260324_000001_create_search_settings".to_string()),
            "server migrator must include search settings migration"
        );
        assert!(
            names.contains(&"m20260324_000002_create_search_documents".to_string()),
            "server migrator must include search documents migration"
        );
        assert!(
            names.contains(&"m20260325_000006_add_search_typo_tolerance_indexes".to_string()),
            "server migrator must include search typo-tolerance indexes migration"
        );
    }

    #[test]
    fn migrator_includes_content_module_migrations() {
        let names: Vec<String> = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .collect();

        assert!(
            names.contains(&"m20260328_000001_create_blog_post_tables".to_string()),
            "server migrator must include blog post tables"
        );
        assert!(
            names.contains(&"m20260328_000001_create_pages_tables".to_string()),
            "server migrator must include pages tables"
        );
        assert!(
            names.contains(&"m20260328_000001_create_comments_tables".to_string()),
            "server migrator must include comments tables"
        );
    }

    #[test]
    fn migrator_includes_foundation_locale_and_flex_multilingual_migrations() {
        let names: Vec<String> = Migrator::migrations()
            .into_iter()
            .map(|migration| migration.name().to_string())
            .collect();

        assert!(
            names.contains(&"m20260317_000001_create_flex_standalone_tables".to_string()),
            "server migrator must include flex standalone tables migration"
        );
        assert!(
            names.contains(&"m20260316_000004_create_topic_field_definitions".to_string()),
            "server migrator must include topic field definitions migration"
        );
        assert!(
            names.contains(&"m20260405_000001_expand_locale_storage_columns".to_string()),
            "server migrator must include locale storage widening migration"
        );
        assert!(
            names.contains(&"m20260405_000002_split_flex_schema_localized_fields".to_string()),
            "server migrator must include flex schema translation split migration"
        );
        assert!(
            names.contains(
                &"m20260405_000003_add_is_localized_to_server_field_definitions".to_string()
            ),
            "server migrator must include attached-mode field definition localization semantics migration"
        );
        assert!(
            names.contains(&"m20260405_000004_create_flex_attached_localized_values".to_string()),
            "server migrator must include attached localized value storage migration"
        );
        assert!(
            names.contains(
                &"m20260410_000001_cleanup_flex_attached_legacy_inline_metadata".to_string()
            ),
            "server migrator must include attached legacy metadata cleanup migration"
        );
    }

    #[tokio::test]
    #[ignore = "diagnostic helper for pinpointing the first SQLite-incompatible migration"]
    async fn sqlite_migrations_apply_incrementally() {
        let db = setup_test_db().await;

        loop {
            let pending = Migrator::get_pending_migrations(&db)
                .await
                .expect("pending migrations should load");
            if pending.is_empty() {
                break;
            }

            let next = pending[0].name().to_string();
            println!("applying {next}");
            if let Err(error) = Migrator::up(&db, Some(1)).await {
                panic!("sqlite incremental migrator failed at {next}: {error:?}");
            }
        }
    }
}

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
mod m20260407_000001_split_flex_entry_localized_values;
mod m20260408_000001_expand_registry_publish_request_governance_states;
mod m20260408_000002_expand_registry_validation_stage_runner_leases;
mod m20260410_000001_cleanup_flex_attached_legacy_inline_metadata;
mod m20260412_000001_reset_registry_identity_and_artifacts;
mod m20260412_000002_split_registry_localized_metadata;
mod m20260419_000001_normalize_registry_governance_event_payloads;
mod m20260426_000001_create_install_sessions;
mod m20260501_000001_create_platform_composition_state;
mod m20260522_000001_add_module_operation_correlation_id;
mod m20260714_900001_enforce_rbac_relation_tenant_integrity;

pub struct Migrator;

struct ModuleMigrationSource {
    slug: &'static str,
    source: &'static dyn rustok_core::MigrationSource,
}

static MODULE_MIGRATION_SOURCES: &[ModuleMigrationSource] = &[
    ModuleMigrationSource {
        slug: "alloy",
        source: &alloy::AlloyModule,
    },
    ModuleMigrationSource {
        slug: "auth",
        source: &rustok_auth::AuthModule,
    },
    ModuleMigrationSource {
        slug: "channel",
        source: &rustok_channel::ChannelModule,
    },
    ModuleMigrationSource {
        slug: "cart",
        source: &rustok_cart::CartModule,
    },
    ModuleMigrationSource {
        slug: "customer",
        source: &rustok_customer::CustomerModule,
    },
    ModuleMigrationSource {
        slug: "product",
        source: &rustok_product::ProductModule,
    },
    ModuleMigrationSource {
        slug: "region",
        source: &rustok_region::RegionModule,
    },
    ModuleMigrationSource {
        slug: "pricing",
        source: &rustok_pricing::PricingModule,
    },
    ModuleMigrationSource {
        slug: "inventory",
        source: &rustok_inventory::InventoryModule,
    },
    ModuleMigrationSource {
        slug: "order",
        source: &rustok_order::OrderModule,
    },
    ModuleMigrationSource {
        slug: "payment",
        source: &rustok_payment::PaymentModule,
    },
    ModuleMigrationSource {
        slug: "fulfillment",
        source: &rustok_fulfillment::FulfillmentModule,
    },
    ModuleMigrationSource {
        slug: "commerce",
        source: &rustok_commerce::CommerceModule,
    },
    ModuleMigrationSource {
        slug: "content",
        source: &rustok_content::ContentModule,
    },
    ModuleMigrationSource {
        slug: "blog",
        source: &rustok_blog::BlogModule,
    },
    ModuleMigrationSource {
        slug: "comments",
        source: &rustok_comments::CommentsModule,
    },
    ModuleMigrationSource {
        slug: "pages",
        source: &rustok_pages::PagesModule,
    },
    ModuleMigrationSource {
        slug: "seo",
        source: &rustok_seo::SeoModule,
    },
    ModuleMigrationSource {
        slug: "forum",
        source: &rustok_forum::ForumModule,
    },
    ModuleMigrationSource {
        slug: "index",
        source: &rustok_index::IndexModule,
    },
    ModuleMigrationSource {
        slug: "search",
        source: &rustok_search::SearchModule,
    },
    ModuleMigrationSource {
        slug: "taxonomy",
        source: &rustok_taxonomy::TaxonomyModule,
    },
    ModuleMigrationSource {
        slug: "workflow",
        source: &rustok_workflow::WorkflowModule,
    },
];

fn module_migration_sources() -> &'static [ModuleMigrationSource] {
    MODULE_MIGRATION_SOURCES
}

fn collect_migration_descriptors() -> Vec<MigrationDescriptor> {
    // Module-owned dependency metadata collection point.
    // Keep descriptors behind the MigrationSource contract for every module whose
    // migrations are included in this server migrator. Modules without
    // cross-module ordering metadata use the trait default.
    module_migration_sources()
        .iter()
        .flat_map(|module| {
            let _module_slug = module.slug;
            module_dependency_descriptors(module.source.migration_dependencies())
        })
        .collect()
}

fn module_dependency_descriptors(
    descriptors: Vec<rustok_core::MigrationDependencyDescriptor>,
) -> impl Iterator<Item = MigrationDescriptor> {
    descriptors.into_iter().map(MigrationDescriptor::from)
}

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
            Box::new(m20260407_000001_split_flex_entry_localized_values::Migration),
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
        all.push(Box::new(
            m20260522_000001_add_module_operation_correlation_id::Migration,
        ));
        all.push(Box::new(
            m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration,
        ));
        let dependencies = collect_migration_descriptors();

        all.sort_by(|a, b| a.name().cmp(b.name()));
        sort_migrations_by_dependencies(&mut all, &dependencies)
            .expect("migration dependency descriptors must be valid");
        all
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDescriptor {
    migration: String,
    after: Vec<String>,
}

impl MigrationDescriptor {
    pub fn new(
        migration: impl Into<String>,
        after: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            migration: migration.into(),
            after: after.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<rustok_core::MigrationDependencyDescriptor> for MigrationDescriptor {
    fn from(descriptor: rustok_core::MigrationDependencyDescriptor) -> Self {
        Self {
            migration: descriptor.migration.to_string(),
            after: descriptor.after.into_iter().map(str::to_string).collect(),
        }
    }
}

fn sort_migrations_by_dependencies(
    migrations: &mut Vec<Box<dyn sea_orm_migration::MigrationTrait>>,
    descriptors: &[MigrationDescriptor],
) -> Result<(), String> {
    let names = migrations
        .iter()
        .map(|migration| migration.name().to_string())
        .collect::<std::collections::BTreeSet<_>>();

    for descriptor in descriptors {
        if !names.contains(&descriptor.migration) {
            return Err(format!(
                "migration descriptor references missing migration {}",
                descriptor.migration
            ));
        }
        for dependency in &descriptor.after {
            if !names.contains(dependency) {
                return Err(format!(
                    "migration {} depends on missing migration {}",
                    descriptor.migration, dependency
                ));
            }
        }
    }

    let mut after_by_name = std::collections::BTreeMap::<String, Vec<String>>::new();
    for descriptor in descriptors.iter().cloned() {
        if after_by_name
            .insert(descriptor.migration.clone(), descriptor.after)
            .is_some()
        {
            return Err(format!(
                "duplicate migration descriptor for {}",
                descriptor.migration
            ));
        }
    }

    let mut sorted: Vec<Box<dyn sea_orm_migration::MigrationTrait>> =
        Vec::with_capacity(migrations.len());
    let mut remaining = std::mem::take(migrations);

    while !remaining.is_empty() {
        let before = remaining.len();
        let mut index = 0;
        while index < remaining.len() {
            let name = remaining[index].name().to_string();
            let deps_satisfied = after_by_name
                .get(&name)
                .into_iter()
                .flatten()
                .all(|dependency| {
                    sorted
                        .iter()
                        .any(|migration| migration.name() == dependency.as_str())
                });
            if deps_satisfied {
                sorted.push(remaining.remove(index));
            } else {
                index += 1;
            }
        }

        if remaining.len() == before {
            let cycle = remaining
                .iter()
                .map(|migration| migration.name().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "migration dependency cycle or unsatisfied dependency: {cycle}"
            ));
        }
    }

    *migrations = sorted;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{sort_migrations_by_dependencies, MigrationDescriptor, Migrator};
    use rustok_test_utils::setup_test_db;
    use sea_orm_migration::MigratorTrait;

    #[test]
    fn module_migration_sources_cover_server_module_crates() {
        let slugs = super::module_migration_sources()
            .iter()
            .map(|module| module.slug)
            .collect::<Vec<_>>();

        assert_eq!(
            slugs,
            vec![
                "alloy",
                "auth",
                "channel",
                "cart",
                "customer",
                "product",
                "region",
                "pricing",
                "inventory",
                "order",
                "payment",
                "fulfillment",
                "commerce",
                "content",
                "blog",
                "comments",
                "pages",
                "seo",
                "forum",
                "index",
                "search",
                "taxonomy",
                "workflow",
            ],
            "descriptor aggregation must cover every module crate whose migrations are included in the server migrator"
        );
    }

    #[test]
    fn dependency_sort_rejects_missing_dependency() {
        let mut migrations: Vec<Box<dyn sea_orm_migration::MigrationTrait>> = vec![
            Box::new(super::m20250101_000001_create_tenants::Migration),
            Box::new(super::m20250101_000002_create_users::Migration),
        ];
        let descriptors = vec![MigrationDescriptor::new(
            "m20250101_000002_create_users",
            ["m99999999_000001_missing_dep"],
        )];

        let err = sort_migrations_by_dependencies(&mut migrations, &descriptors)
            .expect_err("missing dependency must fail");
        assert!(
            err.contains("depends on missing migration"),
            "unexpected error: {err}"
        );
    }
}

mod shared;

mod m20260308_000001_create_oauth_apps;
mod m20260308_000002_create_oauth_tokens;
mod m20260308_000003_create_oauth_codes;
mod m20260308_000004_create_oauth_consents;
mod m20260329_000001_add_oauth_app_granted_permissions;
mod m20260424_000001_rename_legacy_oauth_tables;
mod m20260713_000001_create_auth_invite_consumptions;
mod m20260716_000001_create_flex_field_definition_cache_generation;
mod m20260720_000002_enforce_oauth_tenant_integrity;
mod m20260721_000009_move_oauth_app_copy_to_translations;
mod m20260913_000010_add_oauth_app_translation_target;
mod m20261025_000002_oauth_consent_partial_unique;
mod m20261025_000004_create_auth_invites;
mod m20261101_000002_add_oauth_token_authorization_code;
mod m20261101_000003_add_oauth_token_kind_and_consent_epoch;
mod m20261125_000002_create_auth_jwt_generation_fence;
mod m20261125_000003_add_auth_jwt_cluster_lease;
mod m20261125_000004_add_auth_jwt_replica_grants;
mod m20261126_000001_create_auth_email_change_requests;
mod m20261126_000060_enforce_oauth_family_identity;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    let migrations: Vec<Box<dyn MigrationTrait>> = vec![
        Box::new(m20260308_000001_create_oauth_apps::Migration),
        Box::new(m20260308_000002_create_oauth_tokens::Migration),
        Box::new(m20260308_000003_create_oauth_codes::Migration),
        Box::new(m20260308_000004_create_oauth_consents::Migration),
        Box::new(m20260329_000001_add_oauth_app_granted_permissions::Migration),
        Box::new(m20260424_000001_rename_legacy_oauth_tables::Migration),
        Box::new(m20260713_000001_create_auth_invite_consumptions::Migration),
        Box::new(m20260716_000001_create_flex_field_definition_cache_generation::Migration),
        Box::new(m20260720_000002_enforce_oauth_tenant_integrity::Migration),
        Box::new(m20260721_000009_move_oauth_app_copy_to_translations::Migration),
        Box::new(m20260913_000010_add_oauth_app_translation_target::Migration),
        Box::new(m20261025_000002_oauth_consent_partial_unique::Migration),
        Box::new(m20261025_000004_create_auth_invites::Migration),
        Box::new(m20261101_000002_add_oauth_token_authorization_code::Migration),
        Box::new(m20261101_000003_add_oauth_token_kind_and_consent_epoch::Migration),
    ];
    let mut migrations = migrations;
    migrations.extend(jwt_fence_migrations());
    migrations.push(Box::new(m20261126_000001_create_auth_email_change_requests::Migration));
    migrations.push(Box::new(m20261126_000060_enforce_oauth_family_identity::Migration));
    migrations
}

/// Reuse the exact Auth-owned schema in the independently provisioned store.
pub(crate) fn jwt_fence_migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20261125_000002_create_auth_jwt_generation_fence::Migration),
        Box::new(m20261125_000003_add_auth_jwt_cluster_lease::Migration),
        Box::new(m20261125_000004_add_auth_jwt_replica_grants::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260716_000001_create_flex_field_definition_cache_generation",
            vec!["m20260716_000000_create_field_definition_cache_generation"],
        ),
        MigrationDependencyDescriptor::new(
            "m20261126_000060_enforce_oauth_family_identity",
            vec![
                "m20261025_000005_add_oauth_token_family",
                "m20261101_000003_add_oauth_token_kind_and_consent_epoch",
                "m20261101_000002_add_oauth_token_authorization_code",
            ],
        ),
    ]
}
